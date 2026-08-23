//! Provisioned publisher trust keys with non-oracular lookup semantics.

use std::collections::BTreeMap;

use ed25519_dalek::VerifyingKey;

use crate::IntegrityError;

/// One host-provisioned publisher verification key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedPublisherKey {
    /// Stable publisher identity from the manifest.
    pub publisher_id: String,
    /// Publisher-scoped key identity from the manifest.
    pub key_id: String,
    /// Raw Ed25519 public key bytes.
    pub verifying_key: [u8; 32],
    /// Whether this key currently accepts bundles.
    pub enabled: bool,
}

/// Immutable trust-store snapshot used by bundle verification.
#[derive(Clone, Debug, Default)]
pub struct TrustStore {
    keys: BTreeMap<(String, String), TrustedPublisherKey>,
}

impl TrustStore {
    /// Construct a trust snapshot, rejecting ambiguous or malformed provisioned records.
    ///
    /// # Errors
    ///
    /// Returns [`IntegrityError::TrustConfigurationInvalid`] for empty identities, duplicate
    /// publisher/key pairs, or invalid Ed25519 public key encodings.
    pub fn from_keys(
        keys: impl IntoIterator<Item = TrustedPublisherKey>,
    ) -> Result<Self, IntegrityError> {
        let mut indexed = BTreeMap::new();
        for key in keys {
            if key.publisher_id.is_empty()
                || key.key_id.is_empty()
                || VerifyingKey::from_bytes(&key.verifying_key).is_err()
            {
                return Err(IntegrityError::TrustConfigurationInvalid);
            }
            let identity = (key.publisher_id.clone(), key.key_id.clone());
            if indexed.insert(identity, key).is_some() {
                return Err(IntegrityError::TrustConfigurationInvalid);
            }
        }
        Ok(Self { keys: indexed })
    }

    pub(crate) fn enabled_key(
        &self,
        publisher_id: &str,
        key_id: &str,
    ) -> Result<VerifyingKey, IntegrityError> {
        let key = self
            .keys
            .get(&(publisher_id.to_owned(), key_id.to_owned()))
            .filter(|key| key.enabled)
            .ok_or(IntegrityError::TrustInvalid)?;
        VerifyingKey::from_bytes(&key.verifying_key).map_err(|_| IntegrityError::TrustInvalid)
    }
}
