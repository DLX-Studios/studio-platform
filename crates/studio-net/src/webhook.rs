//! Host-owned inbound webhook listeners.
//!
//! A listener is created and driven by the host.  Guests receive only [`WebhookEvent`] values;
//! they never receive a listener, a port, a socket, or source-verification material.  Declarations
//! are compiled before installation and every delivery is bounded, source-verified, schema-
//! validated, rate-limited, lifetime-checked, and recorded through the host audit seam.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::declaration::valid_header_name;
use crate::error::{BrokerError, BrokerErrorCode};
use crate::limits::BrokerLimits;
use crate::schema::JsonSchema;

/// Host ceilings for inbound webhook declarations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WebhookLimits {
    /// Maximum payload size accepted from a listener, in bytes.
    pub max_payload_bytes: usize,
    /// Maximum deliveries per endpoint in one host-owned rate window.
    pub max_requests_per_window: u32,
    /// Sliding rate-window duration.
    pub rate_window: Duration,
    /// Maximum lifetime of an installed declaration.
    pub max_lifetime: Duration,
}

impl Default for WebhookLimits {
    fn default() -> Self {
        Self {
            max_payload_bytes: 1024 * 1024,
            max_requests_per_window: 120,
            rate_window: Duration::from_secs(60),
            max_lifetime: Duration::from_secs(24 * 60 * 60),
        }
    }
}

impl WebhookLimits {
    /// Validate host configuration before any listener is created.
    pub fn validate(&self) -> Result<(), BrokerError> {
        if self.max_payload_bytes == 0
            || self.max_requests_per_window == 0
            || self.rate_window.is_zero()
            || self.max_lifetime.is_zero()
        {
            return Err(BrokerError::new(BrokerErrorCode::WebhookDeclarationInvalid));
        }
        Ok(())
    }
}

/// Signed declaration's source proof strategy.  Secret names are references only; the bytes are
/// resolved by a host-owned [`WebhookSecretResolver`] at delivery time.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SourceVerification {
    /// HMAC-SHA256 over the exact received payload bytes.
    HmacSha256 {
        /// Header carrying either 64 hexadecimal characters or `sha256=<hex>`.
        header: String,
        /// Host-side protected secret reference.
        secret_name: String,
    },
    /// Constant-time comparison of a bearer value against a host-side protected secret.
    BearerToken {
        /// Header carrying the bearer value.
        header: String,
        /// Host-side protected secret reference.
        secret_name: String,
    },
}

/// Narrower limits and expiration supplied by a signed webhook declaration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebhookDeclaredLimits {
    /// Narrower payload byte bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_payload_bytes: Option<usize>,
    /// Narrower endpoint request allowance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_requests_per_window: Option<u32>,
    /// Lifetime from installation, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifetime_ms: Option<u64>,
}

/// Signed application declaration for one inbound endpoint.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebhookDeclaration {
    /// Stable endpoint identity used in typed events and audit records.
    pub id: String,
    /// Host-owned listener route.  It contains no host or port and always starts with `/`.
    pub path: String,
    /// Webhook method; declarations currently require POST to avoid treating arbitrary traffic as
    /// an application event.
    pub method: crate::declaration::HttpMethod,
    /// Bounded payload schema enforced before guest delivery.
    pub payload_schema: Value,
    /// Required source-verification policy.
    pub source_verification: SourceVerification,
    /// Narrower size, rate, and lifetime bounds.
    #[serde(default)]
    pub limits: WebhookDeclaredLimits,
}

#[derive(Clone, Debug)]
struct CompiledWebhook {
    id: String,
    path: String,
    method: crate::declaration::HttpMethod,
    payload_schema: JsonSchema,
    source_verification: SourceVerification,
    max_payload_bytes: usize,
    max_requests_per_window: u32,
    rate_window: Duration,
    expires_at_ms: u64,
}

impl CompiledWebhook {
    fn compile(
        declaration: &WebhookDeclaration,
        limits: WebhookLimits,
        now_ms: u64,
    ) -> Result<Self, BrokerError> {
        let invalid = || BrokerError::new(BrokerErrorCode::WebhookDeclarationInvalid);
        if !valid_id(&declaration.id)
            || declaration.method != crate::declaration::HttpMethod::Post
            || !valid_path(&declaration.path)
        {
            return Err(invalid());
        }
        let payload_schema = JsonSchema::new(declaration.payload_schema.clone()).map_err(|_| invalid())?;
        let source_verification = validate_source(&declaration.source_verification)?;
        let max_payload_bytes = declaration
            .limits
            .max_payload_bytes
            .unwrap_or(limits.max_payload_bytes);
        let max_requests_per_window = declaration
            .limits
            .max_requests_per_window
            .unwrap_or(limits.max_requests_per_window);
        let lifetime_ms = declaration
            .limits
            .lifetime_ms
            .unwrap_or(duration_ms(limits.max_lifetime));
        if max_payload_bytes == 0
            || max_payload_bytes > limits.max_payload_bytes
            || max_requests_per_window == 0
            || max_requests_per_window > limits.max_requests_per_window
            || lifetime_ms == 0
            || lifetime_ms > duration_ms(limits.max_lifetime)
        {
            return Err(invalid());
        }
        let expires_at_ms = now_ms.checked_add(lifetime_ms).ok_or_else(invalid)?;
        Ok(Self {
            id: declaration.id.clone(),
            path: declaration.path.clone(),
            method: declaration.method,
            payload_schema,
            source_verification,
            max_payload_bytes,
            max_requests_per_window,
            rate_window: limits.rate_window,
            expires_at_ms,
        })
    }
}

/// Raw request supplied to the host by an injected listener implementation.
#[derive(Clone, Debug)]
pub struct InboundRequest {
    /// Listener route path, without query or fragment.
    pub path: String,
    /// Received HTTP method.
    pub method: crate::declaration::HttpMethod,
    /// Case-insensitive request headers.
    pub headers: Vec<(String, String)>,
    /// Exact payload bytes.  They are never copied into a guest event until validated.
    pub body: Vec<u8>,
}

/// Schema-validated event visible to guest code.
#[derive(Clone, Debug, PartialEq)]
pub struct WebhookEvent {
    endpoint_id: String,
    payload: Value,
    received_at_ms: u64,
}

impl WebhookEvent {
    /// Stable declared endpoint identity.
    #[must_use]
    pub fn endpoint_id(&self) -> &str {
        &self.endpoint_id
    }

    /// Parsed payload after source and schema validation.
    #[must_use]
    pub const fn payload(&self) -> &Value {
        &self.payload
    }

    /// Host clock timestamp at admission.
    #[must_use]
    pub const fn received_at_ms(&self) -> u64 {
        self.received_at_ms
    }
}

/// Listener movement seam.  Production hosts adapt their HTTP server here; tests inject a queue.
pub trait InboundListener: Send + Sync {
    /// Receive one request, or `Ok(None)` when no request is currently available.
    fn receive(&self) -> Result<Option<InboundRequest>, ListenerError>;
}

/// Safe listener failure family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListenerError {
    /// The host listener failed without exposing network details.
    Failed,
}

/// Injectable host clock used for declaration lifetimes and rate windows.
pub trait WebhookClock: Send + Sync {
    /// Current monotonic-like timestamp in milliseconds.
    fn now_ms(&self) -> u64;
}

/// System clock implementation for host wiring.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemWebhookClock;

impl WebhookClock for SystemWebhookClock {
    fn now_ms(&self) -> u64 {
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
    }
}

/// Resolves a declaration's protected source-verification secret in host memory only.
pub trait WebhookSecretResolver: Send + Sync {
    /// Resolve a named protected secret.  Missing values fail closed.
    fn resolve(&self, secret_name: &str) -> Option<Vec<u8>>;
}

/// Audit record emitted for every listener delivery attempt, accepted or rejected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebhookAuditRecord {
    /// Declared endpoint identity, or `None` when no declaration matched.
    pub endpoint_id: Option<String>,
    /// Host admission timestamp.
    pub timestamp_ms: u64,
    /// Whether the event was delivered to the guest queue.
    pub accepted: bool,
    /// Stable rejection code, if rejected.
    pub rejection: Option<BrokerErrorCode>,
}

/// Host audit hook.  It must not log request bodies, headers, or secret material.
pub trait WebhookAuditSink: Send + Sync {
    /// Record one safe admission outcome.
    fn record(&self, record: WebhookAuditRecord);
}

/// Restricted guest surface: typed events only.
#[derive(Clone, Debug)]
pub struct WebhookGuestApi {
    queue: Arc<Mutex<VecDeque<WebhookEvent>>>,
}

impl WebhookGuestApi {
    /// Read the next already-validated event.
    pub fn next_event(&self) -> Option<WebhookEvent> {
        self.queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()
    }
}

/// Host-owned webhook admission and delivery coordinator.
pub struct WebhookHost {
    listener: Arc<dyn InboundListener>,
    clock: Arc<dyn WebhookClock>,
    limits: WebhookLimits,
    declarations: Mutex<Vec<CompiledWebhook>>,
    rate_windows: Mutex<HashMap<String, VecDeque<u64>>>,
    queue: Arc<Mutex<VecDeque<WebhookEvent>>>,
    secrets: Mutex<Option<Arc<dyn WebhookSecretResolver>>>,
    audit: Mutex<Option<Arc<dyn WebhookAuditSink>>>,
}

impl std::fmt::Debug for WebhookHost {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("WebhookHost")
    }
}

impl WebhookHost {
    /// Construct a host coordinator around a host-owned listener and injectable clock.
    pub fn new(
        listener: Arc<dyn InboundListener>,
        clock: Arc<dyn WebhookClock>,
        limits: WebhookLimits,
    ) -> Result<Self, BrokerError> {
        limits.validate()?;
        Ok(Self {
            listener,
            clock,
            limits,
            declarations: Mutex::new(Vec::new()),
            rate_windows: Mutex::new(HashMap::new()),
            queue: Arc::new(Mutex::new(VecDeque::new())),
            secrets: Mutex::new(None),
            audit: Mutex::new(None),
        })
    }

    /// Install one defensively compiled signed declaration.
    pub fn declare(&self, declaration: &WebhookDeclaration) -> Result<(), BrokerError> {
        let compiled = CompiledWebhook::compile(declaration, self.limits, self.clock.now_ms())?;
        let mut declarations = self
            .declarations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if declarations
            .iter()
            .any(|seen| seen.id == compiled.id || seen.path == compiled.path)
        {
            return Err(BrokerError::new(BrokerErrorCode::WebhookDeclarationInvalid));
        }
        declarations.push(compiled);
        Ok(())
    }

    /// Bind host-only protected source secrets.
    pub fn set_secret_resolver(&self, resolver: Arc<dyn WebhookSecretResolver>) {
        *self
            .secrets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(resolver);
    }

    /// Bind the host audit sink.
    pub fn set_audit_sink(&self, sink: Arc<dyn WebhookAuditSink>) {
        *self
            .audit
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(sink);
    }

    /// Expose only the typed-event queue to guest code.
    #[must_use]
    pub fn guest_api(&self) -> WebhookGuestApi {
        WebhookGuestApi {
            queue: Arc::clone(&self.queue),
        }
    }

    /// Poll the host listener once.  A rejected request is audited and returned as a safe code.
    pub fn poll_once(&self) -> Result<bool, BrokerError> {
        let request = self.listener.receive().map_err(|_| {
            let error = BrokerError::new(BrokerErrorCode::WebhookListenerFailure);
            self.audit(None, false, Some(error.clone().code()));
            error
        })?;
        let Some(request) = request else {
            return Ok(false);
        };
        self.deliver(request).map(|_| true)
    }

    /// Admit one raw listener request.  This is a host integration seam, not a guest API.
    pub fn deliver(&self, request: InboundRequest) -> Result<WebhookEvent, BrokerError> {
        let now_ms = self.clock.now_ms();
        let matched = self.find_declaration(&request.path);
        let endpoint_id = matched.as_ref().map(|declaration| declaration.id.clone());
        let result = self.admit(matched, &request, now_ms);
        match result {
            Ok(event) => {
                self.queue
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push_back(event.clone());
                self.audit(endpoint_id, true, None);
                Ok(event)
            }
            Err(error) => {
                self.audit(endpoint_id, false, Some(error.clone().code()));
                Err(error)
            }
        }
    }

    fn find_declaration(&self, path: &str) -> Option<CompiledWebhook> {
        self.declarations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|declaration| declaration.path == path)
            .cloned()
    }

    fn admit(
        &self,
        declaration: Option<CompiledWebhook>,
        request: &InboundRequest,
        now_ms: u64,
    ) -> Result<WebhookEvent, BrokerError> {
        let Some(declaration) = declaration else {
            return Err(BrokerError::new(BrokerErrorCode::WebhookEndpointNotDeclared));
        };
        if now_ms >= declaration.expires_at_ms {
            return Err(BrokerError::new(BrokerErrorCode::WebhookExpired));
        }
        if request.method != declaration.method {
            return Err(BrokerError::new(BrokerErrorCode::WebhookMethodNotAllowed));
        }
        if request.body.len() > declaration.max_payload_bytes {
            return Err(BrokerError::new(BrokerErrorCode::WebhookPayloadTooLarge));
        }
        self.check_rate(
            &declaration.id,
            declaration.max_requests_per_window,
            declaration.rate_window,
            now_ms,
        )?;
        self.verify_source(&declaration.source_verification, &request.headers, &request.body)?;
        let payload: Value = serde_json::from_slice(&request.body)
            .map_err(|_| BrokerError::new(BrokerErrorCode::WebhookPayloadMalformed))?;
        declaration
            .payload_schema
            .validate(&payload)
            .map_err(|_| BrokerError::new(BrokerErrorCode::WebhookSchemaMismatch))?;
        Ok(WebhookEvent {
            endpoint_id: declaration.id,
            payload,
            received_at_ms: now_ms,
        })
    }

    fn verify_source(
        &self,
        verification: &SourceVerification,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Result<(), BrokerError> {
        let (header, secret_name) = match verification {
            SourceVerification::HmacSha256 { header, secret_name }
            | SourceVerification::BearerToken { header, secret_name } => (header, secret_name),
        };
        let supplied = headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(header))
            .map(|(_, value)| value.as_bytes());
        let secret = self
            .secrets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .and_then(|resolver| resolver.resolve(secret_name));
        let valid = match (verification, supplied, secret.as_deref()) {
            (SourceVerification::HmacSha256 { .. }, Some(value), Some(secret)) => {
                verify_hmac(secret, body, value)
            }
            (SourceVerification::BearerToken { .. }, Some(value), Some(secret)) => {
                constant_time_eq(value, secret)
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(BrokerError::new(BrokerErrorCode::WebhookSourceInvalid))
        }
    }

    fn check_rate(
        &self,
        id: &str,
        max_requests: u32,
        window: Duration,
        now_ms: u64,
    ) -> Result<(), BrokerError> {
        let window_ms = duration_ms(window);
        let mut windows = self
            .rate_windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let stamps = windows.entry(id.to_owned()).or_default();
        while stamps
            .front()
            .is_some_and(|oldest| now_ms.saturating_sub(*oldest) > window_ms)
        {
            stamps.pop_front();
        }
        if stamps.len() >= usize::try_from(max_requests).unwrap_or(usize::MAX) {
            return Err(BrokerError::new(BrokerErrorCode::WebhookRateLimited));
        }
        stamps.push_back(now_ms);
        Ok(())
    }

    fn audit(&self, endpoint_id: Option<String>, accepted: bool, rejection: Option<BrokerErrorCode>) {
        if let Some(sink) = self
            .audit
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            sink.record(WebhookAuditRecord {
                endpoint_id,
                timestamp_ms: self.clock.now_ms(),
                accepted,
                rejection,
            });
        }
    }
}

fn validate_source(source: &SourceVerification) -> Result<SourceVerification, BrokerError> {
    let (header, secret_name) = match source {
        SourceVerification::HmacSha256 { header, secret_name }
        | SourceVerification::BearerToken { header, secret_name } => (header, secret_name),
    };
    if !valid_header_name(header)
        || secret_name.is_empty()
        || secret_name.len() > 128
        || secret_name.chars().any(char::is_control)
    {
        return Err(BrokerError::new(BrokerErrorCode::WebhookDeclarationInvalid));
    }
    Ok(source.clone())
}

fn verify_hmac(secret: &[u8], body: &[u8], supplied: &[u8]) -> bool {
    let mut key = [0_u8; 64];
    if secret.len() > 64 {
        let digest = Sha256::digest(secret);
        key[..32].copy_from_slice(&digest);
    } else {
        key[..secret.len()].copy_from_slice(secret);
    }
    let mut inner = Sha256::new();
    let mut inner_pad = key;
    for byte in &mut inner_pad {
        *byte ^= 0x36;
    }
    inner.update(inner_pad);
    inner.update(body);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    let mut outer_pad = key;
    for byte in &mut outer_pad {
        *byte ^= 0x5c;
    }
    outer.update(outer_pad);
    outer.update(inner_digest);
    let digest = outer.finalize();
    let supplied = supplied
        .strip_prefix(b"sha256=")
        .or_else(|| supplied.strip_prefix(b"SHA256="))
        .unwrap_or(supplied);
    let Some(decoded) = decode_hex(supplied) else {
        return false;
    };
    constant_time_eq(&digest, &decoded)
}

fn decode_hex(value: &[u8]) -> Option<Vec<u8>> {
    if value.len() != 64 {
        return None;
    }
    value
        .chunks_exact(2)
        .map(|pair| Some((hex(pair[0])? << 4) | hex(pair[1])?))
        .collect()
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    for index in 0..left.len().max(right.len()) {
        difference |= usize::from(left.get(index).copied().unwrap_or(0))
            ^ usize::from(right.get(index).copied().unwrap_or(0));
    }
    difference == 0
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.starts_with(|character: char| character.is_ascii_lowercase())
        && id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        })
}

fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.starts_with('/')
        && !path.bytes().any(|byte| matches!(byte, b'?' | b'#'))
        && !path.contains("//")
        && path.len() <= 512
        && path.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~')
        })
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

// Keep the existing host limit type discoverable without coupling webhook declarations to the
// outbound broker's response/stream ceilings.
impl From<BrokerLimits> for WebhookLimits {
    fn from(limits: BrokerLimits) -> Self {
        Self {
            max_payload_bytes: limits.max_request_bytes,
            max_requests_per_window: limits.max_requests_per_window,
            rate_window: limits.rate_window,
            max_lifetime: limits.stream_max_duration,
        }
    }
}
