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
use surrealdb::{
    Surreal,
    engine::local::{Db, RocksDb},
    opt::{Config, capabilities::Capabilities},
    types::SurrealValue,
};
use thiserror::Error;
use tokio::{
    runtime::{Builder, Runtime},
    sync::oneshot,
};
use tokio::time as tokio_time;

/// File marking a fully initialized Studio store next to the engine data.
const ENGINE_MANIFEST_FILE: &str = ".studio-localstore-engine.json";
/// Temporary file used to replace the manifest atomically.
const ENGINE_MANIFEST_TEMPORARY_FILE: &str = ".studio-localstore-engine.json.tmp";
/// Engine identifier recorded in the manifest for the RocksDB-backed engine.
const ENGINE_MANIFEST_ROCKSDB: &str = "rocksdb";
const NAMESPACE: &str = "studio";
const DATABASE: &str = "designer";
/// Record holding the schema metadata singleton.
const TABLE_METADATA: &str = "studio_store_metadata";
/// Key of the schema metadata record.
const METADATA_KEY: &str = "designer";
/// Table holding one record per persisted batch.
const TABLE_BATCH: &str = "studio_store_batch";
const STORE_SCHEMA_VERSION: u32 = 1;
const ENGINE_FORMAT_VERSION: u32 = 1;

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

impl Durability {
    /// Wire value understood by the embedded engine's `sync` endpoint option.
    fn sync_value(self) -> String {
        match self {
            Self::Every => "every".to_owned(),
            Self::Never => "never".to_owned(),
            Self::Interval(duration) => format!("{}ms", duration.as_millis()),
        }
    }

    /// The engine rejects intervals at or below 100 milliseconds.
    const fn is_valid(self) -> bool {
        !matches!(self, Self::Interval(interval) if interval.as_millis() <= 100)
    }
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
                .zip(0u32..)
                .any(|(entry, index)| entry.ordinal != index)
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

#[derive(Clone, Debug, Deserialize, Serialize, SurrealValue)]
struct StoredMetadata {
    schema_version: u32,
    engine_format_version: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct EngineManifest {
    engine: String,
    format_version: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, SurrealValue)]
struct PersistedBatchEntry {
    batch_id: String,
    ordinal: u32,
    #[surreal(wrap)]
    payload: Value,
}

/// One persisted batch record; replacing this single record is the atomic
/// write unit of `write_batch`.
#[derive(Clone, Debug, Deserialize, Serialize, SurrealValue)]
struct PersistedBatch {
    batch_id: String,
    entries: Vec<PersistedBatchEntry>,
}

/// Validates a host-selected batch identity usable inside record keys.
fn batch_id_is_valid(id: &str) -> bool {
    !id.is_empty() && !id.chars().any(char::is_control)
}

/// Projects a validated [`StoreBatch`] onto its persisted record shape.
fn persisted_batch(batch: &StoreBatch) -> PersistedBatch {
    PersistedBatch {
        batch_id: batch.id().to_owned(),
        entries: batch
            .entries()
            .iter()
            .map(|entry| PersistedBatchEntry {
                batch_id: batch.id().to_owned(),
                ordinal: entry.ordinal,
                payload: entry.payload.clone(),
            })
            .collect(),
    }
}

/// Selects the Studio namespace and database on a freshly opened engine.
async fn select_session(
    database: &Surreal<Db>,
) -> Result<(), LocalStoreDiagnosticCode> {
    database
        .use_ns(NAMESPACE)
        .await
        .map_err(|_| LocalStoreDiagnosticCode::EngineOpenFailed)?;
    database
        .use_db(DATABASE)
        .await
        .map_err(|_| LocalStoreDiagnosticCode::EngineOpenFailed)?;
    Ok(())
}

/// Opens the embedded RocksDB engine with the requested disk sync behavior.
// UNVERIFIED(runtime): `Capabilities::none()` denies every SurrealQL function,
// scripting, live queries, and network targets; the SDK-generated typed
// statements (SELECT/UPSERT) are expected not to require any of them.
async fn connect_rocksdb(
    directory: &Path,
    durability: Durability,
) -> Result<Surreal<Db>, LocalStoreDiagnosticCode> {
    let config = Config::new().capabilities(Capabilities::none());
    let database = Surreal::new::<RocksDb>((directory, config))
        .sync(durability.sync_value())
        .await
        .map_err(|_| LocalStoreDiagnosticCode::EngineOpenFailed)?;
    Ok(database)
}

/// Reads the engine manifest, returning `None` when no store was initialized.
fn read_manifest(
    directory: &Path,
) -> Result<Option<EngineManifest>, LocalStoreDiagnosticCode> {
    let Ok(raw) = std::fs::read_to_string(directory.join(ENGINE_MANIFEST_FILE)) else {
        return Ok(None);
    };
    serde_json::from_str::<EngineManifest>(&raw)
        .map(Some)
        .map_err(|_| LocalStoreDiagnosticCode::EngineManifestCorrupt)
}

/// Rejects manifests recorded by other engines or unsupported formats.
fn validate_manifest(
    manifest: &EngineManifest,
) -> Result<(), LocalStoreDiagnosticCode> {
    if manifest.engine == ENGINE_MANIFEST_ROCKSDB
        && manifest.format_version == ENGINE_FORMAT_VERSION
    {
        Ok(())
    } else {
        Err(LocalStoreDiagnosticCode::EngineIncompatible)
    }
}

/// Publishes the manifest atomically, marking the store fully initialized.
fn write_manifest(directory: &Path) -> Result<(), LocalStoreDiagnosticCode> {
    let manifest = EngineManifest {
        engine: ENGINE_MANIFEST_ROCKSDB.to_owned(),
        format_version: ENGINE_FORMAT_VERSION,
    };
    let raw = serde_json::to_string(&manifest)
        .map_err(|_| LocalStoreDiagnosticCode::OperationFailed)?;
    let temporary = directory.join(ENGINE_MANIFEST_TEMPORARY_FILE);
    std::fs::write(&temporary, raw)
        .map_err(|_| LocalStoreDiagnosticCode::DirectoryInvalid)?;
    std::fs::rename(&temporary, directory.join(ENGINE_MANIFEST_FILE))
        .map_err(|_| LocalStoreDiagnosticCode::DirectoryInvalid)?;
    Ok(())
}

/// Reads the schema metadata singleton; any read failure is treated as damage.
async fn read_schema_metadata(
    database: &Surreal<Db>,
) -> Result<Option<StoredMetadata>, LocalStoreDiagnosticCode> {
    let stored: Option<StoredMetadata> = database
        .select((TABLE_METADATA, METADATA_KEY))
        .await
        .map_err(|_| LocalStoreDiagnosticCode::SchemaMetadataCorrupt)?;
    Ok(stored)
}

/// Rejects schema metadata written by newer hosts or other engine formats.
fn validate_schema_metadata(
    metadata: &StoredMetadata,
) -> Result<(), LocalStoreDiagnosticCode> {
    if metadata.schema_version > STORE_SCHEMA_VERSION
        || metadata.engine_format_version != ENGINE_FORMAT_VERSION
    {
        return Err(LocalStoreDiagnosticCode::SchemaIncompatible);
    }
    Ok(())
}

/// Returns the stored schema metadata, initializing it on a fresh store.
async fn ensure_schema_metadata(
    database: &Surreal<Db>,
) -> Result<StoredMetadata, LocalStoreDiagnosticCode> {
    match read_schema_metadata(database).await? {
        Some(found) => {
            validate_schema_metadata(&found)?;
            Ok(found)
        }
        None => {
            let initial = StoredMetadata {
                schema_version: STORE_SCHEMA_VERSION,
                engine_format_version: ENGINE_FORMAT_VERSION,
            };
            let written: Result<Option<StoredMetadata>, surrealdb::Error> = database
                .upsert((TABLE_METADATA, METADATA_KEY))
                .content(initial)
                .await;
            written
                .map_err(|_| LocalStoreDiagnosticCode::OperationFailed)?
                .ok_or(LocalStoreDiagnosticCode::OperationFailed)
        }
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
    database: Surreal<Db>,
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
        directory: impl Into<PathBuf>,
        durability: Durability,
    ) -> Result<Self, LocalStoreError> {
        let directory = directory.into();
        if !durability.is_valid() {
            return Err(LocalStoreError::new(
                LocalStoreDiagnosticCode::DurabilityInvalid,
            ));
        }
        std::fs::create_dir_all(&directory)
            .map_err(|_| LocalStoreError::new(LocalStoreDiagnosticCode::DirectoryInvalid))?;
        // A missing manifest marks a fresh store; a present one is validated so
        // an incompatible store never gets silently reopened by `open`.
        let fresh_store = read_manifest(&directory)
            .map_err(LocalStoreError::new)?
            .is_none();
        let database =
            connect_rocksdb(&directory, durability).await.map_err(LocalStoreError::new)?;
        select_session(&database).await.map_err(LocalStoreError::new)?;
        ensure_schema_metadata(&database).await.map_err(LocalStoreError::new)?;
        if fresh_store {
            write_manifest(&directory).map_err(LocalStoreError::new)?;
        }
        Ok(Self {
            database,
            directory,
            durability,
        })
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
        directory: impl Into<PathBuf>,
        durability: Durability,
    ) -> Result<Self, LocalStoreError> {
        let directory = directory.into();
        if !durability.is_valid() {
            return Err(LocalStoreError::new(
                LocalStoreDiagnosticCode::DurabilityInvalid,
            ));
        }
        // Recovery never creates a store: the manifest is the marker of a fully
        // initialized one, and its absence means there is nothing to recover.
        match read_manifest(&directory) {
            Ok(Some(manifest)) => validate_manifest(&manifest).map_err(LocalStoreError::new)?,
            Ok(None) => {
                return Err(LocalStoreError::new(
                    LocalStoreDiagnosticCode::RecoveryUnavailable,
                ));
            }
            Err(code) => return Err(LocalStoreError::new(code)),
        }
        let database =
            connect_rocksdb(&directory, durability).await.map_err(LocalStoreError::new)?;
        select_session(&database).await.map_err(LocalStoreError::new)?;
        // A manifest exists but schema metadata does not: the store is damaged.
        match read_schema_metadata(&database).await {
            Ok(Some(found)) => {
                validate_schema_metadata(&found).map_err(LocalStoreError::new)?;
            }
            Ok(None) => {
                return Err(LocalStoreError::new(
                    LocalStoreDiagnosticCode::SchemaMetadataCorrupt,
                ));
            }
            Err(code) => return Err(LocalStoreError::new(code)),
        }
        Ok(Self {
            database,
            directory,
            durability,
        })
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
        async move {
            ensure_schema_metadata(&self.database)
                .await
                .map(|found| StoreMetadata {
                    schema_version: found.schema_version,
                    engine_format_version: found.engine_format_version,
                })
                .map_err(LocalStoreError::new)
        }
    }

    fn write_batch(
        &self,
        batch: &StoreBatch,
    ) -> impl Future<Output = Result<(), LocalStoreError>> + Send {
        async move {
            let record = persisted_batch(batch);
            let written: Result<Option<PersistedBatch>, surrealdb::Error> = self
                .database
                .upsert((TABLE_BATCH, batch.id()))
                .content(record)
                .await;
            written
                .map_err(|_| LocalStoreError::new(LocalStoreDiagnosticCode::OperationFailed))?;
            Ok(())
        }
    }

    fn batch_entries(
        &self,
        batch_id: &str,
    ) -> impl Future<Output = Result<Vec<StoreBatchEntry>, LocalStoreError>> + Send {
        async move {
            if !batch_id_is_valid(batch_id) {
                return Err(LocalStoreError::new(LocalStoreDiagnosticCode::BatchInvalid));
            }
            let stored: Option<PersistedBatch> = self
                .database
                .select((TABLE_BATCH, batch_id))
                .await
                .map_err(|_| LocalStoreError::new(LocalStoreDiagnosticCode::OperationFailed))?;
            Ok(stored
                .map(|record| {
                    record
                        .entries
                        .into_iter()
                        .map(|entry| StoreBatchEntry {
                            ordinal: entry.ordinal,
                            payload: entry.payload,
                        })
                        .collect()
                })
                .unwrap_or_default())
        }
    }

    fn close(self) -> impl Future<Output = Result<(), LocalStoreError>> + Send {
        async move {
            // Durability::Every has already synced every accepted transaction;
            // releasing the handle lets the engine stop its background workers.
            drop(self.database);
            Ok(())
        }
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
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("studio-localstore")
            .enable_all()
            .build()
            .map_err(|_| LocalStoreError::new(LocalStoreDiagnosticCode::ExecutorUnavailable))?;
        Ok(Self { runtime })
    }

    /// Submit a `Send` storage operation without blocking the caller thread.
    ///
    /// The returned task must be awaited before clean shutdown. Do not capture
    /// GPUI state in `operation`; capture only owned request values and send its
    /// result back to the UI through the host's normal UI executor.
    pub fn spawn<T, Operation>(&self, operation: Operation) -> StoreTask<T>
    where
        T: Send + 'static,
        Operation: Future<Output = Result<T, LocalStoreError>> + Send + 'static,
    {
        let (sender, receiver) = oneshot::channel();
        self.runtime.spawn(async move {
            let result = operation.await;
            // Dropping the sender instead of sending marks the task cancelled
            // for the receiver when this runtime is shutting down.
            drop(sender.send(result));
        });
        StoreTask { receiver }
    }

    /// Stop workers after every [`StoreTask`] has resolved and store has closed.
    ///
    /// Dropping this executor before those steps cancels unfinished work, so a
    /// host's shutdown sequence is: stop new work, await tasks, close stores,
    /// then call `shutdown`.
    pub fn shutdown(self) {
        self.runtime.shutdown_timeout(Duration::from_secs(10));
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
        match self.receiver.await {
            Ok(result) => result.map_err(StoreTaskError::Store),
            Err(_) => Err(StoreTaskError::Cancelled),
        }
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

/// Test-support hook for the deterministic forced-termination harness.
///
/// The worker binary enters an open client transaction, writes the given batch
/// without committing, signals `marker`, and parks until it is force-terminated.
/// Recovery must then expose no trace of the interrupted batch. Compiled only
/// with debug assertions; never part of the release surface.
#[cfg(debug_assertions)]
impl EmbeddedLocalStore {
    /// Parks forever inside an uncommitted transaction after creating `marker`.
    #[doc(hidden)]
    pub async fn test_pause_inside_uncommitted_transaction(
        &self,
        batch: &StoreBatch,
        marker: &Path,
    ) {
        // UNVERIFIED(runtime): killing the process while this client-side
        // transaction is open must leave no visible record; the kill-recovery
        // harness asserts exactly that.
        let transaction = match self.database.clone().begin().await {
            Ok(transaction) => transaction,
            Err(_) => {
                let _ = std::fs::write(marker, b"begin-failed");
                return;
            }
        };
        let record = persisted_batch(batch);
        let written: Result<Option<PersistedBatch>, surrealdb::Error> = transaction
            .upsert((TABLE_BATCH, batch.id()))
            .content(record)
            .await;
        if written.is_err() {
            let _ = std::fs::write(marker, b"write-failed");
            return;
        }
        let _ = std::fs::write(marker, b"paused");
        loop {
            tokio_time::sleep(Duration::from_millis(50)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(missing_docs)]

    use super::*;

    async fn memory_database() -> Surreal<Db> {
        // UNVERIFIED(compile): `Surreal::new::<Mem>(())` relies on dev-only
        // feature unification of kv-mem; confirmed against the vendored 3.2.4
        // endpoint impls but never compiled in this workspace.
        Surreal::new::<surrealdb::engine::local::Mem>(())
            .await
            .expect("in-memory engine starts")
    }

    fn sample_entry(ordinal: u32) -> StoreBatchEntry {
        StoreBatchEntry {
            ordinal,
            payload: serde_json::json!({ "ordinal": ordinal }),
        }
    }

    #[test]
    fn durability_wire_values_cover_every_mode() {
        assert_eq!(Durability::Every.sync_value(), "every");
        assert_eq!(Durability::Never.sync_value(), "never");
        assert_eq!(
            Durability::Interval(Duration::from_millis(250)).sync_value(),
            "250ms"
        );
    }

    #[test]
    fn durability_rejects_engine_unsupported_intervals() {
        assert!(Durability::Every.is_valid());
        assert!(Durability::Never.is_valid());
        assert!(Durability::Interval(Duration::from_millis(101)).is_valid());
        assert!(!Durability::Interval(Duration::from_millis(100)).is_valid());
        assert!(!Durability::Interval(Duration::from_millis(1)).is_valid());
    }

    #[test]
    fn diagnostics_carry_stable_safe_messages() {
        for code in [
            LocalStoreDiagnosticCode::DirectoryInvalid,
            LocalStoreDiagnosticCode::EngineIncompatible,
            LocalStoreDiagnosticCode::SchemaMetadataCorrupt,
            LocalStoreDiagnosticCode::OperationFailed,
        ] {
            let diagnostic = LocalStoreDiagnostic::new(code);
            assert!(!diagnostic.message().is_empty());
            assert_eq!(diagnostic.code(), code);
        }
    }

    #[test]
    fn batches_reject_empty_ids_and_ordinal_gaps() {
        assert!(StoreBatch::new("", [sample_entry(0)]).is_err());
        assert!(StoreBatch::new("valid", std::iter::empty::<StoreBatchEntry>()).is_err());
        assert!(StoreBatch::new("valid", [sample_entry(1)]).is_err());
        assert!(StoreBatch::new("valid", [sample_entry(0), sample_entry(2)]).is_err());
        let valid = StoreBatch::new("valid", [sample_entry(0), sample_entry(1)]);
        assert_eq!(valid.expect("batch is valid").entries().len(), 2);
    }

    #[tokio::test]
    async fn schema_metadata_initializes_and_revalidates_on_the_memory_engine() {
        let database = memory_database().await;
        select_session(&database).await.expect("session selects");
        assert!(
            read_schema_metadata(&database).await.unwrap().is_none(),
            "fresh engine has no schema metadata yet"
        );

        ensure_schema_metadata(&database).await.expect("initializes metadata");
        let stored = read_schema_metadata(&database).await.unwrap();
        let found = stored.as_ref().expect("metadata record exists");
        validate_schema_metadata(found).expect("fresh metadata is supported");

        let newer = StoredMetadata {
            schema_version: STORE_SCHEMA_VERSION + 1,
            engine_format_version: ENGINE_FORMAT_VERSION,
        };
        assert!(validate_schema_metadata(&newer).is_err());
    }

    #[tokio::test]
    async fn manifest_round_trips_and_rejects_unknown_engines() {
        let directory = tempfile::tempdir().expect("temporary directory");
        assert!(read_manifest(directory.path()).unwrap().is_none());
        write_manifest(directory.path()).expect("manifest writes");

        let manifest = read_manifest(directory.path()).unwrap().expect("manifest");
        assert_eq!(manifest.engine, ENGINE_MANIFEST_ROCKSDB);
        assert_eq!(manifest.format_version, ENGINE_FORMAT_VERSION);
        validate_manifest(&manifest).expect("manifest validates");

        let mut foreign = manifest.clone();
        foreign.engine = "mem".to_owned();
        assert!(validate_manifest(&foreign).is_err());
        let mut future = manifest.clone();
        future.format_version += 1;
        assert!(validate_manifest(&future).is_err());
    }

    #[tokio::test]
    async fn typed_batches_persist_and_read_back_atomically_on_the_memory_engine() {
        let database = memory_database().await;
        select_session(&database).await.expect("session selects");
        ensure_schema_metadata(&database).await.expect("schema initializes");

        let batch =
            StoreBatch::new("round-trip", [sample_entry(0), sample_entry(1), sample_entry(2)])
                .expect("batch is valid");
        let record = persisted_batch(&batch);
        let written: Result<Option<PersistedBatch>, surrealdb::Error> = database
            .upsert((TABLE_BATCH, batch.id()))
            .content(record)
            .await;
        assert!(written.is_ok(), "batch record upserts");

        let stored: Option<PersistedBatch> =
            database.select((TABLE_BATCH, "round-trip")).await.expect("reads back");
        let stored = stored.expect("record exists");
        assert_eq!(stored.batch_id, "round-trip");
        assert_eq!(stored.entries.len(), 3);
        let missing: Option<PersistedBatch> =
            database.select((TABLE_BATCH, "absent")).await.expect("missing reads");
        assert!(missing.is_none());

        assert!(batch_id_is_valid("ordinary-batch"));
        assert!(!batch_id_is_valid(""));
        assert!(!batch_id_is_valid("bad\u{7}id"));
    }
}
