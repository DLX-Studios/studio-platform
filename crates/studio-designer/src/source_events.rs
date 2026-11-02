//! Bounded local relay for file-change notifications from Designer's MCP host.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::Digest;
use studio_protocol::dev_rpc::{self, DevRpcListener, DevRpcStream};

/// Socket filename used to deliver source events to an open Designer window.
pub const SOCKET_NAME: &str = ".studio-designer.sock";

/// Maximum encoded event frame size, including the newline terminator.
pub const MAX_EVENT_FRAME_BYTES: usize = 32 * 1024 * 1024;

const MAX_SOCKET_PATH_BYTES: usize = 100;
const MAX_EVENT_QUEUE: usize = 8;
const MAX_CLIENT_NAME_BYTES: usize = 80;
const MAX_EVENT_PATH_BYTES: usize = 4_096;
const EVENT_METHOD: &str = "studio.designer.source.changed";
const SOCKET_POLL_INTERVAL: Duration = Duration::from_millis(25);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);

/// A successful MCP file edit observed by the Designer window.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceChangeEvent {
    /// Project-relative path to the edited file.
    pub path: String,
    /// SHA-256 revision of the prior file, absent for a create-only write.
    pub before_revision: Option<String>,
    /// Exact prior UTF-8 file contents, absent for a create-only write.
    pub before_content: Option<String>,
    /// SHA-256 revision after the successful write.
    pub after_revision: String,
    /// Sanitized MCP client name. This is display metadata, not an identity.
    pub client_name: String,
}

impl SourceChangeEvent {
    /// Construct a file event with bounded, display-safe client attribution.
    #[must_use]
    pub fn new(
        path: impl Into<String>,
        before_revision: Option<String>,
        before_content: Option<String>,
        after_revision: impl Into<String>,
        client_name: Option<&str>,
    ) -> Self {
        Self {
            path: path.into(),
            before_revision,
            before_content,
            after_revision: after_revision.into(),
            client_name: sanitize_client_name(client_name),
        }
    }
}

/// Return a compact display name that cannot inject controls into the UI.
#[must_use]
pub fn sanitize_client_name(name: Option<&str>) -> String {
    let Some(name) = name else {
        return "MCP client".to_owned();
    };
    let mut sanitized = String::with_capacity(MAX_CLIENT_NAME_BYTES);
    let mut previous_was_space = true;
    for character in name.chars() {
        let character = if character.is_control() {
            ' '
        } else {
            character
        };
        if character.is_whitespace() {
            if !previous_was_space && sanitized.len() < MAX_CLIENT_NAME_BYTES {
                sanitized.push(' ');
            }
            previous_was_space = true;
            continue;
        }
        if sanitized.len() + character.len_utf8() > MAX_CLIENT_NAME_BYTES {
            break;
        }
        sanitized.push(character);
        previous_was_space = false;
    }
    let sanitized = sanitized.trim();
    if sanitized.is_empty() {
        "MCP client".to_owned()
    } else {
        sanitized.to_owned()
    }
}

/// Resolve the endpoint for a canonical project root.
///
/// The preferred path is `<root>/build/.studio-designer.sock`. If it exceeds
/// the local socket path budget, a deterministic hashed filename in the
/// system temporary directory is used, matching Studio's other local IPC
/// endpoints.
#[must_use]
pub fn endpoint_path(project_root: &Path) -> PathBuf {
    let canonical_root =
        std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    let build_dir = canonical_root.join("build");
    let direct = build_dir.join(SOCKET_NAME);
    if cfg!(unix) && direct.as_os_str().as_encoded_bytes().len() <= MAX_SOCKET_PATH_BYTES {
        return direct;
    }
    let hash = stable_path_hash(&build_dir);
    std::env::temp_dir().join(format!("studio-designer-{hash:016x}.sock"))
}

/// Listener and bounded event queue owned by one open Designer project view.
pub struct SourceEventRelay {
    endpoint: PathBuf,
    receiver: Receiver<SourceChangeEvent>,
    stopped: Arc<AtomicBool>,
    dropped_events: Arc<AtomicUsize>,
    worker: Option<JoinHandle<()>>,
}

impl SourceEventRelay {
    /// Listen beside one project and collect successful agent file edits.
    pub fn start(project_root: &Path) -> io::Result<Self> {
        let canonical_root = std::fs::canonicalize(project_root)?;
        if !canonical_root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "source event root is not a directory",
            ));
        }
        let build_dir = canonical_root.join("build");
        std::fs::create_dir_all(&build_dir)?;
        let endpoint = endpoint_path(&canonical_root);
        if dev_rpc::connect_endpoint(&endpoint).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "a Designer source event relay already owns this project",
            ));
        }
        let listener = dev_rpc::bind_endpoint(&endpoint)?;
        if let Err(error) = listener.set_nonblocking(true) {
            let _ = std::fs::remove_file(&endpoint);
            return Err(error);
        }

        let (sender, receiver) = mpsc::sync_channel(MAX_EVENT_QUEUE);
        let stopped = Arc::new(AtomicBool::new(false));
        let dropped_events = Arc::new(AtomicUsize::new(0));
        let worker_stopped = Arc::clone(&stopped);
        let worker_dropped_events = Arc::clone(&dropped_events);
        let worker = match thread::Builder::new()
            .name("studio-source-events".to_owned())
            .spawn(move || {
                serve_events(listener, sender, worker_stopped, worker_dropped_events);
            }) {
            Ok(worker) => worker,
            Err(error) => {
                let _ = std::fs::remove_file(&endpoint);
                return Err(error);
            }
        };

        Ok(Self {
            endpoint,
            receiver,
            stopped,
            dropped_events,
            worker: Some(worker),
        })
    }

    /// Return the endpoint path shared with the MCP process.
    #[must_use]
    pub fn endpoint_path(&self) -> &Path {
        &self.endpoint
    }

    /// Poll for the next event without blocking.
    pub fn try_recv(&self) -> Result<SourceChangeEvent, TryRecvError> {
        self.receiver.try_recv()
    }

    /// Wait for one event for at most `timeout`.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<SourceChangeEvent, RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }

    /// Return how many events were dropped because the bounded queue was full.
    #[must_use]
    pub fn dropped_events(&self) -> usize {
        self.dropped_events.load(Ordering::Relaxed)
    }
}

impl Drop for SourceEventRelay {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        let _ = std::fs::remove_file(&self.endpoint);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Publish one event to a running Designer relay.
///
/// Delivery is best-effort. A missing Designer listener is reported as an I/O
/// error so MCP callers can safely ignore it while still completing the file
/// write.
pub fn publish(project_root: &Path, event: &SourceChangeEvent) -> io::Result<()> {
    let endpoint = endpoint_path(project_root);
    let mut stream = dev_rpc::connect_endpoint(&endpoint)?;
    stream.set_write_timeout(Some(CONNECTION_TIMEOUT))?;
    let mut frame = encode_event(event)?;
    frame.push(b'\n');
    stream.write_all(&frame)?;
    stream.flush()
}

fn serve_events(
    listener: DevRpcListener,
    sender: mpsc::SyncSender<SourceChangeEvent>,
    stopped: Arc<AtomicBool>,
    dropped_events: Arc<AtomicUsize>,
) {
    while !stopped.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(CONNECTION_TIMEOUT));
                receive_event(&mut stream, &sender, &dropped_events);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(SOCKET_POLL_INTERVAL);
            }
            Err(_) => break,
        }
    }
}

fn receive_event(
    stream: &mut DevRpcStream,
    sender: &mpsc::SyncSender<SourceChangeEvent>,
    dropped_events: &AtomicUsize,
) {
    let mut frame = Vec::new();
    let read = {
        let reader = BufReader::new(&mut *stream);
        reader
            .take((MAX_EVENT_FRAME_BYTES + 1) as u64)
            .read_until(b'\n', &mut frame)
    };
    let Ok(read) = read else {
        return;
    };
    if read == 0 || read > MAX_EVENT_FRAME_BYTES || frame.last() != Some(&b'\n') {
        return;
    }
    frame.pop();
    if frame.last() == Some(&b'\r') {
        frame.pop();
    }
    let Ok(event) = decode_event(&frame) else {
        return;
    };
    if sender.try_send(event).is_err() {
        dropped_events.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Serialize)]
struct EventNotification<'a> {
    jsonrpc: &'a str,
    method: &'a str,
    params: &'a SourceChangeEvent,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedEventNotification {
    jsonrpc: String,
    method: String,
    params: SourceChangeEvent,
}

fn encode_event(event: &SourceChangeEvent) -> io::Result<Vec<u8>> {
    validate_event(event)?;
    let notification = EventNotification {
        jsonrpc: "2.0",
        method: EVENT_METHOD,
        params: event,
    };
    let frame = serde_json::to_vec(&notification).map_err(io::Error::other)?;
    if frame.len() >= MAX_EVENT_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "source event exceeds the frame size limit",
        ));
    }
    Ok(frame)
}

fn decode_event(frame: &[u8]) -> io::Result<SourceChangeEvent> {
    if frame.len() >= MAX_EVENT_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "source event exceeds the frame size limit",
        ));
    }
    let notification: OwnedEventNotification =
        serde_json::from_slice(frame).map_err(io::Error::other)?;
    if notification.jsonrpc != "2.0" || notification.method != EVENT_METHOD {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "source event JSON-RPC method or version is invalid",
        ));
    }
    validate_event(&notification.params)?;
    Ok(notification.params)
}

fn validate_event(event: &SourceChangeEvent) -> io::Result<()> {
    if event.path.is_empty()
        || event.path.len() > MAX_EVENT_PATH_BYTES
        || event.path.starts_with('/')
        || event.path.contains('\\')
        || event.path.contains(':')
        || event.path.chars().any(char::is_control)
        || event
            .path
            .split('/')
            .any(|component| matches!(component, "" | "." | ".."))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "source event path must be a normalized project-relative path",
        ));
    }
    if !is_sha256_revision(&event.after_revision)
        || event
            .before_revision
            .as_ref()
            .is_some_and(|revision| !is_sha256_revision(revision))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "source event revisions must use the sha256:<hex digest> format",
        ));
    }
    if event.before_revision.is_some() != event.before_content.is_some()
        || event
            .before_content
            .as_ref()
            .is_some_and(|content| content.len() > crate::source_workspace::MAX_SOURCE_FILE_BYTES)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "source event prior content and revision are invalid",
        ));
    }
    if let (Some(expected), Some(content)) = (
        event.before_revision.as_deref(),
        event.before_content.as_deref(),
    ) {
        let actual = digest_hex(content.as_bytes());
        if actual != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "source event prior content does not match its revision",
            ));
        }
    }
    if event.client_name.is_empty()
        || event.client_name.len() > MAX_CLIENT_NAME_BYTES
        || event.client_name.chars().any(char::is_control)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "source event client name is invalid",
        ));
    }
    Ok(())
}

fn is_sha256_revision(revision: &str) -> bool {
    revision.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn stable_path_hash(path: &Path) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in path.as_os_str().as_encoded_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

fn digest_hex(content: &[u8]) -> String {
    let digest = sha2::Sha256::digest(content)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{digest}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event() -> SourceChangeEvent {
        SourceChangeEvent::new(
            "src/app.studio",
            Some(digest_hex(b"before")),
            Some("before".to_owned()),
            format!("sha256:{}", "b".repeat(64)),
            Some("Codex\nagent"),
        )
    }

    #[test]
    fn event_codec_round_trips_json_rpc_notification_and_sanitizes_attribution() {
        let event = sample_event();
        assert_eq!(event.client_name, "Codex agent");
        let frame = encode_event(&event).unwrap();
        let decoded = decode_event(&frame).unwrap();
        assert_eq!(decoded, event);
    }

    #[test]
    fn event_codec_rejects_oversized_invalid_and_non_project_events() {
        assert_eq!(
            decode_event(&vec![b'x'; MAX_EVENT_FRAME_BYTES])
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            encode_event(&SourceChangeEvent::new(
                "../secret",
                None,
                None,
                format!("sha256:{}", "b".repeat(64)),
                None
            ))
            .unwrap_err()
            .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn relay_receives_bounded_local_source_events_and_unlinks_on_drop() {
        let project = tempfile::tempdir().unwrap();
        let relay = SourceEventRelay::start(project.path()).unwrap();
        assert_eq!(
            relay.endpoint_path(),
            endpoint_path(project.path()).as_path()
        );
        publish(project.path(), &sample_event()).unwrap();
        assert_eq!(
            relay.recv_timeout(Duration::from_secs(2)).unwrap(),
            sample_event()
        );
        let endpoint = relay.endpoint_path().to_path_buf();
        drop(relay);
        assert!(!endpoint.exists());
    }
}
