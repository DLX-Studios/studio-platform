//! Host-independent state and behavior for Studio's authenticated project dashboard.
//!
//! The dashboard deliberately owns one catalog query for Grid, Index, and Activity.  A native
//! shell can render the returned snapshot in any of those modes without copying query state or
//! reaching into identity, storage, or Designer session implementations.

#![allow(missing_docs)]
#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::assigning_clones,
    clippy::if_not_else
)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Maximum length accepted for catalog identifiers and display metadata.
const MAX_TEXT_LENGTH: usize = 256;

/// Identity/session seam supplied by the authenticated application shell.
pub trait DashboardIdentity {
    /// Stable opaque identity key used to partition persisted dashboard state.
    fn identity_key(&self) -> &str;
    /// Whether the identity has completed authentication.
    fn is_authenticated(&self) -> bool;
}

/// Persistence seam for a dashboard's catalog and query preferences.
pub trait DashboardPersistence: Send + Sync {
    /// Load state for one authenticated identity.
    fn load(&self, identity_key: &str)
    -> Result<Option<DashboardState>, DashboardPersistenceError>;
    /// Atomically persist state for one authenticated identity.
    fn save(
        &self,
        identity_key: &str,
        state: &DashboardState,
    ) -> Result<(), DashboardPersistenceError>;
}

/// Deterministic persistence adapter useful for the native host and tests.
#[derive(Clone, Default)]
pub struct InMemoryDashboardPersistence {
    states: std::sync::Arc<std::sync::Mutex<BTreeMap<String, DashboardState>>>,
}

impl InMemoryDashboardPersistence {
    /// Inspect the last state saved for an identity.
    #[must_use]
    pub fn state(&self, identity_key: &str) -> Option<DashboardState> {
        self.states
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(identity_key)
            .cloned()
    }
}

impl DashboardPersistence for InMemoryDashboardPersistence {
    fn load(
        &self,
        identity_key: &str,
    ) -> Result<Option<DashboardState>, DashboardPersistenceError> {
        Ok(self.state(identity_key))
    }

    fn save(
        &self,
        identity_key: &str,
        state: &DashboardState,
    ) -> Result<(), DashboardPersistenceError> {
        self.states
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(identity_key.to_owned(), state.clone());
        Ok(())
    }
}

/// Sanitized persistence failure returned by a host adapter.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Error)]
#[error("{message}")]
#[serde(deny_unknown_fields)]
pub struct DashboardPersistenceError {
    /// Stable failure category.
    pub code: DashboardPersistenceErrorCode,
    /// User-facing recovery guidance, without backend details.
    pub message: String,
}

/// Categories of dashboard persistence failures.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardPersistenceErrorCode {
    /// The selected store is unavailable.
    Unavailable,
    /// The persisted state failed validation.
    Corrupt,
    /// The persisted state requires a newer application.
    Incompatible,
    /// The adapter rejected the write.
    Rejected,
}

/// Grid, Index, and Activity are presentations over one query.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DashboardMode {
    /// Visual project cards.
    #[default]
    Grid,
    /// Compact sortable project list.
    Index,
    /// Projects with safe recent activity.
    Activity,
}

/// Catalog sort choices.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogSort {
    /// Most recently active projects first.
    #[default]
    Activity,
    /// Name in case-insensitive lexical order.
    Name,
    /// Newest projects first.
    Created,
    /// Most recently updated projects first.
    Updated,
    /// Stable lifecycle/sync state order.
    State,
}

/// Local/cloud and lifecycle filters supported by the dashboard.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogFilter {
    /// Projects whose authority is local.
    Local,
    /// Projects whose authority is cloud.
    Cloud,
    /// Projects currently synchronizing.
    Syncing,
    /// Projects with a sync conflict.
    Conflicted,
    /// Projects recovered from a failed open or quarantined for repair.
    Recovered,
    /// Archived projects.
    Archived,
}

/// Storage authority for one project.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectAuthority {
    /// Device-owned project that works fully offline.
    Local,
    /// Cloud-owned project with an offline cache.
    Cloud,
}

/// Synchronization state visible in catalog metadata.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    /// No sync work is pending.
    #[default]
    Local,
    /// Cloud data is current.
    Synced,
    /// Changes are being uploaded or downloaded.
    Syncing,
    /// Local and cloud revisions need user resolution.
    Conflicted,
    /// A cloud project is cached but cannot currently reach the service.
    Offline,
}

/// Project metadata safe to use in a catalog before opening its design session.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectRecord {
    /// Stable project key.
    pub id: String,
    /// User-provided display name.
    pub name: String,
    /// Optional user-provided description.
    pub description: Option<String>,
    /// Local or cloud ownership.
    pub authority: ProjectAuthority,
    /// Current synchronization state.
    pub sync_state: SyncState,
    /// Whether the project is hidden from the default active catalog.
    pub archived: bool,
    /// Whether the project needs recovery/quarantine attention.
    pub recovered: bool,
    /// Stable logical timestamps (host-defined epoch units).
    pub created_at: u64,
    /// Last metadata/design update timestamp.
    pub updated_at: u64,
    /// Last safe project activity timestamp.
    pub last_activity_at: u64,
    /// Admitted, non-secret metadata used by catalog search.
    pub admitted_metadata: Vec<String>,
    /// Number of project-owned assets affected by destructive deletion.
    pub asset_count: u32,
    /// Whether a logical backup exists for this project.
    pub has_backup: bool,
    /// Whether unsynchronized changes would be discarded by deletion.
    pub has_unsynced_changes: bool,
}

impl ProjectRecord {
    /// Construct a new project with safe defaults.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        authority: ProjectAuthority,
        created_at: u64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            authority,
            sync_state: match authority {
                ProjectAuthority::Local => SyncState::Local,
                ProjectAuthority::Cloud => SyncState::Synced,
            },
            archived: false,
            recovered: false,
            created_at,
            updated_at: created_at,
            last_activity_at: created_at,
            admitted_metadata: Vec::new(),
            asset_count: 0,
            has_backup: false,
            has_unsynced_changes: false,
        }
    }

    fn validate(&self) -> Result<(), DashboardError> {
        validate_text(&self.id, "project id")?;
        validate_text(&self.name, "project name")?;
        if let Some(description) = &self.description {
            validate_text(description, "project description")?;
        }
        if self
            .admitted_metadata
            .iter()
            .any(|value| validate_text(value, "admitted metadata").is_err())
        {
            return Err(DashboardError::InvalidMetadata);
        }
        Ok(())
    }

    fn search_text(&self) -> String {
        let mut text = normalize(&self.name);
        for value in &self.admitted_metadata {
            text.push(' ');
            text.push_str(&normalize(value));
        }
        text
    }
}

/// One safe activity category. It intentionally has no content/body field.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SafeActivityKind {
    /// Project metadata changed.
    ProjectUpdated,
    /// A project was opened.
    ProjectOpened,
    /// A build completed or failed.
    Build,
    /// Synchronization metadata changed.
    Sync,
    /// Recovery or quarantine metadata changed.
    Recovery,
    /// A safe agent-history event occurred.
    Agent,
}

impl SafeActivityKind {
    /// Stable, non-content label suitable for an activity row.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProjectUpdated => "Project updated",
            Self::ProjectOpened => "Project opened",
            Self::Build => "Build activity",
            Self::Sync => "Sync activity",
            Self::Recovery => "Recovery activity",
            Self::Agent => "Agent activity",
        }
    }
}

/// Safe activity metadata. Protected design/content values cannot be represented here.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SafeActivity {
    /// Related project key.
    pub project_id: String,
    /// Safe event category.
    pub kind: SafeActivityKind,
    /// Host-defined epoch timestamp.
    pub occurred_at: u64,
}

impl SafeActivity {
    /// Construct a safe activity entry after validating only its opaque project key.
    pub fn new(
        project_id: impl Into<String>,
        kind: SafeActivityKind,
        occurred_at: u64,
    ) -> Result<Self, DashboardError> {
        let project_id = project_id.into();
        validate_text(&project_id, "activity project id")?;
        Ok(Self {
            project_id,
            kind,
            occurred_at,
        })
    }
}

/// The one persisted query shared by all dashboard modes.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardQuery {
    /// Current presentation mode.
    pub mode: DashboardMode,
    /// Search text over names and admitted metadata.
    pub search: String,
    /// Active catalog filters.
    pub filters: BTreeSet<CatalogFilter>,
    /// Current catalog ordering.
    pub sort: CatalogSort,
    /// Selected project, retained while switching modes and query controls.
    pub selection: Option<String>,
}

/// Durable catalog and query state for one identity.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardState {
    /// Catalog records visible after authentication.
    pub projects: BTreeMap<String, ProjectRecord>,
    /// Safe recent activity metadata.
    pub activity: Vec<SafeActivity>,
    /// The one persisted query.
    pub query: DashboardQuery,
    /// Last project the user asked to open.
    pub last_project_id: Option<String>,
}

/// Materialized dashboard view for a renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DashboardSnapshot {
    /// Current mode.
    pub mode: DashboardMode,
    /// Query controls preserved across modes.
    pub query: DashboardQuery,
    /// Projects matching the query, in deterministic order.
    pub projects: Vec<ProjectRecord>,
    /// Matching safe activity, newest first.
    pub activity: Vec<SafeActivityView>,
    /// Whether the filtered result is empty.
    pub is_empty: bool,
}

/// Safe activity joined to a project display name after authentication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeActivityView {
    /// Related project key.
    pub project_id: String,
    /// Related project name.
    pub project_name: String,
    /// Safe category label.
    pub label: &'static str,
    /// Host-defined epoch timestamp.
    pub occurred_at: u64,
}

/// Consequences shown before a destructive delete confirmation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteConsequences {
    /// Whether the local project data will be removed.
    pub local_project_data: bool,
    /// Whether the cloud copy will be deleted.
    pub cloud_project_copy: bool,
    /// Number of project assets removed.
    pub asset_count: u32,
    /// Whether a logical backup is removed.
    pub backup_removed: bool,
    /// Whether unsynchronized changes are lost.
    pub unsynced_changes_lost: bool,
}

impl DeleteConsequences {
    /// Stable, complete consequence lines for a confirmation surface.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines: Vec<String> = vec![
            if self.local_project_data {
                "Local project data will be removed."
            } else {
                "No local project data will be removed."
            }
            .to_owned(),
            if self.cloud_project_copy {
                "The cloud project copy will be deleted."
            } else {
                "No cloud project copy will be deleted."
            }
            .to_owned(),
        ];
        lines.push(format!(
            "{} project asset(s) will be removed.",
            self.asset_count
        ));
        lines.push(
            if self.backup_removed {
                "The logical backup will be removed."
            } else {
                "The logical backup will be retained."
            }
            .to_owned(),
        );
        lines.push(
            if self.unsynced_changes_lost {
                "Unsynchronized changes will be lost."
            } else {
                "There are no unsynchronized changes to lose."
            }
            .to_owned(),
        );
        lines
    }
}

/// Explicit confirmation token for a destructive delete.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteConfirmation {
    project_id: String,
    consequences: DeleteConsequences,
}

impl DeleteConfirmation {
    /// Confirm the exact consequence preview returned by [`ProjectDashboard::delete_preview`].
    #[must_use]
    pub fn acknowledge(project_id: impl Into<String>, consequences: DeleteConsequences) -> Self {
        Self {
            project_id: project_id.into(),
            consequences,
        }
    }
}

/// Project opening seam supplied by the Designer session host.
pub trait ProjectSessionAdapter {
    /// Open one project, returning only a sanitized category/message.
    fn open_project(&mut self, project_id: &str) -> Result<(), SessionOpenError>;
}

/// Sanitized project-open failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionOpenError {
    /// Stable failure category.
    pub code: SessionOpenErrorCode,
    /// User-facing recovery guidance.
    pub message: &'static str,
}

/// Project-open failure categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionOpenErrorCode {
    /// No durable project state exists.
    Missing,
    /// Durable state failed validation.
    Corrupt,
    /// The project cannot be opened in this application version.
    Incompatible,
    /// The identity is not allowed to open the project.
    Unauthorized,
    /// The project is temporarily unavailable.
    Unavailable,
}

/// Recovery diagnostic returned to the dashboard after a resume/open failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryDiagnostic {
    /// Related project key when known.
    pub project_id: Option<String>,
    /// Stable failure category.
    pub code: SessionOpenErrorCode,
    /// Safe recovery guidance.
    pub message: &'static str,
}

/// Result of trying to open a selected or remembered project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResumeOutcome {
    /// The project session opened.
    Opened(String),
    /// The dashboard remains visible with an actionable diagnostic.
    ReturnedToDashboard(RecoveryDiagnostic),
    /// No remembered project exists.
    NoRememberedProject,
}

/// Dashboard keyboard focus target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DashboardFocus {
    /// Search field.
    Search,
    /// Filter control.
    Filters,
    /// Sort control.
    Sort,
    /// Mode control.
    Mode,
    /// Create action.
    Create,
    /// Import action.
    Import,
    /// Templates action.
    Templates,
    /// One visible project in the current mode.
    Project(usize),
}

/// Keyboard input normalized by a native UI adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DashboardKey {
    /// Move to next focus target.
    Tab,
    /// Move to previous focus target.
    ShiftTab,
    /// Move among projects or controls.
    Up,
    /// Move among projects or controls.
    Down,
    /// Move among projects or controls.
    Left,
    /// Move among projects or controls.
    Right,
    /// First focus target.
    Home,
    /// Last focus target.
    End,
    /// Activate the focused target.
    Enter,
    /// Close a menu/clear transient focus.
    Escape,
}

/// Result of handling one keyboard input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeyboardAction {
    /// Focus moved to a target.
    Focus(DashboardFocus),
    /// The selected project should open.
    OpenProject(String),
    /// Show project creation.
    CreateProject,
    /// Show bounded import review.
    Import,
    /// Show templates.
    Templates,
    /// Clear the search field.
    ClearSearch,
    /// Input was handled without an external action.
    Handled,
}

/// Dashboard domain errors.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum DashboardError {
    /// The identity must be authenticated before catalog state is exposed.
    #[error("dashboard requires an authenticated identity")]
    Unauthenticated,
    /// A project or metadata value violates the bounded text contract.
    #[error("invalid dashboard metadata")]
    InvalidMetadata,
    /// A requested project does not exist.
    #[error("project not found: {0}")]
    ProjectNotFound(String),
    /// A project identifier already exists.
    #[error("project already exists: {0}")]
    ProjectAlreadyExists(String),
    /// A duplicate must use a distinct identity.
    #[error("duplicate project id must be distinct")]
    DuplicateIdentity,
    /// Delete requires acknowledgement of the exact preview.
    #[error("delete requires confirmation")]
    DeleteConfirmationRequired(DeleteConsequences),
    /// The supplied confirmation does not match the current record.
    #[error("delete confirmation is stale")]
    StaleDeleteConfirmation,
    /// The persistence adapter failed.
    #[error(transparent)]
    Persistence(#[from] DashboardPersistenceError),
}

/// Authenticated project dashboard state machine.
pub struct ProjectDashboard<P> {
    identity_key: String,
    persistence: P,
    state: DashboardState,
    focus: DashboardFocus,
}

impl<P: DashboardPersistence> ProjectDashboard<P> {
    /// Load or create dashboard state through an identity and storage adapter.
    pub fn open<I: DashboardIdentity>(
        identity: &I,
        persistence: P,
    ) -> Result<Self, DashboardError> {
        if !identity.is_authenticated() {
            return Err(DashboardError::Unauthenticated);
        }
        let identity_key = identity.identity_key().to_owned();
        validate_text(&identity_key, "identity key")?;
        let state = persistence.load(&identity_key)?.unwrap_or_default();
        validate_state(&state)?;
        let mut dashboard = Self {
            identity_key,
            persistence,
            state,
            focus: DashboardFocus::Search,
        };
        dashboard.restore_focus();
        Ok(dashboard)
    }

    /// Construct a dashboard with an explicit identity key, useful for adapters without a
    /// concrete identity type. The key is still treated as authenticated by the caller.
    pub fn for_authenticated_identity(
        identity_key: impl Into<String>,
        persistence: P,
    ) -> Result<Self, DashboardError> {
        let identity_key = identity_key.into();
        validate_text(&identity_key, "identity key")?;
        let state = persistence.load(&identity_key)?.unwrap_or_default();
        validate_state(&state)?;
        let mut dashboard = Self {
            identity_key,
            persistence,
            state,
            focus: DashboardFocus::Search,
        };
        dashboard.restore_focus();
        Ok(dashboard)
    }

    /// Persisted state, for host adapters and deterministic tests.
    #[must_use]
    pub fn state(&self) -> &DashboardState {
        &self.state
    }

    /// Current keyboard focus.
    #[must_use]
    pub const fn focus(&self) -> DashboardFocus {
        self.focus
    }

    /// Materialize the current mode from the one catalog query.
    #[must_use]
    pub fn snapshot(&self) -> DashboardSnapshot {
        let projects = self.filtered_projects();
        let visible_ids = projects
            .iter()
            .map(|project| project.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut activity = self
            .state
            .activity
            .iter()
            .filter_map(|entry| {
                if !visible_ids.contains(entry.project_id.as_str()) {
                    return None;
                }
                let project = self.state.projects.get(&entry.project_id)?;
                Some(SafeActivityView {
                    project_id: entry.project_id.clone(),
                    project_name: project.name.clone(),
                    label: entry.kind.label(),
                    occurred_at: entry.occurred_at,
                })
            })
            .collect::<Vec<_>>();
        activity.sort_by(|left, right| {
            right
                .occurred_at
                .cmp(&left.occurred_at)
                .then_with(|| left.project_id.cmp(&right.project_id))
                .then_with(|| left.label.cmp(right.label))
        });
        DashboardSnapshot {
            mode: self.state.query.mode,
            query: self.state.query.clone(),
            is_empty: projects.is_empty() && activity.is_empty(),
            projects,
            activity,
        }
    }

    /// Replace the active mode while preserving search, filters, sort, and selection.
    pub fn set_mode(&mut self, mode: DashboardMode) -> Result<(), DashboardError> {
        let previous = self.state.clone();
        self.state.query.mode = mode;
        self.persist_or_restore(previous, Ok(()))
    }

    /// Replace the search query. Matching is case-insensitive over names and admitted metadata.
    pub fn set_search(&mut self, search: impl Into<String>) -> Result<(), DashboardError> {
        let search = search.into();
        validate_search(&search)?;
        let previous = self.state.clone();
        self.state.query.search = search;
        self.persist_or_restore(previous, Ok(()))
    }

    /// Toggle one filter without changing mode, sort, search, or selection.
    pub fn toggle_filter(&mut self, filter: CatalogFilter) -> Result<(), DashboardError> {
        let previous = self.state.clone();
        if !self.state.query.filters.insert(filter) {
            self.state.query.filters.remove(&filter);
        }
        self.persist_or_restore(previous, Ok(()))
    }

    /// Set deterministic catalog ordering.
    pub fn set_sort(&mut self, sort: CatalogSort) -> Result<(), DashboardError> {
        let previous = self.state.clone();
        self.state.query.sort = sort;
        self.persist_or_restore(previous, Ok(()))
    }

    /// Retain or clear a project selection across mode switches.
    pub fn select_project(&mut self, project_id: Option<&str>) -> Result<(), DashboardError> {
        let previous = self.state.clone();
        if let Some(project_id) = project_id {
            if !self.state.projects.contains_key(project_id) {
                return Err(DashboardError::ProjectNotFound(project_id.to_owned()));
            }
            self.state.query.selection = Some(project_id.to_owned());
        } else {
            self.state.query.selection = None;
        }
        self.persist_or_restore(previous, Ok(()))
    }

    /// Add a project to the catalog and persist it.
    pub fn add_project(&mut self, project: ProjectRecord) -> Result<(), DashboardError> {
        project.validate()?;
        if self.state.projects.contains_key(&project.id) {
            return Err(DashboardError::ProjectAlreadyExists(project.id));
        }
        let previous = self.state.clone();
        self.state.projects.insert(project.id.clone(), project);
        self.persist_or_restore(previous, Ok(()))
    }

    /// Append safe activity metadata; arbitrary protected content has no API representation.
    pub fn record_activity(&mut self, activity: SafeActivity) -> Result<(), DashboardError> {
        if !self.state.projects.contains_key(&activity.project_id) {
            return Err(DashboardError::ProjectNotFound(activity.project_id));
        }
        let previous = self.state.clone();
        self.state.activity.push(activity);
        self.persist_or_restore(previous, Ok(()))
    }

    /// Rename a project and update its safe metadata timestamp.
    pub fn rename_project(
        &mut self,
        project_id: &str,
        name: impl Into<String>,
        updated_at: u64,
    ) -> Result<(), DashboardError> {
        let name = name.into();
        validate_text(&name, "project name")?;
        let previous = self.state.clone();
        let project = self
            .state
            .projects
            .get_mut(project_id)
            .ok_or_else(|| DashboardError::ProjectNotFound(project_id.to_owned()))?;
        project.name = name;
        project.updated_at = updated_at;
        project.last_activity_at = updated_at;
        self.persist_or_restore(previous, Ok(()))
    }

    /// Duplicate a project using caller-provided deterministic identity and display name.
    pub fn duplicate_project(
        &mut self,
        source_id: &str,
        new_id: impl Into<String>,
        new_name: impl Into<String>,
        created_at: u64,
    ) -> Result<(), DashboardError> {
        let new_id = new_id.into();
        let new_name = new_name.into();
        validate_text(&new_id, "project id")?;
        validate_text(&new_name, "project name")?;
        if self.state.projects.contains_key(&new_id) {
            return Err(DashboardError::ProjectAlreadyExists(new_id));
        }
        if source_id == new_id {
            return Err(DashboardError::DuplicateIdentity);
        }
        let mut duplicate = self
            .state
            .projects
            .get(source_id)
            .cloned()
            .ok_or_else(|| DashboardError::ProjectNotFound(source_id.to_owned()))?;
        duplicate.id = new_id.clone();
        duplicate.name = new_name;
        duplicate.created_at = created_at;
        duplicate.updated_at = created_at;
        duplicate.last_activity_at = created_at;
        let previous = self.state.clone();
        self.state.projects.insert(new_id, duplicate);
        self.persist_or_restore(previous, Ok(()))
    }

    /// Archive a project without deleting its data.
    pub fn archive_project(&mut self, project_id: &str) -> Result<(), DashboardError> {
        let previous = self.state.clone();
        let project = self
            .state
            .projects
            .get_mut(project_id)
            .ok_or_else(|| DashboardError::ProjectNotFound(project_id.to_owned()))?;
        project.archived = true;
        self.persist_or_restore(previous, Ok(()))
    }

    /// Restore an archived project.
    pub fn restore_project(&mut self, project_id: &str) -> Result<(), DashboardError> {
        let previous = self.state.clone();
        let project = self
            .state
            .projects
            .get_mut(project_id)
            .ok_or_else(|| DashboardError::ProjectNotFound(project_id.to_owned()))?;
        project.archived = false;
        self.persist_or_restore(previous, Ok(()))
    }

    /// Build the exact consequence preview a confirmation surface must show.
    pub fn delete_preview(&self, project_id: &str) -> Result<DeleteConsequences, DashboardError> {
        let project = self
            .state
            .projects
            .get(project_id)
            .ok_or_else(|| DashboardError::ProjectNotFound(project_id.to_owned()))?;
        Ok(DeleteConsequences {
            local_project_data: true,
            cloud_project_copy: project.authority == ProjectAuthority::Cloud,
            asset_count: project.asset_count,
            backup_removed: project.has_backup,
            unsynced_changes_lost: project.has_unsynced_changes
                || project.sync_state == SyncState::Syncing,
        })
    }

    /// Delete only after the caller acknowledges the current consequence preview.
    pub fn delete_project(
        &mut self,
        project_id: &str,
        confirmation: Option<DeleteConfirmation>,
    ) -> Result<ProjectRecord, DashboardError> {
        let consequences = self.delete_preview(project_id)?;
        let Some(confirmation) = confirmation else {
            return Err(DashboardError::DeleteConfirmationRequired(consequences));
        };
        if confirmation.project_id != project_id || confirmation.consequences != consequences {
            return Err(DashboardError::StaleDeleteConfirmation);
        }
        let previous = self.state.clone();
        let deleted = self
            .state
            .projects
            .remove(project_id)
            .ok_or_else(|| DashboardError::ProjectNotFound(project_id.to_owned()))?;
        self.state
            .activity
            .retain(|entry| entry.project_id != project_id);
        if self.state.query.selection.as_deref() == Some(project_id) {
            self.state.query.selection = None;
        }
        if self.state.last_project_id.as_deref() == Some(project_id) {
            self.state.last_project_id = None;
        }
        self.persist_or_restore(previous, Ok(deleted))
    }

    /// Open a project through the session adapter, retaining a dashboard recovery path on failure.
    pub fn open_project<A: ProjectSessionAdapter>(
        &mut self,
        project_id: &str,
        adapter: &mut A,
    ) -> Result<ResumeOutcome, DashboardError> {
        if !self.state.projects.contains_key(project_id) {
            return Err(DashboardError::ProjectNotFound(project_id.to_owned()));
        }
        match adapter.open_project(project_id) {
            Ok(()) => {
                let previous = self.state.clone();
                self.state.last_project_id = Some(project_id.to_owned());
                self.state.query.selection = Some(project_id.to_owned());
                self.persist_or_restore(previous, Ok(ResumeOutcome::Opened(project_id.to_owned())))
            }
            Err(error) => Ok(ResumeOutcome::ReturnedToDashboard(RecoveryDiagnostic {
                project_id: Some(project_id.to_owned()),
                code: error.code,
                message: error.message,
            })),
        }
    }

    /// Resume the last project without trapping the user in a failed editor open.
    pub fn resume_last_project<A: ProjectSessionAdapter>(
        &mut self,
        adapter: &mut A,
    ) -> Result<ResumeOutcome, DashboardError> {
        let Some(project_id) = self.state.last_project_id.clone() else {
            return Ok(ResumeOutcome::NoRememberedProject);
        };
        if !self.state.projects.contains_key(&project_id) {
            let previous = self.state.clone();
            self.state.last_project_id = None;
            return self.persist_or_restore(
                previous,
                Ok(ResumeOutcome::ReturnedToDashboard(RecoveryDiagnostic {
                    project_id: Some(project_id),
                    code: SessionOpenErrorCode::Missing,
                    message: "The last project is unavailable. Choose another project or restore a backup.",
                })),
            );
        }
        self.open_project(&project_id, adapter)
    }

    /// Process keyboard input with focus and activation behavior shared by Grid, Index, and
    /// Activity. The list of projects is always derived from the same filtered query.
    pub fn handle_key(&mut self, key: DashboardKey) -> Result<KeyboardAction, DashboardError> {
        let projects = self.filtered_projects();
        let project_count = projects.len();
        let focus = match key {
            DashboardKey::Escape => {
                if !self.state.query.search.is_empty() {
                    self.set_search(String::new())?;
                    KeyboardAction::ClearSearch
                } else {
                    KeyboardAction::Handled
                }
            }
            DashboardKey::Home => {
                self.focus = DashboardFocus::Search;
                KeyboardAction::Focus(self.focus)
            }
            DashboardKey::End => {
                self.focus = if project_count == 0 {
                    DashboardFocus::Templates
                } else {
                    DashboardFocus::Project(project_count - 1)
                };
                KeyboardAction::Focus(self.focus)
            }
            DashboardKey::Tab | DashboardKey::ShiftTab => {
                self.move_focus(matches!(key, DashboardKey::Tab), project_count);
                KeyboardAction::Focus(self.focus)
            }
            DashboardKey::Up | DashboardKey::Left => {
                self.move_project_focus(false, project_count);
                KeyboardAction::Focus(self.focus)
            }
            DashboardKey::Down | DashboardKey::Right => {
                self.move_project_focus(true, project_count);
                KeyboardAction::Focus(self.focus)
            }
            DashboardKey::Enter => match self.focus {
                DashboardFocus::Project(index) if index < project_count => {
                    let project_id = projects[index].id.clone();
                    self.select_project(Some(&project_id))?;
                    KeyboardAction::OpenProject(project_id)
                }
                DashboardFocus::Create => KeyboardAction::CreateProject,
                DashboardFocus::Import => KeyboardAction::Import,
                DashboardFocus::Templates => KeyboardAction::Templates,
                _ => KeyboardAction::Handled,
            },
        };
        Ok(focus)
    }

    fn move_project_focus(&mut self, forward: bool, project_count: usize) {
        if project_count == 0 {
            return;
        }
        let index = match self.focus {
            DashboardFocus::Project(index) => index,
            _ if forward => 0,
            _ => project_count - 1,
        };
        let index = if forward {
            (index + 1).min(project_count - 1)
        } else {
            index.saturating_sub(1)
        };
        self.focus = DashboardFocus::Project(index);
    }

    fn restore_focus(&mut self) {
        self.focus = DashboardFocus::Search;
        let Some(selection) = self.state.query.selection.as_deref() else {
            return;
        };
        if let Some(index) = self
            .filtered_projects()
            .iter()
            .position(|project| project.id == selection)
        {
            self.focus = DashboardFocus::Project(index);
        }
    }

    fn move_focus(&mut self, forward: bool, project_count: usize) {
        let mut stops = vec![
            DashboardFocus::Search,
            DashboardFocus::Filters,
            DashboardFocus::Sort,
            DashboardFocus::Mode,
            DashboardFocus::Create,
            DashboardFocus::Import,
            DashboardFocus::Templates,
        ];
        stops.extend((0..project_count).map(DashboardFocus::Project));
        let current = stops
            .iter()
            .position(|stop| *stop == self.focus)
            .unwrap_or(0);
        let next = if forward {
            (current + 1) % stops.len()
        } else {
            (current + stops.len() - 1) % stops.len()
        };
        self.focus = stops[next];
    }

    fn filtered_projects(&self) -> Vec<ProjectRecord> {
        let search = normalize(&self.state.query.search);
        let mut projects = self
            .state
            .projects
            .values()
            .filter(|project| {
                let search_matches = search.is_empty() || project.search_text().contains(&search);
                search_matches && matches_filters(project, &self.state.query.filters)
            })
            .cloned()
            .collect::<Vec<_>>();
        projects.sort_by(|left, right| {
            let ordering = match self.state.query.sort {
                CatalogSort::Activity => right.last_activity_at.cmp(&left.last_activity_at),
                CatalogSort::Name => normalize(&left.name).cmp(&normalize(&right.name)),
                CatalogSort::Created => right.created_at.cmp(&left.created_at),
                CatalogSort::Updated => right.updated_at.cmp(&left.updated_at),
                CatalogSort::State => state_rank(left).cmp(&state_rank(right)),
            };
            ordering.then_with(|| left.id.cmp(&right.id))
        });
        projects
    }

    fn persist(&self) -> Result<(), DashboardError> {
        self.persistence
            .save(&self.identity_key, &self.state)
            .map_err(DashboardError::Persistence)
    }

    fn persist_or_restore<T>(
        &mut self,
        previous: DashboardState,
        result: Result<T, DashboardError>,
    ) -> Result<T, DashboardError> {
        let value = result?;
        if let Err(error) = self.persist() {
            self.state = previous;
            return Err(error);
        }
        Ok(value)
    }
}

fn validate_state(state: &DashboardState) -> Result<(), DashboardError> {
    for (key, project) in &state.projects {
        if key != &project.id {
            return Err(DashboardError::InvalidMetadata);
        }
        project.validate()?;
    }
    for activity in &state.activity {
        validate_text(&activity.project_id, "activity project id")?;
    }
    validate_search(&state.query.search)?;
    if let Some(project_id) = &state.query.selection {
        validate_text(project_id, "selection")?;
    }
    if let Some(project_id) = &state.last_project_id {
        validate_text(project_id, "last project")?;
    }
    Ok(())
}

fn matches_filters(project: &ProjectRecord, filters: &BTreeSet<CatalogFilter>) -> bool {
    // Archived projects remain in the persisted catalog but are opt-in in the normal dashboard.
    // Selecting Archived makes the filter explicit and turns the active view into an archive
    // view; combining it with Local/Cloud still applies the authority constraint.
    if project.archived && !filters.contains(&CatalogFilter::Archived) {
        return false;
    }
    let authority_filters = filters
        .iter()
        .filter(|filter| matches!(filter, CatalogFilter::Local | CatalogFilter::Cloud))
        .collect::<Vec<_>>();
    let authority_matches = authority_filters.is_empty()
        || authority_filters.iter().any(|filter| {
            matches!(
                (filter, project.authority),
                (CatalogFilter::Local, ProjectAuthority::Local)
                    | (CatalogFilter::Cloud, ProjectAuthority::Cloud)
            )
        });
    let state_filters = filters
        .iter()
        .filter(|filter| !matches!(filter, CatalogFilter::Local | CatalogFilter::Cloud))
        .collect::<Vec<_>>();
    let state_matches = state_filters.is_empty()
        || state_filters.iter().any(|filter| match (filter, project) {
            (CatalogFilter::Syncing, project) => project.sync_state == SyncState::Syncing,
            (CatalogFilter::Conflicted, project) => project.sync_state == SyncState::Conflicted,
            (CatalogFilter::Recovered, project) => project.recovered,
            (CatalogFilter::Archived, project) => project.archived,
            _ => false,
        });
    authority_matches && state_matches
}

fn state_rank(project: &ProjectRecord) -> (bool, SyncState, bool) {
    (project.archived, project.sync_state, project.recovered)
}

fn validate_text(value: &str, _field: &str) -> Result<(), DashboardError> {
    if value.is_empty() || value.len() > MAX_TEXT_LENGTH || value.chars().any(char::is_control) {
        return Err(DashboardError::InvalidMetadata);
    }
    Ok(())
}

fn validate_search(value: &str) -> Result<(), DashboardError> {
    if value.len() > MAX_TEXT_LENGTH || value.chars().any(char::is_control) {
        return Err(DashboardError::InvalidMetadata);
    }
    Ok(())
}

fn normalize(value: &str) -> String {
    value.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Identity(bool);
    impl DashboardIdentity for Identity {
        fn identity_key(&self) -> &str {
            "local:designer"
        }
        fn is_authenticated(&self) -> bool {
            self.0
        }
    }

    struct Session(bool);
    impl ProjectSessionAdapter for Session {
        fn open_project(&mut self, _: &str) -> Result<(), SessionOpenError> {
            if self.0 {
                Ok(())
            } else {
                Err(SessionOpenError {
                    code: SessionOpenErrorCode::Corrupt,
                    message: "Restore a known-good project backup.",
                })
            }
        }
    }

    #[derive(Clone)]
    struct RejectingPersistence;
    impl DashboardPersistence for RejectingPersistence {
        fn load(&self, _: &str) -> Result<Option<DashboardState>, DashboardPersistenceError> {
            Ok(None)
        }

        fn save(&self, _: &str, _: &DashboardState) -> Result<(), DashboardPersistenceError> {
            Err(DashboardPersistenceError {
                code: DashboardPersistenceErrorCode::Unavailable,
                message: "Dashboard storage is unavailable.".to_owned(),
            })
        }
    }

    fn project(id: &str, name: &str, authority: ProjectAuthority, timestamp: u64) -> ProjectRecord {
        ProjectRecord::new(id, name, authority, timestamp)
    }

    #[test]
    fn mode_switch_preserves_the_single_persisted_query() {
        let storage = InMemoryDashboardPersistence::default();
        let mut dashboard = ProjectDashboard::open(&Identity(true), storage.clone()).unwrap();
        dashboard
            .add_project(project("a", "Alpha", ProjectAuthority::Local, 1))
            .unwrap();
        dashboard.set_search("ALP").unwrap();
        dashboard.toggle_filter(CatalogFilter::Local).unwrap();
        dashboard.set_sort(CatalogSort::Updated).unwrap();
        dashboard.select_project(Some("a")).unwrap();
        let query = dashboard.snapshot().query;
        dashboard.set_mode(DashboardMode::Activity).unwrap();
        assert_eq!(dashboard.snapshot().query.search, query.search);
        assert_eq!(dashboard.snapshot().query.filters, query.filters);
        assert_eq!(dashboard.snapshot().query.sort, query.sort);
        assert_eq!(dashboard.snapshot().query.selection, query.selection);
        assert_eq!(
            storage.state("local:designer").unwrap().query.mode,
            DashboardMode::Activity
        );
    }

    #[test]
    fn search_matches_names_and_admitted_metadata_deterministically() {
        let storage = InMemoryDashboardPersistence::default();
        let mut dashboard = ProjectDashboard::open(&Identity(true), storage).unwrap();
        let mut alpha = project("a", "Alpha", ProjectAuthority::Local, 1);
        alpha.admitted_metadata.push("restaurant".to_owned());
        dashboard.add_project(alpha).unwrap();
        dashboard
            .add_project(project("b", "Beta", ProjectAuthority::Local, 2))
            .unwrap();
        dashboard.set_search("RESTAURANT").unwrap();
        assert_eq!(
            dashboard
                .snapshot()
                .projects
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a"]
        );
    }

    #[test]
    fn delete_requires_current_informed_consequences() {
        let storage = InMemoryDashboardPersistence::default();
        let mut dashboard = ProjectDashboard::open(&Identity(true), storage).unwrap();
        let mut record = project("cloud", "Cloud", ProjectAuthority::Cloud, 1);
        record.asset_count = 3;
        record.has_backup = true;
        record.has_unsynced_changes = true;
        dashboard.add_project(record).unwrap();
        let preview = dashboard.delete_preview("cloud").unwrap();
        assert!(dashboard.delete_project("cloud", None).is_err());
        let confirmation = DeleteConfirmation::acknowledge("cloud", preview.clone());
        assert_eq!(
            dashboard
                .delete_project("cloud", Some(confirmation))
                .unwrap()
                .id,
            "cloud"
        );
        assert!(
            preview
                .lines()
                .iter()
                .any(|line| line.contains("Unsynchronized"))
        );
    }

    #[test]
    fn failed_resume_returns_to_dashboard_with_safe_recovery_diagnostic() {
        let storage = InMemoryDashboardPersistence::default();
        let mut dashboard = ProjectDashboard::open(&Identity(true), storage).unwrap();
        dashboard
            .add_project(project("a", "Alpha", ProjectAuthority::Local, 1))
            .unwrap();
        dashboard.open_project("a", &mut Session(true)).unwrap();
        let outcome = dashboard.resume_last_project(&mut Session(false)).unwrap();
        assert_eq!(
            outcome,
            ResumeOutcome::ReturnedToDashboard(RecoveryDiagnostic {
                project_id: Some("a".to_owned()),
                code: SessionOpenErrorCode::Corrupt,
                message: "Restore a known-good project backup."
            })
        );
    }

    #[test]
    fn keyboard_navigation_activates_projects_in_every_mode() {
        for mode in [
            DashboardMode::Grid,
            DashboardMode::Index,
            DashboardMode::Activity,
        ] {
            let storage = InMemoryDashboardPersistence::default();
            let mut dashboard = ProjectDashboard::open(&Identity(true), storage).unwrap();
            dashboard
                .add_project(project("a", "Alpha", ProjectAuthority::Local, 1))
                .unwrap();
            dashboard.set_mode(mode).unwrap();
            dashboard.handle_key(DashboardKey::Tab).unwrap();
            for _ in 0..6 {
                dashboard.handle_key(DashboardKey::Tab).unwrap();
            }
            assert_eq!(dashboard.focus(), DashboardFocus::Project(0));
            assert_eq!(
                dashboard.handle_key(DashboardKey::Enter).unwrap(),
                KeyboardAction::OpenProject("a".to_owned())
            );
        }
    }

    #[test]
    fn unauthenticated_identity_cannot_load_catalog() {
        let result =
            ProjectDashboard::open(&Identity(false), InMemoryDashboardPersistence::default());
        assert!(matches!(result, Err(DashboardError::Unauthenticated)));
    }

    #[test]
    fn failed_persistence_rolls_back_query_and_catalog_mutations() {
        let mut dashboard = ProjectDashboard::open(&Identity(true), RejectingPersistence).unwrap();
        assert!(
            dashboard
                .add_project(project("a", "Alpha", ProjectAuthority::Local, 1))
                .is_err()
        );
        assert!(dashboard.state().projects.is_empty());
        assert_eq!(dashboard.state().query, DashboardQuery::default());
    }
}
