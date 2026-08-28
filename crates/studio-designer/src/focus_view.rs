//! Session-backed Designer Focus View MVP.
//!
//! [`FocusViewModel`] is the Designer-app seam between a persisted
//! [`studio_design::DesignerSession`] and a native view.  It intentionally
//! stores no editable copy of the design: canvas selection and inspector edits
//! always query or mutate the session, then re-project the resulting revision.
//! GPUI is confined to the [`FocusView`] adapter below.

use std::{
    collections::BTreeMap,
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

use gpui::{
    AnyElement, Context as GpuiContext, FocusHandle, IntoElement, ParentElement, Render, Role,
    Window, div, prelude::*, px, rgb,
};
use gpui_component::{
    Disableable,
    button::{Button, ButtonVariants},
};
use studio_design::{
    Actor, ActorId, ActorKind, CanvasPoint, CanvasSize, Command, CommandBatch, CommandOutcome,
    DefaultDesignerSession, DesignerDiagnostic, DesignerPersistence, DesignerQuery,
    DesignerQueryResult, DesignerSession, HierarchyEdit, HistoryOperation, LibrarySnapshot, NodeId,
    NodeParent, OperationId, ParentPlacement, ProjectionDiagnostic, ProjectionOptions,
    ProjectionReport, PropertyValue, ResizeHandle, STUDIO_DESIGN_SCHEMA_VERSION, SelectionSnapshot,
    SessionContextUpdate, SessionError, SnapConfig, StudioDesign, UndoGroupId, delete_batch,
    drag_batch, duplicate_batch, hierarchy_edit_batch, keyboard_resize_batch, nudge_batch,
    reparent_batch, restore_batch,
};
use studio_protocol::{NodeKind, UiNode};
use thiserror::Error;

/// Stable native Focus View colors.  The view is deliberately small; the
/// existing foundation owns the broader Studio palette and component styling.
const COLOR_CANVAS: u32 = 0x00f5_f7f8;
const COLOR_PANEL: u32 = 0x00ff_ffff;
const COLOR_BORDER: u32 = 0x00df_e3e6;
const COLOR_SELECTED: u32 = 0x00e0_ecff;
const COLOR_TEXT: u32 = 0x0018_2735;
const COLOR_MUTED: u32 = 0x008b_949e;
const COLOR_ERROR: u32 = 0x00fe_f2f2;

/// Reserved source property used by the ticket-40 manipulation commands.
pub const CANVAS_RECT_PROPERTY: &str = studio_design::CANVAS_RECT_PROPERTY;

/// Explicit state shown by Focus View when projection or a command fails.
#[derive(Clone, Debug, PartialEq)]
pub enum FocusViewState {
    /// The session snapshot is projected and ready for canvas interaction.
    Ready,
    /// Projection failed; the source remains available for diagnostics/editing.
    ProjectionFailed(Vec<ProjectionDiagnostic>),
    /// A command was rejected by the domain validator.
    CommandRejected(Vec<DesignerDiagnostic>),
    /// A command raced a newer revision or failed a precondition.
    Conflict(studio_design::BatchConflict),
    /// The persistence adapter could not commit an accepted command.
    PersistenceFailed(studio_design::PersistenceError),
}

/// Read-only state consumed by tests and the GPUI Focus View.
#[derive(Clone, Debug, PartialEq)]
pub struct FocusViewSnapshot {
    /// Current immutable source revision.
    pub revision_id: studio_design::RevisionId,
    /// Selected source screen route.
    pub route: Option<String>,
    /// Current protocol canvas root, absent when projection failed.
    pub canvas: Option<UiNode>,
    /// Stable selected source node.
    pub selected_node_id: Option<NodeId>,
    /// Inspector source node, if the selection still exists.
    pub selected_node: Option<studio_design::DesignNode>,
    /// Explicit projection/command state.
    pub state: FocusViewState,
}

/// Failure selecting a node that the current session does not contain.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum FocusSelectionError {
    /// The supplied stable identity is malformed.
    #[error("invalid Focus View node identity")]
    InvalidIdentity,
    /// The supplied identity is not in the current source snapshot.
    #[error("Focus View node does not exist: {0}")]
    NodeNotFound(NodeId),
}

/// Error constructing a Focus View from a persisted project.
#[derive(Debug, Error)]
pub enum FocusOpenError {
    /// The persisted session could not be opened.
    #[error(transparent)]
    Session(#[from] SessionError),
}

/// Session-backed Focus View model for one project.
pub struct FocusViewModel<P> {
    session: DefaultDesignerSession<P>,
    library: Option<LibrarySnapshot>,
    projection_options: ProjectionOptions,
    projection: ProjectionReport,
    state: FocusViewState,
    last_outcome: Option<CommandOutcome>,
}

impl<P: DesignerPersistence> FocusViewModel<P> {
    /// Open one durable project and immediately prepare its canvas projection.
    pub async fn open(
        persistence: P,
        project_id: &studio_design::ProjectId,
        library: Option<LibrarySnapshot>,
    ) -> Result<Self, FocusOpenError> {
        let session = DefaultDesignerSession::open(persistence, project_id).await?;
        Ok(Self::from_session(session, library))
    }

    /// Create a Focus View around an already-created session.
    #[must_use]
    pub fn from_session(
        session: DefaultDesignerSession<P>,
        library: Option<LibrarySnapshot>,
    ) -> Self {
        let mut model = Self {
            session,
            library,
            projection_options: ProjectionOptions::default(),
            projection: ProjectionReport {
                revision_id: studio_design::RevisionId::INITIAL,
                screen_id: None,
                route: None,
                root: None,
                diagnostics: Vec::new(),
            },
            state: FocusViewState::Ready,
            last_outcome: None,
        };
        model.refresh();
        model
    }

    /// Set explicit projection options used by the next refresh.
    pub fn set_projection_options(&mut self, options: ProjectionOptions) {
        self.projection_options = options;
        self.refresh();
    }

    /// Replace the optional Library snapshot and re-project.
    pub fn set_library(&mut self, library: Option<LibrarySnapshot>) {
        self.library = library;
        self.refresh();
    }

    /// Re-read the immutable source snapshot and deterministically project it.
    pub fn refresh(&mut self) {
        let snapshot = match self.session.query(DesignerQuery::Snapshot) {
            DesignerQueryResult::Snapshot(snapshot) => snapshot,
            _ => unreachable!("DesignerSession returned the wrong query result"),
        };
        let active_screen_id = match self.session.query(DesignerQuery::SessionState) {
            DesignerQueryResult::SessionState(state) => state.active_screen_id,
            _ => unreachable!("DesignerSession returned the wrong query result"),
        };
        let mut options = self.projection_options.clone();
        if options.screen_id.is_none() {
            options.screen_id = active_screen_id;
        }
        self.projection = studio_design::project_report(&snapshot, self.library.as_ref(), options);
        self.state = if self.projection.is_valid() {
            FocusViewState::Ready
        } else {
            FocusViewState::ProjectionFailed(self.projection.diagnostics.clone())
        };
    }

    /// Return an immutable model snapshot for a canvas, inspector, or test.
    #[must_use]
    pub fn snapshot(&self) -> FocusViewSnapshot {
        let selected_node_id = match self.session.query(DesignerQuery::SessionState) {
            DesignerQueryResult::SessionState(state) => state.selection.primary,
            _ => unreachable!("DesignerSession returned the wrong query result"),
        };
        let selected_node = selected_node_id.as_ref().and_then(|node_id| {
            match self.session.query(DesignerQuery::Node {
                node_id: node_id.clone(),
            }) {
                DesignerQueryResult::Node(node) => node,
                _ => unreachable!("DesignerSession returned the wrong query result"),
            }
        });
        FocusViewSnapshot {
            revision_id: self.projection.revision_id,
            route: self.projection.route.clone(),
            canvas: self.projection.root.clone(),
            selected_node_id,
            selected_node,
            state: self.state.clone(),
        }
    }

    /// Borrow the latest diagnostic-rich projection report.
    #[must_use]
    pub const fn projection(&self) -> &ProjectionReport {
        &self.projection
    }

    /// Borrow the last command outcome, when one has been submitted.
    #[must_use]
    pub const fn last_outcome(&self) -> Option<&CommandOutcome> {
        self.last_outcome.as_ref()
    }

    /// Borrow the underlying session for host integrations that need a typed query.
    #[must_use]
    pub const fn session(&self) -> &DefaultDesignerSession<P> {
        &self.session
    }

    /// Borrow the underlying session for host integrations that need to mutate
    /// durable context while keeping the session as the sole source of truth.
    pub fn session_mut(&mut self) -> &mut DefaultDesignerSession<P> {
        &mut self.session
    }

    /// Select one source node and persist the ephemeral selection context.
    pub fn select(&mut self, node_id: &NodeId) -> Result<(), FocusSelectionError> {
        let exists = matches!(
            self.session.query(DesignerQuery::Node {
                node_id: node_id.clone(),
            }),
            DesignerQueryResult::Node(Some(_))
        );
        if !exists {
            return Err(FocusSelectionError::NodeNotFound(node_id.clone()));
        }
        self.session.update_context(SessionContextUpdate {
            selection: Some(SelectionSnapshot {
                node_ids: vec![node_id.clone()],
                primary: Some(node_id.clone()),
            }),
            ..SessionContextUpdate::default()
        });
        Ok(())
    }

    /// Parse and select one stable identity from a GPUI canvas callback.
    pub fn select_str(&mut self, node_id: &str) -> Result<(), FocusSelectionError> {
        let node_id =
            NodeId::new(node_id.to_owned()).map_err(|_| FocusSelectionError::InvalidIdentity)?;
        self.select(&node_id)
    }

    /// Clear the current canvas/inspector selection.
    pub fn clear_selection(&mut self) {
        self.session.update_context(SessionContextUpdate {
            selection: Some(SelectionSnapshot::default()),
            ..SessionContextUpdate::default()
        });
    }

    /// Submit an inspector property edit through the command/undo authority.
    pub async fn edit_property(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
        undo_group_name: impl Into<String>,
        property: impl Into<String>,
        value: Option<PropertyValue>,
    ) -> CommandOutcome {
        let state = match self.session.query(DesignerQuery::SessionState) {
            DesignerQueryResult::SessionState(state) => state,
            _ => unreachable!("DesignerSession returned the wrong query result"),
        };
        let Some(node_id) = state.selection.primary else {
            let outcome = CommandOutcome::Rejected(vec![DesignerDiagnostic {
                code: "FOCUS_SELECTION_REQUIRED".to_owned(),
                severity: studio_design::DiagnosticSeverity::Error,
                message: "select a canvas node before editing its properties".to_owned(),
                node_id: None,
                interaction_id: None,
                collection_id: None,
                binding_id: None,
                form_id: None,
                record_id: None,
            }]);
            self.last_outcome = Some(outcome.clone());
            self.state = match &outcome {
                CommandOutcome::Rejected(diagnostics) => {
                    FocusViewState::CommandRejected(diagnostics.clone())
                }
                _ => FocusViewState::Ready,
            };
            return outcome;
        };
        let batch = CommandBatch {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            operation_id,
            actor,
            project_id: state.project_id,
            base_revision: state.revision_id,
            undo_group_id,
            undo_group_name: undo_group_name.into(),
            preconditions: Vec::new(),
            commands: vec![Command::SetProperty {
                node_id,
                property: property.into(),
                value,
            }],
        };
        let outcome = self.session.submit(batch).await;
        self.apply_outcome(outcome.clone());
        outcome
    }

    /// Submit a deterministic convenience text edit used by the native MVP.
    pub async fn edit_text(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        self.edit_property(
            operation_id,
            actor,
            undo_group_id,
            "Edit text",
            "text",
            Some(PropertyValue::String("Edited".to_owned())),
        )
        .await
    }

    /// Build and submit a ticket-40 nudge through the authoritative session.
    pub async fn nudge_selected(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
        direction: CanvasPoint,
    ) -> CommandOutcome {
        let batch = self.manipulation_batch(
            operation_id,
            actor,
            undo_group_id,
            |context, design, geometry, selection| {
                nudge_batch(
                    context,
                    geometry,
                    selection,
                    direction,
                    8.0,
                    SnapConfig::default(),
                )
                .map_err(|error| (design, error))
            },
        );
        self.submit_manipulation(batch).await
    }

    /// Build and submit a keyboard resize through the authoritative session.
    pub async fn resize_selected(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        let batch = self.manipulation_batch(
            operation_id,
            actor,
            undo_group_id,
            |context, design, geometry, selection| {
                let Some(node_id) = selection.first().cloned() else {
                    return Err((design, studio_design::ManipulationError::EmptySelection));
                };
                keyboard_resize_batch(
                    context,
                    geometry,
                    node_id,
                    ResizeHandle::East,
                    CanvasPoint::new(16.0, 0.0),
                    CanvasSize::new(16.0, 16.0),
                    SnapConfig::default(),
                )
                .map_err(|error| (design, error))
            },
        );
        self.submit_manipulation(batch).await
    }

    /// Build and submit a hierarchy rename through the same command seam.
    pub async fn rename_selected(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        let batch = self.manipulation_batch(
            operation_id,
            actor,
            undo_group_id,
            |context, design, geometry, selection| {
                let Some(node_id) = selection.first().cloned() else {
                    return Err((design, studio_design::ManipulationError::EmptySelection));
                };
                hierarchy_edit_batch(
                    context,
                    &design,
                    geometry,
                    HierarchyEdit::Rename {
                        node_id,
                        name: "Renamed layer".to_owned(),
                    },
                )
                .map_err(|error| (design, error))
            },
        );
        self.submit_manipulation(batch).await
    }

    /// Build and submit a hierarchy reorder through the same command seam.
    pub async fn reorder_selected(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
        index: usize,
    ) -> CommandOutcome {
        let batch = self.manipulation_batch(
            operation_id,
            actor,
            undo_group_id,
            |context, design, geometry, selection| {
                let Some(node_id) = selection.first().cloned() else {
                    return Err((design, studio_design::ManipulationError::EmptySelection));
                };
                hierarchy_edit_batch(
                    context,
                    &design,
                    geometry,
                    HierarchyEdit::Reorder { node_id, index },
                )
                .map_err(|error| (design, error))
            },
        );
        self.submit_manipulation(batch).await
    }

    /// Build and submit a pointer drag using the same snapping algebra as the canvas.
    pub async fn drag_selected(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
        delta: CanvasPoint,
    ) -> CommandOutcome {
        let batch = self.manipulation_batch(
            operation_id,
            actor,
            undo_group_id,
            |context, design, geometry, selection| {
                drag_batch(context, geometry, selection, delta, SnapConfig::default())
                    .map_err(|error| (design, error))
            },
        );
        self.submit_manipulation(batch).await
    }

    /// Build and submit a duplicate while deriving stable IDs from the source subtree.
    pub async fn duplicate_selected(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        let batch = self.manipulation_batch(
            operation_id,
            actor,
            undo_group_id,
            |context, design, geometry, selection| {
                let Some(source_node_id) = selection.first().cloned() else {
                    return Err((design, studio_design::ManipulationError::EmptySelection));
                };
                let Some(parent) = design.parents.get(&source_node_id).cloned() else {
                    return Err((
                        design,
                        studio_design::ManipulationError::MissingNode(source_node_id),
                    ));
                };
                let mut id_map = BTreeMap::new();
                let mut pending = vec![source_node_id.clone()];
                while let Some(node_id) = pending.pop() {
                    let Ok(copy_id) = NodeId::new(format!("{node_id}-copy")) else {
                        return Err((design, studio_design::ManipulationError::InvalidIdentityMap));
                    };
                    id_map.insert(node_id.clone(), copy_id);
                    if let Some(node) = design.nodes.get(&node_id) {
                        pending.extend(node.children.iter().rev().cloned());
                    }
                }
                duplicate_batch(
                    context,
                    &design,
                    geometry,
                    source_node_id,
                    ParentPlacement { parent, index: 0 },
                    id_map,
                )
                .map_err(|error| (design, error))
            },
        );
        self.submit_manipulation(batch).await
    }

    /// Build and submit a delete operation for the selected child subtree.
    pub async fn delete_selected(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        let selection = self.selection_ids();
        let batch = self.manipulation_batch(
            operation_id,
            actor,
            undo_group_id,
            |context, design, _geometry, selection| {
                delete_batch(context, &design, selection).map_err(|error| (design, error))
            },
        );
        let outcome = self.submit_manipulation(batch).await;
        if matches!(outcome, CommandOutcome::Accepted(_)) {
            // The domain session intentionally clears selections that no
            // longer exist. Re-select only the exact roots represented by
            // current tombstones so Restore remains an explicit, safe,
            // session-authorized follow-up rather than an implicit undo.
            self.retain_tombstone_selection(&selection);
        }
        outcome
    }

    /// Build and submit a restore operation from the exact session tombstones.
    pub async fn restore_selected(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        let snapshot = self.source_snapshot();
        let state = match self.session.query(DesignerQuery::SessionState) {
            DesignerQueryResult::SessionState(state) => state,
            _ => unreachable!("DesignerSession returned the wrong query result"),
        };
        let context = studio_design::GestureContext::new(
            operation_id,
            actor,
            state.project_id,
            state.revision_id,
            undo_group_id,
        );
        let batch = restore_batch(&context, &snapshot, &state.selection.node_ids)
            .map_err(|error| (snapshot.design.clone(), error));
        self.submit_manipulation(batch).await
    }

    /// Build and submit a hierarchy reparent operation for the selected child.
    pub async fn reparent_selected(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        let batch = self.manipulation_batch(
            operation_id,
            actor,
            undo_group_id,
            |context, design, geometry, selection| {
                let Some(node_id) = selection.first().cloned() else {
                    return Err((design, studio_design::ManipulationError::EmptySelection));
                };
                let Some(parent) = design.parents.get(&node_id).cloned() else {
                    return Err((
                        design,
                        studio_design::ManipulationError::MissingNode(node_id),
                    ));
                };
                let NodeParent::Node { node_id: parent_id } = parent else {
                    return Err((design, studio_design::ManipulationError::RootNode(node_id)));
                };
                reparent_batch(
                    context,
                    &design,
                    geometry,
                    node_id,
                    ParentPlacement {
                        parent: NodeParent::Node { node_id: parent_id },
                        index: 0,
                    },
                )
                .map_err(|error| (design, error))
            },
        );
        self.submit_manipulation(batch).await
    }

    /// Return the immutable source snapshot currently held by the session.
    #[must_use]
    pub fn source_snapshot(&self) -> studio_design::StudioDesignSnapshot {
        match self.session.query(DesignerQuery::Snapshot) {
            DesignerQueryResult::Snapshot(snapshot) => snapshot,
            _ => unreachable!("DesignerSession returned the wrong query result"),
        }
    }

    /// Return the current session selection, including deleted identities.
    #[must_use]
    pub fn selection_ids(&self) -> Vec<NodeId> {
        match self.session.query(DesignerQuery::SessionState) {
            DesignerQueryResult::SessionState(state) => state.selection.node_ids,
            _ => unreachable!("DesignerSession returned the wrong query result"),
        }
    }

    /// Return selected tombstone roots that can be restored from the current
    /// durable snapshot.
    #[must_use]
    pub fn selected_tombstones(&self) -> Vec<NodeId> {
        let snapshot = self.source_snapshot();
        self.selection_ids()
            .into_iter()
            .filter_map(|selected| {
                if snapshot.tombstones.contains_key(&selected) {
                    Some(selected)
                } else {
                    snapshot
                        .tombstones
                        .iter()
                        .find(|(_, tombstone)| tombstone.nodes.contains_key(&selected))
                        .map(|(root_id, _)| root_id.clone())
                }
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn retain_tombstone_selection(&mut self, candidates: &[NodeId]) {
        let snapshot = self.source_snapshot();
        let roots = candidates
            .iter()
            .filter_map(|selected| {
                if snapshot.tombstones.contains_key(selected) {
                    Some(selected.clone())
                } else {
                    snapshot
                        .tombstones
                        .iter()
                        .find(|(_, tombstone)| tombstone.nodes.contains_key(selected))
                        .map(|(root_id, _)| root_id.clone())
                }
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        self.session.update_context(SessionContextUpdate {
            selection: Some(SelectionSnapshot {
                primary: roots.first().cloned(),
                node_ids: roots,
            }),
            ..SessionContextUpdate::default()
        });
    }

    /// Hit-test the current source geometry and return the stable source identity.
    #[must_use]
    pub fn hit_test(&self, point: CanvasPoint) -> Option<NodeId> {
        studio_design::CanvasGeometry::from_design(&self.source_snapshot().design)
            .hit_test(point, 0.0)
    }

    fn manipulation_batch<F>(
        &self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
        build: F,
    ) -> Result<CommandBatch, (StudioDesign, studio_design::ManipulationError)>
    where
        F: FnOnce(
            &studio_design::GestureContext,
            StudioDesign,
            &studio_design::CanvasGeometry,
            &[NodeId],
        )
            -> Result<CommandBatch, (StudioDesign, studio_design::ManipulationError)>,
    {
        let snapshot = self.source_snapshot();
        let state = match self.session.query(DesignerQuery::SessionState) {
            DesignerQueryResult::SessionState(state) => state,
            _ => unreachable!("DesignerSession returned the wrong query result"),
        };
        let context = studio_design::GestureContext::new(
            operation_id,
            actor,
            state.project_id,
            state.revision_id,
            undo_group_id,
        );
        let geometry = studio_design::CanvasGeometry::from_design(&snapshot.design);
        build(
            &context,
            snapshot.design,
            &geometry,
            &state.selection.node_ids,
        )
    }

    async fn submit_manipulation(
        &mut self,
        batch: Result<CommandBatch, (StudioDesign, studio_design::ManipulationError)>,
    ) -> CommandOutcome {
        let batch = match batch {
            Ok(batch) => batch,
            Err((_design, error)) => {
                let outcome = CommandOutcome::Rejected(vec![DesignerDiagnostic {
                    code: "FOCUS_MANIPULATION_INVALID".to_owned(),
                    severity: studio_design::DiagnosticSeverity::Error,
                    message: error.to_string(),
                    node_id: None,
                    interaction_id: None,
                    collection_id: None,
                    binding_id: None,
                    form_id: None,
                    record_id: None,
                }]);
                self.apply_outcome(outcome.clone());
                return outcome;
            }
        };
        let outcome = self.session.submit(batch).await;
        self.apply_outcome(outcome.clone());
        outcome
    }

    /// Undo the current named group and re-project the resulting revision.
    pub async fn undo(&mut self, operation_id: OperationId, actor: Actor) -> CommandOutcome {
        let state = match self.session.query(DesignerQuery::SessionState) {
            DesignerQueryResult::SessionState(state) => state,
            _ => unreachable!("DesignerSession returned the wrong query result"),
        };
        let outcome = self
            .session
            .undo(HistoryOperation {
                operation_id,
                actor,
                base_revision: state.revision_id,
            })
            .await;
        self.apply_outcome(outcome.clone());
        outcome
    }

    /// Synchronous helper for GPUI callbacks backed by immediate local adapters.
    pub fn edit_text_now(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        block_on(self.edit_text(operation_id, actor, undo_group_id))
    }

    /// Synchronous helper for GPUI callbacks backed by immediate local adapters.
    pub fn undo_now(&mut self, operation_id: OperationId, actor: Actor) -> CommandOutcome {
        block_on(self.undo(operation_id, actor))
    }

    fn apply_outcome(&mut self, outcome: CommandOutcome) {
        match &outcome {
            CommandOutcome::Accepted(_) => {
                self.last_outcome = Some(outcome);
                self.refresh();
            }
            CommandOutcome::Rejected(diagnostics) => {
                self.state = FocusViewState::CommandRejected(diagnostics.clone());
                self.last_outcome = Some(outcome);
            }
            CommandOutcome::Conflict(conflict) => {
                self.state = FocusViewState::Conflict(conflict.clone());
                self.last_outcome = Some(outcome);
            }
            CommandOutcome::PersistenceFailed(error) => {
                self.state = FocusViewState::PersistenceFailed(error.clone());
                self.last_outcome = Some(outcome);
            }
        }
    }
}

/// A minimal native GPUI Focus View over [`FocusViewModel`].
pub struct FocusView<P> {
    /// Session-backed state used by callbacks and render.
    pub model: Option<FocusViewModel<P>>,
    root_focus: FocusHandle,
    operation_in_flight: bool,
}

#[derive(Clone, Copy)]
enum FocusAction {
    EditText,
    Undo,
    Drag,
    NudgeLeft,
    NudgeRight,
    Resize,
    Rename,
    Reorder,
    Reparent,
    Duplicate,
    Delete,
    Restore,
}

impl<P: DesignerPersistence + 'static> FocusView<P> {
    /// Create the GPUI adapter around one already-open model.
    #[must_use]
    pub fn new(model: FocusViewModel<P>, cx: &mut GpuiContext<Self>) -> Self {
        Self {
            model: Some(model),
            root_focus: cx.focus_handle(),
            operation_in_flight: false,
        }
    }

    fn start_action(&mut self, action: FocusAction, cx: &mut GpuiContext<Self>) {
        if self.operation_in_flight {
            return;
        }
        let Some(mut model) = self.model.take() else {
            return;
        };
        let revision = model.snapshot().revision_id.get().saturating_add(1);
        let (name, undo_name) = match action {
            FocusAction::EditText => ("focus-edit", "focus-text-edit"),
            FocusAction::Undo => ("focus-undo", "focus-undo"),
            FocusAction::Drag => ("focus-drag", "focus-drag"),
            FocusAction::NudgeLeft => ("focus-nudge-left", "focus-nudge-left"),
            FocusAction::NudgeRight => ("focus-nudge-right", "focus-nudge-right"),
            FocusAction::Resize => ("focus-resize", "focus-resize"),
            FocusAction::Rename => ("focus-rename", "focus-rename"),
            FocusAction::Reorder => ("focus-reorder", "focus-reorder"),
            FocusAction::Reparent => ("focus-reparent", "focus-reparent"),
            FocusAction::Duplicate => ("focus-duplicate", "focus-duplicate"),
            FocusAction::Delete => ("focus-delete", "focus-delete"),
            FocusAction::Restore => ("focus-restore", "focus-restore"),
        };
        let Ok(operation_id) = OperationId::new(format!("{name}-{revision}")) else {
            self.model = Some(model);
            return;
        };
        let Ok(undo_group_id) = UndoGroupId::new(undo_name) else {
            self.model = Some(model);
            return;
        };
        let Ok(actor) = focus_actor() else {
            self.model = Some(model);
            return;
        };
        self.operation_in_flight = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let _outcome = match action {
                FocusAction::EditText => model.edit_text(operation_id, actor, undo_group_id).await,
                FocusAction::Undo => model.undo(operation_id, actor).await,
                FocusAction::Drag => {
                    model
                        .drag_selected(
                            operation_id,
                            actor,
                            undo_group_id,
                            CanvasPoint::new(16.0, 0.0),
                        )
                        .await
                }
                FocusAction::NudgeLeft => {
                    model
                        .nudge_selected(
                            operation_id,
                            actor,
                            undo_group_id,
                            CanvasPoint::new(-1.0, 0.0),
                        )
                        .await
                }
                FocusAction::NudgeRight => {
                    model
                        .nudge_selected(
                            operation_id,
                            actor,
                            undo_group_id,
                            CanvasPoint::new(1.0, 0.0),
                        )
                        .await
                }
                FocusAction::Resize => {
                    model
                        .resize_selected(operation_id, actor, undo_group_id)
                        .await
                }
                FocusAction::Rename => {
                    model
                        .rename_selected(operation_id, actor, undo_group_id)
                        .await
                }
                FocusAction::Reorder => {
                    model
                        .reorder_selected(operation_id, actor, undo_group_id, 0)
                        .await
                }
                FocusAction::Reparent => {
                    model
                        .reparent_selected(operation_id, actor, undo_group_id)
                        .await
                }
                FocusAction::Duplicate => {
                    model
                        .duplicate_selected(operation_id, actor, undo_group_id)
                        .await
                }
                FocusAction::Delete => {
                    model
                        .delete_selected(operation_id, actor, undo_group_id)
                        .await
                }
                FocusAction::Restore => {
                    model
                        .restore_selected(operation_id, actor, undo_group_id)
                        .await
                }
            };
            this.update(cx, |this, cx| {
                this.model = Some(model);
                this.operation_in_flight = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn render_node(
        &self,
        node: &UiNode,
        selected: Option<&NodeId>,
        cx: &mut GpuiContext<Self>,
    ) -> AnyElement {
        let node_id = node.id.clone();
        let selected_here = selected.is_some_and(|selected| selected.as_str() == node.id);
        let children = node
            .children
            .iter()
            .map(|child| self.render_node(child, selected, cx))
            .collect::<Vec<_>>();
        let kind = format_kind(node.kind);
        div()
            .id(format!("focus-canvas-node-{}", node.id))
            .role(Role::Button)
            .aria_label(format!("{kind} {}", node.id))
            .w_full()
            .p_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(COLOR_BORDER))
            .when(selected_here, |element| element.bg(rgb(COLOR_SELECTED)))
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(format!("{kind} · {}", node.id)),
            )
            .children(children)
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some(model) = this.model.as_mut() {
                    let _ = model.select_str(&node_id);
                }
                cx.notify();
            }))
            .into_any_element()
    }
}

impl<P: DesignerPersistence + 'static> Render for FocusView<P> {
    fn render(&mut self, _window: &mut Window, cx: &mut GpuiContext<Self>) -> impl IntoElement {
        if self.model.is_none() {
            return div()
                .id("studio-focus-loading")
                .role(Role::Status)
                .aria_label("Designer operation in progress")
                .p_4()
                .child("Saving Designer changes…");
        }
        let snapshot = self
            .model
            .as_ref()
            .expect("FocusView model is present after loading guard")
            .snapshot();
        let selected = snapshot.selected_node_id.clone();
        let canvas = snapshot
            .canvas
            .as_ref()
            .map(|root| self.render_node(root, selected.as_ref(), cx))
            .unwrap_or_else(|| {
                div()
                    .id("focus-canvas-error")
                    .role(Role::Alert)
                    .p_3()
                    .bg(rgb(COLOR_ERROR))
                    .child("Canvas projection unavailable")
                    .into_any_element()
            });
        let selected_label = snapshot.selected_node_id.as_ref().map_or_else(
            || "Nothing selected".to_owned(),
            |id| format!("Selected: {id}"),
        );
        let route = snapshot
            .route
            .clone()
            .unwrap_or_else(|| "No route".to_owned());
        let status = match &snapshot.state {
            FocusViewState::Ready => "Ready".to_owned(),
            FocusViewState::ProjectionFailed(diagnostics) => {
                format!("Projection failed ({} diagnostic(s))", diagnostics.len())
            }
            FocusViewState::CommandRejected(diagnostics) => {
                format!("Edit rejected ({} diagnostic(s))", diagnostics.len())
            }
            FocusViewState::Conflict(_) => "Edit conflict".to_owned(),
            FocusViewState::PersistenceFailed(_) => "Persistence failed".to_owned(),
        };
        let can_edit = snapshot.selected_node_id.is_some() && snapshot.selected_node.is_some();
        let restore_selection = self
            .model
            .as_ref()
            .map(FocusViewModel::selected_tombstones)
            .unwrap_or_default();
        let can_restore = !restore_selection.is_empty();
        let diagnostic_details = match &snapshot.state {
            FocusViewState::ProjectionFailed(diagnostics) => diagnostics
                .iter()
                .map(|diagnostic| {
                    format!(
                        "{}: {}{}",
                        diagnostic.code,
                        diagnostic.message,
                        diagnostic
                            .node_id
                            .as_ref()
                            .map_or(String::new(), |id| format!(" (node {id})"))
                    )
                })
                .collect::<Vec<_>>(),
            FocusViewState::CommandRejected(diagnostics) => diagnostics
                .iter()
                .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
                .collect::<Vec<_>>(),
            FocusViewState::Conflict(conflict) => {
                vec![format!("{}: {}", conflict.code, conflict.message)]
            }
            FocusViewState::PersistenceFailed(error) => {
                vec![format!("{:?}: {}", error.code, error.message)]
            }
            FocusViewState::Ready => Vec::new(),
        };
        let hierarchy = self.model.as_ref().map(|model| {
            studio_design::HierarchySnapshot::from_design(&model.source_snapshot().design)
        });
        let action_button = |id: &'static str,
                             label: &'static str,
                             action: FocusAction,
                             disabled: bool,
                             cx: &mut GpuiContext<Self>| {
            Button::new(id)
                .label(label)
                .disabled(disabled)
                .secondary()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.start_action(action, cx);
                }))
        };
        div()
            .id("studio-focus-view")
            .role(Role::Application)
            .aria_label("Studio Designer Focus View")
            .track_focus(&self.root_focus)
            .size_full()
            .flex()
            .gap_3()
            .p_4()
            .bg(rgb(COLOR_CANVAS))
            .text_color(rgb(COLOR_TEXT))
            .child(
                div()
                    .id("focus-canvas-panel")
                    .role(Role::Pane)
                    .aria_label(format!("Canvas {route}"))
                    .flex_1()
                    .min_w_0()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(COLOR_BORDER))
                    .bg(rgb(COLOR_PANEL))
                    .child(div().text_lg().child(format!("Canvas · {route}")))
                    .child(canvas),
            )
            .child(
                div()
                    .id("focus-inspector-panel")
                    .role(Role::Pane)
                    .aria_label("Inspector")
                    .w(px(300.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(COLOR_BORDER))
                    .bg(rgb(COLOR_PANEL))
                    .child(div().text_lg().child("Inspector"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(COLOR_MUTED))
                            .child(selected_label),
                    )
                    .child(
                        Button::new("focus-edit-text")
                            .label("Set text to Edited")
                            .disabled(!can_edit)
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.start_action(FocusAction::EditText, cx);
                            })),
                    )
                    .child(
                        Button::new("focus-undo")
                            .label("Undo")
                            .secondary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.start_action(FocusAction::Undo, cx);
                            })),
                    )
                    .child(div().text_sm().child(status))
                    .child(div().text_sm().child("Hierarchy and canvas controls"))
                    .children(restore_selection.iter().map(|node_id| {
                        div()
                            .id(format!("focus-tombstone-{node_id}"))
                            .role(Role::ListItem)
                            .aria_label(format!("Deleted layer {node_id}"))
                            .text_sm()
                            .child(format!("Deleted layer · {node_id}"))
                    }))
                    .children(
                        hierarchy
                            .as_ref()
                            .map(|hierarchy| {
                                hierarchy
                                    .roots
                                    .iter()
                                    .map(|node| {
                                        div()
                                            .id(format!("focus-hierarchy-{}", node.node_id))
                                            .role(Role::ListItem)
                                            .aria_label(format!("Layer {}", node.name))
                                            .child(format!("{} · {}", node.name, node.node_id))
                                            .into_any_element()
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default(),
                    )
                    .child(action_button(
                        "focus-drag",
                        "Drag right",
                        FocusAction::Drag,
                        !can_edit,
                        cx,
                    ))
                    .child(
                        Button::new("focus-hit-test")
                            .label("Hit-test canvas")
                            .secondary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(model) = this.model.as_mut()
                                    && let Some(node_id) =
                                        model.hit_test(CanvasPoint::new(40.0, 40.0))
                                {
                                    let _ = model.select(&node_id);
                                }
                                cx.notify();
                            })),
                    )
                    .child(action_button(
                        "focus-nudge-left",
                        "Nudge left",
                        FocusAction::NudgeLeft,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-nudge-right",
                        "Nudge right",
                        FocusAction::NudgeRight,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-resize",
                        "Resize wider",
                        FocusAction::Resize,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-rename",
                        "Rename layer",
                        FocusAction::Rename,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-reorder",
                        "Move layer to front",
                        FocusAction::Reorder,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-reparent",
                        "Reparent layer",
                        FocusAction::Reparent,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-duplicate",
                        "Duplicate layer",
                        FocusAction::Duplicate,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-delete",
                        "Delete layer",
                        FocusAction::Delete,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-restore",
                        "Restore layer",
                        FocusAction::Restore,
                        !can_restore,
                        cx,
                    ))
                    .when(!diagnostic_details.is_empty(), |panel| {
                        panel
                            .child(
                                div()
                                    .id("focus-diagnostic-details")
                                    .role(Role::Alert)
                                    .bg(rgb(COLOR_ERROR))
                                    .p_2()
                                    .child("Details"),
                            )
                            .children(diagnostic_details.iter().enumerate().map(
                                |(index, detail)| {
                                    div()
                                        .id(format!("focus-diagnostic-{index}"))
                                        .text_sm()
                                        .child(detail.clone())
                                },
                            ))
                            .child(
                                Button::new("focus-retry")
                                    .label("Retry projection")
                                    .primary()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(model) = this.model.as_mut() {
                                            model.refresh();
                                        }
                                        cx.notify();
                                    })),
                            )
                    }),
            )
    }
}

fn format_kind(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Box => "Box",
        NodeKind::Column => "Column",
        NodeKind::Row => "Row",
        NodeKind::Text => "Text",
        NodeKind::Button => "Button",
        _ => "Node",
    }
}

fn focus_actor() -> Result<Actor, studio_design::InvalidIdentity> {
    Ok(Actor {
        id: ActorId::new("designer-focus")?,
        kind: ActorKind::Human,
        display_name: "Designer".to_owned(),
    })
}

struct NoopWake;
impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}
