//! Host-owned services shared by first-party Studio processes.
//!
//! This crate deliberately exposes typed host seams instead of storage-engine
//! handles. Guests, extensions, agents, and MCP clients do not receive a
//! SurrealDB connection or SurrealQL execution capability.

/// Host-mediated application data namespaces and typed collection helpers.
pub mod application_data;
/// Embedded, host-owned local persistence.
pub mod local_store;

pub use application_data::{
    ApplicationDataError, ApplicationDataErrorCode, ApplicationDataGuestApi,
    ApplicationDataNamespace, CollectionDeclaration, CollectionRequest, CollectionResponse,
    FieldDeclaration, FieldType, ForbiddenDataOperation, GuestDataRequest, PatchOperation,
    RecordId, RecordSchema, StoredRecord,
};
pub use local_store::{
    Durability, EmbeddedLocalStore, LocalStore, LocalStoreDiagnostic, LocalStoreDiagnosticCode,
    LocalStoreError, StoreBatch, StoreBatchEntry, StoreExecutor, StoreMetadata, StoreTask,
    StoreTaskError,
};
