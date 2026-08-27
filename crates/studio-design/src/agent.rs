//! Host-mediated live editing for agents and MCP clients.
//!
//! This module is deliberately independent of transport and presentation. A
//! host gives an agent a [`LiveAgentChannel`], which exposes only explicit
//! reads and the same validated [`crate::DesignerSession`] command engine used
//! by the native editor. Event sinks are a small injection point for an editor
//! dock, telemetry adapter, or deterministic test collector.
#![allow(
    missing_docs,
    reason = "closed agent channel records mirror the documented wire seam"
)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    command::{AppliedBatch, Command, CommandBatch, CommandPrecondition},
    model::{
        Actor, DesignNode, DesignerDiagnostic, DiagnosticSeverity, NodeId, NodeParent, OperationId,
        ProjectId, RevisionId, STUDIO_DESIGN_SCHEMA_VERSION, SelectionSnapshot,
        StudioDesignSnapshot, UndoGroupId,
    },
    persistence::SessionFuture,
    session::{
        BatchConflict, CommandOutcome, DesignerQuery, DesignerQueryResult, DesignerSession,
        HistorySnapshot,
    },
};

/// Stable identity for one host-owned agent run.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AgentRunId(String);

impl AgentRunId {
    /// Create a bounded run identity.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidAgentRunId`] for an empty, oversized, or control-bearing
    /// identity.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidAgentRunId> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
            return Err(InvalidAgentRunId);
        }
        Ok(Self(value))
    }

    /// Borrow the opaque wire identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AgentRunId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A run identity failed its bounded opaque-identity contract.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[error("agent run identity must be 1..=128 bytes and contain no control characters")]
pub struct InvalidAgentRunId;

/// Progress attached to every streamed agent batch.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentProgress {
    pub completed: u32,
    pub total: Option<u32>,
    pub message: String,
}

/// One typed batch in an agent stream.
///
/// The nested [`CommandBatch`] retains the canonical actor, base revision,
/// precondition, and undo-group metadata. The run identity and progress are
/// channel metadata and never become transport or UI state.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCommandBatch {
    pub run_id: AgentRunId,
    pub batch: CommandBatch,
    pub progress: AgentProgress,
}

impl AgentCommandBatch {
    /// Build one stream item from canonical command metadata and progress.
    #[must_use]
    pub fn new(run_id: AgentRunId, batch: CommandBatch, progress: AgentProgress) -> Self {
        Self {
            run_id,
            batch,
            progress,
        }
    }
}

/// Metadata used to admit a run to the host-owned channel.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentRunRequest {
    pub run_id: AgentRunId,
    pub actor: Actor,
    pub total_batches: Option<u32>,
}

impl AgentRunRequest {
    /// Build a run admission request.
    #[must_use]
    pub fn new(run_id: AgentRunId, actor: Actor, total_batches: Option<u32>) -> Self {
        Self {
            run_id,
            actor,
            total_batches,
        }
    }
}

/// A successful, cancelled, rejected, or conflicted streamed batch.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "status",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AgentBatchOutcome {
    Accepted(crate::CommandReceipt),
    Rejected(Vec<DesignerDiagnostic>),
    Conflict(Box<AgentConflict>),
    PersistenceFailed(crate::PersistenceError),
    Cancelled(AgentCancellation),
    RunNotFound,
}

/// Result metadata shared by every streamed batch result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentBatchResult {
    pub run_id: AgentRunId,
    pub operation_id: OperationId,
    pub progress: AgentProgress,
    pub outcome: AgentBatchOutcome,
    pub check: AgentCheckFeedback,
}

/// Short alias for [`AgentCommandBatch`] used by transport adapters.
pub type AgentBatch = AgentCommandBatch;

/// Cancellation result for a batch arriving after the cancellation boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCancellation {
    pub run_id: AgentRunId,
    pub operation_id: OperationId,
    pub message: String,
}

/// A stale agent intent and the current intent that caused an overlap.
///
/// Both the attempted batch and the intervening accepted batches remain in
/// the record. The current snapshot is an immutable conflict context; no
/// mutation is performed while constructing this value.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentConflict {
    pub conflict: BatchConflict,
    pub attempted: CommandBatch,
    pub current: StudioDesignSnapshot,
    pub intervening: Vec<AppliedBatch>,
}

/// Machine-readable validation feedback from the host's `studio check`
/// adapter.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCheckFeedback {
    pub diagnostics: Vec<DesignerDiagnostic>,
}

impl AgentCheckFeedback {
    /// Whether `studio check` found an error diagnostic.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }

    /// Whether `studio check` found a warning diagnostic.
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
    }
}

/// Host adapter for machine-readable `studio check` validation.
pub trait AgentChecker: Send {
    /// Check the immutable post-commit snapshot.
    fn check(&mut self, snapshot: &StudioDesignSnapshot) -> AgentCheckFeedback;
}

impl<F> AgentChecker for F
where
    F: FnMut(&StudioDesignSnapshot) -> AgentCheckFeedback + Send,
{
    fn check(&mut self, snapshot: &StudioDesignSnapshot) -> AgentCheckFeedback {
        self(snapshot)
    }
}

/// Default checker used when a host has not connected its `studio check`
/// implementation yet. The command engine's own design validation still runs
/// before commit.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopAgentChecker;

impl AgentChecker for NoopAgentChecker {
    fn check(&mut self, _snapshot: &StudioDesignSnapshot) -> AgentCheckFeedback {
        AgentCheckFeedback::default()
    }
}

/// UI-agnostic event emitted by a live agent channel.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentEvent {
    RunStarted {
        run_id: AgentRunId,
        actor: Actor,
    },
    Read {
        scope: AgentReadScope,
    },
    Progress {
        run_id: AgentRunId,
        operation_id: OperationId,
        progress: AgentProgress,
    },
    BatchAccepted {
        run_id: AgentRunId,
        receipt: crate::CommandReceipt,
        progress: AgentProgress,
    },
    CheckCompleted {
        run_id: AgentRunId,
        operation_id: OperationId,
        feedback: AgentCheckFeedback,
    },
    Warning {
        run_id: AgentRunId,
        operation_id: OperationId,
        diagnostics: Vec<DesignerDiagnostic>,
    },
    Failure {
        run_id: AgentRunId,
        operation_id: OperationId,
        diagnostics: Vec<DesignerDiagnostic>,
    },
    Conflict {
        run_id: AgentRunId,
        conflict: Box<AgentConflict>,
    },
    RunCancelled {
        run_id: AgentRunId,
    },
    BatchCancelled {
        run_id: AgentRunId,
        operation_id: OperationId,
    },
}

/// Injectable event sink for an editor dock or another host presentation.
pub trait AgentEventSink: Send {
    /// Receive one immutable event in stream order.
    fn emit(&mut self, event: AgentEvent);
}

impl<F> AgentEventSink for F
where
    F: FnMut(AgentEvent) + Send,
{
    fn emit(&mut self, event: AgentEvent) {
        self(event);
    }
}

/// Event sink that intentionally discards events.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopAgentEventSink;

impl AgentEventSink for NoopAgentEventSink {
    fn emit(&mut self, _event: AgentEvent) {}
}

/// Explicit read scopes available to an agent or MCP client.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentReadScope {
    Project,
    Selection,
    Subtree { root_node_id: NodeId },
    Schemas,
    Diagnostics,
    History,
}

/// Short alias for [`AgentReadScope`] used by host adapters.
pub type AgentScope = AgentReadScope;

/// Safe project metadata returned by the project read scope.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentProjectSummary {
    pub project_id: ProjectId,
    pub name: String,
    pub revision_id: RevisionId,
    pub screen_ids: Vec<crate::ScreenId>,
    pub node_count: usize,
    pub composition_count: usize,
    pub token_count: usize,
}

/// Deterministic subtree read result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSubtreeSnapshot {
    pub revision_id: RevisionId,
    pub root_node_id: NodeId,
    pub nodes: Vec<DesignNode>,
    pub parents: BTreeMap<NodeId, NodeParent>,
}

/// A closed command schema description exposed to an agent before mutation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCommandSchema {
    pub name: String,
    pub required_fields: Vec<String>,
    pub supported_preconditions: Vec<String>,
}

/// The typed command vocabulary available through this channel.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSchemaSnapshot {
    pub schema_version: u16,
    pub commands: Vec<AgentCommandSchema>,
}

/// Result of one explicit scoped read.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum AgentReadResult {
    Project(AgentProjectSummary),
    Selection(SelectionSnapshot),
    Subtree(AgentSubtreeSnapshot),
    Schemas(AgentSchemaSnapshot),
    Diagnostics(Vec<DesignerDiagnostic>),
    History(HistorySnapshot),
}

/// A scoped read could not be fulfilled.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AgentReadError {
    #[error("the requested subtree root does not exist: {0}")]
    NodeNotFound(NodeId),
    #[error("the Designer session returned an unexpected query result")]
    UnexpectedResult,
}

/// Error while admitting a run.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AgentRunError {
    #[error("agent run {0} is already active")]
    Duplicate(AgentRunId),
}

/// Summary returned when a host closes a run.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentRunSummary {
    pub run_id: AgentRunId,
    pub actor: Actor,
    pub cancelled: bool,
    pub accepted_operations: Vec<OperationId>,
    pub last_progress: Option<AgentProgress>,
}

struct RunState {
    request: AgentRunRequest,
    cancelled: bool,
    accepted_operations: Vec<OperationId>,
    last_progress: Option<AgentProgress>,
    undo_group_id: Option<UndoGroupId>,
    undo_group_name: Option<String>,
}

/// Host-mediated live agent editing channel.
pub struct LiveAgentChannel<S, C = NoopAgentChecker, E = NoopAgentEventSink> {
    session: S,
    checker: C,
    event_sink: E,
    runs: BTreeMap<AgentRunId, RunState>,
}

/// Short alias for [`LiveAgentChannel`].
pub type AgentChannel<S, C = NoopAgentChecker, E = NoopAgentEventSink> = LiveAgentChannel<S, C, E>;

impl<S: DesignerSession> LiveAgentChannel<S, NoopAgentChecker, NoopAgentEventSink> {
    /// Create a channel with no-op checking and event delivery.
    #[must_use]
    pub fn new(session: S) -> Self {
        Self::with_checker_and_event_sink(session, NoopAgentChecker, NoopAgentEventSink)
    }
}

impl<S: DesignerSession, E: AgentEventSink> LiveAgentChannel<S, NoopAgentChecker, E> {
    /// Create a channel with an injected editor-dock event sink.
    #[must_use]
    pub fn with_event_sink(session: S, event_sink: E) -> Self {
        Self::with_checker_and_event_sink(session, NoopAgentChecker, event_sink)
    }
}

impl<S: DesignerSession, C: AgentChecker, E: AgentEventSink> LiveAgentChannel<S, C, E> {
    /// Create a channel with host-owned check and presentation adapters.
    #[must_use]
    pub fn with_checker_and_event_sink(session: S, checker: C, event_sink: E) -> Self {
        Self {
            session,
            checker,
            event_sink,
            runs: BTreeMap::new(),
        }
    }

    /// Borrow the underlying session for native human edits and undo/redo.
    ///
    /// The caller still crosses the ordinary [`DesignerSession`] seam; this
    /// accessor does not expose persistence or storage handles.
    #[must_use]
    pub fn session(&self) -> &S {
        &self.session
    }

    /// Mutably borrow the underlying session for a host-authored human edit.
    #[must_use]
    pub fn session_mut(&mut self) -> &mut S {
        &mut self.session
    }

    /// Consume the channel and return its injected adapters.
    #[must_use]
    pub fn into_parts(self) -> (S, C, E) {
        (self.session, self.checker, self.event_sink)
    }

    /// Admit one run and establish its cancellation boundary.
    ///
    /// # Errors
    ///
    /// Returns [`AgentRunError::Duplicate`] when the run identity is already
    /// active in this channel.
    pub fn start_run(&mut self, request: AgentRunRequest) -> Result<(), AgentRunError> {
        if self.runs.contains_key(&request.run_id) {
            return Err(AgentRunError::Duplicate(request.run_id));
        }
        let event = AgentEvent::RunStarted {
            run_id: request.run_id.clone(),
            actor: request.actor.clone(),
        };
        let run_id = request.run_id.clone();
        self.runs.insert(
            run_id,
            RunState {
                request,
                cancelled: false,
                accepted_operations: Vec::new(),
                last_progress: None,
                undo_group_id: None,
                undo_group_name: None,
            },
        );
        self.event_sink.emit(event);
        Ok(())
    }

    /// Alias for [`Self::start_run`] used by stream adapters.
    ///
    /// # Errors
    ///
    /// Returns [`AgentRunError::Duplicate`] when the run identity is already
    /// active in this channel.
    pub fn begin_run(&mut self, request: AgentRunRequest) -> Result<(), AgentRunError> {
        self.start_run(request)
    }

    /// Cancel a run at the next batch boundary.
    ///
    /// Batches already accepted by the command engine remain durable and
    /// undoable. A later batch is never submitted to the engine.
    pub fn cancel_run(&mut self, run_id: &AgentRunId) -> bool {
        let Some(run) = self.runs.get_mut(run_id) else {
            return false;
        };
        if run.cancelled {
            return false;
        }
        run.cancelled = true;
        self.event_sink.emit(AgentEvent::RunCancelled {
            run_id: run_id.clone(),
        });
        true
    }

    /// Alias for [`Self::cancel_run`].
    pub fn cancel(&mut self, run_id: &AgentRunId) -> bool {
        self.cancel_run(run_id)
    }

    /// Close a run and return its safe progress summary.
    pub fn finish_run(&mut self, run_id: &AgentRunId) -> Option<AgentRunSummary> {
        let run = self.runs.remove(run_id)?;
        Some(AgentRunSummary {
            run_id: run.request.run_id,
            actor: run.request.actor,
            cancelled: run.cancelled,
            accepted_operations: run.accepted_operations,
            last_progress: run.last_progress,
        })
    }

    /// Read one explicit host-mediated scope.
    ///
    /// # Errors
    ///
    /// Returns an [`AgentReadError`] when the session cannot provide the
    /// requested typed scope or a subtree root is missing.
    pub fn read(&mut self, scope: AgentReadScope) -> Result<AgentReadResult, AgentReadError> {
        self.event_sink.emit(AgentEvent::Read {
            scope: scope.clone(),
        });
        match scope {
            AgentReadScope::Project => match self.session.query(DesignerQuery::Snapshot) {
                DesignerQueryResult::Snapshot(snapshot) => {
                    let design = snapshot.design;
                    Ok(AgentReadResult::Project(AgentProjectSummary {
                        project_id: design.project_id,
                        name: design.name,
                        revision_id: snapshot.revision.id,
                        screen_ids: design.screen_order,
                        node_count: design.nodes.len(),
                        composition_count: design.compositions.len(),
                        token_count: design.tokens.len(),
                    }))
                }
                _ => Err(AgentReadError::UnexpectedResult),
            },
            AgentReadScope::Selection => match self.session.query(DesignerQuery::SessionState) {
                DesignerQueryResult::SessionState(state) => {
                    Ok(AgentReadResult::Selection(state.selection))
                }
                _ => Err(AgentReadError::UnexpectedResult),
            },
            AgentReadScope::Subtree { root_node_id } => {
                let DesignerQueryResult::Snapshot(snapshot) =
                    self.session.query(DesignerQuery::Snapshot)
                else {
                    return Err(AgentReadError::UnexpectedResult);
                };
                let Some(root) = snapshot.design.nodes.get(&root_node_id) else {
                    return Err(AgentReadError::NodeNotFound(root_node_id));
                };
                let mut nodes = Vec::new();
                let mut parents = BTreeMap::new();
                let mut stack = vec![root.id.clone()];
                while let Some(node_id) = stack.pop() {
                    let Some(node) = snapshot.design.nodes.get(&node_id) else {
                        return Err(AgentReadError::NodeNotFound(node_id));
                    };
                    nodes.push(node.clone());
                    if let Some(parent) = snapshot.design.parents.get(&node_id) {
                        parents.insert(node_id.clone(), parent.clone());
                    }
                    stack.extend(node.children.iter().rev().cloned());
                }
                Ok(AgentReadResult::Subtree(AgentSubtreeSnapshot {
                    revision_id: snapshot.revision.id,
                    root_node_id,
                    nodes,
                    parents,
                }))
            }
            AgentReadScope::Schemas => Ok(AgentReadResult::Schemas(command_schemas())),
            AgentReadScope::Diagnostics => match self.session.query(DesignerQuery::Diagnostics) {
                DesignerQueryResult::Diagnostics(diagnostics) => {
                    Ok(AgentReadResult::Diagnostics(diagnostics))
                }
                _ => Err(AgentReadError::UnexpectedResult),
            },
            AgentReadScope::History => match self.session.query(DesignerQuery::History) {
                DesignerQueryResult::History(history) => Ok(AgentReadResult::History(history)),
                _ => Err(AgentReadError::UnexpectedResult),
            },
        }
    }

    /// Alias for [`Self::read`] that emphasizes the scope at call sites.
    ///
    /// # Errors
    ///
    /// Forwards the [`AgentReadError`] returned by [`Self::read`].
    pub fn read_scoped(
        &mut self,
        scope: AgentReadScope,
    ) -> Result<AgentReadResult, AgentReadError> {
        self.read(scope)
    }

    /// Submit one batch at a stream boundary.
    pub fn submit_batch(
        &mut self,
        incoming: AgentCommandBatch,
    ) -> SessionFuture<'_, AgentBatchResult> {
        Box::pin(async move { self.submit_batch_inner(incoming).await })
    }

    /// Alias for [`Self::submit_batch`].
    pub fn submit(&mut self, incoming: AgentCommandBatch) -> SessionFuture<'_, AgentBatchResult> {
        self.submit_batch(incoming)
    }

    /// Submit a finite stream of batches in order, stopping naturally at the
    /// run's cancellation boundary.
    pub fn submit_stream<'a, I>(
        &'a mut self,
        batches: I,
    ) -> SessionFuture<'a, Vec<AgentBatchResult>>
    where
        I: IntoIterator<Item = AgentCommandBatch> + Send + 'a,
        I::IntoIter: Send,
    {
        Box::pin(async move {
            let mut results = Vec::new();
            for batch in batches {
                let result = self.submit_batch(batch).await;
                let cancelled = matches!(&result.outcome, AgentBatchOutcome::Cancelled(_));
                results.push(result);
                if cancelled {
                    break;
                }
            }
            results
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "stream boundary keeps validation and event ordering local"
    )]
    async fn submit_batch_inner(&mut self, incoming: AgentCommandBatch) -> AgentBatchResult {
        let run_id = incoming.run_id.clone();
        let operation_id = incoming.batch.operation_id.clone();
        let progress = incoming.progress.clone();
        let Some(run) = self.runs.get_mut(&run_id) else {
            return AgentBatchResult {
                run_id,
                operation_id,
                progress,
                outcome: AgentBatchOutcome::RunNotFound,
                check: AgentCheckFeedback::default(),
            };
        };
        if run.cancelled {
            self.event_sink.emit(AgentEvent::BatchCancelled {
                run_id: run_id.clone(),
                operation_id: operation_id.clone(),
            });
            return AgentBatchResult {
                run_id: run_id.clone(),
                operation_id: operation_id.clone(),
                progress,
                outcome: AgentBatchOutcome::Cancelled(AgentCancellation {
                    run_id,
                    operation_id,
                    message: "the agent run was cancelled before this batch boundary".to_owned(),
                }),
                check: AgentCheckFeedback::default(),
            };
        }
        if let Some(diagnostics) =
            validate_progress(&run.request, run.last_progress.as_ref(), &progress)
        {
            self.event_sink.emit(AgentEvent::Failure {
                run_id: run_id.clone(),
                operation_id: operation_id.clone(),
                diagnostics: diagnostics.clone(),
            });
            return AgentBatchResult {
                run_id,
                operation_id,
                progress,
                outcome: AgentBatchOutcome::Rejected(diagnostics),
                check: AgentCheckFeedback::default(),
            };
        }
        if incoming.batch.actor != run.request.actor {
            let diagnostics = vec![agent_diagnostic(
                "AGENT_ACTOR_MISMATCH",
                "a streamed batch actor must match the actor that started the run",
            )];
            self.event_sink.emit(AgentEvent::Failure {
                run_id: run_id.clone(),
                operation_id: operation_id.clone(),
                diagnostics: diagnostics.clone(),
            });
            return AgentBatchResult {
                run_id,
                operation_id,
                progress,
                outcome: AgentBatchOutcome::Rejected(diagnostics),
                check: AgentCheckFeedback::default(),
            };
        }
        if let Some(group_id) = &run.undo_group_id {
            if group_id != &incoming.batch.undo_group_id
                || run.undo_group_name.as_deref() != Some(incoming.batch.undo_group_name.as_str())
            {
                let diagnostics = vec![agent_diagnostic(
                    "AGENT_UNDO_GROUP_MISMATCH",
                    "all batches in one agent run must share one undo-group identity and name",
                )];
                self.event_sink.emit(AgentEvent::Failure {
                    run_id: run_id.clone(),
                    operation_id: operation_id.clone(),
                    diagnostics: diagnostics.clone(),
                });
                return AgentBatchResult {
                    run_id,
                    operation_id,
                    progress,
                    outcome: AgentBatchOutcome::Rejected(diagnostics),
                    check: AgentCheckFeedback::default(),
                };
            }
        } else {
            run.undo_group_id = Some(incoming.batch.undo_group_id.clone());
            run.undo_group_name = Some(incoming.batch.undo_group_name.clone());
        }
        run.last_progress = Some(progress.clone());
        self.event_sink.emit(AgentEvent::Progress {
            run_id: run_id.clone(),
            operation_id: operation_id.clone(),
            progress: progress.clone(),
        });

        let DesignerQueryResult::Snapshot(current) = self.session.query(DesignerQuery::Snapshot)
        else {
            return self.internal_failure(run_id, operation_id, progress);
        };
        let mut batch = incoming.batch.clone();
        let mut intervening = Vec::new();
        if batch.base_revision != current.revision.id && batch.base_revision < current.revision.id {
            let DesignerQueryResult::History(history) = self.session.query(DesignerQuery::History)
            else {
                return self.internal_failure(run_id, operation_id, progress);
            };
            // Let the command engine's operation-id idempotency win before
            // stale-base analysis. This is what makes transport retries safe
            // after an independently rebased batch has already committed.
            let already_applied = history_has_operation(&history, &batch.operation_id);
            if !already_applied {
                intervening = intervening_batches(history, batch.base_revision);
                if intervening.is_empty()
                    || intervening
                        .iter()
                        .any(|applied| batches_overlap(&batch, applied, &current))
                {
                    let conflict = overlapping_conflict(
                        &batch,
                        &current,
                        &intervening,
                        "the stale agent batch overlaps an accepted intent",
                    );
                    self.event_sink.emit(AgentEvent::Conflict {
                        run_id: run_id.clone(),
                        conflict: Box::new(conflict.clone()),
                    });
                    return AgentBatchResult {
                        run_id,
                        operation_id,
                        progress,
                        outcome: AgentBatchOutcome::Conflict(Box::new(conflict)),
                        check: AgentCheckFeedback::default(),
                    };
                }
                batch.base_revision = current.revision.id;
            }
        }

        let outcome = self.session.submit(batch).await;
        match outcome {
            CommandOutcome::Accepted(receipt) => {
                let check = match self.session.query(DesignerQuery::Snapshot) {
                    DesignerQueryResult::Snapshot(snapshot) => self.checker.check(&snapshot),
                    _ => AgentCheckFeedback {
                        diagnostics: vec![agent_diagnostic(
                            "AGENT_CHECK_UNAVAILABLE",
                            "the committed snapshot could not be read for studio check",
                        )],
                    },
                };
                if let Some(run) = self.runs.get_mut(&run_id) {
                    run.accepted_operations.push(receipt.operation_id.clone());
                }
                self.event_sink.emit(AgentEvent::BatchAccepted {
                    run_id: run_id.clone(),
                    receipt: receipt.clone(),
                    progress: progress.clone(),
                });
                self.event_sink.emit(AgentEvent::CheckCompleted {
                    run_id: run_id.clone(),
                    operation_id: operation_id.clone(),
                    feedback: check.clone(),
                });
                emit_check_events(&mut self.event_sink, &run_id, &operation_id, &check);
                AgentBatchResult {
                    run_id,
                    operation_id,
                    progress,
                    outcome: AgentBatchOutcome::Accepted(receipt),
                    check,
                }
            }
            CommandOutcome::Rejected(diagnostics) => {
                self.event_sink.emit(AgentEvent::Failure {
                    run_id: run_id.clone(),
                    operation_id: operation_id.clone(),
                    diagnostics: diagnostics.clone(),
                });
                AgentBatchResult {
                    run_id,
                    operation_id,
                    progress,
                    outcome: AgentBatchOutcome::Rejected(diagnostics),
                    check: AgentCheckFeedback::default(),
                }
            }
            CommandOutcome::Conflict(conflict) => {
                let latest = match self.session.query(DesignerQuery::Snapshot) {
                    DesignerQueryResult::Snapshot(snapshot) => snapshot,
                    _ => current,
                };
                let agent_conflict = AgentConflict {
                    conflict,
                    attempted: incoming.batch,
                    current: latest,
                    intervening,
                };
                self.event_sink.emit(AgentEvent::Conflict {
                    run_id: run_id.clone(),
                    conflict: Box::new(agent_conflict.clone()),
                });
                AgentBatchResult {
                    run_id,
                    operation_id,
                    progress,
                    outcome: AgentBatchOutcome::Conflict(Box::new(agent_conflict)),
                    check: AgentCheckFeedback::default(),
                }
            }
            CommandOutcome::PersistenceFailed(error) => {
                let diagnostics = vec![agent_diagnostic(
                    "AGENT_PERSISTENCE_FAILED",
                    error.to_string(),
                )];
                self.event_sink.emit(AgentEvent::Failure {
                    run_id: run_id.clone(),
                    operation_id: operation_id.clone(),
                    diagnostics,
                });
                AgentBatchResult {
                    run_id,
                    operation_id,
                    progress,
                    outcome: AgentBatchOutcome::PersistenceFailed(error),
                    check: AgentCheckFeedback::default(),
                }
            }
        }
    }

    fn internal_failure(
        &mut self,
        run_id: AgentRunId,
        operation_id: OperationId,
        progress: AgentProgress,
    ) -> AgentBatchResult {
        let diagnostics = vec![agent_diagnostic(
            "AGENT_SESSION_QUERY_FAILED",
            "the host could not read the active Designer session",
        )];
        self.event_sink.emit(AgentEvent::Failure {
            run_id: run_id.clone(),
            operation_id: operation_id.clone(),
            diagnostics: diagnostics.clone(),
        });
        AgentBatchResult {
            run_id,
            operation_id,
            progress,
            outcome: AgentBatchOutcome::Rejected(diagnostics),
            check: AgentCheckFeedback::default(),
        }
    }
}

fn command_schemas() -> AgentSchemaSnapshot {
    let conditions = [
        "node_exists",
        "node_missing",
        "parent_equals",
        "child_index_equals",
        "property_equals",
    ];
    let schema = |name: &str, required_fields: &[&str]| AgentCommandSchema {
        name: name.to_owned(),
        required_fields: required_fields
            .iter()
            .map(|field| (*field).to_owned())
            .collect(),
        supported_preconditions: conditions
            .iter()
            .map(|condition| (*condition).to_owned())
            .collect(),
    };
    AgentSchemaSnapshot {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        commands: vec![
            schema("insert_node", &["parent", "node"]),
            schema("move_node", &["node_id", "destination"]),
            schema("reorder_node", &["node_id", "index"]),
            schema(
                "duplicate_node",
                &["source_node_id", "destination", "id_map"],
            ),
            schema("delete_node", &["node_id"]),
            schema("restore_node", &["tombstone"]),
            schema("set_property", &["node_id", "property", "value"]),
            schema("rename_node", &["node_id", "name"]),
        ],
    }
}

fn validate_progress(
    request: &AgentRunRequest,
    previous: Option<&AgentProgress>,
    progress: &AgentProgress,
) -> Option<Vec<DesignerDiagnostic>> {
    if progress.message.len() > 512 || progress.message.chars().any(char::is_control) {
        return Some(vec![agent_diagnostic(
            "AGENT_PROGRESS_INVALID",
            "progress messages must contain at most 512 safe bytes",
        )]);
    }
    if progress.total != request.total_batches {
        return Some(vec![agent_diagnostic(
            "AGENT_PROGRESS_TOTAL_MISMATCH",
            "batch progress total must match the run declaration",
        )]);
    }
    if progress
        .total
        .is_some_and(|total| progress.completed > total)
    {
        return Some(vec![agent_diagnostic(
            "AGENT_PROGRESS_INVALID",
            "completed progress cannot exceed the declared total",
        )]);
    }
    if previous.is_some_and(|prior| progress.completed < prior.completed) {
        return Some(vec![agent_diagnostic(
            "AGENT_PROGRESS_REGRESSION",
            "progress cannot move backwards within one agent run",
        )]);
    }
    None
}

fn intervening_batches(history: HistorySnapshot, base_revision: RevisionId) -> Vec<AppliedBatch> {
    history
        .entries
        .into_iter()
        .flat_map(|entry| entry.batches)
        .filter(|batch| batch.committed_revision > base_revision)
        .collect()
}

fn history_has_operation(history: &HistorySnapshot, operation_id: &OperationId) -> bool {
    history.entries.iter().any(|entry| {
        entry
            .batches
            .iter()
            .any(|batch| batch.batch.operation_id == *operation_id)
    })
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Footprint {
    Node(NodeId),
    Property(NodeId, String),
    Identity(NodeId),
    ChildList(NodeParent),
}

fn batches_overlap(
    candidate: &CommandBatch,
    accepted: &AppliedBatch,
    current: &StudioDesignSnapshot,
) -> bool {
    let candidate_footprint = batch_footprint(candidate, &[], current);
    let accepted_footprint = batch_footprint(&accepted.batch, &accepted.inverse_commands, current);
    candidate_footprint.iter().any(|left| {
        accepted_footprint
            .iter()
            .any(|right| footprints_overlap(left, right))
    })
}

fn batch_footprint(
    batch: &CommandBatch,
    inverse_commands: &[Command],
    current: &StudioDesignSnapshot,
) -> BTreeSet<Footprint> {
    let mut footprints = BTreeSet::new();
    for command in &batch.commands {
        command_footprint(command, current, &mut footprints);
    }
    for command in inverse_commands {
        command_footprint(command, current, &mut footprints);
    }
    for condition in &batch.preconditions {
        precondition_footprint(condition, &mut footprints);
    }
    footprints
}

fn command_footprint(
    command: &Command,
    current: &StudioDesignSnapshot,
    footprints: &mut BTreeSet<Footprint>,
) {
    match command {
        Command::InsertNode { parent, node } => {
            footprints.insert(Footprint::Identity(node.id.clone()));
            footprints.insert(Footprint::ChildList(parent.parent.clone()));
        }
        Command::MoveNode {
            node_id,
            destination,
        } => {
            footprints.insert(Footprint::Node(node_id.clone()));
            footprints.insert(Footprint::ChildList(destination.parent.clone()));
            if let Some(parent) = current.design.parents.get(node_id) {
                footprints.insert(Footprint::ChildList(parent.clone()));
            }
        }
        Command::ReorderNode { node_id, .. } | Command::DeleteNode { node_id } => {
            footprints.insert(Footprint::Node(node_id.clone()));
            if let Some(parent) = current.design.parents.get(node_id) {
                footprints.insert(Footprint::ChildList(parent.clone()));
            }
        }
        Command::DuplicateNode {
            source_node_id,
            destination,
            id_map,
        } => {
            footprints.insert(Footprint::Node(source_node_id.clone()));
            footprints.insert(Footprint::ChildList(destination.parent.clone()));
            for id in id_map.values() {
                footprints.insert(Footprint::Identity(id.clone()));
            }
        }
        Command::RestoreNode { tombstone } => {
            footprints.insert(Footprint::Node(tombstone.root_node_id.clone()));
            footprints.insert(Footprint::ChildList(tombstone.detached_from.clone()));
            for id in tombstone.nodes.keys() {
                footprints.insert(Footprint::Identity(id.clone()));
            }
        }
        Command::SetProperty {
            node_id, property, ..
        } => {
            footprints.insert(Footprint::Property(node_id.clone(), property.clone()));
        }
        Command::RenameNode { node_id, .. } => {
            footprints.insert(Footprint::Property(node_id.clone(), "@name".to_owned()));
        }
    }
}

fn precondition_footprint(condition: &CommandPrecondition, footprints: &mut BTreeSet<Footprint>) {
    match condition {
        CommandPrecondition::NodeExists { node_id }
        | CommandPrecondition::NodeMissing { node_id }
        | CommandPrecondition::ChildIndexEquals { node_id, .. } => {
            footprints.insert(Footprint::Node(node_id.clone()));
        }
        CommandPrecondition::ParentEquals { node_id, parent } => {
            footprints.insert(Footprint::Node(node_id.clone()));
            footprints.insert(Footprint::ChildList(parent.clone()));
        }
        CommandPrecondition::PropertyEquals {
            node_id, property, ..
        } => {
            footprints.insert(Footprint::Property(node_id.clone(), property.clone()));
        }
    }
}

fn footprints_overlap(left: &Footprint, right: &Footprint) -> bool {
    match (left, right) {
        (Footprint::Node(left), right) => node_footprint_matches(left, right),
        (left, Footprint::Node(right)) => node_footprint_matches(right, left),
        (Footprint::Identity(left), Footprint::Identity(right)) => left == right,
        (
            Footprint::Property(left_node, left_property),
            Footprint::Property(right_node, right_property),
        ) => left_node == right_node && left_property == right_property,
        (Footprint::ChildList(left), Footprint::ChildList(right)) => left == right,
        _ => false,
    }
}

fn node_footprint_matches(node_id: &NodeId, footprint: &Footprint) -> bool {
    match footprint {
        Footprint::Node(other) | Footprint::Identity(other) | Footprint::Property(other, _) => {
            node_id == other
        }
        Footprint::ChildList(_) => false,
    }
}

fn overlapping_conflict(
    attempted: &CommandBatch,
    current: &StudioDesignSnapshot,
    intervening: &[AppliedBatch],
    message: &str,
) -> AgentConflict {
    AgentConflict {
        conflict: BatchConflict {
            operation_id: attempted.operation_id.clone(),
            expected_revision: attempted.base_revision,
            actual_revision: current.revision.id,
            failed_precondition: None,
            code: "AGENT_OVERLAPPING_CONFLICT".to_owned(),
            message: message.to_owned(),
        },
        attempted: attempted.clone(),
        current: current.clone(),
        intervening: intervening.to_vec(),
    }
}

fn emit_check_events<E: AgentEventSink>(
    event_sink: &mut E,
    run_id: &AgentRunId,
    operation_id: &OperationId,
    check: &AgentCheckFeedback,
) {
    let warnings = check
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
        .cloned()
        .collect::<Vec<_>>();
    if !warnings.is_empty() {
        event_sink.emit(AgentEvent::Warning {
            run_id: run_id.clone(),
            operation_id: operation_id.clone(),
            diagnostics: warnings,
        });
    }
    let errors = check
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .cloned()
        .collect::<Vec<_>>();
    if !errors.is_empty() {
        event_sink.emit(AgentEvent::Failure {
            run_id: run_id.clone(),
            operation_id: operation_id.clone(),
            diagnostics: errors,
        });
    }
}

fn agent_diagnostic(code: &str, message: impl Into<String>) -> DesignerDiagnostic {
    DesignerDiagnostic {
        code: code.to_owned(),
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        node_id: None,
        interaction_id: None,
        collection_id: None,
        binding_id: None,
        form_id: None,
        record_id: None,
    }
}
