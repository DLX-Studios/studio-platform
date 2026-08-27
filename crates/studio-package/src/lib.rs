//! Deterministic Studio bundle parsing, verification, and packaging boundaries.

mod archive;
mod error;
mod integrity;
mod manifest;
mod migration;
mod pack;
mod provider;
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
    BundleLimits, Capability, IntegrationReference, ManifestPolicy, ManifestV1,
    MigrationDeclaration, Publisher, SecretDeclaration, parse_manifest,
};
pub use migration::{MigrationAdmissionError, VerifiedMigrationBundle};
pub use pack::{PackError, PackInput, PackMode, pack_bundle};
pub use provider::{
    AI_PROVIDER_ID, AI_PROVIDER_VERSION, GITHUB_PROVIDER_ID, GITHUB_PROVIDER_VERSION,
    PROVIDER_DESCRIPTOR_SCHEMA_VERSION, ProviderAdmissionError, ProviderAdmissionErrorCode,
    ProviderAdmissionPlan, ProviderCredentialPolicy, ProviderDescriptor, ProviderDescriptorState,
    ProviderRegistry, ProviderRouteDescriptor, ResolvedProvider,
};
pub use sign::{sign_bundle, sign_document};
pub use trust::{
    ProvisionedPublisherKey, PublisherKeyIdentity, TrustSnapshot, TrustSnapshotEvidence,
    TrustStore, TrustStoreError, TrustStoreErrorCode, TrustedPublisherKey, TRUST_STORE_PATH_ENV,
};
