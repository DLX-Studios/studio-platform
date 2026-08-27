//! Typed command algebra and history records.
#![allow(
    missing_docs,
    reason = "closed command fields mirror their documented command variant"
)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{
    Actor, BindingId, CollectionId, ContentBinding, ContentCollection, ContentCollectionSchema,
    ContentFixture, ContentRecord, DeletionTombstone, DesignNode, FormDefinition, FormId, NodeId,
    NodeParent, OperationId, ProjectId, PropertyValue, RecordId, RevisionId, UndoGroupId,
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
    // --- Content collection CRUD (ticket 49) ---
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
    // --- Typed bindings (ticket 49) ---
    UpsertBinding {
        binding: ContentBinding,
    },
    RemoveBinding {
        binding_id: BindingId,
    },
    // --- Declarative forms (ticket 49) ---
    UpsertForm {
        form: FormDefinition,
    },
    RemoveForm {
        form_id: FormId,
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
