//! Ordered terminal compositor-loss shutdown.

use studio_actions::PendingActionSet;
use studio_security::{OpaqueHandle, PluginPrincipal, SecretPurpose, SecretRegistry};

/// Auditable shutdown ordering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownStep {
    /// Cancel non-terminal actions first.
    ActionsCancelled,
    /// Revoke and zeroize host-private secrets.
    SecretsRevoked,
    /// Permanently stop guest execution.
    InstanceTerminated,
    /// Drop retained UI and navigation state.
    NativeStateClosed,
    /// Request host process exit without restoration.
    ProcessExitRequested,
}

/// Immutable compositor-loss cleanup report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShutdownReport {
    steps: Vec<ShutdownStep>,
    cancelled_actions: usize,
    revoked_secrets: usize,
}

impl ShutdownReport {
    /// Exact cleanup order.
    #[must_use]
    pub fn steps(&self) -> &[ShutdownStep] {
        &self.steps
    }
    /// Number of cancelled requests.
    #[must_use]
    pub const fn cancelled_actions(&self) -> usize {
        self.cancelled_actions
    }
    /// Number of revoked references.
    #[must_use]
    pub const fn revoked_secrets(&self) -> usize {
        self.revoked_secrets
    }
}

/// Host resource owner that treats compositor loss as terminal.
pub struct ShutdownCoordinator {
    principal: PluginPrincipal,
    actions: PendingActionSet,
    secrets: SecretRegistry,
    instance_running: bool,
    native_state_live: bool,
    exit_requested: bool,
}

impl ShutdownCoordinator {
    /// Create an idle coordinator for one exact principal.
    #[must_use]
    pub fn new(principal: PluginPrincipal) -> Self {
        Self {
            principal,
            actions: PendingActionSet::default(),
            secrets: SecretRegistry::new(),
            instance_running: false,
            native_state_live: false,
            exit_requested: false,
        }
    }
    /// Mark a sandbox instance active.
    pub fn start_instance(&mut self) {
        self.instance_running = true;
    }
    /// Mark retained native UI and navigation state active.
    pub fn mount_ui_and_navigation(&mut self) {
        self.native_state_live = true;
    }
    /// Begin a pending protected action.
    ///
    /// # Errors
    ///
    /// Rejects invalid or over-capacity identities.
    pub fn begin_payment(&mut self, request_id: &str) -> Result<(), studio_actions::PaymentError> {
        self.actions.begin(request_id)
    }
    /// Capture a live opaque host reference.
    ///
    /// # Errors
    ///
    /// Rejects invalid secret inputs or unavailable entropy.
    pub fn capture_secret(
        &mut self,
        purpose: SecretPurpose,
        session: &str,
        bytes: &[u8],
    ) -> Result<OpaqueHandle, studio_security::SecretError> {
        self.secrets
            .capture(self.principal.clone(), purpose, session, bytes)
    }
    /// Execute the fixed terminal cleanup sequence.
    pub fn compositor_lost(&mut self) -> ShutdownReport {
        let cancelled_actions = self.actions.cancel_all();
        let revoked_secrets = self.secrets.revoke_all();
        self.instance_running = false;
        self.native_state_live = false;
        self.exit_requested = true;
        ShutdownReport {
            steps: vec![
                ShutdownStep::ActionsCancelled,
                ShutdownStep::SecretsRevoked,
                ShutdownStep::InstanceTerminated,
                ShutdownStep::NativeStateClosed,
                ShutdownStep::ProcessExitRequested,
            ],
            cancelled_actions,
            revoked_secrets,
        }
    }
    /// Terminal compositor loss cannot restore a prior sandbox or native state.
    ///
    /// # Errors
    ///
    /// Always rejects restoration after exit was requested.
    pub fn restore(&self) -> Result<(), &'static str> {
        Err("compositor shutdown is terminal")
    }
    /// Whether any resource remains live.
    #[must_use]
    pub fn has_live_resources(&self) -> bool {
        self.instance_running
            || self.native_state_live
            || !self.actions.is_empty()
            || self.secrets.active_len() != 0
    }
    /// Whether process exit has been requested.
    #[must_use]
    pub const fn exit_requested(&self) -> bool {
        self.exit_requested
    }
}
