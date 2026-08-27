//! Public `DesignerSession` query and command seam.
#![allow(
    missing_docs,
    reason = "closed query/result record fields mirror their documented domain type"
)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{CompareReport, DeviceProfileMatrix, PropertyProvenance};
use crate::{
    command::{CommandBatch, HistoryEntry},
    model::{
        Actor, DesignerDiagnostic, NodeId, OperationId, ProjectId, RevisionId, SelectionSnapshot,
        StudioDesignSnapshot, UndoGroupId,
    },
    persistence::{PersistenceError, SessionFuture},
};

/// The primary host-independent authoring interface.
pub trait DesignerSession: Send {
    /// Execute an immutable typed query against current session state.
    fn query(&self, query: DesignerQuery) -> DesignerQueryResult;

    /// Validate, durably commit, and publish one atomic command batch.
    fn submit(&mut self, batch: CommandBatch) -> SessionFuture<'_, CommandOutcome>;

    /// Apply the current named undo group's validated inverses as a new revision.
    fn undo(&mut self, operation: HistoryOperation) -> SessionFuture<'_, CommandOutcome>;

    /// Reapply the next named undo group's original commands as a new revision.
    fn redo(&mut self, operation: HistoryOperation) -> SessionFuture<'_, CommandOutcome>;

    /// Update ephemeral selection and presentation context owned by the session.
    fn update_context(&mut self, update: SessionContextUpdate) -> SessionStateSnapshot;
}

/// Closed query vocabulary shared by native UI, agents, MCP, tests, and builds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum DesignerQuery {
    Snapshot,
    Node {
        node_id: NodeId,
    },
    Diagnostics,
    History,
    SessionState,
    ResponsiveProfiles,
    ResponsiveInspector {
        node_id: NodeId,
        profile_id: crate::DeviceProfileId,
    },
    CompareProfiles {
        node_id: NodeId,
        left_profile_id: crate::DeviceProfileId,
        right_profile_id: crate::DeviceProfileId,
    },
}

/// Owned immutable result of one typed query.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum DesignerQueryResult {
    Snapshot(StudioDesignSnapshot),
    Node(Option<crate::DesignNode>),
    Diagnostics(Vec<DesignerDiagnostic>),
    History(HistorySnapshot),
    SessionState(SessionStateSnapshot),
    ResponsiveProfiles(DeviceProfileMatrix),
    ResponsiveInspector(Vec<PropertyProvenance>),
    ProfileComparison(CompareReport),
}

/// Result of a command, undo, or redo request.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(
    tag = "status",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum CommandOutcome {
    Accepted(CommandReceipt),
    Rejected(Vec<DesignerDiagnostic>),
    Conflict(BatchConflict),
    PersistenceFailed(PersistenceError),
}

/// Deterministic receipt for one accepted immutable revision.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandReceipt {
    pub operation_id: OperationId,
    pub project_id: ProjectId,
    pub base_revision: RevisionId,
    pub committed_revision: RevisionId,
    pub actor: Actor,
    pub undo_group_id: UndoGroupId,
    pub undo_group_name: String,
    pub command_count: usize,
}

/// Structured stale-base or failed-precondition result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BatchConflict {
    pub operation_id: OperationId,
    pub expected_revision: RevisionId,
    pub actual_revision: RevisionId,
    pub failed_precondition: Option<usize>,
    pub code: String,
    pub message: String,
}

/// Actor-attributed metadata for an undo or redo request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryOperation {
    pub operation_id: OperationId,
    pub actor: Actor,
    pub base_revision: RevisionId,
}

/// Immutable history list plus the next redo position.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HistorySnapshot {
    pub entries: Vec<HistoryEntry>,
    pub cursor: usize,
}

/// Session-owned state deliberately separated from the design document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionStateSnapshot {
    pub project_id: ProjectId,
    pub revision_id: RevisionId,
    pub selection: SelectionSnapshot,
    pub active_screen_id: Option<crate::ScreenId>,
    pub device_profile: Option<String>,
    pub tool: ToolKind,
    pub panel_state: BTreeMap<String, bool>,
    pub history_cursor: usize,
    pub canvas: CanvasStateSnapshot,
}

/// Ephemeral canvas view state preserved while switching profiles or views.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasStateSnapshot {
    pub zoom: String,
    pub pan_x: String,
    pub pan_y: String,
}

impl Default for CanvasStateSnapshot {
    fn default() -> Self {
        Self {
            zoom: "1.0".to_owned(),
            pan_x: "0.0".to_owned(),
            pan_y: "0.0".to_owned(),
        }
    }
}

/// Presentation-context fields a caller may update without mutating design source.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionContextUpdate {
    pub selection: Option<SelectionSnapshot>,
    pub active_screen_id: Option<Option<crate::ScreenId>>,
    pub device_profile: Option<Option<String>>,
    pub tool: Option<ToolKind>,
    pub panel_state: Option<BTreeMap<String, bool>>,
    pub canvas: Option<CanvasStateSnapshot>,
}

/// Closed set of primary authoring tools.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    #[default]
    Select,
    Insert,
    Pan,
    Text,
    Prototype,
}

/// Failure while constructing or opening a session.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SessionError {
    #[error("no durable Designer state exists for project {0}")]
    NotFound(ProjectId),
    #[error("durable Designer state failed validation: {0}")]
    InvalidState(String),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}
