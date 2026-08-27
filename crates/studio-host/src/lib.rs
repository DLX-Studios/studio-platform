//! Host-owned services shared by first-party Studio processes.
//!
//! This crate deliberately exposes typed host seams instead of storage-engine
//! handles. Guests, extensions, agents, and MCP clients do not receive a
//! SurrealDB connection or SurrealQL execution capability.

/// Host-mediated application data namespaces and typed collection helpers.
pub mod application_data;
/// Embedded, host-owned local persistence.
pub mod local_store;
/// Adapter from the host `LocalStore` to Studio Design persistence records.
pub mod designer_persistence;

pub use application_data::{
    APPLICATION_DATA_NAMESPACE_VERSION, ApplicationDataError, ApplicationDataErrorCode,
    ApplicationDataGuestApi, ApplicationDataHandle, ApplicationDataHost, ApplicationDataNamespace,
    CollectionDeclaration, CollectionRequest, CollectionResponse, FieldDeclaration, FieldType,
    ForbiddenDataOperation, GuestDataRequest, PatchOperation, RecordId, RecordSchema, StoredRecord,
};
pub use designer_persistence::LocalStoreDesignerPersistence;
pub use local_store::{
    Durability, EmbeddedLocalStore, LocalStore, LocalStoreDiagnostic, LocalStoreDiagnosticCode,
    LocalStoreError, StoreBatch, StoreBatchEntry, StoreExecutor, StoreMetadata, StoreTask,
    StoreTaskError,
};
