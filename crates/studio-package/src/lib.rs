//! Deterministic Studio bundle parsing, verification, and packaging boundaries.

mod archive;
mod error;
mod integrity;
mod manifest;
mod migration;
mod pack;
mod provider;
mod sign;
mod template;
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
    AI_PROVIDER_ID, AI_PROVIDER_VERSION, APPLE_ALLOWED_SCOPES, APPLE_PROVIDER_ID,
    APPLE_PROVIDER_VERSION, DECLARED_OIDC_ALLOWED_SCOPES, DECLARED_OIDC_DESCRIPTOR_VERSION,
    DECLARED_OIDC_ID_PREFIX, DISCORD_ALLOWED_SCOPES, DISCORD_PROVIDER_ID, DISCORD_PROVIDER_VERSION,
    GITHUB_PROVIDER_ID, GITHUB_PROVIDER_VERSION, GITLAB_ALLOWED_SCOPES, GITLAB_PROVIDER_ID,
    GITLAB_PROVIDER_VERSION, GOOGLE_ALLOWED_SCOPES, GOOGLE_PROVIDER_ID, GOOGLE_PROVIDER_VERSION,
    LINKEDIN_ALLOWED_SCOPES, LINKEDIN_PROVIDER_ID, LINKEDIN_PROVIDER_VERSION,
    MICROSOFT_ALLOWED_SCOPES, MICROSOFT_PROVIDER_ID, MICROSOFT_PROVIDER_VERSION,
    PROVIDER_DESCRIPTOR_SCHEMA_VERSION, ProviderAdmissionError, ProviderAdmissionErrorCode,
    ProviderAdmissionPlan, ProviderCredentialPolicy, ProviderDescriptor, ProviderDescriptorState,
    ProviderRegistry, ProviderRouteDescriptor, ResolvedProvider, is_declared_oidc_id,
    maintained_oauth_descriptors,
};
pub use sign::{sign_bundle, sign_document};
pub use template::{InspectedTemplate, TemplateFiles, inspect_template, pack_template};
pub use trust::{
    ProvisionedPublisherKey, PublisherKeyIdentity, TRUST_STORE_PATH_ENV, TrustSnapshot,
    TrustSnapshotEvidence, TrustStore, TrustStoreError, TrustStoreErrorCode, TrustedPublisherKey,
};
