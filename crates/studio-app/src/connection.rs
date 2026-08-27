//! Shared connection state used by authentication, the project dashboard, and the project shell.
//!
//! The production identity service and grilling issue 09 remain explicit integration gates; this
//! module intentionally supplies only injectable seams and deterministic local behavior.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

use crate::project_dashboard::ProjectAuthority;

/// The small, shared set of connection states rendered by every Studio shell surface.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    /// No network connection is available. Local and cached work remain usable.
    #[default]
    Offline,
    /// A host-owned service request is in flight.
    Connecting,
    /// The last host-owned service request completed successfully.
    Synced,
    /// A request failed in a way that can be retried while local work remains available.
    Warning,
    /// A request failed and requires an explicit recovery action.
    Error,
}

impl ConnectionState {
    /// Return the stable label shared by auth, dashboard, and project-shell indicators.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Offline => "Offline",
            Self::Connecting => "Connecting",
            Self::Synced => "Synced",
            Self::Warning => "Warning",
            Self::Error => "Error",
        }
    }

    /// Whether this state represents an available remote connection.
    #[must_use]
    pub const fn is_online(self) -> bool {
        matches!(self, Self::Synced | Self::Connecting)
    }

    /// Whether the shell should expose a retry or recovery action.
    #[must_use]
    pub const fn is_actionable(self) -> bool {
        matches!(self, Self::Warning | Self::Error)
    }
}

/// A shell-neutral connection indicator. All surfaces intentionally consume this exact record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionIndicator {
    /// Current shared state.
    pub state: ConnectionState,
    /// Stable, short status label suitable for an accessible badge.
    pub label: String,
    /// Safe explanation shown alongside the label.
    pub detail: String,
    /// Whether retrying is expected to be useful.
    pub retryable: bool,
}

impl ConnectionIndicator {
    /// Construct the standard indicator for one shared state.
    #[must_use]
    pub fn from_state(state: ConnectionState) -> Self {
        Self {
            state,
            label: state.label().to_owned(),
            detail: default_detail(state).to_owned(),
            retryable: matches!(state, ConnectionState::Connecting | ConnectionState::Warning),
        }
    }

    /// Construct a shared indicator with safe, caller-provided recovery detail.
    #[must_use]
    pub fn with_detail(state: ConnectionState, detail: impl Into<String>, retryable: bool) -> Self {
        Self {
            state,
            label: state.label().to_owned(),
            detail: detail.into(),
            retryable,
        }
    }

    /// Return the indicator used by a named shell surface.
    ///
    /// The surface parameter is deliberately not encoded into the result. This keeps the
    /// visible state and wording identical in authentication, dashboard, and project-shell UIs.
    #[must_use]
    pub fn for_surface(_: ConnectionSurface, state: ConnectionState) -> Self {
        Self::from_state(state)
    }
}

/// Named consumers of the shared connection indicator contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionSurface {
    /// Cloud registration, verification, and account recovery.
    Auth,
    /// The authenticated project catalog.
    Dashboard,
    /// An opened project's Designer shell.
    ProjectShell,
}

const fn default_detail(state: ConnectionState) -> &'static str {
    match state {
        ConnectionState::Offline => "Cloud features are unavailable; local and cached projects remain usable.",
        ConnectionState::Connecting => "Connecting to Studio Cloud.",
        ConnectionState::Synced => "Cloud data is current.",
        ConnectionState::Warning => "Cloud data may be out of date. Retry when the connection is available.",
        ConnectionState::Error => "Cloud data needs attention. Local work is preserved.",
    }
}

/// A request accepted by an identity adapter. Credential material deliberately does not cross
/// this boundary: the host UI hands credentials to the adapter-owned secure input path.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterRequest {
    /// Display name for the cloud identity.
    pub display_name: String,
    /// Email address used for verification.
    pub email: String,
}

/// Registration details returned by the identity boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationReceipt {
    /// Opaque identity key used to partition local state.
    pub identity_key: String,
    /// Address to which verification was sent.
    pub email: String,
}

/// A verification request. Codes are transient input and are never persisted in onboarding state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyRequest {
    /// Address being verified.
    pub email: String,
    /// One-time code supplied by the user.
    pub code: String,
}

/// Verified identity returned by the identity boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedIdentity {
    /// Opaque identity key used by the host.
    pub identity_key: String,
    /// Verified email address.
    pub email: String,
}

/// Personal workspace creation request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRequest {
    /// Verified identity that owns the workspace.
    pub identity_key: String,
    /// User-visible workspace name.
    pub name: String,
}

/// Workspace returned by the identity boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceReceipt {
    /// Opaque workspace key.
    pub workspace_id: String,
    /// User-visible workspace name.
    pub name: String,
}

/// Sanitized identity-service failure. Backend details and credentials must not be exposed here.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Error)]
#[error("{message}")]
#[serde(deny_unknown_fields)]
pub struct IdentityServiceError {
    /// Stable failure category.
    pub code: IdentityServiceErrorCode,
    /// Safe recovery guidance.
    pub message: String,
    /// Whether repeating the same operation can succeed later.
    pub retryable: bool,
}

/// Identity-service failure categories understood by the onboarding UX.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityServiceErrorCode {
    /// No network connection is available.
    Offline,
    /// The service is temporarily unavailable.
    Unavailable,
    /// The email is already registered.
    AlreadyRegistered,
    /// The verification code has expired.
    VerificationExpired,
    /// The verification code is not valid.
    VerificationInvalid,
    /// The request was rejected by a bounded service policy.
    Rejected,
    /// Workspace creation failed but can be retried.
    WorkspaceUnavailable,
}

/// Identity operations shown in recoverable failure actions.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityOperation {
    /// Register a new cloud identity.
    Register,
    /// Confirm the verification code.
    Verify,
    /// Send another verification email.
    ResendVerification,
    /// Create the personal workspace.
    CreateWorkspace,
}

/// A safe failure retained with the step that produced it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecoverableFailure {
    /// Operation that can be retried or corrected.
    pub operation: IdentityOperation,
    /// Stable service category.
    pub code: IdentityServiceErrorCode,
    /// Safe user-facing explanation.
    pub message: String,
    /// Whether the UI should offer retry.
    pub retryable: bool,
}

/// Ordered steps in cloud identity onboarding.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingStep {
    /// Collect the minimum profile fields.
    #[default]
    Register,
    /// Wait for email verification.
    VerifyEmail,
    /// Name the personal workspace.
    WorkspaceSetup,
    /// Enter the authenticated dashboard.
    Complete,
}

/// Renderer-safe onboarding snapshot shared by native surfaces.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OnboardingSnapshot {
    /// Current ordered step.
    pub step: OnboardingStep,
    /// Email captured during registration, when known.
    pub email: Option<String>,
    /// Verified identity, when verification completed.
    pub identity: Option<VerifiedIdentity>,
    /// Workspace, when setup completed.
    pub workspace: Option<WorkspaceReceipt>,
    /// Shared connection indicator consumed by auth/dashboard/project shell.
    pub indicator: ConnectionIndicator,
    /// Last safe failure, if an action needs recovery.
    pub failure: Option<RecoverableFailure>,
}

/// Injectable identity boundary used by the cloud onboarding state machine.
pub trait IdentityService {
    /// Register a profile and send its verification message.
    fn register(
        &mut self,
        request: &RegisterRequest,
    ) -> Result<RegistrationReceipt, IdentityServiceError>;
    /// Confirm a one-time verification code.
    fn verify(&mut self, request: &VerifyRequest) -> Result<VerifiedIdentity, IdentityServiceError>;
    /// Resend the verification message for a pending registration.
    fn resend_verification(&mut self, email: &str) -> Result<(), IdentityServiceError>;
    /// Create the verified identity's personal workspace.
    fn create_workspace(
        &mut self,
        request: &WorkspaceRequest,
    ) -> Result<WorkspaceReceipt, IdentityServiceError>;
}

#[derive(Clone, Debug)]
enum PendingIdentityAction {
    Register(RegisterRequest),
    Verify(VerifyRequest),
    ResendVerification(String),
    CreateWorkspace(WorkspaceRequest),
}

/// Errors raised by the host-owned onboarding controller.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum OnboardingError {
    /// A field was empty or exceeded the bounded UX contract.
    #[error("invalid onboarding input: {0}")]
    InvalidInput(&'static str),
    /// The action does not match the current onboarding step.
    #[error("onboarding action is not valid during {0:?}")]
    WrongStep(OnboardingStep),
    /// No failed action is available to retry.
    #[error("no onboarding action is available to retry")]
    NothingToRetry,
    /// The service rejected the operation with a safe error.
    #[error(transparent)]
    Service(#[from] IdentityServiceError),
}

/// Host-owned cloud identity onboarding with an injectable service boundary.
pub struct IdentityOnboarding<S> {
    service: S,
    snapshot: OnboardingSnapshot,
    pending: Option<PendingIdentityAction>,
}

impl<S: IdentityService> IdentityOnboarding<S> {
    /// Start at registration without making a network request.
    #[must_use]
    pub fn new(service: S) -> Self {
        Self {
            service,
            snapshot: OnboardingSnapshot {
                step: OnboardingStep::Register,
                email: None,
                identity: None,
                workspace: None,
                indicator: ConnectionIndicator::from_state(ConnectionState::Offline),
                failure: None,
            },
            pending: None,
        }
    }

    /// Read the renderer-safe current state.
    #[must_use]
    pub fn snapshot(&self) -> &OnboardingSnapshot {
        &self.snapshot
    }

    /// Register and transition to email verification.
    pub fn register(&mut self, request: RegisterRequest) -> Result<(), OnboardingError> {
        validate_input(&request.display_name, "display name")?;
        validate_input(&request.email, "email")?;
        if self.snapshot.step != OnboardingStep::Register {
            return Err(OnboardingError::WrongStep(self.snapshot.step));
        }
        self.pending = Some(PendingIdentityAction::Register(request));
        self.run_pending()
    }

    /// Verify the pending email and transition to personal workspace setup.
    pub fn verify(&mut self, code: impl Into<String>) -> Result<(), OnboardingError> {
        if self.snapshot.step != OnboardingStep::VerifyEmail {
            return Err(OnboardingError::WrongStep(self.snapshot.step));
        }
        let code = code.into();
        validate_input(&code, "verification code")?;
        let email = self
            .snapshot
            .email
            .clone()
            .ok_or(OnboardingError::WrongStep(self.snapshot.step))?;
        self.pending = Some(PendingIdentityAction::Verify(VerifyRequest { email, code }));
        self.run_pending()
    }

    /// Alias for [`Self::verify`] matching the confirmation label used by the UI.
    pub fn confirm_verification(&mut self, code: impl Into<String>) -> Result<(), OnboardingError> {
        self.verify(code)
    }

    /// Request another verification email while retaining the current step on failure.
    pub fn resend_verification(&mut self) -> Result<(), OnboardingError> {
        if self.snapshot.step != OnboardingStep::VerifyEmail {
            return Err(OnboardingError::WrongStep(self.snapshot.step));
        }
        let email = self
            .snapshot
            .email
            .clone()
            .ok_or(OnboardingError::WrongStep(self.snapshot.step))?;
        self.pending = Some(PendingIdentityAction::ResendVerification(email));
        self.run_pending()
    }

    /// Create the verified identity's personal workspace.
    pub fn create_workspace(&mut self, name: impl Into<String>) -> Result<(), OnboardingError> {
        if self.snapshot.step != OnboardingStep::WorkspaceSetup {
            return Err(OnboardingError::WrongStep(self.snapshot.step));
        }
        let name = name.into();
        validate_input(&name, "workspace name")?;
        let identity = self
            .snapshot
            .identity
            .as_ref()
            .ok_or(OnboardingError::WrongStep(self.snapshot.step))?;
        self.pending = Some(PendingIdentityAction::CreateWorkspace(WorkspaceRequest {
            identity_key: identity.identity_key.clone(),
            name,
        }));
        self.run_pending()
    }

    /// Alias for [`Self::create_workspace`] used by workspace setup surfaces.
    pub fn setup_workspace(&mut self, name: impl Into<String>) -> Result<(), OnboardingError> {
        self.create_workspace(name)
    }

    /// Retry the last service operation after a recoverable failure.
    pub fn retry(&mut self) -> Result<(), OnboardingError> {
        if !self
            .snapshot
            .failure
            .as_ref()
            .is_some_and(|failure| failure.retryable)
        {
            return Err(OnboardingError::NothingToRetry);
        }
        if self.pending.is_none() {
            return Err(OnboardingError::NothingToRetry);
        }
        self.run_pending()
    }

    fn run_pending(&mut self) -> Result<(), OnboardingError> {
        let Some(action) = self.pending.clone() else {
            return Err(OnboardingError::NothingToRetry);
        };
        self.snapshot.indicator = ConnectionIndicator::from_state(ConnectionState::Connecting);
        self.snapshot.failure = None;
        let result = match &action {
            PendingIdentityAction::Register(request) => self
                .service
                .register(request)
                .map(|receipt| {
                    self.snapshot.email = Some(receipt.email);
                    self.snapshot.step = OnboardingStep::VerifyEmail;
                }),
            PendingIdentityAction::Verify(request) => self.service.verify(request).map(|identity| {
                self.snapshot.email = Some(identity.email.clone());
                self.snapshot.identity = Some(identity);
                self.snapshot.step = OnboardingStep::WorkspaceSetup;
            }),
            PendingIdentityAction::ResendVerification(email) => self
                .service
                .resend_verification(email)
                .map(|()| ()),
            PendingIdentityAction::CreateWorkspace(request) => self
                .service
                .create_workspace(request)
                .map(|workspace| {
                    self.snapshot.workspace = Some(workspace);
                    self.snapshot.step = OnboardingStep::Complete;
                }),
        };
        match result {
            Ok(()) => {
                self.pending = None;
                self.snapshot.failure = None;
                self.snapshot.indicator = ConnectionIndicator::from_state(ConnectionState::Synced);
                Ok(())
            }
            Err(error) => {
                self.snapshot.indicator = indicator_for_identity_error(&error);
                self.snapshot.failure = Some(RecoverableFailure {
                    operation: match action {
                        PendingIdentityAction::Register(_) => IdentityOperation::Register,
                        PendingIdentityAction::Verify(_) => IdentityOperation::Verify,
                        PendingIdentityAction::ResendVerification(_) => {
                            IdentityOperation::ResendVerification
                        }
                        PendingIdentityAction::CreateWorkspace(_) => IdentityOperation::CreateWorkspace,
                    },
                    code: error.code,
                    message: error.message.clone(),
                    retryable: error.retryable,
                });
                Err(OnboardingError::Service(error))
            }
        }
    }
}

fn indicator_for_identity_error(error: &IdentityServiceError) -> ConnectionIndicator {
    let state = if error.code == IdentityServiceErrorCode::Offline {
        ConnectionState::Offline
    } else if error.retryable {
        ConnectionState::Warning
    } else {
        ConnectionState::Error
    };
    ConnectionIndicator::with_detail(state, error.message.clone(), error.retryable)
}

fn validate_input(value: &str, field: &'static str) -> Result<(), OnboardingError> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(OnboardingError::InvalidInput(field));
    }
    Ok(())
}

/// Cached project retained by the host after cloud sign-out or loss of connectivity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CachedProject {
    /// Stable project key.
    pub id: String,
    /// Safe display name.
    pub name: String,
    /// Local or cloud authority.
    pub authority: ProjectAuthority,
    /// Last locally materialized revision.
    pub local_revision: u64,
    /// Number of local operations not yet transferred.
    pub pending_operations: u32,
    /// Whether the local cache can be opened and edited without cloud access.
    pub usable_offline: bool,
    /// Host-visible transfer state.
    pub sync_state: CachedProjectSyncState,
}

impl CachedProject {
    /// Construct a cloud project whose cached local copy remains editable offline.
    #[must_use]
    pub fn cloud(
        id: impl Into<String>,
        name: impl Into<String>,
        local_revision: u64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            authority: ProjectAuthority::Cloud,
            local_revision,
            pending_operations: 0,
            usable_offline: true,
            sync_state: CachedProjectSyncState::Synced,
        }
    }
}

/// Transfer state for one cached project.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CachedProjectSyncState {
    /// Device-owned project with no cloud transfer.
    Local,
    /// Local cache matches the last known cloud revision.
    #[default]
    Synced,
    /// Transfer is in progress.
    Syncing,
    /// Cloud is unavailable while local work remains usable.
    Offline,
    /// Transfer failed and can be retried.
    Warning,
}

/// Safe result returned when opening a cached project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachedProjectSession {
    /// Project key.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Authority remains local or cloud-owned even while offline.
    pub authority: ProjectAuthority,
    /// Cached revision opened by the Designer.
    pub local_revision: u64,
    /// Whether edits can be accepted locally.
    pub editable: bool,
}

/// Error returned by an injected synchronization worker.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Error)]
#[error("{message}")]
#[serde(deny_unknown_fields)]
pub struct SyncWorkerError {
    /// Stable failure category.
    pub code: SyncWorkerErrorCode,
    /// Safe recovery guidance.
    pub message: String,
    /// Whether a subsequent transfer may succeed.
    pub retryable: bool,
}

/// Synchronization worker failure categories.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncWorkerErrorCode {
    /// No network connection is available.
    Offline,
    /// The cloud endpoint is temporarily unavailable.
    Unavailable,
    /// Remote state requires explicit conflict recovery.
    Conflict,
    /// The identity cannot transfer this project.
    Unauthorized,
}

/// Input and result seam for host-owned project synchronization.
pub trait SyncWorker {
    /// Transfer one cached project's admitted local operations.
    fn sync(
        &mut self,
        project: &CachedProject,
    ) -> Result<SyncReceipt, SyncWorkerError>;
}

/// Safe worker receipt; operation bodies never cross the seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyncReceipt {
    /// Number of local operations accepted by the worker.
    pub transferred_operations: u32,
    /// New remote revision, when one was produced.
    pub remote_revision: u64,
}

/// Result of one coordinator transfer attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncOutcome {
    /// Sync was disabled and local pending operations were retained.
    Disabled,
    /// Cloud access is unavailable; cached editing remains available.
    Offline,
    /// The identity is signed out; the cache remains available.
    SignedOut,
    /// There was nothing to transfer.
    NoWork,
    /// The worker accepted the pending operations.
    Transferred(SyncReceipt),
}

/// Synchronization coordinator that keeps local authority independent from cloud transfer.
pub struct SyncCoordinator<W> {
    worker: W,
    projects: BTreeMap<String, CachedProject>,
    enabled: bool,
    connected: bool,
    signed_in: bool,
    indicator: ConnectionIndicator,
}

impl<W: SyncWorker> SyncCoordinator<W> {
    /// Create an enabled coordinator with no cloud request in flight.
    #[must_use]
    pub fn new(worker: W) -> Self {
        Self {
            worker,
            projects: BTreeMap::new(),
            enabled: true,
            connected: true,
            signed_in: true,
            indicator: ConnectionIndicator::from_state(ConnectionState::Synced),
        }
    }

    /// Add a cached project without contacting the worker.
    pub fn add_cached_project(&mut self, project: CachedProject) -> Result<(), SyncError> {
        validate_cached_input(&project.id, "project id")?;
        validate_cached_input(&project.name, "project name")?;
        if self.projects.contains_key(&project.id) {
            return Err(SyncError::DuplicateProject);
        }
        self.projects.insert(project.id.clone(), project);
        Ok(())
    }

    /// Return a deterministic copy of the cached catalog.
    #[must_use]
    pub fn projects(&self) -> Vec<CachedProject> {
        self.projects.values().cloned().collect()
    }

    /// Queue admitted local work while preserving local authority.
    pub fn queue_operations(&mut self, project_id: &str, count: u32) -> Result<(), SyncError> {
        let project = self
            .projects
            .get_mut(project_id)
            .ok_or_else(|| SyncError::ProjectNotFound(project_id.to_owned()))?;
        project.pending_operations = project.pending_operations.saturating_add(count);
        if project.authority == ProjectAuthority::Local {
            project.sync_state = CachedProjectSyncState::Local;
        } else if self.connected {
            project.sync_state = CachedProjectSyncState::Syncing;
        } else {
            project.sync_state = CachedProjectSyncState::Offline;
        }
        Ok(())
    }

    /// Enable transfers again without changing pending local operations.
    pub fn enable_sync(&mut self) {
        self.enabled = true;
        self.update_indicator();
    }

    /// Short alias for [`Self::enable_sync`] used by shell command dispatch.
    pub fn enable(&mut self) {
        self.enable_sync();
    }

    /// Disable transfers reversibly; pending operations and cached projects remain untouched.
    pub fn disable_sync(&mut self) {
        self.enabled = false;
        self.update_indicator();
    }

    /// Short alias for [`Self::disable_sync`] used by shell command dispatch.
    pub fn disable(&mut self) {
        self.disable_sync();
    }

    /// Whether transfer is currently enabled.
    #[must_use]
    pub const fn sync_enabled(&self) -> bool {
        self.enabled
    }

    /// Change connectivity without discarding cache or local operations.
    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
        for project in self.projects.values_mut() {
            if project.authority == ProjectAuthority::Cloud && project.pending_operations > 0 {
                project.sync_state = if connected {
                    CachedProjectSyncState::Syncing
                } else {
                    CachedProjectSyncState::Offline
                };
            }
        }
        self.update_indicator();
    }

    /// Sign out of cloud services while retaining every cached project for offline editing.
    pub fn sign_out(&mut self) {
        self.signed_in = false;
        self.set_connected(false);
    }

    /// Restore cloud authorization after a verified identity signs in.
    pub fn sign_in(&mut self) {
        self.signed_in = true;
        self.set_connected(true);
    }

    /// Return the shared indicator used by all shell surfaces.
    #[must_use]
    pub fn indicator(&self) -> &ConnectionIndicator {
        &self.indicator
    }

    /// Return an identical indicator for auth, dashboard, or project-shell rendering.
    #[must_use]
    pub fn indicator_for(&self, _surface: ConnectionSurface) -> ConnectionIndicator {
        self.indicator.clone()
    }

    /// Open any usable cached project, including after sign-out or offline transition.
    pub fn open_cached_project(
        &self,
        project_id: &str,
    ) -> Result<CachedProjectSession, SyncError> {
        let project = self
            .projects
            .get(project_id)
            .ok_or_else(|| SyncError::ProjectNotFound(project_id.to_owned()))?;
        if !project.usable_offline {
            return Err(SyncError::CacheUnavailable(project_id.to_owned()));
        }
        Ok(CachedProjectSession {
            id: project.id.clone(),
            name: project.name.clone(),
            authority: project.authority,
            local_revision: project.local_revision,
            editable: true,
        })
    }

    /// Attempt one transfer while preserving pending operations on every failure.
    pub fn sync_project(&mut self, project_id: &str) -> Result<SyncOutcome, SyncError> {
        if !self.enabled {
            return Ok(SyncOutcome::Disabled);
        }
        if !self.signed_in {
            return Ok(SyncOutcome::SignedOut);
        }
        if !self.connected {
            self.indicator = ConnectionIndicator::from_state(ConnectionState::Offline);
            return Ok(SyncOutcome::Offline);
        }
        let project = self
            .projects
            .get(project_id)
            .cloned()
            .ok_or_else(|| SyncError::ProjectNotFound(project_id.to_owned()))?;
        if project.pending_operations == 0 {
            return Ok(SyncOutcome::NoWork);
        }
        if project.authority == ProjectAuthority::Local {
            return Ok(SyncOutcome::NoWork);
        }
        if let Some(project) = self.projects.get_mut(project_id) {
            project.sync_state = CachedProjectSyncState::Syncing;
        }
        self.indicator = ConnectionIndicator::from_state(ConnectionState::Connecting);
        match self.worker.sync(&project) {
            Ok(receipt) => {
                if let Some(project) = self.projects.get_mut(project_id) {
                    project.pending_operations = project
                        .pending_operations
                        .saturating_sub(receipt.transferred_operations);
                    project.local_revision = project.local_revision.max(receipt.remote_revision);
                    project.sync_state = if project.pending_operations == 0 {
                        CachedProjectSyncState::Synced
                    } else {
                        CachedProjectSyncState::Syncing
                    };
                }
                self.indicator = ConnectionIndicator::from_state(ConnectionState::Synced);
                Ok(SyncOutcome::Transferred(receipt))
            }
            Err(error) => {
                if let Some(project) = self.projects.get_mut(project_id) {
                    project.sync_state = if error.code == SyncWorkerErrorCode::Offline {
                        CachedProjectSyncState::Offline
                    } else {
                        CachedProjectSyncState::Warning
                    };
                }
                let state = if error.code == SyncWorkerErrorCode::Offline {
                    ConnectionState::Offline
                } else if error.retryable {
                    ConnectionState::Warning
                } else {
                    ConnectionState::Error
                };
                self.indicator = ConnectionIndicator::with_detail(
                    state,
                    error.message.clone(),
                    error.retryable,
                );
                Err(SyncError::Worker(error))
            }
        }
    }

    /// Short alias for [`Self::sync_project`] used by a project-shell sync action.
    pub fn sync(&mut self, project_id: &str) -> Result<SyncOutcome, SyncError> {
        self.sync_project(project_id)
    }

    fn update_indicator(&mut self) {
        self.indicator = if !self.connected || !self.signed_in {
            ConnectionIndicator::from_state(ConnectionState::Offline)
        } else if !self.enabled {
            ConnectionIndicator::with_detail(
                ConnectionState::Warning,
                "Synchronization is paused. Local changes remain available and will resume when enabled.",
                true,
            )
        } else {
            ConnectionIndicator::from_state(ConnectionState::Synced)
        };
    }
}

    /// Synchronization coordinator failures that preserve local cache state.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SyncError {
    /// A cached project key is invalid.
    #[error("invalid cached project field: {0}")]
    InvalidInput(&'static str),
    /// A project key was already present.
    #[error("cached project already exists")]
    DuplicateProject,
    /// The requested project is absent from the local cache.
    #[error("cached project not found: {0}")]
    ProjectNotFound(String),
    /// The local cache cannot open this project offline.
    #[error("cached project is unavailable offline: {0}")]
    CacheUnavailable(String),
    /// The injected worker rejected the transfer.
    #[error(transparent)]
    Worker(#[from] SyncWorkerError),
}

fn validate_cached_input(value: &str, field: &'static str) -> Result<(), SyncError> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(SyncError::InvalidInput(field));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeIdentityService {
        fail: Option<(IdentityOperation, IdentityServiceError)>,
    }

    impl FakeIdentityService {
        fn success() -> Self {
            Self {
                fail: None,
            }
        }

        fn fail_once(operation: IdentityOperation, code: IdentityServiceErrorCode) -> Self {
            Self {
                fail: Some((
                    operation,
                    IdentityServiceError {
                        code,
                        message: "The test identity service is temporarily unavailable. Retry.".into(),
                        retryable: true,
                    },
                )),
            }
        }

        fn maybe_fail(&mut self, operation: IdentityOperation) -> Result<(), IdentityServiceError> {
            if self
                .fail
                .as_ref()
                .is_some_and(|(expected, _)| *expected == operation)
            {
                let (_, error) = self.fail.take().expect("failure was checked");
                return Err(error);
            }
            Ok(())
        }
    }

    impl IdentityService for FakeIdentityService {
        fn register(
            &mut self,
            request: &RegisterRequest,
        ) -> Result<RegistrationReceipt, IdentityServiceError> {
            self.maybe_fail(IdentityOperation::Register)?;
            Ok(RegistrationReceipt {
                identity_key: format!("cloud:{}", request.email),
                email: request.email.clone(),
            })
        }

        fn verify(
            &mut self,
            request: &VerifyRequest,
        ) -> Result<VerifiedIdentity, IdentityServiceError> {
            self.maybe_fail(IdentityOperation::Verify)?;
            if request.code != "1234" {
                return Err(IdentityServiceError {
                    code: IdentityServiceErrorCode::VerificationInvalid,
                    message: "That verification code is not valid. Check the email and try again.".into(),
                    retryable: true,
                });
            }
            Ok(VerifiedIdentity {
                identity_key: format!("cloud:{}", request.email),
                email: request.email.clone(),
            })
        }

        fn resend_verification(&mut self, _: &str) -> Result<(), IdentityServiceError> {
            self.maybe_fail(IdentityOperation::ResendVerification)?;
            Ok(())
        }

        fn create_workspace(
            &mut self,
            request: &WorkspaceRequest,
        ) -> Result<WorkspaceReceipt, IdentityServiceError> {
            self.maybe_fail(IdentityOperation::CreateWorkspace)?;
            Ok(WorkspaceReceipt {
                workspace_id: format!("workspace:{}", request.identity_key),
                name: request.name.clone(),
            })
        }
    }

    #[derive(Default)]
    struct FakeSyncWorker {
        calls: Vec<String>,
        fail_once: bool,
    }

    impl SyncWorker for FakeSyncWorker {
        fn sync(
            &mut self,
            project: &CachedProject,
        ) -> Result<SyncReceipt, SyncWorkerError> {
            self.calls.push(project.id.clone());
            if self.fail_once {
                self.fail_once = false;
                return Err(SyncWorkerError {
                    code: SyncWorkerErrorCode::Unavailable,
                    message: "Cloud sync is unavailable. Local changes are preserved; retry.".into(),
                    retryable: true,
                });
            }
            Ok(SyncReceipt {
                transferred_operations: project.pending_operations,
                remote_revision: project.local_revision + project.pending_operations as u64,
            })
        }
    }

    #[test]
    fn fake_identity_service_drives_register_verify_resend_and_workspace_setup() {
        let mut onboarding = IdentityOnboarding::new(FakeIdentityService::success());
        onboarding
            .register(RegisterRequest {
                display_name: "Avery Morgan".into(),
                email: "avery@example.test".into(),
            })
            .unwrap();
        assert_eq!(onboarding.snapshot().step, OnboardingStep::VerifyEmail);
        assert_eq!(onboarding.snapshot().indicator.state, ConnectionState::Synced);

        onboarding.resend_verification().unwrap();
        onboarding.verify("1234").unwrap();
        assert_eq!(onboarding.snapshot().step, OnboardingStep::WorkspaceSetup);
        onboarding.create_workspace("Avery Studio").unwrap();

        let snapshot = onboarding.snapshot();
        assert_eq!(snapshot.step, OnboardingStep::Complete);
        assert_eq!(snapshot.workspace.as_ref().unwrap().name, "Avery Studio");
        assert!(snapshot.failure.is_none());
    }

    #[test]
    fn identity_failures_keep_the_step_and_retry_the_same_action() {
        let mut onboarding = IdentityOnboarding::new(FakeIdentityService::fail_once(
            IdentityOperation::Register,
            IdentityServiceErrorCode::Unavailable,
        ));
        let request = RegisterRequest {
            display_name: "Avery Morgan".into(),
            email: "avery@example.test".into(),
        };
        assert!(onboarding.register(request).is_err());
        assert_eq!(onboarding.snapshot().step, OnboardingStep::Register);
        assert_eq!(onboarding.snapshot().indicator.state, ConnectionState::Warning);
        assert_eq!(
            onboarding.snapshot().failure.as_ref().unwrap().operation,
            IdentityOperation::Register
        );
        onboarding.retry().unwrap();
        assert_eq!(onboarding.snapshot().step, OnboardingStep::VerifyEmail);
    }

    #[test]
    fn sync_disable_is_reversible_and_preserves_pending_local_work() {
        let mut coordinator = SyncCoordinator::new(FakeSyncWorker::default());
        coordinator.add_cached_project(CachedProject::cloud("northstar", "Northstar", 10)).unwrap();
        coordinator.queue_operations("northstar", 3).unwrap();
        coordinator.disable_sync();
        assert_eq!(coordinator.sync_project("northstar").unwrap(), SyncOutcome::Disabled);
        assert_eq!(coordinator.projects()[0].pending_operations, 3);

        coordinator.enable_sync();
        assert_eq!(
            coordinator.sync_project("northstar").unwrap(),
            SyncOutcome::Transferred(SyncReceipt {
                transferred_operations: 3,
                remote_revision: 13,
            })
        );
        assert_eq!(coordinator.projects()[0].pending_operations, 0);
    }

    #[test]
    fn offline_and_sign_out_keep_cached_projects_editable_and_indicators_consistent() {
        let mut coordinator = SyncCoordinator::new(FakeSyncWorker::default());
        coordinator.add_cached_project(CachedProject::cloud("cached", "Cached", 4)).unwrap();
        coordinator.queue_operations("cached", 1).unwrap();
        coordinator.set_connected(false);
        assert_eq!(coordinator.sync_project("cached").unwrap(), SyncOutcome::Offline);

        let session = coordinator.open_cached_project("cached").unwrap();
        assert!(session.editable);
        assert_eq!(session.authority, ProjectAuthority::Cloud);
        coordinator.sign_out();
        assert_eq!(coordinator.sync_project("cached").unwrap(), SyncOutcome::SignedOut);
        assert!(coordinator.open_cached_project("cached").unwrap().editable);

        let auth = coordinator.indicator_for(ConnectionSurface::Auth);
        let dashboard = coordinator.indicator_for(ConnectionSurface::Dashboard);
        let shell = coordinator.indicator_for(ConnectionSurface::ProjectShell);
        assert_eq!(auth, dashboard);
        assert_eq!(dashboard, shell);
        assert_eq!(shell.state, ConnectionState::Offline);
        assert!(shell.detail.contains("unavailable"));
    }

    #[test]
    fn worker_failure_is_recoverable_without_losing_local_operations() {
        let mut coordinator = SyncCoordinator::new(FakeSyncWorker {
            fail_once: true,
            ..FakeSyncWorker::default()
        });
        coordinator.add_cached_project(CachedProject::cloud("project", "Project", 7)).unwrap();
        coordinator.queue_operations("project", 2).unwrap();
        assert!(coordinator.sync_project("project").is_err());
        assert_eq!(coordinator.projects()[0].pending_operations, 2);
        assert_eq!(coordinator.indicator().state, ConnectionState::Warning);
        coordinator.sync_project("project").unwrap();
        assert_eq!(coordinator.projects()[0].pending_operations, 0);
    }
}
