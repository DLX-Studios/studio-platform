//! Provisioned publisher trust snapshots with non-oracular lookup semantics.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::IntegrityError;

/// Environment variable containing the operator-provisioned trust snapshot path.
pub const TRUST_STORE_PATH_ENV: &str = "STUDIO_TRUST_STORE";

const TRUST_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

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

/// A publisher key record in an operator-provisioned trust snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProvisionedPublisherKey {
    /// Stable publisher identity from the bundle manifest.
    pub publisher_id: String,
    /// Publisher-scoped key identity from the bundle manifest.
    pub key_id: String,
    /// Lowercase or uppercase hex-encoded 32-byte Ed25519 public key.
    #[serde(alias = "verifyingKey")]
    pub public_key: String,
    /// Unix timestamp at which this key becomes valid.
    pub valid_from: u64,
    /// Exclusive Unix timestamp at which this key expires.
    pub expires_at: u64,
    /// Explicit operator disable switch for emergency containment.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

/// A publisher/key identity revoked by an operator.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublisherKeyIdentity {
    /// Stable publisher identity.
    pub publisher_id: String,
    /// Publisher-scoped key identity.
    pub key_id: String,
}

/// Versioned, time-bounded trust data provisioned by the release/operator channel.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustSnapshot {
    /// Schema version of this snapshot document.
    pub schema_version: u32,
    /// Opaque release evidence identity; never contains key material.
    pub snapshot_id: String,
    /// Monotonically increasing snapshot version.
    pub version: u64,
    /// Inclusive Unix timestamp at which this snapshot becomes valid.
    pub valid_from: u64,
    /// Exclusive Unix timestamp at which this snapshot expires.
    pub expires_at: u64,
    /// Publisher keys carried by this snapshot.
    pub keys: Vec<ProvisionedPublisherKey>,
    /// Explicitly revoked publisher/key identities.
    #[serde(default)]
    pub revocations: Vec<PublisherKeyIdentity>,
}

/// Stable failure family for loading an operator trust snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustStoreErrorCode {
    /// The configured snapshot path was not supplied or the file is absent.
    Missing,
    /// The snapshot could not be read from the protected configuration path.
    Unavailable,
    /// The snapshot schema or key records are malformed.
    Malformed,
    /// The snapshot is not active yet.
    NotYetValid,
    /// The snapshot has expired.
    Expired,
    /// The snapshot contains no currently usable key.
    NoActiveKeys,
}

/// Safe trust snapshot loading error. Raw paths and parser/provider details are never retained.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum TrustStoreError {
    /// The configured snapshot path was not supplied or the file is absent.
    #[error("publisher trust configuration is missing")]
    Missing,
    /// The snapshot could not be read from the protected configuration path.
    #[error("publisher trust configuration is unavailable")]
    Unavailable,
    /// The snapshot schema or key records are malformed.
    #[error("publisher trust configuration is malformed")]
    Malformed,
    /// The snapshot is not active yet.
    #[error("publisher trust configuration is not active yet")]
    NotYetValid,
    /// The snapshot has expired.
    #[error("publisher trust configuration has expired")]
    Expired,
    /// The snapshot contains no currently usable key.
    #[error("publisher trust configuration has no active publisher keys")]
    NoActiveKeys,
}

impl TrustStoreError {
    /// Return the stable failure family.
    #[must_use]
    pub const fn code(self) -> TrustStoreErrorCode {
        match self {
            Self::Missing => TrustStoreErrorCode::Missing,
            Self::Unavailable => TrustStoreErrorCode::Unavailable,
            Self::Malformed => TrustStoreErrorCode::Malformed,
            Self::NotYetValid => TrustStoreErrorCode::NotYetValid,
            Self::Expired => TrustStoreErrorCode::Expired,
            Self::NoActiveKeys => TrustStoreErrorCode::NoActiveKeys,
        }
    }
}

/// Immutable trust-store snapshot used by bundle verification.
#[derive(Clone, Debug, Default)]
pub struct TrustStore {
    keys: BTreeMap<(String, String), TrustedPublisherKey>,
    snapshot_id: Option<String>,
    snapshot_version: Option<u64>,
}

impl TrustStore {
    /// Construct a trust snapshot, rejecting ambiguous or malformed provisioned records.
    ///
    /// This constructor is retained for deterministic tests and explicitly supplied host
    /// configuration. Native production startup uses [`Self::load_from_environment`] so a missing
    /// operator snapshot cannot be confused with an empty test fixture.
    ///
    /// # Errors
    ///
    /// Returns [`IntegrityError::TrustConfigurationInvalid`] for empty identities, duplicate
    /// publisher/key pairs, or invalid Ed25519 public key encodings.
    pub fn from_keys(
        keys: impl IntoIterator<Item = TrustedPublisherKey>,
    ) -> Result<Self, IntegrityError> {
        Self::from_keys_with_metadata(keys, None, None)
    }

    /// Load the provisioned trust snapshot selected by [`TRUST_STORE_PATH_ENV`].
    ///
    /// # Errors
    ///
    /// Returns a value-free [`TrustStoreError`] for missing, malformed, unavailable, not-yet-valid,
    /// expired, or keyless configuration.
    pub fn load_from_environment() -> Result<Self, TrustStoreError> {
        let path = std::env::var_os(TRUST_STORE_PATH_ENV)
            .filter(|path| !path.is_empty())
            .ok_or(TrustStoreError::Missing)?;
        Self::load_from_path(Path::new(&path))
    }

    /// Load and validate a provisioned trust snapshot from a protected configuration path.
    ///
    /// # Errors
    ///
    /// Returns a value-free [`TrustStoreError`]; raw paths and parser/provider details are not
    /// included in diagnostics.
    pub fn load_from_path(path: &Path) -> Result<Self, TrustStoreError> {
        let bytes = fs::read(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                TrustStoreError::Missing
            } else {
                TrustStoreError::Unavailable
            }
        })?;
        Self::from_json_at(&bytes, unix_now())
    }

    /// Parse and validate a provisioned trust snapshot against the current clock.
    ///
    /// # Errors
    ///
    /// Returns a value-free [`TrustStoreError`] for invalid schema, time bounds, or key records.
    pub fn from_json(bytes: &[u8]) -> Result<Self, TrustStoreError> {
        Self::from_json_at(bytes, unix_now())
    }

    /// Parse and validate a provisioned trust snapshot at a deterministic Unix timestamp.
    ///
    /// This clock-injected seam keeps expiry and rotation tests deterministic without changing
    /// production behavior.
    ///
    /// # Errors
    ///
    /// Returns a value-free [`TrustStoreError`] for invalid schema, time bounds, or key records.
    pub fn from_json_at(bytes: &[u8], now: u64) -> Result<Self, TrustStoreError> {
        let snapshot: TrustSnapshot =
            serde_json::from_slice(bytes).map_err(|_| TrustStoreError::Malformed)?;
        Self::from_snapshot_at(snapshot, now)
    }

    /// Validate a typed trust snapshot against the current clock.
    ///
    /// # Errors
    ///
    /// Returns a value-free [`TrustStoreError`] for invalid schema, time bounds, or key records.
    pub fn from_snapshot(snapshot: TrustSnapshot) -> Result<Self, TrustStoreError> {
        Self::from_snapshot_at(snapshot, unix_now())
    }

    /// Validate a typed trust snapshot at a deterministic Unix timestamp.
    ///
    /// # Errors
    ///
    /// Returns a value-free [`TrustStoreError`] for invalid schema, time bounds, or key records.
    pub fn from_snapshot_at(snapshot: TrustSnapshot, now: u64) -> Result<Self, TrustStoreError> {
        if snapshot.schema_version != TRUST_SNAPSHOT_SCHEMA_VERSION
            || snapshot.snapshot_id.is_empty()
            || snapshot.snapshot_id.len() > 256
            || !valid_snapshot_id(&snapshot.snapshot_id)
            || snapshot.expires_at <= snapshot.valid_from
        {
            return Err(TrustStoreError::Malformed);
        }
        if now < snapshot.valid_from {
            return Err(TrustStoreError::NotYetValid);
        }
        if now >= snapshot.expires_at {
            return Err(TrustStoreError::Expired);
        }

        let revocation_count = snapshot.revocations.len();
        let revoked = snapshot.revocations.into_iter().collect::<BTreeSet<_>>();
        if revoked.len() != revocation_count
            || revoked
                .iter()
                .any(|identity| !valid_identity(&identity.publisher_id, &identity.key_id))
        {
            return Err(TrustStoreError::Malformed);
        }

        let mut records = Vec::with_capacity(snapshot.keys.len());
        let mut identities = BTreeSet::new();
        for key in snapshot.keys {
            if !valid_identity(&key.publisher_id, &key.key_id)
                || key.expires_at <= key.valid_from
            {
                return Err(TrustStoreError::Malformed);
            }
            if !identities.insert((key.publisher_id.clone(), key.key_id.clone())) {
                return Err(TrustStoreError::Malformed);
            }
            let verifying_key =
                decode_public_key(&key.public_key).ok_or(TrustStoreError::Malformed)?;
            let active_window = key.valid_from <= now && now < key.expires_at;
            records.push(TrustedPublisherKey {
                publisher_id: key.publisher_id,
                key_id: key.key_id,
                verifying_key,
                enabled: key.enabled && active_window,
            });
        }

        let active = records
            .into_iter()
            .map(|mut key| {
                if revoked.contains(&PublisherKeyIdentity {
                    publisher_id: key.publisher_id.clone(),
                    key_id: key.key_id.clone(),
                }) {
                    key.enabled = false;
                }
                key
            })
            .filter(|key| key.enabled)
            .collect::<Vec<_>>();
        if active.is_empty() {
            return Err(TrustStoreError::NoActiveKeys);
        }

        Self::from_keys_with_metadata(
            active,
            Some(snapshot.snapshot_id),
            Some(snapshot.version),
        )
        .map_err(|_| TrustStoreError::Malformed)
    }

    /// Opaque release-evidence identity, when this store came from a provisioned snapshot.
    #[must_use]
    pub fn snapshot_id(&self) -> Option<&str> {
        self.snapshot_id.as_deref()
    }

    /// Provisioned snapshot version, when this store came from a provisioned snapshot.
    #[must_use]
    pub const fn snapshot_version(&self) -> Option<u64> {
        self.snapshot_version
    }

    /// Whether at least one enabled key is available.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Safe release evidence with no private key material.
    #[must_use]
    pub fn evidence(&self) -> Option<TrustSnapshotEvidence<'_>> {
        Some(TrustSnapshotEvidence {
            snapshot_id: self.snapshot_id.as_deref()?,
            version: self.snapshot_version?,
            key_count: self.keys.len(),
        })
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

    fn from_keys_with_metadata(
        keys: impl IntoIterator<Item = TrustedPublisherKey>,
        snapshot_id: Option<String>,
        snapshot_version: Option<u64>,
    ) -> Result<Self, IntegrityError> {
        let mut indexed = BTreeMap::new();
        for key in keys {
            if !valid_identity(&key.publisher_id, &key.key_id)
                || VerifyingKey::from_bytes(&key.verifying_key).is_err()
            {
                return Err(IntegrityError::TrustConfigurationInvalid);
            }
            let identity = (key.publisher_id.clone(), key.key_id.clone());
            if indexed.insert(identity, key).is_some() {
                return Err(IntegrityError::TrustConfigurationInvalid);
            }
        }
        Ok(Self {
            keys: indexed,
            snapshot_id,
            snapshot_version,
        })
    }
}

/// Safe release evidence for an active provisioned trust snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrustSnapshotEvidence<'a> {
    /// Opaque snapshot identity.
    pub snapshot_id: &'a str,
    /// Monotonic snapshot version.
    pub version: u64,
    /// Number of enabled keys, never key material.
    pub key_count: usize,
}

fn default_enabled() -> bool {
    true
}

fn valid_identity(publisher_id: &str, key_id: &str) -> bool {
    !publisher_id.is_empty()
        && publisher_id.len() <= 256
        && !key_id.is_empty()
        && key_id.len() <= 256
        && publisher_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
        && key_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

fn valid_snapshot_id(snapshot_id: &str) -> bool {
    snapshot_id.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
    })
}

fn decode_public_key(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.is_ascii() {
        return None;
    }
    let mut decoded = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        decoded[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Some(decoded)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
