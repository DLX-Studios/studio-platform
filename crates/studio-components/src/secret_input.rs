//! Studio-owned sensitive input state and safe readiness projection.

use std::{error::Error, fmt, time::Instant};

use studio_protocol::HostEvent;
use studio_security::{OpaqueHandle, PluginPrincipal, SecretPurpose, SecretRegistry};
use studio_ui::InstanceId;

use crate::events::secret_ready_event;

/// Stable host-owned secret-input failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretInputErrorCode {
    /// Native control ownership did not match the active instance.
    OwnerMismatch,
    /// The control declaration was malformed.
    InputInvalid,
    /// Authorization was absent, expired, reused, revoked, or incorrectly scoped.
    AuthorizationInvalid,
}

/// Non-sensitive secret-input failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretInputError {
    code: SecretInputErrorCode,
}

impl SecretInputError {
    const fn new(code: SecretInputErrorCode) -> Self {
        Self { code }
    }

    /// Stable code safe for host diagnostics.
    #[must_use]
    pub const fn code(&self) -> SecretInputErrorCode {
        self.code
    }
}

impl fmt::Display for SecretInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            SecretInputErrorCode::OwnerMismatch => "secret input owner mismatch",
            SecretInputErrorCode::InputInvalid => "secret input invalid",
            SecretInputErrorCode::AuthorizationInvalid => "authorization invalid",
        })
    }
}

impl Error for SecretInputError {}

/// Snapshot deliberately excludes raw bytes and the active opaque reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretInputSnapshot {
    /// Stable native node identity.
    pub node_id: String,
    /// Whether the host currently holds an authorization reference.
    pub ready: bool,
}

/// State controller for one Studio-owned sensitive native input.
#[derive(Debug)]
pub struct HostSecretInput {
    owner: InstanceId,
    principal: PluginPrincipal,
    node_id: String,
    purpose: SecretPurpose,
    session_id: String,
    handle: Option<OpaqueHandle>,
}

impl HostSecretInput {
    /// Create one empty host-owned sensitive input.
    ///
    /// # Errors
    ///
    /// Returns [`SecretInputErrorCode::InputInvalid`] for empty or oversized identifiers.
    pub fn new(
        owner: InstanceId,
        principal: PluginPrincipal,
        node_id: impl Into<String>,
        purpose: SecretPurpose,
        session_id: impl Into<String>,
    ) -> Result<Self, SecretInputError> {
        let node_id = node_id.into();
        let session_id = session_id.into();
        if !valid_id(&node_id) || !valid_id(&session_id) {
            return Err(SecretInputError::new(SecretInputErrorCode::InputInvalid));
        }
        Ok(Self {
            owner,
            principal,
            node_id,
            purpose,
            session_id,
            handle: None,
        })
    }

    /// Capture native input and emit only readiness metadata plus an opaque reference.
    ///
    /// # Errors
    ///
    /// Rejects foreign owners and invalid host capture state without including secret bytes.
    pub fn capture_at(
        &mut self,
        owner: &InstanceId,
        registry: &mut SecretRegistry,
        secret: &[u8],
        now: Instant,
    ) -> Result<HostEvent, SecretInputError> {
        self.check_owner(owner)?;
        if let Some(previous) = self.handle.take() {
            registry.revoke(&previous);
        }
        let handle = registry
            .capture_at(
                self.principal.clone(),
                self.purpose,
                self.session_id.clone(),
                secret,
                now,
            )
            .map_err(|_| SecretInputError::new(SecretInputErrorCode::InputInvalid))?;
        let event = secret_ready_event(&self.node_id, &handle.to_token());
        self.handle = Some(handle);
        Ok(event)
    }

    /// Resolve the current authorization exactly once for a host-owned operation.
    ///
    /// # Errors
    ///
    /// Returns one non-oracular authorization failure for all invalid reference states.
    pub fn consume_at<T>(
        &mut self,
        owner: &InstanceId,
        registry: &mut SecretRegistry,
        now: Instant,
        consume: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, SecretInputError> {
        self.check_owner(owner)?;
        let Some(handle) = self.handle.take() else {
            return Err(SecretInputError::new(
                SecretInputErrorCode::AuthorizationInvalid,
            ));
        };
        registry
            .consume_at(
                &handle,
                &self.principal,
                self.purpose,
                &self.session_id,
                now,
                consume,
            )
            .map_err(|_| SecretInputError::new(SecretInputErrorCode::AuthorizationInvalid))
    }

    /// Resolve a guest-returned opaque token against this control's exact host scope.
    ///
    /// # Errors
    ///
    /// Returns the same authorization failure for malformed, absent, foreign, expired, reused,
    /// wrong-purpose, and wrong-session references.
    pub fn consume_reference_at<T>(
        &mut self,
        owner: &InstanceId,
        registry: &mut SecretRegistry,
        token: &str,
        now: Instant,
        consume: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, SecretInputError> {
        self.check_owner(owner)?;
        let handle = OpaqueHandle::from_token(token)
            .map_err(|_| SecretInputError::new(SecretInputErrorCode::AuthorizationInvalid))?;
        let result = registry.consume_at(
            &handle,
            &self.principal,
            self.purpose,
            &self.session_id,
            now,
            consume,
        );
        if self.handle.as_ref() == Some(&handle) && (result.is_ok() || registry.active_len() == 0) {
            self.handle = None;
        }
        result.map_err(|_| SecretInputError::new(SecretInputErrorCode::AuthorizationInvalid))
    }

    /// Revoke the live reference when the control, interface, or instance is removed.
    pub fn teardown(&mut self, registry: &mut SecretRegistry) {
        if let Some(handle) = self.handle.take() {
            registry.revoke(&handle);
        }
    }

    /// Return a secret-free snapshot for native state inspection.
    #[must_use]
    pub fn snapshot(&self) -> SecretInputSnapshot {
        SecretInputSnapshot {
            node_id: self.node_id.clone(),
            ready: self.handle.is_some(),
        }
    }

    /// Exact immutable principal used for registry scoping.
    #[must_use]
    pub const fn principal(&self) -> &PluginPrincipal {
        &self.principal
    }

    fn check_owner(&self, owner: &InstanceId) -> Result<(), SecretInputError> {
        if owner == &self.owner {
            Ok(())
        } else {
            Err(SecretInputError::new(SecretInputErrorCode::OwnerMismatch))
        }
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
