//! Real-endpoint integration suite.
//!
//! TODO(tt-19): wire the approved first-party endpoint (see ticket 19 acceptance item
//! "Integration suite executes against an approved real endpoint, not a simulator") and a
//! production `HttpTransport` implementation over the host network stack. The deterministic
//! serialized runner must NOT enable this feature: it requires operator-provided configuration
//! via `STUDIO_NET_REAL_ENDPOINT_URL` plus outbound network access, so it is compiled only
//! under `--features integration-real`.
//!
//! Planned coverage once wired:
//!
//! 1. Round-trip GET/POST against two declared routes of the approved endpoint.
//! 2. SSE stream from the approved streaming route with reconnect observation.
//! 3. Credential injection against a staging-only named secret.

#![cfg(feature = "integration-real")]
#![allow(missing_docs)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use studio_net::declaration::{HttpMethod, RouteGroupDeclaration};
use studio_net::guest::BrokerRequest;
use studio_net::transport::{
    ByteStream, HttpTransport, IncomingResponse, OutgoingRequest, TransportError,
};

/// Minimal blocking HTTP/1.1 transport for the gated suite only; production wiring will use the
/// host's real network client instead.
struct RawTcpTransport;

impl HttpTransport for RawTcpTransport {
    fn execute(&self, request: OutgoingRequest) -> Result<IncomingResponse, TransportError> {
        let mut stream = connect_and_send(&request)?;
        let mut body = Vec::new();
        stream
            .read_to_end(&mut body)
            .map_err(|_| TransportError::ConnectionFailure)?;
        Ok(IncomingResponse {
            status: parse_status(&body).ok_or(TransportError::ConnectionFailure)?,
            media_type: None,
            body,
        })
    }

    fn open_stream(
        &self,
        request: OutgoingRequest,
    ) -> Result<Box<dyn ByteStream + '_>, TransportError> {
        let stream = connect_and_send(&request)?;
        Ok(Box::new(TcpByteStream {
            inner: Some(stream),
        }))
    }
}

fn connect_and_send(request: &OutgoingRequest) -> Result<TcpStream, TransportError> {
    // The approved endpoint is HTTPS; TLS termination for this gated suite is expected to be
    // provided by the runner environment (TODO(tt-19): confirm proxy/TLS story).
    let rest = request
        .url
        .split_once("://")
        .map_or(request.url.as_str(), |(_scheme, rest)| rest);
    let (authority, path) = match rest.split_once('/') {
        Some((authority, remainder)) => {
            (authority.to_owned(), format!("/{remainder}"))
        }
        None => (rest.to_owned(), String::from("/")),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (
            host.to_owned(),
            port.parse().map_err(|_| TransportError::ConnectionFailure)?,
        ),
        None => (authority.clone(), 443),
    };
    let mut stream = TcpStream::connect((host.as_str(), port))
        .map_err(|_| TransportError::ConnectionFailure)?;
    stream
        .set_read_timeout(Some(request.timeout))
        .map_err(|_| TransportError::ConnectionFailure)?;
    let mut http =
        format!("{} {path} HTTP/1.1\r\nHost: {authority}\r\n", request.method.as_str());
    for (name, value) in &request.headers {
        http.push_str(&format!("{name}: {value}\r\n"));
    }
    if let Some(body) = &request.body {
        http.push_str(&format!("content-length: {}\r\n", body.len()));
    }
    http.push_str("\r\n");
    stream
        .write_all(http.as_bytes())
        .map_err(|_| TransportError::ConnectionFailure)?;
    if let Some(body) = &request.body {
        stream
            .write_all(body)
            .map_err(|_| TransportError::ConnectionFailure)?;
    }
    Ok(stream)
}

fn parse_status(raw: &[u8]) -> Option<u16> {
    let head = std::str::from_utf8(raw.get(0..12)?).ok()?;
    head.split_whitespace().nth(1)?.parse().ok()
}

struct TcpByteStream {
    inner: Option<TcpStream>,
}

impl ByteStream for TcpByteStream {
    fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        let mut buffer = [0_u8; 4096];
        match self.inner.as_mut().expect("open").read(&mut buffer) {
            Ok(0) => Ok(None),
            Ok(read) => Ok(Some(buffer[..read].to_vec())),
            Err(_) => Err(TransportError::ConnectionFailure),
        }
    }
}

fn endpoint() -> Option<(String, RouteGroupDeclaration)> {
    let url = std::env::var("STUDIO_NET_REAL_ENDPOINT_URL").ok()?;
    // TODO(tt-19): replace with the signed declaration fixture for the approved endpoint.
    Some((
        url,
        RouteGroupDeclaration {
            id: String::from("approved-endpoint"),
            origins: Vec::new(),
            methods: vec![HttpMethod::Get],
            paths: vec![String::from("/")],
            allowed_headers: Vec::new(),
            credential: studio_net::declaration::CredentialSource::Public,
            request_schema: None,
            response_schema: None,
            streaming: None,
            limits: Default::default(),
        },
    ))
}

#[test]
fn real_endpoint_round_trip() {
    let Some((url, _group)) = endpoint() else {
        eprintln!("STUDIO_NET_REAL_ENDPOINT_URL unset; skipping real-endpoint suite");
        return;
    };
    let _transport = Arc::new(RawTcpTransport);
    // TODO(tt-19): construct RestBroker with the signed declaration set and assert the typed
    // round trip, including one SSE stream with observed reconnect behavior.
    let _ = Duration::from_secs(1);
}
