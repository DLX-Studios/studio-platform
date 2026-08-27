//! Gated staging transport coverage.
//!
//! This suite deliberately uses the same [`HttpTransport`] seam as production broker wiring,
//! but delegates TLS and HTTP/1.1 connection management to the operator-provided `curl` client.
//! The test adapter keeps response headers in a separate temporary file and exposes only the
//! bounded body to the broker. It is not a simulator and is never compiled into the deterministic
//! test runner: enabling `integration-real` without the complete staging contract fails closed.
//!
//! Required environment is documented in `docs/net/STAGING_TRANSPORT.md`. No credential value is
//! present in this source or in diagnostics; it is read only from the staging operator's process
//! environment and sent through the normal protected-secret injector.

#![cfg(feature = "integration-real")]
#![allow(missing_docs, clippy::all, clippy::pedantic, dead_code)]

mod common;

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use studio_net::broker::RestBroker;
use studio_net::declaration::{CredentialSource, HttpMethod, RouteGroupDeclaration};
use studio_net::error::BrokerErrorCode;
use studio_net::guest::{BrokerRequest, StreamEvent};
use studio_net::limits::{BrokerLimits, DeclaredLimits};
use studio_net::transport::{
    ByteStream, HttpTransport, IncomingResponse, OutgoingRequest, TransportError,
};

const SECRET_NAME: &str = "payments.key";
const CURL_DEFAULT: &str = "curl";
const MAX_HEADERS_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Staging routes are supplied by the approved endpoint operator, never invented by this suite.
struct StagingConfig {
    origin: String,
    get_path: String,
    post_path: String,
    sse_path: String,
    reconnect_path: String,
    stalled_sse_path: String,
    oversized_path: String,
    rejected_path: String,
    credential: String,
    curl: String,
}

impl StagingConfig {
    fn required(name: &str) -> String {
        std::env::var(name).unwrap_or_else(|_| {
            panic!(
                "integration-real requires {name}; refusing to report an absent staging endpoint as a pass"
            )
        })
    }

    fn path(name: &str) -> String {
        let value = Self::required(name);
        assert!(
            value.starts_with('/')
                && !value.contains('?')
                && !value.bytes().any(|byte| byte.is_ascii_control()),
            "{name} must be an absolute path without a query or control character"
        );
        value
    }

    fn load() -> Self {
        let origin = Self::required("STUDIO_NET_REAL_ENDPOINT_URL");
        let Some((scheme, authority)) = origin.split_once("://") else {
            panic!("STUDIO_NET_REAL_ENDPOINT_URL must be an HTTPS origin")
        };
        assert_eq!(scheme.to_ascii_lowercase(), "https", "staging transport requires TLS");
        assert!(
            !authority.is_empty()
                && !authority.contains('/')
                && !authority.bytes().any(|byte| byte.is_ascii_control()),
            "STUDIO_NET_REAL_ENDPOINT_URL must contain only an HTTPS authority"
        );
        let credential = Self::required("STUDIO_NET_STAGING_CREDENTIAL");
        assert!(!credential.is_empty() && !credential.bytes().any(|byte| byte.is_ascii_control()));
        Self {
            origin,
            get_path: Self::path("STUDIO_NET_STAGING_GET_PATH"),
            post_path: Self::path("STUDIO_NET_STAGING_POST_PATH"),
            sse_path: Self::path("STUDIO_NET_STAGING_SSE_PATH"),
            reconnect_path: Self::path("STUDIO_NET_STAGING_RECONNECT_SSE_PATH"),
            stalled_sse_path: Self::path("STUDIO_NET_STAGING_STALLED_SSE_PATH"),
            oversized_path: Self::path("STUDIO_NET_STAGING_OVERSIZED_PATH"),
            rejected_path: Self::path("STUDIO_NET_STAGING_REJECTED_PATH"),
            credential,
            curl: std::env::var("STUDIO_NET_CURL_BIN").unwrap_or_else(|_| CURL_DEFAULT.to_owned()),
        }
    }
}

/// A temporary path whose contents are removed as soon as the request completes.
struct TempPath(PathBuf);

impl TempPath {
    fn new(label: &str) -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "studio-net-{label}-{}-{sequence}",
            std::process::id()
        ));
        Self(path)
    }

    fn as_path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// TLS-capable staging adapter. Curl writes headers and body to different sinks so HTTP metadata
/// can never be mistaken for JSON or SSE data by the broker.
struct CurlTransport {
    program: String,
    max_response_bytes: usize,
}

impl CurlTransport {
    fn new(program: String) -> Self {
        Self {
            program,
            max_response_bytes: MAX_RESPONSE_BYTES,
        }
    }

    fn config(
        &self,
        request: &OutgoingRequest,
        headers_path: &TempPath,
        body_path: Option<&TempPath>,
        response_path: Option<&TempPath>,
    ) -> Result<String, TransportError> {
        if !request.url.starts_with("https://") {
            return Err(TransportError::ConnectionFailure);
        }
        if request.url.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(TransportError::ConnectionFailure);
        }
        let mut config = format!(
            "silent\nshow-error\nhttp1.1\nrequest = \"{}\"\nurl = \"{}\"\ndump-header = \"{}\"\nconnect-timeout = \"{}\"\nmax-time = \"{}\"\n",
            request.method.as_str(),
            curl_quote(&request.url),
            curl_quote(&headers_path.as_path().to_string_lossy()),
            duration_seconds(request.timeout),
            duration_seconds(request.timeout),
        );
        if let Some(path) = response_path {
            config.push_str(&format!("output = \"{}\"\n", curl_quote(&path.as_path().to_string_lossy())));
        }
        if let Some(path) = body_path {
            config.push_str(&format!(
                "data-binary = \"@{}\"\n",
                curl_quote(&path.as_path().to_string_lossy())
            ));
        }
        for (name, value) in &request.headers {
            if name.bytes().any(|byte| byte.is_ascii_control())
                || value.bytes().any(|byte| byte.is_ascii_control())
            {
                return Err(TransportError::ConnectionFailure);
            }
            config.push_str(&format!(
                "header = \"{}\"\n",
                curl_quote(&format!("{name}: {value}"))
            ));
        }
        Ok(config)
    }

    fn spawn(
        &self,
        config: &str,
        streaming: bool,
    ) -> Result<(Child, Option<ChildStdout>), TransportError> {
        let mut child = Command::new(&self.program)
            .arg("--config")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(if streaming { Stdio::piped() } else { Stdio::null() })
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| TransportError::ConnectionFailure)?;
        let Some(mut input) = child.stdin.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(TransportError::ConnectionFailure);
        };
        if input.write_all(config.as_bytes()).is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(TransportError::ConnectionFailure);
        }
        drop(input);
        let stdout = if streaming { child.stdout.take() } else { None };
        Ok((child, stdout))
    }

    fn execute_inner(&self, request: OutgoingRequest) -> Result<IncomingResponse, TransportError> {
        let headers_path = TempPath::new("headers");
        let response_path = TempPath::new("response");
        let body_path = if let Some(body) = request.body.as_ref() {
            let path = TempPath::new("request");
            if OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path.as_path())
                .and_then(|mut file| file.write_all(body))
                .is_err()
            {
                // The actual error is intentionally collapsed into the transport's value-free
                // failure family; request bytes must not reach diagnostics.
                return Err(TransportError::ConnectionFailure);
            }
            Some(path)
        } else {
            None
        };
        let config = self.config(
            &request,
            &headers_path,
            body_path.as_ref(),
            Some(&response_path),
        )?;
        let (mut child, _) = self.spawn(&config, false)?;
        let status = child
            .wait()
            .map_err(|_| TransportError::ConnectionFailure)?;
        if !status.success() {
            return Err(curl_status_error(status.code()));
        }
        let headers = read_limited(headers_path.as_path(), MAX_HEADERS_BYTES)?;
        let body = read_limited(response_path.as_path(), self.max_response_bytes)?;
        let parsed = parse_http_headers(&headers)?;
        Ok(IncomingResponse {
            status: parsed.status,
            media_type: parsed.media_type,
            body,
        })
    }
}

impl HttpTransport for CurlTransport {
    fn execute(&self, request: OutgoingRequest) -> Result<IncomingResponse, TransportError> {
        self.execute_inner(request)
    }

    fn open_stream(
        &self,
        request: OutgoingRequest,
    ) -> Result<Box<dyn ByteStream + '_>, TransportError> {
        let headers_path = TempPath::new("stream-headers");
        let config = self.config(&request, &headers_path, None, None)?;
        let (child, stdout) = self.spawn(&config, true)?;
        let Some(stdout) = stdout else {
            return Err(TransportError::ConnectionFailure);
        };
        Ok(Box::new(CurlByteStream {
            child,
            stdout,
            headers_path,
            header_deadline: std::time::Instant::now() + request.timeout,
            headers_checked: false,
        }))
    }
}

struct CurlByteStream {
    child: Child,
    stdout: ChildStdout,
    headers_path: TempPath,
    header_deadline: std::time::Instant,
    headers_checked: bool,
}

impl CurlByteStream {
    fn check_headers(&mut self) -> Result<(), TransportError> {
        if self.headers_checked {
            return Ok(());
        }
        loop {
            match read_limited(self.headers_path.as_path(), MAX_HEADERS_BYTES) {
                Ok(headers)
                    if !headers.is_empty()
                        && (headers.windows(4).any(|window| window == b"\r\n\r\n")
                            || headers.windows(2).any(|window| window == b"\n\n")) =>
                {
                    let parsed = parse_http_headers(&headers)?;
                    if !(200..=299).contains(&parsed.status)
                        || parsed.media_type.as_deref() != Some("text/event-stream")
                    {
                        return Err(TransportError::ConnectionFailure);
                    }
                    self.headers_checked = true;
                    return Ok(());
                }
                Err(TransportError::BodyTooLarge) => return Err(TransportError::BodyTooLarge),
                _ => {}
            }
            if std::time::Instant::now() >= self.header_deadline {
                return Err(TransportError::TimedOut);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

impl ByteStream for CurlByteStream {
    fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        let mut buffer = [0_u8; 4096];
        match self.stdout.read(&mut buffer) {
            Ok(0) => {
                self.check_headers()?;
                let status = self
                    .child
                    .wait()
                    .map_err(|_| TransportError::ConnectionFailure)?;
                if status.success() {
                    Ok(None)
                } else {
                    Err(curl_status_error(status.code()))
                }
            }
            Ok(read) => {
                self.check_headers()?;
                Ok(Some(buffer[..read].to_vec()))
            }
            Err(_) => Err(TransportError::ConnectionFailure),
        }
    }
}

impl Drop for CurlByteStream {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn curl_quote(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn duration_seconds(timeout: Duration) -> String {
    format!("{:.3}", timeout.as_secs_f64().max(0.001))
}

fn curl_status_error(code: Option<i32>) -> TransportError {
    if code == Some(28) {
        TransportError::TimedOut
    } else {
        TransportError::ConnectionFailure
    }
}

fn read_limited(path: &std::path::Path, limit: usize) -> Result<Vec<u8>, TransportError> {
    let mut file = File::open(path).map_err(|_| TransportError::ConnectionFailure)?;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| TransportError::ConnectionFailure)?;
        if read == 0 {
            return Ok(bytes);
        }
        if bytes.len().saturating_add(read) > limit {
            return Err(TransportError::BodyTooLarge);
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
}

struct ParsedHeaders {
    status: u16,
    media_type: Option<String>,
}

fn parse_http_headers(raw: &[u8]) -> Result<ParsedHeaders, TransportError> {
    let text = std::str::from_utf8(raw).map_err(|_| TransportError::ConnectionFailure)?;
    let block = text
        .split("\r\n\r\n")
        .chain(text.split("\n\n"))
        .filter(|candidate| candidate.trim_start().starts_with("HTTP/"))
        .next_back()
        .ok_or(TransportError::ConnectionFailure)?;
    let mut lines = block.lines();
    let status_line = lines.next().ok_or(TransportError::ConnectionFailure)?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or(TransportError::ConnectionFailure)?;
    let mut media_type = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return Err(TransportError::ConnectionFailure);
        };
        if name.eq_ignore_ascii_case("content-type") {
            media_type = Some(value.trim().split(';').next().unwrap_or_default().to_owned());
        }
    }
    Ok(ParsedHeaders { status, media_type })
}

/// Records only policy facts, never request values, so the staging credential cannot leak into
/// test output while credential injection remains observable.
struct CredentialProbe {
    inner: CurlTransport,
    expected_header: String,
    expected_value: String,
    credential_seen: Arc<AtomicBool>,
    last_event_id_seen: Arc<AtomicBool>,
}

impl CredentialProbe {
    fn inspect(&self, request: &OutgoingRequest) {
        for (name, value) in &request.headers {
            if name == &self.expected_header && value == &self.expected_value {
                self.credential_seen.store(true, Ordering::Release);
            }
            if name == "last-event-id" {
                self.last_event_id_seen.store(true, Ordering::Release);
            }
        }
    }
}

impl HttpTransport for CredentialProbe {
    fn execute(&self, request: OutgoingRequest) -> Result<IncomingResponse, TransportError> {
        self.inspect(&request);
        self.inner.execute(request)
    }

    fn open_stream(
        &self,
        request: OutgoingRequest,
    ) -> Result<Box<dyn ByteStream + '_>, TransportError> {
        self.inspect(&request);
        self.inner.open_stream(request)
    }
}

fn route(
    id: &str,
    origin: &str,
    method: HttpMethod,
    path: String,
    credential: CredentialSource,
    request_schema: Option<serde_json::Value>,
    streaming: Option<studio_net::declaration::StreamingDeclaration>,
    limits: DeclaredLimits,
) -> RouteGroupDeclaration {
    RouteGroupDeclaration {
        id: id.to_owned(),
        origins: vec![origin.to_owned()],
        methods: vec![method],
        paths: vec![path],
        allowed_headers: vec![String::from("x-request-id")],
        credential,
        request_schema,
        response_schema: Some(json!({"type": "object"})).filter(|_| streaming.is_none()),
        streaming,
        limits,
    }
}

fn declarations(config: &StagingConfig) -> Vec<RouteGroupDeclaration> {
    let secret = CredentialSource::NamedSecret {
        name: SECRET_NAME.to_owned(),
        header: String::from("authorization"),
        prefix: Some(String::from("Bearer ")),
    };
    vec![
        route(
            "secret-api",
            &config.origin,
            HttpMethod::Get,
            config.get_path.clone(),
            secret.clone(),
            None,
            None,
            DeclaredLimits::default(),
        ),
        route(
            "post-api",
            &config.origin,
            HttpMethod::Post,
            config.post_path.clone(),
            CredentialSource::Public,
            Some(json!({
                "type": "object",
                "properties": {"message": {"type": "string"}},
                "required": ["message"]
            })),
            None,
            DeclaredLimits::default(),
        ),
        route(
            "events-api",
            &config.origin,
            HttpMethod::Get,
            config.sse_path.clone(),
            CredentialSource::Public,
            None,
            Some(streaming_declaration(2)),
            DeclaredLimits::default(),
        ),
        route(
            "reconnect-api",
            &config.origin,
            HttpMethod::Get,
            config.reconnect_path.clone(),
            CredentialSource::Public,
            None,
            Some(streaming_declaration(1)),
            DeclaredLimits::default(),
        ),
        route(
            "stalled-api",
            &config.origin,
            HttpMethod::Get,
            config.stalled_sse_path.clone(),
            CredentialSource::Public,
            None,
            Some(streaming_declaration(1)),
            DeclaredLimits {
                stream_max_duration_ms: Some(1_000),
                ..DeclaredLimits::default()
            },
        ),
        route(
            "oversized-api",
            &config.origin,
            HttpMethod::Get,
            config.oversized_path.clone(),
            secret,
            None,
            None,
            DeclaredLimits {
                max_response_bytes: Some(1_024),
                ..DeclaredLimits::default()
            },
        ),
        route(
            "rejected-api",
            &config.origin,
            HttpMethod::Get,
            config.rejected_path.clone(),
            CredentialSource::Public,
            None,
            None,
            DeclaredLimits::default(),
        ),
    ]
}

fn streaming_declaration(reconnects: u32) -> studio_net::declaration::StreamingDeclaration {
    studio_net::declaration::StreamingDeclaration {
        chunk_schema: json!({
            "type": "object",
            "properties": {"text": {"type": "string"}},
            "required": ["text"]
        }),
        reconnects: Some(reconnects),
        retry_base_delay_ms: Some(25),
    }
}

fn broker(
    config: &StagingConfig,
    transport: Arc<dyn HttpTransport>,
    limits: BrokerLimits,
) -> Arc<RestBroker<'static>> {
    let mut broker = RestBroker::new(transport, limits);
    for declaration in declarations(config) {
        broker
            .declare_group(&declaration)
            .expect("staging declaration must be admitted");
    }
    let fixture = common::secret_fixture(SECRET_NAME, &config.credential);
    let injector = fixture.injection_handle(&[fixture.key.clone()]);
    broker.set_named_secret_injector(Arc::new(injector));
    Arc::new(broker)
}

fn probe(config: &StagingConfig) -> (Arc<CredentialProbe>, Arc<AtomicBool>, Arc<AtomicBool>) {
    let credential_seen = Arc::new(AtomicBool::new(false));
    let last_event_id_seen = Arc::new(AtomicBool::new(false));
    let transport = Arc::new(CredentialProbe {
        inner: CurlTransport::new(config.curl.clone()),
        expected_header: String::from("authorization"),
        expected_value: format!("Bearer {}", config.credential),
        credential_seen: Arc::clone(&credential_seen),
        last_event_id_seen: Arc::clone(&last_event_id_seen),
    });
    (transport, credential_seen, last_event_id_seen)
}

fn stream_events(handle: &studio_net::guest::StreamHandle) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    while let Some(event) = handle.next_event() {
        let terminal = matches!(
            event,
            StreamEvent::Completed | StreamEvent::Cancelled | StreamEvent::Failed(_)
        );
        events.push(event);
        if terminal {
            break;
        }
    }
    events
}

#[test]
fn staging_get_uses_tls_and_keeps_headers_out_of_json_body() {
    let config = StagingConfig::load();
    let (transport, credential_seen, _) = probe(&config);
    let broker = broker(&config, transport, BrokerLimits::default());
    let response = broker
        .execute(BrokerRequest::new(
            &config.origin,
            HttpMethod::Get,
            &config.get_path,
        ))
        .expect("approved staging GET");
    assert!((200..=299).contains(&response.status()));
    assert!(response.body().is_object(), "staging GET must return a JSON object");
    assert!(!response.body().to_string().contains("HTTP/"));
    assert!(credential_seen.load(Ordering::Acquire), "credential was not injected at send time");
}

#[test]
fn staging_post_carries_a_bounded_body_through_the_broker() {
    let config = StagingConfig::load();
    let (transport, _, _) = probe(&config);
    let broker = broker(&config, transport, BrokerLimits::default());
    let response = broker
        .execute(
            BrokerRequest::new(&config.origin, HttpMethod::Post, &config.post_path)
                .with_header("X-Request-Id", "staging-transport-gate")
                .with_body(json!({"message": "staging-transport-gate"})),
        )
        .expect("approved staging POST");
    assert!((200..=299).contains(&response.status()));
    assert!(response.body().is_object());
}

#[test]
fn staging_route_allowlist_denies_an_unapproved_path_before_network() {
    let config = StagingConfig::load();
    let (transport, _, _) = probe(&config);
    let broker = broker(&config, transport, BrokerLimits::default());
    let error = broker
        .execute(BrokerRequest::new(
            &config.origin,
            HttpMethod::Get,
            "/__studio_net_gate_not_declared",
        ))
        .expect_err("undeclared path");
    assert_eq!(error.code(), BrokerErrorCode::PathNotDeclared);
}

#[test]
fn staging_oversized_response_maps_to_a_stable_limit_error() {
    let config = StagingConfig::load();
    let (transport, _, _) = probe(&config);
    let broker = broker(&config, transport, BrokerLimits::default());
    let error = broker
        .execute(BrokerRequest::new(
            &config.origin,
            HttpMethod::Get,
            &config.oversized_path,
        ))
        .expect_err("oversized response");
    assert_eq!(error.code(), BrokerErrorCode::ResponseTooLarge);
}

#[test]
fn staging_rejection_is_classified_without_credential_redaction_leaks() {
    let config = StagingConfig::load();
    let (transport, _, _) = probe(&config);
    let broker = broker(&config, transport, BrokerLimits::default());
    // Register the staging credential in the broker's scrubber before observing the failure.
    broker
        .execute(BrokerRequest::new(
            &config.origin,
            HttpMethod::Get,
            &config.get_path,
        ))
        .expect("approved staging GET");
    let error = broker
        .execute(BrokerRequest::new(
            &config.origin,
            HttpMethod::Get,
            &config.rejected_path,
        ))
        .expect_err("staging rejection");
    assert_eq!(error.code(), BrokerErrorCode::UpstreamRejected);
    assert!(!format!("{error:?}").contains(&config.credential));
}

#[test]
fn staging_sse_parses_typed_events_and_completes() {
    let config = StagingConfig::load();
    let (transport, _, _) = probe(&config);
    let broker = broker(&config, transport, BrokerLimits::default());
    let events = stream_events(
        &broker
            .open_stream(BrokerRequest::new(
                &config.origin,
                HttpMethod::Get,
                &config.sse_path,
            ))
            .expect("approved staging SSE"),
    );
    assert!(events.iter().any(|event| matches!(event, StreamEvent::Opened)));
    assert!(events.iter().any(|event| matches!(event, StreamEvent::Chunk(value) if value["text"].is_string())));
    assert!(matches!(events.last(), Some(StreamEvent::Completed)));
}

#[test]
fn staging_sse_reconnects_with_last_event_id() {
    let config = StagingConfig::load();
    let (transport, _, last_event_id_seen) = probe(&config);
    let broker = broker(&config, transport, BrokerLimits::default());
    let events = stream_events(
        &broker
            .open_stream(BrokerRequest::new(
                &config.origin,
                HttpMethod::Get,
                &config.reconnect_path,
            ))
            .expect("approved reconnecting staging SSE"),
    );
    assert!(events.iter().any(|event| matches!(event, StreamEvent::RetryScheduled { .. })));
    assert!(events.iter().any(|event| matches!(event, StreamEvent::Chunk(_))));
    assert!(matches!(events.last(), Some(StreamEvent::Completed)));
    assert!(last_event_id_seen.load(Ordering::Acquire));
}

#[test]
fn staging_stalled_sse_hits_the_read_deadline() {
    let config = StagingConfig::load();
    let (transport, _, _) = probe(&config);
    let limits = BrokerLimits {
        stream_idle_timeout: Duration::from_secs(2),
        ..BrokerLimits::default()
    };
    let broker = broker(&config, transport, limits);
    let events = stream_events(
        &broker
            .open_stream(BrokerRequest::new(
                &config.origin,
                HttpMethod::Get,
                &config.stalled_sse_path,
            ))
            .expect("approved stalled staging SSE"),
    );
    assert!(matches!(
        events.last(),
        Some(StreamEvent::Failed(error)) if error.code() == BrokerErrorCode::Timeout
    ));
}

#[test]
fn staging_transport_rejects_plain_http_before_connecting() {
    let transport = CurlTransport::new(CURL_DEFAULT.to_owned());
    let error = transport
        .execute(OutgoingRequest {
            method: HttpMethod::Get,
            url: String::from("http://not-tls.example.test/") ,
            headers: Vec::new(),
            body: None,
            timeout: Duration::from_secs(1),
        })
        .expect_err("plain HTTP must not reach the staging adapter");
    assert_eq!(error, TransportError::ConnectionFailure);
}
