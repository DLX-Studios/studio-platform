//! Presentation state for the Focus and Workbench editor views.
//!
//! Workspace state intentionally contains only presentation concerns. The
//! active project, revision, selection, screen, profile, transform, tool,
//! history, runs, and unsaved work remain owned by [`crate::DesignerSession`]
//! and are read through [`crate::SessionStateSnapshot`]. This keeps switching
//! views from copying or forking authoring state.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{DesignerQuery, DesignerQueryResult, DesignerSession, ProjectId, SessionStateSnapshot};

/// Version for persisted workspace presentation records.
pub const WORKSPACE_STATE_SCHEMA_VERSION: u16 = 1;

/// The two primary editor presentations.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum EditorView {
    /// Canvas-first presentation with collapsible contextual panels.
    #[default]
    Focus,
    /// Persistent multi-panel presentation for deep inspection.
    Workbench,
}

/// Stable identifiers for surfaces that can be shown in either view.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelId {
    /// Screen list and hierarchy tree.
    ScreensHierarchy,
    /// Studio Library entry point.
    Library,
    /// Canvas controls and device/profile controls.
    CanvasControls,
    /// Selected-node inspector.
    Inspector,
    /// Authoring diagnostics and problems.
    Diagnostics,
    /// Prototype interactions and event graph.
    Interactions,
    /// Live agent operation activity.
    AgentActivity,
    /// Revision and named-history controls.
    History,
}

impl PanelId {
    /// Return every Workbench surface in stable presentation order.
    #[must_use]
    pub const fn all() -> [Self; 8] {
        [
            Self::ScreensHierarchy,
            Self::Library,
            Self::CanvasControls,
            Self::Inspector,
            Self::Diagnostics,
            Self::Interactions,
            Self::AgentActivity,
            Self::History,
        ]
    }
}

/// Integer canvas rectangle suitable for deterministic persistence.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PanelGeometry {
    /// Horizontal position in workspace pixels.
    pub x: i32,
    /// Vertical position in workspace pixels.
    pub y: i32,
    /// Width in workspace pixels.
    pub width: u32,
    /// Height in workspace pixels.
    pub height: u32,
}

impl PanelGeometry {
    /// Construct a panel rectangle.
    #[must_use]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Presentation flags and geometry for one panel.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PanelState {
    /// Whether the panel is rendered.
    pub visible: bool,
    /// Whether the panel is collapsed while remaining reachable.
    pub collapsed: bool,
    /// Last user-selected position and size.
    pub geometry: PanelGeometry,
}

/// Panel arrangement for one editor view.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PanelArrangement {
    panels: BTreeMap<PanelId, PanelState>,
}

impl PanelArrangement {
    /// Build the default arrangement for a view.
    #[must_use]
    pub fn default_for(view: EditorView) -> Self {
        let mut panels = BTreeMap::new();
        for (index, panel) in PanelId::all().into_iter().enumerate() {
            let visible = match view {
                EditorView::Focus => matches!(
                    panel,
                    PanelId::CanvasControls
                        | PanelId::Inspector
                        | PanelId::AgentActivity
                        | PanelId::History
                ),
                EditorView::Workbench => true,
            };
            let geometry = default_geometry(view, panel, index);
            panels.insert(
                panel,
                PanelState {
                    visible,
                    collapsed: false,
                    geometry,
                },
            );
        }
        Self { panels }
    }

    /// Get one panel's state.
    #[must_use]
    pub fn panel(&self, panel: PanelId) -> PanelState {
        self.panels.get(&panel).copied().unwrap_or(PanelState {
            visible: false,
            collapsed: false,
            geometry: PanelGeometry::new(0, 0, 320, 240),
        })
    }

    /// Return all persisted panel records.
    #[must_use]
    pub fn panels(&self) -> &BTreeMap<PanelId, PanelState> {
        &self.panels
    }

    /// Set visibility without changing geometry.
    pub fn set_visible(&mut self, panel: PanelId, visible: bool) {
        self.panels
            .entry(panel)
            .or_insert_with(|| PanelState {
                visible: false,
                collapsed: false,
                geometry: PanelGeometry::new(0, 0, 320, 240),
            })
            .visible = visible;
    }

    /// Set collapse state without changing geometry.
    pub fn set_collapsed(&mut self, panel: PanelId, collapsed: bool) {
        self.panels
            .entry(panel)
            .or_insert_with(|| PanelState {
                visible: false,
                collapsed: false,
                geometry: PanelGeometry::new(0, 0, 320, 240),
            })
            .collapsed = collapsed;
    }

    /// Persist a panel's latest geometry.
    pub fn set_geometry(&mut self, panel: PanelId, geometry: PanelGeometry) {
        self.panels
            .entry(panel)
            .or_insert_with(|| PanelState {
                visible: false,
                collapsed: false,
                geometry,
            })
            .geometry = geometry;
    }
}

/// Presentation-only state shared by Focus and Workbench.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceState {
    /// Current presentation; changing this never changes Designer data.
    pub active_view: EditorView,
    /// Focus-specific panel arrangement.
    pub focus: PanelArrangement,
    /// Workbench-specific panel arrangement.
    pub workbench: PanelArrangement,
}

impl Default for WorkspaceState {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceState {
    /// Construct a new workspace with independent defaults for both views.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_view: EditorView::Focus,
            focus: PanelArrangement::default_for(EditorView::Focus),
            workbench: PanelArrangement::default_for(EditorView::Workbench),
        }
    }

    /// Switch presentation and return the selected view.
    pub fn switch_to(&mut self, view: EditorView) -> EditorView {
        self.active_view = view;
        view
    }

    /// Toggle between Focus and Workbench.
    pub fn toggle_view(&mut self) -> EditorView {
        self.switch_to(match self.active_view {
            EditorView::Focus => EditorView::Workbench,
            EditorView::Workbench => EditorView::Focus,
        })
    }

    /// Return the arrangement for the active view.
    #[must_use]
    pub fn active_arrangement(&self) -> &PanelArrangement {
        match self.active_view {
            EditorView::Focus => &self.focus,
            EditorView::Workbench => &self.workbench,
        }
    }

    /// Return the mutable arrangement for the active view.
    #[must_use]
    pub fn active_arrangement_mut(&mut self) -> &mut PanelArrangement {
        match self.active_view {
            EditorView::Focus => &mut self.focus,
            EditorView::Workbench => &mut self.workbench,
        }
    }

    /// Validate a decoded state before applying it.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::InvalidState`] if a decoded record contains
    /// a panel identifier outside the closed panel vocabulary.
    pub fn validate(&self) -> Result<(), WorkspaceError> {
        if !self
            .focus
            .panels
            .keys()
            .all(|panel| PanelId::all().contains(panel))
            || !self
                .workbench
                .panels
                .keys()
                .all(|panel| PanelId::all().contains(panel))
        {
            return Err(WorkspaceError::InvalidState(
                "workspace contains an unknown panel".to_owned(),
            ));
        }
        Ok(())
    }
}

/// A view switch result containing the current authoritative session state.
///
/// `session` is an immutable read snapshot, not another mutable copy of
/// session state. Callers should query the session again after mutations.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewSwitchSnapshot {
    /// Presentation selected after the switch.
    pub view: EditorView,
    /// State read from the one shared `DesignerSession`.
    pub session: SessionStateSnapshot,
}

/// Controller that composes a presentation state with a shared session.
#[derive(Clone, Debug, Default)]
pub struct WorkspaceController {
    state: WorkspaceState,
}

impl WorkspaceController {
    /// Construct a controller from persisted presentation state.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::InvalidState`] when the supplied state is not
    /// valid for the current closed panel vocabulary.
    pub fn from_state(state: WorkspaceState) -> Result<Self, WorkspaceError> {
        state.validate()?;
        Ok(Self { state })
    }

    /// Borrow the presentation state.
    #[must_use]
    pub const fn state(&self) -> &WorkspaceState {
        &self.state
    }

    /// Borrow mutable presentation state for panel operations.
    #[must_use]
    pub const fn state_mut(&mut self) -> &mut WorkspaceState {
        &mut self.state
    }

    /// Switch view while reading shared state from the supplied session.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::SessionQuery`] if the session does not return
    /// a [`DesignerQueryResult::SessionState`] response.
    pub fn switch_to<S: DesignerSession>(
        &mut self,
        view: EditorView,
        session: &S,
    ) -> Result<ViewSwitchSnapshot, WorkspaceError> {
        self.state.switch_to(view);
        let DesignerQueryResult::SessionState(session) = session.query(DesignerQuery::SessionState)
        else {
            return Err(WorkspaceError::SessionQuery);
        };
        Ok(ViewSwitchSnapshot { view, session })
    }

    /// Toggle view while reading shared state from the supplied session.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::SessionQuery`] if the session does not return
    /// a [`DesignerQueryResult::SessionState`] response.
    pub fn toggle<S: DesignerSession>(
        &mut self,
        session: &S,
    ) -> Result<ViewSwitchSnapshot, WorkspaceError> {
        let view = self.state.toggle_view();
        self.switch_to(view, session)
    }

    /// Execute a command-bar or keyboard command against this presentation.
    ///
    /// Panel commands only update the active view's arrangement and then read
    /// the current state from the shared session. They never mutate design
    /// source or copy session state into the workspace controller.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::SessionQuery`] if the session does not return
    /// a [`DesignerQueryResult::SessionState`] response.
    pub fn execute<S: DesignerSession>(
        &mut self,
        command: WorkspaceCommand,
        session: &S,
    ) -> Result<ViewSwitchSnapshot, WorkspaceError> {
        if command == WorkspaceCommand::ToggleView {
            return self.toggle(session);
        }
        if let Some(panel) = command.panel() {
            self.state.active_arrangement_mut().set_visible(panel, true);
        }
        self.switch_to(self.state.active_view, session)
    }
}

/// Stable command-bar and keyboard-reachable editor commands.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceCommand {
    /// Switch or toggle the primary editor presentation.
    ToggleView,
    /// Open the screens and hierarchy surface.
    OpenScreensHierarchy,
    /// Open the Studio Library.
    OpenLibrary,
    /// Open canvas and profile controls.
    OpenCanvasControls,
    /// Open the inspector.
    OpenInspector,
    /// Open diagnostics.
    OpenDiagnostics,
    /// Open interactions.
    OpenInteractions,
    /// Open agent activity.
    OpenAgentActivity,
    /// Open revision history.
    OpenHistory,
}

impl WorkspaceCommand {
    /// Return the panel opened by a panel command.
    #[must_use]
    pub const fn panel(self) -> Option<PanelId> {
        match self {
            Self::OpenScreensHierarchy => Some(PanelId::ScreensHierarchy),
            Self::OpenLibrary => Some(PanelId::Library),
            Self::OpenCanvasControls => Some(PanelId::CanvasControls),
            Self::OpenInspector => Some(PanelId::Inspector),
            Self::OpenDiagnostics => Some(PanelId::Diagnostics),
            Self::OpenInteractions => Some(PanelId::Interactions),
            Self::OpenAgentActivity => Some(PanelId::AgentActivity),
            Self::OpenHistory => Some(PanelId::History),
            Self::ToggleView => None,
        }
    }
}

/// A command-bar entry with keyboard reachability metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CommandDescriptor {
    /// Stable command identity.
    pub command: WorkspaceCommand,
    /// Human-readable command-bar label.
    pub label: &'static str,
    /// Keyboard accelerator rendered in the command bar.
    pub shortcut: &'static str,
}

/// Return the complete command registry exposed in both views.
#[must_use]
pub const fn command_registry() -> [CommandDescriptor; 9] {
    [
        CommandDescriptor {
            command: WorkspaceCommand::ToggleView,
            label: "Switch Focus / Workbench",
            shortcut: "Ctrl+Shift+W",
        },
        CommandDescriptor {
            command: WorkspaceCommand::OpenScreensHierarchy,
            label: "Open Screens & Hierarchy",
            shortcut: "Ctrl+1",
        },
        CommandDescriptor {
            command: WorkspaceCommand::OpenLibrary,
            label: "Open Library",
            shortcut: "Ctrl+2",
        },
        CommandDescriptor {
            command: WorkspaceCommand::OpenCanvasControls,
            label: "Open Canvas Controls",
            shortcut: "Ctrl+3",
        },
        CommandDescriptor {
            command: WorkspaceCommand::OpenInspector,
            label: "Open Inspector",
            shortcut: "Ctrl+4",
        },
        CommandDescriptor {
            command: WorkspaceCommand::OpenDiagnostics,
            label: "Open Diagnostics",
            shortcut: "Ctrl+5",
        },
        CommandDescriptor {
            command: WorkspaceCommand::OpenInteractions,
            label: "Open Interactions",
            shortcut: "Ctrl+6",
        },
        CommandDescriptor {
            command: WorkspaceCommand::OpenAgentActivity,
            label: "Open Agent Activity",
            shortcut: "Ctrl+7",
        },
        CommandDescriptor {
            command: WorkspaceCommand::OpenHistory,
            label: "Open History",
            shortcut: "Ctrl+8",
        },
    ]
}

/// Find commands from command-bar text or a keyboard shortcut.
#[must_use]
pub fn find_commands(query: &str) -> Vec<CommandDescriptor> {
    let query = query.trim().to_ascii_lowercase();
    command_registry()
        .into_iter()
        .filter(|descriptor| {
            query.is_empty()
                || descriptor.label.to_ascii_lowercase().contains(&query)
                || descriptor.shortcut.to_ascii_lowercase() == query
        })
        .collect()
}

/// A workspace state persistence failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WorkspaceError {
    /// The persisted workspace record was invalid.
    #[error("invalid workspace state: {0}")]
    InvalidState(String),
    /// The supplied session did not return its required state query.
    #[error("DesignerSession did not return SessionState for the workspace query")]
    SessionQuery,
    /// The workspace persistence adapter failed.
    #[error("workspace persistence failed: {0}")]
    Persistence(String),
}

/// Durable presentation record keyed by project identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRecord {
    /// Workspace record schema version.
    pub schema_version: u16,
    /// Project this presentation belongs to.
    pub project_id: ProjectId,
    /// Persisted presentation and per-view arrangements.
    pub state: WorkspaceState,
}

/// Minimal persistence seam for per-project presentation preferences.
pub trait WorkspacePersistence: Send + Sync {
    /// Load a project's latest workspace presentation.
    fn load<'a>(
        &'a self,
        project_id: &'a ProjectId,
    ) -> crate::SessionFuture<'a, Result<Option<WorkspaceRecord>, WorkspaceError>>;

    /// Atomically save a project's workspace presentation.
    fn save<'a>(
        &'a self,
        record: &'a WorkspaceRecord,
    ) -> crate::SessionFuture<'a, Result<(), WorkspaceError>>;
}

/// Deterministic process-local persistence useful to native adapters and tests.
#[derive(Clone, Default)]
pub struct InMemoryWorkspacePersistence {
    records: std::sync::Arc<std::sync::Mutex<BTreeMap<ProjectId, WorkspaceRecord>>>,
}

impl InMemoryWorkspacePersistence {
    /// Inspect the last saved record for a project.
    #[must_use]
    pub fn record(&self, project_id: &ProjectId) -> Option<WorkspaceRecord> {
        self.records
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(project_id)
            .cloned()
    }
}

impl WorkspacePersistence for InMemoryWorkspacePersistence {
    fn load<'a>(
        &'a self,
        project_id: &'a ProjectId,
    ) -> crate::SessionFuture<'a, Result<Option<WorkspaceRecord>, WorkspaceError>> {
        Box::pin(async move {
            Ok(self
                .records
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(project_id)
                .cloned())
        })
    }

    fn save<'a>(
        &'a self,
        record: &'a WorkspaceRecord,
    ) -> crate::SessionFuture<'a, Result<(), WorkspaceError>> {
        Box::pin(async move {
            if record.schema_version != WORKSPACE_STATE_SCHEMA_VERSION {
                return Err(WorkspaceError::InvalidState(
                    "unsupported workspace schema version".to_owned(),
                ));
            }
            record.state.validate()?;
            self.records
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(record.project_id.clone(), record.clone());
            Ok(())
        })
    }
}

fn default_geometry(view: EditorView, panel: PanelId, index: usize) -> PanelGeometry {
    match (view, panel) {
        (EditorView::Focus, PanelId::Inspector) => PanelGeometry::new(960, 48, 320, 620),
        (EditorView::Focus, PanelId::CanvasControls) => PanelGeometry::new(360, 48, 320, 48),
        (EditorView::Focus, PanelId::AgentActivity) => PanelGeometry::new(360, 700, 560, 180),
        (EditorView::Focus, PanelId::History) => PanelGeometry::new(0, 700, 350, 180),
        (EditorView::Workbench, PanelId::ScreensHierarchy) => PanelGeometry::new(0, 0, 280, 760),
        (EditorView::Workbench, PanelId::Library) => PanelGeometry::new(0, 520, 280, 240),
        (EditorView::Workbench, PanelId::Inspector) => PanelGeometry::new(1040, 0, 320, 760),
        (EditorView::Workbench, PanelId::Diagnostics) => PanelGeometry::new(280, 520, 380, 240),
        (EditorView::Workbench, PanelId::Interactions) => PanelGeometry::new(660, 520, 380, 240),
        (EditorView::Workbench, PanelId::AgentActivity) => PanelGeometry::new(1040, 520, 320, 240),
        (EditorView::Workbench, PanelId::History) => PanelGeometry::new(280, 760, 760, 160),
        (EditorView::Workbench, PanelId::CanvasControls) => PanelGeometry::new(280, 0, 760, 48),
        (_, _) => {
            let offset = i32::try_from(index).unwrap_or(i32::MAX).saturating_mul(16);
            PanelGeometry::new(offset, offset, 320, 240)
        }
    }
}
