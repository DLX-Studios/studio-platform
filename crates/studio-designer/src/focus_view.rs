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
    AnyElement, Context as GpuiContext, Entity, FocusHandle, IntoElement, ParentElement, Render,
    Role, Subscription, Window, div, prelude::*, px, rgb,
};
use gpui_component::{
    Disableable,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
};
use studio_design::{
    Actor, ActorId, ActorKind, CanvasPoint, CanvasSize, Command, CommandBatch, CommandOutcome,
    DefaultDesignerSession, DesignToken, DesignerDiagnostic, DesignerPersistence, DesignerQuery,
    DesignerQueryResult, DesignerSession, DeviceProfileId, DeviceProfileMatrix, EditorSnapshot,
    HierarchyEdit, HistoryOperation, InteractionId, LayoutProperties, LibrarySnapshot, NodeId,
    NodeParent, OperationId, ParentPlacement, ProjectionDiagnostic, ProjectionOptions,
    ProjectionReport, PropertyValue, ResizeHandle, STUDIO_DESIGN_SCHEMA_VERSION,
    ScriptCommitMetadata, ScriptCommitOutcome, ScriptDocumentAdapter, SelectionSnapshot,
    SessionContextUpdate, SessionError, SnapConfig, StudioDesign, TokenId, TokenKind, TokenValue,
    UndoGroupId, WORKSPACE_STATE_SCHEMA_VERSION, WorkspaceCommand, WorkspaceController,
    WorkspacePersistence, WorkspaceRecord, WorkspaceState, delete_batch, drag_batch,
    duplicate_batch, hierarchy_edit_batch, keyboard_resize_batch, nudge_batch, reparent_batch,
    restore_batch, select_variant,
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
        let session_state = match self.session.query(DesignerQuery::SessionState) {
            DesignerQueryResult::SessionState(state) => state,
            _ => unreachable!("DesignerSession returned the wrong query result"),
        };
        let mut options = self.projection_options.clone();
        if options.screen_id.is_none() {
            options.screen_id = session_state.active_screen_id;
        }
        if options.responsive_variant_id.is_none()
            && let Some(profile_id) = session_state.device_profile.as_deref()
            && let Ok(profile_id) = DeviceProfileId::new(profile_id.to_owned())
            && let Some(profile) = DeviceProfileMatrix::standard().profiles.get(&profile_id)
        {
            options.responsive_variant_id = select_variant(&snapshot.design, profile);
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
    operation_in_flight: bool,
    /// Presentation-only Focus/Workbench controller; both views use `model`.
    workspace: WorkspaceController,
    /// Durable per-project presentation preferences. This never stores a
    /// second design or session; it only records view/panel geometry.
    workspace_persistence: Arc<dyn WorkspacePersistence>,
    /// Native editable Studio Script buffer and its event subscription.
    script_input: Option<Entity<InputState>>,
    _script_subscription: Option<Subscription>,
    script_editor: Option<ScriptDocumentAdapter>,
    script_source: String,
    script_feedback: Option<String>,
    prototype_feedback: Option<String>,
    prototype: Option<studio_design::PrototypeSession>,
    prototype_mode: bool,
    prototype_screen: Option<studio_design::ScreenId>,
    prototype_interaction: Option<InteractionId>,
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
    RenameToken,
    DeleteToken,
    PrototypeRun,
    PrototypeMode,
    PrototypeRoute,
    PrototypeGraph,
}

impl<P: DesignerPersistence + 'static> FocusView<P> {
    /// Create the GPUI adapter around one already-open model.
    #[must_use]
    pub fn new(model: FocusViewModel<P>, cx: &mut GpuiContext<Self>) -> Self {
        Self::with_workspace(
            model,
            Arc::new(studio_design::InMemoryWorkspacePersistence::default()),
            cx,
        )
    }

    /// Create the native view with the host's durable workspace adapter.
    /// Presentation state is loaded before the first frame and saved after
    /// each command-bar/view change.
    pub fn new_with_workspace_persistence(
        model: FocusViewModel<P>,
        workspace_persistence: Arc<dyn WorkspacePersistence>,
        cx: &mut GpuiContext<Self>,
    ) -> Self {
        Self::with_workspace(model, workspace_persistence, cx)
    }

    fn with_workspace(
        model: FocusViewModel<P>,
        workspace_persistence: Arc<dyn WorkspacePersistence>,
        cx: &mut GpuiContext<Self>,
    ) -> Self {
        let project_id = model.session_state().project_id;
        let workspace = block_on(workspace_persistence.load(&project_id))
            .ok()
            .flatten()
            .and_then(|record| WorkspaceController::from_state(record.state).ok())
            .unwrap_or_else(|| {
                WorkspaceController::from_state(WorkspaceState::new())
                    .expect("default workspace state is valid")
            });
        Self {
            model: Some(model),
            root_focus: cx.focus_handle(),
            operation_in_flight: false,
            workspace,
            workspace_persistence,
            script_input: None,
            _script_subscription: None,
            script_editor: None,
            script_source: String::new(),
            script_feedback: None,
            prototype_feedback: None,
            prototype: None,
            prototype_mode: false,
            prototype_screen: None,
            prototype_interaction: None,
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
            state: self.workspace.state().clone(),
        };
        let _ = block_on(self.workspace_persistence.save(&record));
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
            FocusAction::LayoutFlow => ("focus-layout-flow", "focus-layout-flow"),
            FocusAction::LayoutStack => ("focus-layout-stack", "focus-layout-stack"),
            FocusAction::LayoutGrid => ("focus-layout-grid", "focus-layout-grid"),
            FocusAction::LayoutAbsolute => ("focus-layout-absolute", "focus-layout-absolute"),
            FocusAction::LayoutOverlay => ("focus-layout-overlay", "focus-layout-overlay"),
            FocusAction::CreateToken => ("focus-token-create", "focus-token-create"),
            FocusAction::ApplyToken => ("focus-token-apply", "focus-token-apply"),
            FocusAction::OverrideToken => ("focus-token-override", "focus-token-override"),
            FocusAction::ClearToken => ("focus-token-clear", "focus-token-clear"),
            FocusAction::RenameToken => ("focus-token-rename", "focus-token-rename"),
            FocusAction::DeleteToken => ("focus-token-delete", "focus-token-delete"),
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
                FocusAction::RenameToken => {
                    model
                        .rename_focus_token(operation_id, actor, undo_group_id)
                        .await
                }
                FocusAction::DeleteToken => {
                    model
                        .delete_focus_token(operation_id, actor, undo_group_id)
                        .await
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
    fn render(&mut self, window: &mut Window, cx: &mut GpuiContext<Self>) -> impl IntoElement {
        if self.model.is_none() {
            return div()
                .id("studio-focus-loading")
                .role(Role::Status)
                .aria_label("Designer operation in progress")
                .p_4()
                .child("Saving Designer changes…");
        }
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
                InputState::new(window, cx)
                    .default_value(source.clone())
                    .code_editor("studio")
            });
            let subscription = cx.subscribe(&input, |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.script_source = input.read(cx).value().to_string();
                    if let Some(editor) = this.script_editor.as_mut() {
                        editor.replace_source(this.script_source.clone());
                    }
                    this.script_feedback =
                        Some("Unsaved script buffer · press Check & Commit".to_owned());
                    cx.notify();
                }
            });
            self.script_source = source;
            self.script_input = Some(input);
            self._script_subscription = Some(subscription);
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
        let prototype_feedback = self
            .prototype_feedback
            .clone()
            .unwrap_or_else(|| "Prototype preview is isolated from source history".to_owned());
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
            .child(toolbar)
            .child(workbench_surface)
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
                            .child(Input::new(&script_input)),
                    )
                    .child(
                        div()
                            .id("focus-script-feedback")
                            .text_sm()
                            .child(script_feedback),
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
