//! Transport-neutral MCP client adapter.
//!
//! [`McpClient`] intentionally exposes only the shared scoped Designer
//! channel.  It has no database, filesystem, network, GPUI, or privileged
//! mutation handle.  The host supplies the MCP actor identity and scope;
//! every accepted receipt therefore records [`ActorKind::Mcp`] regardless of
//! actor metadata included by an untrusted wire client.

use thiserror::Error;

use crate::{
    access::{DesignerScope, ScopeDenied, ScopedDesignerAccess, ScopedDesignerSession},
    command::CommandBatch,
    model::{Actor, ActorKind, ProjectId},
    persistence::SessionFuture,
    session::{CommandOutcome, DesignerQuery, DesignerQueryResult, HistoryOperation},
};

/// Failure while opening a transport-neutral MCP client.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum McpClientError {
    /// The host must attribute MCP operations to an MCP actor.
    #[error("MCP client actor must use actor kind mcp")]
    ActorKindRequired,
    /// A scope may not be attached to a different project than the session.
    #[error("MCP scope targets project {scope_project}, but the session is for {session_project}")]
    ProjectMismatch {
        /// Project identity in the requested scope.
        scope_project: ProjectId,
        /// Project identity owned by the session.
        session_project: ProjectId,
    },
}

/// An MCP caller connected to one explicitly scoped Designer session.
pub struct McpClient<S> {
    access: ScopedDesignerSession<S>,
    actor: Actor,
}

impl<S: crate::DesignerSession> McpClient<S> {
    /// Connect an MCP actor to one project-bound scope.
    ///
    /// The session is inspected only through its existing public snapshot
    /// query.  The returned client does not expose the wrapped session or any
    /// host persistence handle.
    pub fn connect(session: S, scope: DesignerScope, actor: Actor) -> Result<Self, McpClientError> {
        if actor.kind != ActorKind::Mcp {
            return Err(McpClientError::ActorKindRequired);
        }
        let session_project = match session.query(DesignerQuery::Snapshot) {
            DesignerQueryResult::Snapshot(snapshot) => snapshot.design.project_id,
            DesignerQueryResult::Node(_)
            | DesignerQueryResult::Diagnostics(_)
            | DesignerQueryResult::History(_)
            | DesignerQueryResult::SessionState(_) => {
                unreachable!("DesignerSession::query returns the requested result variant")
            }
        };
        if session_project != *scope.project_id() {
            return Err(McpClientError::ProjectMismatch {
                scope_project: scope.project_id().clone(),
                session_project,
            });
        }
        Ok(Self {
            access: ScopedDesignerSession::new(session, scope),
            actor,
        })
    }

    /// The host-attributed MCP actor identity recorded in receipts.
    #[must_use]
    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    /// The immutable project scope attached to this client.
    #[must_use]
    pub fn scope(&self) -> &DesignerScope {
        self.access.scope()
    }

    /// Execute one scoped query.
    pub fn query(&self, query: DesignerQuery) -> Result<DesignerQueryResult, ScopeDenied> {
        self.access.query_scoped(query)
    }

    /// Submit one typed batch through the same command engine used by agents.
    ///
    /// The wire-provided actor is replaced with the host-supplied MCP actor
    /// before delegation, so callers cannot forge a human or agent audit
    /// identity.
    pub fn submit(
        &mut self,
        batch: CommandBatch,
    ) -> SessionFuture<'_, Result<CommandOutcome, ScopeDenied>> {
        self.access.submit_scoped(self.actor.clone(), batch)
    }

    /// Apply the current admitted undo group as an MCP-authored revision.
    pub fn undo(
        &mut self,
        operation: HistoryOperation,
    ) -> SessionFuture<'_, Result<CommandOutcome, ScopeDenied>> {
        self.access.undo_scoped(self.actor.clone(), operation)
    }

    /// Reapply the next admitted undo group as an MCP-authored revision.
    pub fn redo(
        &mut self,
        operation: HistoryOperation,
    ) -> SessionFuture<'_, Result<CommandOutcome, ScopeDenied>> {
        self.access.redo_scoped(self.actor.clone(), operation)
    }
}

impl<S: crate::DesignerSession> ScopedDesignerAccess for McpClient<S> {
    fn query_scoped(&self, query: DesignerQuery) -> Result<DesignerQueryResult, ScopeDenied> {
        self.query(query)
    }

    fn submit_scoped(
        &mut self,
        _actor: Actor,
        batch: CommandBatch,
    ) -> SessionFuture<'_, Result<CommandOutcome, ScopeDenied>> {
        self.submit(batch)
    }

    fn undo_scoped(
        &mut self,
        _actor: Actor,
        operation: HistoryOperation,
    ) -> SessionFuture<'_, Result<CommandOutcome, ScopeDenied>> {
        self.undo(operation)
    }

    fn redo_scoped(
        &mut self,
        _actor: Actor,
        operation: HistoryOperation,
    ) -> SessionFuture<'_, Result<CommandOutcome, ScopeDenied>> {
        self.redo(operation)
    }

    fn scope(&self) -> &DesignerScope {
        self.access.scope()
    }
}
