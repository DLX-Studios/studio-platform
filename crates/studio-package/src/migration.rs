//! Signed application-migration admission.

use std::collections::BTreeMap;

use serde_json::Value;
use thiserror::Error;

use crate::{
    CanonicalBundleInput, InspectedArchive, IntegrityError, ManifestError, ManifestPolicy,
    ManifestV1, TrustStore, VerifiedIntegrity, parse_manifest, verify_bundle_signature,
};

/// A parsed bundle whose signature and migration assets were admitted together.
///
/// The fields are private on purpose: callers cannot construct an apparently verified package
/// from an unsigned archive. Use [`VerifiedMigrationBundle::admit`] after archive inspection.
#[derive(Clone, Debug)]
pub struct VerifiedMigrationBundle {
    manifest: ManifestV1,
    assets: BTreeMap<String, Vec<u8>>,
    integrity: VerifiedIntegrity,
}

impl VerifiedMigrationBundle {
    /// Admit a structurally inspected, cryptographically signed migration bundle.
    ///
    /// Signature verification is unconditional, including for bundles that would otherwise be
    /// allowed in explicit development mode. Development/unsigned bundles therefore cannot
    /// execute application migrations.
    ///
    /// # Errors
    ///
    /// Returns a safe admission error for invalid manifest data, asset mismatches, or any trust
    /// and signature failure.
    pub fn admit(
        archive: &InspectedArchive,
        manifest_policy: ManifestPolicy,
        trust_store: &TrustStore,
    ) -> Result<Self, MigrationAdmissionError> {
        let manifest = parse_manifest(&archive.manifest, manifest_policy)
            .map_err(MigrationAdmissionError::Manifest)?;
        if manifest.migrations.is_empty() {
            return Err(MigrationAdmissionError::NoMigrations);
        }
        let declared_assets = manifest.assets.clone();
        let archived_assets = archive.assets.keys().cloned().collect::<Vec<_>>();
        if declared_assets != archived_assets {
            return Err(MigrationAdmissionError::AssetMismatch);
        }
        let manifest_value: Value = serde_json::from_slice(&archive.manifest).map_err(|error| {
            MigrationAdmissionError::Manifest(ManifestError::InvalidJson(error.to_string()))
        })?;
        let input = CanonicalBundleInput {
            manifest: manifest_value,
            module_path: manifest.entry.clone(),
            module: archive.module.clone(),
            assets: archive.assets.clone(),
        };
        let integrity = verify_bundle_signature(
            &input,
            &archive.signature,
            &manifest.publisher.id,
            &manifest.publisher.key_id,
            trust_store,
        )
        .map_err(MigrationAdmissionError::Integrity)?;
        Ok(Self {
            manifest,
            assets: archive.assets.clone(),
            integrity,
        })
    }

    /// Parsed manifest whose migration declarations are covered by `integrity`.
    #[must_use]
    pub const fn manifest(&self) -> &ManifestV1 {
        &self.manifest
    }

    /// Exact bytes for a declared migration asset.
    #[must_use]
    pub fn migration_asset(&self, entry: &str) -> Option<&[u8]> {
        self.assets.get(entry).map(Vec::as_slice)
    }

    /// Integrity evidence for the exact signed bundle document.
    #[must_use]
    pub const fn integrity(&self) -> &VerifiedIntegrity {
        &self.integrity
    }
}

/// Safe signed-migration admission failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum MigrationAdmissionError {
    /// Manifest parsing or validation failed.
    #[error("migration manifest invalid")]
    Manifest(#[source] ManifestError),
    /// Migration declarations were absent.
    #[error("signed bundle declares no migrations")]
    NoMigrations,
    /// Declared and archived assets differed.
    #[error("migration assets do not match the signed manifest")]
    AssetMismatch,
    /// Signature or trust verification failed.
    #[error("migration bundle signature verification failed")]
    Integrity(#[source] IntegrityError),
}
