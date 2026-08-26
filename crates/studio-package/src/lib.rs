//! Deterministic Studio bundle parsing, verification, and packaging boundaries.

mod archive;
mod error;
mod integrity;
mod manifest;
mod pack;
mod sign;
mod trust;

pub use archive::{
    ArchiveError, ArchiveErrorCode, ArchiveFiles, ArchivePolicy, InspectedArchive, build_archive,
    inspect_archive,
};
pub use error::{ManifestError, ManifestErrorCode};
pub use integrity::{
    CanonicalBundleInput, IntegrityError, IntegrityErrorCode, VerifiedIntegrity,
    canonical_bundle_document, canonical_document_bytes, canonicalize_json,
    verify_bundle_signature, verify_document_signature,
};
pub use manifest::{
    BundleLimits, Capability, ManifestPolicy, ManifestV1, Publisher, SecretDeclaration,
    parse_manifest,
};
pub use pack::{PackError, PackInput, PackMode, pack_bundle};
pub use sign::{sign_bundle, sign_document};
pub use trust::{TrustStore, TrustedPublisherKey};
