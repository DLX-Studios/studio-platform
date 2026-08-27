//! Host-only transport abstraction behind the broker.
//!
//! Guests never see any type in this module. Production hosts wire a real network client here;
//! deterministic tests wire scripted fakes. The broker owns all policy (bounds, timeouts,
//! redaction); a transport implements only byte movement for requests the broker already
//! admitted and bounded.

use std::time::Duration;

use crate::declaration::HttpMethod;
use zeroize::Zeroize;

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

/// Host limits passed to an injectable HTTPS client.
///
/// The client owns socket and TLS behavior; this value makes the boundaries explicit at the
/// host seam. Implementations must apply all three deadlines to every exchange and stream read,
/// and must stop reading as soon as either byte ceiling is reached.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportLimits {
    /// Maximum time spent establishing a TCP/TLS connection.
    pub connect_timeout: Duration,
    /// Maximum time spent writing request headers and body.
    pub write_timeout: Duration,
    /// Maximum time allowed for one response read or stream idle gap.
    pub read_timeout: Duration,
    /// Maximum response body bytes for a non-streaming exchange.
    pub max_response_bytes: usize,
    /// Maximum bytes delivered by one streaming response.
    pub max_stream_bytes: usize,
    /// Maximum bytes returned by one streaming read.
    pub max_stream_chunk_bytes: usize,
}

impl Default for TransportLimits {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            write_timeout: Duration::from_secs(30),
            read_timeout: Duration::from_secs(60),
            max_response_bytes: 8 * 1024 * 1024,
            max_stream_bytes: 64 * 1024 * 1024,
            max_stream_chunk_bytes: 1024 * 1024,
        }
    }
}

impl TransportLimits {
    /// Whether every configured deadline and byte ceiling is usable.
    #[must_use]
    pub fn is_valid(self) -> bool {
        !self.connect_timeout.is_zero()
            && !self.write_timeout.is_zero()
            && !self.read_timeout.is_zero()
            && self.max_response_bytes > 0
            && self.max_stream_bytes > 0
            && self.max_stream_chunk_bytes > 0
    }

    fn for_request(self, request_timeout: Duration) -> Self {
        Self {
            connect_timeout: self.connect_timeout.min(request_timeout),
            write_timeout: self.write_timeout.min(request_timeout),
            read_timeout: self.read_timeout.min(request_timeout),
            ..self
        }
    }
}

/// TLS-capable host network client injected into [`ProductionHttpTransport`].
///
/// Implementations are responsible for certificate and hostname validation, refusing plaintext
/// URLs, and honoring the supplied connect/write/read deadlines. The request contains credentials
/// only for the duration of this call; implementations must not retain or log it.
pub trait HttpsClient: Send + Sync {
    /// Execute one bounded HTTPS request.
    fn execute(
        &self,
        request: OutgoingRequest,
        limits: TransportLimits,
    ) -> Result<IncomingResponse, TransportError>;

    /// Open one bounded HTTPS byte stream.
    fn open_stream(
        &self,
        request: OutgoingRequest,
        limits: TransportLimits,
    ) -> Result<Box<dyn ByteStream>, TransportError>;
}

/// Host-owned production transport enforcing HTTPS-client limits at the broker boundary.
///
/// A platform supplies the TLS implementation through [`HttpsClient`]. Keeping that client
/// injectable allows deterministic tests while ensuring production wiring has one auditable path:
/// broker admission, this boundary's limits, then the platform's certificate-validating client.
pub struct ProductionHttpTransport {
    client: std::sync::Arc<dyn HttpsClient>,
    limits: TransportLimits,
}

impl std::fmt::Debug for ProductionHttpTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProductionHttpTransport")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl ProductionHttpTransport {
    /// Construct a production transport over a host TLS client.
    #[must_use]
    pub fn new(client: std::sync::Arc<dyn HttpsClient>, limits: TransportLimits) -> Self {
        Self { client, limits }
    }

    /// Construct a transport while rejecting unusable host limits.
    pub fn try_new(
        client: std::sync::Arc<dyn HttpsClient>,
        limits: TransportLimits,
    ) -> Result<Self, TransportConfigError> {
        if limits.is_valid() {
            Ok(Self::new(client, limits))
        } else {
            Err(TransportConfigError::InvalidLimits)
        }
    }

    /// Return the immutable host transport limits.
    #[must_use]
    pub const fn limits(&self) -> TransportLimits {
        self.limits
    }
}

/// Configuration failure for a production transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportConfigError {
    /// One deadline or byte ceiling was zero.
    InvalidLimits,
}

impl std::fmt::Display for TransportConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("invalid HTTPS transport limits")
    }
}

impl std::error::Error for TransportConfigError {}

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
#[derive(Clone)]
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

impl std::fmt::Debug for OutgoingRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let header_names = self
            .headers
            .iter()
            .map(|(name, _value)| name.as_str())
            .collect::<Vec<_>>();
        formatter
            .debug_struct("OutgoingRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("header_names", &header_names)
            .field("body_bytes", &self.body.as_ref().map_or(0, Vec::len))
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// Bounded incoming response reduced to what the response pipeline needs.
pub struct IncomingResponse {
    /// Upstream status code; the broker admits only 200..=299.
    pub status: u16,
    /// Declared-media-type hint such as `application/json`; informational only.
    pub media_type: Option<String>,
    /// Complete bounded body bytes.
    pub body: Vec<u8>,
}

impl std::fmt::Debug for IncomingResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IncomingResponse")
            .field("status", &self.status)
            .field("media_type", &self.media_type)
            .field("body_bytes", &self.body.len())
            .finish()
    }
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

impl HttpTransport for ProductionHttpTransport {
    fn execute(&self, request: OutgoingRequest) -> Result<IncomingResponse, TransportError> {
        if !self.limits.is_valid() || !is_https_url(&request.url) {
            return Err(TransportError::ConnectionFailure);
        }
        let limits = self.limits.for_request(request.timeout);
        let mut response = self.client.execute(request, limits)?;
        if response.body.len() > limits.max_response_bytes {
            // Drop the response immediately. A client that buffers internally still cannot make
            // the oversized value visible to the broker or its caller.
            response.body.zeroize();
            return Err(TransportError::BodyTooLarge);
        }
        Ok(response)
    }

    fn open_stream(
        &self,
        request: OutgoingRequest,
    ) -> Result<Box<dyn ByteStream + '_>, TransportError> {
        if !self.limits.is_valid() || !is_https_url(&request.url) {
            return Err(TransportError::ConnectionFailure);
        }
        let limits = self.limits.for_request(request.timeout);
        let stream = self.client.open_stream(request, limits)?;
        Ok(Box::new(LimitedByteStream {
            inner: stream,
            bytes: 0,
            limits,
        }))
    }
}

struct LimitedByteStream {
    inner: Box<dyn ByteStream>,
    bytes: usize,
    limits: TransportLimits,
}

impl ByteStream for LimitedByteStream {
    fn read_chunk(&mut self) -> Result<Option<Vec<u8>>, TransportError> {
        let Some(chunk) = self.inner.read_chunk()? else {
            return Ok(None);
        };
        if chunk.len() > self.limits.max_stream_chunk_bytes
            || chunk.len() > self.limits.max_stream_bytes.saturating_sub(self.bytes)
        {
            return Err(TransportError::BodyTooLarge);
        }
        self.bytes = self.bytes.saturating_add(chunk.len());
        Ok(Some(chunk))
    }
}

fn is_https_url(url: &str) -> bool {
    let Some(authority_and_path) = url.strip_prefix("https://") else {
        return false;
    };
    let authority = authority_and_path
        .split_once('/')
        .map_or(authority_and_path, |(authority, _)| authority);
    !authority.is_empty()
        && !authority.contains('@')
        && !authority.bytes().any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
}
