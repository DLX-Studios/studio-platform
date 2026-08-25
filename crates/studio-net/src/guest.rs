//! The only broker surface guest code may touch: handles plus typed events.
//!
//! No socket, transport, header-map, or raw HTTP type appears here. Requests are declared
//! shapes; responses are validated JSON values paired with a status code; streams deliver
//! [`StreamEvent`]s that already passed schema validation host-side.

use std::sync::Arc;

use serde_json::Value;

use crate::broker::RestBroker;
use crate::declaration::HttpMethod;
use crate::error::BrokerError;
use crate::streaming::StreamChannel;

/// Guest-authored request description admitted against a signed route group.
#[derive(Clone, Debug)]
pub struct BrokerRequest {
    /// Request origin exactly as declared in some route group (for example,
    /// `https://api.example.com`).
    pub origin: String,
    /// Request method.
    pub method: HttpMethod,
    /// Absolute path starting with `/`, without query string.
    pub path: String,
    /// Raw query string without the leading `?`, if any.
    pub query: Option<String>,
    /// Guest-settable headers; names are case-insensitive and must be declared.
    pub headers: Vec<(String, String)>,
    /// JSON request body, validated against the declared request schema.
    pub body: Option<Value>,
}

impl BrokerRequest {
    /// Start building one request.
    #[must_use]
    pub fn new(origin: impl Into<String>, method: HttpMethod, path: impl Into<String>) -> Self {
        Self {
            origin: origin.into(),
            method,
            path: path.into(),
            query: None,
            headers: Vec::new(),
            body: None,
        }
    }

    /// Attach a raw query string.
    #[must_use]
    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(query.into());
        self
    }

    /// Attach one guest-settable header.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Attach a JSON body.
    #[must_use]
    pub fn with_body(mut self, body: Value) -> Self {
        self.body = Some(body);
        self
    }
}

/// Validated typed response: the only response shape guests can observe.
#[derive(Clone, Debug)]
pub struct TypedResponse {
    status: u16,
    body: Value,
}

impl TypedResponse {
    pub(crate) const fn new(status: u16, body: Value) -> Self {
        Self { status, body }
    }

    /// Upstream status inside the declared success window.
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }

    /// Schema-validated response body.
    #[must_use]
    pub const fn body(&self) -> &Value {
        &self.body
    }
}

/// Typed stream lifecycle and chunk events delivered to guests.
#[derive(Clone, Debug)]
pub enum StreamEvent {
    /// A connection attempt is delivering data for this stream.
    Opened,
    /// One schema-validated server-sent-event chunk payload.
    Chunk(Value),
    /// The host scheduled its own reconnect after a transport failure.
    RetryScheduled {
        /// One-based reconnect attempt number.
        attempt: u32,
        /// Host-owned backoff delay before the next attempt.
        delay_ms: u64,
    },
    /// The upstream closed the stream cleanly after delivering valid chunks.
    Completed,
    /// The stream failed permanently with a stable safe code.
    Failed(BrokerError),
    /// The guest cancelled this stream.
    Cancelled,
}

/// Cancellable typed stream handle handed to guests.
#[derive(Clone, Debug)]
pub struct StreamHandle {
    channel: Arc<StreamChannel>,
}

impl StreamHandle {
    pub(crate) fn new(channel: Arc<StreamChannel>) -> Self {
        Self { channel }
    }

    /// Next typed event; returns `None` once the stream reached a terminal event.
    ///
    /// Blocks until an event is available or cancellation completes.
    pub fn next_event(&self) -> Option<StreamEvent> {
        self.channel.next_event()
    }

    /// Request host-side cancellation. Delivery stops at the next chunk boundary.
    pub fn cancel(&self) {
        self.channel.cancel();
    }
}

/// Restricted broker facade passed to guest runtimes.
///
/// Construction, declaration wiring, credential binding, and transport selection stay host-only;
/// guests receive only admission-checked execution and typed streams.
#[derive(Clone)]
pub struct GuestRestApi {
    broker: Arc<RestBroker>,
}

impl std::fmt::Debug for GuestRestApi {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("GuestRestApi")
    }
}

impl GuestRestApi {
    pub(crate) fn new(broker: Arc<RestBroker>) -> Self {
        Self { broker }
    }

    /// Execute one admitted, bounded, credential-injected request and return its validated
    /// typed response.
    ///
    /// # Errors
    ///
    /// Returns stable [`BrokerError`] codes for every denial or failure shape.
    pub fn execute(&self, request: BrokerRequest) -> Result<TypedResponse, BrokerError> {
        self.broker.execute(request)
    }

    /// Open one declared server-sent-event stream with host-owned reconnect policy.
    ///
    /// # Errors
    ///
    /// Returns stable [`BrokerError`] codes for admission failures before any connection opens.
    pub fn open_stream(&self, request: BrokerRequest) -> Result<StreamHandle, BrokerError> {
        self.broker.open_stream(request)
    }
}
