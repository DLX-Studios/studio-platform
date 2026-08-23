//! Immutable identity for one verified plugin instance.

/// Whether a principal came from production trust verification or explicit developer mode.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TrustMode {
    /// The bundle signature resolved to an enabled provisioned publisher key.
    Production,
    /// The operator explicitly selected an unsigned local development bundle.
    Development,
}

/// Complete identity used for host authorization and opaque-reference scoping.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PluginPrincipal {
    publisher_key_id: String,
    plugin_id: String,
    bundle_digest: [u8; 32],
    instance_id: [u8; 16],
    trust_mode: TrustMode,
}

impl PluginPrincipal {
    /// Create an immutable principal from already verified bundle and runtime identities.
    ///
    /// # Errors
    ///
    /// Returns [`crate::SecurityError`] when either textual identity is empty, oversized, or
    /// contains control characters.
    pub fn new(
        publisher_key_id: impl Into<String>,
        plugin_id: impl Into<String>,
        bundle_digest: [u8; 32],
        instance_id: [u8; 16],
        trust_mode: TrustMode,
    ) -> Result<Self, crate::SecurityError> {
        let publisher_key_id = publisher_key_id.into();
        let plugin_id = plugin_id.into();
        if !valid_id(&publisher_key_id) || !valid_id(&plugin_id) {
            return Err(crate::SecurityError::request_invalid());
        }
        Ok(Self {
            publisher_key_id,
            plugin_id,
            bundle_digest,
            instance_id,
            trust_mode,
        })
    }

    /// Provisioned publisher key identity.
    #[must_use]
    pub fn publisher_key_id(&self) -> &str {
        &self.publisher_key_id
    }

    /// Manifest plugin identity.
    #[must_use]
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// Digest of the exact verified bundle.
    #[must_use]
    pub const fn bundle_digest(&self) -> &[u8; 32] {
        &self.bundle_digest
    }

    /// Fresh runtime instance identity.
    #[must_use]
    pub const fn instance_id(&self) -> &[u8; 16] {
        &self.instance_id
    }

    /// Trust mode associated with this launch.
    #[must_use]
    pub const fn trust_mode(&self) -> TrustMode {
        self.trust_mode
    }
}

fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
