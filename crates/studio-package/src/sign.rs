//! Raw Ed25519 signing for canonical bundle documents.

use ed25519_dalek::{Signer, SigningKey};

use crate::{CanonicalBundleInput, IntegrityError, canonical_bundle_document};

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
