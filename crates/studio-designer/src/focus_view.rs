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
    time::Duration,
};

use gpui_kit::component::{
    Disableable, Selectable,
    button::{Button, ButtonVariants},
    input::{Editor, EditorState, Input, InputEvent, InputState},
    tab::{Tab, TabBar},
};
use gpui_kit::{
    Context as GpuiContext, Entity, FocusHandle, IntoElement, ParentElement, Render, Role,
    Subscription, Window, div, prelude::*, px, rgb,
};
use studio_components::{
    InputAction,
    native_view::{NativeRenderNode, NativeRuntimeView, NativeViewEvent, ProjectionSurface},
};
use studio_design::{
    Actor, ActorId, ActorKind, CanvasPoint, CanvasSize, Command, CommandBatch, CommandOutcome,
    DefaultDesignerSession, DesignToken, DesignerDiagnostic, DesignerPersistence, DesignerQuery,
    DesignerQueryResult, DesignerSession, DeviceProfileId, EditorSnapshot, HierarchyEdit,
    HistoryOperation, InteractionId, LayoutProperties, LibrarySnapshot, NodeId, NodeParent,
    OperationId, ParentPlacement, ProjectionDiagnostic, ProjectionOptions, ProjectionReport,
    PropertyValue, ResizeHandle, STUDIO_DESIGN_SCHEMA_VERSION, ScriptCommitMetadata,
    ScriptCommitOutcome, ScriptDocumentAdapter, SelectionSnapshot, SessionContextUpdate,
    SessionError, SnapConfig, StudioDesign, TokenId, TokenKind, TokenValue, UndoGroupId,
    WORKSPACE_STATE_SCHEMA_VERSION, WorkspaceCommand, WorkspaceController, WorkspacePersistence,
    WorkspaceRecord, WorkspaceState, delete_batch, drag_batch, duplicate_batch,
    hierarchy_edit_batch, keyboard_resize_batch, nudge_batch, reparent_batch, restore_batch,
    select_variant,
};
use studio_protocol::UiNode;
use thiserror::Error;

/// Stable native Focus View colors.  The view is deliberately small; the
/// existing foundation owns the broader Studio palette and component styling.
const COLOR_CANVAS: u32 = 0x00f5_f7f8;
const COLOR_PANEL: u32 = 0x00ff_ffff;
const COLOR_BORDER: u32 = 0x00df_e3e6;
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
        let options = self.effective_projection_options();
        self.projection = studio_design::project_report(&snapshot, self.library.as_ref(), options);
        self.state = if self.projection.is_valid() {
            FocusViewState::Ready
        } else {
            FocusViewState::ProjectionFailed(self.projection.diagnostics.clone())
        };
    }

    fn effective_projection_options(&self) -> ProjectionOptions {
        let session_state = match self.session.query(DesignerQuery::SessionState) {
            DesignerQueryResult::SessionState(state) => state,
            _ => unreachable!("DesignerSession returned the wrong query result"),
        };
        let mut options = self.projection_options.clone();
        if options.screen_id.is_none() {
            options.screen_id = session_state.active_screen_id;
        }
        if options.responsive_variant_id.is_none()
            && let Some(profile) = self.selected_profile()
        {
            options.responsive_variant_id =
                select_variant(&self.source_snapshot().design, &profile);
        }
        options
    }

    // Use the same session-owned profile for variant resolution and native viewport sizing.
    fn selected_profile(&self) -> Option<studio_design::DeviceProfile> {
        let id = DeviceProfileId::new(self.session_state().device_profile?).ok()?;
        self.responsive_profiles()?.profiles.get(&id).cloned()
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

    /// Submit a typed base-layout change for the selected node.
    pub async fn set_layout_selected(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
        layout: LayoutProperties,
    ) -> CommandOutcome {
        let state = self.session_state();
        let Some(node_id) = state.selection.primary else {
            return self.reject_local(
                "FOCUS_SELECTION_REQUIRED",
                "select a canvas node before changing layout",
            );
        };
        self.submit_command(
            operation_id,
            actor,
            undo_group_id,
            "Layout",
            Command::SetLayout { node_id, layout },
        )
        .await
    }

    /// Return the session-owned state used by profile controls and view switches.
    #[must_use]
    pub fn session_state(&self) -> studio_design::SessionStateSnapshot {
        match self.session.query(DesignerQuery::SessionState) {
            DesignerQueryResult::SessionState(state) => state,
            _ => unreachable!("DesignerSession returned the wrong query result"),
        }
    }

    /// Switch the responsive preview profile without creating a design revision.
    pub fn set_profile(&mut self, profile: Option<String>) {
        self.session.update_context(SessionContextUpdate {
            device_profile: Some(profile),
            ..SessionContextUpdate::default()
        });
        self.refresh();
    }

    /// Return the complete profile matrix exposed by the authoritative session.
    #[must_use]
    pub fn responsive_profiles(&self) -> Option<studio_design::DeviceProfileMatrix> {
        match self.session.query(DesignerQuery::ResponsiveProfiles) {
            DesignerQueryResult::ResponsiveProfiles(profiles) => Some(profiles),
            _ => None,
        }
    }

    /// Return token browser entries from the authoritative session.
    #[must_use]
    pub fn tokens(&self) -> Vec<DesignToken> {
        match self.session.query(DesignerQuery::Tokens) {
            DesignerQueryResult::Tokens(tokens) => tokens,
            _ => Vec::new(),
        }
    }

    /// Return token usage records for an authoritative token identity.
    #[must_use]
    pub fn token_usages(&self, token_id: TokenId) -> Vec<studio_design::TokenUsage> {
        match self.session.query(DesignerQuery::TokenUsages { token_id }) {
            DesignerQueryResult::TokenUsages(usages) => usages,
            _ => Vec::new(),
        }
    }

    /// Return the selected node's shared/local token provenance for the
    /// inspector. The query remains session-owned; the native view only
    /// renders this immutable projection.
    #[must_use]
    pub fn node_token_values(&self) -> Vec<studio_design::InspectedTokenValue> {
        let Some(node_id) = self.session_state().selection.primary else {
            return Vec::new();
        };
        match self
            .session
            .query(DesignerQuery::NodeTokenValues { node_id })
        {
            DesignerQueryResult::NodeTokenValues(values) => values,
            _ => Vec::new(),
        }
    }

    /// Create a deterministic sample token through the validated token command family.
    pub async fn create_focus_token(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        let Ok(token_id) = TokenId::new("focus-accent") else {
            return self.reject_local(
                "FOCUS_TOKEN_ID_INVALID",
                "the Focus token identity is invalid",
            );
        };
        let token = DesignToken {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: token_id,
            name: "Focus Accent".to_owned(),
            kind: TokenKind::Length,
            value: TokenValue::Length(studio_design::Length {
                value: "8".to_owned(),
                unit: studio_design::LengthUnit::Pixels,
            }),
        };
        self.submit_command(
            operation_id,
            actor,
            undo_group_id,
            "Create token",
            Command::CreateToken {
                token: Box::new(token),
            },
        )
        .await
    }

    /// Apply the selected token to the selected node's padding property.
    pub async fn apply_focus_token(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        let state = self.session_state();
        let Some(node_id) = state.selection.primary else {
            return self.reject_local(
                "FOCUS_SELECTION_REQUIRED",
                "select a node before applying a token",
            );
        };
        let tokens = self.tokens();
        let Some(token) = tokens.first() else {
            return self.reject_local("FOCUS_TOKEN_REQUIRED", "create a token before applying it");
        };
        self.submit_command(
            operation_id,
            actor,
            undo_group_id,
            "Apply token",
            Command::ApplyToken {
                node_id,
                property: "padding".to_owned(),
                token_id: token.id.clone(),
            },
        )
        .await
    }

    /// Set a local token value while preserving its shared token identity.
    pub async fn override_focus_token(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        let state = self.session_state();
        let Some(node_id) = state.selection.primary else {
            return self.reject_local(
                "FOCUS_SELECTION_REQUIRED",
                "select a node before overriding a token",
            );
        };
        self.submit_command(
            operation_id,
            actor,
            undo_group_id,
            "Override token",
            Command::OverrideToken {
                node_id,
                property: "padding".to_owned(),
                value: TokenValue::Length(studio_design::Length {
                    value: "12".to_owned(),
                    unit: studio_design::LengthUnit::Pixels,
                }),
            },
        )
        .await
    }

    /// Clear the selected node's local token value.
    pub async fn clear_focus_token(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        let state = self.session_state();
        let Some(node_id) = state.selection.primary else {
            return self.reject_local(
                "FOCUS_SELECTION_REQUIRED",
                "select a node before clearing a token",
            );
        };
        self.submit_command(
            operation_id,
            actor,
            undo_group_id,
            "Clear token override",
            Command::ClearTokenOverride {
                node_id,
                property: "padding".to_owned(),
            },
        )
        .await
    }

    /// Rename the first token while retaining its stable identity.
    pub async fn rename_focus_token(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        let tokens = self.tokens();
        let Some(token) = tokens.first() else {
            return self.reject_local("FOCUS_TOKEN_REQUIRED", "create a token before renaming it");
        };
        self.submit_command(
            operation_id,
            actor,
            undo_group_id,
            "Rename token",
            Command::RenameToken {
                token_id: token.id.clone(),
                name: "Focus Accent Renamed".to_owned(),
            },
        )
        .await
    }

    /// Edit the first token's shared value while retaining its stable identity.
    pub async fn edit_focus_token(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        let tokens = self.tokens();
        let Some(token) = tokens.first() else {
            return self.reject_local("FOCUS_TOKEN_REQUIRED", "create a token before editing it");
        };
        let value = match token.kind {
            TokenKind::Length | TokenKind::Spacing | TokenKind::Radius => {
                TokenValue::Length(studio_design::Length {
                    value: "10".to_owned(),
                    unit: studio_design::LengthUnit::Pixels,
                })
            }
            _ => token.value.clone(),
        };
        let mut edited = token.clone();
        edited.value = value;
        self.submit_command(
            operation_id,
            actor,
            undo_group_id,
            "Edit token",
            Command::UpdateToken { token: edited },
        )
        .await
    }

    /// Ask the engine to delete the first token with explicit confirmation.
    pub async fn delete_focus_token(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandOutcome {
        let tokens = self.tokens();
        let Some(token) = tokens.first() else {
            return self.reject_local("FOCUS_TOKEN_REQUIRED", "create a token before deleting it");
        };
        self.submit_command(
            operation_id,
            actor,
            undo_group_id,
            "Delete token",
            Command::DeleteToken {
                token_id: token.id.clone(),
                confirm: true,
            },
        )
        .await
    }

    /// Run the declarative interaction graph in an isolated prototype state.
    pub fn prototype_preview(
        &self,
    ) -> Result<Option<studio_design::PrototypeDispatch>, studio_design::PrototypeError> {
        let snapshot = self.source_snapshot();
        let mut prototype = studio_design::PrototypeSession::new(snapshot.design)?;
        let Some(interaction_id) = prototype.design().interactions.keys().next().cloned() else {
            return Ok(None);
        };
        Ok(Some(prototype.dispatch_interaction(&interaction_id)))
    }

    /// Commit an edited Studio Script buffer through the parser-of-record and session.
    pub async fn commit_script_source(
        &mut self,
        source: String,
        metadata: ScriptCommitMetadata,
    ) -> ScriptCommitOutcome {
        let snapshot = self.source_snapshot();
        let mut editor = match ScriptDocumentAdapter::from_snapshot(&snapshot) {
            Ok(editor) => editor,
            Err(_) => {
                return ScriptCommitOutcome::Invalid {
                    diagnostics: Vec::new(),
                };
            }
        };
        editor.replace_source(source);
        let outcome = editor.commit(&mut self.session, metadata).await;
        if let ScriptCommitOutcome::Session(command_outcome) = &outcome {
            self.apply_outcome(command_outcome.clone());
        } else {
            self.refresh();
        }
        outcome
    }

    async fn submit_command(
        &mut self,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
        undo_group_name: &str,
        command: Command,
    ) -> CommandOutcome {
        let state = self.session_state();
        let batch = CommandBatch {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            operation_id,
            actor,
            project_id: state.project_id,
            base_revision: state.revision_id,
            undo_group_id,
            undo_group_name: undo_group_name.to_owned(),
            preconditions: Vec::new(),
            commands: vec![command],
        };
        let outcome = self.session.submit(batch).await;
        self.apply_outcome(outcome.clone());
        outcome
    }

    fn reject_local(&mut self, code: &str, message: &str) -> CommandOutcome {
        let outcome = CommandOutcome::Rejected(vec![DesignerDiagnostic {
            code: code.to_owned(),
            severity: studio_design::DiagnosticSeverity::Error,
            message: message.to_owned(),
            node_id: None,
            interaction_id: None,
            collection_id: None,
            binding_id: None,
            form_id: None,
            record_id: None,
        }]);
        self.apply_outcome(outcome.clone());
        outcome
    }

    /// Return the immutable source snapshot currently held by the session.
    #[must_use]
    pub fn source_snapshot(&self) -> studio_design::StudioDesignSnapshot {
        match self.session.query(DesignerQuery::Snapshot) {
            DesignerQueryResult::Snapshot(snapshot) => snapshot,
            _ => unreachable!("DesignerSession returned the wrong query result"),
        }
    }

    /// Return canonical Studio Script for the current immutable source.
    #[must_use]
    pub fn script_source(&self) -> String {
        ScriptDocumentAdapter::from_snapshot(&self.source_snapshot())
            .map(|editor| editor.source().to_owned())
            .unwrap_or_default()
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
    preview: Entity<NativeRuntimeView<ProjectionSurface>>,
    _preview_subscription: Subscription,
    operation_in_flight: bool,
    /// Presentation-only Focus/Workbench controller; both views use `model`.
    workspace: WorkspaceController,
    /// Durable per-project presentation preferences. This never stores a
    /// second design or session; it only records view/panel geometry.
    workspace_persistence: Arc<dyn WorkspacePersistence>,
    /// Local source folder associated with the current authenticated project.
    linked_source_root: Option<String>,
    project_controls: Entity<crate::project_controls::ProjectControls>,
    _project_controls_subscription: Subscription,
    /// Native editable Studio Script buffer and its event subscription.
    script_input: Option<Entity<EditorState>>,
    _script_subscription: Option<Subscription>,
    script_editor: Option<ScriptDocumentAdapter>,
    script_source: String,
    script_syncing: bool,
    script_feedback: Option<String>,
    /// File-backed source workspace linked to this authenticated project.
    source_path_input: Entity<InputState>,
    source_input: Option<Entity<EditorState>>,
    _source_subscription: Option<Subscription>,
    source_view: Option<crate::source_view::SourceView>,
    source_event_relay: Option<crate::source_events::SourceEventRelay>,
    source_files: Vec<crate::source_workspace::FileInfo>,
    source_feedback: Option<String>,
    source_syncing: bool,
    preview_status: Option<studio_protocol::dev_rpc::DevStatus>,
    preview_feedback: Option<String>,
    preview_command_pending: bool,
    prototype_feedback: Option<String>,
    prototype: Option<studio_design::PrototypeSession>,
    prototype_mode: bool,
    prototype_screen: Option<studio_design::ScreenId>,
    prototype_interaction: Option<InteractionId>,
    token_delete_confirmation: bool,
    time_input: Entity<InputState>,
    time_input_binding: Option<(NodeId, String)>,
    inspector_feedback: Option<String>,
}

enum PreviewRequest {
    Status,
    Selection(bool),
    Presentation(studio_protocol::dev_rpc::DevPresentationUpdate),
}

#[derive(Clone, Copy)]
enum FocusAction {
    EditText,
    EditTime,
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
    LayoutFlow,
    LayoutStack,
    LayoutGrid,
    LayoutAbsolute,
    LayoutOverlay,
    ProfilePhone,
    ProfileTablet,
    ProfileDesktop,
    ProfileFourK,
    CreateToken,
    ApplyToken,
    OverrideToken,
    ClearToken,
    EditToken,
    RenameToken,
    DeleteToken,
    ConfirmDeleteToken,
    CancelDeleteToken,
    PrototypeRun,
    PrototypeMode,
    PrototypeRoute,
    PrototypeGraph,
}

impl<P: DesignerPersistence + 'static> FocusView<P> {
    /// Create the GPUI adapter around one already-open model.
    #[must_use]
    pub fn new(model: FocusViewModel<P>, window: &mut Window, cx: &mut GpuiContext<Self>) -> Self {
        Self::with_workspace(
            model,
            Arc::new(studio_design::InMemoryWorkspacePersistence::default()),
            window,
            cx,
        )
    }

    /// Create the native view with the host's durable workspace adapter.
    /// Presentation state is loaded before the first frame and saved after
    /// each command-bar/view change.
    pub fn new_with_workspace_persistence(
        model: FocusViewModel<P>,
        workspace_persistence: Arc<dyn WorkspacePersistence>,
        window: &mut Window,
        cx: &mut GpuiContext<Self>,
    ) -> Self {
        Self::with_workspace(model, workspace_persistence, window, cx)
    }

    fn with_workspace(
        model: FocusViewModel<P>,
        workspace_persistence: Arc<dyn WorkspacePersistence>,
        window: &mut Window,
        cx: &mut GpuiContext<Self>,
    ) -> Self {
        let project_id = model.session_state().project_id;
        let workspace_record = block_on(workspace_persistence.load(&project_id))
            .ok()
            .flatten();
        let linked_source_root = workspace_record
            .as_ref()
            .and_then(|record| record.linked_source_root.clone());
        let workspace = workspace_record
            .clone()
            .and_then(|record| WorkspaceController::from_state(record.state).ok())
            .unwrap_or_else(|| {
                WorkspaceController::from_state(WorkspaceState::new())
                    .expect("default workspace state is valid")
            });
        let preview = cx.new(|cx| {
            NativeRuntimeView::with_surface(true, ProjectionSurface::default(), window, cx)
        });
        let preview_subscription =
            cx.subscribe(&preview, |this, _, event: &NativeViewEvent, cx| {
                this.handle_preview_event(event);
                cx.notify();
            });
        let project_controls = cx.new(|cx| {
            crate::project_controls::ProjectControls::new(
                linked_source_root.as_ref().map(std::path::PathBuf::from),
                cx,
            )
        });
        let project_controls_subscription =
            cx.subscribe(&project_controls, |this, _, event, cx| {
                match event {
                    crate::project_controls::ProjectControlsEvent::OpenFolder(root) => {
                        if this.link_project_folder(root, cx) {
                            this.project_controls
                                .update(cx, |controls, cx| controls.open_preview(cx));
                        } else if let Some(error) = this.source_feedback.clone() {
                            this.project_controls
                                .update(cx, |controls, cx| controls.show_error(error, cx));
                        }
                    }
                    crate::project_controls::ProjectControlsEvent::PreviewStatus(status) => {
                        this.preview_status = status.clone();
                        this.preview_feedback = Some(if status.is_some() {
                            "Preview connected".to_owned()
                        } else {
                            "Open a preview from the project toolbar".to_owned()
                        });
                    }
                }
                cx.notify();
            });
        let source_path_input =
            cx.new(|cx| InputState::new(window, cx).default_value("app.studio"));
        let mut view = Self {
            model: Some(model),
            root_focus: cx.focus_handle(),
            preview,
            _preview_subscription: preview_subscription,
            operation_in_flight: false,
            workspace,
            workspace_persistence,
            linked_source_root,
            project_controls,
            _project_controls_subscription: project_controls_subscription,
            script_input: None,
            _script_subscription: None,
            script_editor: None,
            script_source: String::new(),
            script_syncing: false,
            script_feedback: None,
            source_path_input,
            source_input: None,
            _source_subscription: None,
            source_view: None,
            source_event_relay: None,
            source_files: Vec::new(),
            source_feedback: None,
            source_syncing: false,
            preview_status: None,
            preview_feedback: None,
            preview_command_pending: false,
            prototype_feedback: None,
            prototype: None,
            prototype_mode: false,
            prototype_screen: None,
            prototype_interaction: None,
            token_delete_confirmation: false,
            time_input: cx.new(|cx| InputState::new(window, cx).default_value("00:00")),
            time_input_binding: None,
            inspector_feedback: None,
        };
        view.initialize_editor(window, cx);
        if view.linked_source_root.is_some() {
            view.start_source_relay(cx);
        }
        view
    }

    fn initialize_editor(&mut self, window: &mut Window, cx: &mut GpuiContext<Self>) {
        if self.script_input.is_none() {
            let source = self
                .model
                .as_ref()
                .map(FocusViewModel::script_source)
                .unwrap_or_default();
            if self.script_editor.is_none() {
                self.script_editor = self.model.as_ref().and_then(|model| {
                    ScriptDocumentAdapter::from_snapshot(&model.source_snapshot()).ok()
                });
            }
            let input = cx.new(|cx| {
                EditorState::new(window, cx)
                    .default_value(source.clone())
                    .language("studio")
            });
            let subscription = cx.subscribe(&input, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.script_source = input.read(cx).value().to_string();
                    if let Some(editor) = this.script_editor.as_mut() {
                        editor.replace_source(this.script_source.clone());
                    }
                    if !this.script_syncing {
                        this.script_feedback =
                            Some("Unsaved script buffer · press Check & Commit".to_owned());
                    }
                    cx.notify();
                }
            });
            self.script_source = source;
            self.script_input = Some(input);
            self._script_subscription = Some(subscription);
        }
    }

    fn save_workspace(&self) {
        let Some(model) = self.model.as_ref() else {
            return;
        };
        let project_id = model.session_state().project_id;
        let record = WorkspaceRecord {
            schema_version: WORKSPACE_STATE_SCHEMA_VERSION,
            project_id,
            linked_source_root: self.linked_source_root.clone(),
            state: self.workspace.state().clone(),
        };
        let _ = block_on(self.workspace_persistence.save(&record));
    }

    fn source_workspace(&self) -> Result<crate::source_workspace::ProjectSourceWorkspace, String> {
        let root = self
            .linked_source_root
            .as_deref()
            .ok_or_else(|| "Link a local project folder first".to_owned())?;
        crate::source_workspace::ProjectSourceWorkspace::open(root)
            .map_err(|error| error.to_string())
    }

    fn link_project_folder(
        &mut self,
        requested: &std::path::Path,
        cx: &mut GpuiContext<Self>,
    ) -> bool {
        match crate::source_workspace::ProjectSourceWorkspace::open(requested) {
            Ok(workspace) => {
                let root = workspace.root().to_string_lossy().into_owned();
                let changed_root = self.linked_source_root.as_deref() != Some(root.as_str());
                if !changed_root && self.source_event_relay.is_some() {
                    self.source_files = workspace.list_files().unwrap_or_default();
                    self.save_workspace();
                    self.source_feedback = Some("Project folder is already linked".to_owned());
                    return true;
                }
                if changed_root
                    && self
                        .source_view
                        .as_ref()
                        .is_some_and(crate::source_view::SourceView::is_dirty)
                {
                    self.source_feedback = Some(
                        "Save or resolve the open source buffer before changing folders".to_owned(),
                    );
                    return false;
                }
                if changed_root {
                    self.source_view = None;
                    self.source_input = None;
                    self._source_subscription = None;
                }
                self.linked_source_root = Some(root.clone());
                self.preview_status = None;
                self.preview_command_pending = false;
                self.project_controls
                    .update(cx, |controls, cx| controls.set_root(root.into(), cx));
                self.source_files = workspace.list_files().unwrap_or_default();
                self.source_event_relay = None;
                self.start_source_relay(cx);
                self.save_workspace();
                if self.source_feedback.is_none() {
                    self.source_feedback = Some("Project folder linked".to_owned());
                }
                true
            }
            Err(error) => {
                self.source_feedback = Some(format!("Could not link project folder: {error}"));
                false
            }
        }
    }

    fn start_source_relay(&mut self, cx: &mut GpuiContext<Self>) {
        let Some(root) = self.linked_source_root.clone() else {
            return;
        };
        match crate::source_events::SourceEventRelay::start(std::path::Path::new(&root)) {
            Ok(relay) => {
                self.source_event_relay = Some(relay);
                if let Ok(workspace) = self.source_workspace() {
                    self.source_files = workspace.list_files().unwrap_or_default();
                }
            }
            Err(error) => {
                self.source_feedback = Some(format!(
                    "Folder linked · live agent updates unavailable: {error}"
                ));
            }
        }
        let weak = cx.entity().downgrade();
        cx.spawn(async move |_this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let alive = weak.update(&mut cx.clone(), |this, cx| {
                    if this.linked_source_root.as_deref() != Some(root.as_str()) {
                        return false;
                    }
                    if this.source_event_relay.is_some() && this.poll_source_events() {
                        cx.notify();
                    }
                    true
                });
                if !alive.is_ok_and(|alive| alive) {
                    break;
                }
            }
        })
        .detach();
    }

    fn request_preview_control(&mut self, request: PreviewRequest, cx: &mut GpuiContext<Self>) {
        let Some(root) = self.linked_source_root.clone() else {
            self.preview_feedback = Some("Link a project folder first".to_owned());
            return;
        };
        self.preview_command_pending = true;
        self.preview_feedback = Some("Sending preview command…".to_owned());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let build_dir = std::path::PathBuf::from(&root).join("build");
            let result = cx
                .background_executor()
                .spawn(async move {
                    match request {
                        PreviewRequest::Status => studio_protocol::dev_rpc::request_status(
                            &build_dir,
                            Duration::from_secs(2),
                        ),
                        PreviewRequest::Selection(enabled) => {
                            studio_protocol::dev_rpc::set_authoring_selection(
                                &build_dir,
                                enabled,
                                Duration::from_secs(2),
                            )
                        }
                        PreviewRequest::Presentation(update) => {
                            studio_protocol::dev_rpc::set_presentation(
                                &build_dir,
                                &update,
                                Duration::from_secs(2),
                            )
                        }
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                if this.linked_source_root.as_deref() != Some(root.as_str()) {
                    return;
                }
                this.preview_command_pending = false;
                match result {
                    Ok(status) => {
                        this.preview_status = Some(status);
                        this.preview_feedback = Some("Preview updated".to_owned());
                    }
                    Err(error) if error.is_not_running() => {
                        this.preview_status = None;
                        this.preview_feedback = Some("No studio dev preview running".to_owned());
                    }
                    Err(error) => {
                        this.preview_feedback = Some(format!("Preview command failed: {error}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn poll_source_events(&mut self) -> bool {
        let mut changed = false;
        loop {
            let event = match self
                .source_event_relay
                .as_ref()
                .map(crate::source_events::SourceEventRelay::try_recv)
            {
                Some(Ok(event)) => event,
                Some(Err(
                    std::sync::mpsc::TryRecvError::Empty
                    | std::sync::mpsc::TryRecvError::Disconnected,
                ))
                | None => break,
            };
            let Ok(workspace) = self.source_workspace() else {
                continue;
            };
            self.source_files = workspace.list_files().unwrap_or_default();
            changed = true;
            if !self
                .source_view
                .as_ref()
                .is_some_and(|source_view| source_view.path() == event.path)
            {
                continue;
            }
            let current = workspace
                .read_file(&event.path)
                .map_err(|error| error.to_string());
            match current {
                Ok(snapshot) => match self
                    .source_view
                    .as_mut()
                    .expect("matching source view remains open")
                    .observe_agent_change(event, snapshot)
                {
                    Ok(crate::source_view::SourceRefresh::Refreshed) => {
                        self.source_feedback = Some("Source refreshed from agent edit".to_owned());
                        changed = true;
                    }
                    Ok(crate::source_view::SourceRefresh::Conflict) => {
                        self.source_feedback =
                            Some("Agent edit conflicts with unsaved source changes".to_owned());
                        changed = true;
                    }
                    Err(error) => {
                        self.source_feedback =
                            Some(format!("Source update could not be applied: {error}"));
                        changed = true;
                    }
                },
                Err(error) => {
                    self.source_feedback =
                        Some(format!("Could not read agent-edited file: {error}"));
                    changed = true;
                }
            }
        }
        changed
    }

    fn open_source_file(&mut self, path: String, window: &mut Window, cx: &mut GpuiContext<Self>) {
        let snapshot = match self.source_workspace().and_then(|workspace| {
            workspace
                .read_file(&path)
                .map_err(|error| error.to_string())
        }) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.source_feedback = Some(format!("Could not open source file: {error}"));
                return;
            }
        };
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .default_value(snapshot.content.clone())
                .language(source_language(&snapshot.path))
        });
        let subscription = cx.subscribe(&editor, |this, editor, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) && !this.source_syncing {
                if let Some(source_view) = this.source_view.as_mut() {
                    source_view.replace_buffer(editor.read(cx).value().to_string());
                    this.source_feedback = Some("Unsaved source changes".to_owned());
                }
                cx.notify();
            }
        });
        self.source_path_input
            .update(cx, |input, cx| input.set_value(path, window, cx));
        self.source_view = Some(crate::source_view::SourceView::open(snapshot));
        self.source_input = Some(editor);
        self._source_subscription = Some(subscription);
        self.source_feedback = Some("Source file opened".to_owned());
    }

    fn open_source_path(&mut self, window: &mut Window, cx: &mut GpuiContext<Self>) {
        let path = self.source_path_input.read(cx).value().to_string();
        self.open_source_file(path.trim().to_owned(), window, cx);
    }

    fn open_preview_source(
        &mut self,
        path: String,
        line: u64,
        column: u64,
        offset: u64,
        window: &mut Window,
        cx: &mut GpuiContext<Self>,
    ) {
        self.open_source_file(path.clone(), window, cx);
        if self.source_view.is_some() {
            if let Ok(offset) = usize::try_from(offset)
                && let Some(input) = self.source_input.as_ref()
            {
                input.update(cx, |editor, cx| {
                    editor.set_selected_range(offset..offset, cx)
                });
            }
            self.source_feedback = Some(format!("Opened preview source · {path}:{line}:{column}"));
        }
    }

    fn save_source_file(&mut self) {
        let Some(source_view) = self.source_view.as_ref() else {
            return;
        };
        let plan = match source_view.save_plan() {
            Ok(plan) => plan,
            Err(error) => {
                self.source_feedback = Some(error.to_string());
                return;
            }
        };
        let result = self.source_workspace().and_then(|workspace| {
            workspace
                .write_file(&plan.path, &plan.content, Some(&plan.expected_revision))
                .map_err(|error| error.to_string())
        });
        match result {
            Ok(snapshot) => {
                if let Some(source_view) = self.source_view.as_mut() {
                    let _ = source_view.accept_saved(snapshot, &plan.buffer_at_request);
                }
                self.source_feedback = Some("Source saved · studio dev will reload it".to_owned());
            }
            Err(error) => {
                self.source_feedback = Some(format!("Source save failed: {error}"));
            }
        }
    }

    fn accept_incoming_source(&mut self) {
        if let Some(source_view) = self.source_view.as_mut() {
            source_view.accept_disk();
            self.source_feedback = Some("Using the latest saved source".to_owned());
        }
    }

    fn keep_local_source(&mut self) {
        if let Some(source_view) = self.source_view.as_mut() {
            source_view.keep_local();
            self.source_feedback =
                Some("Local source kept · save will check the incoming revision".to_owned());
        }
    }

    fn revert_agent_source(&mut self) {
        let Some(source_view) = self.source_view.as_ref() else {
            return;
        };
        let plan = match source_view.revert_plan() {
            Ok(plan) => plan,
            Err(error) => {
                self.source_feedback = Some(error.to_string());
                return;
            }
        };
        let result = self.source_workspace().and_then(|workspace| {
            workspace
                .write_file(&plan.path, &plan.content, Some(&plan.expected_revision))
                .map_err(|error| error.to_string())
        });
        match result {
            Ok(snapshot) => {
                if let Some(source_view) = self.source_view.as_mut() {
                    let _ = source_view.accept_saved(snapshot, &plan.buffer_at_request);
                }
                self.source_feedback = Some("Agent edit reverted with a revision check".to_owned());
            }
            Err(error) => {
                self.source_feedback = Some(format!("Agent edit was not reverted: {error}"));
            }
        }
    }

    fn execute_workspace(&mut self, command: WorkspaceCommand) {
        if let Some(model) = self.model.as_ref() {
            let _ = self.workspace.execute(command, model.session());
            self.save_workspace();
        }
    }

    fn enter_prototype(&mut self) {
        let Some(model) = self.model.as_ref() else {
            return;
        };
        let snapshot = model.source_snapshot();
        let screen_id = self
            .prototype_screen
            .clone()
            .or_else(|| snapshot.design.screen_order.first().cloned());
        let Some(screen_id) = screen_id else {
            self.prototype_feedback =
                Some("Prototype unavailable · no screens declared".to_owned());
            return;
        };
        match studio_design::PrototypeSession::new_at(&snapshot.design, screen_id.clone()) {
            Ok(prototype) => {
                self.prototype = Some(prototype);
                self.prototype_screen = Some(screen_id);
                self.prototype_mode = true;
                self.prototype_feedback = Some(
                    "Prototype mode active · choose a route or interaction, then run".to_owned(),
                );
            }
            Err(error) => {
                self.prototype_feedback = Some(format!("Prototype diagnostic: {error}"));
            }
        }
    }

    fn leave_prototype(&mut self) {
        self.prototype_mode = false;
        self.prototype = None;
        self.prototype_feedback = Some("Returned to Design mode · source unchanged".to_owned());
    }

    fn select_prototype_route(&mut self, screen_id: studio_design::ScreenId) {
        let Some(model) = self.model.as_ref() else {
            return;
        };
        let snapshot = model.source_snapshot();
        match studio_design::PrototypeSession::new_at(&snapshot.design, screen_id.clone()) {
            Ok(prototype) => {
                let route = prototype.active_route().unwrap_or("/").to_owned();
                self.prototype = Some(prototype);
                self.prototype_screen = Some(screen_id.clone());
                self.prototype_mode = true;
                self.prototype_feedback = Some(format!(
                    "Prototype route selected · {screen_id} · {route} · ephemeral stack reset"
                ));
            }
            Err(error) => self.prototype_feedback = Some(format!("Route diagnostic: {error}")),
        }
    }

    fn select_prototype_interaction(&mut self, interaction_id: InteractionId) {
        if !self.prototype_mode {
            self.enter_prototype();
        }
        self.prototype_interaction = Some(interaction_id.clone());
        self.prototype_feedback = Some(format!(
            "Interaction selected · {interaction_id} · press Run Prototype"
        ));
    }

    fn run_prototype(&mut self) {
        if !self.prototype_mode {
            self.enter_prototype();
        }
        let Some(prototype) = self.prototype.as_mut() else {
            return;
        };
        let interaction_id = self.prototype_interaction.clone().or_else(|| {
            prototype
                .interaction_graph()
                .entries()
                .keys()
                .next()
                .cloned()
        });
        let Some(interaction_id) = interaction_id else {
            self.prototype_feedback = Some("Prototype ready · no interactions declared".to_owned());
            return;
        };
        self.prototype_interaction = Some(interaction_id.clone());
        let dispatch = prototype.dispatch_interaction(&interaction_id);
        self.prototype_screen = dispatch.state.active_screen_id.clone();
        self.prototype_feedback = Some(format!(
            "Prototype run · screen {} · route {} · {} effect(s) · {} diagnostic(s)",
            dispatch
                .state
                .active_screen_id
                .as_ref()
                .map_or_else(|| "none".to_owned(), ToString::to_string),
            prototype.active_route().unwrap_or("/"),
            dispatch.trace.len(),
            dispatch.diagnostics.len()
        ));
    }

    fn start_action(&mut self, action: FocusAction, cx: &mut GpuiContext<Self>) {
        if self.operation_in_flight {
            return;
        }
        match action {
            FocusAction::DeleteToken => {
                self.token_delete_confirmation = true;
                cx.notify();
                return;
            }
            FocusAction::CancelDeleteToken => {
                self.token_delete_confirmation = false;
                cx.notify();
                return;
            }
            FocusAction::ProfilePhone
            | FocusAction::ProfileTablet
            | FocusAction::ProfileDesktop
            | FocusAction::ProfileFourK => {
                if let Some(model) = self.model.as_mut() {
                    let profile = match action {
                        FocusAction::ProfilePhone => "phone-portrait",
                        FocusAction::ProfileTablet => "tablet-portrait",
                        FocusAction::ProfileDesktop => "desktop-landscape",
                        FocusAction::ProfileFourK => "4k-landscape",
                        _ => unreachable!(),
                    };
                    model.set_profile(Some(profile.to_owned()));
                }
                cx.notify();
                return;
            }
            FocusAction::PrototypeRun => {
                self.run_prototype();
                cx.notify();
                return;
            }
            FocusAction::PrototypeMode => {
                if self.prototype_mode {
                    self.leave_prototype();
                } else {
                    self.enter_prototype();
                }
                cx.notify();
                return;
            }
            FocusAction::PrototypeRoute => {
                if let Some(screen_id) = self
                    .model
                    .as_ref()
                    .and_then(|model| model.source_snapshot().design.screen_order.first().cloned())
                {
                    self.select_prototype_route(screen_id);
                }
                cx.notify();
                return;
            }
            FocusAction::PrototypeGraph => {
                if let Some(interaction_id) = self.model.as_ref().and_then(|model| {
                    model
                        .source_snapshot()
                        .design
                        .interactions
                        .keys()
                        .next()
                        .cloned()
                }) {
                    self.select_prototype_interaction(interaction_id);
                }
                cx.notify();
                return;
            }
            _ => {}
        }
        let time_value = self.time_input.read(cx).value().to_string();
        if matches!(action, FocusAction::EditTime)
            && !studio_protocol::is_valid_time_value(&time_value)
        {
            self.inspector_feedback = Some("Enter a 24-hour time as HH:MM or HH:MM:SS.".to_owned());
            cx.notify();
            return;
        }
        self.inspector_feedback = None;
        let Some(mut model) = self.model.take() else {
            return;
        };
        let revision = model.snapshot().revision_id.get().saturating_add(1);
        let (name, undo_name) = match action {
            FocusAction::EditText => ("focus-edit", "focus-text-edit"),
            FocusAction::EditTime => ("focus-edit-time", "focus-time-edit"),
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
            FocusAction::LayoutFlow => ("focus-layout-flow", "focus-layout-flow"),
            FocusAction::LayoutStack => ("focus-layout-stack", "focus-layout-stack"),
            FocusAction::LayoutGrid => ("focus-layout-grid", "focus-layout-grid"),
            FocusAction::LayoutAbsolute => ("focus-layout-absolute", "focus-layout-absolute"),
            FocusAction::LayoutOverlay => ("focus-layout-overlay", "focus-layout-overlay"),
            FocusAction::CreateToken => ("focus-token-create", "focus-token-create"),
            FocusAction::ApplyToken => ("focus-token-apply", "focus-token-apply"),
            FocusAction::OverrideToken => ("focus-token-override", "focus-token-override"),
            FocusAction::ClearToken => ("focus-token-clear", "focus-token-clear"),
            FocusAction::EditToken => ("focus-token-edit", "focus-token-edit"),
            FocusAction::RenameToken => ("focus-token-rename", "focus-token-rename"),
            FocusAction::ConfirmDeleteToken => {
                ("focus-token-delete-confirm", "focus-token-delete-confirm")
            }
            FocusAction::CancelDeleteToken | FocusAction::DeleteToken => {
                unreachable!("token delete confirmation handled above")
            }
            FocusAction::ProfilePhone
            | FocusAction::ProfileTablet
            | FocusAction::ProfileDesktop
            | FocusAction::ProfileFourK
            | FocusAction::PrototypeRun
            | FocusAction::PrototypeMode
            | FocusAction::PrototypeRoute
            | FocusAction::PrototypeGraph => unreachable!("synchronous action handled above"),
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
            let outcome = match action {
                FocusAction::EditText => model.edit_text(operation_id, actor, undo_group_id).await,
                FocusAction::EditTime => {
                    model
                        .edit_property(
                            operation_id,
                            actor,
                            undo_group_id,
                            "Edit time",
                            "value",
                            Some(PropertyValue::String(time_value)),
                        )
                        .await
                }
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
                FocusAction::LayoutFlow => {
                    model
                        .set_layout_selected(
                            operation_id,
                            actor,
                            undo_group_id,
                            LayoutProperties::flow(),
                        )
                        .await
                }
                FocusAction::LayoutStack => {
                    model
                        .set_layout_selected(
                            operation_id,
                            actor,
                            undo_group_id,
                            LayoutProperties::default(),
                        )
                        .await
                }
                FocusAction::LayoutGrid => {
                    let mut layout = LayoutProperties::flow();
                    layout.grid_columns = Some(2);
                    model
                        .set_layout_selected(operation_id, actor, undo_group_id, layout)
                        .await
                }
                FocusAction::LayoutAbsolute => {
                    model
                        .set_layout_selected(
                            operation_id,
                            actor,
                            undo_group_id,
                            LayoutProperties::absolute(),
                        )
                        .await
                }
                FocusAction::LayoutOverlay => {
                    model
                        .set_layout_selected(
                            operation_id,
                            actor,
                            undo_group_id,
                            LayoutProperties::overlay(),
                        )
                        .await
                }
                FocusAction::CreateToken => {
                    model
                        .create_focus_token(operation_id, actor, undo_group_id)
                        .await
                }
                FocusAction::ApplyToken => {
                    model
                        .apply_focus_token(operation_id, actor, undo_group_id)
                        .await
                }
                FocusAction::OverrideToken => {
                    model
                        .override_focus_token(operation_id, actor, undo_group_id)
                        .await
                }
                FocusAction::ClearToken => {
                    model
                        .clear_focus_token(operation_id, actor, undo_group_id)
                        .await
                }
                FocusAction::EditToken => {
                    model
                        .edit_focus_token(operation_id, actor, undo_group_id)
                        .await
                }
                FocusAction::RenameToken => {
                    model
                        .rename_focus_token(operation_id, actor, undo_group_id)
                        .await
                }
                FocusAction::ConfirmDeleteToken => {
                    model
                        .delete_focus_token(operation_id, actor, undo_group_id)
                        .await
                }
                FocusAction::CancelDeleteToken | FocusAction::DeleteToken => {
                    unreachable!("token delete confirmation handled above")
                }
                FocusAction::ProfilePhone
                | FocusAction::ProfileTablet
                | FocusAction::ProfileDesktop
                | FocusAction::ProfileFourK
                | FocusAction::PrototypeRun
                | FocusAction::PrototypeMode
                | FocusAction::PrototypeRoute
                | FocusAction::PrototypeGraph => unreachable!("synchronous action handled above"),
            };
            this.update(cx, |this, cx| {
                this.model = Some(model);
                this.operation_in_flight = false;
                if matches!(outcome, CommandOutcome::Accepted(_)) {
                    this.token_delete_confirmation = false;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn start_script_commit(&mut self, cx: &mut GpuiContext<Self>) {
        if self.operation_in_flight {
            return;
        }
        let Some(mut model) = self.model.take() else {
            return;
        };
        let mut editor = self
            .script_editor
            .take()
            .or_else(|| ScriptDocumentAdapter::from_snapshot(&model.source_snapshot()).ok());
        let source = self
            .script_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_else(|| model.script_source());
        if let Some(editor) = editor.as_mut() {
            editor.replace_source(source.clone());
        }
        let revision = model.snapshot().revision_id.get().saturating_add(1);
        let Ok(operation_id) = OperationId::new(format!("focus-script-{revision}")) else {
            self.model = Some(model);
            return;
        };
        let Ok(undo_group_id) = UndoGroupId::new("focus-script") else {
            self.model = Some(model);
            return;
        };
        let Ok(actor) = focus_actor() else {
            self.model = Some(model);
            return;
        };
        let metadata =
            ScriptCommitMetadata::new(operation_id, actor, undo_group_id, "Studio Script edit");
        self.operation_in_flight = true;
        self.script_feedback = Some("Checking Studio Script…".to_owned());
        cx.notify();
        cx.spawn(async move |this, cx| {
            let outcome = model.commit_script_source(source, metadata).await;
            if matches!(&outcome, ScriptCommitOutcome::Committed { .. })
                && let Some(editor) = editor.as_mut()
            {
                let _ = editor.refresh_from_snapshot(&model.source_snapshot());
            }
            let feedback = match &outcome {
                ScriptCommitOutcome::Committed { receipt, .. } => format!(
                    "Script committed at revision {:?}",
                    receipt.committed_revision
                ),
                ScriptCommitOutcome::NoChanges { .. } => {
                    "Script checked · no design changes".to_owned()
                }
                ScriptCommitOutcome::Invalid { diagnostics } => format!(
                    "Script rejected · {} line-linked diagnostic(s)",
                    diagnostics.len()
                ),
                ScriptCommitOutcome::Session(command_outcome) => {
                    format!("Script session result: {command_outcome:?}")
                }
            };
            this.update(cx, |this, cx| {
                this.model = Some(model);
                this.script_editor = editor;
                this.operation_in_flight = false;
                this.script_feedback = Some(feedback);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn start_script_check(&mut self, cx: &mut GpuiContext<Self>) {
        let Some(editor) = self.script_editor.as_mut() else {
            self.script_feedback = Some("Studio Script editor is still loading".to_owned());
            cx.notify();
            return;
        };
        if let Some(input) = self.script_input.as_ref() {
            editor.replace_source(input.read(cx).value().to_string());
        }
        let snapshot = editor.snapshot();
        self.script_feedback = Some(if snapshot.diagnostics.is_empty() {
            format!(
                "Check passed · {} syntax token(s) · {} outline node(s) · {} base revision",
                snapshot.syntax.len(),
                snapshot.outline.len(),
                snapshot.base_revision.get()
            )
        } else {
            format!(
                "Check found {} line-linked diagnostic(s)",
                snapshot.diagnostics.len()
            )
        });
        cx.notify();
    }

    fn start_script_format(&mut self, window: &mut Window, cx: &mut GpuiContext<Self>) {
        let Some(editor) = self.script_editor.as_mut() else {
            self.script_feedback = Some("Studio Script editor is still loading".to_owned());
            cx.notify();
            return;
        };
        let snapshot = editor.format_source();
        self.script_source = snapshot.source.clone();
        if let Some(input) = self.script_input.as_ref() {
            input.update(cx, |input, cx| {
                input.set_value(snapshot.source.clone(), window, cx)
            });
        }
        self.script_feedback = Some(format!(
            "Formatted · {} syntax token(s) · committing parser-of-record buffer",
            snapshot.syntax.len()
        ));
        self.start_script_commit(cx);
    }

    fn handle_preview_event(&mut self, event: &NativeViewEvent) {
        match event {
            NativeViewEvent::Selected(node_id) if !self.prototype_mode => {
                if let Some(model) = self.model.as_mut() {
                    let _ = model.select_str(node_id);
                }
            }
            NativeViewEvent::Input { node_id, action } if self.prototype_mode => {
                let Ok(node_id) = NodeId::new(node_id.clone()) else {
                    return;
                };
                let event = match action {
                    InputAction::PointerClick
                    | InputAction::KeyboardActivate
                    | InputAction::TouchActivate => studio_design::InteractionEvent::Pressed,
                    InputAction::TextChanged { .. }
                    | InputAction::SelectionChanged { .. }
                    | InputAction::SliderDrag { .. } => studio_design::InteractionEvent::Changed,
                    InputAction::Scroll { .. } => return,
                };
                if let Some(prototype) = self.prototype.as_mut() {
                    let dispatch = prototype.dispatch_event(node_id, event);
                    self.prototype_screen = dispatch.state.active_screen_id;
                    self.prototype_feedback = Some(format!(
                        "Prototype · {} effect(s) · {} diagnostic(s)",
                        dispatch.trace.len(),
                        dispatch.diagnostics.len()
                    ));
                }
            }
            _ => {}
        }
    }

    fn preview_root(&self, snapshot: &FocusViewSnapshot) -> Option<UiNode> {
        let Some(prototype) = self.prototype.as_ref().filter(|_| self.prototype_mode) else {
            return snapshot.canvas.clone();
        };
        let model = self.model.as_ref()?;
        let mut source = model.source_snapshot();
        source.design = prototype.design().clone();
        let state = prototype.state();
        for (id, properties) in state.properties {
            if let Some(node) = source.design.nodes.get_mut(&id) {
                node.properties.extend(properties);
            }
        }
        for (id, visible) in state.visibility {
            if let Some(node) = source.design.nodes.get_mut(&id) {
                node.properties
                    .insert("visible".to_owned(), PropertyValue::Boolean(visible));
            }
        }
        let mut options = model.effective_projection_options();
        options.screen_id = state.active_screen_id;
        studio_design::project_report(&source, model.library.as_ref(), options).root
    }
}

impl<P: DesignerPersistence + 'static> Render for FocusView<P> {
    fn render(&mut self, window: &mut Window, cx: &mut GpuiContext<Self>) -> impl IntoElement {
        if self.model.is_none() {
            return div()
                .id("studio-focus-loading")
                .role(Role::Status)
                .aria_label("Designer operation in progress")
                .p_4()
                .child("Saving Designer changes…");
        }
        // Canvas commands commit through the shared session before this view
        // is rendered again. Rebase a clean editor buffer to that new source
        // revision so canvas edits appear as a canonical text update. A dirty
        // buffer is deliberately left alone; overwriting authored text would
        // violate the editor's invalid-edit isolation guarantee.
        if let (Some(model), Some(editor)) = (self.model.as_ref(), self.script_editor.as_mut()) {
            let source_snapshot = model.source_snapshot();
            if editor.base_revision() != source_snapshot.revision.id && !editor.is_dirty() {
                if let Ok(editor_snapshot) = editor.refresh_from_snapshot(&source_snapshot) {
                    let source = editor_snapshot.source.clone();
                    self.script_source = source.clone();
                    if let Some(input) = self.script_input.as_ref().cloned()
                        && input.read(cx).value().to_string() != source
                    {
                        self.script_syncing = true;
                        input.update(cx, |input, cx| input.set_value(source, window, cx));
                        self.script_syncing = false;
                    }
                }
            }
        }
        if let (Some(source_view), Some(input)) =
            (self.source_view.as_ref(), self.source_input.as_ref())
            && !source_view.is_dirty()
        {
            let source = source_view.buffer().to_owned();
            if input.read(cx).value().to_string() != source {
                self.source_syncing = true;
                input.update(cx, |input, cx| input.set_value(source, window, cx));
                self.source_syncing = false;
            }
        }
        let snapshot = self
            .model
            .as_ref()
            .expect("FocusView model is present after loading guard")
            .snapshot();
        let selected = snapshot.selected_node_id.clone();
        let time_binding = snapshot.selected_node.as_ref().and_then(|node| {
            if !matches!(
                node.source,
                studio_design::DesignNodeSource::Primitive {
                    kind: studio_protocol::NodeKind::TimePicker
                }
            ) {
                return None;
            }
            let value = match node.properties.get("value") {
                Some(PropertyValue::String(value)) => value.clone(),
                _ => "00:00".to_owned(),
            };
            Some((node.id.clone(), value))
        });
        if self.time_input_binding != time_binding {
            if let Some((_, value)) = &time_binding {
                self.time_input
                    .update(cx, |input, cx| input.set_value(value.clone(), window, cx));
            }
            self.time_input_binding = time_binding;
            self.inspector_feedback = None;
        }
        let is_time = self.time_input_binding.is_some();
        let preview_root = self.preview_root(&snapshot);
        let tree = preview_root.as_ref().map(NativeRenderNode::from);
        self.preview.update(cx, |preview, cx| {
            preview.update_surface(
                |surface| {
                    if surface.root == tree {
                        return false;
                    }
                    surface.root = tree;
                    true
                },
                cx,
            );
            preview.set_authoring(
                !self.prototype_mode,
                selected.as_ref().map(ToString::to_string),
                cx,
            );
        });
        let viewport = self
            .model
            .as_ref()
            .and_then(FocusViewModel::selected_profile)
            .map(|profile| profile.content_viewport());
        let canvas = preview_root
            .as_ref()
            .map(|_| {
                div()
                    .id("focus-canvas-surface")
                    .role(Role::Group)
                    .aria_label("Live design canvas")
                    .when(viewport.is_none(), |element| {
                        element.min_h(px(480.0)).w_full()
                    })
                    .when_some(viewport, |element, viewport| {
                        element
                            .w(px(viewport.width as f32))
                            .h(px(viewport.height as f32))
                            .flex_shrink_0()
                    })
                    .child(self.preview.clone())
                    .into_any_element()
            })
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
        let active_view = self.workspace.state().active_view;
        let profile = self
            .model
            .as_ref()
            .and_then(|model| model.session_state().device_profile)
            .unwrap_or_else(|| "base".to_owned());
        let token_count = self.model.as_ref().map_or(0, |model| model.tokens().len());
        let token_entries = self
            .model
            .as_ref()
            .map(|model| {
                model
                    .tokens()
                    .into_iter()
                    .map(|token| {
                        let usages = model.token_usages(token.id.clone());
                        (token, usages)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let token_delete_usages = self
            .model
            .as_ref()
            .and_then(|model| model.tokens().into_iter().next())
            .map(|token| {
                self.model
                    .as_ref()
                    .map(|model| model.token_usages(token.id))
                    .unwrap_or_default()
            })
            .unwrap_or_default()
            .into_iter()
            .map(|usage| {
                format!(
                    "{} · {} · {}",
                    usage.owner,
                    usage.property,
                    usage
                        .node_id
                        .map_or_else(|| "global".to_owned(), |node_id| node_id.to_string())
                )
            })
            .collect::<Vec<_>>();
        let token_provenance = self
            .model
            .as_ref()
            .map(FocusViewModel::node_token_values)
            .unwrap_or_default();
        let prototype_routes = self
            .model
            .as_ref()
            .map(|model| {
                model
                    .source_snapshot()
                    .design
                    .screen_order
                    .iter()
                    .filter_map(|screen_id| {
                        model
                            .source_snapshot()
                            .design
                            .screens
                            .get(screen_id)
                            .map(|screen| {
                                (screen.id.clone(), screen.name.clone(), screen.route.clone())
                            })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let prototype_interactions = self
            .model
            .as_ref()
            .map(|model| {
                studio_design::InteractionGraph::from_design(&model.source_snapshot().design)
                    .entries()
                    .values()
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let script_snapshot = self
            .script_editor
            .as_ref()
            .map(ScriptDocumentAdapter::snapshot)
            .unwrap_or_else(|| EditorSnapshot {
                source: self.script_source.clone(),
                diagnostics: Vec::new(),
                syntax: Vec::new(),
                outline: Vec::new(),
                dirty: false,
                base_revision: snapshot.revision_id,
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
        let command_button = |id: &'static str,
                              label: &'static str,
                              command: WorkspaceCommand,
                              cx: &mut GpuiContext<Self>| {
            Button::new(id)
                .label(label)
                .secondary()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.execute_workspace(command);
                    cx.notify();
                }))
        };
        let toolbar = div()
            .id("focus-command-bar")
            .role(Role::Toolbar)
            .aria_label("Designer command bar")
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                Button::new("focus-view-toggle")
                    .label(match active_view {
                        studio_design::EditorView::Focus => "Open Workbench",
                        studio_design::EditorView::Workbench => "Open Focus",
                    })
                    .primary()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.execute_workspace(WorkspaceCommand::ToggleView);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("focus-profile-current")
                    .text_sm()
                    .child(format!("Profile: {profile}")),
            )
            .child(action_button(
                "focus-profile-phone",
                "Phone",
                FocusAction::ProfilePhone,
                false,
                cx,
            ))
            .child(action_button(
                "focus-profile-tablet",
                "Tablet",
                FocusAction::ProfileTablet,
                false,
                cx,
            ))
            .child(action_button(
                "focus-profile-desktop",
                "Desktop",
                FocusAction::ProfileDesktop,
                false,
                cx,
            ))
            .child(action_button(
                "focus-profile-4k",
                "4K",
                FocusAction::ProfileFourK,
                false,
                cx,
            ))
            .child(action_button(
                "focus-prototype-run",
                "Run Prototype",
                FocusAction::PrototypeRun,
                false,
                cx,
            ))
            .child(action_button(
                "focus-prototype-mode",
                if self.prototype_mode {
                    "Return to Design"
                } else {
                    "Enter Prototype"
                },
                FocusAction::PrototypeMode,
                false,
                cx,
            ))
            .child(action_button(
                "focus-prototype-route",
                "Select start route",
                FocusAction::PrototypeRoute,
                false,
                cx,
            ))
            .child(action_button(
                "focus-prototype-graph",
                "Select interaction",
                FocusAction::PrototypeGraph,
                false,
                cx,
            ))
            .child(command_button(
                "focus-command-hierarchy",
                "Hierarchy",
                WorkspaceCommand::OpenScreensHierarchy,
                cx,
            ))
            .child(command_button(
                "focus-command-library",
                "Library",
                WorkspaceCommand::OpenLibrary,
                cx,
            ))
            .child(command_button(
                "focus-command-diagnostics",
                "Diagnostics",
                WorkspaceCommand::OpenDiagnostics,
                cx,
            ))
            .child(command_button(
                "focus-command-history",
                "History",
                WorkspaceCommand::OpenHistory,
                cx,
            ));
        let workbench_surface = if active_view == studio_design::EditorView::Workbench {
            let panel_label = |id: &'static str, panel: studio_design::PanelId, content: String| {
                let state = self.workspace.state().workbench.panel(panel);
                div().id(id).role(Role::Pane).child(format!(
                    "{content} · {}x{} at {},{} · {}",
                    state.geometry.width,
                    state.geometry.height,
                    state.geometry.x,
                    state.geometry.y,
                    if state.collapsed { "collapsed" } else { "open" }
                ))
            };
            let hierarchy_count = hierarchy
                .as_ref()
                .map_or(0, |hierarchy| hierarchy.roots.len());
            let diagnostic_count = diagnostic_details.len();
            let interaction_count = prototype_interactions.len();
            div()
                .id("studio-workbench-view")
                .role(Role::Pane)
                .aria_label("Studio Designer Workbench")
                .flex()
                .flex_wrap()
                .gap_2()
                .p_2()
                .border_1()
                .border_color(rgb(COLOR_BORDER))
                .bg(rgb(COLOR_PANEL))
                .child(panel_label(
                    "workbench-screens-hierarchy",
                    studio_design::PanelId::ScreensHierarchy,
                    format!("Screens & Hierarchy · {hierarchy_count} root(s)"),
                ))
                .child(panel_label(
                    "workbench-library",
                    studio_design::PanelId::Library,
                    "Library · authoritative snapshot".to_owned(),
                ))
                .child(panel_label(
                    "workbench-canvas-controls",
                    studio_design::PanelId::CanvasControls,
                    format!("Canvas & Profiles · {profile}"),
                ))
                .child(panel_label(
                    "workbench-inspector",
                    studio_design::PanelId::Inspector,
                    format!("Inspector · {selected_label}"),
                ))
                .child(panel_label(
                    "workbench-diagnostics",
                    studio_design::PanelId::Diagnostics,
                    format!("Diagnostics · {diagnostic_count} issue(s)"),
                ))
                .child(panel_label(
                    "workbench-interactions",
                    studio_design::PanelId::Interactions,
                    format!("Interactions · {interaction_count} graph node(s)"),
                ))
                .child(panel_label(
                    "workbench-agent-activity",
                    studio_design::PanelId::AgentActivity,
                    "Agent Activity · shared session state".to_owned(),
                ))
                .child(panel_label(
                    "workbench-history",
                    studio_design::PanelId::History,
                    "History · immutable revisions and undo".to_owned(),
                ))
                .into_any_element()
        } else {
            div().id("studio-workbench-view-hidden").into_any_element()
        };
        let script_input = self
            .script_input
            .as_ref()
            .expect("script input initialized before render")
            .clone();
        let script_feedback = self
            .script_feedback
            .clone()
            .unwrap_or_else(|| "Studio Script · parser-of-record".to_owned());
        let source_path_input = self.source_path_input.clone();
        let source_input = self.source_input.clone();
        let source_feedback = self.source_feedback.clone().unwrap_or_else(|| {
            "Link a local source folder to edit files used by studio dev".to_owned()
        });
        let source_status = self.source_view.as_ref().map_or_else(
            || "No source file open".to_owned(),
            |source_view| {
                let state = if source_view.conflict().is_some() {
                    "Conflict"
                } else if source_view.is_dirty() {
                    "Unsaved changes"
                } else {
                    "Saved"
                };
                format!("{} · {state}", source_view.path())
            },
        );
        let source_conflict = self
            .source_view
            .as_ref()
            .and_then(crate::source_view::SourceView::conflict);
        let source_diff = source_conflict.map(source_conflict_summary);
        let source_conflict_agent = source_conflict.map(|conflict| conflict.client_name.clone());
        let source_has_conflict = source_conflict.is_some();
        let source_is_dirty = self
            .source_view
            .as_ref()
            .is_some_and(crate::source_view::SourceView::is_dirty);
        let source_can_revert = self
            .source_view
            .as_ref()
            .is_some_and(|source_view| source_view.revert_plan().is_ok());
        let preview_status = self.preview_status.clone();
        let preview_selection = preview_status
            .as_ref()
            .and_then(|status| status.selection.clone());
        let preview_presentation = preview_status
            .as_ref()
            .map(|status| status.presentation.clone())
            .unwrap_or_default();
        let preview_enabled = preview_status.is_some();
        let preview_selection_mode = preview_status
            .as_ref()
            .is_some_and(|status| status.authoring_selection);
        let preview_selection_label = preview_selection.as_ref().map_or_else(
            || "No component selected".to_owned(),
            |selection| {
                format!(
                    "Selected node {} · component {} · screen {}",
                    selection.node_id,
                    selection.component_id.as_deref().unwrap_or("unknown"),
                    selection.screen_id.as_deref().unwrap_or("unknown"),
                )
            },
        );
        let preview_source_link = preview_selection.as_ref().and_then(|selection| {
            selection.source.as_ref().map(|source| {
                (
                    source.path.clone(),
                    source.line,
                    source.column,
                    source.offset,
                )
            })
        });
        let preview_screen_source = preview_presentation.screen_source_path.clone();
        let preview_screen_ids = preview_presentation.available_screen_ids.clone();
        let preview_screen_tabs = preview_screen_ids
            .iter()
            .map(|screen_id| Tab::new().label(screen_id.clone()))
            .collect::<Vec<_>>();
        let preview_active_screen_index =
            preview_presentation.screen_id.as_ref().and_then(|active| {
                preview_screen_ids
                    .iter()
                    .position(|screen| screen == active)
            });
        let preview_status_text = preview_status.as_ref().map_or_else(
            || "Preview not connected".to_owned(),
            |status| {
                format!(
                    "{} · revision {} · screen {} · {}×{} · {:?} · zoom {}%",
                    format!("{:?}", status.phase),
                    status.applied_revision,
                    status
                        .presentation
                        .screen_id
                        .as_deref()
                        .unwrap_or("loading"),
                    status.presentation.viewport.width,
                    status.presentation.viewport.height,
                    status.presentation.orientation,
                    status.presentation.zoom_milli / 10,
                )
            },
        );
        let preview_feedback = self
            .preview_feedback
            .clone()
            .unwrap_or_else(|| "Studio dev status is checked automatically".to_owned());
        let preview_command_pending = self.preview_command_pending;
        let prototype_feedback = self
            .prototype_feedback
            .clone()
            .unwrap_or_else(|| "Prototype preview is isolated from source history".to_owned());
        div()
            .id("studio-focus-view")
            .role(Role::Application)
            .aria_label("Studio Designer Focus View")
            .key_context("DesignerProject")
            .track_focus(&self.root_focus)
            .on_action(cx.listener(|this, _: &crate::project_controls::OpenProjectFolder, _, cx| {
                this.project_controls.update(cx, |controls, cx| controls.choose_folder(cx));
            }))
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(rgb(COLOR_CANVAS))
            .text_color(rgb(COLOR_TEXT))
            .child(toolbar)
            .child(self.project_controls.clone())
            .child(workbench_surface)
            .child(div().id("focus-body").flex().flex_1().min_h_0().w_full().gap_3()
            .child(
                div()
                    .id("focus-canvas-panel")
                    .min_h_0()
                    .overflow_scroll()
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
                    .flex_shrink_0()
                    .min_h_0()
                    .overflow_y_scroll()
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
                    .when(is_time, |element| element
                        .child(div().text_sm().child("Time · 24-hour HH:MM or HH:MM:SS"))
                        .child(Input::new(&self.time_input).id("focus-time-value"))
                        .child(Button::new("focus-time-apply").label("Apply time").disabled(self.prototype_mode)
                            .on_click(cx.listener(|this, _, _, cx| this.start_action(FocusAction::EditTime, cx))))
                        .when_some(self.inspector_feedback.clone(), |element, message| element.child(div().id("focus-time-error").role(Role::Alert).child(message))))
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
                    .child(
                        div()
                            .id("focus-token-browser")
                            .role(Role::List)
                            .aria_label("Design token browser")
                            .child(format!("Token browser · {token_count} shared token(s)"))
                            .children(token_entries.iter().map(|(token, usages)| {
                                div()
                                    .id(format!("focus-token-entry-{}", token.id))
                                    .role(Role::ListItem)
                                    .aria_label(format!("Token {}", token.name))
                                    .child(format!(
                                        "{} · shared {:?} · {} usage(s)",
                                        token.name,
                                        token.value,
                                        usages.len()
                                    ))
                                    .children(usages.iter().map(|usage| {
                                        div()
                                            .id(format!(
                                                "focus-token-usage-{}-{}",
                                                token.id, usage.property
                                            ))
                                            .text_xs()
                                            .child(format!(
                                                "Usage · {} · {} · {}",
                                                usage.owner,
                                                usage.property,
                                                usage.node_id
                                                    .as_ref()
                                                    .map_or_else(|| "global".to_owned(), ToString::to_string)
                                            ))
                                    }))
                            }))
                            .children(token_provenance.iter().map(|value| {
                                div()
                                    .id(format!("focus-token-provenance-{}", value.property))
                                    .text_xs()
                                    .child(format!(
                                        "{} · shared {:?} · local {:?} · {}",
                                        value.property,
                                        value.shared_value,
                                        value.local_value,
                                        if value.local_value.is_some() {
                                            "local override"
                                        } else {
                                            "shared provenance"
                                        }
                                    ))
                            })),
                    )
                    .child(div().text_sm().child(prototype_feedback))
                    .child(div().text_sm().child("Layout authoring"))
                    .child(action_button(
                        "focus-layout-flow",
                        "Flow",
                        FocusAction::LayoutFlow,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-layout-stack",
                        "Stack",
                        FocusAction::LayoutStack,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-layout-grid",
                        "Grid",
                        FocusAction::LayoutGrid,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-layout-absolute",
                        "Absolute",
                        FocusAction::LayoutAbsolute,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-layout-overlay",
                        "Overlay",
                        FocusAction::LayoutOverlay,
                        !can_edit,
                        cx,
                    ))
                    .child(div().text_sm().child("Token browser / inspector"))
                    .child(action_button(
                        "focus-token-create",
                        "Create token",
                        FocusAction::CreateToken,
                        false,
                        cx,
                    ))
                    .child(action_button(
                        "focus-token-apply",
                        "Apply token",
                        FocusAction::ApplyToken,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-token-override",
                        "Override token",
                        FocusAction::OverrideToken,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-token-clear",
                        "Clear override",
                        FocusAction::ClearToken,
                        !can_edit,
                        cx,
                    ))
                    .child(action_button(
                        "focus-token-edit",
                        "Edit token value",
                        FocusAction::EditToken,
                        token_count == 0,
                        cx,
                    ))
                    .child(action_button(
                        "focus-token-rename",
                        "Rename token",
                        FocusAction::RenameToken,
                        token_count == 0,
                        cx,
                    ))
                    .child(action_button(
                        "focus-token-delete",
                        "Delete token",
                        FocusAction::DeleteToken,
                        token_count == 0,
                        cx,
                    ))
                    .when(self.token_delete_confirmation, |panel| {
                        panel
                            .child(
                                div()
                                    .id("focus-token-delete-confirmation")
                                    .role(Role::Alert)
                                    .child("Confirm token deletion")
                                    .children(token_delete_usages.iter().enumerate().map(
                                        |(index, usage)| {
                                        div()
                                            .id(format!("focus-token-delete-usage-{index}"))
                                            .text_xs()
                                            .child(format!("Usage to detach · {usage}"))
                                        },
                                    )),
                            )
                            .child(action_button(
                                "focus-token-delete-confirm",
                                "Confirm delete",
                                FocusAction::ConfirmDeleteToken,
                                false,
                                cx,
                            ))
                            .child(action_button(
                                "focus-token-delete-cancel",
                                "Cancel delete",
                                FocusAction::CancelDeleteToken,
                                false,
                                cx,
                            ))
                    })
                    .child(div().text_sm().child("Token values are session-backed; edits show shared/local provenance and usage."))
                    .child(
                        div()
                            .id("focus-prototype-routes")
                            .role(Role::List)
                            .aria_label("Prototype route picker")
                            .child("Prototype routes")
                            .children(prototype_routes.iter().map(|(screen_id, name, route)| {
                                let screen_id = screen_id.clone();
                                Button::new(format!("focus-prototype-route-{screen_id}"))
                                    .label(format!("{name} · {route}"))
                                    .secondary()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_prototype_route(screen_id.clone());
                                        cx.notify();
                                    }))
                            })),
                    )
                    .child(
                        div()
                            .id("focus-prototype-graph")
                            .role(Role::List)
                            .aria_label("Prototype interaction graph")
                            .child("Interaction graph")
                            .children(prototype_interactions.iter().map(|entry| {
                                let interaction_id = entry.interaction_id.clone();
                                Button::new(format!("focus-prototype-interaction-{interaction_id}"))
                                    .label(format!(
                                        "{} · {:?} · {:?}",
                                        interaction_id, entry.event, entry.action
                                    ))
                                    .secondary()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_prototype_interaction(interaction_id.clone());
                                        cx.notify();
                                    }))
                            })),
                    )
                    .child(
                        div()
                            .id("focus-script-panel")
                            .role(Role::Pane)
                            .child("Studio Script"),
                    )
                    .child(
                        div()
                            .id("focus-script-editor")
                            .role(Role::TextInput)
                            .aria_label("Editable Studio Script buffer")
                            .h(px(220.0))
                            .child(Editor::new(&script_input)),
                    )
                    .child(
                        div()
                            .id("focus-script-feedback")
                            .text_sm()
                            .child(script_feedback),
                    )
                    .child(
                        div()
                            .id("focus-preview-bridge")
                            .role(Role::Pane)
                            .child("Studio dev preview"),
                    )
                    .child(
                        div()
                            .id("focus-preview-status")
                            .role(Role::Status)
                            .text_sm()
                            .child(preview_status_text),
                    )
                    .when(preview_screen_tabs.len() > 1, |element| {
                        let screen_ids = preview_screen_ids.clone();
                        element.child(
                            TabBar::new("focus-preview-screens")
                                .underline()
                                .menu(true)
                                .w_full()
                                .max_width(px(180.0))
                                .selected_index(preview_active_screen_index.unwrap_or(0))
                                .children(preview_screen_tabs)
                                .on_click(cx.listener(move |this, index: &usize, _, cx| {
                                    if let Some(screen_id) = screen_ids.get(*index) {
                                        this.request_preview_control(
                                            PreviewRequest::Presentation(
                                                studio_protocol::dev_rpc::DevPresentationUpdate {
                                                    screen_id: Some(screen_id.clone()),
                                                    ..Default::default()
                                                },
                                            ),
                                            cx,
                                        );
                                    }
                                })),
                        )
                    })
                    .child(
                        div()
                            .id("focus-preview-controls")
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                Button::new("focus-preview-refresh")
                                    .label("Refresh status")
                                    .secondary()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request_preview_control(PreviewRequest::Status, cx);
                                    })),
                            )
                            .child(
                                Button::new("focus-preview-selection-mode")
                                    .label(if preview_selection_mode {
                                        "Disable select mode"
                                    } else {
                                        "Enable select mode"
                                    })
                                    .disabled(!preview_enabled || preview_command_pending)
                                    .secondary()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.request_preview_control(
                                            PreviewRequest::Selection(!preview_selection_mode),
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("focus-preview-phone")
                                    .label("Phone")
                                    .disabled(!preview_enabled || preview_command_pending)
                                    .secondary()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request_preview_control(
                                            PreviewRequest::Presentation(
                                                studio_protocol::dev_rpc::DevPresentationUpdate {
                                                    viewport: Some(studio_protocol::dev_rpc::DevViewport {
                                                        width: 390,
                                                        height: 844,
                                                    }),
                                                    orientation: Some(
                                                        studio_protocol::dev_rpc::DevOrientation::Portrait,
                                                    ),
                                                    ..Default::default()
                                                },
                                            ),
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("focus-preview-tablet")
                                    .label("Tablet")
                                    .disabled(!preview_enabled || preview_command_pending)
                                    .secondary()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request_preview_control(
                                            PreviewRequest::Presentation(
                                                studio_protocol::dev_rpc::DevPresentationUpdate {
                                                    viewport: Some(studio_protocol::dev_rpc::DevViewport {
                                                        width: 768,
                                                        height: 1024,
                                                    }),
                                                    orientation: Some(
                                                        studio_protocol::dev_rpc::DevOrientation::Portrait,
                                                    ),
                                                    ..Default::default()
                                                },
                                            ),
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                Button::new("focus-preview-desktop")
                                    .label("Desktop")
                                    .disabled(!preview_enabled || preview_command_pending)
                                    .secondary()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request_preview_control(
                                            PreviewRequest::Presentation(
                                                studio_protocol::dev_rpc::DevPresentationUpdate {
                                                    viewport: Some(studio_protocol::dev_rpc::DevViewport {
                                                        width: 1440,
                                                        height: 900,
                                                    }),
                                                    orientation: Some(
                                                        studio_protocol::dev_rpc::DevOrientation::Landscape,
                                                    ),
                                                    ..Default::default()
                                                },
                                            ),
                                            cx,
                                        );
                                    })),
                            )
                            .child({
                                let orientation = match preview_presentation.orientation {
                                    studio_protocol::dev_rpc::DevOrientation::Portrait => {
                                        studio_protocol::dev_rpc::DevOrientation::Landscape
                                    }
                                    studio_protocol::dev_rpc::DevOrientation::Landscape => {
                                        studio_protocol::dev_rpc::DevOrientation::Portrait
                                    }
                                };
                                Button::new("focus-preview-rotate")
                                    .label("Rotate")
                                    .disabled(!preview_enabled || preview_command_pending)
                                    .secondary()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.request_preview_control(
                                            PreviewRequest::Presentation(
                                                studio_protocol::dev_rpc::DevPresentationUpdate {
                                                    orientation: Some(orientation),
                                                    ..Default::default()
                                                },
                                            ),
                                            cx,
                                        );
                                    }))
                            })
                            .children([750, 1_000, 1_250, 1_500].into_iter().map(|zoom_milli| {
                                let selected = preview_presentation.zoom_milli == zoom_milli;
                                let label = format!("{}%", zoom_milli / 10);
                                Button::new(format!("focus-preview-zoom-{zoom_milli}"))
                                    .label(label)
                                    .secondary()
                                    .selected(selected)
                                    .disabled(!preview_enabled || preview_command_pending)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.request_preview_control(
                                            PreviewRequest::Presentation(
                                                studio_protocol::dev_rpc::DevPresentationUpdate {
                                                    zoom_milli: Some(zoom_milli),
                                                    ..Default::default()
                                                },
                                            ),
                                            cx,
                                        );
                                    }))
                            })),
                    )
                    .children(preview_screen_source.map(|path| {
                        let source_path = path.clone();
                        Button::new("focus-preview-open-screen-source")
                            .label(format!("Open screen source · {path}"))
                            .accessibility_label(format!("Open screen source {path}"))
                            .max_w(px(420.0))
                            .secondary()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_preview_source(
                                    source_path.clone(),
                                    1,
                                    1,
                                    0,
                                    window,
                                    cx,
                                );
                                cx.notify();
                            }))
                    }))
                    .child(
                        div()
                            .id("focus-preview-selection")
                            .role(Role::Status)
                            .text_sm()
                            .child(preview_selection_label),
                    )
                    .children(preview_source_link.map(|(path, line, column, offset)| {
                        Button::new("focus-preview-open-source")
                            .label(format!("Open {path}:{line}:{column}"))
                            .secondary()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_preview_source(
                                    path.clone(),
                                    line,
                                    column,
                                    offset,
                                    window,
                                    cx,
                                );
                                cx.notify();
                            }))
                    }))
                    .child(
                        div()
                            .id("focus-preview-feedback")
                            .text_sm()
                            .child(preview_feedback),
                    )
                    .child(
                        div()
                            .id("focus-source-panel")
                            .role(Role::Pane)
                            .child("Project source files"),
                    )
                    .child(
                        div()
                            .id("focus-source-root-status")
                            .text_xs()
                            .child(self.linked_source_root.clone().unwrap_or_else(|| {
                                "Not linked to this Designer project".to_owned()
                            })),
                    )
                    .child(
                        div()
                            .id("focus-source-file-input")
                            .role(Role::TextInput)
                            .aria_label("Project-relative source file path")
                            .child(Input::new(&source_path_input).id("focus-source-path")),
                    )
                    .child(
                        Button::new("focus-source-open")
                            .label("Open file")
                            .disabled(self.linked_source_root.is_none())
                            .secondary()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_source_path(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("focus-source-file-list")
                            .role(Role::List)
                            .aria_label("Project source files")
                            .text_xs()
                            .child(format!("Accessible source files · {}", self.source_files.len()))
                            .children(self.source_files.iter().take(20).map(|file| {
                                let path = file.path.clone();
                                Button::new(format!("focus-source-file-{}", path))
                                    .label(path.clone())
                                    .secondary()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_source_file(path.clone(), window, cx);
                                        cx.notify();
                                    }))
                            })),
                    )
                    .child(div().id("focus-source-status").role(Role::Status).child(source_status))
                    .child(
                        div()
                            .id("focus-source-editor")
                            .role(Role::TextInput)
                            .aria_label("Editable project source buffer")
                            .h(px(260.0))
                            .when_some(source_input, |element, input| {
                                element.child(Editor::new(&input))
                            }),
                    )
                    .child(
                        div()
                            .id("focus-source-actions")
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                Button::new("focus-source-save")
                                    .label("Save source")
                                    .disabled(!source_is_dirty || source_has_conflict)
                                    .primary()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.save_source_file();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("focus-source-keep-local")
                                    .label("Keep local edits")
                                    .disabled(!source_has_conflict)
                                    .secondary()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.keep_local_source();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("focus-source-use-incoming")
                                    .label("Use agent version")
                                    .disabled(!source_has_conflict)
                                    .secondary()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.accept_incoming_source();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("focus-source-revert-agent")
                                    .label("Revert agent edit")
                                    .disabled(!source_can_revert)
                                    .secondary()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.revert_agent_source();
                                        cx.notify();
                                    })),
                            ),
                    )
                    .children(source_conflict_agent.map(|agent| {
                        div()
                            .id("focus-source-agent-attribution")
                            .role(Role::Status)
                            .text_sm()
                            .child(format!("Incoming edit reported by {agent}"))
                    }))
                    .children(source_diff.map(|diff| {
                        div()
                            .id("focus-source-diff")
                            .role(Role::Status)
                            .text_xs()
                            .child(diff)
                    }))
                    .child(
                        div()
                            .id("focus-source-feedback")
                            .text_sm()
                            .child(source_feedback),
                    )
                    .child(
                        div()
                            .id("focus-script-diagnostics")
                            .role(Role::Status)
                            .text_sm()
                            .child(if script_snapshot.diagnostics.is_empty() {
                                "No line-linked diagnostics".to_owned()
                            } else {
                                script_snapshot
                                    .diagnostics
                                    .iter()
                                    .map(|diagnostic| {
                                        format!(
                                            "line {}:{} · {} · {}",
                                            diagnostic.line(),
                                            diagnostic.column(),
                                            diagnostic.code,
                                            diagnostic.message
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" | ")
                            }),
                    )
                    .child(
                        div()
                            .id("focus-script-outline")
                            .role(Role::List)
                            .text_sm()
                            .child(format!(
                                "Outline {} node(s) · syntax highlights {} token(s)",
                                script_snapshot.outline.len(),
                                script_snapshot.syntax.len()
                            ))
                            .children(script_snapshot.outline.iter().map(|outline| {
                                div()
                                    .id(format!("focus-script-outline-{}", outline.id))
                                    .role(Role::ListItem)
                                    .child(format!(
                                        "{} <{}> · line {}:{}",
                                        outline.id, outline.kind, outline.line, outline.column
                                    ))
                            })),
                    )
                    .child(
                        Button::new("focus-script-check")
                            .label("Check")
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.start_script_check(cx);
                            })),
                    )
                    .child(
                        Button::new("focus-script-format")
                            .label("Format & Commit")
                            .secondary()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.start_script_format(window, cx);
                            })),
                    )
                    .child(
                        div()
                            .id("focus-script-diff")
                            .text_sm()
                            .child(if script_snapshot.dirty {
                                "Canvas ↔ text diff · pending authored changes"
                            } else {
                                "Canvas ↔ text diff · synchronized"
                            }),
                    )
                    .child(
                        div()
                            .id("focus-script-comments")
                            .text_sm()
                            .child(format!(
                                "Source comments retained · {} comment token(s)",
                                script_snapshot
                                    .syntax
                                    .iter()
                                    .filter(|token| {
                                        token.kind == studio_design::SyntaxTokenKind::Comment
                                    })
                                    .count()
                            )),
                    )
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
            ))
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

fn source_language(path: &str) -> &'static str {
    match std::path::Path::new(path)
        .extension()
        .and_then(std::ffi::OsStr::to_str)
    {
        Some("studio") => "studio",
        Some("rs") => "rust",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("json") => "json",
        Some("toml") => "toml",
        Some("md") => "markdown",
        _ => "text",
    }
}

fn source_conflict_summary(conflict: &crate::source_view::SourceConflict) -> String {
    let base = conflict.base.content.lines().collect::<Vec<_>>();
    let incoming = conflict.incoming.content.lines().collect::<Vec<_>>();
    let mut changes = Vec::new();
    for index in 0..base.len().max(incoming.len()) {
        let before = base.get(index).copied();
        let after = incoming.get(index).copied();
        if before == after {
            continue;
        }
        if let Some(before) = before {
            changes.push(format!("− {}", truncate_source_line(before)));
        }
        if let Some(after) = after {
            changes.push(format!("+ {}", truncate_source_line(after)));
        }
        if changes.len() >= 12 {
            changes.push("… diff preview truncated".to_owned());
            break;
        }
    }
    if changes.is_empty() {
        "Incoming revision differs; line preview is unavailable".to_owned()
    } else {
        changes.join("\n")
    }
}

fn truncate_source_line(line: &str) -> String {
    let mut shortened = line.chars().take(160).collect::<String>();
    if line.chars().count() > 160 {
        shortened.push('…');
    }
    shortened
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
