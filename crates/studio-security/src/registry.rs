//! Instance-scoped, expiring, single-use opaque secret registry.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use zeroize::Zeroizing;

use crate::{OpaqueHandle, PluginPrincipal, SecretError, SecretPurpose, secret::SecretRecord};

const DEFAULT_TTL: Duration = Duration::from_mins(2);

/// Host-owned registry for raw sensitive values.
pub struct SecretRegistry {
    records: HashMap<OpaqueHandle, SecretRecord>,
    cleared_count: usize,
}

impl SecretRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
            cleared_count: 0,
        }
    }

    /// Capture a secret using the host monotonic clock and default 120-second lifetime.
    ///
    /// # Errors
    ///
    /// Returns [`SecretError`] for malformed metadata or unavailable secure entropy.
    pub fn capture(
        &mut self,
        owner: PluginPrincipal,
        purpose: SecretPurpose,
        session_id: impl Into<String>,
        secret: &[u8],
    ) -> Result<OpaqueHandle, SecretError> {
        self.capture_at(owner, purpose, session_id, secret, Instant::now())
    }

    /// Capture a secret at an explicit monotonic instant for deterministic host testing.
    ///
    /// # Errors
    ///
    /// Returns [`SecretError`] for malformed metadata or unavailable secure entropy.
    pub fn capture_at(
        &mut self,
        owner: PluginPrincipal,
        purpose: SecretPurpose,
        session_id: impl Into<String>,
        secret: &[u8],
        now: Instant,
    ) -> Result<OpaqueHandle, SecretError> {
        let session_id = session_id.into();
        if secret.is_empty()
            || secret.len() > 4096
            || session_id.is_empty()
            || session_id.len() > 128
            || session_id.chars().any(char::is_control)
        {
            return Err(SecretError::capture_invalid());
        }
        let handle = loop {
            let mut bytes = [0_u8; 32];
            getrandom::fill(&mut bytes).map_err(|_| SecretError::entropy_unavailable())?;
            let candidate = OpaqueHandle(bytes);
            if !self.records.contains_key(&candidate) {
                break candidate;
            }
        };
        self.records.insert(
            handle.clone(),
            SecretRecord {
                bytes: Zeroizing::new(secret.to_vec()),
                owner,
                purpose,
                session_id,
                expires_at: now + DEFAULT_TTL,
            },
        );
        Ok(handle)
    }

    /// Resolve and consume a secret exactly once inside a bounded host callback.
    ///
    /// # Errors
    ///
    /// All absent, expired, reused, foreign-owner, wrong-purpose, and wrong-session cases return
    /// the same [`crate::SecretErrorCode::AuthorizationInvalid`] code.
    pub fn consume_at<T>(
        &mut self,
        handle: &OpaqueHandle,
        owner: &PluginPrincipal,
        purpose: SecretPurpose,
        session_id: &str,
        now: Instant,
        consume: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, SecretError> {
        let expired = self
            .records
            .get(handle)
            .is_some_and(|record| now >= record.expires_at);
        if expired {
            self.clear(handle);
            return Err(SecretError::authorization_invalid());
        }
        let valid = self.records.get(handle).is_some_and(|record| {
            &record.owner == owner && record.purpose == purpose && record.session_id == session_id
        });
        if !valid {
            return Err(SecretError::authorization_invalid());
        }
        let Some(record) = self.records.remove(handle) else {
            return Err(SecretError::authorization_invalid());
        };
        let result = consume(record.bytes.as_slice());
        self.cleared_count += 1;
        drop(record);
        Ok(result)
    }

    /// Revoke one reference without revealing whether it existed.
    pub fn revoke(&mut self, handle: &OpaqueHandle) {
        self.clear(handle);
    }

    /// Revoke every record owned by an exact principal and return the number cleared.
    pub fn revoke_owner(&mut self, owner: &PluginPrincipal) -> usize {
        let handles = self
            .records
            .iter()
            .filter(|(_, record)| &record.owner == owner)
            .map(|(handle, _)| handle.clone())
            .collect::<Vec<_>>();
        for handle in &handles {
            self.clear(handle);
        }
        handles.len()
    }

    /// Revoke every live record during terminal host cleanup.
    pub fn revoke_all(&mut self) -> usize {
        let count = self.records.len();
        self.records.clear();
        self.cleared_count = self.cleared_count.saturating_add(count);
        count
    }

    /// Number of currently live host-private records.
    #[must_use]
    pub fn active_len(&self) -> usize {
        self.records.len()
    }

    /// Number of records dropped through consume, expiry, or revocation.
    #[must_use]
    pub const fn cleared_count(&self) -> usize {
        self.cleared_count
    }

    fn clear(&mut self, handle: &OpaqueHandle) {
        if self.records.remove(handle).is_some() {
            self.cleared_count += 1;
        }
    }
}

impl Default for SecretRegistry {
    fn default() -> Self {
        Self::new()
    }
}
