//! Typed command algebra and history records.
#![allow(
    missing_docs,
    reason = "closed command fields mirror their documented command variant"
)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{
    Actor, BindingPath, DeletionTombstone, DesignNode, DesignToken, Interaction, NodeId,
    NodeParent, OperationId, ProjectId, PropertyValue, ResponsiveNodeOverride, ResponsiveVariant,
    ReusableComposition, RevisionId, TokenId, UndoGroupId,
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
}

/// Closed command families for structural editing and semantic design data.
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
    /// Define a named breakpoint/input profile.
    DefineResponsiveVariant {
        variant: ResponsiveVariant,
    },
    /// Replace a breakpoint/input profile while retaining its identity.
    UpdateResponsiveVariant {
        variant: ResponsiveVariant,
    },
    /// Remove a profile that is not referenced by a node override.
    RemoveResponsiveVariant {
        variant_id: crate::ResponsiveVariantId,
    },
    /// Set or clear a complete base-plus-profile override on one node.
    SetResponsiveOverride {
        node_id: NodeId,
        variant_id: crate::ResponsiveVariantId,
        value: Option<ResponsiveNodeOverride>,
    },
    /// Define a design token with a stable identity.
    DefineToken {
        token: DesignToken,
    },
    /// Replace a token's value and metadata while retaining its identity.
    UpdateToken {
        token: DesignToken,
    },
    /// Remove a token after all references have been cleared.
    RemoveToken {
        token_id: TokenId,
    },
    /// Apply a shared token reference to a node property.
    ApplyToken {
        node_id: NodeId,
        property: String,
        token_id: TokenId,
    },
    /// Set or clear a typed content binding on a node property.
    SetBinding {
        node_id: NodeId,
        property: String,
        binding: Option<BindingPath>,
    },
    /// Add one node-originated interaction to the declarative graph.
    DefineInteraction {
        interaction: Interaction,
    },
    /// Replace an interaction while retaining its identity.
    UpdateInteraction {
        interaction: Interaction,
    },
    /// Remove an interaction and its source-node attachment.
    RemoveInteraction {
        interaction_id: crate::InteractionId,
    },
    /// Register a project-owned reusable composition definition.
    DefineComposition {
        composition: ReusableComposition,
    },
    /// Replace a composition definition and propagate its version to instances.
    UpdateComposition {
        composition: ReusableComposition,
    },
    /// Remove a composition definition with no remaining instances.
    RemoveComposition {
        composition_id: crate::CompositionId,
    },
    /// Create a stable-ID instance of a reusable composition.
    InstantiateComposition {
        node_id: NodeId,
        name: String,
        parent: ParentPlacement,
        composition_id: crate::CompositionId,
        inputs: BTreeMap<String, PropertyValue>,
    },
    /// Set or clear one declared composition input on an instance.
    SetCompositionInput {
        node_id: NodeId,
        input: String,
        value: Option<PropertyValue>,
    },
    /// Set or clear one contract-admitted instance override.
    SetCompositionOverride {
        node_id: NodeId,
        input: String,
        value: Option<PropertyValue>,
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
