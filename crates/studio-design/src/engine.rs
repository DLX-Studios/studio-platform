//! Validated command execution and immutable history implementation.

use std::collections::{BTreeMap, BTreeSet};

use crate::responsive::{DeviceProfileMatrix, compare_profiles, inspect_node};
use crate::session::CanvasStateSnapshot;
use crate::content::{
    binding_diagnostics, validate_binding_shape, validate_collection_schema, validate_fixture,
    validate_form_shape, validate_record,
};

use crate::{
    NodeKind,
    command::{
        AppliedBatch, Command, CommandBatch, CommandPrecondition, HistoryEntry, ParentPlacement,
    },
    model::{
        Actor, Alignment, BindingId, BindingPath, CollectionId, ContentBinding, ContentCollection,
        ContentCollectionSchema, ContentFixture, ContentRecord, DeletionTombstone, DesignNode,
        DesignNodeSource, DesignToken, DesignerDiagnostic, DiagnosticSeverity, FixtureKind,
        FormDefinition, FormId, InspectedTokenValue, Interaction,
        InteractionAction,
        InteractionId, LayoutProperties, Length, LengthUnit, NodeId, NodeParent, OperationId,
        Placement, ProjectId, PropertyValue, RecordId, ResponsiveNodeOverride, ResponsiveVariant,
        ResponsiveVariantId, RevisionId, RevisionMetadata, RevisionReason,
        STUDIO_DESIGN_SCHEMA_VERSION, SelectionSnapshot, StudioDesign, StudioDesignSnapshot,
        StyleProperties, TokenId, TokenKind, TokenOverride, TokenUsage, TokenValue,
        TombstoneReference, UndoGroupId, ValueKind,
    },
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
    canvas_transform: crate::CanvasTransform,
    runs: Vec<crate::AgentRun>,
    unsaved_work: crate::UnsavedWork,
    panel_state: BTreeMap<String, bool>,
    last_diagnostics: Vec<DesignerDiagnostic>,
    profile_matrix: DeviceProfileMatrix,
    canvas: CanvasStateSnapshot,
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
            canvas_transform: crate::CanvasTransform::IDENTITY,
            runs: Vec::new(),
            unsaved_work: crate::UnsavedWork::default(),
            panel_state: BTreeMap::new(),
            last_diagnostics: Vec::new(),
            profile_matrix: DeviceProfileMatrix::standard(),
            canvas: CanvasStateSnapshot::default(),
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
            canvas_transform: crate::CanvasTransform::IDENTITY,
            runs: Vec::new(),
            unsaved_work: crate::UnsavedWork::default(),
            panel_state: BTreeMap::new(),
            last_diagnostics: Vec::new(),
            profile_matrix: DeviceProfileMatrix::standard(),
            canvas: CanvasStateSnapshot::default(),
        })
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
            return self.reject(batch_diagnostics);
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
            Err(diagnostics) => return self.reject(diagnostics),
        };
        let diagnostics = validate_snapshot(&snapshot);
        if !diagnostics.is_empty() {
            return self.reject(diagnostics);
        }
        let Some(committed_revision) = current_revision.checked_next() else {
            return self.reject(vec![diagnostic(
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
            return self.reject(vec![diagnostic(
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
                        return self.reject(diagnostics);
                    }
                }
                commands
            }
            RevisionReason::Initial | RevisionReason::Command => Vec::new(),
        };

        let mut snapshot = self.state.current.clone();
        if let Err(diagnostics) = apply_commands(&mut snapshot, &commands) {
            return self.reject(diagnostics);
        }
        let diagnostics = validate_snapshot(&snapshot);
        if !diagnostics.is_empty() {
            return self.reject(diagnostics);
        }
        let Some(committed_revision) = current_revision.checked_next() else {
            return self.reject(vec![diagnostic(
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
        self.last_diagnostics.clear();
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
            canvas_transform: self.canvas_transform,
            runs: self.runs.clone(),
            unsaved_work: self.unsaved_work.clone(),
            panel_state: self.panel_state.clone(),
            history_cursor: self.state.history_cursor,
            canvas: self.canvas.clone(),
        }
    }

    fn reject(&mut self, diagnostics: Vec<DesignerDiagnostic>) -> CommandOutcome {
        self.last_diagnostics.clone_from(&diagnostics);
        CommandOutcome::Rejected(diagnostics)
    }

    fn diagnostics(&self) -> Vec<DesignerDiagnostic> {
        self.state
            .diagnostics
            .iter()
            .cloned()
            .chain(self.last_diagnostics.iter().cloned())
            .collect()
    }
}

impl<P: DesignerPersistence> DesignerSession for DefaultDesignerSession<P> {
    fn query(&self, query: DesignerQuery) -> DesignerQueryResult {
        match query {
            DesignerQuery::Snapshot => DesignerQueryResult::Snapshot(self.state.current.clone()),
            DesignerQuery::Node { node_id } => {
                DesignerQueryResult::Node(self.state.current.design.nodes.get(&node_id).cloned())
            }
            DesignerQuery::NodeForProfile { node_id, profile } => DesignerQueryResult::Node(
                self.state
                    .current
                    .design
                    .node_for_profile(&node_id, profile.as_deref()),
            ),
            DesignerQuery::Tokens => DesignerQueryResult::Tokens(
                self.state.current.design.tokens.values().cloned().collect(),
            ),
            DesignerQuery::Token { token_id } => {
                DesignerQueryResult::Token(self.state.current.design.tokens.get(&token_id).cloned())
            }
            DesignerQuery::TokenUsages { token_id } => {
                DesignerQueryResult::TokenUsages(token_usages(&self.state.current, &token_id))
            }
            DesignerQuery::NodeTokenValues { node_id } => DesignerQueryResult::NodeTokenValues(
                inspected_token_values(&self.state.current, &node_id),
            ),
            DesignerQuery::Collection { collection_id } => DesignerQueryResult::Collection(
                self.state.current.design.collections.get(&collection_id).cloned(),
            ),
            DesignerQuery::Collections => DesignerQueryResult::Collections(
                self.state.current.design.collections.values().cloned().collect(),
            ),
            DesignerQuery::Bindings => DesignerQueryResult::Bindings(
                self.state.current.design.bindings.values().cloned().collect(),
            ),
            DesignerQuery::Forms => DesignerQueryResult::Forms(
                self.state.current.design.forms.values().cloned().collect(),
            ),
            DesignerQuery::Preview {
                collection_id,
                fixture,
            } => DesignerQueryResult::Preview(
                self.state
                    .current
                    .design
                    .collections
                    .get(&collection_id)
                    .map(|collection| crate::content::preview_collection(collection, fixture)),
            ),
            DesignerQuery::ValidateForm { form_id, values } => DesignerQueryResult::FormValidation(
                self.state
                    .current
                    .design
                    .forms
                    .get(&form_id)
                    .map(|form| crate::content::validate_form_values(form, &values))
                    .unwrap_or(crate::FormValidationResult {
                        valid: false,
                        field_errors: std::iter::once((
                            "form".to_owned(),
                            "the form does not exist".to_owned(),
                        ))
                        .collect(),
                    }),
            ),
            DesignerQuery::Diagnostics => DesignerQueryResult::Diagnostics(self.diagnostics()),
            DesignerQuery::DiagnosticsForNode { node_id } => DesignerQueryResult::Diagnostics(
                self.diagnostics()
                    .iter()
                    .filter(|diagnostic| diagnostic.node_id.as_ref() == Some(&node_id))
                    .cloned()
                    .collect(),
            ),
            DesignerQuery::History => DesignerQueryResult::History(HistorySnapshot {
                entries: self.state.history.clone(),
                cursor: self.state.history_cursor,
            }),
            DesignerQuery::SessionState => DesignerQueryResult::SessionState(self.session_state()),
            DesignerQuery::ResponsiveProfiles => {
                DesignerQueryResult::ResponsiveProfiles(self.profile_matrix.clone())
            }
            DesignerQuery::ResponsiveInspector {
                node_id,
                profile_id,
            } => {
                let entries = self
                    .state
                    .current
                    .design
                    .nodes
                    .get(&node_id)
                    .and_then(|node| {
                        self.profile_matrix
                            .profiles
                            .get(&profile_id)
                            .map(|profile| inspect_node(&self.state.current.design, node, profile))
                    })
                    .unwrap_or_default();
                DesignerQueryResult::ResponsiveInspector(entries)
            }
            DesignerQuery::CompareProfiles {
                node_id,
                left_profile_id,
                right_profile_id,
            } => {
                let report = self
                    .state
                    .current
                    .design
                    .nodes
                    .get(&node_id)
                    .and_then(|node| {
                        self.profile_matrix
                            .profiles
                            .get(&left_profile_id)
                            .and_then(|left| {
                                self.profile_matrix.profiles.get(&right_profile_id).map(|right| {
                                    compare_profiles(&self.state.current.design, node, left, right)
                                })
                            })
                    })
                    .unwrap_or(crate::CompareReport {
                        node_id,
                        left_profile: left_profile_id,
                        right_profile: right_profile_id,
                        differences: Vec::new(),
                    });
                DesignerQueryResult::ProfileComparison(report)
            }
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
        if let Some(canvas_transform) = update.canvas_transform {
            self.canvas_transform = canvas_transform;
        }
        if let Some(runs) = update.runs {
            self.runs = runs;
        }
        if let Some(unsaved_work) = update.unsaved_work {
            self.unsaved_work = unsaved_work;
        }
        if let Some(panel_state) = update.panel_state {
            self.panel_state = panel_state;
        }
        if let Some(canvas) = update.canvas {
            self.canvas = canvas;
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
        CommandPrecondition::BreakpointPropertyEquals {
            node_id,
            variant_id,
            property,
            value,
        } => design
            .nodes
            .get(node_id)
            .and_then(|node| node.responsive_overrides.get(variant_id))
            .and_then(|override_value| override_value.properties.get(property))
            != value.as_ref(),
        CommandPrecondition::TokenExists { token_id } => !design.tokens.contains_key(token_id),
        CommandPrecondition::TokenMissing { token_id } => design.tokens.contains_key(token_id),
        CommandPrecondition::TokenValueEquals { token_id, value } => design
            .tokens
            .get(token_id)
            .is_none_or(|token| token.value != *value),
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
        Command::SetBreakpointProperty {
            node_id,
            variant_id,
            property,
            value,
        } => set_breakpoint_property(snapshot, node_id, variant_id, property, value.as_ref()),
        Command::SetBreakpointOverride {
            node_id,
            variant_id,
            value,
        } => set_breakpoint_override(snapshot, node_id, variant_id, value.as_deref()),
        Command::SetLayout { node_id, layout } => set_layout(snapshot, node_id, layout),
        Command::SetResponsiveLayout {
            node_id,
            variant_id,
            layout,
        } => set_responsive_layout(snapshot, node_id, variant_id, layout),
        Command::RemoveResponsiveLayout {
            node_id,
            variant_id,
        } => remove_responsive_layout(snapshot, node_id, variant_id),
        Command::RenameNode { node_id, name } => rename_node(snapshot, node_id, name),
        Command::DefineResponsiveVariant { variant } => {
            define_responsive_variant(snapshot, variant)
        }
        Command::UpdateResponsiveVariant { variant } => {
            update_responsive_variant(snapshot, variant)
        }
        Command::RemoveResponsiveVariant { variant_id } => {
            remove_responsive_variant(snapshot, variant_id)
        }
        Command::SetResponsiveOverride {
            node_id,
            variant_id,
            value,
        } => set_responsive_override(snapshot, node_id, variant_id, value.as_ref()),
        Command::DefineToken { token } => define_token(snapshot, token),
        Command::UpdateToken { token } => update_token(snapshot, token),
        Command::RemoveToken { token_id } => remove_token(snapshot, token_id),
        Command::ApplyToken {
            node_id,
            property,
            token_id,
        } => apply_token(snapshot, node_id, property, token_id),
        Command::CreateToken { token } => create_token(snapshot, token),
        Command::EditToken { token_id, value } => edit_token(snapshot, token_id, value),
        Command::OverrideToken {
            node_id,
            property,
            value,
        } => override_token(snapshot, node_id, property, value),
        Command::ClearTokenOverride { node_id, property } => {
            clear_token_override(snapshot, node_id, property)
        }
        Command::RenameToken { token_id, name } => rename_token(snapshot, token_id, name),
        Command::DeleteToken { token_id, confirm } => delete_token(snapshot, token_id, *confirm),
        Command::SetTokenOverride {
            node_id,
            property,
            value,
        } => set_token_override(snapshot, node_id, property, value.as_ref()),
        Command::RestoreTokenApplication {
            node_id,
            property,
            property_value,
            override_value,
        } => restore_token_application(
            snapshot,
            node_id,
            property,
            property_value.as_ref(),
            override_value.as_ref(),
        ),
        Command::SetBinding {
            node_id,
            property,
            binding,
        } => set_binding(snapshot, node_id, property, binding.as_ref()),
        Command::DefineInteraction { interaction } => define_interaction(snapshot, interaction),
        Command::UpdateInteraction { interaction } => update_interaction(snapshot, interaction),
        Command::RemoveInteraction { interaction_id } => {
            remove_interaction(snapshot, interaction_id)
        }
        Command::DefineComposition { composition } => define_composition(snapshot, composition),
        Command::UpdateComposition { composition } => update_composition(snapshot, composition),
        Command::RemoveComposition { composition_id } => {
            remove_composition(snapshot, composition_id)
        }
        Command::InstantiateComposition {
            node_id,
            name,
            parent,
            composition_id,
            inputs,
        } => instantiate_composition(snapshot, node_id, name, parent, composition_id, inputs),
        Command::SetCompositionInput {
            node_id,
            input,
            value,
        } => set_composition_input(snapshot, node_id, input, value.as_ref(), false),
        Command::SetCompositionOverride {
            node_id,
            input,
            value,
        } => set_composition_input(snapshot, node_id, input, value.as_ref(), true),
        Command::CreateCollection { collection } => create_collection(snapshot, collection),
        Command::UpdateCollectionSchema {
            collection_id,
            schema,
        } => update_collection_schema(snapshot, collection_id, schema),
        Command::DeleteCollection { collection_id } => delete_collection(snapshot, collection_id),
        Command::CreateRecord {
            collection_id,
            record,
        } => create_record(snapshot, collection_id, record),
        Command::UpdateRecord {
            collection_id,
            record_id,
            values,
        } => update_record(snapshot, collection_id, record_id, values),
        Command::DeleteRecord {
            collection_id,
            record_id,
        } => delete_record(snapshot, collection_id, record_id),
        Command::SetFixture {
            collection_id,
            fixture,
        } => set_fixture(snapshot, collection_id, fixture),
        Command::UpsertBinding { binding } => upsert_binding(snapshot, binding),
        Command::RemoveBinding { binding_id } => remove_binding(snapshot, binding_id),
        Command::UpsertForm { form } => upsert_form(snapshot, form),
        Command::RemoveForm { form_id } => remove_form(snapshot, form_id),
    }
}

fn insert_node(
fn create_token(
    snapshot: &mut StudioDesignSnapshot,
    token: &crate::DesignToken,
) -> Result<Command, DesignerDiagnostic> {
    validate_token(token)?;
    if snapshot.design.tokens.contains_key(&token.id) {
        return Err(diagnostic(
            "DESIGN_TOKEN_EXISTS",
            format!("token identity {} already exists", token.id),
            None,
        ));
    }
    snapshot
        .design
        .tokens
        .insert(token.id.clone(), token.clone());
    Ok(Command::DeleteToken {
        token_id: token.id.clone(),
        confirm: true,
    })
}

fn edit_token(
    snapshot: &mut StudioDesignSnapshot,
    token_id: &TokenId,
    value: &TokenValue,
) -> Result<Command, DesignerDiagnostic> {
    let token = snapshot
        .design
        .tokens
        .get_mut(token_id)
        .ok_or_else(|| token_missing(token_id))?;
    validate_token_value(token.kind, value)?;
    let prior = std::mem::replace(&mut token.value, value.clone());
    Ok(Command::EditToken {
        token_id: token_id.clone(),
        value: prior,
    })
}

fn apply_token(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    property: &str,
    token_id: &TokenId,
) -> Result<Command, DesignerDiagnostic> {
    if !valid_field_name(property) {
        return Err(node_diagnostic(
            "DESIGN_PROPERTY_INVALID",
            "a property name must contain 1..=128 safe bytes",
            node_id,
        ));
    }
    if !snapshot.design.tokens.contains_key(token_id) {
        return Err(token_missing(token_id));
    }
    let node = snapshot
        .design
        .nodes
        .get_mut(node_id)
        .ok_or_else(|| missing_node(node_id))?;
    let prior = node
        .properties
        .insert(property.to_owned(), PropertyValue::Token(token_id.clone()));
    let prior_override = node.token_overrides.remove(property);
    Ok(Command::RestoreTokenApplication {
        node_id: node_id.clone(),
        property: property.to_owned(),
        property_value: prior,
        override_value: prior_override,
    })
}

fn override_token(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    property: &str,
    value: &TokenValue,
) -> Result<Command, DesignerDiagnostic> {
    let token_id = bound_token_id(snapshot, node_id, property)?;
    let token = snapshot
        .design
        .tokens
        .get(&token_id)
        .ok_or_else(|| token_missing(&token_id))?;
    validate_token_value(token.kind, value)?;
    let node = snapshot
        .design
        .nodes
        .get_mut(node_id)
        .expect("binding was validated");
    let prior = node.token_overrides.insert(
        property.to_owned(),
        TokenOverride {
            token_id,
            value: value.clone(),
        },
    );
    Ok(match prior {
        Some(value) => Command::SetTokenOverride {
            node_id: node_id.clone(),
            property: property.to_owned(),
            value: Some(value),
        },
        None => Command::ClearTokenOverride {
            node_id: node_id.clone(),
            property: property.to_owned(),
        },
    })
}

fn clear_token_override(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    property: &str,
) -> Result<Command, DesignerDiagnostic> {
    let node = snapshot
        .design
        .nodes
        .get_mut(node_id)
        .ok_or_else(|| missing_node(node_id))?;
    let prior = node.token_overrides.remove(property).ok_or_else(|| {
        node_diagnostic(
            "DESIGN_TOKEN_OVERRIDE_MISSING",
            "the property has no local token override",
            node_id,
        )
    })?;
    Ok(Command::SetTokenOverride {
        node_id: node_id.clone(),
        property: property.to_owned(),
        value: Some(prior),
    })
}

fn set_token_override(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    property: &str,
    value: Option<&TokenOverride>,
) -> Result<Command, DesignerDiagnostic> {
    let current_binding = bound_token_id(snapshot, node_id, property)?;
    if let Some(value) = value {
        if value.token_id != current_binding {
            return Err(node_diagnostic(
                "DESIGN_TOKEN_OVERRIDE_BINDING",
                "a local override must retain the property's current token binding",
                node_id,
            ));
        }
        let token = snapshot
            .design
            .tokens
            .get(&value.token_id)
            .ok_or_else(|| token_missing(&value.token_id))?;
        validate_token_value(token.kind, &value.value)?;
    }
    let node = snapshot
        .design
        .nodes
        .get_mut(node_id)
        .expect("binding was validated");
    let prior = match value {
        Some(value) => node
            .token_overrides
            .insert(property.to_owned(), value.clone()),
        None => node.token_overrides.remove(property),
    };
    Ok(Command::SetTokenOverride {
        node_id: node_id.clone(),
        property: property.to_owned(),
        value: prior,
    })
}

fn restore_token_application(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    property: &str,
    property_value: Option<&PropertyValue>,
    override_value: Option<&TokenOverride>,
) -> Result<Command, DesignerDiagnostic> {
    let node = snapshot
        .design
        .nodes
        .get_mut(node_id)
        .ok_or_else(|| missing_node(node_id))?;
    let prior_property = match property_value {
        Some(value) => node.properties.insert(property.to_owned(), value.clone()),
        None => node.properties.remove(property),
    };
    let prior_override = match override_value {
        Some(value) => node
            .token_overrides
            .insert(property.to_owned(), value.clone()),
        None => node.token_overrides.remove(property),
    };
    Ok(Command::RestoreTokenApplication {
        node_id: node_id.clone(),
        property: property.to_owned(),
        property_value: prior_property,
        override_value: prior_override,
    })
}

fn rename_token(
    snapshot: &mut StudioDesignSnapshot,
    token_id: &TokenId,
    name: &str,
) -> Result<Command, DesignerDiagnostic> {
    if name.trim().is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err(diagnostic(
            "DESIGN_TOKEN_NAME_INVALID",
            "a token name must contain 1..=256 safe bytes",
            None,
        ));
    }
    let token = snapshot
        .design
        .tokens
        .get_mut(token_id)
        .ok_or_else(|| token_missing(token_id))?;
    let prior = std::mem::replace(&mut token.name, name.to_owned());
    Ok(Command::RenameToken {
        token_id: token_id.clone(),
        name: prior,
    })
}

fn delete_token(
    snapshot: &mut StudioDesignSnapshot,
    token_id: &TokenId,
    confirm: bool,
) -> Result<Command, DesignerDiagnostic> {
    if !snapshot.design.tokens.contains_key(token_id) {
        return Err(token_missing(token_id));
    }
    let usages = token_usages(snapshot, token_id);
    if !usages.is_empty() && !confirm {
        let listed = usages
            .iter()
            .map(|usage| format!("{}:{}", usage.owner, usage.property))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(diagnostic(
            "DESIGN_TOKEN_DELETE_CONFIRMATION_REQUIRED",
            format!("token {token_id} is used by {listed}; confirm deletion to continue"),
            None,
        ));
    }
    let token = snapshot
        .design
        .tokens
        .remove(token_id)
        .expect("existence was checked");
    Ok(Command::CreateToken {
        token: Box::new(token),
    })
}

fn bound_token_id(
    snapshot: &StudioDesignSnapshot,
    node_id: &NodeId,
    property: &str,
) -> Result<TokenId, DesignerDiagnostic> {
    let node = snapshot
        .design
        .nodes
        .get(node_id)
        .ok_or_else(|| missing_node(node_id))?;
    match node.properties.get(property) {
        Some(PropertyValue::Token(token_id)) => Ok(token_id.clone()),
        _ => Err(node_diagnostic(
            "DESIGN_TOKEN_BINDING_REQUIRED",
            "the property must be bound to a token before it can be overridden",
            node_id,
        )),
    }
}

fn validate_token(token: &crate::DesignToken) -> Result<(), DesignerDiagnostic> {
    if token.schema_version != STUDIO_DESIGN_SCHEMA_VERSION {
        return Err(diagnostic(
            "DESIGN_TOKEN_INVALID",
            "a token schema version is unsupported",
            None,
        ));
    }
    if token.name.trim().is_empty()
        || token.name.len() > 256
        || token.name.chars().any(char::is_control)
    {
        return Err(diagnostic(
            "DESIGN_TOKEN_NAME_INVALID",
            "a token name must contain 1..=256 safe bytes",
            None,
        ));
    }
    validate_token_value(token.kind, &token.value)
}

fn validate_token_value(kind: TokenKind, value: &TokenValue) -> Result<(), DesignerDiagnostic> {
    let compatible = match kind {
        TokenKind::Color => matches!(value, TokenValue::Color(_)),
        TokenKind::Typography => matches!(value, TokenValue::Typography(_)),
        TokenKind::Spacing | TokenKind::Radius | TokenKind::Length => {
            matches!(value, TokenValue::Length(_))
        }
        TokenKind::Border => matches!(value, TokenValue::Border(_)),
        TokenKind::Shadow => matches!(value, TokenValue::Shadow(_)),
        TokenKind::Motion => matches!(value, TokenValue::Motion(_)),
        TokenKind::Number => matches!(value, TokenValue::Number(_)),
        TokenKind::String => matches!(value, TokenValue::String(_)),
    };
    compatible.then_some(()).ok_or_else(|| {
        diagnostic(
            "DESIGN_TOKEN_VALUE_KIND_MISMATCH",
            "the token value does not match the token kind",
            None,
        )
    })
}

fn token_missing(token_id: &TokenId) -> DesignerDiagnostic {
    diagnostic(
        "DESIGN_TOKEN_MISSING",
        format!("the command references unknown token {token_id}"),
        None,
    )
}

fn token_usages(snapshot: &StudioDesignSnapshot, token_id: &TokenId) -> Vec<TokenUsage> {
    let mut usages = Vec::new();
    for (node_id, node) in &snapshot.design.nodes {
        for (property, value) in &node.properties {
            collect_token_usages(
                value,
                token_id,
                format!("node:{node_id}"),
                property,
                Some(node_id),
                node.token_overrides
                    .get(property)
                    .map(|override_value| override_value.value.clone()),
                &mut usages,
            );
        }
        collect_paint_usage(
            node.style.background.as_ref(),
            token_id,
            node_id,
            "style.background",
            &mut usages,
        );
        collect_paint_usage(
            node.style.foreground.as_ref(),
            token_id,
            node_id,
            "style.foreground",
            &mut usages,
        );
        collect_paint_usage(
            node.style.border_color.as_ref(),
            token_id,
            node_id,
            "style.border_color",
            &mut usages,
        );
        if let DesignNodeSource::CompositionInstance {
            inputs,
            admitted_overrides,
            ..
        } = &node.source
        {
            for (property, value) in inputs {
                collect_token_usages(
                    value,
                    token_id,
                    format!("node:{node_id}.inputs"),
                    property,
                    Some(node_id),
                    None,
                    &mut usages,
                );
            }
            for (property, value) in admitted_overrides {
                collect_token_usages(
                    value,
                    token_id,
                    format!("node:{node_id}.admitted_overrides"),
                    property,
                    Some(node_id),
                    None,
                    &mut usages,
                );
            }
        }
        for (variant_id, responsive) in &node.responsive_overrides {
            for (property, value) in &responsive.properties {
                collect_token_usages(
                    value,
                    token_id,
                    format!("node:{node_id}.responsive:{variant_id}"),
                    property,
                    Some(node_id),
                    None,
                    &mut usages,
                );
            }
        }
    }
    for (interaction_id, interaction) in &snapshot.design.interactions {
        if let InteractionAction::SetProperty {
            node_id,
            property,
            value,
        } = &interaction.action
        {
            collect_token_usages(
                value,
                token_id,
                format!("interaction:{interaction_id}"),
                property,
                Some(node_id),
                None,
                &mut usages,
            );
        }
    }
    usages
}

fn collect_token_usages(
    value: &PropertyValue,
    token_id: &TokenId,
    owner: String,
    property: &str,
    node_id: Option<&NodeId>,
    local_override: Option<TokenValue>,
    usages: &mut Vec<TokenUsage>,
) {
    match value {
        PropertyValue::Token(found) if found == token_id => usages.push(TokenUsage {
            token_id: token_id.clone(),
            owner,
            property: property.to_owned(),
            node_id: node_id.cloned(),
            local_override,
        }),
        PropertyValue::List(values) => {
            for (index, value) in values.iter().enumerate() {
                collect_token_usages(
                    value,
                    token_id,
                    owner.clone(),
                    &format!("{property}[{index}]"),
                    node_id,
                    None,
                    usages,
                );
            }
        }
        _ => {}
    }
}

fn collect_paint_usage(
    paint: Option<&crate::Paint>,
    token_id: &TokenId,
    node_id: &NodeId,
    property: &str,
    usages: &mut Vec<TokenUsage>,
) {
    if matches!(paint, Some(crate::Paint::Token(found)) if found == token_id) {
        usages.push(TokenUsage {
            token_id: token_id.clone(),
            owner: format!("node:{node_id}"),
            property: property.to_owned(),
            node_id: Some(node_id.clone()),
            local_override: None,
        });
    }
}

fn inspected_token_values(
    snapshot: &StudioDesignSnapshot,
    node_id: &NodeId,
) -> Vec<InspectedTokenValue> {
    let Some(node) = snapshot.design.nodes.get(node_id) else {
        return Vec::new();
    };
    node.properties
        .iter()
        .filter_map(|(property, value)| {
            let PropertyValue::Token(token_id) = value else {
                return None;
            };
            Some(InspectedTokenValue {
                property: property.clone(),
                token_id: token_id.clone(),
                shared_value: snapshot
                    .design
                    .tokens
                    .get(token_id)
                    .map(|token| token.value.clone()),
                local_value: node
                    .token_overrides
                    .get(property)
                    .map(|override_value| override_value.value.clone()),
            })
        })
        .collect()
}

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
    if is_typed_layout_property(property) {
        return Err(node_diagnostic(
            "DESIGN_LAYOUT_TYPED_REQUIRED",
            "layout fields must be edited through a typed layout command",
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

fn set_breakpoint_property(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    variant_id: &ResponsiveVariantId,
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
    if !snapshot.design.responsive_variants.contains_key(variant_id) {
        return Err(node_diagnostic(
            "DESIGN_RESPONSIVE_VARIANT_MISSING",
            "the breakpoint override references an unknown responsive variant",
            node_id,
        ));
    }
    let node = snapshot
        .design
        .nodes
        .get_mut(node_id)
        .ok_or_else(|| missing_node(node_id))?;
    let prior = node
        .responsive_overrides
        .get(variant_id)
        .and_then(|override_value| override_value.properties.get(property))
        .cloned();
    let entry = node
        .responsive_overrides
        .entry(variant_id.clone())
        .or_insert_with(|| ResponsiveNodeOverride {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            properties: BTreeMap::new(),
            layout: LayoutProperties::default(),
            style: StyleProperties::default(),
        });
    match value {
        Some(value) => {
            entry.properties.insert(property.to_owned(), value.clone());
        }
        None => {
            entry.properties.remove(property);
        }
    }
    let empty = entry.properties.is_empty()
        && entry.layout == LayoutProperties::default()
        && entry.style == StyleProperties::default();
    if empty {
        node.responsive_overrides.remove(variant_id);
    }
    Ok(Command::SetBreakpointProperty {
        node_id: node_id.clone(),
        variant_id: variant_id.clone(),
        property: property.to_owned(),
        value: prior,
    })
}

fn set_breakpoint_override(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    variant_id: &ResponsiveVariantId,
    value: Option<&ResponsiveNodeOverride>,
) -> Result<Command, DesignerDiagnostic> {
    if !snapshot.design.responsive_variants.contains_key(variant_id) {
        return Err(node_diagnostic(
            "DESIGN_RESPONSIVE_VARIANT_MISSING",
            "the breakpoint override references an unknown responsive variant",
            node_id,
        ));
    }
    if value.is_some_and(|item| item.schema_version != STUDIO_DESIGN_SCHEMA_VERSION) {
        return Err(node_diagnostic(
            "DESIGN_RESPONSIVE_INVALID",
            "a breakpoint override has an unsupported schema version",
            node_id,
        ));
    }
    let node = snapshot
        .design
        .nodes
        .get_mut(node_id)
        .ok_or_else(|| missing_node(node_id))?;
    let prior = if let Some(value) = value {
        node.responsive_overrides
            .insert(variant_id.clone(), value.clone())
    } else {
        node.responsive_overrides.remove(variant_id)
    };
    Ok(Command::SetBreakpointOverride {
        node_id: node_id.clone(),
        variant_id: variant_id.clone(),
        value: prior.map(Box::new),
    })
}

fn set_layout(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    layout: &LayoutProperties,
) -> Result<Command, DesignerDiagnostic> {
    let node = snapshot
        .design
        .nodes
        .get_mut(node_id)
        .ok_or_else(|| missing_node(node_id))?;
    let prior = std::mem::replace(&mut node.layout, layout.clone());
    Ok(Command::SetLayout {
        node_id: node_id.clone(),
        layout: prior,
    })
}

fn set_responsive_layout(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    variant_id: &crate::ResponsiveVariantId,
    layout: &LayoutProperties,
) -> Result<Command, DesignerDiagnostic> {
    if !snapshot.design.responsive_variants.contains_key(variant_id) {
        return Err(node_diagnostic(
            "DESIGN_RESPONSIVE_VARIANT_MISSING",
            "the responsive layout references an unknown device profile",
            node_id,
        ));
    }
    let node = snapshot
        .design
        .nodes
        .get_mut(node_id)
        .ok_or_else(|| missing_node(node_id))?;
    let prior = node
        .responsive_overrides
        .get(variant_id)
        .map(|value| value.layout.clone());
    node.responsive_overrides
        .entry(variant_id.clone())
        .or_insert_with(|| crate::ResponsiveNodeOverride {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            properties: BTreeMap::new(),
            layout: LayoutProperties::default(),
            style: crate::StyleProperties::default(),
        })
        .layout = layout.clone();
    Ok(match prior {
        Some(prior) => Command::SetResponsiveLayout {
            node_id: node_id.clone(),
            variant_id: variant_id.clone(),
            layout: prior,
        },
        None => Command::RemoveResponsiveLayout {
            node_id: node_id.clone(),
            variant_id: variant_id.clone(),
        },
    })
}

fn remove_responsive_layout(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    variant_id: &crate::ResponsiveVariantId,
) -> Result<Command, DesignerDiagnostic> {
    let node = snapshot
        .design
        .nodes
        .get_mut(node_id)
        .ok_or_else(|| missing_node(node_id))?;
    let Some(override_value) = node.responsive_overrides.remove(variant_id) else {
        return Err(node_diagnostic(
            "DESIGN_RESPONSIVE_LAYOUT_MISSING",
            "the responsive layout override does not exist",
            node_id,
        ));
    };
    Ok(Command::SetResponsiveLayout {
        node_id: node_id.clone(),
        variant_id: variant_id.clone(),
        layout: override_value.layout,
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

fn define_responsive_variant(
    snapshot: &mut StudioDesignSnapshot,
    variant: &ResponsiveVariant,
) -> Result<Command, DesignerDiagnostic> {
    validate_responsive_variant(variant)?;
    if snapshot
        .design
        .responsive_variants
        .contains_key(&variant.id)
    {
        return Err(diagnostic(
            "DESIGN_RESPONSIVE_EXISTS",
            "the responsive variant identity already exists",
            None,
        ));
    }
    snapshot
        .design
        .responsive_variants
        .insert(variant.id.clone(), variant.clone());
    Ok(Command::RemoveResponsiveVariant {
        variant_id: variant.id.clone(),
    })
}

fn update_responsive_variant(
    snapshot: &mut StudioDesignSnapshot,
    variant: &ResponsiveVariant,
) -> Result<Command, DesignerDiagnostic> {
    validate_responsive_variant(variant)?;
    let prior = snapshot
        .design
        .responsive_variants
        .get(&variant.id)
        .cloned()
        .ok_or_else(|| {
            diagnostic(
                "DESIGN_RESPONSIVE_MISSING",
                "the responsive variant does not exist",
                None,
            )
        })?;
    snapshot
        .design
        .responsive_variants
        .insert(variant.id.clone(), variant.clone());
    Ok(Command::UpdateResponsiveVariant { variant: prior })
}

fn remove_responsive_variant(
    snapshot: &mut StudioDesignSnapshot,
    variant_id: &crate::ResponsiveVariantId,
) -> Result<Command, DesignerDiagnostic> {
    let variant = snapshot
        .design
        .responsive_variants
        .get(variant_id)
        .cloned()
        .ok_or_else(|| {
            diagnostic(
                "DESIGN_RESPONSIVE_MISSING",
                "the responsive variant does not exist",
                None,
            )
        })?;
    if snapshot
        .design
        .nodes
        .values()
        .any(|node| node.responsive_overrides.contains_key(variant_id))
    {
        return Err(diagnostic(
            "DESIGN_RESPONSIVE_IN_USE",
            "clear responsive overrides before removing the variant",
            None,
        ));
    }
    snapshot.design.responsive_variants.remove(variant_id);
    Ok(Command::DefineResponsiveVariant { variant })
}

fn set_responsive_override(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    variant_id: &crate::ResponsiveVariantId,
    value: Option<&ResponsiveNodeOverride>,
) -> Result<Command, DesignerDiagnostic> {
    if !snapshot.design.responsive_variants.contains_key(variant_id) {
        return Err(diagnostic(
            "DESIGN_RESPONSIVE_MISSING",
            "the responsive override references an unknown variant",
            None,
        ));
    }
    if let Some(value) = value {
        validate_responsive_override(&snapshot.design, node_id, value)?;
    }
    let node = snapshot
        .design
        .nodes
        .get_mut(node_id)
        .ok_or_else(|| missing_node(node_id))?;
    let prior = match value {
        Some(value) => node
            .responsive_overrides
            .insert(variant_id.clone(), value.clone()),
        None => node.responsive_overrides.remove(variant_id),
    };
    Ok(Command::SetResponsiveOverride {
        node_id: node_id.clone(),
        variant_id: variant_id.clone(),
        value: prior,
    })
}

fn define_token(
    snapshot: &mut StudioDesignSnapshot,
    token: &DesignToken,
) -> Result<Command, DesignerDiagnostic> {
    validate_token(token)?;
    if snapshot.design.tokens.contains_key(&token.id) {
        return Err(diagnostic(
            "DESIGN_TOKEN_EXISTS",
            "the token identity already exists",
            None,
        ));
    }
    snapshot
        .design
        .tokens
        .insert(token.id.clone(), token.clone());
    Ok(Command::RemoveToken {
        token_id: token.id.clone(),
    })
}

fn update_token(
    snapshot: &mut StudioDesignSnapshot,
    token: &DesignToken,
) -> Result<Command, DesignerDiagnostic> {
    validate_token(token)?;
    let prior = snapshot
        .design
        .tokens
        .get(&token.id)
        .cloned()
        .ok_or_else(|| diagnostic("DESIGN_TOKEN_MISSING", "the token does not exist", None))?;
    snapshot
        .design
        .tokens
        .insert(token.id.clone(), token.clone());
    Ok(Command::UpdateToken { token: prior })
}

fn remove_token(
    snapshot: &mut StudioDesignSnapshot,
    token_id: &TokenId,
) -> Result<Command, DesignerDiagnostic> {
    let token = snapshot
        .design
        .tokens
        .get(token_id)
        .cloned()
        .ok_or_else(|| diagnostic("DESIGN_TOKEN_MISSING", "the token does not exist", None))?;
    if design_references_token(&snapshot.design, token_id) {
        return Err(diagnostic(
            "DESIGN_TOKEN_IN_USE",
            "clear every token reference before removing the token",
            None,
        ));
    }
    snapshot.design.tokens.remove(token_id);
    Ok(Command::DefineToken { token })
}

fn set_binding(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    property: &str,
    binding: Option<&BindingPath>,
) -> Result<Command, DesignerDiagnostic> {
    if let Some(binding) = binding {
        validate_binding(binding, node_id)?;
    }
    set_property(
        snapshot,
        node_id,
        property,
        binding
            .map(|binding| PropertyValue::Binding(binding.clone()))
            .as_ref(),
    )
}

fn define_interaction(
    snapshot: &mut StudioDesignSnapshot,
    interaction: &Interaction,
) -> Result<Command, DesignerDiagnostic> {
    validate_interaction(&snapshot.design, interaction)?;
    if snapshot.design.interactions.contains_key(&interaction.id) {
        return Err(interaction_diagnostic(
            "DESIGN_INTERACTION_EXISTS",
            "the interaction identity already exists",
            &interaction.id,
        ));
    }
    attach_interaction(snapshot, interaction)?;
    snapshot
        .design
        .interactions
        .insert(interaction.id.clone(), interaction.clone());
    Ok(Command::RemoveInteraction {
        interaction_id: interaction.id.clone(),
    })
}

fn update_interaction(
    snapshot: &mut StudioDesignSnapshot,
    interaction: &Interaction,
) -> Result<Command, DesignerDiagnostic> {
    validate_interaction(&snapshot.design, interaction)?;
    let prior = snapshot
        .design
        .interactions
        .get(&interaction.id)
        .cloned()
        .ok_or_else(|| {
            interaction_diagnostic(
                "DESIGN_INTERACTION_MISSING",
                "the interaction does not exist",
                &interaction.id,
            )
        })?;
    detach_interaction(snapshot, &prior);
    attach_interaction(snapshot, interaction)?;
    snapshot
        .design
        .interactions
        .insert(interaction.id.clone(), interaction.clone());
    Ok(Command::UpdateInteraction { interaction: prior })
}

fn remove_interaction(
    snapshot: &mut StudioDesignSnapshot,
    interaction_id: &crate::InteractionId,
) -> Result<Command, DesignerDiagnostic> {
    let interaction = snapshot
        .design
        .interactions
        .get(interaction_id)
        .cloned()
        .ok_or_else(|| {
            interaction_diagnostic(
                "DESIGN_INTERACTION_MISSING",
                "the interaction does not exist",
                interaction_id,
            )
        })?;
    detach_interaction(snapshot, &interaction);
    snapshot.design.interactions.remove(interaction_id);
    Ok(Command::DefineInteraction { interaction })
}

fn define_composition(
    snapshot: &mut StudioDesignSnapshot,
    composition: &crate::ReusableComposition,
) -> Result<Command, DesignerDiagnostic> {
    validate_composition(&snapshot.design, composition)?;
    if snapshot.design.compositions.contains_key(&composition.id) {
        return Err(diagnostic(
            "DESIGN_COMPOSITION_EXISTS",
            "the composition identity already exists",
            None,
        ));
    }
    snapshot
        .design
        .compositions
        .insert(composition.id.clone(), composition.clone());
    Ok(Command::RemoveComposition {
        composition_id: composition.id.clone(),
    })
}

fn update_composition(
    snapshot: &mut StudioDesignSnapshot,
    composition: &crate::ReusableComposition,
) -> Result<Command, DesignerDiagnostic> {
    validate_composition(&snapshot.design, composition)?;
    let prior = snapshot
        .design
        .compositions
        .get(&composition.id)
        .cloned()
        .ok_or_else(|| {
            diagnostic(
                "DESIGN_COMPOSITION_MISSING",
                "the composition does not exist",
                None,
            )
        })?;
    if composition.root_node_id != prior.root_node_id {
        return Err(diagnostic(
            "DESIGN_COMPOSITION_ROOT_IMMUTABLE",
            "a composition definition cannot change its root identity",
            Some(prior.root_node_id),
        ));
    }
    if composition.definition_version == prior.definition_version {
        return Err(diagnostic(
            "DESIGN_COMPOSITION_VERSION_INVALID",
            "a composition update must change definition_version",
            Some(composition.root_node_id.clone()),
        ));
    }
    snapshot
        .design
        .compositions
        .insert(composition.id.clone(), composition.clone());
    for node in snapshot.design.nodes.values_mut() {
        if let DesignNodeSource::CompositionInstance {
            composition_id,
            definition_version,
            ..
        } = &mut node.source
            && composition_id == &composition.id
        {
            *definition_version = composition.definition_version;
        }
    }
    Ok(Command::UpdateComposition { composition: prior })
}

fn remove_composition(
    snapshot: &mut StudioDesignSnapshot,
    composition_id: &crate::CompositionId,
) -> Result<Command, DesignerDiagnostic> {
    let composition = snapshot
        .design
        .compositions
        .get(composition_id)
        .cloned()
        .ok_or_else(|| {
            diagnostic(
                "DESIGN_COMPOSITION_MISSING",
                "the composition does not exist",
                None,
            )
        })?;
    if snapshot.design.nodes.values().any(|node| {
        matches!(&node.source, DesignNodeSource::CompositionInstance { composition_id: id, .. } if id == composition_id)
    }) {
        return Err(diagnostic(
            "DESIGN_COMPOSITION_IN_USE",
            "remove every composition instance before removing the definition",
            Some(composition.root_node_id),
        ));
    }
    snapshot.design.compositions.remove(composition_id);
    Ok(Command::DefineComposition { composition })
}

fn instantiate_composition(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    name: &str,
    parent: &ParentPlacement,
    composition_id: &crate::CompositionId,
    inputs: &BTreeMap<String, PropertyValue>,
) -> Result<Command, DesignerDiagnostic> {
    if snapshot.design.nodes.contains_key(node_id) || id_is_tombstoned(snapshot, node_id) {
        return Err(node_diagnostic(
            "DESIGN_NODE_EXISTS",
            "the composition instance identity already exists",
            node_id,
        ));
    }
    let composition = snapshot
        .design
        .compositions
        .get(composition_id)
        .cloned()
        .ok_or_else(|| {
            diagnostic(
                "DESIGN_COMPOSITION_MISSING",
                "the composition does not exist",
                None,
            )
        })?;
    if name.trim().is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err(node_diagnostic(
            "DESIGN_NODE_NAME_INVALID",
            "a node name must contain 1..=256 safe bytes",
            node_id,
        ));
    }
    validate_composition_values(
        &snapshot.design,
        &composition,
        inputs,
        &BTreeMap::new(),
        node_id,
    )?;
    let node = DesignNode {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        id: node_id.clone(),
        name: name.to_owned(),
        source: DesignNodeSource::CompositionInstance {
            composition_id: composition.id,
            definition_version: composition.definition_version,
            inputs: inputs.clone(),
            admitted_overrides: BTreeMap::new(),
        },
        children: Vec::new(),
        properties: BTreeMap::new(),
        token_overrides: BTreeMap::new(),
        layout: Default::default(),
        style: Default::default(),
        accessibility: Default::default(),
        responsive_overrides: BTreeMap::new(),
        interaction_ids: Vec::new(),
    };
    insert_node(snapshot, parent, &node)
}

fn set_composition_input(
    snapshot: &mut StudioDesignSnapshot,
    node_id: &NodeId,
    input: &str,
    value: Option<&PropertyValue>,
    override_value: bool,
) -> Result<Command, DesignerDiagnostic> {
    if !valid_field_name(input) {
        return Err(node_diagnostic(
            "DESIGN_COMPOSITION_INPUT_INVALID",
            "a composition input name must contain 1..=128 safe bytes",
            node_id,
        ));
    }
    let composition_id = match snapshot.design.nodes.get(node_id).map(|node| &node.source) {
        Some(DesignNodeSource::CompositionInstance { composition_id, .. }) => {
            composition_id.clone()
        }
        Some(_) => {
            return Err(node_diagnostic(
                "DESIGN_COMPOSITION_INSTANCE_REQUIRED",
                "the command targets a primitive node, not a composition instance",
                node_id,
            ));
        }
        None => return Err(missing_node(node_id)),
    };
    let composition = snapshot
        .design
        .compositions
        .get(&composition_id)
        .cloned()
        .ok_or_else(|| {
            diagnostic(
                "DESIGN_COMPOSITION_MISSING",
                "the composition does not exist",
                None,
            )
        })?;
    let contract = composition.inputs.get(input).ok_or_else(|| {
        node_diagnostic(
            "DESIGN_COMPOSITION_INPUT_UNKNOWN",
            "the input is not declared by the composition contract",
            node_id,
        )
    })?;
    if override_value && !contract.overridable {
        return Err(node_diagnostic(
            "DESIGN_COMPOSITION_OVERRIDE_FORBIDDEN",
            "the composition contract does not admit an override for this input",
            node_id,
        ));
    }
    if let Some(value) = value {
        validate_property_value(&snapshot.design, value, Some(node_id))?;
        if !value_matches_kind(&snapshot.design, value, contract.value_kind) {
            return Err(node_diagnostic(
                "DESIGN_COMPOSITION_INPUT_TYPE",
                "the value does not match the composition input type",
                node_id,
            ));
        }
    }
    let node = snapshot
        .design
        .nodes
        .get_mut(node_id)
        .expect("composition instance was checked above");
    let DesignNodeSource::CompositionInstance {
        inputs,
        admitted_overrides,
        ..
    } = &mut node.source
    else {
        unreachable!("composition instance was checked above")
    };
    let target = if override_value {
        admitted_overrides
    } else {
        inputs
    };
    let prior = match value {
        Some(value) => target.insert(input.to_owned(), value.clone()),
        None => target.remove(input),
    };
    Ok(if override_value {
        Command::SetCompositionOverride {
            node_id: node_id.clone(),
            input: input.to_owned(),
            value: prior,
        }
    } else {
        Command::SetCompositionInput {
            node_id: node_id.clone(),
            input: input.to_owned(),
            value: prior,
        }
    })
}

fn validate_responsive_variant(variant: &ResponsiveVariant) -> Result<(), DesignerDiagnostic> {
    if variant.schema_version != STUDIO_DESIGN_SCHEMA_VERSION {
        return Err(diagnostic(
            "DESIGN_RESPONSIVE_INVALID",
            "the responsive variant schema version is unsupported",
            None,
        ));
    }
    if variant.name.trim().is_empty()
        || variant.name.len() > 128
        || variant.name.chars().any(char::is_control)
    {
        return Err(diagnostic(
            "DESIGN_RESPONSIVE_INVALID",
            "a responsive variant name must contain 1..=128 safe bytes",
            None,
        ));
    }
    if variant
        .minimum_width
        .zip(variant.maximum_width)
        .is_some_and(|(minimum, maximum)| minimum > maximum)
    {
        return Err(diagnostic(
            "DESIGN_RESPONSIVE_INVALID",
            "a responsive variant minimum width cannot exceed its maximum width",
            None,
        ));
    }
    Ok(())
}

fn validate_responsive_override(
    design: &StudioDesign,
    node_id: &NodeId,
    value: &ResponsiveNodeOverride,
) -> Result<(), DesignerDiagnostic> {
    if value.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
        || value.layout.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
        || value.style.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
    {
        return Err(node_diagnostic(
            "DESIGN_RESPONSIVE_INVALID",
            "a responsive override has an unsupported schema version",
            node_id,
        ));
    }
    for (property, property_value) in &value.properties {
        if !valid_field_name(property) {
            return Err(node_diagnostic(
                "DESIGN_PROPERTY_INVALID",
                "a responsive property name must contain 1..=128 safe bytes",
                node_id,
            ));
        }
        validate_property_value(design, property_value, Some(node_id))?;
    }
    validate_layout_style(design, &value.layout, &value.style, node_id)?;
    Ok(())
}

fn validate_layout_style(
    design: &StudioDesign,
    layout: &crate::LayoutProperties,
    style: &crate::StyleProperties,
    node_id: &NodeId,
) -> Result<(), DesignerDiagnostic> {
    if layout.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
        || style.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
    {
        return Err(node_diagnostic(
            "DESIGN_STYLE_SCHEMA_INVALID",
            "layout and style values must use the supported schema version",
            node_id,
        ));
    }
    for paint in [&style.background, &style.foreground, &style.border_color]
        .into_iter()
        .flatten()
    {
        if let crate::Paint::Token(token_id) = paint
            && !design.tokens.contains_key(token_id)
        {
            return Err(node_diagnostic(
                "DESIGN_TOKEN_MISSING",
                "a style references an unknown token",
                node_id,
            ));
        }
    }
    Ok(())
}

fn validate_binding(binding: &BindingPath, node_id: &NodeId) -> Result<(), DesignerDiagnostic> {
    if !valid_field_name(&binding.collection)
        || binding.segments.is_empty()
        || binding
            .segments
            .iter()
            .any(|segment| !valid_field_name(segment))
    {
        return Err(node_diagnostic(
            "DESIGN_BINDING_INVALID",
            "a binding requires a safe collection and at least one safe path segment",
            node_id,
        ));
    }
    Ok(())
}

fn validate_property_value(
    design: &StudioDesign,
    value: &PropertyValue,
    node_id: Option<&NodeId>,
) -> Result<(), DesignerDiagnostic> {
    let error = |code: &str, message: &str| diagnostic(code, message, node_id.cloned());
    match value {
        PropertyValue::Token(token_id) if !design.tokens.contains_key(token_id) => {
            return Err(error(
                "DESIGN_TOKEN_MISSING",
                "a property references an unknown token",
            ));
        }
        PropertyValue::Binding(binding) => {
            validate_binding(
                binding,
                node_id.unwrap_or(&NodeId::new("binding").expect("literal identity")),
            )?;
        }
        PropertyValue::Node(target) if !design.nodes.contains_key(target) => {
            return Err(error(
                "DESIGN_NODE_MISSING",
                "a property references an unknown node",
            ));
        }
        PropertyValue::List(values) => {
            for value in values {
                validate_property_value(design, value, node_id)?;
            }
        }
        PropertyValue::String(_)
        | PropertyValue::Boolean(_)
        | PropertyValue::Integer(_)
        | PropertyValue::Decimal(_)
        | PropertyValue::Length(_)
        | PropertyValue::Color(_)
        | PropertyValue::Token(_)
        | PropertyValue::Node(_)
        | PropertyValue::Asset(_) => {}
    }
    Ok(())
}

fn value_matches_kind(design: &StudioDesign, value: &PropertyValue, kind: ValueKind) -> bool {
    matches!(
        (value, kind),
        (PropertyValue::String(_), ValueKind::String)
            | (PropertyValue::Boolean(_), ValueKind::Boolean)
            | (PropertyValue::Integer(_), ValueKind::Integer)
            | (PropertyValue::Decimal(_), ValueKind::Decimal)
            | (PropertyValue::Length(_), ValueKind::Length)
            | (PropertyValue::Color(_), ValueKind::Color)
            | (PropertyValue::Token(_), ValueKind::Token)
            | (PropertyValue::Binding(_), ValueKind::Binding)
            | (PropertyValue::Node(_), ValueKind::Node)
            | (PropertyValue::Asset(_), ValueKind::Asset)
            | (PropertyValue::List(_), ValueKind::List)
    ) && (!matches!(value, PropertyValue::Token(token_id) if !design.tokens.contains_key(token_id)))
}

fn validate_interaction(
    design: &StudioDesign,
    interaction: &Interaction,
) -> Result<(), DesignerDiagnostic> {
    if interaction.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
        || !design.nodes.contains_key(&interaction.source.node_id)
    {
        return Err(interaction_diagnostic(
            "DESIGN_INTERACTION_INVALID",
            "an interaction requires a valid source node and schema version",
            &interaction.id,
        ));
    }
    validate_interaction_action(design, &interaction.id, &interaction.action)
}

fn validate_interaction_action(
    design: &StudioDesign,
    interaction_id: &InteractionId,
    action: &InteractionAction,
) -> Result<(), DesignerDiagnostic> {
    match action {
        InteractionAction::Navigate { screen_id, .. }
            if !design.screens.contains_key(screen_id) =>
        {
            Err(interaction_diagnostic(
                "DESIGN_INTERACTION_TARGET_MISSING",
                "a navigation interaction references an unknown screen",
                interaction_id,
            ))
        }
        InteractionAction::SetProperty {
            node_id,
            property,
            value,
        } => {
            if !design.nodes.contains_key(node_id) {
                return Err(interaction_diagnostic(
                    "DESIGN_INTERACTION_TARGET_MISSING",
                    "a property interaction references an unknown node",
                    interaction_id,
                ));
            }
            if !valid_field_name(property) {
                return Err(interaction_diagnostic(
                    "DESIGN_PROPERTY_INVALID",
                    "an interaction property name must contain 1..=128 safe bytes",
                    interaction_id,
                ));
            }
            validate_property_value(design, value, None).map_err(|_| {
                interaction_diagnostic(
                    "DESIGN_INTERACTION_VALUE_INVALID",
                    "an interaction property value is invalid",
                    interaction_id,
                )
            })
        }
        InteractionAction::ToggleVisibility { node_id } if !design.nodes.contains_key(node_id) => {
            Err(interaction_diagnostic(
                "DESIGN_INTERACTION_TARGET_MISSING",
                "a visibility interaction references an unknown node",
                interaction_id,
            ))
        }
        InteractionAction::Emit { event } if !valid_field_name(event) => {
            Err(interaction_diagnostic(
                "DESIGN_INTERACTION_EVENT_INVALID",
                "an emitted event name must contain 1..=128 safe bytes",
                interaction_id,
            ))
        }
        InteractionAction::Sequence { interaction_ids } => {
            if interaction_ids.iter().any(|id| id == interaction_id)
                || interaction_ids
                    .iter()
                    .any(|id| !design.interactions.contains_key(id))
            {
                return Err(interaction_diagnostic(
                    "DESIGN_INTERACTION_TARGET_MISSING",
                    "an interaction sequence contains an unknown or self-referencing interaction",
                    interaction_id,
                ));
            }
            Ok(())
        }
        InteractionAction::Navigate { .. }
        | InteractionAction::ToggleVisibility { .. }
        | InteractionAction::Emit { .. } => Ok(()),
    }
}

fn attach_interaction(
    snapshot: &mut StudioDesignSnapshot,
    interaction: &Interaction,
) -> Result<(), DesignerDiagnostic> {
    let node = snapshot
        .design
        .nodes
        .get_mut(&interaction.source.node_id)
        .ok_or_else(|| missing_node(&interaction.source.node_id))?;
    if !node.interaction_ids.contains(&interaction.id) {
        node.interaction_ids.push(interaction.id.clone());
    }
    Ok(())
}

fn detach_interaction(snapshot: &mut StudioDesignSnapshot, interaction: &Interaction) {
    if let Some(node) = snapshot.design.nodes.get_mut(&interaction.source.node_id) {
        node.interaction_ids.retain(|id| id != &interaction.id);
    }
}

fn validate_composition(
    design: &StudioDesign,
    composition: &crate::ReusableComposition,
) -> Result<(), DesignerDiagnostic> {
    if composition.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
        || !design.nodes.contains_key(&composition.root_node_id)
        || design.parents.get(&composition.root_node_id)
            != Some(&NodeParent::Composition {
                composition_id: composition.id.clone(),
            })
    {
        return Err(diagnostic(
            "DESIGN_COMPOSITION_INVALID",
            "a composition requires a valid composition-owned root node",
            Some(composition.root_node_id.clone()),
        ));
    }
    if composition.name.trim().is_empty()
        || composition.name.len() > 128
        || composition.name.chars().any(char::is_control)
        || composition.definition_version == 0
    {
        return Err(diagnostic(
            "DESIGN_COMPOSITION_INVALID",
            "a composition name must be safe and its definition version non-zero",
            Some(composition.root_node_id.clone()),
        ));
    }
    for input in composition.inputs.values() {
        if let Some(default) = &input.default {
            validate_property_value(design, default, Some(&composition.root_node_id))?;
            if !value_matches_kind(design, default, input.value_kind) {
                return Err(diagnostic(
                    "DESIGN_COMPOSITION_INPUT_TYPE",
                    "a composition input default does not match its declared type",
                    Some(composition.root_node_id.clone()),
                ));
            }
        }
    }
    if composition
        .inputs
        .keys()
        .any(|name| !valid_field_name(name))
        || composition.slots.keys().any(|name| !valid_field_name(name))
    {
        return Err(diagnostic(
            "DESIGN_COMPOSITION_CONTRACT_INVALID",
            "composition input and slot names must contain 1..=128 safe bytes",
            Some(composition.root_node_id.clone()),
        ));
    }
    Ok(())
}

fn validate_composition_values(
    design: &StudioDesign,
    composition: &crate::ReusableComposition,
    inputs: &BTreeMap<String, PropertyValue>,
    overrides: &BTreeMap<String, PropertyValue>,
    node_id: &NodeId,
) -> Result<(), DesignerDiagnostic> {
    for key in inputs.keys().chain(overrides.keys()) {
        if !valid_field_name(key) {
            return Err(node_diagnostic(
                "DESIGN_COMPOSITION_INPUT_INVALID",
                "a composition input name must contain 1..=128 safe bytes",
                node_id,
            ));
        }
    }
    for (key, value) in inputs {
        let Some(contract) = composition.inputs.get(key) else {
            return Err(node_diagnostic(
                "DESIGN_COMPOSITION_INPUT_UNKNOWN",
                "the instance supplies an input not declared by the composition",
                node_id,
            ));
        };
        validate_property_value(design, value, Some(node_id))?;
        if !value_matches_kind(design, value, contract.value_kind) {
            return Err(node_diagnostic(
                "DESIGN_COMPOSITION_INPUT_TYPE",
                "an instance input does not match the composition contract",
                node_id,
            ));
        }
    }
    for (key, value) in overrides {
        let Some(contract) = composition.inputs.get(key) else {
            return Err(node_diagnostic(
                "DESIGN_COMPOSITION_INPUT_UNKNOWN",
                "the instance overrides an input not declared by the composition",
                node_id,
            ));
        };
        if !contract.overridable {
            return Err(node_diagnostic(
                "DESIGN_COMPOSITION_OVERRIDE_FORBIDDEN",
                "the composition contract does not admit an override for this input",
                node_id,
            ));
        }
        validate_property_value(design, value, Some(node_id))?;
        if !value_matches_kind(design, value, contract.value_kind) {
            return Err(node_diagnostic(
                "DESIGN_COMPOSITION_INPUT_TYPE",
                "an instance override does not match the composition contract",
                node_id,
            ));
        }
    }
    for (key, contract) in &composition.inputs {
        if contract.required && contract.default.is_none() && !inputs.contains_key(key) {
            return Err(node_diagnostic(
                "DESIGN_COMPOSITION_INPUT_REQUIRED",
                "a required composition input has no supplied value or default",
                node_id,
            ));
        }
    }
    Ok(())
}

fn design_references_token(design: &StudioDesign, token_id: &TokenId) -> bool {
    design.nodes.values().any(|node| {
        node.properties
            .values()
            .any(|value| property_contains_token(value, token_id))
            || [&node.style.background, &node.style.foreground, &node.style.border_color]
                .into_iter()
                .flatten()
                .any(|paint| matches!(paint, crate::Paint::Token(id) if id == token_id))
            || node
                .responsive_overrides
                .values()
                .any(|value| {
                    value
                        .properties
                        .values()
                        .any(|value| property_contains_token(value, token_id))
                        || [&value.style.background, &value.style.foreground, &value.style.border_color]
                            .into_iter()
                            .flatten()
                            .any(|paint| matches!(paint, crate::Paint::Token(id) if id == token_id))
                })
            || matches!(&node.source, DesignNodeSource::CompositionInstance { inputs, admitted_overrides, .. } if inputs.values().any(|value| property_contains_token(value, token_id)) || admitted_overrides.values().any(|value| property_contains_token(value, token_id)))
    }) || design.compositions.values().any(|composition| {
        composition
            .inputs
            .values()
            .filter_map(|input| input.default.as_ref())
            .any(|value| property_contains_token(value, token_id))
    })
}

fn property_contains_token(value: &PropertyValue, token_id: &TokenId) -> bool {
    match value {
        PropertyValue::Token(id) => id == token_id,
        PropertyValue::List(values) => values
            .iter()
            .any(|value| property_contains_token(value, token_id)),
        PropertyValue::String(_)
        | PropertyValue::Boolean(_)
        | PropertyValue::Integer(_)
        | PropertyValue::Decimal(_)
        | PropertyValue::Length(_)
        | PropertyValue::Color(_)
        | PropertyValue::Binding(_)
        | PropertyValue::Node(_)
        | PropertyValue::Asset(_) => false,
    }
}

fn insert_child(
fn create_collection(
    snapshot: &mut StudioDesignSnapshot,
    collection: &ContentCollection,
) -> Result<Command, DesignerDiagnostic> {
    if snapshot.design.collections.contains_key(&collection.id) {
        return Err(collection_diagnostic(
            &collection.id,
            "CONTENT_COLLECTION_EXISTS",
            "a collection with this identity already exists",
        ));
    }
    validate_content_collection(collection)?;
    snapshot
        .design
        .collections
        .insert(collection.id.clone(), collection.clone());
    Ok(Command::DeleteCollection {
        collection_id: collection.id.clone(),
    })
}

fn update_collection_schema(
    snapshot: &mut StudioDesignSnapshot,
    collection_id: &CollectionId,
    schema: &ContentCollectionSchema,
) -> Result<Command, DesignerDiagnostic> {
    let diagnostics = validate_collection_schema(schema);
    if let Some(first) = diagnostics
        .into_iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
    {
        return Err(first);
    }
    let collection = snapshot
        .design
        .collections
        .get_mut(collection_id)
        .ok_or_else(|| {
            collection_diagnostic(
                collection_id,
                "CONTENT_COLLECTION_MISSING",
                "the collection does not exist",
            )
        })?;
    // Check existing records against new schema.
    for record in collection.records.values() {
        let temp = ContentCollection {
            schema: schema.clone(),
            ..collection.clone()
        };
        let record_diags = validate_record(&temp, record);
        if record_diags
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
        {
            return Err(collection_diagnostic(
                collection_id,
                "CONTENT_SCHEMA_BREAKS_RECORDS",
                format!(
                    "new schema would invalidate record '{}': {}",
                    record.id, record_diags[0].message
                ),
            ));
        }
    }
    let prior = std::mem::replace(&mut collection.schema, schema.clone());
    // Update fixture edge records validation if needed.
    let _ = validate_fixture(&collection.fixture);
    Ok(Command::UpdateCollectionSchema {
        collection_id: collection_id.clone(),
        schema: prior,
    })
}

fn delete_collection(
    snapshot: &mut StudioDesignSnapshot,
    collection_id: &CollectionId,
) -> Result<Command, DesignerDiagnostic> {
    let collection = snapshot
        .design
        .collections
        .remove(collection_id)
        .ok_or_else(|| {
            collection_diagnostic(
                collection_id,
                "CONTENT_COLLECTION_MISSING",
                "the collection does not exist",
            )
        })?;
    // Remove bindings that pointed at this collection; inverse will restore them.
    let removed_bindings = snapshot
        .design
        .bindings
        .iter()
        .filter(|(_, b)| &b.source.collection_id == collection_id)
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    for binding_id in &removed_bindings {
        snapshot.design.bindings.remove(binding_id);
    }
    // Deleting a collection implicitly deletes its records (they live inside the collection).
    // Inverse restores the full collection snapshot.
    let _ = removed_bindings;
    Ok(Command::CreateCollection { collection })
}

fn create_record(
    snapshot: &mut StudioDesignSnapshot,
    collection_id: &CollectionId,
    record: &ContentRecord,
) -> Result<Command, DesignerDiagnostic> {
    let collection = snapshot
        .design
        .collections
        .get_mut(collection_id)
        .ok_or_else(|| {
            collection_diagnostic(
                collection_id,
                "CONTENT_COLLECTION_MISSING",
                "the collection does not exist",
            )
        })?;
    if collection.records.contains_key(&record.id) {
        return Err(record_diagnostic_for(
            collection_id,
            &record.id,
            "CONTENT_RECORD_EXISTS",
            "a record with this identity already exists",
        ));
    }
    let diagnostics = validate_record(collection, record);
    if let Some(first) = diagnostics
        .into_iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
    {
        return Err(first);
    }
    collection.records.insert(record.id.clone(), record.clone());
    Ok(Command::DeleteRecord {
        collection_id: collection_id.clone(),
        record_id: record.id.clone(),
    })
}

fn update_record(
    snapshot: &mut StudioDesignSnapshot,
    collection_id: &CollectionId,
    record_id: &RecordId,
    values: &BTreeMap<String, PropertyValue>,
) -> Result<Command, DesignerDiagnostic> {
    // Validate against an immutable view first to avoid borrow conflicts.
    let collection_view = snapshot
        .design
        .collections
        .get(collection_id)
        .ok_or_else(|| {
            collection_diagnostic(
                collection_id,
                "CONTENT_COLLECTION_MISSING",
                "the collection does not exist",
            )
        })?;
    let existing = collection_view.records.get(record_id).ok_or_else(|| {
        record_diagnostic_for(
            collection_id,
            record_id,
            "CONTENT_RECORD_MISSING",
            "the record does not exist",
        )
    })?;
    let candidate = ContentRecord {
        schema_version: existing.schema_version,
        id: record_id.clone(),
        values: values.clone(),
    };
    let diagnostics = validate_record(collection_view, &candidate);
    if let Some(first) = diagnostics
        .into_iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
    {
        return Err(first);
    }
    let collection = snapshot
        .design
        .collections
        .get_mut(collection_id)
        .expect("collection exists after validation");
    let record = collection
        .records
        .get_mut(record_id)
        .expect("record exists after validation");
    let prior = std::mem::replace(&mut record.values, values.clone());
    Ok(Command::UpdateRecord {
        collection_id: collection_id.clone(),
        record_id: record_id.clone(),
        values: prior,
    })
}

fn delete_record(
    snapshot: &mut StudioDesignSnapshot,
    collection_id: &CollectionId,
    record_id: &RecordId,
) -> Result<Command, DesignerDiagnostic> {
    let collection = snapshot
        .design
        .collections
        .get_mut(collection_id)
        .ok_or_else(|| {
            collection_diagnostic(
                collection_id,
                "CONTENT_COLLECTION_MISSING",
                "the collection does not exist",
            )
        })?;
    let record = collection.records.remove(record_id).ok_or_else(|| {
        record_diagnostic_for(
            collection_id,
            record_id,
            "CONTENT_RECORD_MISSING",
            "the record does not exist",
        )
    })?;
    Ok(Command::CreateRecord {
        collection_id: collection_id.clone(),
        record,
    })
}

fn set_fixture(
    snapshot: &mut StudioDesignSnapshot,
    collection_id: &CollectionId,
    fixture: &ContentFixture,
) -> Result<Command, DesignerDiagnostic> {
    let diagnostics = validate_fixture(fixture);
    if let Some(first) = diagnostics
        .into_iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
    {
        return Err(first);
    }
    let collection = snapshot
        .design
        .collections
        .get_mut(collection_id)
        .ok_or_else(|| {
            collection_diagnostic(
                collection_id,
                "CONTENT_COLLECTION_MISSING",
                "the collection does not exist",
            )
        })?;
    // Validate edge records against schema when fixture is Edge.
    if fixture.edge_records.iter().any(|r| {
        let diags = validate_record(collection, r);
        diags
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }) {
        return Err(collection_diagnostic(
            collection_id,
            "CONTENT_FIXTURE_EDGE_INVALID",
            "an edge record does not match the collection schema",
        ));
    }
    let prior = std::mem::replace(&mut collection.fixture, fixture.clone());
    Ok(Command::SetFixture {
        collection_id: collection_id.clone(),
        fixture: prior,
    })
}

fn upsert_binding(
    snapshot: &mut StudioDesignSnapshot,
    binding: &ContentBinding,
) -> Result<Command, DesignerDiagnostic> {
    let shape_diags = validate_binding_shape(binding);
    if let Some(first) = shape_diags
        .into_iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
    {
        return Err(first);
    }
    if !snapshot.design.nodes.contains_key(&binding.node_id) {
        return Err(node_diagnostic(
            "CONTENT_BINDING_NODE_MISSING",
            "binding target node does not exist",
            &binding.node_id,
        ));
    }
    if !valid_field_name(&binding.property) {
        return Err(node_diagnostic(
            "CONTENT_BINDING_PROPERTY_INVALID",
            "binding property name is invalid",
            &binding.node_id,
        ));
    }
    let prior = snapshot
        .design
        .bindings
        .insert(binding.id.clone(), binding.clone());
    if let Some(prior_binding) = prior {
        Ok(Command::UpsertBinding {
            binding: prior_binding,
        })
    } else {
        Ok(Command::RemoveBinding {
            binding_id: binding.id.clone(),
        })
    }
}

fn remove_binding(
    snapshot: &mut StudioDesignSnapshot,
    binding_id: &BindingId,
) -> Result<Command, DesignerDiagnostic> {
    let binding =
        snapshot
            .design
            .bindings
            .remove(binding_id)
            .ok_or_else(|| DesignerDiagnostic {
                code: "CONTENT_BINDING_MISSING".to_owned(),
                severity: DiagnosticSeverity::Error,
                message: "the binding does not exist".to_owned(),
                node_id: None,
                interaction_id: None,
                collection_id: None,
                binding_id: Some(binding_id.clone()),
                form_id: None,
                record_id: None,
            })?;
    Ok(Command::UpsertBinding { binding })
}

fn upsert_form(
    snapshot: &mut StudioDesignSnapshot,
    form: &FormDefinition,
) -> Result<Command, DesignerDiagnostic> {
    let diagnostics = validate_form_shape(form);
    if let Some(first) = diagnostics
        .into_iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
    {
        return Err(first);
    }
    if let Some(target) = &form.target_collection_id
        && !snapshot.design.collections.contains_key(target)
    {
        return Err(collection_diagnostic(
            target,
            "FORM_TARGET_COLLECTION_MISSING",
            "form target collection does not exist",
        ));
    }
    let prior = snapshot.design.forms.insert(form.id.clone(), form.clone());
    if let Some(prior_form) = prior {
        Ok(Command::UpsertForm { form: prior_form })
    } else {
        Ok(Command::RemoveForm {
            form_id: form.id.clone(),
        })
    }
}

fn remove_form(
    snapshot: &mut StudioDesignSnapshot,
    form_id: &FormId,
) -> Result<Command, DesignerDiagnostic> {
    let form = snapshot
        .design
        .forms
        .remove(form_id)
        .ok_or_else(|| DesignerDiagnostic {
            code: "FORM_MISSING".to_owned(),
            severity: DiagnosticSeverity::Error,
            message: "the form does not exist".to_owned(),
            node_id: None,
            interaction_id: None,
            collection_id: None,
            binding_id: None,
            form_id: Some(form_id.clone()),
            record_id: None,
        })?;
    Ok(Command::UpsertForm { form })
}

fn validate_content_collection(collection: &ContentCollection) -> Result<(), DesignerDiagnostic> {
    if collection.schema_version != STUDIO_DESIGN_SCHEMA_VERSION {
        return Err(collection_diagnostic(
            &collection.id,
            "CONTENT_COLLECTION_SCHEMA_INVALID",
            "collection has an unsupported schema version",
        ));
    }
    if collection.name.trim().is_empty()
        || collection.name.len() > 256
        || collection.name.chars().any(char::is_control)
    {
        return Err(collection_diagnostic(
            &collection.id,
            "CONTENT_COLLECTION_NAME_INVALID",
            "collection name must be 1..=256 safe bytes",
        ));
    }
    let schema_diags = validate_collection_schema(&collection.schema);
    if let Some(first) = schema_diags
        .into_iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
    {
        return Err(DesignerDiagnostic {
            collection_id: Some(collection.id.clone()),
            ..first
        });
    }
    let fixture_diags = validate_fixture(&collection.fixture);
    if let Some(first) = fixture_diags
        .into_iter()
        .find(|d| d.severity == DiagnosticSeverity::Error)
    {
        return Err(DesignerDiagnostic {
            collection_id: Some(collection.id.clone()),
            ..first
        });
    }
    for record in collection.records.values() {
        let diags = validate_record(collection, record);
        if let Some(first) = diags
            .into_iter()
            .find(|d| d.severity == DiagnosticSeverity::Error)
        {
            return Err(first);
        }
    }
    if collection.fixture.edge_records.iter().any(|r| {
        let diags = validate_record(collection, r);
        diags
            .iter()
            .any(|d| d.severity == DiagnosticSeverity::Error)
    }) {
        return Err(collection_diagnostic(
            &collection.id,
            "CONTENT_FIXTURE_EDGE_INVALID",
            "an edge record does not match the collection schema",
        ));
    }
    Ok(())
}

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
    let mut diagnostics = snapshot
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
            collection_id: None,
            binding_id: None,
            form_id: None,
            record_id: None,
        })
        .collect::<Vec<_>>();
    diagnostics.extend(binding_diagnostics(&snapshot.design));
    diagnostics
}

#[allow(clippy::too_many_lines)]
fn validate_snapshot(snapshot: &StudioDesignSnapshot) -> Vec<DesignerDiagnostic> {
    let mut diagnostics = validate_design(&snapshot.design);
    let dangling_interactions = snapshot
        .tombstones
        .values()
        .flat_map(|tombstone| &tombstone.references)
        .filter_map(|reference| reference.owner.strip_prefix("interaction:"))
        .collect::<BTreeSet<_>>();
    diagnostics.retain(|diagnostic| {
        !(matches!(
            diagnostic.code.as_str(),
            "DESIGN_INTERACTION_INVALID" | "DESIGN_INTERACTION_TARGET_MISSING"
        ) && diagnostic
            .interaction_id
            .as_ref()
            .is_some_and(|id| dangling_interactions.contains(id.as_str())))
    });
    diagnostics
}

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
    diagnostics.extend(validate_layout(design));
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
        if let DesignNodeSource::CompositionInstance {
            composition_id,
            definition_version,
            inputs,
            admitted_overrides,
        } = &node.source
        {
            match design.compositions.get(composition_id) {
                None => diagnostics.push(node_diagnostic(
                    "DESIGN_COMPOSITION_MISSING",
                    "a composition instance references an unknown definition",
                    node_id,
                )),
                Some(composition) => {
                    if *definition_version != composition.definition_version {
                        diagnostics.push(node_diagnostic(
                            "DESIGN_COMPOSITION_VERSION_STALE",
                            "a composition instance does not match its definition version",
                            node_id,
                        ));
                    }
                    if let Err(error) = validate_composition_values(
                        design,
                        composition,
                        inputs,
                        admitted_overrides,
                        node_id,
                    ) {
                        diagnostics.push(error);
                    }
                }
            }
        }
        for (variant_id, value) in &node.responsive_overrides {
            if !design.responsive_variants.contains_key(variant_id) {
                diagnostics.push(node_diagnostic(
                    "DESIGN_RESPONSIVE_INVALID",
                    "a responsive override references an unknown variant",
                    node_id,
                ));
            } else if let Err(error) = validate_responsive_override(design, node_id, value) {
                diagnostics.push(error);
            }
            for property in value.properties.keys() {
                if !valid_field_name(property) {
                    diagnostics.push(node_diagnostic(
                        "DESIGN_PROPERTY_INVALID",
                        "a responsive property name must contain 1..=128 safe bytes",
                        node_id,
                    ));
                }
            }
        }
        if let Err(error) = validate_layout_style(design, &node.layout, &node.style, node_id) {
            diagnostics.push(error);
        }
        for (property, value) in &node.properties {
            if let Err(error) = validate_property_value(design, value, Some(node_id)) {
                diagnostics.push(DesignerDiagnostic {
                    message: format!("invalid node property {property}: {}", error.message),
                    ..error
                });
            }
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
        for interaction_id in &node.interaction_ids {
            if let Some(interaction) = design.interactions.get(interaction_id)
                && interaction.source.node_id != *node_id
            {
                diagnostics.push(node_diagnostic(
                    "DESIGN_INTERACTION_SOURCE_INVALID",
                    "a node interaction attachment does not match the interaction source",
                    node_id,
                ));
            }
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
        } else if let Err(error) = validate_token(token) {
            diagnostics.push(error);
        }
    }
    for (variant_id, variant) in &design.responsive_variants {
        if variant.schema_version != STUDIO_DESIGN_SCHEMA_VERSION || variant.id != *variant_id {
            diagnostics.push(diagnostic(
                "DESIGN_RESPONSIVE_INVALID",
                "a responsive variant identity, bounds, or schema version is invalid",
                None,
            ));
        } else if let Err(error) = validate_responsive_variant(variant) {
            diagnostics.push(error);
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
        } else if let Err(error) = validate_interaction(design, interaction) {
            diagnostics.push(error);
        }
    }
    for (collection_id, collection) in &design.collections {
        if collection.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
            || collection.id != *collection_id
        {
            diagnostics.push(collection_diagnostic(
                collection_id,
                "CONTENT_COLLECTION_SCHEMA_INVALID",
                "a collection identity or schema version is invalid",
            ));
        }
        diagnostics.extend(
            validate_collection_schema(&collection.schema)
                .into_iter()
                .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
                .map(|diagnostic| DesignerDiagnostic {
                    collection_id: Some(collection_id.clone()),
                    ..diagnostic
                }),
        );
        diagnostics.extend(
            validate_fixture(&collection.fixture)
                .into_iter()
                .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
                .map(|diagnostic| DesignerDiagnostic {
                    collection_id: Some(collection_id.clone()),
                    ..diagnostic
                }),
        );
        for (record_id, record) in &collection.records {
            if record.id != *record_id {
                diagnostics.push(record_diagnostic_for(
                    collection_id,
                    record_id,
                    "CONTENT_RECORD_ID_MISMATCH",
                    "a record identity does not match its map key",
                ));
            }
            diagnostics.extend(validate_record(collection, record));
        }
        for edge in &collection.fixture.edge_records {
            diagnostics.extend(validate_record(collection, edge));
        }
    }
    for (binding_id, binding) in &design.bindings {
        if binding.schema_version != STUDIO_DESIGN_SCHEMA_VERSION || binding.id != *binding_id {
            diagnostics.push(DesignerDiagnostic {
                code: "CONTENT_BINDING_SCHEMA_INVALID".to_owned(),
                severity: DiagnosticSeverity::Error,
                message: "a binding identity or schema version is invalid".to_owned(),
                node_id: Some(binding.node_id.clone()),
                interaction_id: None,
                collection_id: Some(binding.source.collection_id.clone()),
                binding_id: Some(binding_id.clone()),
                form_id: None,
                record_id: None,
            });
        }
        diagnostics.extend(
            validate_binding_shape(binding)
                .into_iter()
                .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error),
        );
    }
    diagnostics.extend(binding_diagnostics(design));
    for (form_id, form) in &design.forms {
        if form.schema_version != STUDIO_DESIGN_SCHEMA_VERSION || form.id != *form_id {
            diagnostics.push(DesignerDiagnostic {
                code: "FORM_SCHEMA_INVALID".to_owned(),
                severity: DiagnosticSeverity::Error,
                message: "a form identity or schema version is invalid".to_owned(),
                node_id: None,
                interaction_id: None,
                collection_id: form.target_collection_id.clone(),
                binding_id: None,
                form_id: Some(form_id.clone()),
                record_id: None,
            });
        }
        diagnostics.extend(
            validate_form_shape(form)
                .into_iter()
                .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error),
        );
    }
    diagnostics
}

/// Validate layout, sizing, constraints, and overlay placement semantics.
///
/// This is public so inspector and canvas code can explain a rejected edit
/// before submitting a batch. Diagnostics are ordered by stable node identity
/// and always carry the affected node for selection/focus routing.
#[must_use]
pub fn validate_layout(design: &StudioDesign) -> Vec<DesignerDiagnostic> {
    let mut diagnostics = Vec::new();
    for (node_id, node) in &design.nodes {
        diagnostics.extend(validate_layout_for_node(design, node_id, node));
        for (variant_id, override_value) in &node.responsive_overrides {
            if design.responsive_variants.contains_key(variant_id) {
                diagnostics.extend(validate_layout_for_node(
                    design,
                    node_id,
                    &DesignNode {
                        layout: override_value.layout.clone(),
                        ..node.clone()
                    },
                ));
            }
        }
    }
    diagnostics
}

#[allow(clippy::too_many_lines)]
fn validate_layout_for_node(
    design: &StudioDesign,
    node_id: &NodeId,
    node: &DesignNode,
) -> Vec<DesignerDiagnostic> {
    let layout = &node.layout;
    let mut diagnostics = Vec::new();
    if layout.schema_version != STUDIO_DESIGN_SCHEMA_VERSION {
        diagnostics.push(node_diagnostic(
            "DESIGN_LAYOUT_SCHEMA_INVALID",
            "the layout schema version is unsupported",
            node_id,
        ));
    }
    if let Some(position) = &layout.position
        && position.schema_version != STUDIO_DESIGN_SCHEMA_VERSION
    {
        diagnostics.push(node_diagnostic(
            "DESIGN_LAYOUT_POSITION_SCHEMA_INVALID",
            "the positioned layout schema version is unsupported",
            node_id,
        ));
    }

    for (name, value, allow_auto) in [
        ("width", &layout.width, true),
        ("height", &layout.height, true),
        ("min_width", &layout.min_width, false),
        ("max_width", &layout.max_width, false),
        ("min_height", &layout.min_height, false),
        ("max_height", &layout.max_height, false),
        ("gap", &layout.gap, false),
        ("row_gap", &layout.row_gap, false),
        ("column_gap", &layout.column_gap, false),
        ("padding", &layout.padding, false),
    ] {
        if let Some(value) = value
            && (!valid_length(value, allow_auto, true)
                || (allow_auto && value.unit == LengthUnit::Auto && value.value.trim() != "auto"))
        {
            diagnostics.push(node_diagnostic(
                "DESIGN_LAYOUT_LENGTH_INVALID",
                format!("{name} must be a finite, non-negative length with a supported unit"),
                node_id,
            ));
        }
    }
    if let Some(position) = &layout.position {
        for value in [
            &position.top,
            &position.right,
            &position.bottom,
            &position.left,
        ]
        .into_iter()
        .flatten()
        {
            if !valid_length(value, false, false) {
                diagnostics.push(node_diagnostic(
                    "DESIGN_LAYOUT_POSITION_INVALID",
                    "overlay edges must be finite lengths with a supported unit",
                    node_id,
                ));
                break;
            }
        }
    }
    for (minimum, maximum, name) in [
        (&layout.min_width, &layout.max_width, "width"),
        (&layout.min_height, &layout.max_height, "height"),
    ] {
        if let (Some(minimum), Some(maximum)) = (minimum, maximum)
            && minimum.unit == maximum.unit
            && valid_length(minimum, false, true)
            && valid_length(maximum, false, true)
            && parse_length(minimum)
                .zip(parse_length(maximum))
                .is_some_and(|(minimum, maximum)| minimum > maximum)
        {
            diagnostics.push(node_diagnostic(
                "DESIGN_LAYOUT_CONSTRAINT_INVALID",
                format!("minimum {name} must not exceed maximum {name}"),
                node_id,
            ));
        }
    }
    if let (Some(size), Some(minimum), Some(maximum)) =
        (&layout.width, &layout.min_width, &layout.max_width)
    {
        if size.unit == minimum.unit
            && parse_length(size)
                .zip(parse_length(minimum))
                .is_some_and(|(size, minimum)| size < minimum)
        {
            diagnostics.push(node_diagnostic(
                "DESIGN_LAYOUT_CONSTRAINT_INVALID",
                "width is below its minimum constraint",
                node_id,
            ));
        }
        if size.unit == maximum.unit
            && parse_length(size)
                .zip(parse_length(maximum))
                .is_some_and(|(size, maximum)| size > maximum)
        {
            diagnostics.push(node_diagnostic(
                "DESIGN_LAYOUT_CONSTRAINT_INVALID",
                "width exceeds its maximum constraint",
                node_id,
            ));
        }
    }
    if let (Some(size), Some(minimum), Some(maximum)) =
        (&layout.height, &layout.min_height, &layout.max_height)
    {
        if size.unit == minimum.unit
            && parse_length(size)
                .zip(parse_length(minimum))
                .is_some_and(|(size, minimum)| size < minimum)
        {
            diagnostics.push(node_diagnostic(
                "DESIGN_LAYOUT_CONSTRAINT_INVALID",
                "height is below its minimum constraint",
                node_id,
            ));
        }
        if size.unit == maximum.unit
            && parse_length(size)
                .zip(parse_length(maximum))
                .is_some_and(|(size, maximum)| size > maximum)
        {
            diagnostics.push(node_diagnostic(
                "DESIGN_LAYOUT_CONSTRAINT_INVALID",
                "height exceeds its maximum constraint",
                node_id,
            ));
        }
    }

    let is_container = matches!(
        node_kind(node),
        Some(NodeKind::Box | NodeKind::Row | NodeKind::Column | NodeKind::Stack | NodeKind::Grid)
    );
    if (layout.gap.is_some() || layout.row_gap.is_some() || layout.column_gap.is_some())
        && !is_container
    {
        diagnostics.push(node_diagnostic(
            "DESIGN_LAYOUT_CONTAINER_INVALID",
            "gaps may only be authored on a layout container",
            node_id,
        ));
    }
    if (layout.row_gap.is_some() || layout.column_gap.is_some())
        && node_kind(node) != Some(NodeKind::Grid)
    {
        diagnostics.push(node_diagnostic(
            "DESIGN_LAYOUT_GRID_INVALID",
            "row_gap and column_gap require a Grid container",
            node_id,
        ));
    }
    if layout
        .grid_columns
        .is_some_and(|columns| !(1..=64).contains(&columns))
    {
        diagnostics.push(node_diagnostic(
            "DESIGN_LAYOUT_GRID_INVALID",
            "grid_columns must be between 1 and 64",
            node_id,
        ));
    }
    if layout.grid_columns.is_some() && node_kind(node) != Some(NodeKind::Grid) {
        diagnostics.push(node_diagnostic(
            "DESIGN_LAYOUT_GRID_INVALID",
            "grid_columns requires a Grid container",
            node_id,
        ));
    }
    if layout.alignment.is_some() && !is_container {
        diagnostics.push(node_diagnostic(
            "DESIGN_LAYOUT_CONTAINER_INVALID",
            "alignment may only be authored on a layout container",
            node_id,
        ));
    }
    if layout.alignment == Some(Alignment::SpaceBetween)
        && !matches!(node_kind(node), Some(NodeKind::Row | NodeKind::Column))
    {
        diagnostics.push(node_diagnostic(
            "DESIGN_LAYOUT_ALIGNMENT_INVALID",
            "space_between alignment requires a Row or Column container",
            node_id,
        ));
    }

    let parent_kind = parent_kind(design, node_id);
    if matches!(
        layout.placement,
        Some(Placement::Overlay | Placement::Absolute)
    ) && parent_kind != Some(NodeKind::Stack)
    {
        diagnostics.push(node_diagnostic(
            "DESIGN_LAYOUT_OVERLAY_PARENT_INVALID",
            "overlay and absolute placement require a Stack parent",
            node_id,
        ));
    }
    if layout.position.is_some()
        && !matches!(
            layout.placement,
            Some(Placement::Overlay | Placement::Absolute)
        )
    {
        diagnostics.push(node_diagnostic(
            "DESIGN_LAYOUT_POSITION_INVALID",
            "position edges require overlay or absolute placement",
            node_id,
        ));
    }
    diagnostics
}

fn node_kind(node: &DesignNode) -> Option<NodeKind> {
    match &node.source {
        DesignNodeSource::Primitive { kind } => Some(*kind),
        DesignNodeSource::CompositionInstance { .. } => None,
    }
}

fn parent_kind(design: &StudioDesign, node_id: &NodeId) -> Option<NodeKind> {
    let NodeParent::Node { node_id: parent_id } = design.parents.get(node_id)? else {
        return None;
    };
    design.nodes.get(parent_id).and_then(node_kind)
}

fn valid_length(length: &Length, allow_auto: bool, nonnegative: bool) -> bool {
    if length.value.trim().is_empty()
        || length.value.chars().any(char::is_control)
        || (!allow_auto && length.unit == LengthUnit::Auto)
    {
        return false;
    }
    if length.unit == LengthUnit::Auto {
        return allow_auto && length.value.trim() == "auto";
    }
    let Some(value) = parse_length(length) else {
        return false;
    };
    !nonnegative || value >= 0.0
}

fn parse_length(length: &Length) -> Option<f64> {
    length
        .value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn is_typed_layout_property(property: &str) -> bool {
    matches!(
        property,
        "width"
            | "height"
            | "min_width"
            | "max_width"
            | "min_height"
            | "max_height"
            | "gap"
            | "row_gap"
            | "column_gap"
            | "padding"
            | "placement"
            | "position"
            | "alignment"
            | "grid_columns"
            | "top"
            | "right"
            | "bottom"
            | "left"
    )
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

fn interaction_diagnostic(
    code: &str,
    message: impl Into<String>,
    interaction_id: &InteractionId,
) -> DesignerDiagnostic {
    DesignerDiagnostic {
        code: code.to_owned(),
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        node_id: None,
        interaction_id: Some(interaction_id.clone()),
        collection_id: None,
        binding_id: None,
        form_id: None,
        record_id: None,
    }
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
        collection_id: None,
        binding_id: None,
        form_id: None,
        record_id: None,
    }
}

fn collection_diagnostic(
    collection_id: &CollectionId,
    code: &str,
    message: impl Into<String>,
) -> DesignerDiagnostic {
    DesignerDiagnostic {
        code: code.to_owned(),
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        node_id: None,
        interaction_id: None,
        collection_id: Some(collection_id.clone()),
        binding_id: None,
        form_id: None,
        record_id: None,
    }
}

fn record_diagnostic_for(
    collection_id: &CollectionId,
    record_id: &RecordId,
    code: &str,
    message: impl Into<String>,
) -> DesignerDiagnostic {
    DesignerDiagnostic {
        code: code.to_owned(),
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        node_id: None,
        interaction_id: None,
        collection_id: Some(collection_id.clone()),
        binding_id: None,
        form_id: None,
        record_id: Some(record_id.clone()),
    }
}

fn join_diagnostics(diagnostics: &[DesignerDiagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}
