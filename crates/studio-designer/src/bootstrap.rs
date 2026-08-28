//! Production native product bootstrap and route composition.
//!
//! This module is the seam between the host-owned persistence/authentication
//! services and GPUI. It intentionally keeps route state renderer-neutral so
//! first-launch, remembered-session, offline, and recovery behavior can be
//! tested without starting a window.

#![allow(missing_docs)]
#![allow(clippy::all, clippy::pedantic, clippy::restriction, clippy::nursery)]

use std::{path::PathBuf, sync::Arc};

use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div,
};
use gpui_component::button::{Button, ButtonVariants};
use serde::{Serialize, de::DeserializeOwned};
use studio_design::{
    Actor, ActorId, ActorKind, DefaultDesignerSession, DesignNode, NodeId, NodeKind, NodeParent,
    OperationId, ProjectId, PropertyValue, Screen, ScreenId, StudioDesign, UndoGroupId,
};
use studio_host::{
    Durability, EmbeddedLocalStore, IdentityError, IdentityService, IdentitySession,
    IdentitySnapshot, IdentityState, LocalStoreDesignerPersistence, LocalStoreDiagnosticCode,
    LocalStoreError,
};
use studio_security::OsSessionCredentialStore;

use crate::connection::{
    CachedProject, ConnectionIndicator, ConnectionState, SyncCoordinator, SyncReceipt, SyncWorker,
    SyncWorkerError, SyncWorkerErrorCode,
};
use crate::focus_view::{FocusOpenError, FocusView, FocusViewModel};
use crate::project_dashboard::{
    DashboardError, DashboardPersistence as DashboardPersistenceTrait, DashboardPersistenceError,
    DashboardPersistenceErrorCode, DashboardState, ProjectDashboard,
};
use crate::settings::{GlobalSettings, ProjectSettings};
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
    identity_service: IdentityService<EmbeddedLocalStore, OsSessionCredentialStore>,
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
        let identity_service =
            IdentityService::with_credentials(Arc::clone(&store), OsSessionCredentialStore);
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
        }
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
                    Ok(model) => {
                        shell.focus_view = Some(cx.new(|cx| FocusView::new(model, cx)));
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
) -> Result<FocusViewModel<LocalStoreDesignerPersistence>, FocusOpenError> {
    let persistence =
        LocalStoreDesignerPersistence::new_shared(Arc::clone(&store)).map_err(|error| {
            FocusOpenError::Session(studio_design::SessionError::Persistence(error))
        })?;
    match FocusViewModel::open(persistence.clone(), &project_id, None).await {
        Ok(model) => Ok(model),
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
            Ok(FocusViewModel::from_session(session, None))
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

impl Render for NativeProductShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let route = self.bootstrap.state().route().clone();
        let indicator = self.bootstrap.state().indicator().clone();
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
            ProductRoute::Welcome => {
                content =
                    content
                        .child("Your local Studio projects stay available offline.")
                        .child(Button::new("dismiss-welcome").label("Continue").on_click(
                            cx.listener(|this, _, _, cx| {
                                let _ = this.bootstrap.dismiss_welcome();
                                cx.notify();
                            }),
                        ));
            }
            ProductRoute::IdentityChooser => {
                content = content.child("Choose a remembered identity or create a local identity.");
                if let Some(identity) = self.bootstrap.state().identity().identities().first() {
                    let identity_id = identity.identity_id.clone();
                    let route = match identity.state {
                        IdentityState::Available => ProductRoute::SignIn { identity_id },
                        IdentityState::Locked => ProductRoute::Unlock { identity_id },
                    };
                    content = content.child(self.route_button(
                        "choose-identity",
                        "Continue with this identity",
                        route,
                        cx,
                    ));
                }
                content = content.child(self.route_button(
                    "create-identity",
                    "Create identity",
                    ProductRoute::CreateIdentity,
                    cx,
                ));
            }
            ProductRoute::CreateIdentity => {
                content = content
                    .child("Identity creation is handled by the host-owned secure form.")
                    .child(self.route_button(
                        "back-identity",
                        "Back to identities",
                        ProductRoute::IdentityChooser,
                        cx,
                    ));
            }
            ProductRoute::SignIn { .. } | ProductRoute::Unlock { .. } => {
                content = content
                    .child("Credentials stay inside the host-owned secure input path.")
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
                content = content.child(self.route_button(
                    "back-dashboard",
                    "Back to dashboard",
                    ProductRoute::Dashboard,
                    cx,
                ));
            }
            ProductRoute::Dashboard => {
                content = content
                    .child("Local and cached projects remain available when Cloud is offline.")
                    .child(
                        Button::new("open-project")
                            .label("Open project")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let _ = this.bootstrap.state_mut().open_project("local-project");
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
                                cx.notify();
                            })),
                    );
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

        content
    }
}
