//! Durable Studio Design transactions stored through the host `LocalStore`.

#![allow(missing_docs)]
#![allow(clippy::all, clippy::pedantic, clippy::restriction, clippy::nursery)]
#![allow(
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::needless_pass_by_value
)]

use std::{fmt::Write as _, sync::Arc};

use serde::{Deserialize, Serialize};
use studio_design::{
    ConflictPersistence, ConflictRecord, DesignerPersistence, DesignerTransaction,
    DurableDesignerState, PersistenceError, PersistenceErrorCode, ProjectId,
    RESILIENCE_SCHEMA_VERSION, RecoveryPersistence, RecoveryRecord, STUDIO_DESIGN_SCHEMA_VERSION,
    SessionFuture, WORKSPACE_STATE_SCHEMA_VERSION, WorkspaceError, WorkspacePersistence,
    WorkspaceRecord,
};

use crate::local_store::{
    Durability, EmbeddedLocalStore, LocalStore, LocalStoreDiagnosticCode, LocalStoreError,
    StoreBatch, StoreBatchEntry,
};

const ADAPTER_SCHEMA_VERSION: u16 = 1;

const CONFLICT_RECORDS_BATCH_PREFIX: &str = "designer-conflicts-";
const RECOVERY_RECORDS_BATCH_PREFIX: &str = "designer-recovery-";

/// `LocalStore`-backed implementation of the domain-owned persistence contract.
///
/// Each project owns one deterministic `LocalStore` record. Replacing that
/// record atomically publishes the complete latest snapshot, immutable
/// revisions, accepted batches, receipts, tombstones, and history cursor.
/// `Durability::Every` is enforced so an accepted Designer receipt means the
/// transaction has reached the host's durable point.
#[derive(Clone)]
pub struct LocalStoreDesignerPersistence {
    store: Arc<EmbeddedLocalStore>,
}

impl LocalStoreDesignerPersistence {
    /// Wrap a durable embedded `LocalStore` for Designer use.
    ///
    /// # Errors
    ///
    /// Returns a sanitized rejection when the store is not configured with
    /// [`Durability::Every`].
    pub fn new(store: EmbeddedLocalStore) -> Result<Self, PersistenceError> {
        if store.durability() != Durability::Every {
            return Err(PersistenceError {
                code: PersistenceErrorCode::Rejected,
                message: "Designer persistence requires durability for every accepted transaction"
                    .to_owned(),
            });
        }
        Ok(Self {
            store: Arc::new(store),
        })
    }

    /// Wrap a shared durable store so Designer persistence and Library asset
    /// metadata can use the same host-owned LocalStore without exposing its
    /// engine to the Designer domain.
    pub fn new_shared(store: Arc<EmbeddedLocalStore>) -> Result<Self, PersistenceError> {
        if store.durability() != Durability::Every {
            return Err(PersistenceError {
                code: PersistenceErrorCode::Rejected,
                message: "Designer persistence requires durability for every accepted transaction"
                    .to_owned(),
            });
        }
        Ok(Self { store })
    }

    /// Recover the owned `LocalStore` after every session and adapter clone is dropped.
    ///
    /// The returned `Err(Self)` retains ownership when another adapter clone
    /// still exists, allowing the host to retry its orderly shutdown sequence.
    pub fn try_into_store(self) -> Result<EmbeddedLocalStore, Self> {
        match Arc::try_unwrap(self.store) {
            Ok(store) => Ok(store),
            Err(store) => Err(Self { store }),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedDesignerEnvelope {
    adapter_schema_version: u16,
    project_id: ProjectId,
    transaction: DesignerTransaction,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedConflictEnvelope {
    adapter_schema_version: u16,
    resilience_schema_version: u16,
    project_id: ProjectId,
    records: Vec<ConflictRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedRecoveryEnvelope {
    adapter_schema_version: u16,
    resilience_schema_version: u16,
    project_id: ProjectId,
    records: Vec<RecoveryRecord>,
}

impl DesignerPersistence for LocalStoreDesignerPersistence {
    fn load<'a>(
        &'a self,
        project_id: &'a ProjectId,
    ) -> SessionFuture<'a, Result<Option<DurableDesignerState>, PersistenceError>> {
        Box::pin(async move {
            let entries = self
                .store
                .batch_entries(&project_batch_id(project_id))
                .await
                .map_err(map_store_error)?;
            if entries.is_empty() {
                return Ok(None);
            }
            let [entry] = entries.as_slice() else {
                return Err(corrupt_record());
            };
            if entry.ordinal != 0 {
                return Err(corrupt_record());
            }
            let envelope: PersistedDesignerEnvelope =
                serde_json::from_value(entry.payload.clone()).map_err(|_| corrupt_record())?;
            if envelope.adapter_schema_version != ADAPTER_SCHEMA_VERSION
                || envelope.project_id != *project_id
                || envelope.transaction.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
                || envelope.transaction.project_id != *project_id
                || envelope.transaction.state.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
                || envelope.transaction.state.current.design.project_id != *project_id
                || envelope.transaction.sequence
                    != envelope.transaction.state.current.revision.id.get()
            {
                return Err(incompatible_record());
            }
            Ok(Some(envelope.transaction.state))
        })
    }

    fn commit<'a>(
        &'a self,
        transaction: &'a DesignerTransaction,
    ) -> SessionFuture<'a, Result<(), PersistenceError>> {
        Box::pin(async move {
            if transaction.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
                || transaction.state.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
                || transaction.project_id != transaction.state.current.design.project_id
                || transaction.sequence != transaction.state.current.revision.id.get()
            {
                return Err(PersistenceError {
                    code: PersistenceErrorCode::Rejected,
                    message: "the Designer transaction metadata is inconsistent".to_owned(),
                });
            }
            let payload = serde_json::to_value(PersistedDesignerEnvelope {
                adapter_schema_version: ADAPTER_SCHEMA_VERSION,
                project_id: transaction.project_id.clone(),
                transaction: transaction.clone(),
            })
            .map_err(|_| PersistenceError {
                code: PersistenceErrorCode::Rejected,
                message: "the Designer transaction could not be encoded".to_owned(),
            })?;
            let batch = StoreBatch::new(
                project_batch_id(&transaction.project_id),
                [StoreBatchEntry {
                    ordinal: 0,
                    payload,
                }],
            )
            .map_err(map_store_error)?;
            self.store
                .write_batch(&batch)
                .await
                .map_err(map_store_error)
        })
    }
}

impl WorkspacePersistence for LocalStoreDesignerPersistence {
    fn load<'a>(
        &'a self,
        project_id: &'a ProjectId,
    ) -> SessionFuture<'a, Result<Option<WorkspaceRecord>, WorkspaceError>> {
        Box::pin(async move {
            let entries = self
                .store
                .batch_entries(&workspace_batch_id(project_id))
                .await
                .map_err(workspace_store_error)?;
            if entries.is_empty() {
                return Ok(None);
            }
            let [entry] = entries.as_slice() else {
                return Err(WorkspaceError::InvalidState(
                    "workspace record contains multiple entries".to_owned(),
                ));
            };
            if entry.ordinal != 0 {
                return Err(WorkspaceError::InvalidState(
                    "workspace record has an invalid entry ordinal".to_owned(),
                ));
            }
            let record: WorkspaceRecord =
                serde_json::from_value(entry.payload.clone()).map_err(|_| {
                    WorkspaceError::InvalidState("workspace record is damaged".to_owned())
                })?;
            validate_workspace_record(&record, project_id)?;
            Ok(Some(record))
        })
    }

    fn save<'a>(
        &'a self,
        record: &'a WorkspaceRecord,
    ) -> SessionFuture<'a, Result<(), WorkspaceError>> {
        Box::pin(async move {
            validate_workspace_record(record, &record.project_id)?;
            let payload = serde_json::to_value(record).map_err(|_| {
                WorkspaceError::Persistence("workspace record could not be encoded".to_owned())
            })?;
            let batch = StoreBatch::new(
                workspace_batch_id(&record.project_id),
                [StoreBatchEntry {
                    ordinal: 0,
                    payload,
                }],
            )
            .map_err(workspace_store_error)?;
            self.store
                .write_batch(&batch)
                .await
                .map_err(workspace_store_error)
        })
    }
}

impl ConflictPersistence for LocalStoreDesignerPersistence {
    fn load_conflicts<'a>(
        &'a self,
        project_id: &'a ProjectId,
    ) -> SessionFuture<'a, Result<Vec<ConflictRecord>, PersistenceError>> {
        Box::pin(async move {
            let entries = self
                .store
                .batch_entries(&resilience_batch_id(
                    CONFLICT_RECORDS_BATCH_PREFIX,
                    project_id,
                ))
                .await
                .map_err(map_store_error)?;
            if entries.is_empty() {
                return Ok(Vec::new());
            }
            let [entry] = entries.as_slice() else {
                return Err(corrupt_record());
            };
            if entry.ordinal != 0 {
                return Err(corrupt_record());
            }
            let envelope: PersistedConflictEnvelope =
                serde_json::from_value(entry.payload.clone()).map_err(|_| corrupt_record())?;
            validate_conflict_envelope(envelope)
        })
    }

    fn save_conflicts<'a>(
        &'a self,
        project_id: &'a ProjectId,
        records: &'a [ConflictRecord],
    ) -> SessionFuture<'a, Result<(), PersistenceError>> {
        Box::pin(async move {
            if records.iter().any(|record| {
                record.schema_version != RESILIENCE_SCHEMA_VERSION
                    || record.project_id != *project_id
            }) {
                return Err(rejected_record(
                    "conflict records contain inconsistent metadata",
                ));
            }
            let payload = serde_json::to_value(PersistedConflictEnvelope {
                adapter_schema_version: ADAPTER_SCHEMA_VERSION,
                resilience_schema_version: RESILIENCE_SCHEMA_VERSION,
                project_id: project_id.clone(),
                records: records.to_vec(),
            })
            .map_err(|_| rejected_record("conflict records could not be encoded"))?;
            let batch = StoreBatch::new(
                resilience_batch_id(CONFLICT_RECORDS_BATCH_PREFIX, project_id),
                [StoreBatchEntry {
                    ordinal: 0,
                    payload,
                }],
            )
            .map_err(map_store_error)?;
            self.store
                .write_batch(&batch)
                .await
                .map_err(map_store_error)
        })
    }
}

impl RecoveryPersistence for LocalStoreDesignerPersistence {
    fn load_recovery<'a>(
        &'a self,
        project_id: &'a ProjectId,
    ) -> SessionFuture<'a, Result<Vec<RecoveryRecord>, PersistenceError>> {
        Box::pin(async move {
            let entries = self
                .store
                .batch_entries(&resilience_batch_id(
                    RECOVERY_RECORDS_BATCH_PREFIX,
                    project_id,
                ))
                .await
                .map_err(map_store_error)?;
            if entries.is_empty() {
                return Ok(Vec::new());
            }
            let [entry] = entries.as_slice() else {
                return Err(corrupt_record());
            };
            if entry.ordinal != 0 {
                return Err(corrupt_record());
            }
            let envelope: PersistedRecoveryEnvelope =
                serde_json::from_value(entry.payload.clone()).map_err(|_| corrupt_record())?;
            validate_recovery_envelope(envelope)
        })
    }

    fn save_recovery<'a>(
        &'a self,
        project_id: &'a ProjectId,
        records: &'a [RecoveryRecord],
    ) -> SessionFuture<'a, Result<(), PersistenceError>> {
        Box::pin(async move {
            if records.iter().any(|record| {
                record.schema_version != RESILIENCE_SCHEMA_VERSION
                    || record.project_id != *project_id
            }) {
                return Err(rejected_record(
                    "recovery records contain inconsistent metadata",
                ));
            }
            let payload = serde_json::to_value(PersistedRecoveryEnvelope {
                adapter_schema_version: ADAPTER_SCHEMA_VERSION,
                resilience_schema_version: RESILIENCE_SCHEMA_VERSION,
                project_id: project_id.clone(),
                records: records.to_vec(),
            })
            .map_err(|_| rejected_record("recovery records could not be encoded"))?;
            let batch = StoreBatch::new(
                resilience_batch_id(RECOVERY_RECORDS_BATCH_PREFIX, project_id),
                [StoreBatchEntry {
                    ordinal: 0,
                    payload,
                }],
            )
            .map_err(map_store_error)?;
            self.store
                .write_batch(&batch)
                .await
                .map_err(map_store_error)
        })
    }
}

fn project_batch_id(project_id: &ProjectId) -> String {
    resilience_batch_id("designer-project-", project_id)
}

fn resilience_batch_id(prefix: &str, project_id: &ProjectId) -> String {
    let mut batch_id = String::with_capacity(prefix.len() + project_id.as_str().len() * 2);
    batch_id.push_str(prefix);
    for byte in project_id.as_str().bytes() {
        write!(&mut batch_id, "{byte:02x}").expect("writing to a String cannot fail");
    }
    batch_id
}

fn workspace_batch_id(project_id: &ProjectId) -> String {
    let mut batch_id = String::with_capacity(20 + project_id.as_str().len() * 2);
    batch_id.push_str("designer-workspace-");
    for byte in project_id.as_str().bytes() {
        write!(&mut batch_id, "{byte:02x}").expect("writing to a String cannot fail");
    }
    batch_id
}

fn validate_workspace_record(
    record: &WorkspaceRecord,
    project_id: &ProjectId,
) -> Result<(), WorkspaceError> {
    if record.schema_version != WORKSPACE_STATE_SCHEMA_VERSION || record.project_id != *project_id {
        return Err(WorkspaceError::InvalidState(
            "workspace record metadata is incompatible".to_owned(),
        ));
    }
    record.state.validate()
}

fn workspace_store_error(error: LocalStoreError) -> WorkspaceError {
    WorkspaceError::Persistence(error.diagnostic().message().to_owned())
}

fn validate_conflict_envelope(
    envelope: PersistedConflictEnvelope,
) -> Result<Vec<ConflictRecord>, PersistenceError> {
    if envelope.adapter_schema_version != ADAPTER_SCHEMA_VERSION
        || envelope.resilience_schema_version != RESILIENCE_SCHEMA_VERSION
        || envelope.records.iter().any(|record| {
            record.schema_version != RESILIENCE_SCHEMA_VERSION
                || record.project_id != envelope.project_id
        })
    {
        return Err(incompatible_record());
    }
    Ok(envelope.records)
}

fn validate_recovery_envelope(
    envelope: PersistedRecoveryEnvelope,
) -> Result<Vec<RecoveryRecord>, PersistenceError> {
    if envelope.adapter_schema_version != ADAPTER_SCHEMA_VERSION
        || envelope.resilience_schema_version != RESILIENCE_SCHEMA_VERSION
        || envelope.records.iter().any(|record| {
            record.schema_version != RESILIENCE_SCHEMA_VERSION
                || record.project_id != envelope.project_id
        })
    {
        return Err(incompatible_record());
    }
    Ok(envelope.records)
}

fn rejected_record(message: &str) -> PersistenceError {
    PersistenceError {
        code: PersistenceErrorCode::Rejected,
        message: message.to_owned(),
    }
}

fn map_store_error(error: LocalStoreError) -> PersistenceError {
    let code = match error.diagnostic().code() {
        LocalStoreDiagnosticCode::EngineManifestCorrupt
        | LocalStoreDiagnosticCode::SchemaMetadataCorrupt => PersistenceErrorCode::Corrupt,
        LocalStoreDiagnosticCode::EngineIncompatible
        | LocalStoreDiagnosticCode::SchemaIncompatible => PersistenceErrorCode::Incompatible,
        LocalStoreDiagnosticCode::BatchInvalid => PersistenceErrorCode::Rejected,
        LocalStoreDiagnosticCode::DirectoryInvalid
        | LocalStoreDiagnosticCode::DurabilityInvalid
        | LocalStoreDiagnosticCode::RecoveryUnavailable
        | LocalStoreDiagnosticCode::EngineOpenFailed
        | LocalStoreDiagnosticCode::OperationFailed
        | LocalStoreDiagnosticCode::ExecutorUnavailable
        | LocalStoreDiagnosticCode::QueryTimedOut => PersistenceErrorCode::Unavailable,
    };
    PersistenceError {
        code,
        message: error.diagnostic().message().to_owned(),
    }
}

fn corrupt_record() -> PersistenceError {
    PersistenceError {
        code: PersistenceErrorCode::Corrupt,
        message: "the durable Designer record is damaged".to_owned(),
    }
}

fn incompatible_record() -> PersistenceError {
    PersistenceError {
        code: PersistenceErrorCode::Incompatible,
        message: "the durable Designer record requires a supported migration".to_owned(),
    }
}
