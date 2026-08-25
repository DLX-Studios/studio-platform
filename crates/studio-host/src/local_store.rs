//! Typed, host-owned access to the Designer's embedded local store.
//!
//! The production implementation is exactly SurrealDB 3.2.4 backed by
//! RocksDB. The engine stays private to this module: callers can persist and
//! recover typed batches, but cannot obtain a Surreal handle or execute
//! SurrealQL.

use std::{
    future::Future,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use surrealdb::types::SurrealValue;
use thiserror::Error;
use tokio::{runtime::Runtime, sync::oneshot};

/// Explicitly selected disk-flush policy for accepted store transactions.
///
/// `Every` is the required mode for accepted Designer edits: returning from a
/// successful batch means its transaction has been synced to disk. `Interval`
/// and `Never` are available only for development, benchmarks, and disposable
/// stores; they may lose acknowledged writes on forced termination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Durability {
    /// Sync every transaction commit before reporting success.
    Every,
    /// Let the operating system schedule flushing; acknowledged writes may be lost.
    Never,
    /// Flush periodically; acknowledged writes since the last flush may be lost.
    ///
    /// SurrealDB requires an interval greater than 100 milliseconds.
    Interval(Duration),
}

/// Stable code for a diagnostic that is safe to show outside the storage boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalStoreDiagnosticCode {
    /// The selected store directory is not usable.
    DirectoryInvalid,
    /// The selected durability mode is invalid for the embedded engine.
    DurabilityInvalid,
    /// Recovery was requested for a directory that does not contain a store.
    RecoveryUnavailable,
    /// The host engine manifest could not be parsed or was incomplete.
    EngineManifestCorrupt,
    /// The on-disk engine manifest belongs to an unsupported engine or format.
    EngineIncompatible,
    /// RocksDB could not open the selected directory.
    EngineOpenFailed,
    /// The Store's own schema metadata is malformed.
    SchemaMetadataCorrupt,
    /// The Store schema version is unsupported by this host.
    SchemaIncompatible,
    /// A typed batch was invalid before it reached the engine.
    BatchInvalid,
    /// A storage operation did not complete.
    OperationFailed,
    /// The background executor could not start.
    ExecutorUnavailable,
}

/// Actionable diagnostic that intentionally excludes engine and filesystem error text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalStoreDiagnostic {
    code: LocalStoreDiagnosticCode,
    message: &'static str,
}

impl LocalStoreDiagnostic {
    const fn new(code: LocalStoreDiagnosticCode) -> Self {
        let message = match code {
            LocalStoreDiagnosticCode::DirectoryInvalid => {
                "Choose an existing writable application-data directory."
            }
            LocalStoreDiagnosticCode::DurabilityInvalid => {
                "Choose Every, Never, or an interval greater than 100 milliseconds."
            }
            LocalStoreDiagnosticCode::RecoveryUnavailable => {
                "No local store exists at the selected recovery directory."
            }
            LocalStoreDiagnosticCode::EngineManifestCorrupt => {
                "The local store manifest is damaged. Restore a known-good logical backup."
            }
            LocalStoreDiagnosticCode::EngineIncompatible => {
                "This local store was created by an unsupported engine format. Upgrade or restore it before opening."
            }
            LocalStoreDiagnosticCode::EngineOpenFailed => {
                "The local store could not be opened safely. Restore a known-good logical backup."
            }
            LocalStoreDiagnosticCode::SchemaMetadataCorrupt => {
                "The local store schema metadata is damaged. Restore a known-good logical backup."
            }
            LocalStoreDiagnosticCode::SchemaIncompatible => {
                "This local store needs a supported schema migration before it can open."
            }
            LocalStoreDiagnosticCode::BatchInvalid => "The requested storage batch is invalid.",
            LocalStoreDiagnosticCode::OperationFailed => {
                "The local store operation did not complete. No partial batch was accepted."
            }
            LocalStoreDiagnosticCode::ExecutorUnavailable => {
                "The storage worker could not start. Restart the host process."
            }
        };
        Self { code, message }
    }

    /// Stable machine-readable code.
    #[must_use]
    pub const fn code(&self) -> LocalStoreDiagnosticCode {
        self.code
    }

    /// Safe human-readable recovery guidance.
    #[must_use]
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

/// Local-store failure with a stable, sanitized diagnostic.
#[derive(Debug, Error)]
#[error("{diagnostic_message}")]
pub struct LocalStoreError {
    diagnostic: LocalStoreDiagnostic,
    diagnostic_message: &'static str,
}

impl LocalStoreError {
    const fn new(code: LocalStoreDiagnosticCode) -> Self {
        let diagnostic = LocalStoreDiagnostic::new(code);
        Self {
            diagnostic_message: diagnostic.message,
            diagnostic,
        }
    }

    /// The safe diagnostic suitable for UI, logs, and automation results.
    #[must_use]
    pub const fn diagnostic(&self) -> &LocalStoreDiagnostic {
        &self.diagnostic
    }
}

/// Metadata used to verify that a local store belongs to this host and schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreMetadata {
    schema_version: u32,
    engine_format_version: u32,
}

impl StoreMetadata {
    /// Current Studio schema version stored in the database.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Current host-owned engine manifest format version.
    #[must_use]
    pub const fn engine_format_version(&self) -> u32 {
        self.engine_format_version
    }
}

/// One typed record in an atomic storage batch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, SurrealValue)]
pub struct StoreBatchEntry {
    /// Zero-based order within its batch.
    pub ordinal: u32,
    /// Opaque, serializable host payload. Domain layers define its schema.
    #[surreal(wrap)]
    pub payload: Value,
}

/// One atomic batch of host-owned persistence records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreBatch {
    id: String,
    entries: Vec<StoreBatchEntry>,
}

impl StoreBatch {
    /// Construct a batch after checking its stable ID and ordered entries.
    ///
    /// # Errors
    ///
    /// Returns a safe `BatchInvalid` diagnostic for empty IDs, empty batches,
    /// duplicate ordinals, or ordinal gaps.
    pub fn new(
        id: impl Into<String>,
        entries: impl IntoIterator<Item = StoreBatchEntry>,
    ) -> Result<Self, LocalStoreError> {
        let id = id.into();
        let entries = entries.into_iter().collect::<Vec<_>>();
        if id.is_empty()
            || id.chars().any(char::is_control)
            || entries.is_empty()
            || entries
                .iter()
                .enumerate()
                .any(|(index, entry)| entry.ordinal != index as u32)
        {
            return Err(LocalStoreError::new(LocalStoreDiagnosticCode::BatchInvalid));
        }
        Ok(Self { id, entries })
    }

    /// Stable host-selected identity for the batch.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Ordered records to be persisted together.
    #[must_use]
    pub fn entries(&self) -> &[StoreBatchEntry] {
        &self.entries
    }
}

/// Host-owned persistence interface used by first-party Studio processes.
///
/// Implementations must keep their engine and query language private. All
/// futures and results crossing the boundary are `Send`; callers must pass
/// owned request data rather than GPUI contexts, views, or entities.
pub trait LocalStore: Send + Sync {
    /// Read the initialized schema metadata.
    fn metadata(&self) -> impl Future<Output = Result<StoreMetadata, LocalStoreError>> + Send;

    /// Atomically replace all entries for one stable batch identity.
    fn write_batch(
        &self,
        batch: &StoreBatch,
    ) -> impl Future<Output = Result<(), LocalStoreError>> + Send;

    /// Read one batch's records in their host-defined order.
    fn batch_entries(
        &self,
        batch_id: &str,
    ) -> impl Future<Output = Result<Vec<StoreBatchEntry>, LocalStoreError>> + Send;

    /// Cleanly release the store after all pending calls have completed.
    fn close(self) -> impl Future<Output = Result<(), LocalStoreError>> + Send
    where
        Self: Sized;
}

/// RocksDB-backed SurrealDB implementation of [`LocalStore`].
///
/// The embedded client is never exposed. The chosen directory and durability
/// are retained only for host-level diagnostics and recovery routing.
pub struct EmbeddedLocalStore {
    directory: PathBuf,
    durability: Durability,
}

impl EmbeddedLocalStore {
    /// Open or create a LocalStore in `directory` with explicit durability.
    ///
    /// Call this from [`StoreExecutor`] rather than a GPUI UI callback.
    /// Reopening a compatible directory is idempotent.
    ///
    /// # Errors
    ///
    /// Returns a safe diagnostic if directory preparation, manifest validation,
    /// engine initialization, or schema metadata validation fails.
    pub async fn open(
        _directory: impl Into<PathBuf>,
        _durability: Durability,
    ) -> Result<Self, LocalStoreError> {
        todo!("open and initialize the embedded RocksDB store")
    }

    /// Recover an existing LocalStore after a prior process ended unexpectedly.
    ///
    /// Unlike [`Self::open`], recovery never creates a missing store. RocksDB's
    /// own recovery runs first, then Studio engine/schema metadata is validated.
    ///
    /// # Errors
    ///
    /// Returns `RecoveryUnavailable` for a missing store and stable safe codes
    /// for incompatible or damaged fixtures.
    pub async fn recover(
        _directory: impl Into<PathBuf>,
        _durability: Durability,
    ) -> Result<Self, LocalStoreError> {
        todo!("recover and validate an existing embedded RocksDB store")
    }

    /// Selected directory, for host-level recovery and diagnostic routing.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Selected explicit durability mode.
    #[must_use]
    pub const fn durability(&self) -> Durability {
        self.durability
    }
}

impl LocalStore for EmbeddedLocalStore {
    fn metadata(&self) -> impl Future<Output = Result<StoreMetadata, LocalStoreError>> + Send {
        async move { todo!("read and validate Studio schema metadata") }
    }

    fn write_batch(
        &self,
        _batch: &StoreBatch,
    ) -> impl Future<Output = Result<(), LocalStoreError>> + Send {
        async move { todo!("commit a typed atomic batch") }
    }

    fn batch_entries(
        &self,
        _batch_id: &str,
    ) -> impl Future<Output = Result<Vec<StoreBatchEntry>, LocalStoreError>> + Send {
        async move { todo!("read one typed batch") }
    }

    fn close(self) -> impl Future<Output = Result<(), LocalStoreError>> + Send {
        async move { todo!("release the embedded engine cleanly") }
    }
}

/// Dedicated multithreaded executor for storage calls initiated by a GPUI host.
///
/// GPUI callbacks submit owned `Send` futures through [`StoreExecutor::spawn`]
/// and later await the returned [`StoreTask`] from their UI integration. The
/// helper never accepts a GPUI context, view, or entity reference. This keeps
/// RocksDB and its async work off the UI thread without making GPUI a
/// dependency of the host foundation.
pub struct StoreExecutor {
    runtime: Runtime,
}

impl StoreExecutor {
    /// Create the multithreaded runtime used only inside the LocalStore boundary.
    ///
    /// # Errors
    ///
    /// Returns a safe diagnostic if the host cannot create storage worker threads.
    pub fn new() -> Result<Self, LocalStoreError> {
        todo!("create the storage-only Tokio runtime")
    }

    /// Submit a `Send` storage operation without blocking the caller thread.
    ///
    /// The returned task must be awaited before clean shutdown. Do not capture
    /// GPUI state in `operation`; capture only owned request values and send its
    /// result back to the UI through the host's normal UI executor.
    pub fn spawn<T, Operation>(&self, _operation: Operation) -> StoreTask<T>
    where
        T: Send + 'static,
        Operation: Future<Output = Result<T, LocalStoreError>> + Send + 'static,
    {
        todo!("schedule one storage future on the boundary runtime")
    }

    /// Stop workers after every [`StoreTask`] has resolved and store has closed.
    ///
    /// Dropping this executor before those steps cancels unfinished work, so a
    /// host's shutdown sequence is: stop new work, await tasks, close stores,
    /// then call `shutdown`.
    pub fn shutdown(self) {
        let _ = self.runtime;
        todo!("wait for the storage runtime to shut down")
    }
}

/// Result of one off-UI-thread storage operation.
#[must_use = "await the task before shutting down the LocalStore executor"]
pub struct StoreTask<T> {
    receiver: oneshot::Receiver<Result<T, LocalStoreError>>,
}

impl<T> StoreTask<T> {
    /// Await the typed operation result on the caller's chosen async executor.
    pub async fn resolve(self) -> Result<T, StoreTaskError> {
        let _ = self.receiver;
        todo!("resolve the typed result from the storage runtime")
    }
}

/// Failure while waiting for an off-UI-thread storage operation.
#[derive(Debug, Error)]
pub enum StoreTaskError {
    /// The LocalStore operation returned a safe diagnostic.
    #[error(transparent)]
    Store(#[from] LocalStoreError),
    /// The storage executor shut down before this task could return a result.
    #[error("storage task cancelled during shutdown")]
    Cancelled,
}
