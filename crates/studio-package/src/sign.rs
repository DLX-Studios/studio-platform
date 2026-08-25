//! Raw Ed25519 signing for canonical bundle documents.

use ed25519_dalek::{Signer, SigningKey};
use serde_json::Value;

use crate::{
    CanonicalBundleInput, IntegrityError, canonical_bundle_document, canonical_document_bytes,
};

/// Sign the exact RFC 8785 domain-separated bundle document with a raw 32-byte seed.
///
/// # Errors
///
/// Returns a canonicalization error when the signing document cannot be represented.
pub fn sign_bundle(
    input: &CanonicalBundleInput,
    signing_seed: &[u8; 32],
) -> Result<[u8; 64], IntegrityError> {
    let document = canonical_bundle_document(input)?;
    Ok(SigningKey::from_bytes(signing_seed)
        .sign(&document)
        .to_bytes())
}

/// Sign one standalone canonical JSON document with a raw 32-byte seed.
///
/// The signature covers [`canonical_document_bytes`], the same domain-separated form checked by
/// `verify_document_signature`.
///
/// # Errors
///
/// Returns a canonicalization error when the document cannot be represented.
pub fn sign_document(
    document: &Value,
    signing_seed: &[u8; 32],
) -> Result<[u8; 64], IntegrityError> {
    let bytes = canonical_document_bytes(document)?;
    Ok(SigningKey::from_bytes(signing_seed)
        .sign(&bytes)
        .to_bytes())
}
