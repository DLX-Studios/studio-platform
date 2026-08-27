//! Domain-owned persistence contract for durable Designer transactions.
#![allow(
    missing_docs,
    reason = "closed persistence records mirror their documented domain type"
)]

use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    command::HistoryEntry,
    model::{DesignerDiagnostic, ProjectId, StudioDesignSnapshot},
    session::CommandReceipt,
};

/// Boxed future used by object-safe domain interfaces.
pub type SessionFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Complete durable project state needed to reopen a Designer session.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DurableDesignerState {
    pub schema_version: u16,
    pub current: StudioDesignSnapshot,
    pub revisions: Vec<StudioDesignSnapshot>,
    pub receipts: Vec<CommandReceipt>,
    pub history: Vec<HistoryEntry>,
    pub history_cursor: usize,
    pub diagnostics: Vec<DesignerDiagnostic>,
}

/// One atomic durable write selected by the domain command engine.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignerTransaction {
    pub schema_version: u16,
    pub project_id: ProjectId,
    pub sequence: u64,
    pub state: DurableDesignerState,
}

/// Stable persistence failure categories safe to return across the seam.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceErrorCode {
    Unavailable,
    Corrupt,
    Incompatible,
    Rejected,
}

/// Sanitized persistence failure that exposes no backend details.
#[derive(Clone, Debug, Deserialize, Error, Eq, PartialEq, Serialize)]
#[error("{message}")]
#[serde(deny_unknown_fields)]
pub struct PersistenceError {
    pub code: PersistenceErrorCode,
    pub message: String,
}

/// Durable storage adapter consumed by the command engine.
///
/// Implementations live outside this crate. The contract speaks only in
/// domain records, so neither storage handles nor journal layouts leak into
/// [`crate::DesignerSession`].
pub trait DesignerPersistence: Send + Sync {
    /// Load the last completely durable state for a project.
    fn load<'a>(
        &'a self,
        project_id: &'a ProjectId,
    ) -> SessionFuture<'a, Result<Option<DurableDesignerState>, PersistenceError>>;

    /// Atomically make the transaction the project's last durable state.
    fn commit<'a>(
        &'a self,
        transaction: &'a DesignerTransaction,
    ) -> SessionFuture<'a, Result<(), PersistenceError>>;
}

/// Deterministic process-local persistence used by pure domain tests and previews.
#[derive(Clone, Default)]
pub struct InMemoryDesignerPersistence {
    states: Arc<Mutex<BTreeMap<ProjectId, DesignerTransaction>>>,
}

impl InMemoryDesignerPersistence {
    /// Return the last committed transaction for inspection by adapter tests.
    #[must_use]
    pub fn transaction(&self, project_id: &ProjectId) -> Option<DesignerTransaction> {
        self.states
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(project_id)
            .cloned()
    }
}

impl DesignerPersistence for InMemoryDesignerPersistence {
    fn load<'a>(
        &'a self,
        project_id: &'a ProjectId,
    ) -> SessionFuture<'a, Result<Option<DurableDesignerState>, PersistenceError>> {
        Box::pin(async move {
            Ok(self
                .states
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(project_id)
                .map(|transaction| transaction.state.clone()))
        })
    }

    fn commit<'a>(
        &'a self,
        transaction: &'a DesignerTransaction,
    ) -> SessionFuture<'a, Result<(), PersistenceError>> {
        Box::pin(async move {
            let mut states = self
                .states
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(current) = states.get(&transaction.project_id) {
                if current.sequence > transaction.sequence {
                    return Err(PersistenceError {
                        code: PersistenceErrorCode::Rejected,
                        message: "the durable store already contains a newer revision".to_owned(),
                    });
                }
                if current.sequence == transaction.sequence && current != transaction {
                    return Err(PersistenceError {
                        code: PersistenceErrorCode::Rejected,
                        message: "the durable revision identity has different content".to_owned(),
                    });
                }
            }
            states.insert(transaction.project_id.clone(), transaction.clone());
            Ok(())
        })
    }
}
