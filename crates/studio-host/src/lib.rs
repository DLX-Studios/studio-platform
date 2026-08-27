//! Host-owned services shared by first-party Studio processes.
//!
//! This crate deliberately exposes typed host seams instead of storage-engine
//! handles. Guests, extensions, agents, and MCP clients do not receive a
//! SurrealDB connection or SurrealQL execution capability.

/// Adapter from the host `LocalStore` to Studio Design persistence records.
pub mod designer_persistence;
/// Host-owned, content-addressed Studio Library asset admission and catalog.
pub mod library_assets;
/// Embedded, host-owned local persistence.
pub mod local_store;

pub use designer_persistence::LocalStoreDesignerPersistence;
pub use library_assets::{
    AssetAdmission, AssetAdmissionRequest, AssetBlob, AssetFormat, AssetKind, AssetMetadata,
    LibraryAsset, MediaKind, AssetProvenance,
    AssetSourceKind, AssetUsage, BlobReference, DeletePolicy, DeleteResult, LibraryAssetError,
    LibraryAssetStore, LibraryDiagnostic, LibraryDiagnosticCode, LibraryPanelAction,
    LibraryPanelKey, LibraryPanelState, RuntimeVariant, RuntimeVariantSpec,
};

pub use local_store::{
    Durability, EmbeddedLocalStore, LocalStore, LocalStoreDiagnostic, LocalStoreDiagnosticCode,
    LocalStoreError, StoreBatch, StoreBatchEntry, StoreExecutor, StoreMetadata, StoreTask,
    StoreTaskError,
};
