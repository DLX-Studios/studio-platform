//! A stateless OAuth redirect broker.
//!
//! The broker is the public half of a desktop authorization flow. A provider sends the browser
//! to `{broker}/cb/{application_id}`; the broker answers `302` with a `Location` pointing at the
//! application's own loopback listener, and the browser follows it. The provider never learns
//! the loopback address, and the application never sees the provider's traffic.
//!
//! # Stateless by construction
//!
//! The broker stores nothing. There is no database, no session table, and no disk state: every
//! request is answered from the request itself plus the configured shared secret. The loopback
//! port the redirect must reach travels inside the provider's `state` value, which the
//! application minted before it opened the browser and which the broker recovers with
//! [`studio_oauth_redirect::decode_state`].
//!
//! # The state value is authenticated, so this is not an open redirector
//!
//! A broker that merely read a port out of `state` would let anyone aim its `302` at an
//! arbitrary service on the machine that runs it. That is an open redirector wearing a loopback
//! address, and it is the reason the shared secret exists. The port is instead covered by an
//! authentication tag computed with the secret, so the broker only ever forwards to a port an
//! application proved it chose. A forged, tampered, or foreign state value is refused with `400`
//! and nothing is forwarded.
//!
//! # Deployment: TLS is terminated by a reverse proxy
//!
//! The service speaks plain HTTP on purpose. It is deployed behind a TLS-terminating reverse
//! proxy, and the redirect URI registered with a provider is always `https://`, which the client
//! side enforces when it registers. The service never performs a TLS handshake, so the
//! certificate, its renewal, and the cipher policy stay with the proxy.
//!
//! # What the broker does not keep
//!
//! The `code`, the `state`, and the forwarded `Location` are zeroed as soon as the response has
//! been written, and the request log records only the method, the path, and the status. A query
//! string, a header, and a body never reach the log.
//!
//! # Running the service
//!
//! [`serve`] runs the accept loop over an already-bound listener, so a deployment, a test, or
//! any other host can choose the address and the lifetime of the service. The binary in this
//! crate is a thin wrapper: it reads the environment, binds the listener, and calls [`serve`].

use std::collections::BTreeSet;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use studio_oauth_redirect::{
    MINIMUM_SECRET_BYTES, RedirectParseError, application_id_from_path, decode_state,
    encode_redirect_params, loopback_callback_url, parse_redirect_params,
};
use zeroize::{Zeroize, Zeroizing};

/// The largest request body the broker accepts, in bytes.
///
/// A request that declares a longer body is refused with `413` before the body is buffered.
pub const MAX_BODY_BYTES: usize = 16_384;

/// The longest application identifier the broker serves.
///
/// The shared contract keeps its own copy of this rule private, so the broker repeats it here
/// to decide whether an identifier is syntactically usable at all.
pub const MAX_APPLICATION_ID_BYTES: usize = 128;

/// How long a connection may stay idle before the broker stops waiting for it.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// How long a response may take to reach the client.
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// The largest request head the broker buffers.
const MAX_HEAD_BYTES: usize = 8_192;

/// The body a refusal carries.
///
/// The body is fixed: it never repeats anything the client sent.
const REFUSAL_BODY: &str = "request refused\n";

/// The longest path a log line carries.
const MAX_LOGGED_PATH_BYTES: usize = 256;

/// Why a broker configuration was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerConfigError {
    /// The shared secret was shorter than the shared minimum.
    SecretTooShort,
}

/// The configuration one broker instance serves with.
///
/// The configuration is immutable once built, so one instance can be shared by every connection
/// thread without further synchronization.
#[derive(Clone)]
pub struct BrokerConfig {
    secret: Zeroizing<Vec<u8>>,
    applications: Option<Arc<BTreeSet<String>>>,
}

impl fmt::Debug for BrokerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrokerConfig")
            .field("secret", &"[redacted]")
            .field(
                "applications",
                &self.applications.as_ref().map(|allowed| allowed.len()),
            )
            .finish()
    }
}

impl BrokerConfig {
    /// Configure a broker from the values an operator deploys it with.
    ///
    /// `applications` is the allowlist of application identifiers the broker serves. An empty
    /// list means every syntactically valid identifier is served.
    ///
    /// # Errors
    ///
    /// Returns [`BrokerConfigError::SecretTooShort`] when the shared secret is shorter than
    /// [`MINIMUM_SECRET_BYTES`], because a short secret would make a state value forgeable.
    pub fn new(secret: &[u8], applications: &[&str]) -> Result<Self, BrokerConfigError> {
        if secret.len() < MINIMUM_SECRET_BYTES {
            return Err(BrokerConfigError::SecretTooShort);
        }
        let applications: BTreeSet<String> = applications
            .iter()
            .map(|identifier| String::from(*identifier))
            .collect();
        Ok(Self {
            secret: Zeroizing::new(secret.to_vec()),
            applications: if applications.is_empty() {
                None
            } else {
                Some(Arc::new(applications))
            },
        })
    }

    /// Whether this broker serves the given application identifier.
    #[must_use]
    pub fn serves(&self, application_id: &str) -> bool {
        self.applications
            .as_ref()
            .is_none_or(|allowed| allowed.contains(application_id))
    }

    /// Whether the broker serves every syntactically valid application identifier.
    #[must_use]
    pub fn is_unrestricted(&self) -> bool {
        self.applications.is_none()
    }

    /// The number of application identifiers on the allowlist.
    #[must_use]
    pub fn application_count(&self) -> usize {
        self.applications
            .as_ref()
            .map_or(0, |allowed| allowed.len())
    }
}

/// What the broker answers one request with.
///
/// The decision carries no I/O: the same value drives the response writer and the request log,
/// and a test can inspect it without opening a socket.
pub enum BrokerDecision {
    /// Forward the provider's redirect to the loopback port the state value names.
    Forward {
        /// The `Location` the response carries.
        ///
        /// The value is held in a zeroizing buffer, so it is wiped when the decision is dropped.
        location: Zeroizing<String>,
    },
    /// Refuse the request.
    Reject {
        /// The status the refusal carries.
        status: u16,
        /// A fixed description of the refusal.
        ///
        /// The reason never quotes the request, so it is safe to log.
        reason: &'static str,
    },
}

impl fmt::Debug for BrokerDecision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Forward { .. } => formatter
                .debug_struct("Forward")
                .field("location", &"[redacted]")
                .finish(),
            Self::Reject { status, reason } => formatter
                .debug_struct("Reject")
                .field("status", status)
                .field("reason", reason)
                .finish(),
        }
    }
}

impl BrokerDecision {
    /// The status the decision answers with.
    #[must_use]
    pub fn status(&self) -> u16 {
        match self {
            Self::Forward { .. } => 302,
            Self::Reject { status, .. } => *status,
        }
    }

    /// The forwarding location, when the decision forwards.
    #[must_use]
    pub fn location(&self) -> Option<&str> {
        match self {
            Self::Forward { location } => Some(location.as_str()),
            Self::Reject { .. } => None,
        }
    }

    /// The refusal reason, when the decision rejects.
    #[must_use]
    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Forward { .. } => None,
            Self::Reject { reason, .. } => Some(reason),
        }
    }
}

/// Decide what one request is answered with.
///
/// `target` is the request target, that is the path and the optional query string. For `GET` the
/// parameters come from the query string; for `POST` they come from `body`, which is only read
/// when `content_type` names the form encoding the shared contract uses.
#[must_use]
pub fn decide(
    config: &BrokerConfig,
    method: &str,
    target: &str,
    content_type: Option<&str>,
    body: &str,
) -> BrokerDecision {
    if method != "GET" && method != "POST" {
        return BrokerDecision::Reject {
            status: 405,
            reason: "method not allowed",
        };
    }
    if body.len() > MAX_BODY_BYTES {
        return BrokerDecision::Reject {
            status: 413,
            reason: "request body too large",
        };
    }

    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (target, None),
    };
    let Some(application_id) = application_id_from_path(path) else {
        return BrokerDecision::Reject {
            status: 404,
            reason: "unknown application",
        };
    };
    if !valid_application_id(application_id) || !config.serves(application_id) {
        return BrokerDecision::Reject {
            status: 404,
            reason: "unknown application",
        };
    }

    let payload = if method == "GET" {
        query.unwrap_or("")
    } else {
        if !is_form_urlencoded(content_type) {
            return BrokerDecision::Reject {
                status: 400,
                reason: "unsupported content type",
            };
        }
        body
    };

    let mut params = match parse_redirect_params(payload) {
        Ok(params) => params,
        Err(error) => {
            return BrokerDecision::Reject {
                status: 400,
                reason: parse_reason(error),
            };
        }
    };
    let Some(state) = params.get("state") else {
        return BrokerDecision::Reject {
            status: 400,
            reason: "missing state",
        };
    };
    let Ok(port) = decode_state(state, config.secret.as_slice()) else {
        return BrokerDecision::Reject {
            status: 400,
            reason: "invalid state",
        };
    };

    // Every provider parameter is relayed, not only the code: the application reads the same
    // fields on its side, and a provider may add a payload field of its own.
    let encoded = Zeroizing::new(encode_redirect_params(&params));
    let base = loopback_callback_url(port);
    let mut location = String::with_capacity(base.len() + 1 + encoded.len());
    location.push_str(&base);
    location.push('?');
    location.push_str(&encoded);
    for value in params.values_mut() {
        value.zeroize();
    }
    BrokerDecision::Forward {
        location: Zeroizing::new(location),
    }
}

/// Serve requests on an already-bound listener until the listener fails.
///
/// The listener is taken by value so the caller keeps control of the address: a deployment binds
/// the address it publishes, and a test binds `127.0.0.1:0` and reads the port the operating
/// system assigned. One thread serves one connection, and every connection carries a read
/// timeout, so a client that opens a socket and says nothing cannot hold a thread indefinitely.
///
/// # Errors
///
/// Returns an error when the listener stops accepting connections.
// The listener is taken by value on purpose: the service owns the socket for as long as it runs,
// so a caller hands the socket over and stops holding it. A reference would keep the caller's
// handle borrowed for the whole life of the service, which is exactly what ownership avoids.
#[allow(clippy::needless_pass_by_value)]
pub fn serve(listener: TcpListener, config: &BrokerConfig) -> io::Result<()> {
    let config = Arc::new(config.clone());
    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => {
                let config = Arc::clone(&config);
                let spawned = thread::Builder::new()
                    .name(String::from("broker-connection"))
                    .spawn(move || {
                        let _ = handle_connection(stream, &config);
                    });
                if spawned.is_err() {
                    eprintln!("studio-oauth-broker: cannot start a connection thread");
                }
            }
            Err(error) => {
                eprintln!("studio-oauth-broker: cannot accept a connection: {error}");
            }
        }
    }
    Ok(())
}

/// Whether an application identifier is one the broker can serve.
///
/// The shared contract keeps its own copy of this rule private, so the broker repeats it here.
fn valid_application_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_APPLICATION_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

/// Whether a content type names the form encoding the shared contract uses.
fn is_form_urlencoded(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|value| {
        value.split(';').next().is_some_and(|media_type| {
            media_type
                .trim()
                .eq_ignore_ascii_case("application/x-www-form-urlencoded")
        })
    })
}

/// The refusal reason for a payload the shared parser refused.
///
/// The reasons are fixed strings: no part of the offending payload is repeated.
fn parse_reason(error: RedirectParseError) -> &'static str {
    match error {
        RedirectParseError::Malformed => "malformed parameters",
        RedirectParseError::InvalidField => "invalid parameter name",
        RedirectParseError::InvalidValue => "invalid parameter value",
        RedirectParseError::DuplicateField => "duplicate parameter",
    }
}

/// Why one request could not be read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RequestFailure {
    /// The client sent nothing complete before the connection ended or timed out.
    Incomplete,
    /// The request head was larger than the broker buffers.
    HeadTooLarge,
    /// The request line or the headers were not usable.
    Malformed,
}

impl RequestFailure {
    /// The status the refusal carries.
    fn status(self) -> u16 {
        match self {
            Self::Incomplete => 408,
            Self::HeadTooLarge => 413,
            Self::Malformed => 400,
        }
    }

    /// The reason the refusal carries.
    fn reason(self) -> &'static str {
        match self {
            Self::Incomplete => "incomplete request",
            Self::HeadTooLarge => "request head too large",
            Self::Malformed => "malformed request",
        }
    }
}

/// The parts of a request the broker reads.
struct RequestHead {
    method: String,
    target: Zeroizing<String>,
    content_type: Option<String>,
    content_length: Option<u64>,
}

/// Serve one connection.
///
/// # Errors
///
/// Returns an error when the response could not be written.
fn handle_connection(mut stream: TcpStream, config: &BrokerConfig) -> io::Result<()> {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    let mut buffer = Zeroizing::new(Vec::new());

    let head_end = match read_head(&mut stream, &mut buffer) {
        Ok(head_end) => head_end,
        Err(failure) => {
            refuse(&mut stream, failure);
            return Ok(());
        }
    };
    let head = match parse_request(&buffer.as_slice()[..head_end]) {
        Ok(head) => head,
        Err(failure) => {
            refuse(&mut stream, failure);
            return Ok(());
        }
    };

    let declared = head.content_length.unwrap_or(0);
    if declared > MAX_BODY_BYTES as u64 {
        let mut decision = BrokerDecision::Reject {
            status: 413,
            reason: "request body too large",
        };
        let status = decision.status();
        write_response(&mut stream, &mut decision)?;
        log_request(
            Some(&head.method),
            Some(request_path(head.target.as_str())),
            status,
        );
        return Ok(());
    }

    let body = match read_body(&mut stream, &mut buffer, head_end, declared) {
        Ok(body) => body,
        Err(failure) => {
            refuse(&mut stream, failure);
            return Ok(());
        }
    };
    let mut decision = decide(
        config,
        &head.method,
        head.target.as_str(),
        head.content_type.as_deref(),
        body.as_str(),
    );
    let status = decision.status();
    write_response(&mut stream, &mut decision)?;
    log_request(
        Some(&head.method),
        Some(request_path(head.target.as_str())),
        status,
    );
    Ok(())
}

/// Answer a request that could not be read, and log it.
///
/// A client that cannot be answered any more is not worth an error: the connection is dropped
/// either way.
fn refuse(stream: &mut TcpStream, failure: RequestFailure) {
    let status = failure.status();
    let mut decision = BrokerDecision::Reject {
        status,
        reason: failure.reason(),
    };
    let _ = write_response(stream, &mut decision);
    log_request(None, None, status);
}

/// Read the request head into `buffer`.
///
/// # Errors
///
/// Returns a failure when the head never arrives, exceeds the bound, or cannot be read.
fn read_head(
    stream: &mut TcpStream,
    buffer: &mut Zeroizing<Vec<u8>>,
) -> Result<usize, RequestFailure> {
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream
            .read(&mut chunk)
            .map_err(|_| RequestFailure::Incomplete)?;
        if read == 0 {
            return Err(RequestFailure::Incomplete);
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(end) = find_head_end(buffer.as_slice()) {
            return Ok(end);
        }
        if buffer.len() > MAX_HEAD_BYTES {
            return Err(RequestFailure::HeadTooLarge);
        }
    }
}

/// The end of the request head in `buffer`, if the head is complete.
fn find_head_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
}

/// Read exactly `length` body bytes, joining what the head read already carried.
///
/// # Errors
///
/// Returns a failure when the body ends early or is not text.
fn read_body(
    stream: &mut TcpStream,
    buffer: &mut Zeroizing<Vec<u8>>,
    head_end: usize,
    length: u64,
) -> Result<Zeroizing<String>, RequestFailure> {
    let mut body = Zeroizing::new(Vec::new());
    body.extend_from_slice(&buffer.as_slice()[head_end..]);
    let expected = usize::try_from(length).map_err(|_| RequestFailure::Malformed)?;
    let mut chunk = [0_u8; 1024];
    while body.len() < expected {
        let read = stream
            .read(&mut chunk)
            .map_err(|_| RequestFailure::Incomplete)?;
        if read == 0 {
            return Err(RequestFailure::Incomplete);
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(expected);
    let text =
        String::from_utf8(std::mem::take(&mut *body)).map_err(|_| RequestFailure::Malformed)?;
    Ok(Zeroizing::new(text))
}

/// Parse the request head.
///
/// # Errors
///
/// Returns a failure when the request line or the headers are not usable.
fn parse_request(bytes: &[u8]) -> Result<RequestHead, RequestFailure> {
    let text = std::str::from_utf8(bytes).map_err(|_| RequestFailure::Malformed)?;
    let mut lines = text.split("\r\n");
    let request_line = lines.next().ok_or(RequestFailure::Malformed)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or(RequestFailure::Malformed)?.to_owned();
    let target = parts.next().ok_or(RequestFailure::Malformed)?.to_owned();
    let version = parts.next().ok_or(RequestFailure::Malformed)?;
    if parts.next().is_some() || !version.starts_with("HTTP/") {
        return Err(RequestFailure::Malformed);
    }

    let mut content_type = None;
    let mut content_length = None;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(RequestFailure::Malformed);
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        match name.as_str() {
            "content-type" => {
                if content_type.is_some() {
                    return Err(RequestFailure::Malformed);
                }
                content_type = Some(value.to_owned());
            }
            "content-length" => {
                if content_length.is_some() {
                    return Err(RequestFailure::Malformed);
                }
                content_length = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| RequestFailure::Malformed)?,
                );
            }
            "transfer-encoding" => return Err(RequestFailure::Malformed),
            _ => {}
        }
    }
    Ok(RequestHead {
        method,
        target: Zeroizing::new(target),
        content_type,
        content_length,
    })
}

/// Write the response the decision reached.
///
/// The forwarded location and the refusal body are the only bytes written, and the forwarded
/// location is wiped once it has been sent.
///
/// # Errors
///
/// Returns an error when the response could not be written.
fn write_response(stream: &mut TcpStream, decision: &mut BrokerDecision) -> io::Result<()> {
    match decision {
        BrokerDecision::Forward { location } => {
            let head = Zeroizing::new(format!(
                "HTTP/1.1 302 Found\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                location.as_str()
            ));
            let written = stream
                .write_all(head.as_bytes())
                .and_then(|()| stream.flush());
            location.zeroize();
            written
        }
        BrokerDecision::Reject { status, .. } => {
            let head = format!(
                "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                status,
                status_phrase(*status),
                REFUSAL_BODY.len()
            );
            stream.write_all(head.as_bytes())?;
            stream.write_all(REFUSAL_BODY.as_bytes())?;
            stream.flush()
        }
    }
}

/// The reason phrase a status is written with.
fn status_phrase(status: u16) -> &'static str {
    match status {
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        _ => "Error",
    }
}

/// The path part of a request target, without its query string.
fn request_path(target: &str) -> &str {
    match target.split_once('?') {
        Some((path, _)) => path,
        None => target,
    }
}

/// Log one request.
///
/// The line carries the method, the path, and the status, and nothing else: no query string, no
/// header, no body, and no forwarded location.
fn log_request(method: Option<&str>, path: Option<&str>, status: u16) {
    let method = method.map_or(String::from("-"), sanitize);
    let path = path.map_or(String::from("-"), sanitize);
    let mut stdout = io::stdout().lock();
    let _ = writeln!(stdout, "{method} {path} {status}");
    let _ = stdout.flush();
}

/// Make a value safe to write on one log line.
///
/// A control character would let a client forge log lines, so anything that is not printable
/// ASCII is replaced, and the result is bounded.
fn sanitize(value: &str) -> String {
    let mut output = String::with_capacity(value.len().min(MAX_LOGGED_PATH_BYTES));
    for character in value.chars().take(MAX_LOGGED_PATH_BYTES) {
        if character.is_ascii_graphic() || character == ' ' {
            output.push(character);
        } else {
            output.push('?');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_oauth_redirect::{STATE_ENTROPY_BYTES, encode_state};

    const SECRET: &[u8] = b"a-shared-secret-of-at-least-thirty-two-bytes";
    const OTHER_SECRET: &[u8] = b"another-secret-of-at-least-thirty-two-bytes";
    const PORT: u16 = 43_123;

    fn config() -> BrokerConfig {
        BrokerConfig::new(SECRET, &[]).expect("the secret is long enough")
    }

    fn state(secret: &[u8], port: u16) -> String {
        encode_state(port, secret, &[7_u8; STATE_ENTROPY_BYTES])
    }

    fn location_of(decision: &BrokerDecision) -> String {
        decision
            .location()
            .expect("the decision forwards")
            .to_owned()
    }

    #[test]
    fn a_valid_get_redirect_forwards_to_the_loopback_port() {
        let state = state(SECRET, PORT);
        let target = format!("/cb/desktop?code=abc123&state={state}");
        let decision = decide(&config(), "GET", &target, None, "");
        assert_eq!(decision.status(), 302);
        let location = location_of(&decision);
        assert!(
            location.starts_with(&format!("http://127.0.0.1:{PORT}/oauth/callback?")),
            "unexpected location {location}"
        );
        assert!(
            location.contains("code=abc123"),
            "missing code in {location}"
        );
        assert!(
            location.contains(&format!("state={state}")),
            "missing state in {location}"
        );
    }

    #[test]
    fn a_form_post_redirect_relays_every_provider_field() {
        let state = state(SECRET, PORT);
        let body = format!("code=abc123&state={state}&user=%7B%22name%22%3A%22Ada%22%7D");
        let decision = decide(
            &config(),
            "POST",
            "/cb/desktop",
            Some("application/x-www-form-urlencoded"),
            &body,
        );
        assert_eq!(decision.status(), 302);
        let location = location_of(&decision);
        assert!(
            location.contains("code=abc123"),
            "missing code in {location}"
        );
        assert!(
            location.contains(&format!("state={state}")),
            "missing state in {location}"
        );
        assert!(
            location.contains("user=%7B%22name%22%3A%22Ada%22%7D"),
            "missing provider payload in {location}"
        );
    }

    #[test]
    fn a_state_minted_with_another_secret_is_refused() {
        let target = format!(
            "/cb/desktop?code=abc123&state={}",
            state(OTHER_SECRET, PORT)
        );
        let decision = decide(&config(), "GET", &target, None, "");
        assert_eq!(decision.status(), 400);
        assert!(
            decision.location().is_none(),
            "a forged state was forwarded"
        );
    }

    #[test]
    fn a_state_naming_a_privileged_port_is_refused() {
        let target = format!("/cb/desktop?code=abc123&state={}", state(SECRET, 22));
        let decision = decide(&config(), "GET", &target, None, "");
        assert_eq!(decision.status(), 400);
        assert!(
            decision.location().is_none(),
            "a privileged port was forwarded"
        );
    }

    #[test]
    fn a_path_outside_the_broker_prefix_is_refused() {
        let decision = decide(&config(), "GET", "/oauth/callback?code=abc123", None, "");
        assert_eq!(decision.status(), 404);
        assert!(decision.location().is_none());
    }

    #[test]
    fn an_application_outside_the_allowlist_is_refused() {
        let allowed = BrokerConfig::new(SECRET, &["desktop"]).expect("the secret is long enough");
        let state = state(SECRET, PORT);
        let target = format!("/cb/other?code=abc123&state={state}");
        let decision = decide(&allowed, "GET", &target, None, "");
        assert_eq!(decision.status(), 404);
        assert!(decision.location().is_none());
    }

    #[test]
    fn a_post_with_the_wrong_content_type_is_refused() {
        let state = state(SECRET, PORT);
        let body = format!("code=abc123&state={state}");
        let decision = decide(
            &config(),
            "POST",
            "/cb/desktop",
            Some("application/json"),
            &body,
        );
        assert_eq!(decision.status(), 400);
        assert!(decision.location().is_none());
    }

    #[test]
    fn a_get_with_a_duplicate_parameter_is_refused() {
        let state = state(SECRET, PORT);
        let target = format!("/cb/desktop?code=one&code=two&state={state}");
        let decision = decide(&config(), "GET", &target, None, "");
        assert_eq!(decision.status(), 400);
        assert!(decision.location().is_none());
    }

    #[test]
    fn a_redirect_without_parameters_is_refused() {
        let decision = decide(&config(), "GET", "/cb/desktop", None, "");
        assert_eq!(decision.status(), 400);
        assert!(decision.location().is_none());
    }

    #[test]
    fn a_method_other_than_get_or_post_is_refused() {
        let state = state(SECRET, PORT);
        let target = format!("/cb/desktop?code=abc123&state={state}");
        let decision = decide(&config(), "DELETE", &target, None, "");
        assert_eq!(decision.status(), 405);
        assert!(decision.location().is_none());
    }

    #[test]
    fn the_rendered_location_carries_no_control_character() {
        let state = state(SECRET, PORT);
        let target = format!("/cb/desktop?code=a%20b%C3%A9&state={state}");
        let decision = decide(&config(), "GET", &target, None, "");
        let location = location_of(&decision);
        assert!(
            !location.chars().any(char::is_control),
            "the location carries a control character"
        );
        assert!(!location.contains('\n'), "the location carries a newline");
        assert!(
            !location.contains('\r'),
            "the location carries a carriage return"
        );
    }

    #[test]
    fn a_short_secret_is_refused() {
        assert_eq!(
            BrokerConfig::new(b"too-short", &[]).unwrap_err(),
            BrokerConfigError::SecretTooShort
        );
    }

    #[test]
    fn an_empty_allowlist_serves_every_application() {
        let config = config();
        assert!(config.is_unrestricted());
        assert!(config.serves("desktop"));
        assert!(config.serves("other"));
    }
}
