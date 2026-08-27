//! Validated command execution and immutable history implementation.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    command::{
        AppliedBatch, Command, CommandBatch, CommandPrecondition, HistoryEntry, ParentPlacement,
    },
    model::{
        Actor, DeletionTombstone, DesignNode, DesignNodeSource, DesignerDiagnostic,
        DiagnosticSeverity, InteractionAction, NodeId, NodeParent, OperationId, ProjectId,
        PropertyValue, RevisionId, RevisionMetadata, RevisionReason, STUDIO_DESIGN_SCHEMA_VERSION,
        SelectionSnapshot, StudioDesign, StudioDesignSnapshot, TombstoneReference, UndoGroupId,
    },
    recovery::RecoveryBundle,
    persistence::{DesignerPersistence, DesignerTransaction, DurableDesignerState},
    session::{
        BatchConflict, CommandOutcome, CommandReceipt, DesignerQuery, DesignerQueryResult,
        DesignerSession, HistoryOperation, HistorySnapshot, SessionContextUpdate, SessionError,
        SessionStateSnapshot, ToolKind,
    },
};

/// Default host-independent implementation of [`DesignerSession`].
pub struct DefaultDesignerSession<P> {
    persistence: P,
    state: DurableDesignerState,
    selection: SelectionSnapshot,
    active_screen_id: Option<crate::ScreenId>,
    device_profile: Option<String>,
    tool: ToolKind,
    panel_state: BTreeMap<String, bool>,
}

impl<P: DesignerPersistence> DefaultDesignerSession<P> {
    /// Consume the session and return its persistence adapter for orderly host shutdown.
    #[must_use]
    pub fn into_persistence(self) -> P {
        self.persistence
    }

    /// Validate and durably create a project before returning its session.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidState`] for an invalid source model or a
    /// persistence error when the initial revision cannot be made durable.
    pub async fn create(
        persistence: P,
        design: StudioDesign,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> Result<Self, SessionError> {
        let diagnostics = validate_design(&design);
        if !diagnostics.is_empty() {
            return Err(SessionError::InvalidState(join_diagnostics(&diagnostics)));
        }
        let active_screen_id = design.screen_order.first().cloned();
        let metadata = RevisionMetadata {
            id: RevisionId::INITIAL,
            parent_id: None,
            operation_id,
            actor,
            undo_group_id,
            undo_group_name: "Create project".to_owned(),
            reason: RevisionReason::Initial,
        };
        let snapshot = StudioDesignSnapshot {
            revision: metadata,
            design,
            tombstones: BTreeMap::new(),
        };
        let state = DurableDesignerState {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            current: snapshot.clone(),
            revisions: vec![snapshot],
            receipts: Vec::new(),
            history: Vec::new(),
            history_cursor: 0,
            diagnostics: Vec::new(),
        };
        let transaction = transaction_for(&state);
        persistence.commit(&transaction).await?;
        Ok(Self {
            persistence,
            state,
            selection: SelectionSnapshot::default(),
            active_screen_id,
            device_profile: None,
            tool: ToolKind::default(),
            panel_state: BTreeMap::new(),
        })
    }

    /// Reopen the last completely durable project revision.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::NotFound`] when no project record exists and
    /// [`SessionError::InvalidState`] when the durable domain record is invalid.
    pub async fn open(persistence: P, project_id: &ProjectId) -> Result<Self, SessionError> {
        let state = persistence
            .load(project_id)
            .await?
            .ok_or_else(|| SessionError::NotFound(project_id.clone()))?;
        validate_durable_state(&state, project_id)?;
        let active_screen_id = state.current.design.screen_order.first().cloned();
        Ok(Self {
            persistence,
            state,
            selection: SelectionSnapshot::default(),
            active_screen_id,
            device_profile: None,
            tool: ToolKind::default(),
            panel_state: BTreeMap::new(),
        })
    }

    /// Rebuild a session from a logical snapshot and ordered operation journal.
    ///
    /// The snapshot is committed as the recovery base, then each journal batch
    /// is submitted through the same validation and persistence path as an
    /// ordinary edit.  A journal mismatch therefore leaves no partially
    /// restored session visible to the caller.
    pub async fn restore_from_recovery(
        persistence: P,
        bundle: &RecoveryBundle,
    ) -> Result<Self, SessionError> {
        bundle
            .validate()
            .map_err(|error| SessionError::InvalidState(error.to_string()))?;
        let base_snapshot = bundle.snapshot.revision.clone();
        let diagnostics = validate_design(&base_snapshot.design);
        if !diagnostics.is_empty() {
            return Err(SessionError::InvalidState(join_diagnostics(&diagnostics)));
        }
        let state = DurableDesignerState {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            current: base_snapshot.clone(),
            revisions: vec![base_snapshot.clone()],
            receipts: Vec::new(),
            history: Vec::new(),
            history_cursor: 0,
            diagnostics: Vec::new(),
        };
        let transaction = transaction_for(&state);
        persistence.commit(&transaction).await?;
        let mut session = Self {
            persistence,
            state,
            selection: SelectionSnapshot::default(),
            active_screen_id: session_active_screen(&base_snapshot),
            device_profile: None,
            tool: ToolKind::default(),
            panel_state: BTreeMap::new(),
        };
        for entry in &bundle.journal {
            let outcome = session.submit_batch(entry.batch.clone()).await;
            match outcome {
                CommandOutcome::Accepted(receipt)
                    if receipt.committed_revision == entry.committed_revision => {}
                CommandOutcome::Accepted(_) => {
                    return Err(SessionError::InvalidState(
                        "recovery journal revision does not match replay".to_owned(),
                    ));
                }
                CommandOutcome::Rejected(_) | CommandOutcome::Conflict(_) => {
                    return Err(SessionError::InvalidState(
                        "recovery journal operation could not be replayed".to_owned(),
                    ));
                }
                CommandOutcome::PersistenceFailed(error) => {
                    return Err(SessionError::Persistence(error));
                }
            }
        }
        Ok(session)
    }

    #[allow(clippy::too_many_lines)]
    async fn submit_batch(&mut self, batch: CommandBatch) -> CommandOutcome {
        if let Some(receipt) = self
            .state
            .receipts
            .iter()
            .find(|receipt| receipt.operation_id == batch.operation_id)
        {
            return CommandOutcome::Accepted(receipt.clone());
        }
        let current_revision = self.state.current.revision.id;
        if batch.base_revision != current_revision {
            return CommandOutcome::Conflict(stale_conflict(
                batch.operation_id,
                batch.base_revision,
                current_revision,
            ));
        }
        let batch_diagnostics = validate_batch_shape(&batch, &self.state.current.design);
        if !batch_diagnostics.is_empty() {
            return CommandOutcome::Rejected(batch_diagnostics);
        }
        if let Some(index) = failed_precondition(&self.state.current.design, &batch.preconditions) {
            return CommandOutcome::Conflict(precondition_conflict(
                batch.operation_id,
                current_revision,
                index,
            ));
        }

        let mut snapshot = self.state.current.clone();
        let mut inverse_commands = match apply_commands(&mut snapshot, &batch.commands) {
            Ok(commands) => commands,
            Err(diagnostics) => return CommandOutcome::Rejected(diagnostics),
        };
        let diagnostics = validate_design(&snapshot.design);
        if !diagnostics.is_empty() {
            return CommandOutcome::Rejected(diagnostics);
        }
        let Some(committed_revision) = current_revision.checked_next() else {
            return CommandOutcome::Rejected(vec![diagnostic(
                "DESIGN_REVISION_EXHAUSTED",
                "the project revision sequence is exhausted",
                None,
            )]);
        };
        finalize_new_tombstones(&mut snapshot, committed_revision);
        finalize_inverse_tombstones(&mut inverse_commands, committed_revision);
        snapshot.revision = RevisionMetadata {
            id: committed_revision,
            parent_id: Some(current_revision),
            operation_id: batch.operation_id.clone(),
            actor: batch.actor.clone(),
            undo_group_id: batch.undo_group_id.clone(),
            undo_group_name: batch.undo_group_name.clone(),
            reason: RevisionReason::Command,
        };
        let receipt = receipt_for_batch(&batch, committed_revision);
        let mut candidate = self.state.clone();
        candidate.current = snapshot.clone();
        candidate.revisions.push(snapshot);
        candidate.diagnostics = reference_diagnostics(&candidate.current);
        candidate.receipts.push(receipt.clone());
        if candidate.history_cursor < candidate.history.len() {
            candidate.history.truncate(candidate.history_cursor);
        }
        let applied = AppliedBatch {
            batch: batch.clone(),
            inverse_commands,
            committed_revision,
        };
        if candidate
            .history
            .last()
            .is_some_and(|entry| entry.undo_group_id == batch.undo_group_id)
        {
            candidate
                .history
                .last_mut()
                .expect("history has a last entry")
                .batches
                .push(applied);
        } else {
            candidate.history.push(HistoryEntry {
                undo_group_id: batch.undo_group_id,
                name: batch.undo_group_name,
                batches: vec![applied],
            });
        }
        candidate.history_cursor = candidate.history.len();
        self.commit_candidate(candidate, receipt).await
    }

    #[allow(clippy::too_many_lines)]
    async fn apply_history(
        &mut self,
        operation: HistoryOperation,
        reason: RevisionReason,
    ) -> CommandOutcome {
        if let Some(receipt) = self
            .state
            .receipts
            .iter()
            .find(|receipt| receipt.operation_id == operation.operation_id)
        {
            return CommandOutcome::Accepted(receipt.clone());
        }
        let current_revision = self.state.current.revision.id;
        if operation.base_revision != current_revision {
            return CommandOutcome::Conflict(stale_conflict(
                operation.operation_id,
                operation.base_revision,
                current_revision,
            ));
        }
        let entry_index = match reason {
            RevisionReason::Undo => self.state.history_cursor.checked_sub(1),
            RevisionReason::Redo => (self.state.history_cursor < self.state.history.len())
                .then_some(self.state.history_cursor),
            RevisionReason::Initial | RevisionReason::Command => None,
        };
        let Some(entry_index) = entry_index else {
            let action = if reason == RevisionReason::Undo {
                "undo"
            } else {
                "redo"
            };
            return CommandOutcome::Rejected(vec![diagnostic(
                "DESIGN_HISTORY_UNAVAILABLE",
                format!("there is no named command group to {action}"),
                None,
            )]);
        };
        let entry = self.state.history[entry_index].clone();
        let commands = match reason {
            RevisionReason::Undo => entry
                .batches
                .iter()
                .rev()
                .flat_map(|batch| batch.inverse_commands.clone())
                .collect::<Vec<_>>(),
            RevisionReason::Redo => {
                let mut commands = Vec::new();
                let mut projected = self.state.current.clone();
                for batch in &entry.batches {
                    if let Some(index) =
                        failed_precondition(&projected.design, &batch.batch.preconditions)
                    {
                        return CommandOutcome::Conflict(precondition_conflict(
                            operation.operation_id,
                            current_revision,
                            index,
                        ));
                    }
                    commands.extend(batch.batch.commands.clone());
                    if let Err(diagnostics) = apply_commands(&mut projected, &batch.batch.commands)
                    {
                        return CommandOutcome::Rejected(diagnostics);
                    }
                }
                commands
            }
            RevisionReason::Initial | RevisionReason::Command => Vec::new(),
        };

        let mut snapshot = self.state.current.clone();
        if let Err(diagnostics) = apply_commands(&mut snapshot, &commands) {
            return CommandOutcome::Rejected(diagnostics);
        }
        let diagnostics = validate_design(&snapshot.design);
        if !diagnostics.is_empty() {
            return CommandOutcome::Rejected(diagnostics);
        }
        let Some(committed_revision) = current_revision.checked_next() else {
            return CommandOutcome::Rejected(vec![diagnostic(
                "DESIGN_REVISION_EXHAUSTED",
                "the project revision sequence is exhausted",
                None,
            )]);
        };
        finalize_new_tombstones(&mut snapshot, committed_revision);
        snapshot.revision = RevisionMetadata {
            id: committed_revision,
            parent_id: Some(current_revision),
            operation_id: operation.operation_id.clone(),
            actor: operation.actor.clone(),
            undo_group_id: entry.undo_group_id.clone(),
            undo_group_name: entry.name.clone(),
            reason,
        };
        let receipt = CommandReceipt {
            operation_id: operation.operation_id,
            project_id: snapshot.design.project_id.clone(),
            base_revision: current_revision,
            committed_revision,
            actor: operation.actor,
            undo_group_id: entry.undo_group_id,
            undo_group_name: entry.name,
            command_count: commands.len(),
        };
        let mut candidate = self.state.clone();
        candidate.current = snapshot.clone();
        candidate.revisions.push(snapshot);
        candidate.receipts.push(receipt.clone());
        candidate.diagnostics = reference_diagnostics(&candidate.current);
        match reason {
            RevisionReason::Undo => candidate.history_cursor -= 1,
            RevisionReason::Redo => candidate.history_cursor += 1,
            RevisionReason::Initial | RevisionReason::Command => {}
        }
        self.commit_candidate(candidate, receipt).await
    }

    async fn commit_candidate(
        &mut self,
        candidate: DurableDesignerState,
        receipt: CommandReceipt,
    ) -> CommandOutcome {
        let transaction = transaction_for(&candidate);
        if let Err(error) = self.persistence.commit(&transaction).await {
            return CommandOutcome::PersistenceFailed(error);
        }
        self.state = candidate;
        self.selection
            .node_ids
            .retain(|node_id| self.state.current.design.nodes.contains_key(node_id));
        if self
            .selection
            .primary
            .as_ref()
            .is_some_and(|node_id| !self.state.current.design.nodes.contains_key(node_id))
        {
            self.selection.primary = None;
        }
        CommandOutcome::Accepted(receipt)
    }

    fn session_state(&self) -> SessionStateSnapshot {
        SessionStateSnapshot {
            project_id: self.state.current.design.project_id.clone(),
            revision_id: self.state.current.revision.id,
            selection: self.selection.clone(),
            active_screen_id: self.active_screen_id.clone(),
            device_profile: self.device_profile.clone(),
            tool: self.tool,
            panel_state: self.panel_state.clone(),
            history_cursor: self.state.history_cursor,
        }
    }
}

impl<P: DesignerPersistence> DesignerSession for DefaultDesignerSession<P> {
    fn query(&self, query: DesignerQuery) -> DesignerQueryResult {
        match query {
            DesignerQuery::Snapshot => DesignerQueryResult::Snapshot(self.state.current.clone()),
            DesignerQuery::Node { node_id } => {
                DesignerQueryResult::Node(self.state.current.design.nodes.get(&node_id).cloned())
            }
            DesignerQuery::Diagnostics => {
                DesignerQueryResult::Diagnostics(self.state.diagnostics.clone())
            }
            DesignerQuery::History => DesignerQueryResult::History(HistorySnapshot {
                entries: self.state.history.clone(),
                cursor: self.state.history_cursor,
            }),
            DesignerQuery::SessionState => DesignerQueryResult::SessionState(self.session_state()),
        }
    }

    fn submit(&mut self, batch: CommandBatch) -> crate::SessionFuture<'_, CommandOutcome> {
        Box::pin(async move { self.submit_batch(batch).await })
    }

    fn undo(&mut self, operation: HistoryOperation) -> crate::SessionFuture<'_, CommandOutcome> {
        Box::pin(async move { self.apply_history(operation, RevisionReason::Undo).await })
    }

    fn redo(&mut self, operation: HistoryOperation) -> crate::SessionFuture<'_, CommandOutcome> {
        Box::pin(async move { self.apply_history(operation, RevisionReason::Redo).await })
    }

    fn update_context(&mut self, update: SessionContextUpdate) -> SessionStateSnapshot {
        if let Some(selection) = update.selection {
            self.selection = selection;
        }
        if let Some(active_screen_id) = update.active_screen_id {
            self.active_screen_id = active_screen_id;
        }
        if let Some(device_profile) = update.device_profile {
            self.device_profile = device_profile;
        }
        if let Some(tool) = update.tool {
            self.tool = tool;
        }
        if let Some(panel_state) = update.panel_state {
            self.panel_state = panel_state;
        }
        self.session_state()
    }
}

fn transaction_for(state: &DurableDesignerState) -> DesignerTransaction {
    DesignerTransaction {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        project_id: state.current.design.project_id.clone(),
        sequence: state.current.revision.id.get(),
        state: state.clone(),
    }
}

fn session_active_screen(snapshot: &StudioDesignSnapshot) -> Option<crate::ScreenId> {
    snapshot.design.screen_order.first().cloned()
}

fn validate_durable_state(
    state: &DurableDesignerState,
    project_id: &ProjectId,
) -> Result<(), SessionError> {
    if state.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
        || state.current.design.project_id != *project_id
        || state.history_cursor > state.history.len()
        || state.revisions.last() != Some(&state.current)
        || state
            .revisions
            .windows(2)
            .any(|pair| pair[0].revision.id >= pair[1].revision.id)
    {
        return Err(SessionError::InvalidState(
            "durable revision/history metadata is inconsistent".to_owned(),
        ));
    }
    let diagnostics = validate_design(&state.current.design);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(SessionError::InvalidState(join_diagnostics(&diagnostics)))
    }
}

fn validate_batch_shape(batch: &CommandBatch, design: &StudioDesign) -> Vec<DesignerDiagnostic> {
    let mut diagnostics = Vec::new();
    if batch.schema_version != STUDIO_DESIGN_SCHEMA_VERSION {
        diagnostics.push(diagnostic(
            "DESIGN_SCHEMA_UNSUPPORTED",
            "the command batch schema version is unsupported",
            None,
        ));
    }
    if batch.project_id != design.project_id {
        diagnostics.push(diagnostic(
            "DESIGN_PROJECT_MISMATCH",
            "the command batch targets a different project",
            None,
        ));
    }
    if batch.commands.is_empty() {
        diagnostics.push(diagnostic(
            "DESIGN_BATCH_EMPTY",
            "an atomic command batch must contain at least one command",
            None,
        ));
    }
    if batch.undo_group_name.trim().is_empty() || batch.undo_group_name.len() > 256 {
        diagnostics.push(diagnostic(
            "DESIGN_UNDO_NAME_INVALID",
            "an undo group name must contain 1..=256 bytes",
            None,
        ));
    }
    diagnostics
}

fn failed_precondition(design: &StudioDesign, conditions: &[CommandPrecondition]) -> Option<usize> {
    conditions.iter().position(|condition| match condition {
        CommandPrecondition::NodeExists { node_id } => !design.nodes.contains_key(node_id),
        CommandPrecondition::NodeMissing { node_id } => design.nodes.contains_key(node_id),
        CommandPrecondition::ParentEquals { node_id, parent } => {
            design.parents.get(node_id) != Some(parent)
        }
        CommandPrecondition::ChildIndexEquals { node_id, index } => {
            child_index(design, node_id) != Some(*index)
        }
        CommandPrecondition::PropertyEquals {
            node_id,
            property,
            value,
        } => design
            .nodes
            .get(node_id)
            .is_none_or(|node| node.properties.get(property) != value.as_ref()),
    })
}

fn apply_commands(
    snapshot: &mut StudioDesignSnapshot,
    commands: &[Command],
) -> Result<Vec<Command>, Vec<DesignerDiagnostic>> {
    let mut inverses = Vec::with_capacity(commands.len());
    for command in commands {
        match apply_command(snapshot, command) {
            Ok(inverse) => inverses.push(inverse),
            Err(diagnostic) => return Err(vec![diagnostic]),
        }
    }
    inverses.reverse();
    Ok(inverses)
}

fn apply_command(
    snapshot: &mut StudioDesignSnapshot,
    command: &Command,
) -> Result<Command, DesignerDiagnostic> {
    match command {
        Command::InsertNode { parent, node } => insert_node(snapshot, parent, node),
        Command::MoveNode {
            node_id,
            destination,
        } => move_node(snapshot, node_id, destination),
        Command::ReorderNode { node_id, index } => reorder_node(snapshot, node_id, *index),
        Command::DuplicateNode {
            source_node_id,
            destination,
            id_map,
        } => duplicate_node(snapshot, source_node_id, destination, id_map),
        Command::DeleteNode { node_id } => delete_node(snapshot, node_id),
        Command::RestoreNode { tombstone } => restore_node(snapshot, tombstone),
        Command::SetProperty {
            node_id,
            property,
            value,
        } => set_property(snapshot, node_id, property, value.as_ref()),
        Command::RenameNode { node_id, name } => rename_node(snapshot, node_id, name),
    }
}

fn insert_node(
    snapshot: &mut StudioDesignSnapshot,
    placement: &ParentPlacement,
    node: &DesignNode,
) -> Result<Command, DesignerDiagnostic> {
    if snapshot.design.nodes.contains_key(&node.id) {
        return Err(node_diagnostic(
            "DESIGN_NODE_EXISTS",
            "the inserted node identity already exists",
            &node.id,
        ));
    }
    if !node.children.is_empty() {
        return Err(node_diagnostic(
            "DESIGN_INSERT_CHILDREN",
            "insert_node accepts one childless node; insert descendants in the same batch",
            &node.id,
        ));
    }
    let reclaim = reclaimable_tombstone(snapshot, node);
    if id_is_tombstoned(snapshot, &node.id) && reclaim.is_none() {
        return Err(node_diagnostic(
            "DESIGN_ID_TOMBSTONED",
            "the inserted identity belongs to a different deletion tombstone",
            &node.id,
        ));
    }
    insert_child(&mut snapshot.design, placement, node.id.clone())?;
    snapshot
        .design
        .parents
        .insert(node.id.clone(), placement.parent.clone());
    snapshot.design.nodes.insert(node.id.clone(), node.clone());
    if let Some(root_id) = reclaim {
        snapshot.tombstones.remove(&root_id);
    }
    Ok(Command::DeleteNode {
        node_id: node.id.clone(),
    })
}

fn move_node(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    destination: &ParentPlacement,
) -> Result<Command, DesignerDiagnostic> {
    let old_parent = snapshot
        .design
        .parents
        .get(node_id)
        .cloned()
        .ok_or_else(|| missing_node(node_id))?;
    let old_index = child_index(&snapshot.design, node_id)
        .ok_or_else(|| invalid_parent(node_id, "the parent index does not contain the node"))?;
    require_nested_parent(node_id, &old_parent)?;
    require_nested_parent(node_id, &destination.parent)?;
    if destination_contains(&snapshot.design, node_id, &destination.parent) {
        return Err(node_diagnostic(
            "DESIGN_MOVE_CYCLE",
            "a node cannot move under itself or one of its descendants",
            node_id,
        ));
    }
    remove_child(&mut snapshot.design, node_id, &old_parent)?;
    if let Err(error) = insert_child(&mut snapshot.design, destination, node_id.clone()) {
        let rollback = ParentPlacement {
            parent: old_parent.clone(),
            index: old_index,
        };
        insert_child(&mut snapshot.design, &rollback, node_id.clone())
            .expect("validated original parent accepts rollback");
        return Err(error);
    }
    snapshot
        .design
        .parents
        .insert(node_id.clone(), destination.parent.clone());
    Ok(Command::MoveNode {
        node_id: node_id.clone(),
        destination: ParentPlacement {
            parent: old_parent,
            index: old_index,
        },
    })
}

fn reorder_node(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    index: usize,
) -> Result<Command, DesignerDiagnostic> {
    let parent = snapshot
        .design
        .parents
        .get(node_id)
        .cloned()
        .ok_or_else(|| missing_node(node_id))?;
    require_nested_parent(node_id, &parent)?;
    let old_index = child_index(&snapshot.design, node_id)
        .ok_or_else(|| invalid_parent(node_id, "the parent index does not contain the node"))?;
    remove_child(&mut snapshot.design, node_id, &parent)?;
    let placement = ParentPlacement {
        parent: parent.clone(),
        index,
    };
    if let Err(error) = insert_child(&mut snapshot.design, &placement, node_id.clone()) {
        let rollback = ParentPlacement {
            parent,
            index: old_index,
        };
        insert_child(&mut snapshot.design, &rollback, node_id.clone())
            .expect("validated original parent accepts rollback");
        return Err(error);
    }
    Ok(Command::ReorderNode {
        node_id: node_id.clone(),
        index: old_index,
    })
}

#[allow(clippy::too_many_lines)]
fn duplicate_node(
    snapshot: &mut StudioDesignSnapshot,
    source_node_id: &NodeId,
    destination: &ParentPlacement,
    id_map: &BTreeMap<NodeId, NodeId>,
) -> Result<Command, DesignerDiagnostic> {
    let source_ids = subtree_ids(&snapshot.design, source_node_id)?;
    let source_set = source_ids.iter().cloned().collect::<BTreeSet<_>>();
    if id_map.keys().cloned().collect::<BTreeSet<_>>() != source_set
        || id_map.values().cloned().collect::<BTreeSet<_>>().len() != id_map.len()
    {
        return Err(node_diagnostic(
            "DESIGN_DUPLICATE_ID_MAP",
            "duplicate_node requires one unique destination identity for every subtree node",
            source_node_id,
        ));
    }
    let mut cloned_nodes = BTreeMap::new();
    for source_id in &source_ids {
        let new_id = id_map
            .get(source_id)
            .expect("validated identity map contains every source")
            .clone();
        if snapshot.design.nodes.contains_key(&new_id) {
            return Err(node_diagnostic(
                "DESIGN_NODE_EXISTS",
                "a duplicated node identity already exists",
                &new_id,
            ));
        }
        let mut node = snapshot
            .design
            .nodes
            .get(source_id)
            .expect("collected subtree identity exists")
            .clone();
        node.id = new_id.clone();
        node.children = node
            .children
            .iter()
            .map(|child| id_map.get(child).expect("child is in subtree").clone())
            .collect();
        node.interaction_ids.clear();
        remap_node_references(&mut node, id_map);
        cloned_nodes.insert(new_id, node);
    }
    let new_root_id = id_map
        .get(source_node_id)
        .expect("validated root mapping exists")
        .clone();
    let reclaim = snapshot
        .tombstones
        .get(&new_root_id)
        .filter(|tombstone| tombstone.nodes == cloned_nodes)
        .map(|tombstone| tombstone.root_node_id.clone());
    if cloned_nodes
        .keys()
        .any(|node_id| id_is_tombstoned(snapshot, node_id))
        && reclaim.is_none()
    {
        return Err(node_diagnostic(
            "DESIGN_ID_TOMBSTONED",
            "a duplicated identity belongs to a different deletion tombstone",
            &new_root_id,
        ));
    }
    insert_child(&mut snapshot.design, destination, new_root_id.clone())?;
    for (source_id, new_id) in id_map {
        let parent = if source_id == source_node_id {
            destination.parent.clone()
        } else {
            match snapshot
                .design
                .parents
                .get(source_id)
                .expect("source subtree parent exists")
            {
                NodeParent::Node { node_id } => NodeParent::Node {
                    node_id: id_map
                        .get(node_id)
                        .expect("subtree parent is mapped")
                        .clone(),
                },
                NodeParent::Screen { .. } | NodeParent::Composition { .. } => {
                    return Err(invalid_parent(
                        source_node_id,
                        "only the duplicate root may have a root owner",
                    ));
                }
            }
        };
        snapshot.design.parents.insert(new_id.clone(), parent);
    }
    snapshot.design.nodes.extend(cloned_nodes);
    if let Some(root_id) = reclaim {
        snapshot.tombstones.remove(&root_id);
    }
    Ok(Command::DeleteNode {
        node_id: new_root_id,
    })
}

fn delete_node(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
) -> Result<Command, DesignerDiagnostic> {
    let parent = snapshot
        .design
        .parents
        .get(node_id)
        .cloned()
        .ok_or_else(|| missing_node(node_id))?;
    require_nested_parent(node_id, &parent)?;
    let detached_index = child_index(&snapshot.design, node_id)
        .ok_or_else(|| invalid_parent(node_id, "the parent index does not contain the node"))?;
    let ids = subtree_ids(&snapshot.design, node_id)?;
    let id_set = ids.iter().cloned().collect::<BTreeSet<_>>();
    let references = collect_references(&snapshot.design, &id_set);
    let mut nodes = BTreeMap::new();
    let mut parents = BTreeMap::new();
    for id in &ids {
        nodes.insert(
            id.clone(),
            snapshot
                .design
                .nodes
                .get(id)
                .expect("collected subtree node exists")
                .clone(),
        );
        parents.insert(
            id.clone(),
            snapshot
                .design
                .parents
                .get(id)
                .expect("collected subtree parent exists")
                .clone(),
        );
    }
    remove_child(&mut snapshot.design, node_id, &parent)?;
    for id in &ids {
        snapshot.design.nodes.remove(id);
        snapshot.design.parents.remove(id);
    }
    let tombstone = DeletionTombstone {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        root_node_id: node_id.clone(),
        nodes,
        parents,
        detached_from: parent,
        detached_index,
        deleted_in_revision: None,
        references,
    };
    snapshot
        .tombstones
        .insert(node_id.clone(), tombstone.clone());
    Ok(Command::RestoreNode {
        tombstone: Box::new(tombstone),
    })
}

fn restore_node(
    snapshot: &mut StudioDesignSnapshot,
    tombstone: &DeletionTombstone,
) -> Result<Command, DesignerDiagnostic> {
    if !snapshot
        .tombstones
        .get(&tombstone.root_node_id)
        .is_some_and(|current| same_tombstone_content(current, tombstone))
    {
        return Err(node_diagnostic(
            "DESIGN_TOMBSTONE_MISMATCH",
            "restore_node requires the exact current deletion tombstone",
            &tombstone.root_node_id,
        ));
    }
    if tombstone
        .nodes
        .keys()
        .any(|node_id| snapshot.design.nodes.contains_key(node_id))
    {
        return Err(node_diagnostic(
            "DESIGN_RESTORE_COLLISION",
            "a restored node identity already exists",
            &tombstone.root_node_id,
        ));
    }
    let placement = ParentPlacement {
        parent: tombstone.detached_from.clone(),
        index: tombstone.detached_index,
    };
    insert_child(
        &mut snapshot.design,
        &placement,
        tombstone.root_node_id.clone(),
    )?;
    snapshot.design.nodes.extend(tombstone.nodes.clone());
    snapshot.design.parents.extend(tombstone.parents.clone());
    snapshot.tombstones.remove(&tombstone.root_node_id);
    Ok(Command::DeleteNode {
        node_id: tombstone.root_node_id.clone(),
    })
}

fn set_property(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    property: &str,
    value: Option<&PropertyValue>,
) -> Result<Command, DesignerDiagnostic> {
    if !valid_field_name(property) {
        return Err(node_diagnostic(
            "DESIGN_PROPERTY_INVALID",
            "a property name must contain 1..=128 safe bytes",
            node_id,
        ));
    }
    let node = snapshot
        .design
        .nodes
        .get_mut(node_id)
        .ok_or_else(|| missing_node(node_id))?;
    let prior = match value {
        Some(value) => node.properties.insert(property.to_owned(), value.clone()),
        None => node.properties.remove(property),
    };
    Ok(Command::SetProperty {
        node_id: node_id.clone(),
        property: property.to_owned(),
        value: prior,
    })
}

fn rename_node(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    name: &str,
) -> Result<Command, DesignerDiagnostic> {
    if name.trim().is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err(node_diagnostic(
            "DESIGN_NODE_NAME_INVALID",
            "a node name must contain 1..=256 safe bytes",
            node_id,
        ));
    }
    let node = snapshot
        .design
        .nodes
        .get_mut(node_id)
        .ok_or_else(|| missing_node(node_id))?;
    let prior = std::mem::replace(&mut node.name, name.to_owned());
    Ok(Command::RenameNode {
        node_id: node_id.clone(),
        name: prior,
    })
}

fn insert_child(
    design: &mut StudioDesign,
    placement: &ParentPlacement,
    child: NodeId,
) -> Result<(), DesignerDiagnostic> {
    let NodeParent::Node { node_id } = &placement.parent else {
        return Err(invalid_parent(
            &child,
            "screen and composition roots cannot be replaced by structural commands",
        ));
    };
    let parent = design
        .nodes
        .get_mut(node_id)
        .ok_or_else(|| missing_node(node_id))?;
    if placement.index > parent.children.len() {
        return Err(node_diagnostic(
            "DESIGN_CHILD_INDEX_INVALID",
            "the requested child index is outside the destination parent",
            &child,
        ));
    }
    parent.children.insert(placement.index, child);
    Ok(())
}

fn remove_child(
    design: &mut StudioDesign,
    node_id: &NodeId,
    parent: &NodeParent,
) -> Result<(), DesignerDiagnostic> {
    let NodeParent::Node { node_id: parent_id } = parent else {
        return Err(invalid_parent(
            node_id,
            "screen and composition roots cannot be removed",
        ));
    };
    let parent_node = design
        .nodes
        .get_mut(parent_id)
        .ok_or_else(|| missing_node(parent_id))?;
    let Some(index) = parent_node
        .children
        .iter()
        .position(|child| child == node_id)
    else {
        return Err(invalid_parent(
            node_id,
            "the indexed parent does not contain the node",
        ));
    };
    parent_node.children.remove(index);
    Ok(())
}

fn require_nested_parent(node_id: &NodeId, parent: &NodeParent) -> Result<(), DesignerDiagnostic> {
    if matches!(parent, NodeParent::Node { .. }) {
        Ok(())
    } else {
        Err(invalid_parent(
            node_id,
            "screen and composition roots are not movable or deletable",
        ))
    }
}

fn child_index(design: &StudioDesign, node_id: &NodeId) -> Option<usize> {
    match design.parents.get(node_id)? {
        NodeParent::Node { node_id: parent_id } => design
            .nodes
            .get(parent_id)?
            .children
            .iter()
            .position(|child| child == node_id),
        NodeParent::Screen { screen_id } => design
            .screens
            .get(screen_id)
            .is_some_and(|screen| screen.root_node_id == *node_id)
            .then_some(0),
        NodeParent::Composition { composition_id } => design
            .compositions
            .get(composition_id)
            .is_some_and(|composition| composition.root_node_id == *node_id)
            .then_some(0),
    }
}

fn destination_contains(design: &StudioDesign, node_id: &NodeId, parent: &NodeParent) -> bool {
    let mut cursor = match parent {
        NodeParent::Node { node_id } => Some(node_id),
        NodeParent::Screen { .. } | NodeParent::Composition { .. } => None,
    };
    while let Some(current) = cursor {
        if current == node_id {
            return true;
        }
        cursor = match design.parents.get(current) {
            Some(NodeParent::Node { node_id }) => Some(node_id),
            Some(NodeParent::Screen { .. } | NodeParent::Composition { .. }) | None => None,
        };
    }
    false
}

fn subtree_ids(design: &StudioDesign, root: &NodeId) -> Result<Vec<NodeId>, DesignerDiagnostic> {
    if !design.nodes.contains_key(root) {
        return Err(missing_node(root));
    }
    let mut ids = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(node_id) = stack.pop() {
        let node = design
            .nodes
            .get(&node_id)
            .ok_or_else(|| missing_node(&node_id))?;
        ids.push(node_id);
        stack.extend(node.children.iter().rev().cloned());
    }
    Ok(ids)
}

fn remap_node_references(node: &mut DesignNode, id_map: &BTreeMap<NodeId, NodeId>) {
    for value in node.properties.values_mut() {
        remap_property_value(value, id_map);
    }
    for override_value in node.responsive_overrides.values_mut() {
        for value in override_value.properties.values_mut() {
            remap_property_value(value, id_map);
        }
    }
    if let DesignNodeSource::CompositionInstance {
        inputs,
        admitted_overrides,
        ..
    } = &mut node.source
    {
        for value in inputs.values_mut().chain(admitted_overrides.values_mut()) {
            remap_property_value(value, id_map);
        }
    }
}

fn remap_property_value(value: &mut PropertyValue, id_map: &BTreeMap<NodeId, NodeId>) {
    match value {
        PropertyValue::Node(node_id) => {
            if let Some(replacement) = id_map.get(node_id) {
                *node_id = replacement.clone();
            }
        }
        PropertyValue::List(values) => {
            for value in values {
                remap_property_value(value, id_map);
            }
        }
        PropertyValue::String(_)
        | PropertyValue::Boolean(_)
        | PropertyValue::Integer(_)
        | PropertyValue::Decimal(_)
        | PropertyValue::Length(_)
        | PropertyValue::Color(_)
        | PropertyValue::Token(_)
        | PropertyValue::Binding(_)
        | PropertyValue::Asset(_) => {}
    }
}

fn reclaimable_tombstone(snapshot: &StudioDesignSnapshot, node: &DesignNode) -> Option<NodeId> {
    snapshot
        .tombstones
        .values()
        .find(|tombstone| {
            tombstone.root_node_id == node.id
                && tombstone.nodes.len() == 1
                && tombstone.nodes.get(&node.id) == Some(node)
        })
        .map(|tombstone| tombstone.root_node_id.clone())
}

fn same_tombstone_content(left: &DeletionTombstone, right: &DeletionTombstone) -> bool {
    left.schema_version == right.schema_version
        && left.root_node_id == right.root_node_id
        && left.nodes == right.nodes
        && left.parents == right.parents
        && left.detached_from == right.detached_from
        && left.detached_index == right.detached_index
        && left.references == right.references
}

fn id_is_tombstoned(snapshot: &StudioDesignSnapshot, node_id: &NodeId) -> bool {
    snapshot
        .tombstones
        .values()
        .any(|tombstone| tombstone.nodes.contains_key(node_id))
}

fn collect_references(
    design: &StudioDesign,
    deleted: &BTreeSet<NodeId>,
) -> Vec<TombstoneReference> {
    let mut references = Vec::new();
    for (interaction_id, interaction) in &design.interactions {
        if deleted.contains(&interaction.source.node_id) {
            references.push(TombstoneReference {
                owner: format!("interaction:{interaction_id}"),
                field: "source.node_id".to_owned(),
                target_node_id: interaction.source.node_id.clone(),
            });
        }
        collect_action_references(
            &interaction.action,
            interaction_id,
            deleted,
            &mut references,
        );
    }
    for (owner_id, node) in &design.nodes {
        if deleted.contains(owner_id) {
            continue;
        }
        for (property, value) in &node.properties {
            collect_property_references(value, owner_id, property, deleted, &mut references);
        }
    }
    references
}

fn collect_action_references(
    action: &InteractionAction,
    interaction_id: &crate::InteractionId,
    deleted: &BTreeSet<NodeId>,
    references: &mut Vec<TombstoneReference>,
) {
    let target = match action {
        InteractionAction::SetProperty { node_id, .. }
        | InteractionAction::ToggleVisibility { node_id } => Some(node_id),
        InteractionAction::Navigate { .. }
        | InteractionAction::Emit { .. }
        | InteractionAction::Sequence { .. } => None,
    };
    if let Some(target) = target.filter(|target| deleted.contains(*target)) {
        references.push(TombstoneReference {
            owner: format!("interaction:{interaction_id}"),
            field: "action.node_id".to_owned(),
            target_node_id: target.clone(),
        });
    }
}

fn collect_property_references(
    value: &PropertyValue,
    owner_id: &NodeId,
    property: &str,
    deleted: &BTreeSet<NodeId>,
    references: &mut Vec<TombstoneReference>,
) {
    match value {
        PropertyValue::Node(target) if deleted.contains(target) => {
            references.push(TombstoneReference {
                owner: format!("node:{owner_id}"),
                field: format!("properties.{property}"),
                target_node_id: target.clone(),
            });
        }
        PropertyValue::List(values) => {
            for value in values {
                collect_property_references(value, owner_id, property, deleted, references);
            }
        }
        PropertyValue::String(_)
        | PropertyValue::Boolean(_)
        | PropertyValue::Integer(_)
        | PropertyValue::Decimal(_)
        | PropertyValue::Length(_)
        | PropertyValue::Color(_)
        | PropertyValue::Token(_)
        | PropertyValue::Binding(_)
        | PropertyValue::Node(_)
        | PropertyValue::Asset(_) => {}
    }
}

fn finalize_new_tombstones(snapshot: &mut StudioDesignSnapshot, revision: RevisionId) {
    for tombstone in snapshot.tombstones.values_mut() {
        if tombstone.deleted_in_revision.is_none() {
            tombstone.deleted_in_revision = Some(revision);
        }
    }
}

fn finalize_inverse_tombstones(commands: &mut [Command], revision: RevisionId) {
    for command in commands {
        if let Command::RestoreNode { tombstone } = command
            && tombstone.deleted_in_revision.is_none()
        {
            tombstone.deleted_in_revision = Some(revision);
        }
    }
}

fn reference_diagnostics(snapshot: &StudioDesignSnapshot) -> Vec<DesignerDiagnostic> {
    snapshot
        .tombstones
        .values()
        .flat_map(|tombstone| &tombstone.references)
        .map(|reference| DesignerDiagnostic {
            code: "DESIGN_REFERENCE_DELETED".to_owned(),
            severity: DiagnosticSeverity::Warning,
            message: format!(
                "{} still references deleted node {} through {}",
                reference.owner, reference.target_node_id, reference.field
            ),
            node_id: Some(reference.target_node_id.clone()),
            interaction_id: None,
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn validate_design(design: &StudioDesign) -> Vec<DesignerDiagnostic> {
    let mut diagnostics = Vec::new();
    if design.schema_version != STUDIO_DESIGN_SCHEMA_VERSION {
        diagnostics.push(diagnostic(
            "DESIGN_SCHEMA_UNSUPPORTED",
            "the Studio Design schema version is unsupported",
            None,
        ));
    }
    if design.name.trim().is_empty() {
        diagnostics.push(diagnostic(
            "DESIGN_NAME_INVALID",
            "a Studio Design name cannot be empty",
            None,
        ));
    }
    let ordered_screens = design.screen_order.iter().cloned().collect::<BTreeSet<_>>();
    if ordered_screens.len() != design.screen_order.len()
        || ordered_screens != design.screens.keys().cloned().collect()
    {
        diagnostics.push(diagnostic(
            "DESIGN_SCREEN_ORDER_INVALID",
            "screen_order must contain every screen identity exactly once",
            None,
        ));
    }
    if design.nodes.len() != design.parents.len() || design.nodes.keys().ne(design.parents.keys()) {
        diagnostics.push(diagnostic(
            "DESIGN_PARENT_INDEX_INVALID",
            "the parent index must contain exactly one owner for every node",
            None,
        ));
    }
    for (screen_id, screen) in &design.screens {
        if screen.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
            || screen.id != *screen_id
            || !screen.route.starts_with('/')
            || !design.nodes.contains_key(&screen.root_node_id)
            || design.parents.get(&screen.root_node_id)
                != Some(&NodeParent::Screen {
                    screen_id: screen_id.clone(),
                })
        {
            diagnostics.push(diagnostic(
                "DESIGN_SCREEN_INVALID",
                "a screen record, route, root, or root parent is invalid",
                Some(screen.root_node_id.clone()),
            ));
        }
    }
    for (composition_id, composition) in &design.compositions {
        if composition.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
            || composition.id != *composition_id
            || !design.nodes.contains_key(&composition.root_node_id)
            || design.parents.get(&composition.root_node_id)
                != Some(&NodeParent::Composition {
                    composition_id: composition_id.clone(),
                })
        {
            diagnostics.push(diagnostic(
                "DESIGN_COMPOSITION_INVALID",
                "a composition record, root, or root parent is invalid",
                Some(composition.root_node_id.clone()),
            ));
        }
    }
    for (node_id, node) in &design.nodes {
        if node.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
            || node.layout.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
            || node.style.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
            || node.accessibility.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
            || node.id != *node_id
        {
            diagnostics.push(node_diagnostic(
                "DESIGN_NODE_SCHEMA_INVALID",
                "a node identity or nested schema version is invalid",
                node_id,
            ));
        }
        let unique_children = node.children.iter().cloned().collect::<BTreeSet<_>>();
        if unique_children.len() != node.children.len() {
            diagnostics.push(node_diagnostic(
                "DESIGN_CHILD_DUPLICATE",
                "a node child list contains a duplicate identity",
                node_id,
            ));
        }
        for child in &node.children {
            if !design.nodes.contains_key(child)
                || design.parents.get(child)
                    != Some(&NodeParent::Node {
                        node_id: node_id.clone(),
                    })
            {
                diagnostics.push(node_diagnostic(
                    "DESIGN_CHILD_INDEX_INVALID",
                    "a child identity or its indexed parent is inconsistent",
                    child,
                ));
            }
        }
        if let DesignNodeSource::CompositionInstance { composition_id, .. } = &node.source
            && !design.compositions.contains_key(composition_id)
        {
            diagnostics.push(node_diagnostic(
                "DESIGN_COMPOSITION_MISSING",
                "a composition instance references an unknown definition",
                node_id,
            ));
        }
        if node.responsive_overrides.iter().any(|(variant_id, value)| {
            !design.responsive_variants.contains_key(variant_id)
                || value.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
        }) {
            diagnostics.push(node_diagnostic(
                "DESIGN_RESPONSIVE_INVALID",
                "a responsive override references an unknown or unsupported variant",
                node_id,
            ));
        }
        if node
            .interaction_ids
            .iter()
            .any(|interaction_id| !design.interactions.contains_key(interaction_id))
        {
            diagnostics.push(node_diagnostic(
                "DESIGN_INTERACTION_MISSING",
                "a node references an unknown interaction",
                node_id,
            ));
        }
        if parent_chain_cycles(design, node_id) {
            diagnostics.push(node_diagnostic(
                "DESIGN_PARENT_CYCLE",
                "the parent index contains a cycle",
                node_id,
            ));
        }
    }
    for (token_id, token) in &design.tokens {
        if token.schema_version != STUDIO_DESIGN_SCHEMA_VERSION || token.id != *token_id {
            diagnostics.push(diagnostic(
                "DESIGN_TOKEN_INVALID",
                "a token identity or schema version is invalid",
                None,
            ));
        }
    }
    for (variant_id, variant) in &design.responsive_variants {
        if variant.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
            || variant.id != *variant_id
            || variant
                .minimum_width
                .zip(variant.maximum_width)
                .is_some_and(|(minimum, maximum)| minimum > maximum)
        {
            diagnostics.push(diagnostic(
                "DESIGN_RESPONSIVE_INVALID",
                "a responsive variant identity, bounds, or schema version is invalid",
                None,
            ));
        }
    }
    for (interaction_id, interaction) in &design.interactions {
        if interaction.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
            || interaction.id != *interaction_id
        {
            diagnostics.push(diagnostic(
                "DESIGN_INTERACTION_INVALID",
                "an interaction identity or schema version is invalid",
                None,
            ));
        }
    }
    diagnostics
}

fn parent_chain_cycles(design: &StudioDesign, start: &NodeId) -> bool {
    let mut seen = BTreeSet::new();
    let mut cursor = Some(start);
    while let Some(node_id) = cursor {
        if !seen.insert(node_id) {
            return true;
        }
        cursor = match design.parents.get(node_id) {
            Some(NodeParent::Node { node_id }) => Some(node_id),
            Some(NodeParent::Screen { .. } | NodeParent::Composition { .. }) | None => None,
        };
    }
    false
}

fn receipt_for_batch(batch: &CommandBatch, committed_revision: RevisionId) -> CommandReceipt {
    CommandReceipt {
        operation_id: batch.operation_id.clone(),
        project_id: batch.project_id.clone(),
        base_revision: batch.base_revision,
        committed_revision,
        actor: batch.actor.clone(),
        undo_group_id: batch.undo_group_id.clone(),
        undo_group_name: batch.undo_group_name.clone(),
        command_count: batch.commands.len(),
    }
}

fn stale_conflict(
    operation_id: OperationId,
    expected_revision: RevisionId,
    actual_revision: RevisionId,
) -> BatchConflict {
    BatchConflict {
        operation_id,
        expected_revision,
        actual_revision,
        failed_precondition: None,
        code: "DESIGN_STALE_REVISION".to_owned(),
        message: "the batch base revision is stale".to_owned(),
    }
}

fn precondition_conflict(
    operation_id: OperationId,
    revision: RevisionId,
    failed_precondition: usize,
) -> BatchConflict {
    BatchConflict {
        operation_id,
        expected_revision: revision,
        actual_revision: revision,
        failed_precondition: Some(failed_precondition),
        code: "DESIGN_PRECONDITION_FAILED".to_owned(),
        message: "a structural or property precondition no longer holds".to_owned(),
    }
}

fn valid_field_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

fn missing_node(node_id: &NodeId) -> DesignerDiagnostic {
    node_diagnostic(
        "DESIGN_NODE_MISSING",
        "the command references an unknown node identity",
        node_id,
    )
}

fn invalid_parent(node_id: &NodeId, message: impl Into<String>) -> DesignerDiagnostic {
    node_diagnostic("DESIGN_PARENT_INVALID", message, node_id)
}

fn node_diagnostic(code: &str, message: impl Into<String>, node_id: &NodeId) -> DesignerDiagnostic {
    diagnostic(code, message, Some(node_id.clone()))
}

fn diagnostic(
    code: &str,
    message: impl Into<String>,
    node_id: Option<NodeId>,
) -> DesignerDiagnostic {
    DesignerDiagnostic {
        code: code.to_owned(),
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        node_id,
        interaction_id: None,
    }
}

fn join_diagnostics(diagnostics: &[DesignerDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
