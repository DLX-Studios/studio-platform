//! Canonical digest documents and strict signed-bundle verification.

use std::collections::BTreeMap;

use ed25519_dalek::Signature;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::TrustStore;

const SIGNATURE_DOMAIN: &str = "studio.bundle.signature.v1";

/// Exact logical bundle inputs covered by a publisher signature.
#[derive(Clone, Debug, PartialEq)]
pub struct CanonicalBundleInput {
    /// Complete parsed manifest value.
    pub manifest: Value,
    /// Normalized archive path of the executable module.
    pub module_path: String,
    /// Exact executable module bytes.
    pub module: Vec<u8>,
    /// Normalized asset paths and exact bytes in lexical order.
    pub assets: BTreeMap<String, Vec<u8>>,
}

/// Successful verification evidence retained for later audit stages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedIntegrity {
    /// Exact RFC 8785 document whose signature was checked.
    pub signed_document: Vec<u8>,
    /// SHA-256 of `signed_document`.
    pub document_sha256: [u8; 32],
}

/// Stable signed-bundle rejection family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrityErrorCode {
    /// Canonical JSON serialization failed.
    CanonicalizationInvalid,
    /// Provisioned trust-store contents are malformed or ambiguous.
    TrustConfigurationInvalid,
    /// Publisher/key lookup failed or resolved to a disabled key.
    TrustInvalid,
    /// Signature encoding or cryptographic verification failed.
    SignatureInvalid,
}

/// Detailed integrity verification failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum IntegrityError {
    /// RFC 8785 serialization rejected the supplied JSON value.
    #[error("canonical JSON serialization failed: {0}")]
    CanonicalizationInvalid(String),
    /// Host trust configuration is invalid.
    #[error("invalid trust-store configuration")]
    TrustConfigurationInvalid,
    /// The requested publisher key is not currently trusted.
    #[error("publisher trust verification failed")]
    TrustInvalid,
    /// The raw signature is malformed or does not verify.
    #[error("bundle signature verification failed")]
    SignatureInvalid,
}

impl IntegrityError {
    /// Return the stable family for this detailed integrity rejection.
    #[must_use]
    pub const fn code(&self) -> IntegrityErrorCode {
        match self {
            Self::CanonicalizationInvalid(_) => IntegrityErrorCode::CanonicalizationInvalid,
            Self::TrustConfigurationInvalid => IntegrityErrorCode::TrustConfigurationInvalid,
            Self::TrustInvalid => IntegrityErrorCode::TrustInvalid,
            Self::SignatureInvalid => IntegrityErrorCode::SignatureInvalid,
        }
    }
}

/// Serialize a JSON value using RFC 8785 JSON Canonicalization Scheme rules.
///
/// # Errors
///
/// Returns [`IntegrityError::CanonicalizationInvalid`] for values JCS cannot represent.
pub fn canonicalize_json(value: &Value) -> Result<Vec<u8>, IntegrityError> {
    serde_jcs::to_vec(value)
        .map_err(|error| IntegrityError::CanonicalizationInvalid(error.to_string()))
}

/// Construct the domain-separated, canonical digest document covered by the signature.
///
/// Raw module and asset content is represented by its length and SHA-256 digest, while the full
/// manifest remains present so all declared policy inputs are signed.
///
/// # Errors
///
/// Returns [`IntegrityError::CanonicalizationInvalid`] if canonical serialization fails.
pub fn canonical_bundle_document(input: &CanonicalBundleInput) -> Result<Vec<u8>, IntegrityError> {
    let assets = input
        .assets
        .iter()
        .map(|(path, bytes)| {
            json!({
                "path": path,
                "length": bytes.len(),
                "sha256": hex_sha256(bytes),
            })
        })
        .collect::<Vec<_>>();
    canonicalize_json(&json!({
        "domain": SIGNATURE_DOMAIN,
        "manifest": input.manifest,
        "module": {
            "path": input.module_path,
            "length": input.module.len(),
            "sha256": hex_sha256(&input.module),
        },
        "assets": assets,
    }))
}

/// Verify one exact raw Ed25519 signature against a currently enabled provisioned key.
///
/// # Errors
///
/// Returns a stable [`IntegrityError`] for canonicalization, trust, encoding, or verification
/// failures. Unknown, mismatched, and disabled trust records intentionally share one error.
pub fn verify_bundle_signature(
    input: &CanonicalBundleInput,
    signature: &[u8],
    publisher_id: &str,
    key_id: &str,
    trust_store: &TrustStore,
) -> Result<VerifiedIntegrity, IntegrityError> {
    let verifying_key = trust_store.enabled_key(publisher_id, key_id)?;
    let raw_signature: &[u8; 64] = signature
        .try_into()
        .map_err(|_| IntegrityError::SignatureInvalid)?;
    let signature = Signature::from_bytes(raw_signature);
    let signed_document = canonical_bundle_document(input)?;
    verifying_key
        .verify_strict(&signed_document, &signature)
        .map_err(|_| IntegrityError::SignatureInvalid)?;
    Ok(VerifiedIntegrity {
        document_sha256: Sha256::digest(&signed_document).into(),
        signed_document,
    })
}

fn hex_sha256(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing into a String cannot fail");
            output
        })
}
