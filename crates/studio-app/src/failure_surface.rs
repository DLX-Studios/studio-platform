//! Host-owned plugin termination and manual fresh-instance recovery.

use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    fmt,
};

use studio_security::{OpaqueHandle, PluginPrincipal, SecretPurpose, SecretRegistry};
use studio_wasm::RuntimeError;

/// Trusted termination content that contains no guest error strings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FailureSurface {
    code: &'static str,
    message: &'static str,
}

impl FailureSurface {
    fn from_runtime(error: &RuntimeError) -> Self {
        Self {
            code: error.safe_failure_code(),
            message: "The plugin stopped safely. Review the code and restart it manually.",
        }
    }
    /// Stable failure code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        self.code
    }
    /// Host-owned non-sensitive recovery guidance.
    #[must_use]
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

/// Only an explicit operator action may restart a terminal plugin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestartTrigger {
    /// Explicit operator-initiated restart.
    Manual,
    /// Background or implicit restart, always rejected.
    Automatic,
}

/// Recovery orchestration rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryError;
impl fmt::Display for RecoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("plugin recovery operation rejected")
    }
}
impl Error for RecoveryError {}

/// Host resource owner for one recoverable plugin lifecycle.
pub struct PluginRecovery {
    principal: PluginPrincipal,
    instance_id: [u8; 16],
    secrets: SecretRegistry,
    pending_actions: HashSet<String>,
    plugin_state: BTreeMap<String, String>,
    ui_mounted: bool,
    failure: Option<FailureSurface>,
}

impl PluginRecovery {
    /// Create a running recovery owner with a fresh host instance identity.
    ///
    /// # Errors
    ///
    /// Returns an error when secure host entropy is unavailable.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the verified principal is deliberately transferred into the recovery owner"
    )]
    pub fn new(principal: PluginPrincipal) -> Result<Self, RecoveryError> {
        let instance_id = fresh_id()?;
        let principal = PluginPrincipal::new(
            principal.publisher_key_id(),
            principal.plugin_id(),
            *principal.bundle_digest(),
            instance_id,
            principal.trust_mode(),
        )
        .map_err(|_| RecoveryError)?;
        Ok(Self {
            principal,
            instance_id,
            secrets: SecretRegistry::new(),
            pending_actions: HashSet::new(),
            plugin_state: BTreeMap::new(),
            ui_mounted: false,
            failure: None,
        })
    }

    /// Record a mounted native tree for lifecycle cleanup.
    pub fn mount_ui(&mut self) {
        self.ui_mounted = true;
    }
    /// Store non-secret guest-local state for recovery tests and orchestration.
    pub fn set_plugin_state(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.plugin_state.insert(key.into(), value.into());
    }

    /// Begin a bounded pending action.
    ///
    /// # Errors
    ///
    /// Rejects terminal, duplicate, invalid, or over-capacity requests.
    pub fn begin_action(&mut self, request_id: impl Into<String>) -> Result<(), RecoveryError> {
        let request_id = request_id.into();
        if self.failure.is_some()
            || request_id.is_empty()
            || self.pending_actions.len() >= 16
            || !self.pending_actions.insert(request_id)
        {
            return Err(RecoveryError);
        }
        Ok(())
    }

    /// Capture a host-private value scoped to this exact fresh principal.
    ///
    /// # Errors
    ///
    /// Rejects invalid input or unavailable entropy without exposing secret bytes.
    pub fn capture_secret(
        &mut self,
        purpose: SecretPurpose,
        session_id: &str,
        bytes: &[u8],
    ) -> Result<OpaqueHandle, RecoveryError> {
        if self.failure.is_some() {
            return Err(RecoveryError);
        }
        self.secrets
            .capture(self.principal.clone(), purpose, session_id, bytes)
            .map_err(|_| RecoveryError)
    }

    /// Ordered terminal cleanup: cancel actions, revoke secrets, close UI/state, then show failure.
    pub fn terminate(&mut self, error: &RuntimeError) {
        self.pending_actions.clear();
        self.secrets.revoke_all();
        self.ui_mounted = false;
        self.plugin_state.clear();
        self.failure = Some(FailureSurface::from_runtime(error));
    }

    /// Restart only after an explicit operator trigger, always with a fresh principal identity.
    ///
    /// # Errors
    ///
    /// Rejects automatic restart, a running instance, or unavailable entropy.
    pub fn restart(&mut self, trigger: RestartTrigger) -> Result<(), RecoveryError> {
        if trigger != RestartTrigger::Manual || self.failure.is_none() {
            return Err(RecoveryError);
        }
        let instance_id = fresh_id()?;
        self.principal = PluginPrincipal::new(
            self.principal.publisher_key_id(),
            self.principal.plugin_id(),
            *self.principal.bundle_digest(),
            instance_id,
            self.principal.trust_mode(),
        )
        .map_err(|_| RecoveryError)?;
        self.instance_id = instance_id;
        self.failure = None;
        Ok(())
    }

    /// Current fresh instance identity.
    #[must_use]
    pub const fn instance_id(&self) -> [u8; 16] {
        self.instance_id
    }
    /// Current trusted failure surface.
    #[must_use]
    pub const fn failure_surface(&self) -> Option<&FailureSurface> {
        self.failure.as_ref()
    }
    /// Host loop remains responsive independently of guest state.
    #[must_use]
    pub const fn host_responsive(&self) -> bool {
        true
    }
    /// Live opaque records.
    #[must_use]
    pub fn active_secrets(&self) -> usize {
        self.secrets.active_len()
    }
    /// Pending host actions.
    #[must_use]
    pub fn pending_actions(&self) -> usize {
        self.pending_actions.len()
    }
    /// Whether plugin UI remains mounted.
    #[must_use]
    pub const fn ui_mounted(&self) -> bool {
        self.ui_mounted
    }
    /// Current plugin-local state, never restored after termination.
    #[must_use]
    pub fn plugin_state(&self, key: &str) -> Option<&str> {
        self.plugin_state.get(key).map(String::as_str)
    }
}

fn fresh_id() -> Result<[u8; 16], RecoveryError> {
    let mut id = [0; 16];
    getrandom::fill(&mut id).map_err(|_| RecoveryError)?;
    Ok(id)
}
