//! Production native product bootstrap and route composition.
//!
//! This module is the seam between the host-owned persistence/authentication
//! services and GPUI. It intentionally keeps route state renderer-neutral so
//! first-launch, remembered-session, offline, and recovery behavior can be
//! tested without starting a window.

#![allow(missing_docs)]
#![allow(clippy::all, clippy::pedantic, clippy::restriction, clippy::nursery)]

use std::{
    future::Future,
    path::PathBuf,
    sync::Arc,
    task::{Context as TaskContext, Poll, Wake, Waker},
};

use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Subscription, Window, div,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use serde::{Serialize, de::DeserializeOwned};
use studio_design::{
    Actor, ActorId, ActorKind, DefaultDesignerSession, DesignNode, NodeId, NodeKind, NodeParent,
    OperationId, ProjectId, PropertyValue, Screen, ScreenId, StudioDesign, UndoGroupId,
};
use studio_host::{
    CreateIdentityRequest, Durability, EmbeddedLocalStore, IdentityError, IdentityService,
    IdentitySession, IdentitySnapshot, IdentityState, IdentitySummary,
    LocalStoreDesignerPersistence, LocalStoreDiagnosticCode, LocalStoreError,
};
use studio_security::OsSessionCredentialStore;

use crate::connection::{
    CachedProject, ConnectionIndicator, ConnectionState, SyncCoordinator, SyncReceipt, SyncWorker,
    SyncWorkerError, SyncWorkerErrorCode,
};
use crate::focus_view::{FocusOpenError, FocusView, FocusViewModel};
use crate::project_dashboard::{
    DashboardError, DashboardPersistence as DashboardPersistenceTrait, DashboardPersistenceError,
    DashboardPersistenceErrorCode, DashboardState, DeleteConfirmation, ProjectAuthority,
    ProjectDashboard, ProjectRecord,
};
use crate::settings::{GlobalSettingChange, GlobalSettings, ProjectSettings};
use crate::welcome::welcome_screen;
use crate::{
    IdentityShellRoute, IdentityShellState, SettingsController, SettingsError, SettingsErrorCode,
    SettingsPersistence,
};

const DASHBOARD_BATCH_PREFIX: &str = "studio-dashboard-v1-";
const SETTINGS_GLOBAL_BATCH: &str = "studio-settings-global-v1";
const SETTINGS_PROJECT_BATCH_PREFIX: &str = "studio-settings-project-v1-";

/// Whether a native Wayland endpoint is available for the Designer launch.
#[must_use]
pub fn wayland_available() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty())
        || std::env::var_os("WAYLAND_SOCKET").is_some_and(|value| !value.is_empty())
}

/// Offline-first worker used until a cloud transport is provisioned.
///
/// It deliberately never fabricates a successful transfer: cached projects
/// remain editable and the shared indicator communicates that cloud work is
/// unavailable.
#[derive(Clone, Copy, Debug, Default)]
pub struct OfflineSyncWorker;

impl SyncWorker for OfflineSyncWorker {
    fn sync(
        &mut self,
        _project: &CachedProject,
        _operations: &[crate::connection::SyncOutboxEnvelope],
    ) -> Result<SyncReceipt, SyncWorkerError> {
        Err(SyncWorkerError {
            code: SyncWorkerErrorCode::Offline,
            message: "Cloud sync is unavailable; local and cached projects remain usable."
                .to_owned(),
            retryable: true,
        })
    }
}

/// Routes owned by the shipped native product shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductRoute {
    /// First-launch product welcome.
    Welcome,
    /// Remembered identities and sessions.
    IdentityChooser,
    /// Local identity creation form.
    CreateIdentity,
    /// Password sign-in form for one identity.
    SignIn { identity_id: String },
    /// Password unlock form for one locked identity.
    Unlock { identity_id: String },
    /// Authenticated project dashboard.
    Dashboard,
    /// Global and project settings.
    Settings,
    /// Product help navigator.
    Help,
    /// Product and license information.
    About,
    /// Shared cloud/sync status.
    SyncStatus,
    /// Conflict center, before opening an editor.
    Conflicts,
    /// Recovery center, before opening an editor.
    Recovery,
    /// An authenticated project editor route.
    Project { project_id: String },
}

impl ProductRoute {
    /// Canonical path exposed to accessibility tooling and diagnostics.
    #[must_use]
    pub fn path(&self) -> String {
        match self {
            Self::Welcome => "/welcome".to_owned(),
            Self::IdentityChooser => "/identity".to_owned(),
            Self::CreateIdentity => "/identity/create".to_owned(),
            Self::SignIn { identity_id } => format!("/identity/{identity_id}/sign-in"),
            Self::Unlock { identity_id } => format!("/identity/{identity_id}/unlock"),
            Self::Dashboard => "/dashboard".to_owned(),
            Self::Settings => "/settings".to_owned(),
            Self::Help => "/help".to_owned(),
            Self::About => "/about".to_owned(),
            Self::SyncStatus => "/dashboard/sync".to_owned(),
            Self::Conflicts => "/dashboard/conflicts".to_owned(),
            Self::Recovery => "/dashboard/recovery".to_owned(),
            Self::Project { project_id } => format!("/projects/{project_id}"),
        }
    }

    /// Stable human-readable route title.
    #[must_use]
    pub const fn title(&self) -> &'static str {
        match self {
            Self::Welcome => "Welcome to Studio",
            Self::IdentityChooser => "Choose an identity",
            Self::CreateIdentity => "Create a local identity",
            Self::SignIn { .. } => "Sign in",
            Self::Unlock { .. } => "Unlock identity",
            Self::Dashboard => "Project Dashboard",
            Self::Settings => "Settings",
            Self::Help => "Help",
            Self::About => "About Studio",
            Self::SyncStatus => "Sync status",
            Self::Conflicts => "Conflicts",
            Self::Recovery => "Recovery",
            Self::Project { .. } => "Studio Designer",
        }
    }
}

/// Durable dashboard persistence backed by the host LocalStore.
#[derive(Clone)]
pub struct LocalStoreDashboardPersistence {
    store: Arc<EmbeddedLocalStore>,
}

impl LocalStoreDashboardPersistence {
    /// Wrap the store opened by native startup.
    #[must_use]
    pub fn new(store: Arc<EmbeddedLocalStore>) -> Self {
        Self { store }
    }
}

impl DashboardPersistenceTrait for LocalStoreDashboardPersistence {
    fn load(
        &self,
        identity_key: &str,
    ) -> Result<Option<DashboardState>, DashboardPersistenceError> {
        load_record(&self.store, &dashboard_batch_id(identity_key)).map_err(dashboard_error)
    }

    fn save(
        &self,
        identity_key: &str,
        state: &DashboardState,
    ) -> Result<(), DashboardPersistenceError> {
        save_record(&self.store, &dashboard_batch_id(identity_key), state).map_err(dashboard_error)
    }
}

/// Durable global/project settings persistence backed by the host LocalStore.
#[derive(Clone)]
pub struct LocalStoreSettingsPersistence {
    store: Arc<EmbeddedLocalStore>,
}

impl LocalStoreSettingsPersistence {
    /// Wrap the store opened by native startup.
    #[must_use]
    pub fn new(store: Arc<EmbeddedLocalStore>) -> Self {
        Self { store }
    }
}

impl SettingsPersistence for LocalStoreSettingsPersistence {
    fn load_global(&self) -> Result<Option<GlobalSettings>, SettingsError> {
        load_record(&self.store, SETTINGS_GLOBAL_BATCH).map_err(settings_error)
    }

    fn save_global(&self, settings: &GlobalSettings) -> Result<(), SettingsError> {
        save_record(&self.store, SETTINGS_GLOBAL_BATCH, settings).map_err(settings_error)
    }

    fn load_project(&self, project_id: &str) -> Result<Option<ProjectSettings>, SettingsError> {
        load_record(&self.store, &settings_project_batch_id(project_id)).map_err(settings_error)
    }

    fn save_project(&self, settings: &ProjectSettings) -> Result<(), SettingsError> {
        save_record(
            &self.store,
            &settings_project_batch_id(&settings.project_id),
            settings,
        )
        .map_err(settings_error)
    }
}

fn load_record<T: DeserializeOwned>(
    store: &EmbeddedLocalStore,
    batch_id: &str,
) -> Result<Option<T>, LocalStoreError> {
    let entries = store.batch_entries_blocking(batch_id)?;
    if entries.is_empty() {
        return Ok(None);
    }
    if entries.len() != 1 {
        return Err(LocalStoreError::new_for_adapter(
            LocalStoreDiagnosticCode::BatchInvalid,
        ));
    }
    serde_json::from_value(entries[0].payload.clone())
        .map(Some)
        .map_err(|_| {
            LocalStoreError::new_for_adapter(LocalStoreDiagnosticCode::SchemaMetadataCorrupt)
        })
}

fn save_record<T: Serialize>(
    store: &EmbeddedLocalStore,
    batch_id: &str,
    record: &T,
) -> Result<(), LocalStoreError> {
    let payload = serde_json::to_value(record)
        .map_err(|_| LocalStoreError::new_for_adapter(LocalStoreDiagnosticCode::BatchInvalid))?;
    let batch = studio_host::StoreBatch::new(
        batch_id,
        [studio_host::StoreBatchEntry {
            ordinal: 0,
            payload,
        }],
    )?;
    store.write_batch_blocking(&batch)
}

fn dashboard_batch_id(identity_key: &str) -> String {
    stable_batch_id(DASHBOARD_BATCH_PREFIX, identity_key)
}

fn settings_project_batch_id(project_id: &str) -> String {
    stable_batch_id(SETTINGS_PROJECT_BATCH_PREFIX, project_id)
}

fn stable_batch_id(prefix: &str, value: &str) -> String {
    let mut id = String::from(prefix);
    for byte in value.as_bytes() {
        id.push_str(&format!("{byte:02x}"));
    }
    id
}

fn dashboard_error(error: LocalStoreError) -> DashboardPersistenceError {
    DashboardPersistenceError {
        code: match error.diagnostic().code() {
            LocalStoreDiagnosticCode::SchemaMetadataCorrupt
            | LocalStoreDiagnosticCode::BatchInvalid => DashboardPersistenceErrorCode::Corrupt,
            _ => DashboardPersistenceErrorCode::Unavailable,
        },
        message: "The local dashboard state is unavailable. Local projects remain protected."
            .to_owned(),
    }
}

fn settings_error(error: LocalStoreError) -> SettingsError {
    SettingsError {
        code: match error.diagnostic().code() {
            LocalStoreDiagnosticCode::SchemaMetadataCorrupt
            | LocalStoreDiagnosticCode::BatchInvalid => SettingsErrorCode::Corrupt,
            _ => SettingsErrorCode::Unavailable,
        },
        message: "The local settings state is unavailable. Try again after restarting Studio."
            .to_owned(),
    }
}

/// Renderer-neutral product state used by the GPUI shell and deterministic tests.
#[derive(Clone, Debug)]
pub struct NativeProductState {
    route: ProductRoute,
    identity: IdentityShellState,
    session: Option<IdentitySession>,
    indicator: ConnectionIndicator,
}

impl NativeProductState {
    /// Compose startup state from a host snapshot and an optionally resumed session.
    #[must_use]
    pub fn new(snapshot: IdentitySnapshot, session: Option<IdentitySession>) -> Self {
        let identity = IdentityShellState::from_snapshot(snapshot);
        let route = if session.is_some() {
            ProductRoute::Dashboard
        } else if matches!(identity.route(), IdentityShellRoute::Welcome) {
            ProductRoute::Welcome
        } else {
            ProductRoute::IdentityChooser
        };
        let connected = session.is_some();
        Self {
            route,
            identity,
            session,
            indicator: ConnectionIndicator::from_state(if connected {
                ConnectionState::Synced
            } else {
                ConnectionState::Offline
            }),
        }
    }

    /// Current route.
    #[must_use]
    pub const fn route(&self) -> &ProductRoute {
        &self.route
    }

    /// Current identity snapshot shown by the chooser.
    #[must_use]
    pub const fn identity(&self) -> &IdentityShellState {
        &self.identity
    }

    /// Current authenticated session, if any.
    #[must_use]
    pub const fn session(&self) -> Option<&IdentitySession> {
        self.session.as_ref()
    }

    /// Shared connection indicator for every product surface.
    #[must_use]
    pub const fn indicator(&self) -> &ConnectionIndicator {
        &self.indicator
    }

    /// Whether dashboard/project routes are currently authorized.
    #[must_use]
    pub const fn is_authenticated(&self) -> bool {
        self.session.is_some()
    }

    /// Install a host-authenticated session and enter the dashboard.
    ///
    /// The opaque session is accepted only when its identity is present in the
    /// host snapshot, preserving the identity shell's fail-closed invariant.
    pub fn set_authenticated_session(&mut self, session: IdentitySession) -> bool {
        if !self.identity.enter_project(&session) {
            return false;
        }
        self.session = Some(session);
        self.indicator = ConnectionIndicator::from_state(ConnectionState::Offline);
        self.route = ProductRoute::Dashboard;
        true
    }

    /// Persisted welcome dismissal transitions to the identity chooser.
    pub fn dismiss_welcome(&mut self) {
        self.identity.dismiss_welcome();
        self.route = ProductRoute::IdentityChooser;
    }

    /// Leave welcome for the local identity gate: sign-in, unlock, or create.
    pub fn open_local_identity(&mut self) {
        self.identity.dismiss_welcome();
        let selected = self
            .identity
            .identities()
            .first()
            .map(|identity| (identity.identity_id.clone(), identity.state));
        if let Some((identity_id, state)) = selected {
            let _ = self.identity.choose_identity(&identity_id);
            self.route = match state {
                IdentityState::Available => ProductRoute::SignIn { identity_id },
                IdentityState::Locked => ProductRoute::Unlock { identity_id },
            };
        } else {
            self.identity.begin_create_identity();
            self.route = ProductRoute::CreateIdentity;
        }
    }

    /// Navigate to a product route, rejecting authenticated routes when signed out.
    pub fn navigate(&mut self, route: ProductRoute) -> bool {
        if matches!(
            route,
            ProductRoute::Dashboard
                | ProductRoute::Settings
                | ProductRoute::SyncStatus
                | ProductRoute::Conflicts
                | ProductRoute::Recovery
                | ProductRoute::Project { .. }
        ) && !self.is_authenticated()
        {
            return false;
        }
        self.route = route;
        true
    }

    /// Open a project only after authentication has established a session.
    pub fn open_project(&mut self, project_id: impl Into<String>) -> bool {
        self.navigate(ProductRoute::Project {
            project_id: project_id.into(),
        })
    }

    /// Clear the active session and return to the identity gate.
    pub fn sign_out(&mut self) {
        self.session = None;
        self.indicator = ConnectionIndicator::from_state(ConnectionState::Offline);
        self.route = ProductRoute::IdentityChooser;
    }
}

/// Host-owned production services composed before GPUI starts.
pub struct NativeProductBootstrap {
    store: Arc<EmbeddedLocalStore>,
    identity_service: Arc<IdentityService<EmbeddedLocalStore, OsSessionCredentialStore>>,
    sync: SyncCoordinator<OfflineSyncWorker>,
    state: NativeProductState,
}

impl NativeProductBootstrap {
    /// Open production persistence, restore a remembered session, and compose route state.
    pub fn open(directory: impl Into<PathBuf>) -> Result<Self, BootstrapError> {
        let store = Arc::new(EmbeddedLocalStore::open_blocking(
            directory,
            Durability::Every,
        )?);
        let identity_service = Arc::new(IdentityService::with_credentials(
            Arc::clone(&store),
            OsSessionCredentialStore,
        ));
        let snapshot = identity_service.snapshot_blocking()?;
        let session = snapshot
            .sessions
            .iter()
            .filter(|session| {
                session.remembered && matches!(session.state, studio_host::SessionState::Available)
            })
            .find_map(|session| identity_service.resume_blocking(&session.session_id).ok());
        let mut sync = SyncCoordinator::new(OfflineSyncWorker);
        sync.set_connected(false);
        if session.is_none() {
            sync.sign_out();
        }
        let mut state = NativeProductState::new(snapshot, session);
        state.indicator = sync.indicator().clone();
        Ok(Self {
            store,
            identity_service,
            sync,
            state,
        })
    }

    /// Resolve the default per-user Studio data directory.
    #[must_use]
    pub fn default_data_directory() -> PathBuf {
        if let Some(path) = std::env::var_os("STUDIO_DATA_DIR") {
            return PathBuf::from(path);
        }
        if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(path).join("studio");
        }
        std::env::var_os("HOME")
            .map(|path| PathBuf::from(path).join(".local/share/studio"))
            .unwrap_or_else(|| PathBuf::from(".studio"))
    }

    /// Access the composed product state.
    #[must_use]
    pub const fn state(&self) -> &NativeProductState {
        &self.state
    }

    /// Access mutable product state for a renderer or host action dispatcher.
    #[must_use]
    pub fn state_mut(&mut self) -> &mut NativeProductState {
        &mut self.state
    }

    /// Access the shared offline-first synchronization coordinator.
    #[must_use]
    pub const fn sync(&self) -> &SyncCoordinator<OfflineSyncWorker> {
        &self.sync
    }

    /// Open the durable dashboard for the active identity.
    pub fn dashboard(
        &self,
    ) -> Result<ProjectDashboard<LocalStoreDashboardPersistence>, DashboardError> {
        let identity = self
            .state
            .session()
            .ok_or(DashboardError::Unauthenticated)?;
        ProjectDashboard::for_authenticated_identity(
            identity.identity_id(),
            LocalStoreDashboardPersistence::new(Arc::clone(&self.store)),
        )
    }

    /// Open the durable settings controller.
    pub fn settings(
        &self,
    ) -> Result<SettingsController<LocalStoreSettingsPersistence>, SettingsError> {
        SettingsController::new(LocalStoreSettingsPersistence::new(Arc::clone(&self.store)))
    }

    /// Persist the welcome choice and update the route.
    pub fn dismiss_welcome(&mut self) -> Result<(), IdentityError> {
        self.identity_service.dismiss_welcome_blocking()?;
        self.state.dismiss_welcome();
        Ok(())
    }

    /// Persist welcome dismissal and open the local identity gate.
    pub fn open_local_identity(&mut self) -> Result<(), IdentityError> {
        self.identity_service.dismiss_welcome_blocking()?;
        self.state.open_local_identity();
        Ok(())
    }

    /// End use of the active session while retaining its remembered credential.
    pub fn sign_out(&mut self) -> Result<(), IdentityError> {
        if let Some(session) = self.state.session().cloned() {
            self.identity_service.sign_out(&session)?;
        }
        self.sync.sign_out();
        self.state.sign_out();
        Ok(())
    }

    /// Revoke the active session and return to the chooser.
    pub fn revoke_active_session(&mut self) -> Result<(), IdentityError> {
        if let Some(session) = self.state.session().cloned() {
            self.identity_service
                .revoke_session_blocking(session.session_id())?;
        }
        self.sync.sign_out();
        self.state.sign_out();
        Ok(())
    }

    /// Create a local identity from a renderer-facing form submission.
    ///
    /// Validation is intentionally repeated at this boundary so deterministic
    /// callers and native UI share the same fail-closed behavior. The host
    /// remains the source of truth for password policy and persistence.
    pub fn create_identity_blocking(
        &mut self,
        display_name: impl Into<String>,
        password: impl Into<Vec<u8>>,
        confirmation: impl Into<Vec<u8>>,
    ) -> Result<IdentitySummary, IdentityError> {
        let display_name = display_name.into();
        let password = password.into();
        let confirmation = confirmation.into();
        if display_name.trim().is_empty() || password != confirmation {
            return Err(IdentityError::InvalidInput);
        }
        let service = Arc::clone(&self.identity_service);
        let request = CreateIdentityRequest {
            display_name,
            email: None,
            avatar: None,
            password,
        };
        let summary = block_on_native(service.create_identity(request))?;
        let snapshot = block_on_native(service.snapshot())?;
        self.state.identity.refresh(snapshot);
        self.state.route = ProductRoute::IdentityChooser;
        Ok(summary)
    }

    /// Authenticate an available identity and enter the dashboard.
    pub fn sign_in_blocking(
        &mut self,
        identity_id: &str,
        password: impl AsRef<[u8]>,
        remember: bool,
    ) -> Result<(), IdentityError> {
        let service = Arc::clone(&self.identity_service);
        let result = block_on_native(service.sign_in(identity_id, password.as_ref(), remember));
        self.refresh_identity_snapshot()?;
        let session = result?;
        self.install_session(session);
        Ok(())
    }

    /// Authenticate a locked identity through the explicit unlock gate.
    pub fn unlock_blocking(
        &mut self,
        identity_id: &str,
        password: impl AsRef<[u8]>,
        remember: bool,
    ) -> Result<(), IdentityError> {
        let service = Arc::clone(&self.identity_service);
        let result = block_on_native(service.unlock(identity_id, password.as_ref(), remember));
        self.refresh_identity_snapshot()?;
        let session = result?;
        self.install_session(session);
        Ok(())
    }

    fn refresh_identity_snapshot(&mut self) -> Result<(), IdentityError> {
        let snapshot = block_on_native(self.identity_service.snapshot())?;
        self.state.identity.refresh(snapshot);
        Ok(())
    }

    fn install_session(&mut self, session: IdentitySession) {
        self.sync.set_connected(false);
        let _ = self.state.set_authenticated_session(session);
        self.state.indicator = self.sync.indicator().clone();
    }
}

struct NativeWake;

impl Wake for NativeWake {
    fn wake(self: Arc<Self>) {}
}

fn block_on_native<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(NativeWake));
    let mut context = TaskContext::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

/// Safe native bootstrap failure.
#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    /// Local persistence failed closed.
    #[error(transparent)]
    Store(#[from] LocalStoreError),
    /// Identity catalog or OS credential restore failed closed.
    #[error(transparent)]
    Identity(#[from] IdentityError),
}

/// Minimal GPUI application shell for the product routes.
pub struct NativeProductShell {
    bootstrap: NativeProductBootstrap,
    focus_view: Option<Entity<FocusView<LocalStoreDesignerPersistence>>>,
    focus_project_id: Option<String>,
    focus_loading: bool,
    focus_error: Option<String>,
    create_name: Option<Entity<InputState>>,
    create_password: Option<Entity<InputState>>,
    create_confirmation: Option<Entity<InputState>>,
    gate_password: Option<Entity<InputState>>,
    input_subscriptions: Vec<Subscription>,
    input_route: Option<ProductRoute>,
    remember_session: bool,
    identity_error: Option<String>,
    dashboard: Option<ProjectDashboard<LocalStoreDashboardPersistence>>,
    dashboard_error: Option<String>,
    next_project_number: u32,
    pending_delete: Option<(String, crate::project_dashboard::DeleteConsequences)>,
    settings: Option<SettingsController<LocalStoreSettingsPersistence>>,
    settings_error: Option<String>,
}

impl NativeProductShell {
    /// Create the authenticated Designer route shell.
    pub fn new(bootstrap: NativeProductBootstrap, _reduced_motion: bool) -> Self {
        Self {
            bootstrap,
            focus_view: None,
            focus_project_id: None,
            focus_loading: false,
            focus_error: None,
            create_name: None,
            create_password: None,
            create_confirmation: None,
            gate_password: None,
            input_subscriptions: Vec::new(),
            input_route: None,
            remember_session: true,
            identity_error: None,
            dashboard: None,
            dashboard_error: None,
            next_project_number: 1,
            pending_delete: None,
            settings: None,
            settings_error: None,
        }
    }

    fn ensure_identity_inputs(
        &mut self,
        route: &ProductRoute,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.input_route.as_ref() == Some(route) {
            return;
        }
        self.input_route = Some(route.clone());
        self.input_subscriptions.clear();
        self.identity_error = None;
        self.create_name = None;
        self.create_password = None;
        self.create_confirmation = None;
        self.gate_password = None;
        match route {
            ProductRoute::CreateIdentity => {
                let name = cx.new(|cx| InputState::new(window, cx).placeholder("Display name"));
                let password = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Password")
                        .masked(true)
                });
                let confirmation = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Confirm password")
                        .masked(true)
                });
                self.create_name = Some(name.clone());
                self.create_password = Some(password.clone());
                self.create_confirmation = Some(confirmation.clone());
                for input in [name, password, confirmation] {
                    self.input_subscriptions.push(cx.subscribe(
                        &input,
                        |this, _, event: &InputEvent, cx| {
                            if matches!(event, InputEvent::Change) {
                                this.identity_error = None;
                                cx.notify();
                            }
                        },
                    ));
                }
            }
            ProductRoute::SignIn { .. } | ProductRoute::Unlock { .. } => {
                let password = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Password")
                        .masked(true)
                });
                self.gate_password = Some(password.clone());
                self.input_subscriptions.push(cx.subscribe(
                    &password,
                    |this, _, event: &InputEvent, cx| {
                        if matches!(event, InputEvent::Change) {
                            this.identity_error = None;
                            cx.notify();
                        }
                    },
                ));
            }
            _ => {}
        }
    }

    fn ensure_dashboard(&mut self) {
        if self.dashboard.is_some() || self.dashboard_error.is_some() {
            return;
        }
        match self.bootstrap.dashboard() {
            Ok(dashboard) => self.dashboard = Some(dashboard),
            Err(error) => self.dashboard_error = Some(error.to_string()),
        }
    }

    fn ensure_settings(&mut self) {
        if self.settings.is_some() || self.settings_error.is_some() {
            return;
        }
        match self.bootstrap.settings() {
            Ok(settings) => self.settings = Some(settings),
            Err(error) => self.settings_error = Some(error.to_string()),
        }
    }

    fn create_project(&mut self) -> Option<String> {
        let dashboard = self.dashboard.as_mut()?;
        let number = self.next_project_number;
        self.next_project_number = self.next_project_number.saturating_add(1);
        let id = format!("local-project-{number}");
        let project = ProjectRecord::new(
            &id,
            format!("Untitled project {number}"),
            ProjectAuthority::Local,
            number as u64,
        );
        dashboard.add_project(project).ok()?;
        Some(id)
    }

    fn submit_create(&mut self, cx: &mut Context<Self>) {
        let Some(name) = self
            .create_name
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
        else {
            return;
        };
        let Some(password) = self
            .create_password
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
        else {
            return;
        };
        let Some(confirmation) = self
            .create_confirmation
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
        else {
            return;
        };
        match self.bootstrap.create_identity_blocking(
            name,
            password.into_bytes(),
            confirmation.into_bytes(),
        ) {
            Ok(_) => {
                self.identity_error = None;
                self.input_route = None;
                cx.notify();
            }
            Err(error) => {
                self.identity_error = Some(safe_identity_error(&error));
                cx.notify();
            }
        }
    }

    fn submit_gate(&mut self, identity_id: String, unlock: bool, cx: &mut Context<Self>) {
        let Some(password) = self
            .gate_password
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
        else {
            return;
        };
        let result = if unlock {
            self.bootstrap
                .unlock_blocking(&identity_id, password.as_bytes(), self.remember_session)
        } else {
            self.bootstrap.sign_in_blocking(
                &identity_id,
                password.as_bytes(),
                self.remember_session,
            )
        };
        match result {
            Ok(()) => {
                self.identity_error = None;
                self.input_route = None;
                self.dashboard = None;
                self.dashboard_error = None;
            }
            Err(error) => {
                self.identity_error = Some(safe_identity_error(&error));
                if !unlock && error.code() == studio_host::IdentityErrorCode::WrongPassword {
                    self.bootstrap
                        .state_mut()
                        .navigate(ProductRoute::Unlock { identity_id });
                    self.input_route = None;
                }
            }
        }
        cx.notify();
    }

    fn start_focus_open(&mut self, project_id: String, cx: &mut Context<Self>) {
        if self.focus_loading {
            return;
        }
        let Some(identity) = self.bootstrap.state().session().cloned() else {
            self.focus_error =
                Some("Authentication is required before opening a project.".to_owned());
            return;
        };
        let Ok(project_id_typed) = ProjectId::new(project_id.clone()) else {
            self.focus_error = Some("The project identity is invalid.".to_owned());
            return;
        };
        let Ok(actor_id) = ActorId::new(format!("designer-{}", identity.identity_id())) else {
            self.focus_error =
                Some("The authenticated identity cannot author this project.".to_owned());
            return;
        };
        let actor = Actor {
            id: actor_id,
            kind: ActorKind::Human,
            display_name: "Designer".to_owned(),
        };
        let store = Arc::clone(&self.bootstrap.store);
        self.focus_loading = true;
        self.focus_error = None;
        self.focus_project_id = Some(project_id);
        cx.spawn(async move |this, cx| {
            let result = open_or_create_focus(store, project_id_typed, actor).await;
            this.update(cx, |shell, cx| {
                shell.focus_loading = false;
                match result {
                    Ok((model, workspace_persistence)) => {
                        shell.focus_view = Some(cx.new(|cx| {
                            FocusView::new_with_workspace_persistence(
                                model,
                                workspace_persistence,
                                cx,
                            )
                        }));
                        shell.focus_error = None;
                    }
                    Err(error) => {
                        shell.focus_view = None;
                        shell.focus_error = Some(safe_focus_error(&error));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn retry_focus(&mut self, cx: &mut Context<Self>) {
        let Some(project_id) = self.focus_project_id.clone() else {
            return;
        };
        self.focus_error = None;
        self.focus_view = None;
        self.start_focus_open(project_id, cx);
    }

    fn route_button(
        &self,
        id: &'static str,
        label: &'static str,
        route: ProductRoute,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .label(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.bootstrap.state_mut().navigate(route.clone());
                cx.notify();
            }))
    }
}

async fn open_or_create_focus(
    store: Arc<EmbeddedLocalStore>,
    project_id: ProjectId,
    actor: Actor,
) -> Result<
    (
        FocusViewModel<LocalStoreDesignerPersistence>,
        Arc<dyn studio_design::WorkspacePersistence>,
    ),
    FocusOpenError,
> {
    let persistence =
        LocalStoreDesignerPersistence::new_shared(Arc::clone(&store)).map_err(|error| {
            FocusOpenError::Session(studio_design::SessionError::Persistence(error))
        })?;
    let workspace_persistence: Arc<dyn studio_design::WorkspacePersistence> =
        Arc::new(persistence.clone());
    match FocusViewModel::open(persistence.clone(), &project_id, None).await {
        Ok(model) => Ok((model, workspace_persistence)),
        Err(FocusOpenError::Session(studio_design::SessionError::NotFound(_))) => {
            let operation_id =
                OperationId::new(format!("create-{project_id}")).map_err(|error| {
                    FocusOpenError::Session(studio_design::SessionError::InvalidState(
                        error.to_string(),
                    ))
                })?;
            let undo_group_id =
                UndoGroupId::new(format!("create-{project_id}")).map_err(|error| {
                    FocusOpenError::Session(studio_design::SessionError::InvalidState(
                        error.to_string(),
                    ))
                })?;
            let session = DefaultDesignerSession::create(
                persistence,
                starter_design(project_id.clone()),
                operation_id,
                actor,
                undo_group_id,
            )
            .await
            .map_err(FocusOpenError::Session)?;
            Ok((
                FocusViewModel::from_session(session, None),
                workspace_persistence,
            ))
        }
        Err(error) => Err(error),
    }
}

fn starter_design(project_id: ProjectId) -> StudioDesign {
    let screen_id = ScreenId::new("home").expect("static screen identity is valid");
    let root_id = NodeId::new("canvas").expect("static node identity is valid");
    let headline_id = NodeId::new("headline").expect("static node identity is valid");
    let mut root = DesignNode::primitive(root_id.clone(), "Canvas", NodeKind::Box);
    root.children.push(headline_id.clone());
    root.properties.insert(
        crate::focus_view::CANVAS_RECT_PROPERTY.to_owned(),
        studio_design::CanvasRect::new(0.0, 0.0, 640.0, 480.0)
            .to_property_value()
            .expect("static canvas geometry is valid"),
    );
    let mut headline = DesignNode::primitive(headline_id.clone(), "Headline", NodeKind::Text);
    headline.properties.insert(
        "text".to_owned(),
        PropertyValue::String("Welcome to Studio Designer".to_owned()),
    );
    headline.properties.insert(
        crate::focus_view::CANVAS_RECT_PROPERTY.to_owned(),
        studio_design::CanvasRect::new(32.0, 32.0, 320.0, 48.0)
            .to_property_value()
            .expect("static canvas geometry is valid"),
    );
    let mut design = StudioDesign::empty(project_id, "Studio Designer project");
    design.nodes.insert(root_id.clone(), root);
    design.nodes.insert(headline_id.clone(), headline);
    design.parents.insert(
        root_id.clone(),
        NodeParent::Screen {
            screen_id: screen_id.clone(),
        },
    );
    design.parents.insert(
        headline_id,
        NodeParent::Node {
            node_id: root_id.clone(),
        },
    );
    design.screens.insert(
        screen_id.clone(),
        Screen {
            schema_version: studio_design::STUDIO_DESIGN_SCHEMA_VERSION,
            id: screen_id.clone(),
            name: "Home".to_owned(),
            route: "/home".to_owned(),
            root_node_id: root_id,
        },
    );
    design.screen_order.push(screen_id);
    design
}

fn safe_focus_error(error: &FocusOpenError) -> String {
    match error {
        FocusOpenError::Session(studio_design::SessionError::NotFound(_)) => {
            "The project has no durable Designer revision.".to_owned()
        }
        FocusOpenError::Session(studio_design::SessionError::InvalidState(_)) => {
            "The durable Designer revision failed validation.".to_owned()
        }
        FocusOpenError::Session(studio_design::SessionError::Persistence(error)) => {
            format!("Designer persistence is unavailable ({:?}).", error.code)
        }
    }
}

fn safe_identity_error(error: &IdentityError) -> String {
    match error.code() {
        studio_host::IdentityErrorCode::InvalidInput => {
            "Enter a display name and a non-empty matching password.".to_owned()
        }
        studio_host::IdentityErrorCode::WrongPassword => {
            "That password was not accepted. This identity is now locked; use Unlock to try again."
                .to_owned()
        }
        studio_host::IdentityErrorCode::Locked => {
            "This identity is locked. Enter its password to unlock it.".to_owned()
        }
        studio_host::IdentityErrorCode::NotFound => {
            "That identity is no longer available.".to_owned()
        }
        studio_host::IdentityErrorCode::CredentialUnavailable => {
            "The protected session could not be accessed. Try again.".to_owned()
        }
        studio_host::IdentityErrorCode::StoreUnavailable
        | studio_host::IdentityErrorCode::CatalogCorrupt
        | studio_host::IdentityErrorCode::EntropyUnavailable => {
            "Studio could not complete this identity operation. Try again later.".to_owned()
        }
    }
}

impl Render for NativeProductShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let route = self.bootstrap.state().route().clone();
        let indicator = self.bootstrap.state().indicator().clone();
        let route_is_unlock = matches!(&route, ProductRoute::Unlock { .. });
        self.ensure_identity_inputs(&route, window, cx);
        if matches!(&route, ProductRoute::Welcome) {
            return welcome_screen(
                cx.listener(|this, _, _, cx| {
                    let _ = this.bootstrap.dismiss_welcome();
                    cx.notify();
                }),
                cx.listener(|this, _, _, cx| {
                    let _ = this.bootstrap.open_local_identity();
                    cx.notify();
                }),
            )
            .into_any_element();
        }
        let mut content = div()
            .id("native-product-content")
            .flex()
            .flex_col()
            .gap_3()
            .p_6()
            .child(div().text_2xl().child(route.title()))
            .child(div().text_sm().child(format!("Route: {}", route.path())))
            .child(
                div()
                    .text_sm()
                    .child(format!("{} — {}", indicator.label, indicator.detail)),
            );

        match route {
            ProductRoute::Welcome => {}
            ProductRoute::IdentityChooser => {
                content = content.child("Choose a remembered identity or create a local identity.");
                let identities = self.bootstrap.state().identity().identities().to_vec();
                if identities.is_empty() {
                    content = content.child("No identities yet — create one to begin.");
                }
                for identity in identities {
                    let identity_id = identity.identity_id.clone();
                    let identity_route = match identity.state {
                        IdentityState::Available => ProductRoute::SignIn { identity_id },
                        IdentityState::Locked => ProductRoute::Unlock { identity_id },
                    };
                    content = content.child(
                        div()
                            .id(format!("identity-{}", identity.identity_id))
                            .child(div().text_lg().child(identity.display_name))
                            .child(div().text_sm().child(match identity.state {
                                IdentityState::Available => "Available",
                                IdentityState::Locked => "Locked — unlock required",
                            }))
                            .child(self.route_button(
                                "choose-identity",
                                "Continue",
                                identity_route,
                                cx,
                            )),
                    );
                }
                content = content.child(self.route_button(
                    "create-identity",
                    "Create identity",
                    ProductRoute::CreateIdentity,
                    cx,
                ));
            }
            ProductRoute::CreateIdentity => {
                if let (Some(name), Some(password), Some(confirmation)) = (
                    self.create_name.clone(),
                    self.create_password.clone(),
                    self.create_confirmation.clone(),
                ) {
                    content = content
                        .child(
                            "Create a local identity. Passwords stay in the host identity service.",
                        )
                        .child(Input::new(&name))
                        .child(Input::new(&password))
                        .child(Input::new(&confirmation));
                }
                if let Some(error) = self.identity_error.clone() {
                    content = content.child(
                        div()
                            .id("identity-error")
                            .role(gpui::Role::Alert)
                            .child(error),
                    );
                }
                content = content
                    .child(
                        Button::new("submit-create-identity")
                            .label("Create identity")
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| this.submit_create(cx))),
                    )
                    .child(self.route_button(
                        "back-identity",
                        "Back to identities",
                        ProductRoute::IdentityChooser,
                        cx,
                    ));
            }
            ProductRoute::SignIn { identity_id } | ProductRoute::Unlock { identity_id } => {
                let is_unlock = route_is_unlock;
                if let Some(password) = self.gate_password.clone() {
                    content = content
                        .child(if is_unlock {
                            "Unlock this identity."
                        } else {
                            "Sign in with your password."
                        })
                        .child(Input::new(&password));
                }
                content = content.child(
                    Button::new("remember-session")
                        .label(if self.remember_session {
                            "Remember this session: on"
                        } else {
                            "Remember this session: off"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.remember_session = !this.remember_session;
                            cx.notify();
                        })),
                );
                if let Some(error) = self.identity_error.clone() {
                    content = content.child(
                        div()
                            .id("identity-error")
                            .role(gpui::Role::Alert)
                            .child(error),
                    );
                }
                let identity_id_for_submit = identity_id.clone();
                content = content
                    .child(
                        Button::new("submit-identity-gate")
                            .label(if is_unlock { "Unlock" } else { "Sign in" })
                            .primary()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.submit_gate(identity_id_for_submit.clone(), is_unlock, cx)
                            })),
                    )
                    .child(self.route_button(
                        "back-identity",
                        "Back to identities",
                        ProductRoute::IdentityChooser,
                        cx,
                    ));
            }
            ProductRoute::Project { project_id } => {
                if self.focus_project_id.as_deref() != Some(project_id.as_str()) {
                    self.focus_project_id = Some(project_id.clone());
                    self.focus_view = None;
                    self.focus_error = None;
                    self.focus_loading = false;
                }
                if self.focus_view.is_none() && self.focus_error.is_none() && !self.focus_loading {
                    self.start_focus_open(project_id.clone(), cx);
                }
                if let Some(project) = self.focus_view.clone() {
                    content = content.child(project);
                } else if self.focus_loading {
                    content = content.child(
                        div()
                            .id("designer-project-loading")
                            .role(gpui::Role::Status)
                            .child("Opening the durable Designer project…"),
                    );
                } else if let Some(error) = self.focus_error.clone() {
                    content = content
                        .child(
                            div()
                                .id("designer-project-error")
                                .role(gpui::Role::Alert)
                                .child("Project editor unavailable")
                                .child(div().text_sm().child(error)),
                        )
                        .child(
                            Button::new("retry-project-open")
                                .label("Retry project open")
                                .primary()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.retry_focus(cx);
                                    cx.notify();
                                })),
                        );
                }
                content = content
                    .child(self.route_button(
                        "back-dashboard",
                        "Back to dashboard",
                        ProductRoute::Dashboard,
                        cx,
                    ))
                    .child(self.route_button(
                        "back-identity-from-project",
                        "Back to identity chooser",
                        ProductRoute::IdentityChooser,
                        cx,
                    ));
            }
            ProductRoute::Dashboard => {
                self.ensure_dashboard();
                let dashboard_snapshot = self.dashboard.as_ref().map(ProjectDashboard::snapshot);
                content = content
                    .child("Local and cached projects remain available when Cloud is offline.");
                if let Some(error) = self.dashboard_error.clone() {
                    content = content.child(
                        div()
                            .id("dashboard-error")
                            .role(gpui::Role::Alert)
                            .child(error),
                    );
                } else if let Some(snapshot) = dashboard_snapshot {
                    if snapshot.is_empty {
                        content = content.child("Your projects will appear here. Create your first project to get started.");
                    }
                    for project in snapshot.projects {
                        let project_id = project.id.clone();
                        let open_project_id = project_id.clone();
                        let delete_id = project.id.clone();
                        let project_name = project.name.clone();
                        let mut card = div()
                            .id(format!("project-{}", project.id))
                            .child(div().text_lg().child(project.name))
                            .child(div().text_sm().child(format!(
                                "{:?} · {:?}",
                                project.authority, project.sync_state
                            )))
                            .child(
                                Button::new(format!("open-{}", project_id))
                                    .label("Open project")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if this.dashboard.as_mut().is_some_and(|dashboard| {
                                            dashboard
                                                .state()
                                                .projects
                                                .contains_key(&open_project_id)
                                        }) {
                                            let _ = this
                                                .bootstrap
                                                .state_mut()
                                                .open_project(open_project_id.clone());
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new(format!("delete-{}", delete_id))
                                    .label("Delete…")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(dashboard) = this.dashboard.as_ref() {
                                            if let Ok(preview) =
                                                dashboard.delete_preview(&delete_id)
                                            {
                                                this.pending_delete =
                                                    Some((delete_id.clone(), preview));
                                            }
                                        }
                                        cx.notify();
                                    })),
                            );
                        if let Some((pending_id, consequences)) = &self.pending_delete {
                            if pending_id == &project_id {
                                let lines = consequences.lines();
                                card = card
                                    .child(
                                        div()
                                            .id(format!("delete-preview-{}", pending_id))
                                            .role(gpui::Role::Alert)
                                            .child(lines.join(" ")),
                                    )
                                    .child(
                                        Button::new(format!("confirm-delete-{}", pending_id))
                                            .label(format!("Confirm delete {}", project_name))
                                            .primary()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let Some((id, consequences)) =
                                                    this.pending_delete.take()
                                                {
                                                    if let Some(dashboard) = this.dashboard.as_mut()
                                                    {
                                                        let confirmation =
                                                            DeleteConfirmation::acknowledge(
                                                                id.clone(),
                                                                consequences,
                                                            );
                                                        let _ = dashboard.delete_project(
                                                            &id,
                                                            Some(confirmation),
                                                        );
                                                    }
                                                }
                                                cx.notify();
                                            })),
                                    );
                            }
                        }
                        content = content.child(card);
                    }
                }
                content = content
                    .child(
                        Button::new("create-project")
                            .label("Create project")
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let _ = this.create_project();
                                cx.notify();
                            })),
                    )
                    .child(self.route_button("settings", "Settings", ProductRoute::Settings, cx))
                    .child(self.route_button("help", "Help", ProductRoute::Help, cx))
                    .child(self.route_button("sync", "Sync status", ProductRoute::SyncStatus, cx))
                    .child(self.route_button("conflicts", "Conflicts", ProductRoute::Conflicts, cx))
                    .child(self.route_button("recovery", "Recovery", ProductRoute::Recovery, cx))
                    .child(
                        Button::new("sign-out")
                            .label("Sign out")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let _ = this.bootstrap.sign_out();
                                this.dashboard = None;
                                this.input_route = None;
                                cx.notify();
                            })),
                    );
            }
            ProductRoute::Settings => {
                self.ensure_settings();
                content = content.child("Global settings are persisted locally on this device.");
                if let Some(error) = self.settings_error.clone() {
                    content = content.child(
                        div()
                            .id("settings-error")
                            .role(gpui::Role::Alert)
                            .child(error),
                    );
                } else if let Some(settings) = self.settings.as_ref() {
                    let global = settings.global().clone();
                    content = content
                        .child(format!("Theme: {:?} · Language: {:?}", global.theme, global.language))
                        .child(Button::new("toggle-reduced-motion").label(format!("Reduced motion: {}", if global.reduced_motion { "on" } else { "off" })).on_click(cx.listener(|this, _, _, cx| {
                            if let Some(settings) = this.settings.as_mut() {
                                let value = !settings.global().reduced_motion;
                                let _ = settings.update_global(GlobalSettingChange::ReducedMotion(value));
                            }
                            cx.notify();
                        })))
                        .child("Theme, language, and transport-specific settings are available through their owning adapters.");
                }
                content = content.child(self.route_button(
                    "back-dashboard",
                    "Back to dashboard",
                    ProductRoute::Dashboard,
                    cx,
                ));
            }
            _ => {
                content = content
                    .child(self.route_button("dashboard", "Dashboard", ProductRoute::Dashboard, cx))
                    .child(self.route_button("settings", "Settings", ProductRoute::Settings, cx))
                    .child(self.route_button("help", "Help", ProductRoute::Help, cx))
                    .child(self.route_button("about", "About", ProductRoute::About, cx))
                    .child(self.route_button("sync", "Sync status", ProductRoute::SyncStatus, cx))
                    .child(self.route_button("conflicts", "Conflicts", ProductRoute::Conflicts, cx))
                    .child(self.route_button("recovery", "Recovery", ProductRoute::Recovery, cx))
                    .when(self.bootstrap.state().is_authenticated(), |element| {
                        element.child(Button::new("sign-out").label("Sign out").on_click(
                            cx.listener(|this, _, _, cx| {
                                let _ = this.bootstrap.sign_out();
                                cx.notify();
                            }),
                        ))
                    });
            }
        }

        content.into_any_element()
    }
}
