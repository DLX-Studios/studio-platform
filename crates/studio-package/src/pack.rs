//! Deterministic `.studio` bundle composition.

use std::collections::BTreeMap;

use serde_json::Value;
use thiserror::Error;

use crate::{
    ArchiveError, ArchiveFiles, ArchivePolicy, CanonicalBundleInput, IntegrityError, ManifestError,
    ManifestPolicy, build_archive, canonicalize_json, parse_manifest, sign_bundle,
};

/// Explicit output trust mode; unsigned output cannot be selected implicitly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackMode {
    /// Sign with a raw Ed25519 32-byte seed.
    Signed([u8; 32]),
    /// Emit an all-zero signature sentinel accepted only by explicit host developer mode.
    DevelopmentUnsigned,
}

/// Complete deterministic packager input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackInput {
    /// Closed manifest JSON.
    pub manifest: Vec<u8>,
    /// Exact WebAssembly module.
    pub module: Vec<u8>,
    /// Declared assets sorted by normalized path.
    pub assets: BTreeMap<String, Vec<u8>>,
    /// Explicit signing/development mode.
    pub mode: PackMode,
}

/// Safe packager failure without source content or key material.
#[derive(Debug, Error)]
pub enum PackError {
    /// Manifest admission failed.
    #[error("bundle manifest invalid")]
    Manifest(#[source] ManifestError),
    /// Signing document construction failed.
    #[error("bundle signing input invalid")]
    Integrity(#[source] IntegrityError),
    /// Deterministic archive construction failed.
    #[error("bundle archive invalid")]
    Archive(#[source] ArchiveError),
    /// Declared asset identities did not exactly match supplied assets.
    #[error("declared bundle assets do not match inputs")]
    AssetMismatch,
}

/// Produce a byte-identical stored-ZIP bundle for identical inputs.
///
/// # Errors
///
/// Returns a safe manifest, asset, signing, or archive error.
pub fn pack_bundle(input: PackInput) -> Result<Vec<u8>, PackError> {
    let manifest =
        parse_manifest(&input.manifest, ManifestPolicy::default()).map_err(PackError::Manifest)?;
    if manifest.assets != input.assets.keys().cloned().collect::<Vec<_>>() {
        return Err(PackError::AssetMismatch);
    }
    let manifest_value: Value = serde_json::from_slice(&input.manifest)
        .map_err(|error| PackError::Manifest(ManifestError::InvalidJson(error.to_string())))?;
    let canonical_manifest = canonicalize_json(&manifest_value).map_err(PackError::Integrity)?;
    let signing_input = CanonicalBundleInput {
        manifest: manifest_value,
        module_path: manifest.entry,
        module: input.module.clone(),
        assets: input.assets.clone(),
    };
    let signature = match input.mode {
        PackMode::Signed(seed) => sign_bundle(&signing_input, &seed)
            .map_err(PackError::Integrity)?
            .to_vec(),
        PackMode::DevelopmentUnsigned => vec![0; 64],
    };
    build_archive(
        &ArchiveFiles {
            manifest: canonical_manifest,
            module: input.module,
            signature,
            assets: input.assets,
        },
        ArchivePolicy::default(),
    )
    .map_err(PackError::Archive)
}
