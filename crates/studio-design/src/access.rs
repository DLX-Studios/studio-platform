//! Scoped access to the host-owned [`crate::DesignerSession`] seam.
//!
//! This module is deliberately transport-neutral.  Agent channels and MCP
//! adapters can both delegate through [`ScopedDesignerAccess`] so scope
//! checks, command validation, conflicts, and receipts cannot drift between
//! callers.

#![allow(clippy::all)]
#![allow(
    clippy::doc_markdown,
    clippy::manual_let_else,
    clippy::missing_errors_doc,
    clippy::result_large_err
)]

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    StudioDesignSnapshot,
    command::{Command, CommandBatch, CommandPrecondition, HistoryEntry},
    model::{Actor, NodeId, NodeParent, OperationId, ProjectId},
    persistence::SessionFuture,
    session::{
        CommandOutcome, DesignerQuery, DesignerQueryResult, DesignerSession, HistoryOperation,
    },
};

/// A capability that may be granted to one untrusted Designer caller.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum DesignerCapability {
    /// Read the complete project snapshot.
    ProjectRead,
    /// Read the session-owned selection and presentation state.
    SelectionRead,
    /// Read one node and its descendants.
    SubtreeRead(NodeId),
    /// Read command-schema metadata when a channel exposes it.
    SchemasRead,
    /// Read authoring diagnostics.
    DiagnosticsRead,
    /// Read immutable revision history.
    HistoryRead,
    /// Submit commands anywhere in the bound project.
    CommandWrite,
    /// Submit commands only within one node subtree.
    SubtreeWrite(NodeId),
    /// Apply undo or redo within the caller's command scope.
    HistoryWrite,
}

/// Explicit project-bound read and mutation capabilities.
///
/// A scope starts empty.  Callers must opt into every read and mutation they
/// need; merely possessing a [`DesignerSession`] adapter never grants access.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DesignerScope {
    project_id: ProjectId,
    capabilities: BTreeSet<DesignerCapability>,
}

impl DesignerScope {
    /// Create an empty scope bound to one project identity.
    #[must_use]
    pub fn for_project(project_id: ProjectId) -> Self {
        Self {
            project_id,
            capabilities: BTreeSet::new(),
        }
    }

    /// The project identity to which this scope is bound.
    #[must_use]
    pub fn project_id(&self) -> &ProjectId {
        &self.project_id
    }

    /// All capabilities granted by this scope.
    #[must_use]
    pub fn capabilities(&self) -> &BTreeSet<DesignerCapability> {
        &self.capabilities
    }

    /// Grant complete project snapshot reads.
    #[must_use]
    pub fn allow_project_read(mut self) -> Self {
        self.capabilities.insert(DesignerCapability::ProjectRead);
        self
    }

    /// Grant session selection/presentation reads.
    #[must_use]
    pub fn allow_selection_read(mut self) -> Self {
        self.capabilities.insert(DesignerCapability::SelectionRead);
        self
    }

    /// Grant reads of one node and all of its descendants.
    #[must_use]
    pub fn allow_subtree_read(mut self, root: NodeId) -> Self {
        self.capabilities
            .insert(DesignerCapability::SubtreeRead(root));
        self
    }

    /// Grant command schema reads for a future channel query.
    #[must_use]
    pub fn allow_schemas_read(mut self) -> Self {
        self.capabilities.insert(DesignerCapability::SchemasRead);
        self
    }

    /// Grant authoring diagnostic reads.
    #[must_use]
    pub fn allow_diagnostics_read(mut self) -> Self {
        self.capabilities
            .insert(DesignerCapability::DiagnosticsRead);
        self
    }

    /// Grant immutable revision-history reads.
    #[must_use]
    pub fn allow_history_read(mut self) -> Self {
        self.capabilities.insert(DesignerCapability::HistoryRead);
        self
    }

    /// Grant command writes anywhere in the bound project.
    #[must_use]
    pub fn allow_command_write(mut self) -> Self {
        self.capabilities.insert(DesignerCapability::CommandWrite);
        self
    }

    /// Grant command writes only within one node subtree.
    #[must_use]
    pub fn allow_subtree_write(mut self, root: NodeId) -> Self {
        self.capabilities
            .insert(DesignerCapability::SubtreeWrite(root));
        self
    }

    /// Grant undo/redo for commands admitted by this scope.
    #[must_use]
    pub fn allow_history_write(mut self) -> Self {
        self.capabilities.insert(DesignerCapability::HistoryWrite);
        self
    }

    fn contains(&self, capability: &DesignerCapability) -> bool {
        self.capabilities.contains(capability)
    }

    fn has_project_write(&self) -> bool {
        self.contains(&DesignerCapability::CommandWrite)
    }

    fn read_node_allowed(&self, design: &crate::StudioDesign, node_id: &NodeId) -> bool {
        self.contains(&DesignerCapability::ProjectRead)
            || self.capabilities.iter().any(|capability| {
                let DesignerCapability::SubtreeRead(root) = capability else {
                    return false;
                };
                is_in_subtree(design, node_id, root)
            })
    }

    fn write_node_allowed(&self, design: &crate::StudioDesign, node_id: &NodeId) -> bool {
        self.has_project_write()
            || self.capabilities.iter().any(|capability| {
                let DesignerCapability::SubtreeWrite(root) = capability else {
                    return false;
                };
                is_in_subtree(design, node_id, root)
            })
    }

    fn write_parent_allowed(&self, design: &crate::StudioDesign, parent: &NodeParent) -> bool {
        match parent_anchor(design, parent) {
            Some(node_id) => self.write_node_allowed(design, &node_id),
            None => false,
        }
    }
}

/// Operation categories used in deterministic scope-denial records.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScopedOperation {
    /// A typed Designer query.
    Query(DesignerQuery),
    /// A command-batch submission.
    Submit {
        /// Stable operation identity supplied by the caller.
        operation_id: OperationId,
    },
    /// An undo request.
    Undo {
        /// Stable operation identity supplied by the caller.
        operation_id: OperationId,
    },
    /// A redo request.
    Redo {
        /// Stable operation identity supplied by the caller.
        operation_id: OperationId,
    },
}

/// Stable, transport-neutral denial returned before an untrusted operation
/// reaches the underlying session.
#[derive(Clone, Debug, Deserialize, Eq, Error, PartialEq, Serialize)]
#[error("{message}")]
#[serde(deny_unknown_fields)]
pub struct ScopeDenied {
    /// Stable denial code shared by agent and MCP adapters.
    pub code: String,
    /// The project the caller attempted to access.
    pub project_id: ProjectId,
    /// The operation that was denied.
    pub operation: ScopedOperation,
    /// The capability required by the operation.
    pub required: DesignerCapability,
    /// Safe explanation suitable for a caller or audit record.
    pub message: String,
}

impl ScopeDenied {
    fn missing(
        project_id: &ProjectId,
        operation: ScopedOperation,
        required: DesignerCapability,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: "DESIGN_SCOPE_DENIED".to_owned(),
            project_id: project_id.clone(),
            operation,
            required,
            message: message.into(),
        }
    }

    fn project_mismatch(project_id: &ProjectId, operation: ScopedOperation) -> Self {
        Self::missing(
            project_id,
            operation,
            DesignerCapability::ProjectRead,
            "the requested project is outside the caller scope",
        )
    }
}

/// Shared scoped query/command interface for agent and MCP channels.
pub trait ScopedDesignerAccess: Send {
    /// Execute one query after scope validation.
    fn query_scoped(&self, query: DesignerQuery) -> Result<DesignerQueryResult, ScopeDenied>;

    /// Submit one actor-attributed command batch after scope validation.
    fn submit_scoped(
        &mut self,
        actor: Actor,
        batch: CommandBatch,
    ) -> SessionFuture<'_, Result<CommandOutcome, ScopeDenied>>;

    /// Undo one admitted history group after scope validation.
    fn undo_scoped(
        &mut self,
        actor: Actor,
        operation: HistoryOperation,
    ) -> SessionFuture<'_, Result<CommandOutcome, ScopeDenied>>;

    /// Redo one admitted history group after scope validation.
    fn redo_scoped(
        &mut self,
        actor: Actor,
        operation: HistoryOperation,
    ) -> SessionFuture<'_, Result<CommandOutcome, ScopeDenied>>;

    /// Return the immutable scope attached to this access adapter.
    fn scope(&self) -> &DesignerScope;
}

/// A scope-enforcing adapter around the existing public DesignerSession seam.
pub struct ScopedDesignerSession<S> {
    inner: S,
    scope: DesignerScope,
}

impl<S: DesignerSession> ScopedDesignerSession<S> {
    /// Attach an explicit scope to a session.
    #[must_use]
    pub fn new(inner: S, scope: DesignerScope) -> Self {
        Self { inner, scope }
    }

    /// Borrow the attached scope.
    #[must_use]
    pub fn scope(&self) -> &DesignerScope {
        &self.scope
    }

    fn snapshot(&self) -> StudioDesignSnapshot {
        match self.inner.query(DesignerQuery::Snapshot) {
            DesignerQueryResult::Snapshot(snapshot) => snapshot,
            _ => {
                unreachable!("DesignerSession::query returns the requested result variant")
            }
        }
    }

    fn project_check(
        &self,
        snapshot: &StudioDesignSnapshot,
        operation: ScopedOperation,
    ) -> Result<(), ScopeDenied> {
        if snapshot.design.project_id == *self.scope.project_id() {
            Ok(())
        } else {
            Err(ScopeDenied::project_mismatch(
                self.scope.project_id(),
                operation,
            ))
        }
    }

    fn query_check(
        &self,
        snapshot: &StudioDesignSnapshot,
        query: &DesignerQuery,
    ) -> Result<(), ScopeDenied> {
        let operation = ScopedOperation::Query(query.clone());
        self.project_check(snapshot, operation.clone())?;
        match query {
            DesignerQuery::Snapshot if self.scope.contains(&DesignerCapability::ProjectRead) => {
                Ok(())
            }
            DesignerQuery::Node { node_id }
                if self.scope.read_node_allowed(&snapshot.design, node_id) =>
            {
                Ok(())
            }
            DesignerQuery::Diagnostics
                if self.scope.contains(&DesignerCapability::DiagnosticsRead) =>
            {
                Ok(())
            }
            DesignerQuery::History if self.scope.contains(&DesignerCapability::HistoryRead) => {
                Ok(())
            }
            DesignerQuery::SessionState
                if self.scope.contains(&DesignerCapability::SelectionRead) =>
            {
                Ok(())
            }
            DesignerQuery::Snapshot => Err(ScopeDenied::missing(
                self.scope.project_id(),
                operation,
                DesignerCapability::ProjectRead,
                "project snapshot reads are outside the caller scope",
            )),
            DesignerQuery::Node { node_id } => Err(ScopeDenied::missing(
                self.scope.project_id(),
                operation,
                DesignerCapability::SubtreeRead(node_id.clone()),
                "the requested node is outside the caller scope",
            )),
            DesignerQuery::Diagnostics => Err(ScopeDenied::missing(
                self.scope.project_id(),
                operation,
                DesignerCapability::DiagnosticsRead,
                "diagnostic reads are outside the caller scope",
            )),
            DesignerQuery::History => Err(ScopeDenied::missing(
                self.scope.project_id(),
                operation,
                DesignerCapability::HistoryRead,
                "history reads are outside the caller scope",
            )),
            DesignerQuery::SessionState => Err(ScopeDenied::missing(
                self.scope.project_id(),
                operation,
                DesignerCapability::SelectionRead,
                "selection reads are outside the caller scope",
            )),
            _ => Err(ScopeDenied::missing(
                self.scope.project_id(),
                operation,
                DesignerCapability::ProjectRead,
                "the requested query is outside the caller scope",
            )),
        }
    }

    fn batch_check(
        &self,
        snapshot: &StudioDesignSnapshot,
        batch: &CommandBatch,
    ) -> Result<(), ScopeDenied> {
        let operation = ScopedOperation::Submit {
            operation_id: batch.operation_id.clone(),
        };
        self.project_check(snapshot, operation.clone())?;
        if batch.project_id != *self.scope.project_id() {
            return Err(ScopeDenied::missing(
                self.scope.project_id(),
                operation,
                DesignerCapability::CommandWrite,
                "the command targets a project outside the caller scope",
            ));
        }
        if self.scope.has_project_write() {
            return Ok(());
        }
        if batch
            .preconditions
            .iter()
            .all(|precondition| precondition_allowed(&self.scope, &snapshot.design, precondition))
            && batch
                .commands
                .iter()
                .all(|command| command_allowed(&self.scope, &snapshot.design, command))
        {
            return Ok(());
        }
        Err(ScopeDenied::missing(
            self.scope.project_id(),
            operation,
            DesignerCapability::CommandWrite,
            "one or more command targets are outside the caller scope",
        ))
    }

    fn history_check(
        &self,
        snapshot: &StudioDesignSnapshot,
        operation: &HistoryOperation,
        redo: bool,
    ) -> Result<(), ScopeDenied> {
        let scoped_operation = if redo {
            ScopedOperation::Redo {
                operation_id: operation.operation_id.clone(),
            }
        } else {
            ScopedOperation::Undo {
                operation_id: operation.operation_id.clone(),
            }
        };
        self.project_check(snapshot, scoped_operation.clone())?;
        if !self.scope.contains(&DesignerCapability::HistoryWrite) {
            return Err(ScopeDenied::missing(
                self.scope.project_id(),
                scoped_operation,
                DesignerCapability::HistoryWrite,
                "history mutations are outside the caller scope",
            ));
        }
        if self.scope.has_project_write() {
            return Ok(());
        }
        let history = match self.inner.query(DesignerQuery::History) {
            DesignerQueryResult::History(history) => history,
            _ => {
                unreachable!("DesignerSession::query returns the requested result variant")
            }
        };
        let entry = if redo {
            history.entries.get(history.cursor)
        } else {
            history
                .cursor
                .checked_sub(1)
                .and_then(|index| history.entries.get(index))
        };
        let commands = entry
            .map(|entry| history_commands(entry, redo))
            .unwrap_or_default();
        if commands
            .iter()
            .all(|command| command_allowed(&self.scope, &snapshot.design, command))
        {
            Ok(())
        } else {
            Err(ScopeDenied::missing(
                self.scope.project_id(),
                scoped_operation,
                DesignerCapability::CommandWrite,
                "the history group contains commands outside the caller scope",
            ))
        }
    }

    fn submit_as(
        &mut self,
        actor: Actor,
        mut batch: CommandBatch,
    ) -> SessionFuture<'_, Result<CommandOutcome, ScopeDenied>> {
        let snapshot = self.snapshot();
        if let Err(error) = self.batch_check(&snapshot, &batch) {
            return Box::pin(async move { Err(error) });
        }
        batch.actor = actor;
        Box::pin(async move { Ok(self.inner.submit(batch).await) })
    }

    fn history_as(
        &mut self,
        actor: Actor,
        mut operation: HistoryOperation,
        redo: bool,
    ) -> SessionFuture<'_, Result<CommandOutcome, ScopeDenied>> {
        let snapshot = self.snapshot();
        if let Err(error) = self.history_check(&snapshot, &operation, redo) {
            return Box::pin(async move { Err(error) });
        }
        operation.actor = actor;
        if redo {
            Box::pin(async move { Ok(self.inner.redo(operation).await) })
        } else {
            Box::pin(async move { Ok(self.inner.undo(operation).await) })
        }
    }
}

impl<S: DesignerSession> ScopedDesignerAccess for ScopedDesignerSession<S> {
    fn query_scoped(&self, query: DesignerQuery) -> Result<DesignerQueryResult, ScopeDenied> {
        let snapshot = self.snapshot();
        self.query_check(&snapshot, &query)?;
        Ok(self.inner.query(query))
    }

    fn submit_scoped(
        &mut self,
        actor: Actor,
        batch: CommandBatch,
    ) -> SessionFuture<'_, Result<CommandOutcome, ScopeDenied>> {
        self.submit_as(actor, batch)
    }

    fn undo_scoped(
        &mut self,
        actor: Actor,
        operation: HistoryOperation,
    ) -> SessionFuture<'_, Result<CommandOutcome, ScopeDenied>> {
        self.history_as(actor, operation, false)
    }

    fn redo_scoped(
        &mut self,
        actor: Actor,
        operation: HistoryOperation,
    ) -> SessionFuture<'_, Result<CommandOutcome, ScopeDenied>> {
        self.history_as(actor, operation, true)
    }

    fn scope(&self) -> &DesignerScope {
        &self.scope
    }
}

fn parent_anchor(design: &crate::StudioDesign, parent: &NodeParent) -> Option<NodeId> {
    match parent {
        NodeParent::Node { node_id } => Some(node_id.clone()),
        NodeParent::Screen { screen_id } => design
            .screens
            .get(screen_id)
            .map(|screen| screen.root_node_id.clone()),
        NodeParent::Composition { composition_id } => design
            .compositions
            .get(composition_id)
            .map(|composition| composition.root_node_id.clone()),
    }
}

fn is_in_subtree(design: &crate::StudioDesign, node_id: &NodeId, root: &NodeId) -> bool {
    let mut cursor = Some(node_id);
    while let Some(current) = cursor {
        if current == root {
            return true;
        }
        cursor = match design.parents.get(current) {
            Some(NodeParent::Node { node_id }) => Some(node_id),
            Some(NodeParent::Screen { .. } | NodeParent::Composition { .. }) | None => None,
        };
    }
    false
}

fn precondition_allowed(
    scope: &DesignerScope,
    design: &crate::StudioDesign,
    precondition: &CommandPrecondition,
) -> bool {
    if scope.has_project_write() {
        return true;
    }
    let node_id = match precondition {
        CommandPrecondition::NodeExists { node_id }
        | CommandPrecondition::NodeMissing { node_id }
        | CommandPrecondition::ParentEquals { node_id, .. }
        | CommandPrecondition::ChildIndexEquals { node_id, .. }
        | CommandPrecondition::PropertyEquals { node_id, .. } => node_id,
        _ => return false,
    };
    !design.nodes.contains_key(node_id) || scope.write_node_allowed(design, node_id)
}

fn command_allowed(scope: &DesignerScope, design: &crate::StudioDesign, command: &Command) -> bool {
    if scope.has_project_write() {
        return true;
    }
    match command {
        Command::InsertNode { parent, .. } => scope.write_parent_allowed(design, &parent.parent),
        Command::MoveNode {
            node_id,
            destination,
        } => {
            design
                .parents
                .get(node_id)
                .is_some_and(|parent| scope.write_parent_allowed(design, parent))
                && scope.write_node_allowed(design, node_id)
                && scope.write_parent_allowed(design, &destination.parent)
        }
        Command::ReorderNode { node_id, .. } => {
            design
                .parents
                .get(node_id)
                .is_some_and(|parent| scope.write_parent_allowed(design, parent))
                && scope.write_node_allowed(design, node_id)
        }
        Command::DuplicateNode {
            source_node_id,
            destination,
            ..
        } => {
            scope.write_node_allowed(design, source_node_id)
                && scope.write_parent_allowed(design, &destination.parent)
        }
        Command::DeleteNode { node_id }
        | Command::SetProperty { node_id, .. }
        | Command::RenameNode { node_id, .. } => scope.write_node_allowed(design, node_id),
        Command::RestoreNode { tombstone } => {
            scope.write_parent_allowed(design, &tombstone.detached_from)
        }
        _ => false,
    }
}

fn history_commands(entry: &HistoryEntry, redo: bool) -> Vec<Command> {
    if redo {
        entry
            .batches
            .iter()
            .flat_map(|batch| batch.batch.commands.clone())
            .collect()
    } else {
        entry
            .batches
            .iter()
            .rev()
            .flat_map(|batch| batch.inverse_commands.clone())
            .collect()
    }
}
