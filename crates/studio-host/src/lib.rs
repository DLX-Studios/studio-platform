//! Host-owned services shared by first-party Studio processes.
//!
//! This crate deliberately exposes typed host seams instead of storage-engine
//! handles. Guests, extensions, agents, and MCP clients do not receive a
//! SurrealDB connection or SurrealQL execution capability.

/// Host-mediated application data namespaces and typed collection helpers.
pub mod application_data;
/// Embedded, host-owned local persistence.
pub mod local_store;
/// Host-owned center authority, station enrollment, offline replay, and conflicts.
pub mod topology;

pub use application_data::{
    APPLICATION_DATA_NAMESPACE_VERSION, ApplicationDataError, ApplicationDataErrorCode,
    ApplicationDataGuestApi, ApplicationDataHandle, ApplicationDataHost, ApplicationDataNamespace,
    CollectionDeclaration, CollectionRequest, CollectionResponse, FieldDeclaration, FieldType,
    ForbiddenDataOperation, GuestDataRequest, PatchOperation, RecordId, RecordSchema, StoredRecord,
};
pub use local_store::{
    Durability, EmbeddedLocalStore, LocalStore, LocalStoreDiagnostic, LocalStoreDiagnosticCode,
    LocalStoreError, StoreBatch, StoreBatchEntry, StoreExecutor, StoreMetadata, StoreTask,
    StoreTaskError,
};
pub use topology::{
    ApplyResult, CenterConflict, CenterId, CenterServer, CenterSnapshot, CenterTopology,
    ConflictId, ConflictResolution, ConflictState, Enrollment, OperationId, OperationOutcome,
    OperationReceipt, PairingToken, PersistentCenter, SharedRecord, Station, StationCache,
    StationId, StationLocalState, StationSettings, StationWriteResult, TopologyError,
    TopologyErrorCode, WriteIntent, WriteOperation,
};
