//! Protected storage for opaque remembered identity-session tokens.
//!
//! The identity host is given a token only at session creation. Persistent
//! remembered tokens are sent to this abstraction, whose production adapter is
//! the operating-system credential facility. The identity catalog stores only
//! session metadata; it never stores token bytes.

use std::{collections::HashMap, sync::{Arc, Mutex}};

use sha2::{Digest, Sha256};

const CREDENTIAL_SERVICE: &str = "com.dlx-studios.studio.identity-sessions.v1";
const KEY_DOMAIN: &[u8] = b"studio.identity-session-key.v1";

/// Stable failure family for protected remembered-session storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionCredentialErrorCode {
    /// The credential facility could not read or write the exact session key.
    Unavailable,
    /// Session metadata used an invalid identity or session identifier.
    InvalidKey,
}

/// Safe remembered-session storage failure that never contains token material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SessionCredentialError {
    code: SessionCredentialErrorCode,
}

impl SessionCredentialError {
    const fn unavailable() -> Self {
        Self {
            code: SessionCredentialErrorCode::Unavailable,
        }
    }

    const fn invalid_key() -> Self {
        Self {
            code: SessionCredentialErrorCode::InvalidKey,
        }
    }

    /// Stable code suitable for host diagnostics.
    #[must_use]
    pub const fn code(self) -> SessionCredentialErrorCode {
        self.code
    }
}

impl std::fmt::Display for SessionCredentialError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.code {
            SessionCredentialErrorCode::Unavailable => "session credential facility unavailable",
            SessionCredentialErrorCode::InvalidKey => "session credential key invalid",
        })
    }
}

impl std::error::Error for SessionCredentialError {}

/// Host-only remembered-token vault used by the identity service.
pub trait SessionCredentialStore: Send + Sync {
    /// Store or replace token bytes under one identity/session pair.
    fn store(
        &self,
        identity_id: &str,
        session_id: &str,
        token: &[u8],
    ) -> Result<(), SessionCredentialError>;

    /// Check the exact token without returning token bytes to the caller.
    fn matches(
        &self,
        identity_id: &str,
        session_id: &str,
        token: &[u8],
    ) -> Result<bool, SessionCredentialError>;

    /// Load a token transiently inside the trusted host boundary.
    ///
    /// This method is intentionally available only to host implementations;
    /// the returned bytes are never part of an identity snapshot or guest API.
    fn load(
        &self,
        identity_id: &str,
        session_id: &str,
    ) -> Result<Option<Vec<u8>>, SessionCredentialError>;

    /// Revoke the exact identity/session credential. Missing entries succeed.
    fn revoke(
        &self,
        identity_id: &str,
        session_id: &str,
    ) -> Result<(), SessionCredentialError>;
}

/// Operating-system credential-facility adapter for remembered sessions.
#[derive(Clone, Copy, Debug, Default)]
pub struct OsSessionCredentialStore;

impl SessionCredentialStore for OsSessionCredentialStore {
    fn store(
        &self,
        identity_id: &str,
        session_id: &str,
        token: &[u8],
    ) -> Result<(), SessionCredentialError> {
        validate_key(identity_id, session_id, token)?;
        entry(identity_id, session_id)?
            .set_secret(token)
            .map_err(|_| SessionCredentialError::unavailable())
    }

    fn matches(
        &self,
        identity_id: &str,
        session_id: &str,
        token: &[u8],
    ) -> Result<bool, SessionCredentialError> {
        validate_key(identity_id, session_id, token)?;
        Ok(self
            .load(identity_id, session_id)?
            .is_some_and(|stored| constant_time_eq(&stored, token)))
    }

    fn load(
        &self,
        identity_id: &str,
        session_id: &str,
    ) -> Result<Option<Vec<u8>>, SessionCredentialError> {
        validate_key(identity_id, session_id, b"token")?;
        match entry(identity_id, session_id)?.get_secret() {
            Ok(stored) => Ok(Some(stored)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(SessionCredentialError::unavailable()),
        }
    }

    fn revoke(
        &self,
        identity_id: &str,
        session_id: &str,
    ) -> Result<(), SessionCredentialError> {
        validate_key(identity_id, session_id, b"token")?;
        match entry(identity_id, session_id)?
            .delete_credential()
        {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(SessionCredentialError::unavailable()),
        }
    }
}

/// Deterministic protected-token store for host tests and disposable hosts.
#[derive(Clone, Default)]
pub struct MemorySessionCredentialStore {
    records: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MemorySessionCredentialStore {
    /// Number of currently stored remembered tokens.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.lock().expect("session store lock is not poisoned").len()
    }
}

impl std::fmt::Debug for MemorySessionCredentialStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MemorySessionCredentialStore")
            .field("records", &self.len())
            .finish()
    }
}

impl SessionCredentialStore for MemorySessionCredentialStore {
    fn store(
        &self,
        identity_id: &str,
        session_id: &str,
        token: &[u8],
    ) -> Result<(), SessionCredentialError> {
        validate_key(identity_id, session_id, token)?;
        self.records
            .lock()
            .expect("session store lock is not poisoned")
            .insert(key(identity_id, session_id), token.to_vec());
        Ok(())
    }

    fn matches(
        &self,
        identity_id: &str,
        session_id: &str,
        token: &[u8],
    ) -> Result<bool, SessionCredentialError> {
        validate_key(identity_id, session_id, token)?;
        let matches = self
            .records
            .lock()
            .expect("session store lock is not poisoned")
            .get(&key(identity_id, session_id))
            .is_some_and(|stored| constant_time_eq(stored, token));
        Ok(matches)
    }

    fn load(
        &self,
        identity_id: &str,
        session_id: &str,
    ) -> Result<Option<Vec<u8>>, SessionCredentialError> {
        validate_key(identity_id, session_id, b"token")?;
        Ok(self
            .records
            .lock()
            .expect("session store lock is not poisoned")
            .get(&key(identity_id, session_id))
            .cloned())
    }

    fn revoke(
        &self,
        identity_id: &str,
        session_id: &str,
    ) -> Result<(), SessionCredentialError> {
        validate_key(identity_id, session_id, b"token")?;
        self.records
            .lock()
            .expect("session store lock is not poisoned")
            .remove(&key(identity_id, session_id));
        Ok(())
    }
}

fn entry(
    identity_id: &str,
    session_id: &str,
) -> Result<keyring::Entry, SessionCredentialError> {
    keyring::Entry::new(CREDENTIAL_SERVICE, &key(identity_id, session_id))
        .map_err(|_| SessionCredentialError::unavailable())
}

fn validate_key(
    identity_id: &str,
    session_id: &str,
    token: &[u8],
) -> Result<(), SessionCredentialError> {
    if identity_id.is_empty()
        || session_id.is_empty()
        || identity_id.len() > 128
        || session_id.len() > 128
        || identity_id.chars().any(char::is_control)
        || session_id.chars().any(char::is_control)
        || token.is_empty()
    {
        Err(SessionCredentialError::invalid_key())
    } else {
        Ok(())
    }
}

fn key(identity_id: &str, session_id: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(KEY_DOMAIN);
    digest.update(identity_id.as_bytes());
    digest.update([0]);
    digest.update(session_id.as_bytes());
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (&left, &right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_matches_and_revokes_without_exposing_records() {
        let store = MemorySessionCredentialStore::default();
        store.store("identity", "session", b"opaque-token").unwrap();
        assert!(store.matches("identity", "session", b"opaque-token").unwrap());
        assert!(!store.matches("identity", "session", b"wrong-token").unwrap());
        assert_eq!(store.len(), 1);
        store.revoke("identity", "session").unwrap();
        assert!(!store.matches("identity", "session", b"opaque-token").unwrap());
        assert_eq!(store.len(), 0);
        assert!(!format!("{store:?}").contains("opaque-token"));
    }
}
