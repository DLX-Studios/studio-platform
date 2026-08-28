//! Session-backed Designer Focus View MVP.
//!
//! [`FocusViewModel`] is the app-side seam between a persisted
//! [`studio_design::DesignerSession`] and a native view.  It intentionally
//! stores no editable copy of the design: canvas selection and inspector edits
//! always query or mutate the session, then re-project the resulting revision.
//! GPUI is confined to the [`FocusView`] adapter below.

use std::{
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
    Actor, ActorId, ActorKind, Command, CommandBatch, CommandOutcome, DefaultDesignerSession,
    DesignerDiagnostic, DesignerPersistence, DesignerQuery, DesignerQueryResult, DesignerSession,
    HistoryOperation, LibrarySnapshot, NodeId, OperationId, ProjectionDiagnostic,
    ProjectionOptions, ProjectionReport, PropertyValue, STUDIO_DESIGN_SCHEMA_VERSION,
    SelectionSnapshot, SessionContextUpdate, SessionError, UndoGroupId,
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
    pub model: FocusViewModel<P>,
    root_focus: FocusHandle,
}

impl<P: DesignerPersistence + 'static> FocusView<P> {
    /// Create the GPUI adapter around one already-open model.
    #[must_use]
    pub fn new(model: FocusViewModel<P>, cx: &mut GpuiContext<Self>) -> Self {
        Self {
            model,
            root_focus: cx.focus_handle(),
        }
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
                let _ = this.model.select_str(&node_id);
                cx.notify();
            }))
            .into_any_element()
    }
}

impl<P: DesignerPersistence + 'static> Render for FocusView<P> {
    fn render(&mut self, _window: &mut Window, cx: &mut GpuiContext<Self>) -> impl IntoElement {
        let snapshot = self.model.snapshot();
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
        let can_edit = snapshot.selected_node_id.is_some();
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
                                if let (Ok(operation_id), Ok(undo_group_id), Ok(actor)) = (
                                    OperationId::new(format!(
                                        "focus-edit-{}",
                                        this.model.snapshot().revision_id.get() + 1
                                    )),
                                    UndoGroupId::new("focus-text-edit"),
                                    focus_actor(),
                                ) {
                                    let _ = this.model.edit_text_now(
                                        operation_id,
                                        actor,
                                        undo_group_id,
                                    );
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("focus-undo")
                            .label("Undo")
                            .secondary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let (Ok(operation_id), Ok(actor)) = (
                                    OperationId::new(format!(
                                        "focus-undo-{}",
                                        this.model.snapshot().revision_id.get() + 1
                                    )),
                                    focus_actor(),
                                ) {
                                    let _ = this.model.undo_now(operation_id, actor);
                                }
                                cx.notify();
                            })),
                    )
                    .child(div().text_sm().child(status)),
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
