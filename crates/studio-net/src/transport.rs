//! Host-only transport abstraction behind the broker.
//!
//! Guests never see any type in this module. Production hosts wire a real network client here;
//! deterministic tests wire scripted fakes. The broker owns all policy (bounds, timeouts,
//! redaction); a transport implements only byte movement for requests the broker already
//! admitted and bounded.

use std::time::Duration;

use crate::declaration::HttpMethod;

/// Closed transport failure family carrying no provider context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportError {
    /// The exchange did not complete inside the supplied deadline.
    TimedOut,
    /// A connection-level failure occurred before or during the exchange.
    ConnectionFailure,
    /// The peer exceeded the caller-declared body bound; bytes were discarded.
    BodyTooLarge,
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::TimedOut => "transport timed out",
            Self::ConnectionFailure => "transport connection failure",
            Self::BodyTooLarge => "response exceeded declared bound",
        })
    }
}

impl std::error::Error for TransportError {}

/// Bounded outgoing request assembled by the broker immediately before send.
///
/// Headers are ordered pairs with lowercase names. Credential material may be appended by the
/// send-time injection sink only; nothing in this structure is ever logged or exposed to guests.
#[derive(Clone, Debug)]
pub struct OutgoingRequest {
    /// Admitted method.
    pub method: HttpMethod,
    /// Absolute URL assembled from the declared origin and admitted path plus query.
    pub url: String,
    /// Ordered lowercase-name headers, including any injected credential header.
    pub headers: Vec<(String, String)>,
    /// Bounded request body, if the route declares one.
    pub body: Option<Vec<u8>>,
    /// Effective per-route-group timeout the transport must honor.
    pub timeout: Duration,
}

/// Bounded incoming response reduced to what the response pipeline needs.
#[derive(Debug)]
pub struct IncomingResponse {
    /// Upstream status code; the broker admits only 200..=299.
    pub status: u16,
    /// Declared-media-type hint such as `application/json`; informational only.
    pub media_type: Option<String>,
    /// Complete bounded body bytes.
    pub body: Vec<u8>,
}

/// Incremental byte source for declared server-sent-event routes.
pub trait ByteStream: Send {
    /// Read the next buffered byte chunk.
    ///
    /// Returns `Ok(None)` on clean end-of-stream. Blocking reads are bounded by the idle timeout
    /// the transport applies from [`OutgoingRequest::timeout`] semantics declared per stream;
    /// cancellation is observed by the broker between chunks.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] for connection failure, idle timeout, or bound violation.
    fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError>;
}

/// Host network movement seam. Implementations must enforce the supplied timeout and byte
/// bounds and must never retain, log, or expose credential-bearing request bytes.
pub trait HttpTransport: Send + Sync {
    /// Execute one admitted, bounded exchange.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] without provider context.
    fn execute(&self, request: OutgoingRequest) -> Result<IncomingResponse, TransportError>;

    /// Open one declared server-sent-event stream.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] without provider context.
    fn open_stream(
        &self,
        request: OutgoingRequest,
    ) -> Result<Box<dyn ByteStream + '_>, TransportError>;
}
