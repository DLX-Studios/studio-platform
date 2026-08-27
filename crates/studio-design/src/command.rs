//! Typed command algebra and history records.
#![allow(
    missing_docs,
    reason = "closed command fields mirror their documented command variant"
)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{
    Actor, DeletionTombstone, DesignNode, NodeId, NodeParent, OperationId, ProjectId,
    PropertyValue, RevisionId, TokenId, TokenOverride, TokenValue, UndoGroupId,
};

/// One atomic, actor-attributed mutation request.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandBatch {
    pub schema_version: u16,
    pub operation_id: OperationId,
    pub actor: Actor,
    pub project_id: ProjectId,
    pub base_revision: RevisionId,
    pub undo_group_id: UndoGroupId,
    pub undo_group_name: String,
    pub preconditions: Vec<CommandPrecondition>,
    pub commands: Vec<Command>,
}

/// A structural or property precondition evaluated before any command applies.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandPrecondition {
    NodeExists {
        node_id: NodeId,
    },
    NodeMissing {
        node_id: NodeId,
    },
    ParentEquals {
        node_id: NodeId,
        parent: NodeParent,
    },
    ChildIndexEquals {
        node_id: NodeId,
        index: usize,
    },
    PropertyEquals {
        node_id: NodeId,
        property: String,
        value: Option<PropertyValue>,
    },
    TokenExists {
        token_id: TokenId,
    },
    TokenMissing {
        token_id: TokenId,
    },
    TokenValueEquals {
        token_id: TokenId,
        value: TokenValue,
    },
}

/// Structural and property-edit commands implemented by ticket 37.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    InsertNode {
        parent: ParentPlacement,
        node: Box<DesignNode>,
    },
    MoveNode {
        node_id: NodeId,
        destination: ParentPlacement,
    },
    ReorderNode {
        node_id: NodeId,
        index: usize,
    },
    DuplicateNode {
        source_node_id: NodeId,
        destination: ParentPlacement,
        id_map: BTreeMap<NodeId, NodeId>,
    },
    DeleteNode {
        node_id: NodeId,
    },
    RestoreNode {
        tombstone: Box<DeletionTombstone>,
    },
    SetProperty {
        node_id: NodeId,
        property: String,
        value: Option<PropertyValue>,
    },
    RenameNode {
        node_id: NodeId,
        name: String,
    },
    /// Add a project-owned token while retaining the supplied identity.
    CreateToken {
        token: Box<crate::DesignToken>,
    },
    /// Change a token's shared value without changing its identity.
    EditToken {
        token_id: TokenId,
        value: TokenValue,
    },
    /// Bind a node property to shared token intent.
    ApplyToken {
        node_id: NodeId,
        property: String,
        token_id: TokenId,
    },
    /// Set a local value while retaining the shared token binding.
    OverrideToken {
        node_id: NodeId,
        property: String,
        value: TokenValue,
    },
    /// Clear a local value and reveal the shared token value.
    ClearTokenOverride {
        node_id: NodeId,
        property: String,
    },
    /// Rename a token; consumers continue to reference its stable identity.
    RenameToken {
        token_id: TokenId,
        name: String,
    },
    /// Delete a token. Referenced tokens require `confirm: true`.
    DeleteToken {
        token_id: TokenId,
        confirm: bool,
    },
    /// Internal/publicly serializable inverse for restoring an override.
    SetTokenOverride {
        node_id: NodeId,
        property: String,
        value: Option<TokenOverride>,
    },
    /// Inverse that restores both a binding and its local override atomically.
    RestoreTokenApplication {
        node_id: NodeId,
        property: String,
        property_value: Option<PropertyValue>,
        override_value: Option<TokenOverride>,
    },
}

/// A target parent and ordered child position.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ParentPlacement {
    pub parent: NodeParent,
    pub index: usize,
}

/// One applied command batch and its validated inverse command list.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppliedBatch {
    pub batch: CommandBatch,
    pub inverse_commands: Vec<Command>,
    pub committed_revision: RevisionId,
}

/// Contiguous batches sharing a user-visible named undo identity.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryEntry {
    pub undo_group_id: UndoGroupId,
    pub name: String,
    pub batches: Vec<AppliedBatch>,
}
