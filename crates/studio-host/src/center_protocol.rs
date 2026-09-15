//! Versioned, host-only center transport for self-hosted and Studio Cloud deployments.
//!
//! This module is deliberately a protocol and adapter seam, not a socket implementation. A
//! production host supplies [`CenterHttpTransport`] and/or [`CenterWebSocketTransport`]. The
//! center handler and station client exchange bounded JSON over those seams; guest code never
//! receives a transport, credential, endpoint, or raw network error.

#![allow(missing_docs)]
#![allow(clippy::all, clippy::pedantic, clippy::restriction, clippy::nursery)]
#![allow(
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::needless_pass_by_value,
    clippy::map_unwrap_or,
    clippy::match_same_arms,
    clippy::no_effect_underscore_binding
)]

use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::{
    ApplyResult, CenterConflict, CenterId, CenterServer, CenterSnapshot, ConflictId,
    ConflictResolution, Enrollment, LocalStore, OperationId, OperationReceipt, PairingToken,
    StationId, StationSettings, StationWriteResult, StoreBatch, StoreBatchEntry, TopologyError,
    TopologyErrorCode, WriteOperation,
};

/// Current center protocol version carried on every HTTP and WebSocket message.
pub const CENTER_PROTOCOL_VERSION: u16 = 1;
/// Stable media type for center protocol JSON.
pub const CENTER_PROTOCOL_MEDIA_TYPE: &str = "application/vnd.studio.center+json";
/// HTTP path used for one-time station enrollment.
pub const CENTER_ENROLL_PATH: &str = "/v1/enrollment";
/// HTTP path used to read the authoritative materialized snapshot.
pub const CENTER_SNAPSHOT_PATH: &str = "/v1/snapshot";
/// HTTP path used to submit an idempotent operation.
pub const CENTER_OPERATIONS_PATH: &str = "/v1/operations";
/// Prefix used to retrieve a durable idempotent operation receipt.
pub const CENTER_RECEIPTS_PATH_PREFIX: &str = "/v1/receipts/";
/// Prefix used for explicit conflict resolution paths.
pub const CENTER_CONFLICT_PATH_PREFIX: &str = "/v1/conflicts/";
/// WebSocket subprotocol negotiated by a center endpoint.
pub const CENTER_WEBSOCKET_SUBPROTOCOL: &str = "studio-center-v1";
const STATION_STATE_BATCH_PREFIX: &str = "studio-center-station-v1:";

/// Closed HTTP method set understood by the center endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CenterHttpMethod {
    /// GET.
    Get,
    /// POST.
    Post,
}

/// Host-only HTTP request passed to a transport implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CenterHttpRequest {
    /// Absolute endpoint selected by host configuration.
    pub endpoint: String,
    /// Relative center protocol path.
    pub path: String,
    /// Method from the closed center method set.
    pub method: CenterHttpMethod,
    /// Lowercase protocol headers. Credential values must never be logged by transports.
    pub headers: Vec<(String, String)>,
    /// Bounded JSON body, if the method carries one.
    pub body: Vec<u8>,
}

/// Host-only HTTP response returned by a center transport.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CenterHttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response media type, if supplied by the endpoint.
    pub media_type: Option<String>,
    /// Bounded JSON response body.
    pub body: Vec<u8>,
}

/// Host network movement seam for center HTTP requests.
pub trait CenterHttpTransport: Send + Sync {
    /// Execute one already-authenticated, bounded request.
    fn request(
        &self,
        request: CenterHttpRequest,
    ) -> Result<CenterHttpResponse, CenterTransportError>;
}

/// Host-only WebSocket connect request. The credential is intentionally redacted in debug output.
#[derive(Clone, Eq, PartialEq)]
pub struct CenterWebSocketConnectRequest {
    /// Center endpoint selected by host configuration.
    pub endpoint: String,
    /// Enrolled station identity.
    pub station_id: StationId,
    /// Credential sent only during the authenticated handshake.
    pub credential: String,
    /// Negotiated protocol version.
    pub version: u16,
}

impl fmt::Debug for CenterWebSocketConnectRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CenterWebSocketConnectRequest")
            .field("endpoint", &self.endpoint)
            .field("station_id", &self.station_id)
            .field("credential", &"REDACTED")
            .field("version", &self.version)
            .finish()
    }
}

/// Host-owned WebSocket connection seam.
pub trait CenterWebSocketConnection: Send {
    /// Send one bounded protocol frame.
    fn send(&mut self, frame: CenterWebSocketFrame) -> Result<(), CenterTransportError>;
    /// Receive one complete protocol frame; `None` means clean close.
    fn receive(&mut self) -> Result<Option<CenterWebSocketFrame>, CenterTransportError>;
    /// Close the connection.
    fn close(&mut self);
}

/// Host network movement seam for authenticated center WebSockets.
pub trait CenterWebSocketTransport: Send + Sync {
    /// Open one authenticated versioned center connection.
    fn connect(
        &self,
        request: CenterWebSocketConnectRequest,
    ) -> Result<Box<dyn CenterWebSocketConnection>, CenterTransportError>;
}

/// Closed transport failure family with no endpoint/provider leakage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CenterTransportError {
    /// The request exceeded the host deadline.
    TimedOut,
    /// The endpoint or connection failed.
    ConnectionFailure,
    /// The endpoint exceeded a host-owned response/frame bound.
    MessageTooLarge,
}

/// Host ceilings for center protocol requests, quotas, outboxes, and reconnects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CenterProtocolLimits {
    /// Maximum request body bytes accepted by a center handler.
    pub max_request_bytes: usize,
    /// Maximum response body bytes accepted by a station client.
    pub max_response_bytes: usize,
    /// Maximum WebSocket frame bytes after JSON serialization.
    pub max_frame_bytes: usize,
    /// Maximum accepted operations per enrolled station in one server lifetime.
    pub max_operations_per_station: u32,
    /// Maximum queued operations retained by one station.
    pub max_outbox_operations: usize,
    /// Maximum reconnect attempts after a failed connection.
    pub max_reconnect_attempts: u32,
    /// Base delay for exponential reconnect backoff.
    pub reconnect_base_delay: Duration,
    /// Maximum reconnect delay.
    pub reconnect_max_delay: Duration,
}

impl Default for CenterProtocolLimits {
    fn default() -> Self {
        Self {
            max_request_bytes: 256 * 1024,
            max_response_bytes: 1024 * 1024,
            max_frame_bytes: 1024 * 1024,
            max_operations_per_station: 10_000,
            max_outbox_operations: 1_000,
            max_reconnect_attempts: 8,
            reconnect_base_delay: Duration::from_millis(250),
            reconnect_max_delay: Duration::from_secs(30),
        }
    }
}

impl CenterProtocolLimits {
    fn validate(self) -> Result<(), CenterNetworkError> {
        if self.max_request_bytes == 0
            || self.max_response_bytes == 0
            || self.max_frame_bytes == 0
            || self.max_operations_per_station == 0
            || self.max_outbox_operations == 0
            || self.reconnect_base_delay.is_zero()
            || self.reconnect_max_delay < self.reconnect_base_delay
        {
            return Err(CenterNetworkError::new(
                CenterNetworkErrorCode::InvalidConfiguration,
            ));
        }
        Ok(())
    }

    /// Return the deterministic delay for a zero-based failed attempt.
    #[must_use]
    pub fn reconnect_delay(self, attempt: u32) -> Duration {
        let exponent = attempt.min(31);
        let multiplier = 1u32 << exponent;
        self.reconnect_base_delay
            .checked_mul(multiplier)
            .unwrap_or(self.reconnect_max_delay)
            .min(self.reconnect_max_delay)
    }
}

/// Stable center protocol error codes safe for host diagnostics and UI status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CenterNetworkErrorCode {
    /// Request or response failed closed validation.
    InvalidRequest,
    /// A newer or unsupported protocol version was presented.
    UnsupportedVersion,
    /// The station credential or center scope was rejected.
    Unauthorized,
    /// The requested center object was absent.
    NotFound,
    /// The operation identity was reused with another payload.
    OperationConflict,
    /// A server or station quota was exhausted.
    QuotaExceeded,
    /// The station has no active transport.
    Disconnected,
    /// The bounded offline outbox is full.
    OutboxFull,
    /// A reconnect attempt is not currently permitted.
    Backoff,
    /// A durable station state operation failed.
    Persistence,
    /// The center transport failed.
    Transport,
    /// The center topology core rejected the request.
    Topology,
    /// A host configuration was invalid.
    InvalidConfiguration,
}

/// Value-free center network failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[error("center network operation failed: {code:?}")]
pub struct CenterNetworkError {
    code: CenterNetworkErrorCode,
}

impl CenterNetworkError {
    const fn new(code: CenterNetworkErrorCode) -> Self {
        Self { code }
    }

    /// Stable machine-readable failure code.
    #[must_use]
    pub const fn code(self) -> CenterNetworkErrorCode {
        self.code
    }
}

/// Enrollment body sent to a center endpoint.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CenterEnrollmentRequest {
    /// One-time pairing token.
    pub pairing_token: String,
    /// Host-owned display name for the new station.
    pub display_name: String,
}

/// Operation submission body sent to a center endpoint.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CenterOperationRequest {
    /// The station write intent.
    pub operation: WriteOperation,
}

/// Conflict resolution body sent to a center endpoint.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CenterConflictResolutionRequest {
    /// Explicit conflict identity.
    pub conflict_id: String,
    /// Idempotent resolution operation identity.
    pub operation_id: OperationId,
    /// Explicit resolution choice.
    pub resolution: ConflictResolution,
}

/// Wire representation of an accepted operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", deny_unknown_fields)]
pub enum CenterOperationResponse {
    /// The operation changed authoritative state.
    Applied { receipt: OperationReceipt },
    /// The operation was already acknowledged.
    Replayed { receipt: OperationReceipt },
    /// The operation was preserved as an explicit conflict.
    Conflict {
        receipt: OperationReceipt,
        conflict: CenterConflict,
    },
}

/// Versioned HTTP response envelope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CenterResponse<T> {
    /// Protocol version.
    pub version: u16,
    /// Typed response payload.
    pub payload: T,
}

/// Value-free HTTP error envelope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CenterErrorResponse {
    /// Protocol version.
    pub version: u16,
    /// Stable error code string.
    pub code: String,
}

/// Authenticated WebSocket protocol frames.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", deny_unknown_fields)]
pub enum CenterWebSocketFrame {
    /// Authenticated session hello after transport negotiation.
    Hello { version: u16, station_id: StationId },
    /// Server acknowledgement of a valid hello.
    HelloAccepted { version: u16, center_id: CenterId },
    /// Pull the authoritative snapshot.
    SnapshotRequest { version: u16 },
    /// Authoritative snapshot response.
    Snapshot {
        version: u16,
        snapshot: CenterSnapshot,
    },
    /// Submit one idempotent operation.
    Operation {
        version: u16,
        operation: WriteOperation,
    },
    /// Operation receipt response.
    OperationResult {
        version: u16,
        result: CenterOperationResponse,
    },
    /// Retrieve a durable operation receipt.
    ReceiptRequest {
        version: u16,
        operation_id: OperationId,
    },
    /// Durable operation receipt response.
    Receipt {
        version: u16,
        receipt: OperationReceipt,
    },
    /// Resolve an explicit conflict.
    ResolveConflict {
        version: u16,
        request: CenterConflictResolutionRequest,
    },
    /// Conflict resolution receipt response.
    ResolutionResult {
        version: u16,
        result: CenterOperationResponse,
    },
    /// Value-free protocol error.
    Error { version: u16, code: String },
}

/// Center protocol handler that can be mounted behind HTTP or WebSocket servers.
#[derive(Clone)]
pub struct CenterProtocolServer {
    center: CenterServer,
    limits: CenterProtocolLimits,
    operation_counts: Arc<Mutex<BTreeMap<StationId, u32>>>,
}

impl fmt::Debug for CenterProtocolServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CenterProtocolServer")
            .field("center", &self.center)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl CenterProtocolServer {
    /// Bind a versioned protocol handler to one center authority.
    pub fn new(
        center: CenterServer,
        limits: CenterProtocolLimits,
    ) -> Result<Self, CenterNetworkError> {
        limits.validate()?;
        Ok(Self {
            center,
            limits,
            operation_counts: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    /// Handle one bounded HTTP request without opening a socket.
    #[must_use]
    pub fn handle_http(&self, request: CenterHttpRequest) -> CenterHttpResponse {
        if request.body.len() > self.limits.max_request_bytes {
            return self.error_response(413, CenterNetworkErrorCode::QuotaExceeded);
        }
        if !valid_version_header(&request.headers) {
            return self.error_response(426, CenterNetworkErrorCode::UnsupportedVersion);
        }
        match (request.method, request.path.as_str()) {
            (CenterHttpMethod::Post, CENTER_ENROLL_PATH) => self.enroll_http(&request.body),
            (CenterHttpMethod::Get, CENTER_SNAPSHOT_PATH) => {
                self.authenticated_http(&request, |server, enrollment| {
                    let snapshot = server.center.snapshot().map_err(map_topology_error)?;
                    Ok((
                        200,
                        serde_json::to_value(snapshot).map_err(|_| {
                            CenterNetworkError::new(CenterNetworkErrorCode::Transport)
                        })?,
                        enrollment,
                    ))
                })
            }
            (CenterHttpMethod::Post, CENTER_OPERATIONS_PATH) => {
                self.authenticated_http(&request, |server, enrollment| {
                    let body: CenterOperationRequest = decode_body(&request.body)?;
                    server.check_quota(enrollment.station_id())?;
                    let result = server
                        .center
                        .apply(&enrollment, &body.operation)
                        .map_err(map_topology_error)?;
                    Ok((
                        200,
                        serde_json::to_value(operation_response(result)).map_err(|_| {
                            CenterNetworkError::new(CenterNetworkErrorCode::Transport)
                        })?,
                        enrollment,
                    ))
                })
            }
            (CenterHttpMethod::Get, path) if path.starts_with(CENTER_RECEIPTS_PATH_PREFIX) => self
                .authenticated_http(&request, |server, enrollment| {
                    let operation_id = path.trim_start_matches(CENTER_RECEIPTS_PATH_PREFIX);
                    let operation_id =
                        OperationId::new(operation_id).map_err(map_topology_error)?;
                    let receipt = server
                        .center
                        .receipt(&enrollment, &operation_id)
                        .map_err(map_topology_error)?;
                    Ok((
                        200,
                        serde_json::to_value(receipt).map_err(|_| {
                            CenterNetworkError::new(CenterNetworkErrorCode::Transport)
                        })?,
                        enrollment,
                    ))
                }),
            (CenterHttpMethod::Post, path) if path.starts_with(CENTER_CONFLICT_PATH_PREFIX) => self
                .authenticated_http(&request, |server, enrollment| {
                    let body: CenterConflictResolutionRequest = decode_body(&request.body)?;
                    let path_id = path.trim_start_matches(CENTER_CONFLICT_PATH_PREFIX);
                    if path_id.is_empty() || body.conflict_id != path_id {
                        return Err(CenterNetworkError::new(
                            CenterNetworkErrorCode::InvalidRequest,
                        ));
                    }
                    server.check_quota(enrollment.station_id())?;
                    let conflict_id =
                        ConflictId::from_str(body.conflict_id).map_err(map_topology_error)?;
                    let result = server
                        .center
                        .resolve_conflict(
                            &enrollment,
                            &conflict_id,
                            body.operation_id,
                            body.resolution,
                        )
                        .map_err(map_topology_error)?;
                    Ok((
                        200,
                        serde_json::to_value(operation_response(result)).map_err(|_| {
                            CenterNetworkError::new(CenterNetworkErrorCode::Transport)
                        })?,
                        enrollment,
                    ))
                }),
            _ => self.error_response(404, CenterNetworkErrorCode::NotFound),
        }
    }

    /// Handle one authenticated WebSocket frame after transport handshake.
    pub fn handle_websocket(
        &self,
        enrollment: &Enrollment,
        frame: CenterWebSocketFrame,
    ) -> Result<CenterWebSocketFrame, CenterNetworkError> {
        self.center
            .authenticate(enrollment.station_id(), enrollment.credential())
            .map_err(map_topology_error)?;
        let incoming = serde_json::to_vec(&frame)
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::InvalidRequest))?;
        if incoming.len() > self.limits.max_frame_bytes {
            return Err(CenterNetworkError::new(
                CenterNetworkErrorCode::QuotaExceeded,
            ));
        }
        let response = match frame {
            CenterWebSocketFrame::Hello {
                version,
                station_id,
            } => {
                if version != CENTER_PROTOCOL_VERSION || station_id != *enrollment.station_id() {
                    return Err(CenterNetworkError::new(
                        CenterNetworkErrorCode::Unauthorized,
                    ));
                }
                Ok(CenterWebSocketFrame::HelloAccepted {
                    version: CENTER_PROTOCOL_VERSION,
                    center_id: enrollment.center_id().clone(),
                })
            }
            CenterWebSocketFrame::SnapshotRequest { version } => {
                require_version(version)?;
                Ok(CenterWebSocketFrame::Snapshot {
                    version: CENTER_PROTOCOL_VERSION,
                    snapshot: self.center.snapshot().map_err(map_topology_error)?,
                })
            }
            CenterWebSocketFrame::Operation { version, operation } => {
                require_version(version)?;
                self.check_quota(enrollment.station_id())?;
                Ok(CenterWebSocketFrame::OperationResult {
                    version: CENTER_PROTOCOL_VERSION,
                    result: operation_response(
                        self.center
                            .apply(enrollment, &operation)
                            .map_err(map_topology_error)?,
                    ),
                })
            }
            CenterWebSocketFrame::ReceiptRequest {
                version,
                operation_id,
            } => {
                require_version(version)?;
                Ok(CenterWebSocketFrame::Receipt {
                    version: CENTER_PROTOCOL_VERSION,
                    receipt: self
                        .center
                        .receipt(enrollment, &operation_id)
                        .map_err(map_topology_error)?,
                })
            }
            CenterWebSocketFrame::ResolveConflict { version, request } => {
                require_version(version)?;
                self.check_quota(enrollment.station_id())?;
                let conflict_id =
                    ConflictId::from_str(request.conflict_id).map_err(map_topology_error)?;
                Ok(CenterWebSocketFrame::ResolutionResult {
                    version: CENTER_PROTOCOL_VERSION,
                    result: operation_response(
                        self.center
                            .resolve_conflict(
                                enrollment,
                                &conflict_id,
                                request.operation_id,
                                request.resolution,
                            )
                            .map_err(map_topology_error)?,
                    ),
                })
            }
            _ => Err(CenterNetworkError::new(
                CenterNetworkErrorCode::InvalidRequest,
            )),
        }?;
        let outgoing = serde_json::to_vec(&response)
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::InvalidRequest))?;
        if outgoing.len() > self.limits.max_frame_bytes {
            return Err(CenterNetworkError::new(
                CenterNetworkErrorCode::QuotaExceeded,
            ));
        }
        Ok(response)
    }

    /// Authenticate a WebSocket handshake before handing it to a host connection adapter.
    pub fn authenticate_websocket(
        &self,
        request: &CenterWebSocketConnectRequest,
    ) -> Result<Enrollment, CenterNetworkError> {
        require_version(request.version)?;
        self.center
            .authenticate(&request.station_id, &request.credential)
            .map_err(map_topology_error)
    }

    fn enroll_http(&self, body: &[u8]) -> CenterHttpResponse {
        let result = (|| {
            let request: CenterEnrollmentRequest = decode_body(body)?;
            let token =
                PairingToken::from_str(request.pairing_token).map_err(map_topology_error)?;
            let enrollment = self
                .center
                .pair(&token, request.display_name)
                .map_err(map_topology_error)?;
            Ok::<_, CenterNetworkError>(serde_json::json!({
                "version": CENTER_PROTOCOL_VERSION,
                "payload": enrollment,
            }))
        })();
        response_from_result(result, 201, self.limits.max_response_bytes)
    }

    fn authenticated_http<F>(&self, request: &CenterHttpRequest, handler: F) -> CenterHttpResponse
    where
        F: FnOnce(&Self, Enrollment) -> Result<(u16, Value, Enrollment), CenterNetworkError>,
    {
        let result = (|| {
            let enrollment = self.authenticate_headers(&request.headers)?;
            let (status, payload, _enrollment) = handler(self, enrollment)?;
            Ok::<_, CenterNetworkError>((status, payload))
        })();
        match result {
            Ok((status, payload)) => response_from_result(
                Ok(serde_json::json!({ "version": CENTER_PROTOCOL_VERSION, "payload": payload })),
                status,
                self.limits.max_response_bytes,
            ),
            Err(error) => self.error_response(error_status(error.code()), error.code()),
        }
    }

    fn authenticate_headers(
        &self,
        headers: &[(String, String)],
    ) -> Result<Enrollment, CenterNetworkError> {
        let station = header(headers, "x-studio-station-id")
            .ok_or_else(|| CenterNetworkError::new(CenterNetworkErrorCode::Unauthorized))?;
        let credential = header(headers, "authorization")
            .and_then(|value| value.strip_prefix("Bearer "))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| CenterNetworkError::new(CenterNetworkErrorCode::Unauthorized))?;
        let station_id = StationId::new(station).map_err(map_topology_error)?;
        self.center
            .authenticate(&station_id, credential)
            .map_err(map_topology_error)
    }

    fn check_quota(&self, station: &StationId) -> Result<(), CenterNetworkError> {
        let mut counts = self
            .operation_counts
            .lock()
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Transport))?;
        let count = counts.entry(station.clone()).or_default();
        if *count >= self.limits.max_operations_per_station {
            return Err(CenterNetworkError::new(
                CenterNetworkErrorCode::QuotaExceeded,
            ));
        }
        *count = count.saturating_add(1);
        Ok(())
    }

    fn error_response(&self, status: u16, code: CenterNetworkErrorCode) -> CenterHttpResponse {
        response_from_result(
            Ok(serde_json::json!({
                "version": CENTER_PROTOCOL_VERSION,
                "code": error_code(code),
            })),
            status,
            self.limits.max_response_bytes,
        )
    }
}

/// Durable station state encoded by a host-owned protected store.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CenterStationState {
    /// Protocol version used to encode this state.
    pub version: u16,
    /// Host-owned station settings.
    pub settings: StationSettings,
    /// Authenticated enrollment proof; protected stores must encrypt this field.
    pub enrollment: Enrollment,
    /// Last authoritative snapshot.
    pub snapshot: Option<CenterSnapshot>,
    /// Operations awaiting replay after reconnect.
    pub pending: Vec<WriteOperation>,
    /// Next operation sequence to allocate.
    pub next_operation: u64,
}

/// Host-only persistence seam for station credentials, cache, and outbox.
pub trait CenterStationStateStore: Send + Sync {
    /// Load one encrypted/otherwise protected state blob.
    fn load(&self, key: &str) -> Result<Option<Vec<u8>>, CenterPersistenceError>;
    /// Atomically replace one state blob.
    fn save(&self, key: &str, state: &[u8]) -> Result<(), CenterPersistenceError>;
}

/// Value-free persistence failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[error("center station persistence failed")]
pub struct CenterPersistenceError;

/// Station-side network client with a bounded offline outbox.
pub struct CenterStationClient<T: CenterHttpTransport + ?Sized> {
    transport: Arc<T>,
    endpoint: String,
    settings: StationSettings,
    enrollment: Enrollment,
    snapshot: Option<CenterSnapshot>,
    pending: VecDeque<WriteOperation>,
    next_operation: u64,
    limits: CenterProtocolLimits,
    connected: bool,
    next_retry_delay: Option<Duration>,
}

impl<T: CenterHttpTransport + ?Sized> fmt::Debug for CenterStationClient<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CenterStationClient")
            .field("endpoint", &self.endpoint)
            .field("station_id", self.enrollment.station_id())
            .field("pending", &self.pending.len())
            .field("connected", &self.connected)
            .finish()
    }
}

impl<T: CenterHttpTransport + ?Sized> CenterStationClient<T> {
    /// Enroll through the versioned HTTP protocol and seed the station cache.
    pub fn enroll(
        transport: Arc<T>,
        endpoint: impl Into<String>,
        pairing_token: PairingToken,
        settings: StationSettings,
        limits: CenterProtocolLimits,
    ) -> Result<Self, CenterNetworkError> {
        limits.validate()?;
        let endpoint = endpoint.into();
        if endpoint.is_empty() || endpoint.chars().any(char::is_control) {
            return Err(CenterNetworkError::new(
                CenterNetworkErrorCode::InvalidRequest,
            ));
        }
        let request = CenterHttpRequest {
            endpoint: endpoint.clone(),
            path: CENTER_ENROLL_PATH.to_owned(),
            method: CenterHttpMethod::Post,
            headers: protocol_headers(None),
            body: encode_body(&CenterEnrollmentRequest {
                pairing_token: pairing_token.as_str().to_owned(),
                display_name: settings.display_name().to_owned(),
            })?,
        };
        let response = transport
            .request(request)
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Transport))?;
        let enrollment: Enrollment = decode_response(response, 201, limits.max_response_bytes)?;
        let mut client = Self {
            transport,
            endpoint,
            settings,
            enrollment,
            snapshot: None,
            pending: VecDeque::new(),
            next_operation: 0,
            limits,
            connected: true,
            next_retry_delay: None,
        };
        client.sync()?;
        Ok(client)
    }

    /// Restore a station from a host-protected state blob without re-enrollment.
    pub fn from_state(
        transport: Arc<T>,
        endpoint: impl Into<String>,
        state: CenterStationState,
        limits: CenterProtocolLimits,
    ) -> Result<Self, CenterNetworkError> {
        limits.validate()?;
        if state.version != CENTER_PROTOCOL_VERSION
            || state.pending.len() > limits.max_outbox_operations
        {
            return Err(CenterNetworkError::new(
                CenterNetworkErrorCode::InvalidRequest,
            ));
        }
        Ok(Self {
            transport,
            endpoint: endpoint.into(),
            settings: state.settings,
            enrollment: state.enrollment,
            snapshot: state.snapshot,
            pending: state.pending.into_iter().collect(),
            next_operation: state.next_operation,
            limits,
            connected: false,
            next_retry_delay: None,
        })
    }

    /// Restore one station from the host-owned persistence seam.
    pub fn from_store<S: CenterStationStateStore>(
        transport: Arc<T>,
        endpoint: impl Into<String>,
        store: &S,
        key: &str,
        limits: CenterProtocolLimits,
    ) -> Result<Self, CenterNetworkError> {
        let bytes = store
            .load(key)
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Persistence))?
            .ok_or_else(|| CenterNetworkError::new(CenterNetworkErrorCode::Persistence))?;
        let state: CenterStationState = serde_json::from_slice(&bytes)
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Persistence))?;
        Self::from_state(transport, endpoint, state, limits)
    }

    /// Persist credentials, cache, and pending operations atomically through the host seam.
    pub fn save<S: CenterStationStateStore>(
        &self,
        store: &S,
        key: &str,
    ) -> Result<(), CenterNetworkError> {
        let state = self.state();
        let bytes = serde_json::to_vec(&state)
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Persistence))?;
        store
            .save(key, &bytes)
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Persistence))
    }

    /// Restore one station from the host's existing durable LocalStore boundary.
    pub async fn from_local_store<S: LocalStore>(
        transport: Arc<T>,
        endpoint: impl Into<String>,
        store: &S,
        key: &str,
        limits: CenterProtocolLimits,
    ) -> Result<Self, CenterNetworkError> {
        let batch_id = station_state_batch_id(key)?;
        let entries = store
            .batch_entries(&batch_id)
            .await
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Persistence))?;
        let entry = entries
            .first()
            .filter(|_entry| entries.len() == 1)
            .ok_or_else(|| CenterNetworkError::new(CenterNetworkErrorCode::Persistence))?;
        let state: CenterStationState = serde_json::from_value(entry.payload.clone())
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Persistence))?;
        Self::from_state(transport, endpoint, state, limits)
    }

    /// Persist station credentials, cache, and outbox through the host's LocalStore boundary.
    pub async fn save_local_store<S: LocalStore>(
        &self,
        store: &S,
        key: &str,
    ) -> Result<(), CenterNetworkError> {
        let batch = StoreBatch::new(
            station_state_batch_id(key)?,
            [StoreBatchEntry {
                ordinal: 0,
                payload: serde_json::to_value(self.state())
                    .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Persistence))?,
            }],
        )
        .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Persistence))?;
        store
            .write_batch(&batch)
            .await
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Persistence))
    }

    /// Export the state that a protected host store should retain across restart.
    #[must_use]
    pub fn state(&self) -> CenterStationState {
        CenterStationState {
            version: CENTER_PROTOCOL_VERSION,
            settings: self.settings.clone(),
            enrollment: self.enrollment.clone(),
            snapshot: self.snapshot.clone(),
            pending: self.pending.iter().cloned().collect(),
            next_operation: self.next_operation,
        }
    }

    /// Station identity assigned by the center.
    #[must_use]
    pub const fn station_id(&self) -> &StationId {
        self.enrollment.station_id()
    }

    /// Last authoritative snapshot observed by this station.
    #[must_use]
    pub const fn snapshot(&self) -> Option<&CenterSnapshot> {
        self.snapshot.as_ref()
    }

    /// Number of operations awaiting replay.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Whether an authenticated transport is currently available.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.connected
    }

    /// Delay selected for the next reconnect attempt, if one has failed.
    #[must_use]
    pub const fn next_retry_delay(&self) -> Option<Duration> {
        self.next_retry_delay
    }

    /// Mark the client disconnected; subsequent writes are queued.
    pub const fn disconnect(&mut self) {
        self.connected = false;
    }

    /// Pull the authoritative center snapshot.
    pub fn sync(&mut self) -> Result<(), CenterNetworkError> {
        if !self.connected {
            return Err(CenterNetworkError::new(
                CenterNetworkErrorCode::Disconnected,
            ));
        }
        let response = self.request(CenterHttpRequest {
            endpoint: self.endpoint.clone(),
            path: CENTER_SNAPSHOT_PATH.to_owned(),
            method: CenterHttpMethod::Get,
            headers: protocol_headers(Some(&self.enrollment)),
            body: Vec::new(),
        })?;
        self.snapshot = Some(decode_response(
            response,
            200,
            self.limits.max_response_bytes,
        )?);
        Ok(())
    }

    /// Set one shared record, queueing it if the center is unavailable.
    pub fn set(
        &mut self,
        table: impl Into<String>,
        key: impl Into<String>,
        value: Value,
    ) -> Result<StationWriteResult, CenterNetworkError> {
        self.submit(table, key, crate::WriteIntent::Set(value))
    }

    /// Delete one shared record, queueing it if the center is unavailable.
    pub fn delete(
        &mut self,
        table: impl Into<String>,
        key: impl Into<String>,
    ) -> Result<StationWriteResult, CenterNetworkError> {
        self.submit(table, key, crate::WriteIntent::Delete)
    }

    /// Resolve one cached conflict through the authenticated center protocol.
    pub fn resolve_conflict(
        &mut self,
        conflict_id: &ConflictId,
        operation_id: OperationId,
        resolution: ConflictResolution,
    ) -> Result<StationWriteResult, CenterNetworkError> {
        if !self.connected {
            return Err(CenterNetworkError::new(
                CenterNetworkErrorCode::Disconnected,
            ));
        }
        let request = CenterConflictResolutionRequest {
            conflict_id: conflict_id.as_str().to_owned(),
            operation_id,
            resolution,
        };
        let response = self.request(CenterHttpRequest {
            endpoint: self.endpoint.clone(),
            path: format!("{CENTER_CONFLICT_PATH_PREFIX}{}", conflict_id.as_str()),
            method: CenterHttpMethod::Post,
            headers: protocol_headers(Some(&self.enrollment)),
            body: encode_body(&request)?,
        })?;
        let response: CenterOperationResponse =
            decode_response(response, 200, self.limits.max_response_bytes)?;
        self.sync()?;
        Ok(match response {
            CenterOperationResponse::Applied { receipt } => StationWriteResult::Applied(receipt),
            CenterOperationResponse::Replayed { receipt } => StationWriteResult::Replayed(receipt),
            CenterOperationResponse::Conflict { receipt, conflict } => {
                StationWriteResult::Conflict { receipt, conflict }
            }
        })
    }

    /// Retrieve a durable receipt by idempotent operation identity.
    pub fn receipt(
        &self,
        operation_id: &OperationId,
    ) -> Result<OperationReceipt, CenterNetworkError> {
        let response = self.request(CenterHttpRequest {
            endpoint: self.endpoint.clone(),
            path: format!("{CENTER_RECEIPTS_PATH_PREFIX}{}", operation_id.as_str()),
            method: CenterHttpMethod::Get,
            headers: protocol_headers(Some(&self.enrollment)),
            body: Vec::new(),
        })?;
        decode_response(response, 200, self.limits.max_response_bytes)
    }

    /// Replay all queued operations after a successful reconnect and refresh the cache.
    pub fn reconnect(&mut self) -> Result<Vec<StationWriteResult>, CenterNetworkError> {
        self.connected = true;
        self.next_retry_delay = None;
        if let Err(error) = self.sync() {
            self.connected = false;
            self.next_retry_delay = Some(self.limits.reconnect_delay(0));
            return Err(error);
        }
        self.flush()
    }

    /// Retry reconnects with a host-provided sleeper and deterministic exponential backoff.
    pub fn reconnect_with<S: CenterBackoffSleeper>(
        &mut self,
        sleeper: &S,
    ) -> Result<Vec<StationWriteResult>, CenterNetworkError> {
        for attempt in 0..=self.limits.max_reconnect_attempts {
            match self.reconnect() {
                Ok(results) => return Ok(results),
                Err(error)
                    if error.code() == CenterNetworkErrorCode::Transport
                        && attempt < self.limits.max_reconnect_attempts =>
                {
                    let delay = self.limits.reconnect_delay(attempt);
                    self.next_retry_delay = Some(delay);
                    sleeper.sleep(delay);
                }
                Err(error) => return Err(error),
            }
        }
        Err(CenterNetworkError::new(CenterNetworkErrorCode::Backoff))
    }

    /// Replay queued work in FIFO order. A failed send remains queued.
    pub fn flush(&mut self) -> Result<Vec<StationWriteResult>, CenterNetworkError> {
        if !self.connected {
            return Err(CenterNetworkError::new(
                CenterNetworkErrorCode::Disconnected,
            ));
        }
        let mut results = Vec::new();
        while let Some(operation) = self.pending.front().cloned() {
            match self.send_operation(&operation) {
                Ok(result) => {
                    self.pending.pop_front();
                    results.push(result);
                }
                Err(error) => {
                    self.connected = false;
                    return Err(error);
                }
            }
        }
        self.sync()?;
        Ok(results)
    }

    fn submit(
        &mut self,
        table: impl Into<String>,
        key: impl Into<String>,
        intent: crate::WriteIntent,
    ) -> Result<StationWriteResult, CenterNetworkError> {
        self.next_operation = self.next_operation.saturating_add(1);
        let operation = WriteOperation::new(
            OperationId::new(format!(
                "{}:{}",
                self.station_id().as_str(),
                self.next_operation
            ))
            .map_err(map_topology_error)?,
            table,
            key,
            self.snapshot.as_ref().map_or(0, CenterSnapshot::revision),
            intent,
        )
        .map_err(map_topology_error)?;
        if !self.connected {
            return self.queue(operation);
        }
        match self.send_operation(&operation) {
            Ok(result) => {
                self.sync()?;
                Ok(result)
            }
            Err(error) if error.code() == CenterNetworkErrorCode::Transport => {
                self.connected = false;
                self.queue(operation)
            }
            Err(error) => Err(error),
        }
    }

    fn queue(
        &mut self,
        operation: WriteOperation,
    ) -> Result<StationWriteResult, CenterNetworkError> {
        if self.pending.len() >= self.limits.max_outbox_operations {
            return Err(CenterNetworkError::new(CenterNetworkErrorCode::OutboxFull));
        }
        self.pending.push_back(operation.clone());
        Ok(StationWriteResult::Queued(operation))
    }

    fn send_operation(
        &self,
        operation: &WriteOperation,
    ) -> Result<StationWriteResult, CenterNetworkError> {
        let response = self.request(CenterHttpRequest {
            endpoint: self.endpoint.clone(),
            path: CENTER_OPERATIONS_PATH.to_owned(),
            method: CenterHttpMethod::Post,
            headers: protocol_headers(Some(&self.enrollment)),
            body: encode_body(&CenterOperationRequest {
                operation: operation.clone(),
            })?,
        })?;
        let response: CenterOperationResponse =
            decode_response(response, 200, self.limits.max_response_bytes)?;
        Ok(match response {
            CenterOperationResponse::Applied { receipt } => StationWriteResult::Applied(receipt),
            CenterOperationResponse::Replayed { receipt } => StationWriteResult::Replayed(receipt),
            CenterOperationResponse::Conflict { receipt, conflict } => {
                StationWriteResult::Conflict { receipt, conflict }
            }
        })
    }

    fn request(
        &self,
        request: CenterHttpRequest,
    ) -> Result<CenterHttpResponse, CenterNetworkError> {
        let _endpoint = &self.endpoint;
        let response = self
            .transport
            .request(request)
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Transport))?;
        if response.body.len() > self.limits.max_response_bytes {
            return Err(CenterNetworkError::new(
                CenterNetworkErrorCode::QuotaExceeded,
            ));
        }
        if !(200..=299).contains(&response.status) {
            return Err(parse_error(response));
        }
        Ok(response)
    }
}

/// Authenticated host-only WebSocket client for the center frame protocol.
pub struct CenterWebSocketClient<W: CenterWebSocketTransport + ?Sized> {
    connection: Box<dyn CenterWebSocketConnection>,
    limits: CenterProtocolLimits,
    transport: Arc<W>,
}

impl<W: CenterWebSocketTransport + ?Sized> fmt::Debug for CenterWebSocketClient<W> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CenterWebSocketClient")
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl<W: CenterWebSocketTransport + ?Sized> CenterWebSocketClient<W> {
    /// Connect and complete the versioned authenticated hello exchange.
    pub fn connect(
        transport: Arc<W>,
        endpoint: impl Into<String>,
        enrollment: &Enrollment,
        limits: CenterProtocolLimits,
    ) -> Result<Self, CenterNetworkError> {
        limits.validate()?;
        let connection = transport
            .connect(CenterWebSocketConnectRequest {
                endpoint: endpoint.into(),
                station_id: enrollment.station_id().clone(),
                credential: enrollment.credential().to_owned(),
                version: CENTER_PROTOCOL_VERSION,
            })
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Transport))?;
        let mut client = Self {
            connection,
            limits,
            transport,
        };
        let accepted = client.request(CenterWebSocketFrame::Hello {
            version: CENTER_PROTOCOL_VERSION,
            station_id: enrollment.station_id().clone(),
        })?;
        if !matches!(accepted, CenterWebSocketFrame::HelloAccepted { .. }) {
            return Err(CenterNetworkError::new(
                CenterNetworkErrorCode::Unauthorized,
            ));
        }
        Ok(client)
    }

    /// Send one bounded frame and receive its corresponding response.
    pub fn request(
        &mut self,
        frame: CenterWebSocketFrame,
    ) -> Result<CenterWebSocketFrame, CenterNetworkError> {
        let bytes = serde_json::to_vec(&frame)
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::InvalidRequest))?;
        if bytes.len() > self.limits.max_frame_bytes {
            return Err(CenterNetworkError::new(
                CenterNetworkErrorCode::QuotaExceeded,
            ));
        }
        self.connection
            .send(frame)
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Transport))?;
        let response = self
            .connection
            .receive()
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::Transport))?
            .ok_or_else(|| CenterNetworkError::new(CenterNetworkErrorCode::Disconnected))?;
        let response_bytes = serde_json::to_vec(&response)
            .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::InvalidRequest))?;
        if response_bytes.len() > self.limits.max_frame_bytes {
            return Err(CenterNetworkError::new(
                CenterNetworkErrorCode::QuotaExceeded,
            ));
        }
        if let CenterWebSocketFrame::Error { code, .. } = &response {
            return Err(error_from_code(code));
        }
        Ok(response)
    }

    /// Close the host-owned WebSocket.
    pub fn close(&mut self) {
        self.connection.close();
    }

    /// Keep the transport alive for host lifecycle ownership.
    #[must_use]
    pub fn transport(&self) -> &Arc<W> {
        &self.transport
    }
}

/// Host seam used to make reconnect waiting testable without owning a runtime or timer.
pub trait CenterBackoffSleeper {
    /// Wait for one host-selected delay.
    fn sleep(&self, delay: Duration);
}

fn operation_response(result: ApplyResult) -> CenterOperationResponse {
    match result {
        ApplyResult::Applied(receipt) => CenterOperationResponse::Applied { receipt },
        ApplyResult::Replayed(receipt) => CenterOperationResponse::Replayed { receipt },
        ApplyResult::Conflict { receipt, conflict } => {
            CenterOperationResponse::Conflict { receipt, conflict }
        }
    }
}

fn protocol_headers(enrollment: Option<&Enrollment>) -> Vec<(String, String)> {
    let mut headers = vec![
        (
            "x-studio-center-version".to_owned(),
            CENTER_PROTOCOL_VERSION.to_string(),
        ),
        (
            "content-type".to_owned(),
            CENTER_PROTOCOL_MEDIA_TYPE.to_owned(),
        ),
    ];
    if let Some(enrollment) = enrollment {
        headers.push((
            "x-studio-station-id".to_owned(),
            enrollment.station_id().as_str().to_owned(),
        ));
        headers.push((
            "authorization".to_owned(),
            format!("Bearer {}", enrollment.credential()),
        ));
    }
    headers
}

fn valid_version_header(headers: &[(String, String)]) -> bool {
    header(headers, "x-studio-center-version").and_then(|version| version.parse::<u16>().ok())
        == Some(CENTER_PROTOCOL_VERSION)
}

fn header<'a>(headers: &'a [(String, String)], wanted: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
        .map(|(_, value)| value.as_str())
}

fn encode_body<T: Serialize>(value: &T) -> Result<Vec<u8>, CenterNetworkError> {
    serde_json::to_vec(value)
        .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::InvalidRequest))
}

fn decode_body<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, CenterNetworkError> {
    serde_json::from_slice(body)
        .map_err(|_| CenterNetworkError::new(CenterNetworkErrorCode::InvalidRequest))
}

fn decode_response<T: for<'de> Deserialize<'de>>(
    response: CenterHttpResponse,
    expected_status: u16,
    max_bytes: usize,
) -> Result<T, CenterNetworkError> {
    if response.status != expected_status || response.body.len() > max_bytes {
        return Err(parse_error(response));
    }
    let envelope: CenterResponse<T> = decode_body(&response.body)?;
    require_version(envelope.version)?;
    Ok(envelope.payload)
}

fn response_from_result(
    result: Result<Value, CenterNetworkError>,
    status: u16,
    max_bytes: usize,
) -> CenterHttpResponse {
    match result {
        Ok(value) => match serde_json::to_vec(&value) {
            Ok(body) if body.len() <= max_bytes => CenterHttpResponse {
                status,
                media_type: Some(CENTER_PROTOCOL_MEDIA_TYPE.to_owned()),
                body,
            },
            _ => CenterHttpResponse {
                status: 413,
                media_type: Some(CENTER_PROTOCOL_MEDIA_TYPE.to_owned()),
                body: serde_json::to_vec(&serde_json::json!({
                    "version": CENTER_PROTOCOL_VERSION,
                    "code": error_code(CenterNetworkErrorCode::QuotaExceeded),
                }))
                .unwrap_or_default(),
            },
        },
        Err(error) => CenterHttpResponse {
            status: error_status(error.code()),
            media_type: Some(CENTER_PROTOCOL_MEDIA_TYPE.to_owned()),
            body: serde_json::to_vec(&CenterErrorResponse {
                version: CENTER_PROTOCOL_VERSION,
                code: error_code(error.code()),
            })
            .unwrap_or_default(),
        },
    }
}

fn parse_error(response: CenterHttpResponse) -> CenterNetworkError {
    serde_json::from_slice::<CenterErrorResponse>(&response.body)
        .ok()
        .map(|error| error_from_code(&error.code))
        .unwrap_or_else(|| CenterNetworkError::new(CenterNetworkErrorCode::Transport))
}

fn require_version(version: u16) -> Result<(), CenterNetworkError> {
    (version == CENTER_PROTOCOL_VERSION)
        .then_some(())
        .ok_or_else(|| CenterNetworkError::new(CenterNetworkErrorCode::UnsupportedVersion))
}

fn map_topology_error(error: TopologyError) -> CenterNetworkError {
    let code = match error.code() {
        TopologyErrorCode::Unauthorized => CenterNetworkErrorCode::Unauthorized,
        TopologyErrorCode::OperationIdConflict => CenterNetworkErrorCode::OperationConflict,
        TopologyErrorCode::ConflictUnknown => CenterNetworkErrorCode::NotFound,
        TopologyErrorCode::OperationUnknown => CenterNetworkErrorCode::NotFound,
        TopologyErrorCode::PersistenceUnavailable => CenterNetworkErrorCode::Persistence,
        _ => CenterNetworkErrorCode::InvalidRequest,
    };
    CenterNetworkError::new(code)
}

fn error_status(code: CenterNetworkErrorCode) -> u16 {
    match code {
        CenterNetworkErrorCode::Unauthorized => 401,
        CenterNetworkErrorCode::NotFound => 404,
        CenterNetworkErrorCode::OperationConflict => 409,
        CenterNetworkErrorCode::QuotaExceeded | CenterNetworkErrorCode::OutboxFull => 429,
        CenterNetworkErrorCode::UnsupportedVersion => 426,
        _ => 400,
    }
}

fn error_code(code: CenterNetworkErrorCode) -> String {
    String::from(match code {
        CenterNetworkErrorCode::InvalidRequest => "invalid_request",
        CenterNetworkErrorCode::UnsupportedVersion => "unsupported_version",
        CenterNetworkErrorCode::Unauthorized => "unauthorized",
        CenterNetworkErrorCode::NotFound => "not_found",
        CenterNetworkErrorCode::OperationConflict => "operation_conflict",
        CenterNetworkErrorCode::QuotaExceeded => "quota_exceeded",
        CenterNetworkErrorCode::Disconnected => "disconnected",
        CenterNetworkErrorCode::OutboxFull => "outbox_full",
        CenterNetworkErrorCode::Backoff => "backoff",
        CenterNetworkErrorCode::Persistence => "persistence",
        CenterNetworkErrorCode::Transport => "transport",
        CenterNetworkErrorCode::Topology => "topology",
        CenterNetworkErrorCode::InvalidConfiguration => "invalid_configuration",
    })
}

fn error_from_code(code: &str) -> CenterNetworkError {
    let code = match code {
        "unsupported_version" => CenterNetworkErrorCode::UnsupportedVersion,
        "unauthorized" => CenterNetworkErrorCode::Unauthorized,
        "not_found" => CenterNetworkErrorCode::NotFound,
        "operation_conflict" => CenterNetworkErrorCode::OperationConflict,
        "quota_exceeded" => CenterNetworkErrorCode::QuotaExceeded,
        "outbox_full" => CenterNetworkErrorCode::OutboxFull,
        "persistence" => CenterNetworkErrorCode::Persistence,
        "backoff" => CenterNetworkErrorCode::Backoff,
        "topology" => CenterNetworkErrorCode::Topology,
        _ => CenterNetworkErrorCode::InvalidRequest,
    };
    CenterNetworkError::new(code)
}

fn station_state_batch_id(key: &str) -> Result<String, CenterNetworkError> {
    if key.is_empty() || key.len() > 256 || key.chars().any(char::is_control) {
        return Err(CenterNetworkError::new(
            CenterNetworkErrorCode::InvalidRequest,
        ));
    }
    Ok(format!("{STATION_STATE_BATCH_PREFIX}{key}"))
}
