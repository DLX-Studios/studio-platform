//! Host-owned services shared by first-party Studio processes.
//!
//! This crate deliberately exposes typed host seams instead of storage-engine
//! handles. Guests, extensions, agents, and MCP clients do not receive a
//! SurrealDB connection or SurrealQL execution capability.

/// Embedded, host-owned local persistence.
pub mod local_store;
pub mod migrations;

pub use local_store::{
    Durability, EmbeddedLocalStore, LocalStore, LocalStoreDiagnostic, LocalStoreDiagnosticCode,
    LocalStoreError, StoreBatch, StoreBatchEntry, StoreExecutor, StoreMetadata, StoreTask,
    StoreTaskError,
};
pub use migrations::{
    MigrationError, MigrationErrorCode, MigrationLifecycle, MigrationRunner, MigrationState,
    MigrationStepError, MigrationRunReport, RecoveryPoint, MIGRATION_STATE_BATCH_ID,
};
