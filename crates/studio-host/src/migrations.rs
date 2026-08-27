//! Host-owned, pre-launch application-data migration lifecycle.
//!
//! This module deliberately operates on one opaque, host-owned JSON state batch. Migration code
//! receives no database handle, namespace selector, query language, or guest capability. The
//! bundle and every migration asset must first pass [`VerifiedMigrationBundle::admit`], which
//! unconditionally verifies the publisher signature.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use studio_package::{MigrationAdmissionError, MigrationDeclaration, VerifiedMigrationBundle};
use thiserror::Error;

use crate::{LocalStore, StoreBatch, StoreBatchEntry};

/// Reserved LocalStore batch containing the application-data migration envelope.
pub const MIGRATION_STATE_BATCH_ID: &str = "__studio.application-data.migrations";
const MIGRATION_STATE_FORMAT_VERSION: u16 = 1;
const INITIAL_APPLICATION_SCHEMA_VERSION: u32 = 1;
const MAX_STATE_BYTES: usize = 4 * 1024 * 1024;

/// Lifecycle state persisted around every migration boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum MigrationLifecycle {
    /// No migration is currently executing.
    Idle,
    /// Recovery data was durably written before migration execution.
    RecoveryPointCreated { migration_id: String },
    /// The migration action is running against an in-memory candidate.
    Applying { migration_id: String },
    /// The candidate was written and is awaiting the final commit boundary.
    Validating { migration_id: String },
    /// All requested migrations completed and passed validation.
    Committed,
    /// The application is quarantined until an explicit restore/retry action.
    Quarantined { migration_id: String },
}

/// Durable pre-migration recovery point.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryPoint {
    /// Stable deterministic identity for this migration attempt.
    pub id: String,
    /// Application schema version before execution.
    pub schema_version: u32,
    /// Opaque application data before execution.
    pub data: Value,
    /// Migrations completed before this recovery point was created.
    pub completed: Vec<String>,
}

/// Host-owned persisted application-data state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MigrationState {
    format_version: u16,
    schema_version: u32,
    data: Value,
    completed: Vec<String>,
    recovery_point: Option<RecoveryPoint>,
    lifecycle: MigrationLifecycle,
}

impl MigrationState {
    /// Construct an initial state for a host-selected schema version.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationErrorCode::StateCorrupt`] for schema version zero or data that exceeds
    /// the host state ceiling.
    pub fn new(schema_version: u32, data: Value) -> Result<Self, MigrationError> {
        let state = Self {
            format_version: MIGRATION_STATE_FORMAT_VERSION,
            schema_version,
            data,
            completed: Vec::new(),
            recovery_point: None,
            lifecycle: MigrationLifecycle::Idle,
        };
        validate_state(&state)?;
        Ok(state)
    }

    /// Current application schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Borrow the opaque application data.
    #[must_use]
    pub const fn data(&self) -> &Value {
        &self.data
    }

    /// Borrow the persisted migration lifecycle state.
    #[must_use]
    pub const fn lifecycle(&self) -> &MigrationLifecycle {
        &self.lifecycle
    }

    /// Migrations recorded as completed by the host.
    #[must_use]
    pub fn completed(&self) -> &[String] {
        &self.completed
    }

    /// Borrow the retained recovery point, if one exists.
    #[must_use]
    pub const fn recovery_point(&self) -> Option<&RecoveryPoint> {
        self.recovery_point.as_ref()
    }
}

/// Failure returned by host-defined migration action or validation code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationStepError {
    /// The action rejected the input or produced an invalid candidate.
    Rejected,
    /// The action was interrupted, including an injected crash rehearsal.
    Interrupted,
}

/// Stable host-owned migration failure family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationErrorCode {
    /// The migration package could not be admitted.
    AdmissionInvalid,
    /// Persisted migration state is malformed or too large.
    StateCorrupt,
    /// Persisted state is quarantined and needs an explicit recovery action.
    Quarantined,
    /// The requested migration chain does not match the current version.
    VersionUnsupported,
    /// Migration declarations are not a contiguous forward-only chain.
    OrderInvalid,
    /// A declared migration payload is not present in the verified bundle.
    AssetMissing,
    /// A migration action failed or was interrupted.
    ActionFailed,
    /// Post-migration validation failed.
    ValidationFailed,
    /// The LocalStore operation failed.
    StorageFailed,
    /// No recovery point is available to restore.
    RecoveryUnavailable,
}

/// Safe migration failure without storage, package, or application-data details.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum MigrationError {
    /// Signed package admission failed.
    #[error("signed migration admission failed")]
    Admission(#[source] MigrationAdmissionError),
    /// Persisted state is damaged or violates host limits.
    #[error("application migration state is invalid")]
    StateCorrupt,
    /// The application is quarantined pending explicit recovery.
    #[error("application migration is quarantined; restore the recovery point before retrying")]
    Quarantined,
    /// The current state cannot be advanced by the supplied package.
    #[error("application migration version is unsupported")]
    VersionUnsupported,
    /// The package's migration chain is invalid.
    #[error("application migration order is invalid")]
    OrderInvalid,
    /// A verified declaration has no corresponding verified asset.
    #[error("application migration asset is missing")]
    AssetMissing,
    /// Action execution failed; state is quarantined.
    #[error("application migration action failed; application was quarantined")]
    ActionFailed,
    /// Candidate validation failed; state is quarantined.
    #[error("application migration validation failed; application was quarantined")]
    ValidationFailed,
    /// Host persistence failed.
    #[error("application migration storage operation failed")]
    StorageFailed,
    /// No durable recovery point exists.
    #[error("application migration recovery point is unavailable")]
    RecoveryUnavailable,
}

impl MigrationError {
    /// Stable failure family for diagnostics and recovery routing.
    #[must_use]
    pub const fn code(&self) -> MigrationErrorCode {
        match self {
            Self::Admission(_) => MigrationErrorCode::AdmissionInvalid,
            Self::StateCorrupt => MigrationErrorCode::StateCorrupt,
            Self::Quarantined => MigrationErrorCode::Quarantined,
            Self::VersionUnsupported => MigrationErrorCode::VersionUnsupported,
            Self::OrderInvalid => MigrationErrorCode::OrderInvalid,
            Self::AssetMissing => MigrationErrorCode::AssetMissing,
            Self::ActionFailed => MigrationErrorCode::ActionFailed,
            Self::ValidationFailed => MigrationErrorCode::ValidationFailed,
            Self::StorageFailed => MigrationErrorCode::StorageFailed,
            Self::RecoveryUnavailable => MigrationErrorCode::RecoveryUnavailable,
        }
    }
}

/// Report returned after a migration lifecycle completes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationRunReport {
    /// Schema version before this run.
    pub initial_version: u32,
    /// Schema version after this run.
    pub final_version: u32,
    /// Migration IDs applied in order.
    pub applied: Vec<String>,
    /// Recovery point retained for explicit restore.
    pub recovery_point_id: Option<String>,
}

/// Host-owned application-schema migration runner over a [`LocalStore`] batch.
///
/// This runner never changes [`crate::StoreMetadata::engine_format_version`]. Engine/on-disk
/// upgrades remain the separate [`crate::EmbeddedLocalStore`] recovery and qualification path;
/// combining the two would allow an application migration to silently open an incompatible store.
pub struct MigrationRunner<'a, S> {
    store: &'a S,
    state_batch_id: String,
}

impl<'a, S> MigrationRunner<'a, S> {
    /// Bind a runner to the reserved application migration state batch.
    #[must_use]
    pub fn new(store: &'a S) -> Self {
        Self {
            store,
            state_batch_id: MIGRATION_STATE_BATCH_ID.to_owned(),
        }
    }

    /// Bind a runner to an explicit host-owned state batch.
    ///
    /// This is useful when one LocalStore contains multiple application namespaces. The ID is
    /// host-selected; guest requests cannot select it.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationErrorCode::StateCorrupt`] for an empty, control-bearing, or reserved
    /// invalid batch ID.
    pub fn for_batch(
        store: &'a S,
        state_batch_id: impl Into<String>,
    ) -> Result<Self, MigrationError> {
        let state_batch_id = state_batch_id.into();
        if state_batch_id.is_empty() || state_batch_id.chars().any(char::is_control) {
            return Err(MigrationError::StateCorrupt);
        }
        Ok(Self { store, state_batch_id })
    }
}

impl<S: LocalStore> MigrationRunner<'_, S> {
    /// Persist initial state only when no migration envelope exists.
    ///
    /// # Errors
    ///
    /// Returns [`MigrationErrorCode::StorageFailed`] when the host store cannot be written, or
    /// [`MigrationErrorCode::StateCorrupt`] if an envelope already exists.
    pub async fn initialize(&self, state: MigrationState) -> Result<(), MigrationError> {
        validate_state(&state)?;
        if !self.load_entries().await?.is_empty() {
            return Err(MigrationError::StateCorrupt);
        }
        self.persist(&state).await
    }

    /// Read current host-owned state, treating a fresh store as schema version one.
    pub async fn state(&self) -> Result<MigrationState, MigrationError> {
        self.load().await
    }

    /// Restore the last pre-migration recovery point and clear quarantine.
    ///
    /// Restoring is an explicit host operation. It writes the old data and version as a new
    /// durable state before a caller can retry or launch the guest.
    pub async fn restore_recovery_point(&self) -> Result<MigrationState, MigrationError> {
        let current = self.load().await?;
        let recovery = current
            .recovery_point
            .clone()
            .ok_or(MigrationError::RecoveryUnavailable)?;
        let restored = MigrationState {
            format_version: MIGRATION_STATE_FORMAT_VERSION,
            schema_version: recovery.schema_version,
            data: recovery.data,
            completed: recovery.completed,
            recovery_point: None,
            lifecycle: MigrationLifecycle::Idle,
        };
        self.persist(&restored).await?;
        Ok(restored)
    }

    /// Execute all required signed migrations with the default object/size validator.
    pub async fn run<F>(
        &self,
        package: &VerifiedMigrationBundle,
        action: F,
    ) -> Result<MigrationRunReport, MigrationError>
    where
        F: FnMut(&MigrationDeclaration, &[u8], &mut Value) -> Result<(), MigrationStepError> + Send,
    {
        self.run_with_validator(package, action, |_migration, data| {
            if data.is_object() {
                Ok(())
            } else {
                Err(MigrationStepError::Rejected)
            }
        })
        .await
    }

    /// Execute migrations with an application-specific post-migration validator.
    pub async fn run_with_validator<F, V>(
        &self,
        package: &VerifiedMigrationBundle,
        mut action: F,
        mut validator: V,
    ) -> Result<MigrationRunReport, MigrationError>
    where
        F: FnMut(&MigrationDeclaration, &[u8], &mut Value) -> Result<(), MigrationStepError> + Send,
        V: FnMut(&MigrationDeclaration, &Value) -> Result<(), MigrationStepError> + Send,
    {
        validate_package_chain(package)?;
        let mut state = self.load().await?;
        if matches!(&state.lifecycle, MigrationLifecycle::Quarantined { .. }) {
            return Err(MigrationError::Quarantined);
        }

        // A process crash can occur after the recovery envelope is written but before the final
        // candidate commit. The old snapshot is authoritative in that case.
        if !matches!(&state.lifecycle, MigrationLifecycle::Idle | MigrationLifecycle::Committed) {
            let recovery = state
                .recovery_point
                .clone()
                .ok_or(MigrationError::StateCorrupt)?;
            state.schema_version = recovery.schema_version;
            state.data = recovery.data;
            state.completed = recovery.completed;
            state.lifecycle = MigrationLifecycle::Idle;
            state.recovery_point = None;
            self.persist(&state).await?;
        }

        let declarations = required_declarations(&state, package)?;
        let initial_version = state.schema_version;
        if declarations.is_empty() {
            return Ok(MigrationRunReport {
                initial_version,
                final_version: state.schema_version,
                applied: Vec::new(),
                recovery_point_id: state.recovery_point.as_ref().map(|point| point.id.clone()),
            });
        }

        let recovery = RecoveryPoint {
            id: recovery_id(package, state.schema_version),
            schema_version: state.schema_version,
            data: state.data.clone(),
            completed: state.completed.clone(),
        };
        state.recovery_point = Some(recovery.clone());
        state.lifecycle = MigrationLifecycle::RecoveryPointCreated {
            migration_id: declarations[0].id.clone(),
        };
        self.persist(&state).await?;

        let mut applied = Vec::with_capacity(declarations.len());
        for declaration in declarations {
            let asset = package
                .migration_asset(&declaration.entry)
                .ok_or_else(|| {
                    // Admission checks this too, but retain a second host-side guard at the
                    // execution boundary so future package changes cannot create a bypass.
                    MigrationError::AssetMissing
                })?;
            state.lifecycle = MigrationLifecycle::Applying {
                migration_id: declaration.id.clone(),
            };
            self.persist(&state).await?;

            let mut candidate = state.data.clone();
            if action(&declaration, asset, &mut candidate).is_err() {
                return self.quarantine(&state, &declaration.id, MigrationError::ActionFailed).await;
            }
            if validator(&declaration, &candidate).is_err() {
                return self
                    .quarantine(&state, &declaration.id, MigrationError::ValidationFailed)
                    .await;
            }
            state.data = candidate;
            state.schema_version = declaration.to_version;
            state.completed.push(declaration.id.clone());
            state.lifecycle = MigrationLifecycle::Validating {
                migration_id: declaration.id.clone(),
            };
            self.persist(&state).await?;
            applied.push(declaration.id);
        }

        state.lifecycle = MigrationLifecycle::Committed;
        self.persist(&state).await?;
        Ok(MigrationRunReport {
            initial_version,
            final_version: state.schema_version,
            applied,
            recovery_point_id: Some(recovery.id),
        })
    }

    async fn quarantine(
        &self,
        state: &MigrationState,
        migration_id: &str,
        error: MigrationError,
    ) -> Result<MigrationRunReport, MigrationError> {
        let mut quarantined = state.clone();
        if let Some(recovery) = &state.recovery_point {
            quarantined.schema_version = recovery.schema_version;
            quarantined.data = recovery.data.clone();
            quarantined.completed = recovery.completed.clone();
        }
        quarantined.lifecycle = MigrationLifecycle::Quarantined {
            migration_id: migration_id.to_owned(),
        };
        self.persist(&quarantined).await?;
        Err(error)
    }

    async fn load_entries(&self) -> Result<Vec<StoreBatchEntry>, MigrationError> {
        self.store
            .batch_entries(&self.state_batch_id)
            .await
            .map_err(|_| MigrationError::StorageFailed)
    }

    async fn load(&self) -> Result<MigrationState, MigrationError> {
        let entries = self.load_entries().await?;
        if entries.is_empty() {
            return MigrationState::new(INITIAL_APPLICATION_SCHEMA_VERSION, json!({}));
        }
        if entries.len() != 1 || entries[0].ordinal != 0 {
            return Err(MigrationError::StateCorrupt);
        }
        let state: MigrationState = serde_json::from_value(entries[0].payload.clone())
            .map_err(|_| MigrationError::StateCorrupt)?;
        validate_state(&state)?;
        Ok(state)
    }

    async fn persist(&self, state: &MigrationState) -> Result<(), MigrationError> {
        validate_state(state)?;
        let payload = serde_json::to_value(state).map_err(|_| MigrationError::StateCorrupt)?;
        let batch = StoreBatch::new(
            self.state_batch_id.clone(),
            [StoreBatchEntry { ordinal: 0, payload }],
        )
        .map_err(|_| MigrationError::StateCorrupt)?;
        self.store
            .write_batch(&batch)
            .await
            .map_err(|_| MigrationError::StorageFailed)
    }
}

fn validate_state(state: &MigrationState) -> Result<(), MigrationError> {
    if state.format_version != MIGRATION_STATE_FORMAT_VERSION
        || state.schema_version == 0
        || serde_json::to_vec(state)
            .map_err(|_| MigrationError::StateCorrupt)?
            .len()
            > MAX_STATE_BYTES
    {
        return Err(MigrationError::StateCorrupt);
    }
    let mut completed = BTreeSet::new();
    if state
        .completed
        .iter()
        .any(|id| !valid_migration_id(id) || !completed.insert(id))
    {
        return Err(MigrationError::StateCorrupt);
    }
    if let Some(recovery) = &state.recovery_point {
        if recovery.id.is_empty()
            || recovery.schema_version == 0
            || recovery.completed.iter().any(|id| !valid_migration_id(id))
        {
            return Err(MigrationError::StateCorrupt);
        }
    }
    Ok(())
}

fn valid_migration_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.starts_with(|character: char| character.is_ascii_lowercase())
        && id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        })
}

fn validate_package_chain(package: &VerifiedMigrationBundle) -> Result<(), MigrationError> {
    let migrations = &package.manifest().migrations;
    if migrations.is_empty()
        || migrations.windows(2).any(|pair| {
            pair[0].from_version >= pair[1].from_version
                || pair[0].to_version != pair[1].from_version
        })
    {
        return Err(MigrationError::OrderInvalid);
    }
    if migrations
        .iter()
        .any(|migration| package.migration_asset(&migration.entry).is_none())
    {
        return Err(MigrationError::AssetMissing);
    }
    Ok(())
}

fn required_declarations<'a>(
    state: &MigrationState,
    package: &'a VerifiedMigrationBundle,
) -> Result<Vec<&'a MigrationDeclaration>, MigrationError> {
    if package
        .manifest()
        .migrations
        .last()
        .is_some_and(|migration| state.schema_version > migration.to_version)
    {
        return Err(MigrationError::VersionUnsupported);
    }
    let mut current = state.schema_version;
    let mut required = Vec::new();
    for migration in &package.manifest().migrations {
        if migration.to_version <= current {
            if !state.completed.iter().any(|id| id == &migration.id) {
                return Err(MigrationError::StateCorrupt);
            }
            continue;
        }
        if migration.from_version != current {
            return Err(MigrationError::VersionUnsupported);
        }
        required.push(migration);
        current = migration.to_version;
    }
    Ok(required)
}

fn recovery_id(package: &VerifiedMigrationBundle, schema_version: u32) -> String {
    let digest = package.integrity().document_sha256;
    let mut encoded = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    format!("migration-v{schema_version}-{encoded}")
}

#[cfg(test)]
mod tests {
    #![allow(missing_docs)]

    use std::{
        collections::BTreeMap,
        future::Future,
        sync::{Arc, Mutex},
    };

    use ed25519_dalek::{Signer, SigningKey};
    use studio_package::{
        ArchiveFiles, ArchivePolicy, CanonicalBundleInput, TrustStore, TrustedPublisherKey,
        ManifestPolicy, build_archive, canonical_bundle_document, inspect_archive,
    };

    use super::*;

    #[derive(Default)]
    struct MemoryStore {
        batches: Mutex<BTreeMap<String, Vec<StoreBatchEntry>>>,
    }

    impl LocalStore for MemoryStore {
        fn metadata(
            &self,
        ) -> impl Future<Output = Result<crate::StoreMetadata, crate::LocalStoreError>> + Send {
            async { panic!("migration tests never query engine metadata") }
        }

        fn write_batch(
            &self,
            batch: &StoreBatch,
        ) -> impl Future<Output = Result<(), crate::LocalStoreError>> + Send {
            let id = batch.id().to_owned();
            let entries = batch.entries().to_vec();
            async move {
                self.batches
                    .lock()
                    .expect("memory store lock")
                    .insert(id, entries);
                Ok(())
            }
        }

        fn batch_entries(
            &self,
            batch_id: &str,
        ) -> impl Future<Output = Result<Vec<StoreBatchEntry>, crate::LocalStoreError>> + Send {
            let batch_id = batch_id.to_owned();
            async move {
                Ok(self
                    .batches
                    .lock()
                    .expect("memory store lock")
                    .get(&batch_id)
                    .cloned()
                    .unwrap_or_default())
            }
        }

        fn close(self) -> impl Future<Output = Result<(), crate::LocalStoreError>> + Send {
            async { Ok(()) }
        }
    }

    fn signed_package() -> VerifiedMigrationBundle {
        let migration_entry = "assets/migrations/v1-to-v2.json";
        let manifest = json!({
            "schemaVersion": 1,
            "id": "com.example.app",
            "name": "Example App",
            "version": "1.0.0",
            "publisher": {"id": "example", "keyId": "key-1"},
            "entry": "module.wasm",
            "sdkVersion": "^0.1.0",
            "protocolVersion": 1,
            "capabilities": [],
            "limits": {"memoryMiB": 16, "eventFuel": 10000000},
            "assets": [migration_entry],
            "migrations": [{
                "id": "v1-to-v2",
                "fromVersion": 1,
                "toVersion": 2,
                "entry": migration_entry
            }]
        });
        let module = vec![0, 97, 115, 109];
        let migration = br#"{"operation":"add-version"}"#.to_vec();
        let assets = BTreeMap::from([(migration_entry.to_owned(), migration)]);
        let input = CanonicalBundleInput {
            manifest: manifest.clone(),
            module_path: "module.wasm".to_owned(),
            module: module.clone(),
            assets: assets.clone(),
        };
        let signing_key = SigningKey::from_bytes(&[9; 32]);
        let signature = signing_key
            .sign(&canonical_bundle_document(&input).expect("canonical document"))
            .to_bytes();
        let archive = build_archive(
            &ArchiveFiles {
                manifest: serde_json::to_vec(&manifest).expect("manifest JSON"),
                module,
                signature: signature.to_vec(),
                assets,
            },
            ArchivePolicy::default(),
        )
        .expect("archive builds");
        let inspected =
            inspect_archive(&archive, ArchivePolicy::default()).expect("archive inspects");
        let trust = TrustStore::from_keys([TrustedPublisherKey {
            publisher_id: "example".to_owned(),
            key_id: "key-1".to_owned(),
            verifying_key: signing_key.verifying_key().to_bytes(),
            enabled: true,
        }])
        .expect("trust store");
        VerifiedMigrationBundle::admit(&inspected, ManifestPolicy::default(), &trust)
            .expect("migration bundle admits")
    }

    #[tokio::test]
    async fn migration_run_is_idempotent_and_retains_a_recovery_point() {
        let store = MemoryStore::default();
        let runner = MigrationRunner::new(&store);
        runner
            .initialize(MigrationState::new(1, json!({"records": []})).expect("state"))
            .await
            .expect("initial state persists");
        let package = signed_package();
        let mut calls = 0;
        let first = runner
            .run(&package, |_migration, asset, data| {
                calls += 1;
                assert!(!asset.is_empty());
                data["schema"] = json!(2);
                Ok(())
            })
            .await
            .expect("migration succeeds");
        assert_eq!(calls, 1);
        assert_eq!(first.final_version, 2);
        assert!(first.recovery_point_id.is_some());
        assert!(matches!(
            runner.state().await.unwrap().lifecycle(),
            MigrationLifecycle::Committed
        ));

        let second = runner
            .run(&package, |_migration, _asset, _data| {
                panic!("completed migration must not run again");
            })
            .await
            .expect("replay is a no-op");
        assert!(second.applied.is_empty());
    }

    #[tokio::test]
    async fn failed_migration_quarantines_and_explicit_restore_recovers_prior_data() {
        let store = MemoryStore::default();
        let runner = MigrationRunner::new(&store);
        runner
            .initialize(MigrationState::new(1, json!({"stable": true})).expect("state"))
            .await
            .expect("initial state persists");
        let package = signed_package();
        assert_eq!(
            runner
                .run(&package, |_migration, _asset, _data| Err(MigrationStepError::Rejected))
                .await
                .unwrap_err()
                .code(),
            MigrationErrorCode::ActionFailed
        );
        let quarantined = runner.state().await.expect("quarantine persists");
        assert!(matches!(quarantined.lifecycle(), MigrationLifecycle::Quarantined { .. }));
        assert_eq!(quarantined.schema_version(), 1);
        assert_eq!(quarantined.data(), &json!({"stable": true}));
        let restored = runner
            .restore_recovery_point()
            .await
            .expect("recovery point restores");
        assert!(matches!(restored.lifecycle(), MigrationLifecycle::Idle));
        assert_eq!(restored.data(), &json!({"stable": true}));
    }

    #[tokio::test]
    async fn validator_failure_never_persists_a_half_migrated_candidate() {
        let store = MemoryStore::default();
        let runner = MigrationRunner::new(&store);
        runner
            .initialize(MigrationState::new(1, json!({"stable": true})).expect("state"))
            .await
            .expect("initial state persists");
        let package = signed_package();
        assert_eq!(
            runner
                .run_with_validator(
                    &package,
                    |_migration, _asset, data| {
                        data["half"] = json!(true);
                        Ok(())
                    },
                    |_migration, _data| Err(MigrationStepError::Rejected),
                )
                .await
                .unwrap_err()
                .code(),
            MigrationErrorCode::ValidationFailed
        );
        let state = runner.state().await.expect("quarantine persists");
        assert_eq!(state.schema_version(), 1);
        assert_eq!(state.data(), &json!({"stable": true}));
    }

    #[tokio::test]
    async fn migration_version_checks_fail_closed_without_running_the_action() {
        let store = MemoryStore::default();
        let runner = MigrationRunner::new(&store);
        runner
            .initialize(MigrationState::new(3, json!({"stable": true})).expect("state"))
            .await
            .expect("initial state persists");
        let package = signed_package();
        let result = runner
            .run(&package, |_migration, _asset, _data| {
                panic!("unsupported schema must not run migration code")
            })
            .await;
        assert_eq!(result.unwrap_err().code(), MigrationErrorCode::VersionUnsupported);
    }

    #[tokio::test]
    async fn crash_mid_migration_recovers_the_prior_state_before_retry() {
        let store = Arc::new(MemoryStore::default());
        let runner = MigrationRunner::new(store.as_ref());
        runner
            .initialize(MigrationState::new(1, json!({"stable": true})).expect("state"))
            .await
            .expect("initial state persists");
        let package = signed_package();
        let crash_store = Arc::clone(&store);
        let crash_package = package.clone();
        let crashed = tokio::spawn(async move {
            MigrationRunner::new(crash_store.as_ref())
                .run(&crash_package, |_migration, _asset, _data| {
                    panic!("injected migration crash")
                })
                .await
        });
        assert!(crashed.await.is_err(), "the rehearsal must inject a process-like crash");

        let recovered = runner
            .run(&package, |_migration, _asset, data| {
                assert_eq!(data, &json!({"stable": true}));
                data["schema"] = json!(2);
                Ok(())
            })
            .await
            .expect("retry recovers and applies atomically");
        assert_eq!(recovered.final_version, 2);
        assert_eq!(runner.state().await.expect("state").data(), &json!({
            "stable": true,
            "schema": 2
        }));
    }
}
