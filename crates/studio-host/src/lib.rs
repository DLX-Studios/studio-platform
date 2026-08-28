//! Host-owned services shared by first-party Studio processes.
//!
//! This crate deliberately exposes typed host seams instead of storage-engine
//! handles. Guests, extensions, agents, and MCP clients do not receive a
//! `SurrealDB` connection or `SurrealQL` execution capability.

/// Host-mediated application data namespaces and typed collection helpers.
pub mod application_data;
/// Host-owned append-only application audit records.
pub mod audit_log;
/// Versioned host-only HTTP/WebSocket center protocol and station client.
pub mod center_protocol;
/// Adapter from the host `LocalStore` to Studio Design persistence records.
pub mod designer_persistence;
/// Host-owned local identity and offline session authentication.
pub mod identity;
/// Host-owned, content-addressed Studio Library asset admission and catalog.
pub mod library_assets;
/// Embedded, host-owned local persistence.
pub mod local_store;
pub mod migrations;
/// Host-owned application users, credentials, roles, and row-scoped authorization.
pub mod rbac;
/// Host-owned center authority, station enrollment, offline replay, and conflicts.
pub mod topology;
/// Declarative, host-owned scheduled and event-triggered workflows.
pub mod workflows;

pub use application_data::{
    APPLICATION_DATA_NAMESPACE_VERSION, ApplicationDataError, ApplicationDataErrorCode,
    ApplicationDataGuestApi, ApplicationDataHandle, ApplicationDataHost, ApplicationDataNamespace,
    ApplicationDataQueryGuestApi, CollectionDeclaration, CollectionRequest, CollectionResponse,
    FieldDeclaration, FieldType, ForbiddenDataOperation, GuestDataRequest, PatchOperation,
    QueryDeclaration, QueryLimits, QueryRequest, QueryResponse, QuerySource, RecordId,
    RecordSchema, StoredRecord, SurrealQueryDeclaration, SurrealQueryError, SurrealQueryErrorCode,
    SurrealQueryLimits, SurrealQueryRequest, SurrealQueryResponse,
};
pub use audit_log::{
    AUDIT_LOG_FORMAT_VERSION, AuditEvent, AuditEventKind, AuditEventType, AuditLog,
    AuditLogDiagnostic, AuditLogError, AuditLogErrorCode, AuditQuery, AuditRecord,
};
pub use center_protocol::{
    CENTER_CONFLICT_PATH_PREFIX, CENTER_ENROLL_PATH, CENTER_OPERATIONS_PATH,
    CENTER_PROTOCOL_MEDIA_TYPE, CENTER_PROTOCOL_VERSION, CENTER_RECEIPTS_PATH_PREFIX,
    CENTER_SNAPSHOT_PATH, CENTER_WEBSOCKET_SUBPROTOCOL, CenterBackoffSleeper,
    CenterConflictResolutionRequest, CenterEnrollmentRequest, CenterErrorResponse,
    CenterHttpMethod, CenterHttpRequest, CenterHttpResponse, CenterHttpTransport,
    CenterNetworkError, CenterNetworkErrorCode, CenterOperationRequest, CenterOperationResponse,
    CenterPersistenceError, CenterProtocolLimits, CenterProtocolServer, CenterResponse,
    CenterStationClient, CenterStationState, CenterStationStateStore, CenterTransportError,
    CenterWebSocketClient, CenterWebSocketConnectRequest, CenterWebSocketConnection,
    CenterWebSocketFrame, CenterWebSocketTransport,
};
pub use designer_persistence::LocalStoreDesignerPersistence;
pub use identity::{
    CreateIdentityRequest, IdentityError, IdentityErrorCode, IdentityKind, IdentityService,
    IdentitySession, IdentitySnapshot, IdentityState, IdentitySummary, SessionState,
    SessionSummary,
};
pub use library_assets::{
    AssetAdmission, AssetAdmissionRequest, AssetBlob, AssetFormat, AssetKind, AssetMetadata,
    AssetProvenance, AssetSourceKind, AssetUsage, BlobReference, DeletePolicy, DeleteResult,
    LibraryAsset, LibraryAssetError, LibraryAssetStore, LibraryDiagnostic, LibraryDiagnosticCode,
    LibraryPanelAction, LibraryPanelKey, LibraryPanelState, MediaKind, RuntimeVariant,
    RuntimeVariantSpec,
};
pub use local_store::{
    Durability, EmbeddedLocalStore, LocalStore, LocalStoreDiagnostic, LocalStoreDiagnosticCode,
    LocalStoreError, StoreBatch, StoreBatchEntry, StoreExecutor, StoreMetadata, StoreTask,
    StoreTaskError, SurrealQueryStore,
};
pub use migrations::{
    MIGRATION_STATE_BATCH_ID, MigrationError, MigrationErrorCode, MigrationLifecycle,
    MigrationRunReport, MigrationRunner, MigrationState, MigrationStepError, RecoveryPoint,
};
pub use rbac::{
    ApplicationAuditEvent, ApplicationAuditEventKind, ApplicationAuditOutcome,
    ApplicationAuditSink, ApplicationRbacHandle, ApplicationRbacSettings, ApplicationSession,
    AuthorizationTarget, AuthorizedApplicationDataHandle, CollectionGrant, CredentialInput,
    CredentialKind, DataOperation, RbacError, RbacErrorCode, RoleDefinition, RowScope,
    ThrottlePolicy,
};
pub use topology::{
    ApplyResult, CenterConflict, CenterId, CenterServer, CenterSnapshot, CenterTopology,
    ConflictId, ConflictResolution, ConflictState, Enrollment, OperationId, OperationOutcome,
    OperationReceipt, PairingToken, PersistentCenter, SharedRecord, Station, StationCache,
    StationId, StationLocalState, StationSettings, StationWriteResult, TopologyError,
    TopologyErrorCode, WriteIntent, WriteOperation,
};
pub use workflows::{
    ManualWorkflowClock, MemoryWorkflowAuditLog, MemoryWorkflowRuntime, MissedFirePolicy,
    NoopWorkflowAudit, QueueWorkflowEventSource, RetryPolicy, SystemWorkflowClock, WorkflowAction,
    WorkflowAuditEntry, WorkflowAuditSink, WorkflowClock, WorkflowCommit, WorkflowDefinition,
    WorkflowDiagnostic, WorkflowDiagnosticCode, WorkflowEngine, WorkflowError, WorkflowEvent,
    WorkflowEventSource, WorkflowEventSourceKind, WorkflowPayload, WorkflowPluginEvent,
    WorkflowRunReport, WorkflowRunStatus, WorkflowRuntime, WorkflowRuntimeError,
    WorkflowRuntimeErrorCode, WorkflowTrigger,
};
