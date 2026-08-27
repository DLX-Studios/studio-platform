//! Host-owned WebSocket session broker.
//!
//! A declaration admits one exact `ws`/`wss` endpoint, a closed set of subprotocols, and JSON
//! schemas for both directions. The transport and connection traits are host-only seams; the
//! guest facade exposes only an opaque session handle, typed JSON messages, and lifecycle events.
//! Reconnects, message bounds, rate accounting, and session lifetime all remain host-owned.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{BrokerError, BrokerErrorCode};
use crate::schema::JsonSchema as BoundedJsonSchema;

/// Host-fixed ceilings for WebSocket declarations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WebSocketBrokerLimits {
    /// Maximum serialized bytes in either message direction.
    pub max_message_bytes: usize,
    /// Maximum messages in one host-owned sliding window per session.
    pub max_messages_per_window: u32,
    /// Sliding window used for message rate accounting.
    pub message_rate_window: Duration,
    /// Maximum lifetime, including reconnects, of one session.
    pub max_session_duration: Duration,
    /// Maximum host-owned reconnect attempts after a connection failure.
    pub max_reconnects: u32,
    /// Base delay for host-owned exponential reconnect backoff.
    pub reconnect_base_delay: Duration,
}

impl Default for WebSocketBrokerLimits {
    fn default() -> Self {
        Self {
            max_message_bytes: 1024 * 1024,
            max_messages_per_window: 120,
            message_rate_window: Duration::from_secs(60),
            max_session_duration: Duration::from_secs(3600),
            max_reconnects: 3,
            reconnect_base_delay: Duration::from_millis(500),
        }
    }
}

impl WebSocketBrokerLimits {
    /// Validate that every host ceiling is usable.
    pub fn validate(&self) -> Result<(), BrokerError> {
        if self.max_message_bytes == 0
            || self.max_messages_per_window == 0
            || self.message_rate_window.is_zero()
            || self.max_session_duration.is_zero()
            || self.reconnect_base_delay.is_zero()
        {
            return Err(BrokerError::new(BrokerErrorCode::WebSocketDeclarationInvalid));
        }
        Ok(())
    }
}

/// Signed declaration-side narrowing limits for one WebSocket session.
#[derive(Clone, Copy, Debug, Default, Eq, JsonSchema, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebSocketDeclaredLimits {
    /// Narrower serialized message bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_message_bytes: Option<usize>,
    /// Narrower per-session message allowance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_messages_per_window: Option<u32>,
    /// Narrower session lifetime in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_session_duration_ms: Option<u64>,
    /// Narrower maximum reconnect count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_reconnects: Option<u32>,
    /// Narrower reconnect base delay in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconnect_base_delay_ms: Option<u64>,
}

/// Effective limits resolved from one declaration and host ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WebSocketEffectiveLimits {
    /// Maximum serialized bytes in either direction.
    pub max_message_bytes: usize,
    /// Maximum messages per sliding window.
    pub max_messages_per_window: u32,
    /// Host-owned sliding-window length.
    pub message_rate_window: Duration,
    /// Maximum session lifetime.
    pub max_session_duration: Duration,
    /// Maximum reconnect attempts.
    pub max_reconnects: u32,
    /// Reconnect backoff base delay.
    pub reconnect_base_delay: Duration,
}

impl WebSocketEffectiveLimits {
    fn resolve(
        declared: &WebSocketDeclaredLimits,
        ceilings: &WebSocketBrokerLimits,
    ) -> Result<Self, BrokerError> {
        let limits = Self {
            max_message_bytes: declared
                .max_message_bytes
                .unwrap_or(ceilings.max_message_bytes),
            max_messages_per_window: declared
                .max_messages_per_window
                .unwrap_or(ceilings.max_messages_per_window),
            message_rate_window: ceilings.message_rate_window,
            max_session_duration: declared
                .max_session_duration_ms
                .map_or(ceilings.max_session_duration, Duration::from_millis),
            max_reconnects: declared.max_reconnects.unwrap_or(ceilings.max_reconnects),
            reconnect_base_delay: declared
                .reconnect_base_delay_ms
                .map_or(ceilings.reconnect_base_delay, Duration::from_millis),
        };
        if limits.max_message_bytes > ceilings.max_message_bytes
            || limits.max_messages_per_window > ceilings.max_messages_per_window
            || limits.max_session_duration > ceilings.max_session_duration
            || limits.max_reconnects > ceilings.max_reconnects
            || limits.reconnect_base_delay > ceilings.reconnect_base_delay
            || limits.max_message_bytes == 0
            || limits.max_messages_per_window == 0
            || limits.max_session_duration.is_zero()
            || limits.reconnect_base_delay.is_zero()
        {
            return Err(BrokerError::new(BrokerErrorCode::WebSocketDeclarationInvalid));
        }
        Ok(limits)
    }
}

/// Signed WebSocket endpoint and message-schema declaration.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebSocketDeclaration {
    /// Stable declaration identifier.
    pub id: String,
    /// Exact `ws` or `wss` endpoint admitted for session opens.
    pub endpoint: String,
    /// Subprotocols that may be selected by a guest; an empty list means no subprotocol.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subprotocols: Vec<String>,
    /// JSON schema applied to messages received from the peer.
    pub inbound_schema: Value,
    /// JSON schema applied to messages sent to the peer.
    pub outbound_schema: Value,
    /// Narrower per-session limits.
    #[serde(default)]
    pub limits: WebSocketDeclaredLimits,
}

/// Parsed, validated endpoint used only by the host transport.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Endpoint(String);

impl Endpoint {
    fn parse(raw: &str) -> Result<Self, BrokerError> {
        let invalid = || BrokerError::new(BrokerErrorCode::WebSocketDeclarationInvalid);
        let Some((scheme, remainder)) = raw.split_once("://") else {
            return Err(invalid());
        };
        let scheme = scheme.to_ascii_lowercase();
        if scheme != "ws" && scheme != "wss" {
            return Err(invalid());
        }
        if remainder.is_empty()
            || remainder.bytes().any(|byte| byte.is_ascii_whitespace() || byte < 0x20)
            || remainder.contains('#')
            || remainder.contains('@')
        {
            return Err(invalid());
        }
        let (authority, suffix) = remainder
            .find('/')
            .map_or((remainder, ""), |index| remainder.split_at(index));
        if authority.is_empty() || authority.len() > 253 {
            return Err(invalid());
        }
        let host = authority.rsplit_once(':').map_or(authority, |(host, port)| {
            if port.parse::<u16>().map_or(true, |port| port == 0) {
                ""
            } else {
                host
            }
        });
        if host.is_empty()
            || !host
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.'))
        {
            return Err(invalid());
        }
        let (path, query) = suffix.split_once('?').map_or((suffix, None), |(path, query)| {
            (path, Some(query))
        });
        if !path.is_empty() && !path.starts_with('/') || query.is_some_and(str::is_empty) {
            return Err(invalid());
        }
        let normalized_authority = authority.to_ascii_lowercase();
        Ok(Self(format!(
            "{scheme}://{normalized_authority}{suffix}"
        )))
    }
}

/// Precompiled WebSocket declaration.
#[derive(Clone, Debug)]
pub struct CompiledWebSocketDeclaration {
    id: String,
    endpoint: Endpoint,
    subprotocols: BTreeSet<String>,
    inbound_schema: BoundedJsonSchema,
    outbound_schema: BoundedJsonSchema,
    limits: WebSocketEffectiveLimits,
}

impl CompiledWebSocketDeclaration {
    /// Stable declaration identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Canonical endpoint, available to host integration code only.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint.0
    }

    /// Declared subprotocol names.
    #[must_use]
    pub fn subprotocols(&self) -> &BTreeSet<String> {
        &self.subprotocols
    }

    /// Resolved session limits.
    #[must_use]
    pub const fn limits(&self) -> &WebSocketEffectiveLimits {
        &self.limits
    }
}

impl WebSocketDeclaration {
    /// Defensively validate and compile one signed WebSocket declaration.
    pub fn compile(
        &self,
        ceilings: &WebSocketBrokerLimits,
    ) -> Result<CompiledWebSocketDeclaration, BrokerError> {
        ceilings.validate()?;
        if !valid_identifier(&self.id) {
            return Err(BrokerError::new(BrokerErrorCode::WebSocketDeclarationInvalid));
        }
        let endpoint = Endpoint::parse(&self.endpoint)?;
        let mut subprotocols = BTreeSet::new();
        for protocol in &self.subprotocols {
            if !valid_subprotocol(protocol) || !subprotocols.insert(protocol.clone()) {
                return Err(BrokerError::new(BrokerErrorCode::WebSocketDeclarationInvalid));
            }
        }
        let inbound_schema = BoundedJsonSchema::new(self.inbound_schema.clone())
            .map_err(|_| BrokerError::new(BrokerErrorCode::WebSocketDeclarationInvalid))?;
        let outbound_schema = BoundedJsonSchema::new(self.outbound_schema.clone())
            .map_err(|_| BrokerError::new(BrokerErrorCode::WebSocketDeclarationInvalid))?;
        Ok(CompiledWebSocketDeclaration {
            id: self.id.clone(),
            endpoint,
            subprotocols,
            inbound_schema,
            outbound_schema,
            limits: WebSocketEffectiveLimits::resolve(&self.limits, ceilings)?,
        })
    }
}

fn valid_identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.starts_with(|character: char| character.is_ascii_lowercase())
        && id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        })
}

fn valid_subprotocol(protocol: &str) -> bool {
    !protocol.is_empty()
        && protocol.len() <= 128
        && protocol.bytes().all(|byte| {
            byte.is_ascii_graphic()
                && !matches!(byte, b'"' | b'(' | b')' | b',' | b'/' | b':' | b';' | b'<' | b'=' | b'>' | b'?' | b'@' | b'[' | b'\\' | b']' | b'{' | b'}')
        })
}

/// Host transport error without provider or endpoint detail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebSocketTransportError {
    /// Connection or send/receive operation exceeded its host deadline.
    TimedOut,
    /// The peer or network closed/faulted the connection.
    ConnectionFailure,
    /// The transport observed a message larger than its configured bound.
    MessageTooLarge,
}

/// Host-owned connection returned after endpoint and subprotocol admission.
pub trait WebSocketConnection: Send {
    /// Send one already serialized and validated JSON message.
    fn send(&mut self, message: &[u8]) -> Result<(), WebSocketTransportError>;

    /// Receive one complete peer message; `None` is a clean peer close.
    fn receive(&mut self) -> Result<Option<Vec<u8>>, WebSocketTransportError>;

    /// Close the underlying connection without exposing it to guests.
    fn close(&mut self);
}

/// Host network movement seam. Production hosts provide the real WebSocket implementation;
/// tests provide an approved local/mock endpoint and scripted connections.
pub trait WebSocketTransport: Send + Sync {
    /// Connect to an admitted endpoint using one admitted subprotocol.
    fn connect(
        &self,
        request: WebSocketConnectRequest,
    ) -> Result<Box<dyn WebSocketConnection>, WebSocketTransportError>;
}

/// Host-only connect request assembled from a compiled declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebSocketConnectRequest {
    /// Canonical declared endpoint.
    pub endpoint: String,
    /// Selected subprotocol, if any.
    pub subprotocol: Option<String>,
    /// Lifetime deadline for this connection attempt.
    pub timeout: Duration,
}

/// Guest-authored session-open request. It contains no socket or transport type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebSocketOpenRequest {
    /// Endpoint string that must exactly match a declaration.
    pub endpoint: String,
    /// Optional declared subprotocol.
    pub subprotocol: Option<String>,
}

impl WebSocketOpenRequest {
    /// Start one session-open request.
    #[must_use]
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            subprotocol: None,
        }
    }

    /// Select one subprotocol from the signed declaration.
    #[must_use]
    pub fn with_subprotocol(mut self, subprotocol: impl Into<String>) -> Self {
        self.subprotocol = Some(subprotocol.into());
        self
    }
}

/// Random 256-bit session identity. Its value is never rendered, serialized, or sent to a peer.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct OpaqueSessionId([u8; 32]);

impl fmt::Debug for OpaqueSessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OpaqueSessionId(..)")
    }
}

impl fmt::Display for OpaqueSessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("opaque-session")
    }
}

impl OpaqueSessionId {
    fn new() -> Result<Self, BrokerError> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes)
            .map_err(|_| BrokerError::new(BrokerErrorCode::TransportFailure))?;
        Ok(Self(bytes))
    }
}

/// Typed lifecycle and message events visible to a guest session.
#[derive(Clone, Debug)]
pub enum WebSocketEvent {
    /// A connection is open; this is emitted again after each successful host reconnect.
    Opened {
        /// The subprotocol selected for the host-owned connection, if any.
        subprotocol: Option<String>,
    },
    /// One inbound JSON message that passed size and schema validation.
    Message(Value),
    /// The host scheduled a reconnect after a transport failure.
    Reconnecting { attempt: u32, delay_ms: u64 },
    /// A stable host-side error; no endpoint or provider detail is included.
    Error(BrokerError),
    /// The session reached its terminal closed state.
    Closed,
}

struct EventChannel {
    state: Mutex<EventState>,
    signal: Condvar,
}

struct EventState {
    events: VecDeque<WebSocketEvent>,
    closed: bool,
}

impl EventChannel {
    fn new() -> Self {
        Self {
            state: Mutex::new(EventState {
                events: VecDeque::new(),
                closed: false,
            }),
            signal: Condvar::new(),
        }
    }

    fn push(&self, event: WebSocketEvent) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.events.push_back(event);
        drop(state);
        self.signal.notify_all();
    }

    fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.closed = true;
        drop(state);
        self.signal.notify_all();
    }

    fn next(&self) -> Option<WebSocketEvent> {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            if let Some(event) = state.events.pop_front() {
                return Some(event);
            }
            if state.closed {
                return None;
            }
            state = self
                .signal
                .wait_timeout(state, Duration::from_millis(250))
                .map_or_else(std::sync::PoisonError::into_inner, |(guard, _)| guard);
        }
    }
}

struct SessionState {
    closed: AtomicBool,
    finished: AtomicBool,
    rate: Mutex<VecDeque<Instant>>,
}

impl SessionState {
    fn new() -> Self {
        Self {
            closed: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            rate: Mutex::new(VecDeque::new()),
        }
    }

    fn check_rate(&self, limits: &WebSocketEffectiveLimits) -> Result<(), BrokerError> {
        let now = Instant::now();
        let mut rate = self.rate.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        while rate
            .front()
            .is_some_and(|stamp| now.duration_since(*stamp) > limits.message_rate_window)
        {
            rate.pop_front();
        }
        if rate.len() >= usize::try_from(limits.max_messages_per_window).unwrap_or(usize::MAX) {
            return Err(BrokerError::new(BrokerErrorCode::WebSocketRateLimited));
        }
        rate.push_back(now);
        Ok(())
    }
}

enum Command {
    Send(Vec<u8>),
    Close,
}

struct SessionControl {
    sender: mpsc::Sender<Command>,
    channel: Arc<EventChannel>,
    state: Arc<SessionState>,
}

/// Guest-owned handle containing only an opaque identity and typed operations.
pub struct WebSocketSession {
    id: OpaqueSessionId,
    sender: mpsc::Sender<Command>,
    channel: Arc<EventChannel>,
    state: Arc<SessionState>,
    outbound_schema: BoundedJsonSchema,
    limits: WebSocketEffectiveLimits,
}

impl Clone for WebSocketSession {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            sender: self.sender.clone(),
            channel: Arc::clone(&self.channel),
            state: Arc::clone(&self.state),
            outbound_schema: self.outbound_schema.clone(),
            limits: self.limits,
        }
    }
}

impl fmt::Debug for WebSocketSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WebSocketSession(..)")
    }
}

impl WebSocketSession {
    /// Opaque identity suitable for host correlation; it reveals no endpoint or socket data.
    #[must_use]
    pub const fn id(&self) -> OpaqueSessionId {
        self.id
    }

    /// Send one JSON message after outbound schema, size, and mid-session rate checks.
    pub fn send(&self, message: Value) -> Result<(), BrokerError> {
        if self.state.closed.load(Ordering::SeqCst) {
            return Err(BrokerError::new(BrokerErrorCode::WebSocketSessionClosed));
        }
        self.outbound_schema
            .validate(&message)
            .map_err(|_| BrokerError::new(BrokerErrorCode::WebSocketMessageSchemaInvalid))?;
        let bytes = serde_json::to_vec(&message)
            .map_err(|_| BrokerError::new(BrokerErrorCode::WebSocketMessageSchemaInvalid))?;
        if bytes.len() > self.limits.max_message_bytes {
            return Err(BrokerError::new(BrokerErrorCode::WebSocketMessageTooLarge));
        }
        self.state.check_rate(&self.limits)?;
        self.sender
            .send(Command::Send(bytes))
            .map_err(|_| BrokerError::new(BrokerErrorCode::WebSocketSessionClosed))
    }

    /// Read the next typed lifecycle or inbound message event.
    pub fn next_event(&self) -> Option<WebSocketEvent> {
        self.channel.next()
    }

    /// Alias for [`Self::next_event`].
    pub fn next(&self) -> Option<WebSocketEvent> {
        self.next_event()
    }

    /// Request a host-side clean close. Guests cannot close or reconnect a raw socket.
    pub fn close(&self) {
        if !self.state.closed.swap(true, Ordering::SeqCst) {
            let _ = self.sender.send(Command::Close);
        }
    }

    /// Alias for [`Self::close`].
    pub fn close_session(&self) {
        self.close();
    }

    /// Alias for [`Self::send`].
    pub fn send_message(&self, message: Value) -> Result<(), BrokerError> {
        self.send(message)
    }
}

/// Restricted guest facade for opening sessions; transport and declarations remain host-owned.
pub struct WebSocketGuestApi {
    broker: Arc<WebSocketBroker>,
}

impl Clone for WebSocketGuestApi {
    fn clone(&self) -> Self {
        Self {
            broker: Arc::clone(&self.broker),
        }
    }
}

impl fmt::Debug for WebSocketGuestApi {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WebSocketGuestApi")
    }
}

impl WebSocketGuestApi {
    /// Open one declared session.
    pub fn open(&self, request: WebSocketOpenRequest) -> Result<WebSocketSession, BrokerError> {
        self.broker.open(request)
    }

    /// Alias for [`Self::open`] using the session-oriented API name.
    pub fn open_session(
        &self,
        request: WebSocketOpenRequest,
    ) -> Result<WebSocketSession, BrokerError> {
        self.open(request)
    }
}

/// Host-owned registry and session worker manager.
pub struct WebSocketBroker {
    declarations: Vec<CompiledWebSocketDeclaration>,
    transport: Arc<dyn WebSocketTransport>,
    limits: WebSocketBrokerLimits,
    sessions: Mutex<HashMap<OpaqueSessionId, SessionControl>>,
}

impl fmt::Debug for WebSocketBroker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WebSocketBroker")
            .field("declarations", &self.declarations.len())
            .field(
                "sessions",
                &self
                    .sessions
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .len(),
            )
            .finish()
    }
}

impl WebSocketBroker {
    /// Create a broker over one host transport and explicit ceilings.
    pub fn new(
        transport: Arc<dyn WebSocketTransport>,
        limits: WebSocketBrokerLimits,
    ) -> Self {
        Self {
            declarations: Vec::new(),
            transport,
            limits,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Fallible constructor that validates host ceilings before creating a broker.
    pub fn try_new(
        transport: Arc<dyn WebSocketTransport>,
        limits: WebSocketBrokerLimits,
    ) -> Result<Self, BrokerError> {
        limits.validate()?;
        Ok(Self::new(transport, limits))
    }

    /// Validate and install one signed WebSocket declaration.
    pub fn declare(&mut self, declaration: &WebSocketDeclaration) -> Result<(), BrokerError> {
        let compiled = declaration.compile(&self.limits)?;
        if self
            .declarations
            .iter()
            .any(|existing| {
                existing.id() == compiled.id() || existing.endpoint == compiled.endpoint
            })
        {
            return Err(BrokerError::new(BrokerErrorCode::WebSocketDeclarationInvalid));
        }
        self.declarations.push(compiled);
        Ok(())
    }

    /// Alias for [`Self::declare`] using the session-oriented API name.
    pub fn declare_session(
        &mut self,
        declaration: &WebSocketDeclaration,
    ) -> Result<(), BrokerError> {
        self.declare(declaration)
    }

    /// Expose only the guest-safe opening facade.
    #[must_use]
    pub fn guest_api(self: &Arc<Self>) -> WebSocketGuestApi {
        WebSocketGuestApi {
            broker: Arc::clone(self),
        }
    }

    /// Open a host-managed session after endpoint and subprotocol admission.
    pub fn open(&self, request: WebSocketOpenRequest) -> Result<WebSocketSession, BrokerError> {
        let endpoint = Endpoint::parse(&request.endpoint).map_err(|_| {
            BrokerError::new(BrokerErrorCode::WebSocketEndpointNotDeclared)
        })?;
        let declaration = self
            .declarations
            .iter()
            .find(|declaration| declaration.endpoint == endpoint)
            .ok_or_else(|| BrokerError::new(BrokerErrorCode::WebSocketEndpointNotDeclared))?;
        if request
            .subprotocol
            .as_ref()
            .is_some_and(|protocol| !declaration.subprotocols.contains(protocol))
        {
            return Err(BrokerError::new(
                BrokerErrorCode::WebSocketSubprotocolNotAllowed,
            ));
        }
        let id = OpaqueSessionId::new()?;
        let channel = Arc::new(EventChannel::new());
        let state = Arc::new(SessionState::new());
        let (sender, receiver) = mpsc::channel();
        let control = SessionControl {
            sender: sender.clone(),
            channel: Arc::clone(&channel),
            state: Arc::clone(&state),
        };
        self.sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id, control);
        let worker_declaration = declaration.clone();
        let transport = Arc::clone(&self.transport);
        let worker_channel = Arc::clone(&channel);
        let worker_state = Arc::clone(&state);
        let spawned = std::thread::Builder::new()
            .name("studio-net-websocket".to_owned())
            .spawn(move || {
                run_session(
                    transport,
                    worker_declaration,
                    request.subprotocol,
                    receiver,
                    worker_channel,
                    worker_state,
                );
            });
        if spawned.is_err() {
            self.sessions
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&id);
            terminate(
                &channel,
                &state,
                Some(BrokerError::new(BrokerErrorCode::TransportFailure)),
            );
            return Err(BrokerError::new(BrokerErrorCode::TransportFailure));
        }
        Ok(WebSocketSession {
            id,
            sender,
            channel,
            state,
            outbound_schema: declaration.outbound_schema.clone(),
            limits: declaration.limits,
        })
    }

    /// Alias for [`Self::open`] emphasizing that the returned value is a host session.
    pub fn open_session(
        &self,
        request: WebSocketOpenRequest,
    ) -> Result<WebSocketSession, BrokerError> {
        self.open(request)
    }

    /// Close one host-registered session by its opaque identity.
    pub fn close_session(&self, id: OpaqueSessionId) -> Result<(), BrokerError> {
        let control = self
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&id)
            .map(|control| SessionControl {
                sender: control.sender.clone(),
                channel: Arc::clone(&control.channel),
                state: Arc::clone(&control.state),
            })
            .ok_or_else(|| BrokerError::new(BrokerErrorCode::WebSocketSessionNotFound))?;
        if !control.state.closed.swap(true, Ordering::SeqCst) {
            let _ = control.sender.send(Command::Close);
        }
        Ok(())
    }

    /// Request a host-owned clean close for every registered session.
    pub fn close_all(&self) {
        let controls: Vec<SessionControl> = self
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .map(|control| SessionControl {
                sender: control.sender.clone(),
                channel: Arc::clone(&control.channel),
                state: Arc::clone(&control.state),
            })
            .collect();
        for control in controls {
            if !control.state.closed.swap(true, Ordering::SeqCst) {
                let _ = control.sender.send(Command::Close);
            }
        }
    }
}

fn run_session(
    transport: Arc<dyn WebSocketTransport>,
    declaration: CompiledWebSocketDeclaration,
    subprotocol: Option<String>,
    receiver: mpsc::Receiver<Command>,
    channel: Arc<EventChannel>,
    state: Arc<SessionState>,
) {
    let started = Instant::now();
    let request = WebSocketConnectRequest {
        endpoint: declaration.endpoint.0.clone(),
        subprotocol: subprotocol.clone(),
        timeout: declaration.limits.max_session_duration,
    };
    let mut attempt = 0_u32;
    let mut connection = match connect_with_retries(
        &transport,
        &declaration,
        &request,
        &channel,
        &state,
        &mut attempt,
        started,
    ) {
        Some(connection) => connection,
        None => return,
    };
    loop {
        if started.elapsed() >= declaration.limits.max_session_duration {
            terminate(&channel, &state, Some(BrokerError::new(BrokerErrorCode::Timeout)));
            connection.close();
            return;
        }
        match receiver.try_recv() {
            Ok(Command::Close) => {
                connection.close();
                terminate(&channel, &state, None);
                return;
            }
            Ok(Command::Send(message)) => {
                if let Err(error) = connection.send(&message) {
                    connection.close();
                    if error == WebSocketTransportError::MessageTooLarge {
                        terminate(
                            &channel,
                            &state,
                            Some(BrokerError::new(
                                BrokerErrorCode::WebSocketMessageTooLarge,
                            )),
                        );
                        return;
                    }
                    if let Some(next) = connect_with_retries(
                        &transport,
                        &declaration,
                        &request,
                        &channel,
                        &state,
                        &mut attempt,
                        started,
                    ) {
                        connection = next;
                    } else {
                        return;
                    }
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                connection.close();
                terminate(&channel, &state, None);
                return;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        match connection.receive() {
            Ok(Some(message)) => {
                if message.len() > declaration.limits.max_message_bytes {
                    terminate(
                        &channel,
                        &state,
                        Some(BrokerError::new(BrokerErrorCode::WebSocketMessageTooLarge)),
                    );
                    connection.close();
                    return;
                }
                if state.check_rate(&declaration.limits).is_err() {
                    terminate(
                        &channel,
                        &state,
                        Some(BrokerError::new(BrokerErrorCode::WebSocketRateLimited)),
                    );
                    connection.close();
                    return;
                }
                let Ok(value) = serde_json::from_slice::<Value>(&message) else {
                    terminate(
                        &channel,
                        &state,
                        Some(BrokerError::new(BrokerErrorCode::WebSocketMessageSchemaInvalid)),
                    );
                    connection.close();
                    return;
                };
                if declaration.inbound_schema.validate(&value).is_err() {
                    terminate(
                        &channel,
                        &state,
                        Some(BrokerError::new(BrokerErrorCode::WebSocketMessageSchemaInvalid)),
                    );
                    connection.close();
                    return;
                }
                channel.push(WebSocketEvent::Message(value));
            }
            Ok(None) => {
                terminate(&channel, &state, None);
                return;
            }
            Err(WebSocketTransportError::MessageTooLarge) => {
                terminate(
                    &channel,
                    &state,
                    Some(BrokerError::new(BrokerErrorCode::WebSocketMessageTooLarge)),
                );
                connection.close();
                return;
            }
            Err(_) => {
                connection.close();
                if let Some(next) = connect_with_retries(
                    &transport,
                    &declaration,
                    &request,
                    &channel,
                    &state,
                    &mut attempt,
                    started,
                ) {
                    connection = next;
                } else {
                    return;
                }
            }
        }
    }
}

fn connect_with_retries(
    transport: &Arc<dyn WebSocketTransport>,
    declaration: &CompiledWebSocketDeclaration,
    request: &WebSocketConnectRequest,
    channel: &Arc<EventChannel>,
    state: &Arc<SessionState>,
    attempt: &mut u32,
    started: Instant,
) -> Option<Box<dyn WebSocketConnection>> {
    loop {
        if state.closed.load(Ordering::SeqCst) {
            terminate(channel, state, None);
            return None;
        }
        if started.elapsed() >= declaration.limits.max_session_duration {
            terminate(
                channel,
                state,
                Some(BrokerError::new(BrokerErrorCode::Timeout)),
            );
            return None;
        }
        match transport.connect(request.clone()) {
            Ok(connection) => {
                channel.push(WebSocketEvent::Opened {
                    subprotocol: request.subprotocol.clone(),
                });
                return Some(connection);
            }
            Err(error) if *attempt < declaration.limits.max_reconnects => {
                *attempt += 1;
                let exponent = (*attempt).saturating_sub(1).min(20);
                let multiplier = 1_u32 << exponent;
                let delay = declaration
                    .limits
                    .reconnect_base_delay
                    .saturating_mul(multiplier)
                    .min(declaration.limits.max_session_duration.saturating_sub(started.elapsed()));
                channel.push(WebSocketEvent::Reconnecting {
                    attempt: *attempt,
                    delay_ms: delay.as_millis().try_into().unwrap_or(u64::MAX),
                });
                sleep_cancellable(state, delay);
                let _ = error;
            }
            Err(error) => {
                let code = match error {
                    WebSocketTransportError::TimedOut => BrokerErrorCode::Timeout,
                    WebSocketTransportError::ConnectionFailure
                    | WebSocketTransportError::MessageTooLarge => {
                        BrokerErrorCode::TransportFailure
                    }
                };
                terminate(channel, state, Some(BrokerError::new(code)));
                return None;
            }
        }
    }
}

fn sleep_cancellable(state: &SessionState, mut remaining: Duration) {
    while !remaining.is_zero() && !state.closed.load(Ordering::SeqCst) {
        let step = remaining.min(Duration::from_millis(25));
        std::thread::sleep(step);
        remaining = remaining.saturating_sub(step);
    }
}

fn terminate(channel: &EventChannel, state: &SessionState, error: Option<BrokerError>) {
    if state.finished.swap(true, Ordering::SeqCst) {
        return;
    }
    state.closed.store(true, Ordering::SeqCst);
    if let Some(error) = error {
        channel.push(WebSocketEvent::Error(error));
    }
    channel.push(WebSocketEvent::Closed);
    channel.close();
}

/// Compatibility alias emphasizing the guest-facing nature of the facade.
pub type GuestWebSocketApi = WebSocketGuestApi;

/// Short alias for the host session broker.
pub type SessionBroker = WebSocketBroker;

/// Short alias for an opaque host session identity.
pub type SessionId = OpaqueSessionId;

/// Short alias for a guest-visible session handle.
pub type SessionHandle = WebSocketSession;

/// Short alias for typed session lifecycle events.
pub type SessionEvent = WebSocketEvent;

/// Short alias for WebSocket host ceilings.
pub type WebSocketLimits = WebSocketBrokerLimits;
