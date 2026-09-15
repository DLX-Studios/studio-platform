//! sb-daemon — Studio Builder in-guest daemon.
//!
//! The daemon exposes a small authenticated HTTP API for probes and a
//! persistent Pi RPC bridge. Pi remains alive between browser requests, so
//! conversation context and tool state are not lost after every prompt.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, Command as TokioCommand};
use tokio::sync::{Mutex, RwLock, broadcast};

const POLL_INTERVAL: Duration = Duration::from_secs(15);
const ERROR_BACKOFF: Duration = Duration::from_secs(30);
const LISTEN_PORT: u16 = 4545;
const MAX_REQUEST_BYTES: usize = 1024 * 1024;
const MAX_REPLAY_EVENTS: usize = 4_000;
const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize)]
struct Probe {
    hostname: String,
    daemon_version: &'static str,
    uptime_secs: u64,
    disk_root_free_gb: Option<f64>,
    disk_workspace_free_gb: Option<f64>,
    rustc_version: Option<String>,
    cargo_version: Option<String>,
    sccache_version: Option<String>,
    pi_version: Option<String>,
    os: Option<String>,
}

#[derive(Debug, Serialize)]
struct HelloBody {
    #[serde(flatten)]
    probe: Probe,
}

#[derive(Debug, Deserialize)]
struct HelloResponse {
    #[allow(dead_code)]
    ok: bool,
    machine_id: Option<i64>,
    #[serde(default)]
    commands: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
struct SequencedEvent {
    seq: u64,
    event: Value,
}

struct AgentBridge {
    input: Mutex<Option<ChildStdin>>,
    start_lock: Mutex<()>,
    events: RwLock<VecDeque<SequencedEvent>>,
    bus: broadcast::Sender<SequencedEvent>,
    next_seq: AtomicU64,
    running: AtomicBool,
    workspace: PathBuf,
    session_dir: PathBuf,
}

impl AgentBridge {
    fn new() -> Arc<Self> {
        let (bus, _) = broadcast::channel(512);
        Arc::new(Self {
            input: Mutex::new(None),
            start_lock: Mutex::new(()),
            events: RwLock::new(VecDeque::with_capacity(MAX_REPLAY_EVENTS)),
            bus,
            next_seq: AtomicU64::new(1),
            running: AtomicBool::new(false),
            workspace: std::env::var("SB_WORKSPACE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/root/workspace/studio")),
            session_dir: std::env::var("SB_PI_SESSION_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/root/.studio-builder/pi-sessions")),
        })
    }

    async fn emit(&self, event: Value) {
        let item = SequencedEvent {
            seq: self.next_seq.fetch_add(1, Ordering::Relaxed),
            event,
        };
        let mut events = self.events.write().await;
        if events.len() == MAX_REPLAY_EVENTS {
            events.pop_front();
        }
        events.push_back(item.clone());
        drop(events);
        let _ = self.bus.send(item);
    }

    async fn ensure_started(self: &Arc<Self>) -> Result<()> {
        if self.running.load(Ordering::Acquire) {
            return Ok(());
        }
        let _guard = self.start_lock.lock().await;
        if self.running.load(Ordering::Acquire) {
            return Ok(());
        }

        std::fs::create_dir_all(&self.session_dir).context("create Pi session directory")?;
        let configured_bin = std::env::var("SB_PI_BIN").ok();
        let pi_bin = configured_bin.as_deref().unwrap_or_else(|| {
            if Path::new("/root/.local/bin/pi").exists() {
                "/root/.local/bin/pi"
            } else {
                "pi"
            }
        });
        let mut command = TokioCommand::new(pi_bin);
        command
            .arg("--mode")
            .arg("rpc")
            .arg("--continue")
            .arg("--session-dir")
            .arg(&self.session_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(false);
        if self.workspace.is_dir() {
            command.current_dir(&self.workspace);
        }
        if let Ok(model) = std::env::var("SB_PI_MODEL") {
            if !model.trim().is_empty() {
                command.arg("--model").arg(model);
            }
        }

        let mut child = command
            .spawn()
            .with_context(|| format!("start {pi_bin} --mode rpc"))?;
        let stdin = child.stdin.take().context("Pi stdin unavailable")?;
        let stdout = child.stdout.take().context("Pi stdout unavailable")?;
        let stderr = child.stderr.take().context("Pi stderr unavailable")?;
        *self.input.lock().await = Some(stdin);
        self.running.store(true, Ordering::Release);
        self.emit(json!({"type":"bridge_status","state":"connected"}))
            .await;

        let stdout_bridge = Arc::clone(self);
        tokio::spawn(async move {
            let mut records = BufReader::new(stdout).split(b'\n');
            while let Ok(Some(mut bytes)) = records.next_segment().await {
                if bytes.last() == Some(&b'\r') {
                    bytes.pop();
                }
                if bytes.is_empty() {
                    continue;
                }
                match serde_json::from_slice::<Value>(&bytes) {
                    Ok(event) => stdout_bridge.emit(event).await,
                    Err(error) => {
                        stdout_bridge
                            .emit(json!({
                                "type":"bridge_error",
                                "message": format!("Invalid Pi RPC record: {error}")
                            }))
                            .await;
                    }
                }
            }
        });

        let stderr_bridge = Arc::clone(self);
        tokio::spawn(async move {
            let mut records = BufReader::new(stderr).split(b'\n');
            while let Ok(Some(bytes)) = records.next_segment().await {
                if !bytes.is_empty() {
                    let message = String::from_utf8_lossy(&bytes);
                    stderr_bridge
                        .emit(json!({"type":"bridge_log","level":"error","message":message}))
                        .await;
                }
            }
        });

        let wait_bridge = Arc::clone(self);
        tokio::spawn(async move {
            let status = child.wait().await;
            wait_bridge.running.store(false, Ordering::Release);
            *wait_bridge.input.lock().await = None;
            wait_bridge
                .emit(json!({
                    "type":"bridge_status",
                    "state":"disconnected",
                    "message": match status {
                        Ok(value) => format!("Pi exited with {value}"),
                        Err(error) => format!("Could not wait for Pi: {error}"),
                    }
                }))
                .await;
        });

        self.write_rpc(json!({"id":"studio-bootstrap-state","type":"get_state"}))
            .await?;
        self.write_rpc(json!({"id":"studio-bootstrap-messages","type":"get_messages"}))
            .await?;
        self.write_rpc(json!({"id":"studio-bootstrap-models","type":"get_available_models"}))
            .await?;
        self.write_rpc(
            json!({"id":"studio-bootstrap-thinking","type":"get_available_thinking_levels"}),
        )
        .await?;
        Ok(())
    }

    async fn write_rpc(&self, command: Value) -> Result<()> {
        let mut input = self.input.lock().await;
        let stdin = input.as_mut().context("Pi RPC process is not connected")?;
        let mut record = serde_json::to_vec(&command)?;
        record.push(b'\n');
        stdin
            .write_all(&record)
            .await
            .context("write Pi RPC command")?;
        stdin.flush().await.context("flush Pi RPC command")?;
        Ok(())
    }

    async fn command(self: &Arc<Self>, mut command: Value) -> Result<()> {
        self.ensure_started().await?;
        let kind = command.get("type").and_then(Value::as_str).unwrap_or("");
        const ALLOWED: &[&str] = &[
            "prompt",
            "steer",
            "follow_up",
            "abort",
            "clear_queue",
            "new_session",
            "get_state",
            "get_messages",
            "get_available_models",
            "get_available_thinking_levels",
            "set_model",
            "set_thinking_level",
            "compact",
        ];
        if !ALLOWED.contains(&kind) {
            anyhow::bail!("unsupported Pi RPC command: {kind}");
        }
        if command.get("id").is_none() {
            command["id"] =
                Value::String(format!("studio-{}", self.next_seq.load(Ordering::Relaxed)));
        }
        self.write_rpc(command).await
    }

    async fn replay_after(&self, after: u64) -> Vec<SequencedEvent> {
        self.events
            .read()
            .await
            .iter()
            .filter(|event| event.seq > after)
            .cloned()
            .collect()
    }
}

struct HttpRequest {
    method: String,
    target: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let url = std::env::var("SB_DAEMON_URL").unwrap_or_default();
    let token = std::env::var("SB_DAEMON_TOKEN").context("SB_DAEMON_TOKEN is not set")?;
    let base = url.trim_end_matches('/').to_string();
    let agent = AgentBridge::new();

    println!("[sb-daemon] v{DAEMON_VERSION} starting");
    let listener_token = token.clone();
    tokio::spawn(async move {
        if let Err(error) = listener(listener_token, agent).await {
            eprintln!("[sb-daemon] listener crashed: {error}");
        }
    });

    if !base.is_empty() {
        println!("[sb-daemon] dialing home: {base}");
        run_dial_home(&base, &token).await
    } else {
        println!("[sb-daemon] SB_DAEMON_URL not set — pull mode only");
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    }
}

async fn listener(token: String, agent: Arc<AgentBridge>) -> Result<()> {
    let addr = format!("0.0.0.0:{LISTEN_PORT}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("[sb-daemon] listening on {addr} (probe + Pi RPC bridge)");

    loop {
        let (mut socket, peer) = listener.accept().await?;
        let token = token.clone();
        let agent = Arc::clone(&agent);
        tokio::spawn(async move {
            let request = match read_request(&mut socket).await {
                Ok(request) => request,
                Err(error) => {
                    let _ = write_json(
                        &mut socket,
                        "400 Bad Request",
                        &json!({"error":error.to_string()}),
                    )
                    .await;
                    return;
                }
            };
            let authorized = request
                .headers
                .get("authorization")
                .is_some_and(|value| value == &format!("Bearer {token}"));
            if !authorized {
                eprintln!("[sb-daemon] unauthorized request from {peer}");
                let _ = write_json(
                    &mut socket,
                    "401 Unauthorized",
                    &json!({"error":"unauthorized"}),
                )
                .await;
                return;
            }

            let path = request.target.split('?').next().unwrap_or("");
            match (request.method.as_str(), path) {
                ("GET", "/probe") => {
                    let _ = write_json(&mut socket, "200 OK", &collect_probe()).await;
                }
                ("GET", "/agent/events") => {
                    if let Err(error) = stream_agent_events(&mut socket, &request, agent).await {
                        eprintln!("[sb-daemon] agent event stream closed: {error}");
                    }
                }
                ("POST", "/agent/command") => {
                    let command = serde_json::from_slice::<Value>(&request.body);
                    match command {
                        Ok(command) => match agent.command(command).await {
                            Ok(()) => {
                                let _ =
                                    write_json(&mut socket, "202 Accepted", &json!({"ok":true}))
                                        .await;
                            }
                            Err(error) => {
                                agent
                                    .emit(
                                        json!({"type":"bridge_error","message":error.to_string()}),
                                    )
                                    .await;
                                let _ = write_json(
                                    &mut socket,
                                    "503 Service Unavailable",
                                    &json!({"error":error.to_string()}),
                                )
                                .await;
                            }
                        },
                        Err(error) => {
                            let _ = write_json(
                                &mut socket,
                                "400 Bad Request",
                                &json!({"error":error.to_string()}),
                            )
                            .await;
                        }
                    }
                }
                _ => {
                    let _ = write_json(&mut socket, "404 Not Found", &json!({"error":"not found"}))
                        .await;
                }
            }
        });
    }
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> Result<HttpRequest> {
    let mut buffer = Vec::with_capacity(8192);
    let mut chunk = [0_u8; 4096];
    let header_end;
    loop {
        let count = socket.read(&mut chunk).await?;
        if count == 0 {
            anyhow::bail!("connection closed before request completed");
        }
        buffer.extend_from_slice(&chunk[..count]);
        if buffer.len() > MAX_REQUEST_BYTES {
            anyhow::bail!("request is too large");
        }
        if let Some(index) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            header_end = index + 4;
            break;
        }
    }

    let header_text =
        std::str::from_utf8(&buffer[..header_end]).context("request headers are not UTF-8")?;
    let mut lines = header_text.split("\r\n");
    let mut request_line = lines.next().unwrap_or("").split_whitespace();
    let method = request_line.next().unwrap_or("").to_string();
    let target = request_line.next().unwrap_or("").to_string();
    let headers: HashMap<String, String> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    if header_end + content_length > MAX_REQUEST_BYTES {
        anyhow::bail!("request body is too large");
    }
    while buffer.len() < header_end + content_length {
        let count = socket.read(&mut chunk).await?;
        if count == 0 {
            anyhow::bail!("connection closed before body completed");
        }
        buffer.extend_from_slice(&chunk[..count]);
    }
    Ok(HttpRequest {
        method,
        target,
        headers,
        body: buffer[header_end..header_end + content_length].to_vec(),
    })
}

async fn write_json<T: Serialize>(
    socket: &mut tokio::net::TcpStream,
    status: &str,
    value: &T,
) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    socket.write_all(headers.as_bytes()).await?;
    socket.write_all(&body).await?;
    socket.flush().await?;
    Ok(())
}

async fn stream_agent_events(
    socket: &mut tokio::net::TcpStream,
    request: &HttpRequest,
    agent: Arc<AgentBridge>,
) -> Result<()> {
    if let Err(error) = agent.ensure_started().await {
        agent
            .emit(json!({"type":"bridge_error","message":error.to_string()}))
            .await;
    }
    let query_after = request
        .target
        .split_once('?')
        .and_then(|(_, query)| {
            query
                .split('&')
                .find_map(|part| part.strip_prefix("after="))
        })
        .and_then(|value| value.parse::<u64>().ok());
    let after = query_after
        .or_else(|| {
            request
                .headers
                .get("last-event-id")
                .and_then(|value| value.parse().ok())
        })
        .unwrap_or(0);
    socket
        .write_all(
            // With no Content-Length the response is deliberately
            // close-delimited. The dashboard keeps reading until the client
            // disconnects, then EventSource reconnects with Last-Event-ID.
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache, no-transform\r\nConnection: close\r\nX-Accel-Buffering: no\r\n\r\n",
        )
        .await?;
    // Subscribe before taking the replay snapshot so no event can fall into
    // the gap between history delivery and live delivery.
    let mut receiver = agent.bus.subscribe();
    let replay = agent.replay_after(after).await;
    let replay_tip = replay.last().map(|event| event.seq).unwrap_or(after);
    for event in replay {
        write_sse(socket, &event).await?;
    }
    let mut heartbeat = tokio::time::interval(Duration::from_secs(15));
    loop {
        tokio::select! {
            received = receiver.recv() => match received {
                Ok(event) if event.seq > replay_tip => write_sse(socket, &event).await?,
                Ok(_) => {},
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    socket.write_all(b"event: reset\ndata: {}\n\n").await?;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
            _ = heartbeat.tick() => {
                socket.write_all(b": keep-alive\n\n").await?;
                socket.flush().await?;
            }
        }
    }
    Ok(())
}

async fn write_sse(socket: &mut tokio::net::TcpStream, event: &SequencedEvent) -> Result<()> {
    let data = serde_json::to_string(event)?;
    socket
        .write_all(format!("id: {}\ndata: {data}\n\n", event.seq).as_bytes())
        .await?;
    socket.flush().await?;
    Ok(())
}

async fn run_dial_home(base: &str, token: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let mut consecutive_errors = 0_u32;
    loop {
        let probe = collect_probe();
        match report(&client, base, token, &probe).await {
            Ok(response) => {
                if consecutive_errors > 0 {
                    println!("[sb-daemon] connection re-established");
                }
                consecutive_errors = 0;
                if let Some(id) = response.machine_id {
                    println!("[sb-daemon] hello acknowledged (machine {id})");
                }
                if !response.commands.is_empty() {
                    println!(
                        "[sb-daemon] dashboard sent {} legacy commands",
                        response.commands.len()
                    );
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
            Err(error) => {
                consecutive_errors += 1;
                eprintln!("[sb-daemon] dial-home failed ({error}), backing off…");
                tokio::time::sleep(ERROR_BACKOFF).await;
            }
        }
    }
}

async fn report(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    probe: &Probe,
) -> Result<HelloResponse> {
    let response = client
        .post(format!("{base}/api/daemon/hello"))
        .bearer_auth(token)
        .json(&HelloBody {
            probe: probe.clone(),
        })
        .send()
        .await
        .context("request failed")?;
    let status = response.status();
    let body: HelloResponse = response.json().await.context("invalid JSON response")?;
    if !status.is_success() {
        anyhow::bail!("server returned {status}");
    }
    Ok(body)
}

fn collect_probe() -> Probe {
    Probe {
        hostname: read_first_line("/etc/hostname").unwrap_or_else(|_| "unknown".into()),
        daemon_version: DAEMON_VERSION,
        uptime_secs: uptime_secs(),
        disk_root_free_gb: disk_free_gb("/"),
        disk_workspace_free_gb: disk_free_gb("/root/workspace"),
        rustc_version: version_of("rustc"),
        cargo_version: version_of("cargo"),
        sccache_version: version_of("sccache"),
        pi_version: version_of("pi"),
        os: read_first_line("/etc/os-release").ok().and_then(|line| {
            line.strip_prefix("PRETTY_NAME=")
                .map(|value| value.trim_matches('"').to_string())
        }),
    }
}

fn read_first_line(path: &str) -> Result<String> {
    Ok(std::fs::read_to_string(path)?
        .lines()
        .next()
        .unwrap_or_default()
        .to_string())
}

fn uptime_secs() -> u64 {
    read_first_line("/proc/uptime")
        .ok()
        .and_then(|value| value.split_whitespace().next()?.parse().ok())
        .unwrap_or(0)
}

fn disk_free_gb(path: &str) -> Option<f64> {
    let output = Command::new("df").arg("-BG").arg(path).output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    stdout
        .lines()
        .last()?
        .split_whitespace()
        .nth(3)?
        .trim_end_matches('G')
        .parse()
        .ok()
}

fn version_of(binary: &str) -> Option<String> {
    let output = Command::new(binary).arg("--version").output().ok()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(String::from)
}
