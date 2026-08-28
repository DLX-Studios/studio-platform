//! Typed command algebra and history records.
#![allow(
    missing_docs,
    reason = "closed command fields mirror their documented command variant"
)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::manipulation::CanvasRect;
use crate::model::{
    Actor, BindingId, BindingPath, CollectionId, ContentBinding, ContentCollection,
    ContentCollectionSchema, ContentFixture, ContentRecord, DeletionTombstone, DesignNode,
    DesignToken, FormDefinition, FormId, InstalledPlugin, Interaction, LayoutProperties, NodeId,
    NodeParent, OperationId, ProjectId, PropertyValue, RecordId, ResponsiveNodeOverride,
    ResponsiveVariant, ResponsiveVariantId, ReusableComposition, RevisionId, SettingKey,
    SettingValue, TokenId, TokenOverride, TokenValue, UndoGroupId,
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
    BreakpointPropertyEquals {
        node_id: NodeId,
        variant_id: ResponsiveVariantId,
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

/// Closed command families for structural editing and semantic design data.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    InsertNode {
        parent: ParentPlacement,
        node: Box<DesignNode>,
    },
    /// Add one screen record; its root is inserted by a sibling command in the same batch.
    InsertScreen {
        screen: Box<crate::Screen>,
        index: usize,
    },
    /// Remove a screen after its root has been removed.
    RemoveScreen {
        screen_id: crate::ScreenId,
    },
    /// Add, replace, or remove one design token.
    SetToken {
        token_id: TokenId,
        token: Option<DesignToken>,
    },
    /// Add, replace, or remove one generated settings value.
    SetSetting {
        key: SettingKey,
        value: Option<SettingValue>,
    },
    /// Add, replace, or remove one project plugin reference.
    SetPlugin {
        plugin_id: crate::PluginId,
        plugin: Option<InstalledPlugin>,
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
    SetLayout {
        node_id: NodeId,
        layout: LayoutProperties,
    },
    SetResponsiveLayout {
        node_id: NodeId,
        variant_id: ResponsiveVariantId,
        layout: LayoutProperties,
    },
    RemoveResponsiveLayout {
        node_id: NodeId,
        variant_id: ResponsiveVariantId,
    },
    /// Set or clear one sparse property override at a breakpoint.
    SetBreakpointProperty {
        node_id: NodeId,
        variant_id: ResponsiveVariantId,
        property: String,
        value: Option<PropertyValue>,
    },
    /// Replace or clear the complete typed override for a breakpoint.
    SetBreakpointOverride {
        node_id: NodeId,
        variant_id: ResponsiveVariantId,
        value: Option<Box<ResponsiveNodeOverride>>,
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
    /// Add a project-owned token while retaining the supplied identity.
    CreateToken {
        token: Box<DesignToken>,
    },
    /// Change a token's shared value without changing its identity.
    EditToken {
        token_id: TokenId,
        value: TokenValue,
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
    /// Delete a token, optionally confirming existing usages.
    DeleteToken {
        token_id: TokenId,
        confirm: bool,
    },
    /// Internal inverse for restoring a token override.
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
    CreateCollection {
        collection: ContentCollection,
    },
    UpdateCollectionSchema {
        collection_id: CollectionId,
        schema: ContentCollectionSchema,
    },
    DeleteCollection {
        collection_id: CollectionId,
    },
    CreateRecord {
        collection_id: CollectionId,
        record: ContentRecord,
    },
    UpdateRecord {
        collection_id: CollectionId,
        record_id: RecordId,
        values: BTreeMap<String, PropertyValue>,
    },
    DeleteRecord {
        collection_id: CollectionId,
        record_id: RecordId,
    },
    SetFixture {
        collection_id: CollectionId,
        fixture: ContentFixture,
    },
    UpsertBinding {
        binding: ContentBinding,
    },
    RemoveBinding {
        binding_id: BindingId,
    },
    UpsertForm {
        form: FormDefinition,
    },
    RemoveForm {
        form_id: FormId,
    },
    /// Set the editor canvas frame for a node.
    SetCanvasRect {
        node_id: NodeId,
        rect: CanvasRect,
    },
    /// Remove the editor canvas frame for a node.
    ClearCanvasRect {
        node_id: NodeId,
    },
}

impl Command {
    /// Build a typed base-layout edit for an inspector or canvas handle.
    #[must_use]
    pub fn set_layout(node_id: NodeId, layout: LayoutProperties) -> Self {
        Self::SetLayout { node_id, layout }
    }

    /// Build a typed breakpoint-layout edit for an inspector or canvas handle.
    #[must_use]
    pub fn set_responsive_layout(
        node_id: NodeId,
        variant_id: ResponsiveVariantId,
        layout: LayoutProperties,
    ) -> Self {
        Self::SetResponsiveLayout {
            node_id,
            variant_id,
            layout,
        }
    }
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
