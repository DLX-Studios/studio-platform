//! Conflict and recovery-center domain seams.
//!
//! This module deliberately does not implement cloud synchronization.  A sync
//! provider may submit two complete, actor-attributed intents through
//! [`ConflictPersistence`] and the host can render the resulting center before
//! opening a project.  Recovery records are local logical data: a validated
//! snapshot and an ordered operation journal are enough to rebuild a session
//! without depending on an engine-specific backup format.

#![allow(missing_docs)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]
#![allow(clippy::match_same_arms)]

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Actor, CommandBatch, DefaultDesignerSession, DesignerPersistence, PersistenceError, ProjectId,
    RevisionId, SessionError, StudioDesignSnapshot,
};

/// Current schema for conflict and recovery center records.
pub const RESILIENCE_SCHEMA_VERSION: u16 = 1;

const MAX_REASON_LENGTH: usize = 512;

/// One complete authoring intent retained when two revisions conflict.
///
/// The original command batch is retained rather than reduced to a display
/// summary.  This means a later manual resolver can inspect the exact values,
/// preconditions, actor, and operation identity from both sides.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConflictIntent {
    /// The operation identity submitted by the author.
    pub operation_id: crate::OperationId,
    /// The actor that authored the operation.
    pub actor: Actor,
    /// Revision against which the intent was authored.
    pub base_revision: RevisionId,
    /// The command batch, preserved byte-for-byte at the domain level.
    pub batch: CommandBatch,
    /// Optional local/cloud revision assigned before the conflict was detected.
    pub resulting_revision: Option<RevisionId>,
}

impl ConflictIntent {
    /// Construct an intent from its original command batch.
    pub fn new(
        batch: CommandBatch,
        resulting_revision: Option<RevisionId>,
    ) -> Result<Self, ResilienceError> {
        if batch.actor.display_name.trim().is_empty()
            || batch.actor.display_name.len() > 256
            || batch.actor.display_name.chars().any(char::is_control)
        {
            return Err(ResilienceError::InvalidRecord("conflict intent metadata"));
        }
        Ok(Self {
            operation_id: batch.operation_id.clone(),
            actor: batch.actor.clone(),
            base_revision: batch.base_revision,
            batch,
            resulting_revision,
        })
    }

    /// Number of commands in this intent for a compact center row.
    #[must_use]
    pub fn command_count(&self) -> usize {
        self.batch.commands.len()
    }
}

/// Lifecycle of a conflict record.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictStatus {
    /// Neither intent has been selected yet.
    Pending,
    /// A user-selected resolution plan has been retained.
    Resolved,
}

/// Explicit resolution choices exposed by the conflict center.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionChoice {
    /// Continue with the local intent while retaining the remote intent in the record.
    KeepLocal,
    /// Continue with the remote intent while retaining the local intent in the record.
    KeepRemote,
    /// Preserve both intents as branches for a subsequent merge/duplicate action.
    KeepBoth,
}

/// A durable conflict record containing both competing intents.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConflictRecord {
    /// Record schema version.
    pub schema_version: u16,
    /// Deterministic conflict identity.
    pub conflict_id: String,
    /// Project to which both intents belong.
    pub project_id: ProjectId,
    /// Host-provided logical detection time.
    pub detected_at: u64,
    /// Local/device intent.
    pub local: ConflictIntent,
    /// Remote/cloud intent.
    pub remote: ConflictIntent,
    /// Current center lifecycle state.
    pub status: ConflictStatus,
    /// Chosen resolution, if the record has been handled.
    pub resolution: Option<ResolutionChoice>,
}

impl ConflictRecord {
    /// Construct a pending conflict with a stable identity derived from both operations.
    pub fn new(
        project_id: ProjectId,
        detected_at: u64,
        local: ConflictIntent,
        remote: ConflictIntent,
    ) -> Result<Self, ResilienceError> {
        if local.batch.project_id != project_id || remote.batch.project_id != project_id {
            return Err(ResilienceError::ProjectMismatch);
        }
        let conflict_id = Self::deterministic_id(&project_id, &local, &remote);
        Ok(Self {
            schema_version: RESILIENCE_SCHEMA_VERSION,
            conflict_id,
            project_id,
            detected_at,
            local,
            remote,
            status: ConflictStatus::Pending,
            resolution: None,
        })
    }

    /// Produce a stable ID independent of map or network ordering.
    #[must_use]
    pub fn deterministic_id(
        project_id: &ProjectId,
        local: &ConflictIntent,
        remote: &ConflictIntent,
    ) -> String {
        format!(
            "{}:{}:{}",
            project_id.as_str(),
            local.operation_id.as_str(),
            remote.operation_id.as_str()
        )
    }

    /// Resolve the record without dropping either original intent.
    pub fn resolve(&mut self, choice: ResolutionChoice) -> Result<ResolutionPlan, ResilienceError> {
        if self.status == ConflictStatus::Resolved {
            return Err(ResilienceError::AlreadyResolved);
        }
        self.status = ConflictStatus::Resolved;
        self.resolution = Some(choice);
        Ok(ResolutionPlan {
            conflict_id: self.conflict_id.clone(),
            choice,
            retained: match choice {
                ResolutionChoice::KeepLocal => vec![self.local.clone(), self.remote.clone()],
                ResolutionChoice::KeepRemote => vec![self.remote.clone(), self.local.clone()],
                ResolutionChoice::KeepBoth => vec![self.local.clone(), self.remote.clone()],
            },
        })
    }

    /// Return whether the record still needs operator action.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self.status, ConflictStatus::Pending)
    }
}

/// The result of resolving a conflict; both intents remain available for audit or merge.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionPlan {
    /// Conflict record being resolved.
    pub conflict_id: String,
    /// Choice selected by the operator.
    pub choice: ResolutionChoice,
    /// Intents retained by the plan, in deterministic precedence order.
    pub retained: Vec<ConflictIntent>,
}

/// Async persistence contract for conflicts. Cloud sync may implement a producer separately;
/// this interface only stores validated records and never knows a transport or credential.
pub trait ConflictPersistence: Send + Sync {
    /// Load all conflict records for one project.
    fn load_conflicts<'a>(
        &'a self,
        project_id: &'a ProjectId,
    ) -> crate::SessionFuture<'a, Result<Vec<ConflictRecord>, PersistenceError>>;
    /// Replace all conflict records for one project atomically.
    fn save_conflicts<'a>(
        &'a self,
        project_id: &'a ProjectId,
        records: &'a [ConflictRecord],
    ) -> crate::SessionFuture<'a, Result<(), PersistenceError>>;
}

/// Deterministic conflict persistence for native previews and pure tests.
#[derive(Clone, Default)]
pub struct InMemoryConflictPersistence {
    records: Arc<Mutex<BTreeMap<ProjectId, Vec<ConflictRecord>>>>,
}

impl InMemoryConflictPersistence {
    /// Inspect records saved for one project.
    #[must_use]
    pub fn records(&self, project_id: &ProjectId) -> Vec<ConflictRecord> {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(project_id)
            .cloned()
            .unwrap_or_default()
    }
}

impl ConflictPersistence for InMemoryConflictPersistence {
    fn load_conflicts<'a>(
        &'a self,
        project_id: &'a ProjectId,
    ) -> crate::SessionFuture<'a, Result<Vec<ConflictRecord>, PersistenceError>> {
        Box::pin(async move { Ok(self.records(project_id)) })
    }

    fn save_conflicts<'a>(
        &'a self,
        project_id: &'a ProjectId,
        records: &'a [ConflictRecord],
    ) -> crate::SessionFuture<'a, Result<(), PersistenceError>> {
        Box::pin(async move {
            self.records
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(project_id.clone(), records.to_vec());
            Ok(())
        })
    }
}

/// Conflict-center state loaded before a project editor is opened.
pub struct ConflictCenter<P> {
    project_id: ProjectId,
    persistence: P,
    records: Vec<ConflictRecord>,
}

impl<P: ConflictPersistence> ConflictCenter<P> {
    /// Load pending and resolved records for a project.
    pub async fn open(persistence: P, project_id: ProjectId) -> Result<Self, ResilienceError> {
        let mut records = persistence.load_conflicts(&project_id).await?;
        records.sort_by(|left, right| left.conflict_id.cmp(&right.conflict_id));
        validate_conflicts(&project_id, &records)?;
        Ok(Self {
            project_id,
            persistence,
            records,
        })
    }

    /// Borrow all records in deterministic conflict-ID order.
    #[must_use]
    pub fn records(&self) -> &[ConflictRecord] {
        &self.records
    }

    /// Borrow only records requiring action.
    #[must_use]
    pub fn pending(&self) -> Vec<&ConflictRecord> {
        self.records
            .iter()
            .filter(|record| record.is_pending())
            .collect()
    }

    /// Add a conflict while preserving both source intents.
    pub async fn record(&mut self, record: ConflictRecord) -> Result<(), ResilienceError> {
        validate_conflicts(&self.project_id, std::slice::from_ref(&record))?;
        if self
            .records
            .iter()
            .any(|current| current.conflict_id == record.conflict_id)
        {
            return Err(ResilienceError::DuplicateConflict);
        }
        let previous = self.records.clone();
        self.records.push(record);
        self.records
            .sort_by(|left, right| left.conflict_id.cmp(&right.conflict_id));
        if let Err(error) = self.persist().await {
            self.records = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Resolve one record and persist the updated center atomically.
    pub async fn resolve(
        &mut self,
        conflict_id: &str,
        choice: ResolutionChoice,
    ) -> Result<ResolutionPlan, ResilienceError> {
        let previous = self.records.clone();
        let plan = {
            let record = self
                .records
                .iter_mut()
                .find(|record| record.conflict_id == conflict_id)
                .ok_or_else(|| ResilienceError::ConflictNotFound(conflict_id.to_owned()))?;
            record.resolve(choice)?
        };
        if let Err(error) = self.persist().await {
            self.records = previous;
            return Err(error);
        }
        Ok(plan)
    }

    async fn persist(&self) -> Result<(), ResilienceError> {
        self.persistence
            .save_conflicts(&self.project_id, &self.records)
            .await
            .map_err(ResilienceError::from)
    }
}

/// A logical, engine-independent snapshot used as the base of a recovery bundle.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalSnapshot {
    /// Recovery schema version.
    pub schema_version: u16,
    /// Project represented by the snapshot.
    pub project_id: ProjectId,
    /// Complete validated design state at the snapshot revision.
    pub revision: StudioDesignSnapshot,
    /// Number of journal entries already included in this snapshot.
    pub journal_cursor: u64,
    /// Host-provided logical capture time.
    pub captured_at: u64,
}

impl LogicalSnapshot {
    /// Construct a logical snapshot from an immutable Designer snapshot.
    pub fn new(
        project_id: ProjectId,
        revision: StudioDesignSnapshot,
        journal_cursor: u64,
        captured_at: u64,
    ) -> Result<Self, ResilienceError> {
        if revision.design.project_id != project_id {
            return Err(ResilienceError::ProjectMismatch);
        }
        Ok(Self {
            schema_version: RESILIENCE_SCHEMA_VERSION,
            project_id,
            revision,
            journal_cursor,
            captured_at,
        })
    }
}

/// One accepted operation after a logical snapshot.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct JournalEntry {
    /// Monotonic operation-log position.
    pub sequence: u64,
    /// Project owning the operation.
    pub project_id: ProjectId,
    /// Original command batch to replay.
    pub batch: CommandBatch,
    /// Revision expected after replaying this entry.
    pub committed_revision: RevisionId,
}

impl JournalEntry {
    /// Construct an operation-journal entry after checking ownership metadata.
    pub fn new(
        sequence: u64,
        project_id: ProjectId,
        batch: CommandBatch,
        committed_revision: RevisionId,
    ) -> Result<Self, ResilienceError> {
        if sequence == 0 || batch.project_id != project_id {
            return Err(ResilienceError::InvalidRecord("journal entry metadata"));
        }
        Ok(Self {
            sequence,
            project_id,
            batch,
            committed_revision,
        })
    }
}

/// Snapshot-plus-journal input used to rebuild a working Designer session.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryBundle {
    /// Base logical snapshot.
    pub snapshot: LogicalSnapshot,
    /// Ordered operations not included in the snapshot.
    pub journal: Vec<JournalEntry>,
}

impl RecoveryBundle {
    /// Validate ordering, ownership, and the snapshot/journal cursor boundary.
    pub fn validate(&self) -> Result<(), ResilienceError> {
        if self.snapshot.schema_version != RESILIENCE_SCHEMA_VERSION
            || self.snapshot.revision.design.project_id != self.snapshot.project_id
        {
            return Err(ResilienceError::InvalidRecord("logical snapshot metadata"));
        }
        let mut expected_sequence = self.snapshot.journal_cursor.saturating_add(1);
        let mut expected_revision = self.snapshot.revision.revision.id;
        for entry in &self.journal {
            if entry.project_id != self.snapshot.project_id
                || entry.batch.project_id != self.snapshot.project_id
                || entry.sequence != expected_sequence
                || entry.batch.base_revision != expected_revision
            {
                return Err(ResilienceError::JournalGap);
            }
            expected_sequence = expected_sequence.saturating_add(1);
            expected_revision = entry.committed_revision;
        }
        Ok(())
    }

    /// Restore a working [`DefaultDesignerSession`] by replaying every journal entry.
    pub async fn restore<P: DesignerPersistence>(
        &self,
        persistence: P,
    ) -> Result<DefaultDesignerSession<P>, ResilienceError> {
        self.validate()?;
        DefaultDesignerSession::restore_from_recovery(persistence, self)
            .await
            .map_err(ResilienceError::from)
    }
}

/// Recovery center lifecycle, including interrupted migrations and quarantine.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecoveryState {
    /// The project can be restored from its local logical bundle.
    Recoverable,
    /// An upgrade stopped before its migration completed.
    InterruptedUpgrade { from_schema: u16, to_schema: u16 },
    /// A migration is currently staged and must not be opened as a project.
    Migrating { from_schema: u16, to_schema: u16 },
    /// The last restore attempt failed but source data remains available.
    RestoreFailed,
    /// The project is isolated from normal opening pending operator action.
    Quarantined,
    /// Recovery completed and the project may be opened.
    Restored,
}

/// Durable recovery/quarantine record for one project.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryRecord {
    /// Recovery schema version.
    pub schema_version: u16,
    /// Stable recovery record identity.
    pub recovery_id: String,
    /// Project represented by this record.
    pub project_id: ProjectId,
    /// Current recovery lifecycle state.
    pub state: RecoveryState,
    /// Snapshot and operation journal used for rebuilding.
    pub bundle: RecoveryBundle,
    /// Safe operator-facing reason for a failed/quarantined state.
    pub reason: Option<String>,
}

impl RecoveryRecord {
    /// Construct a recoverable record from a validated bundle.
    pub fn new(
        recovery_id: impl Into<String>,
        bundle: RecoveryBundle,
    ) -> Result<Self, ResilienceError> {
        let recovery_id = recovery_id.into();
        if recovery_id.is_empty()
            || recovery_id.len() > 128
            || recovery_id.chars().any(char::is_control)
        {
            return Err(ResilienceError::InvalidRecord("recovery identity"));
        }
        bundle.validate()?;
        Ok(Self {
            schema_version: RESILIENCE_SCHEMA_VERSION,
            project_id: bundle.snapshot.project_id.clone(),
            state: RecoveryState::Recoverable,
            bundle,
            reason: None,
            recovery_id,
        })
    }

    /// Mark an interrupted upgrade as requiring migration/recovery action.
    pub fn interrupted_upgrade(&mut self, from_schema: u16, to_schema: u16) {
        self.state = RecoveryState::InterruptedUpgrade {
            from_schema,
            to_schema,
        };
    }

    /// Mark an interrupted upgrade as actively migrating.
    pub fn begin_migration(&mut self) -> Result<(), ResilienceError> {
        let &RecoveryState::InterruptedUpgrade {
            from_schema,
            to_schema,
        } = &self.state
        else {
            return Err(ResilienceError::InvalidRecoveryTransition);
        };
        self.state = RecoveryState::Migrating {
            from_schema,
            to_schema,
        };
        Ok(())
    }

    /// Mark a successfully migrated/restored record safe to open.
    pub fn mark_restored(&mut self) -> Result<(), ResilienceError> {
        if !matches!(
            self.state,
            RecoveryState::Recoverable | RecoveryState::Migrating { .. }
        ) {
            return Err(ResilienceError::InvalidRecoveryTransition);
        }
        self.state = RecoveryState::Restored;
        self.reason = None;
        Ok(())
    }

    /// Move a record into quarantine with a bounded safe reason.
    pub fn quarantine(&mut self, reason: impl Into<String>) -> Result<(), ResilienceError> {
        let reason = reason.into();
        validate_reason(&reason)?;
        self.state = RecoveryState::Quarantined;
        self.reason = Some(reason);
        Ok(())
    }

    /// Record a failed restore while retaining the original bundle.
    pub fn restore_failed(&mut self, reason: impl Into<String>) -> Result<(), ResilienceError> {
        let reason = reason.into();
        validate_reason(&reason)?;
        self.state = RecoveryState::RestoreFailed;
        self.reason = Some(reason);
        Ok(())
    }

    /// Return whether opening the project is safe without another recovery action.
    #[must_use]
    pub const fn can_open(&self) -> bool {
        matches!(self.state, RecoveryState::Restored)
    }
}

/// Async persistence contract for recovery bundles and quarantine state.
pub trait RecoveryPersistence: Send + Sync {
    /// Load all recovery records for a project.
    fn load_recovery<'a>(
        &'a self,
        project_id: &'a ProjectId,
    ) -> crate::SessionFuture<'a, Result<Vec<RecoveryRecord>, PersistenceError>>;
    /// Replace all recovery records for one project atomically.
    fn save_recovery<'a>(
        &'a self,
        project_id: &'a ProjectId,
        records: &'a [RecoveryRecord],
    ) -> crate::SessionFuture<'a, Result<(), PersistenceError>>;
}

/// Deterministic recovery persistence for native previews and pure tests.
#[derive(Clone, Default)]
pub struct InMemoryRecoveryPersistence {
    records: Arc<Mutex<BTreeMap<ProjectId, Vec<RecoveryRecord>>>>,
}

impl InMemoryRecoveryPersistence {
    /// Inspect records saved for one project.
    #[must_use]
    pub fn records(&self, project_id: &ProjectId) -> Vec<RecoveryRecord> {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(project_id)
            .cloned()
            .unwrap_or_default()
    }
}

impl RecoveryPersistence for InMemoryRecoveryPersistence {
    fn load_recovery<'a>(
        &'a self,
        project_id: &'a ProjectId,
    ) -> crate::SessionFuture<'a, Result<Vec<RecoveryRecord>, PersistenceError>> {
        Box::pin(async move { Ok(self.records(project_id)) })
    }

    fn save_recovery<'a>(
        &'a self,
        project_id: &'a ProjectId,
        records: &'a [RecoveryRecord],
    ) -> crate::SessionFuture<'a, Result<(), PersistenceError>> {
        Box::pin(async move {
            self.records
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(project_id.clone(), records.to_vec());
            Ok(())
        })
    }
}

/// Recovery-center state that can be shown before opening the project editor.
pub struct RecoveryCenter<P> {
    project_id: ProjectId,
    persistence: P,
    records: Vec<RecoveryRecord>,
}

impl<P: RecoveryPersistence> RecoveryCenter<P> {
    /// Load local recovery/quarantine records for a project.
    pub async fn open(persistence: P, project_id: ProjectId) -> Result<Self, ResilienceError> {
        let mut records = persistence.load_recovery(&project_id).await?;
        records.sort_by(|left, right| left.recovery_id.cmp(&right.recovery_id));
        validate_recovery(&project_id, &records)?;
        Ok(Self {
            project_id,
            persistence,
            records,
        })
    }

    /// Borrow records in stable recovery-ID order.
    #[must_use]
    pub fn records(&self) -> &[RecoveryRecord] {
        &self.records
    }

    /// Add a recovery record and persist it atomically.
    pub async fn record(&mut self, record: RecoveryRecord) -> Result<(), ResilienceError> {
        validate_recovery(&self.project_id, std::slice::from_ref(&record))?;
        if self
            .records
            .iter()
            .any(|current| current.recovery_id == record.recovery_id)
        {
            return Err(ResilienceError::DuplicateRecovery);
        }
        let previous = self.records.clone();
        self.records.push(record);
        self.records
            .sort_by(|left, right| left.recovery_id.cmp(&right.recovery_id));
        if let Err(error) = self.persist().await {
            self.records = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Quarantine a record without deleting its source snapshot or journal.
    pub async fn quarantine(
        &mut self,
        recovery_id: &str,
        reason: impl Into<String>,
    ) -> Result<(), ResilienceError> {
        let previous = self.records.clone();
        {
            let record = self
                .records
                .iter_mut()
                .find(|record| record.recovery_id == recovery_id)
                .ok_or_else(|| ResilienceError::RecoveryNotFound(recovery_id.to_owned()))?;
            record.quarantine(reason)?;
        }
        if let Err(error) = self.persist().await {
            self.records = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Move an interrupted-upgrade record into the migration state.
    pub async fn mark_interrupted_upgrade(
        &mut self,
        recovery_id: &str,
        from_schema: u16,
        to_schema: u16,
    ) -> Result<(), ResilienceError> {
        let previous = self.records.clone();
        {
            let record = self
                .records
                .iter_mut()
                .find(|record| record.recovery_id == recovery_id)
                .ok_or_else(|| ResilienceError::RecoveryNotFound(recovery_id.to_owned()))?;
            record.interrupted_upgrade(from_schema, to_schema);
        }
        if let Err(error) = self.persist().await {
            self.records = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Move an interrupted-upgrade record into the migration state.
    pub async fn begin_migration(&mut self, recovery_id: &str) -> Result<(), ResilienceError> {
        let previous = self.records.clone();
        {
            let record = self
                .records
                .iter_mut()
                .find(|record| record.recovery_id == recovery_id)
                .ok_or_else(|| ResilienceError::RecoveryNotFound(recovery_id.to_owned()))?;
            record.begin_migration()?;
        }
        if let Err(error) = self.persist().await {
            self.records = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Mark a completed migration safe to open without replaying the bundle.
    pub async fn mark_restored(&mut self, recovery_id: &str) -> Result<(), ResilienceError> {
        let previous = self.records.clone();
        {
            let record = self
                .records
                .iter_mut()
                .find(|record| record.recovery_id == recovery_id)
                .ok_or_else(|| ResilienceError::RecoveryNotFound(recovery_id.to_owned()))?;
            record.mark_restored()?;
        }
        if let Err(error) = self.persist().await {
            self.records = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Restore one recoverable bundle and mark it restored only after replay succeeds.
    pub async fn restore(
        &mut self,
        recovery_id: &str,
        persistence: impl DesignerPersistence,
    ) -> Result<DefaultDesignerSession<impl DesignerPersistence>, ResilienceError> {
        let previous = self.records.clone();
        let bundle = {
            let record = self
                .records
                .iter()
                .find(|record| record.recovery_id == recovery_id)
                .ok_or_else(|| ResilienceError::RecoveryNotFound(recovery_id.to_owned()))?;
            if !matches!(
                record.state,
                RecoveryState::Recoverable | RecoveryState::RestoreFailed
            ) {
                return Err(ResilienceError::NotRecoverable);
            }
            record.bundle.clone()
        };
        let session = match bundle.restore(persistence).await {
            Ok(session) => session,
            Err(error) => {
                {
                    let record = self
                        .records
                        .iter_mut()
                        .find(|record| record.recovery_id == recovery_id)
                        .expect("recovery record was loaded above");
                    record.restore_failed(error.to_string())?;
                }
                self.persist().await?;
                return Err(error);
            }
        };
        {
            let record = self
                .records
                .iter_mut()
                .find(|record| record.recovery_id == recovery_id)
                .expect("recovery record was loaded above");
            record.state = RecoveryState::Restored;
            record.reason = None;
        }
        if let Err(error) = self.persist().await {
            self.records = previous;
            return Err(error);
        }
        Ok(session)
    }

    async fn persist(&self) -> Result<(), ResilienceError> {
        self.persistence
            .save_recovery(&self.project_id, &self.records)
            .await
            .map_err(ResilienceError::from)
    }
}

/// Stable, sanitized center failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ResilienceError {
    /// The record contains invalid metadata or an unsafe reason.
    #[error("invalid resilience record: {0}")]
    InvalidRecord(&'static str),
    /// Both intents or a recovery bundle target another project.
    #[error("resilience record project mismatch")]
    ProjectMismatch,
    /// A journal sequence or revision chain is incomplete.
    #[error("recovery journal has a gap or stale base revision")]
    JournalGap,
    /// An identical conflict was already recorded.
    #[error("conflict already exists")]
    DuplicateConflict,
    /// An identical recovery record was already recorded.
    #[error("recovery record already exists")]
    DuplicateRecovery,
    /// A conflict ID was not loaded in this center.
    #[error("conflict not found: {0}")]
    ConflictNotFound(String),
    /// A recovery ID was not loaded in this center.
    #[error("recovery record not found: {0}")]
    RecoveryNotFound(String),
    /// A resolved conflict cannot be resolved twice.
    #[error("conflict is already resolved")]
    AlreadyResolved,
    /// Only recoverable or previously failed records may be restored.
    #[error("recovery record is not recoverable")]
    NotRecoverable,
    /// A migration or restore transition was requested from an invalid state.
    #[error("invalid recovery state transition")]
    InvalidRecoveryTransition,
    /// Underlying Designer persistence rejected the rebuild.
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    /// The rebuilt Designer session rejected an invalid state or operation.
    #[error(transparent)]
    Session(#[from] SessionError),
}

fn validate_conflicts(
    project_id: &ProjectId,
    records: &[ConflictRecord],
) -> Result<(), ResilienceError> {
    let mut previous = None;
    for record in records {
        if record.schema_version != RESILIENCE_SCHEMA_VERSION
            || record.project_id != *project_id
            || record.local.batch.project_id != *project_id
            || record.remote.batch.project_id != *project_id
            || record.local.operation_id != record.local.batch.operation_id
            || record.remote.operation_id != record.remote.batch.operation_id
            || record.local.base_revision != record.local.batch.base_revision
            || record.remote.base_revision != record.remote.batch.base_revision
            || record.conflict_id
                != ConflictRecord::deterministic_id(
                    &record.project_id,
                    &record.local,
                    &record.remote,
                )
            || (record.status == ConflictStatus::Pending && record.resolution.is_some())
            || (record.status == ConflictStatus::Resolved && record.resolution.is_none())
            || previous.is_some_and(|id: &str| id >= record.conflict_id.as_str())
        {
            return Err(ResilienceError::InvalidRecord("conflict record metadata"));
        }
        previous = Some(record.conflict_id.as_str());
    }
    Ok(())
}

fn validate_recovery(
    project_id: &ProjectId,
    records: &[RecoveryRecord],
) -> Result<(), ResilienceError> {
    let mut previous = None;
    for record in records {
        if record.schema_version != RESILIENCE_SCHEMA_VERSION
            || record.project_id != *project_id
            || record.bundle.snapshot.project_id != *project_id
            || previous.is_some_and(|id: &str| id >= record.recovery_id.as_str())
        {
            return Err(ResilienceError::InvalidRecord("recovery record metadata"));
        }
        record.bundle.validate()?;
        previous = Some(record.recovery_id.as_str());
    }
    Ok(())
}

fn validate_reason(reason: &str) -> Result<(), ResilienceError> {
    if reason.is_empty() || reason.len() > MAX_REASON_LENGTH || reason.chars().any(char::is_control)
    {
        Err(ResilienceError::InvalidRecord("recovery reason"))
    } else {
        Ok(())
    }
}
