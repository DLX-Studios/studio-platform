//! Signed, staged application update coordination.
//!
//! The channel owns rollout policy and installation state, while the host owns
//! downloading, replacing, and starting an application. Candidates are only
//! admitted after the package trust boundary verifies their signed update
//! document. Health failures always restore the previously active version.

#![allow(missing_docs)]
#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::assigning_clones,
    clippy::format_collect
)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use studio_package::{IntegrityError, TrustStore, VerifiedIntegrity, verify_document_signature};
use thiserror::Error;

/// Wire format version for signed update documents.
pub const UPDATE_DOCUMENT_VERSION: u16 = 1;
const MAX_INSTALLATION_ID: usize = 128;
const MAX_UPDATE_ID: usize = 128;
const MAX_VERSION: usize = 64;
const MAX_HISTORY: usize = 64;

/// A signed, immutable update-channel document.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateDocument {
    /// Signed document schema version.
    pub document_version: u16,
    /// Stable update identity, unique within a channel.
    pub update_id: String,
    /// Application semantic version to activate.
    pub version: String,
    /// Release channel, such as `stable` or `preview`.
    pub channel: String,
    /// SHA-256 digest of the exact candidate artifact bytes, lowercase hex.
    pub artifact_sha256: String,
    /// Target rollout percentage in the closed range 0..=100.
    pub rollout_percent: u8,
    /// Publisher identity used by the provisioned trust store.
    pub publisher_id: String,
    /// Publisher signing-key identity used by the provisioned trust store.
    pub key_id: String,
    /// Optional migration bundle identity that must be prepared before activation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_id: Option<String>,
}

/// Signed candidate artifact awaiting host installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedUpdate {
    /// Signed update document.
    pub document: UpdateDocument,
    /// Raw Ed25519 signature over the canonical update document.
    pub signature: Vec<u8>,
    /// Exact artifact bytes whose digest is covered by `document`.
    pub artifact: Vec<u8>,
}

/// Candidate admitted by signature, trust, and artifact-digest checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedUpdate {
    document: UpdateDocument,
    artifact: Vec<u8>,
    integrity: VerifiedIntegrity,
}

impl VerifiedUpdate {
    /// Admit one candidate through the provisioned package trust boundary.
    ///
    /// # Errors
    ///
    /// Returns [`UpdateError`] when the document, signature, or artifact digest is invalid.
    pub fn admit(candidate: SignedUpdate, trust_store: &TrustStore) -> Result<Self, UpdateError> {
        validate_document(&candidate.document)?;
        let actual_digest = hex_digest(&candidate.artifact);
        if actual_digest != candidate.document.artifact_sha256 {
            return Err(UpdateError::ArtifactDigestMismatch);
        }
        let value = serde_json::to_value(&candidate.document)
            .map_err(|_| UpdateError::DocumentInvalid("serialization"))?;
        let integrity = verify_document_signature(
            &value,
            &candidate.signature,
            &candidate.document.publisher_id,
            &candidate.document.key_id,
            trust_store,
        )
        .map_err(UpdateError::Integrity)?;
        Ok(Self {
            document: candidate.document,
            artifact: candidate.artifact,
            integrity,
        })
    }

    /// Signed update metadata.
    #[must_use]
    pub const fn document(&self) -> &UpdateDocument {
        &self.document
    }

    /// Exact verified artifact bytes for the host installer.
    #[must_use]
    pub fn artifact(&self) -> &[u8] {
        &self.artifact
    }

    /// Cryptographic evidence for the signed document.
    #[must_use]
    pub const fn integrity(&self) -> &VerifiedIntegrity {
        &self.integrity
    }
}

/// Persistent per-installation update state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallationState {
    /// Stable installation identity.
    pub installation_id: String,
    /// Version currently serving traffic.
    pub active_version: String,
    /// Version restored after the last failed activation, if any.
    pub previous_version: Option<String>,
    /// Candidate currently staged for this installation.
    pub staged_update_id: Option<String>,
    /// Last failure code, without provider or credential data.
    pub last_error: Option<UpdateErrorCode>,
    /// Monotonic state sequence for stale-writer detection.
    pub revision: u64,
    /// Bounded state transition history.
    pub history: Vec<InstallationEvent>,
}

/// One redacted installation state transition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallationEvent {
    /// Monotonic installation state sequence.
    pub revision: u64,
    /// Update identity, when the event concerns a candidate.
    pub update_id: Option<String>,
    /// Version associated with the transition, when known.
    pub version: Option<String>,
    /// Transition kind.
    pub kind: InstallationEventKind,
}

/// Safe installation transition kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallationEventKind {
    /// Candidate was not selected by deterministic rollout policy.
    Skipped,
    /// Candidate was selected and staged.
    Staged,
    /// Candidate passed host installation and health checks.
    Activated,
    /// Candidate failed health and the previous version was restored.
    RolledBack,
    /// Candidate installation failed before health could run.
    InstallFailed,
}

/// Host-provided installation and health boundary.
pub trait InstallationHost {
    /// Install and start the exact verified artifact.
    fn install(&mut self, installation_id: &str, update: &VerifiedUpdate) -> Result<(), HostError>;
    /// Check that the new version is serving correctly.
    fn health_check(&mut self, installation_id: &str, version: &str) -> Result<(), HostError>;
    /// Restore the previous version after a failed candidate.
    fn rollback(&mut self, installation_id: &str, version: &str) -> Result<(), HostError>;
}

/// Persistence boundary for per-installation state.
pub trait UpdateStateStore {
    /// Load one installation's state.
    fn load(&self, installation_id: &str) -> Result<Option<InstallationState>, StoreError>;
    /// Persist state using the expected revision as a compare-and-swap.
    fn save(&mut self, state: InstallationState, expected_revision: u64) -> Result<(), StoreError>;
}

/// Deterministic in-memory state store for tests and disposable hosts.
#[derive(Clone, Debug, Default)]
pub struct MemoryStateStore {
    states: BTreeMap<String, InstallationState>,
}

impl MemoryStateStore {
    /// Return a read-only snapshot of all persisted installation states.
    #[must_use]
    pub const fn states(&self) -> &BTreeMap<String, InstallationState> {
        &self.states
    }
}

impl UpdateStateStore for MemoryStateStore {
    fn load(&self, installation_id: &str) -> Result<Option<InstallationState>, StoreError> {
        Ok(self.states.get(installation_id).cloned())
    }

    fn save(&mut self, state: InstallationState, expected_revision: u64) -> Result<(), StoreError> {
        let current = self
            .states
            .get(&state.installation_id)
            .map_or(0, |value| value.revision);
        if current != expected_revision {
            return Err(StoreError::RevisionConflict);
        }
        self.states.insert(state.installation_id.clone(), state);
        Ok(())
    }
}

/// Rollout and installation coordinator.
#[derive(Debug)]
pub struct UpdateChannel<S> {
    state_store: S,
    channel: String,
}

impl<S: UpdateStateStore> UpdateChannel<S> {
    /// Create a coordinator for one named release channel.
    ///
    /// # Errors
    ///
    /// Returns [`UpdateError::DocumentInvalid`] when the channel name is empty or unsafe.
    pub fn new(state_store: S, channel: impl Into<String>) -> Result<Self, UpdateError> {
        let channel = channel.into();
        if channel.is_empty() || channel.len() > 64 || !is_safe_identifier(&channel) {
            return Err(UpdateError::DocumentInvalid("channel"));
        }
        Ok(Self {
            state_store,
            channel,
        })
    }

    /// Access the underlying state store after coordination is complete.
    #[must_use]
    pub fn state_store(&self) -> &S {
        &self.state_store
    }

    /// Stage and activate a candidate for many installations in one channel operation.
    ///
    /// Selection is deterministic from `(installation_id, update_id)`, so retries cannot move
    /// an installation between rollout cohorts. A health failure invokes rollback before the
    /// failed state is persisted.
    ///
    /// # Errors
    ///
    /// Returns a safe coordination error. Individual installation outcomes are retained in the
    /// returned report when one installation fails.
    pub fn roll_out<H: InstallationHost>(
        &mut self,
        update: &VerifiedUpdate,
        installations: &[String],
        host: &mut H,
    ) -> Result<RolloutReport, UpdateError> {
        if update.document.channel != self.channel {
            return Err(UpdateError::ChannelMismatch);
        }
        let mut outcomes = Vec::with_capacity(installations.len());
        for installation_id in installations {
            outcomes.push(self.roll_out_one(update, installation_id, host)?);
        }
        Ok(RolloutReport {
            update_id: update.document.update_id.clone(),
            outcomes,
        })
    }

    fn roll_out_one<H: InstallationHost>(
        &mut self,
        update: &VerifiedUpdate,
        installation_id: &str,
        host: &mut H,
    ) -> Result<InstallationOutcome, UpdateError> {
        validate_installation_id(installation_id)?;
        let loaded = self
            .state_store
            .load(installation_id)
            .map_err(UpdateError::Store)?
            .ok_or(UpdateError::InstallationUnknown)?;
        let expected_revision = loaded.revision;
        if loaded.active_version == update.document.version {
            return Ok(InstallationOutcome::AlreadyActive);
        }
        if !eligible(
            installation_id,
            &update.document.update_id,
            update.document.rollout_percent,
        ) {
            let state = transition(
                loaded,
                InstallationEventKind::Skipped,
                Some(&update.document.update_id),
                Some(&update.document.version),
            );
            self.state_store
                .save(state, expected_revision)
                .map_err(UpdateError::Store)?;
            return Ok(InstallationOutcome::Skipped);
        }

        let staged = transition(
            loaded,
            InstallationEventKind::Staged,
            Some(&update.document.update_id),
            Some(&update.document.version),
        );
        let previous_version = staged.active_version.clone();
        self.state_store
            .save(staged, expected_revision)
            .map_err(UpdateError::Store)?;
        let staged_state = self
            .state_store
            .load(installation_id)
            .map_err(UpdateError::Store)?
            .ok_or(UpdateError::InstallationUnknown)?;
        let staged_revision = staged_state.revision;

        if host.install(installation_id, update).is_err() {
            let mut failed = transition(
                staged_state,
                InstallationEventKind::InstallFailed,
                Some(&update.document.update_id),
                Some(&update.document.version),
            );
            failed.staged_update_id = None;
            failed.last_error = Some(UpdateErrorCode::HostInstall);
            self.state_store
                .save(failed, staged_revision)
                .map_err(UpdateError::Store)?;
            return Ok(InstallationOutcome::Failed(UpdateErrorCode::HostInstall));
        }
        if host
            .health_check(installation_id, &update.document.version)
            .is_err()
        {
            let rollback_ok = host
                .rollback(installation_id, previous_version.as_str())
                .is_ok();
            let current = self
                .state_store
                .load(installation_id)
                .map_err(UpdateError::Store)?
                .ok_or(UpdateError::InstallationUnknown)?;
            let mut rolled_back = transition(
                current,
                InstallationEventKind::RolledBack,
                Some(&update.document.update_id),
                Some(&previous_version),
            );
            rolled_back.active_version = previous_version;
            rolled_back.previous_version = Some(update.document.version.clone());
            rolled_back.staged_update_id = None;
            rolled_back.last_error = Some(if rollback_ok {
                UpdateErrorCode::HealthCheck
            } else {
                UpdateErrorCode::RollbackFailed
            });
            self.state_store
                .save(rolled_back, staged_revision)
                .map_err(UpdateError::Store)?;
            return Ok(InstallationOutcome::Failed(if rollback_ok {
                UpdateErrorCode::HealthCheck
            } else {
                UpdateErrorCode::RollbackFailed
            }));
        }

        let current = self
            .state_store
            .load(installation_id)
            .map_err(UpdateError::Store)?
            .ok_or(UpdateError::InstallationUnknown)?;
        let mut activated = transition(
            current,
            InstallationEventKind::Activated,
            Some(&update.document.update_id),
            Some(&update.document.version),
        );
        activated.previous_version = Some(activated.active_version.clone());
        activated.active_version = update.document.version.clone();
        activated.staged_update_id = None;
        activated.last_error = None;
        self.state_store
            .save(activated, staged_revision)
            .map_err(UpdateError::Store)?;
        Ok(InstallationOutcome::Activated)
    }
}

/// Per-installation result of a rollout attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallationOutcome {
    /// Installation was already running the candidate version.
    AlreadyActive,
    /// Installation was outside the deterministic rollout cohort.
    Skipped,
    /// Installation passed install and health checks.
    Activated,
    /// Installation failed and recorded a redacted error code.
    Failed(UpdateErrorCode),
}

/// Aggregate channel operation result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RolloutReport {
    /// Candidate update identity.
    pub update_id: String,
    /// Results in the same order as the requested installation IDs.
    pub outcomes: Vec<InstallationOutcome>,
}

/// Host installation failure classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostError {
    /// Artifact could not be installed or started.
    Install,
    /// Candidate did not become healthy.
    Health,
    /// Previous version could not be restored.
    Rollback,
}

/// Persistence failure classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreError {
    /// State was changed by another writer after it was read.
    RevisionConflict,
    /// Store could not load or persist its record.
    Unavailable,
}

/// Stable update-channel rejection family.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateErrorCode {
    /// Candidate signature or provisioned trust failed.
    Integrity,
    /// Candidate document is malformed.
    DocumentInvalid,
    /// Candidate artifact digest does not match the signed document.
    ArtifactDigest,
    /// Candidate targets a different channel.
    ChannelMismatch,
    /// Installation identity is invalid.
    InstallationInvalid,
    /// Installation has no persisted baseline version.
    InstallationUnknown,
    /// Host installation failed.
    HostInstall,
    /// Health check failed.
    HealthCheck,
    /// Rollback failed.
    RollbackFailed,
    /// State persistence failed.
    Store,
}

/// Detailed update-channel failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum UpdateError {
    /// Signature/trust verification failed.
    #[error("update integrity verification failed")]
    Integrity(#[source] IntegrityError),
    /// Closed update document validation failed.
    #[error("update document invalid: {0}")]
    DocumentInvalid(&'static str),
    /// Artifact bytes did not match the signed digest.
    #[error("update artifact digest mismatch")]
    ArtifactDigestMismatch,
    /// Candidate channel did not match the coordinator.
    #[error("update channel mismatch")]
    ChannelMismatch,
    /// Installation identity is malformed.
    #[error("installation identity invalid")]
    InstallationInvalid,
    /// Installation has no baseline state.
    #[error("installation state unavailable")]
    InstallationUnknown,
    /// Host-side state persistence failed.
    #[error("update state persistence failed")]
    Store(StoreError),
}

impl UpdateError {
    /// Stable redacted diagnostic family.
    #[must_use]
    pub const fn code(&self) -> UpdateErrorCode {
        match self {
            Self::Integrity(_) => UpdateErrorCode::Integrity,
            Self::DocumentInvalid(_) => UpdateErrorCode::DocumentInvalid,
            Self::ArtifactDigestMismatch => UpdateErrorCode::ArtifactDigest,
            Self::ChannelMismatch => UpdateErrorCode::ChannelMismatch,
            Self::InstallationInvalid => UpdateErrorCode::InstallationInvalid,
            Self::InstallationUnknown => UpdateErrorCode::InstallationUnknown,
            Self::Store(_) => UpdateErrorCode::Store,
        }
    }
}

fn validate_document(document: &UpdateDocument) -> Result<(), UpdateError> {
    if document.document_version != UPDATE_DOCUMENT_VERSION {
        return Err(UpdateError::DocumentInvalid("version"));
    }
    if document.update_id.is_empty()
        || document.update_id.len() > MAX_UPDATE_ID
        || !is_safe_identifier(&document.update_id)
    {
        return Err(UpdateError::DocumentInvalid("updateId"));
    }
    if document.version.is_empty() || document.version.len() > MAX_VERSION {
        return Err(UpdateError::DocumentInvalid("version"));
    }
    if document.channel.is_empty() || !is_safe_identifier(&document.channel) {
        return Err(UpdateError::DocumentInvalid("channel"));
    }
    if document.rollout_percent > 100 {
        return Err(UpdateError::DocumentInvalid("rolloutPercent"));
    }
    if document.artifact_sha256.len() != 64
        || !document
            .artifact_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || document.artifact_sha256 != document.artifact_sha256.to_ascii_lowercase()
    {
        return Err(UpdateError::DocumentInvalid("artifactSha256"));
    }
    if document.publisher_id.is_empty() || document.key_id.is_empty() {
        return Err(UpdateError::DocumentInvalid("publisher"));
    }
    if document
        .migration_id
        .as_deref()
        .is_some_and(|value| value.is_empty() || !is_safe_identifier(value))
    {
        return Err(UpdateError::DocumentInvalid("migrationId"));
    }
    Ok(())
}

fn validate_installation_id(value: &str) -> Result<(), UpdateError> {
    if value.is_empty() || value.len() > MAX_INSTALLATION_ID || !is_safe_identifier(value) {
        return Err(UpdateError::InstallationInvalid);
    }
    Ok(())
}

fn is_safe_identifier(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn eligible(installation_id: &str, update_id: &str, rollout_percent: u8) -> bool {
    if rollout_percent >= 100 {
        return true;
    }
    let digest = Sha256::digest(
        format!("studio.update.cohort.v1\0{installation_id}\0{update_id}").as_bytes(),
    );
    u16::from_be_bytes([digest[0], digest[1]]) % 100 < u16::from(rollout_percent)
}

fn transition(
    mut state: InstallationState,
    kind: InstallationEventKind,
    update_id: Option<&str>,
    version: Option<&str>,
) -> InstallationState {
    state.revision = state.revision.saturating_add(1);
    state.history.push(InstallationEvent {
        revision: state.revision,
        update_id: update_id.map(ToOwned::to_owned),
        version: version.map(ToOwned::to_owned),
        kind,
    });
    if state.history.len() > MAX_HISTORY {
        let overflow = state.history.len() - MAX_HISTORY;
        state.history.drain(..overflow);
    }
    state.staged_update_id = update_id.map(ToOwned::to_owned);
    state
}

/// Construct the canonical signed JSON value for an update document.
#[must_use]
pub fn canonical_update_document(document: &UpdateDocument) -> Value {
    json!(document)
}

/// Compute the lowercase artifact digest used by [`UpdateDocument`].
#[must_use]
pub fn artifact_digest(artifact: &[u8]) -> String {
    hex_digest(artifact)
}
