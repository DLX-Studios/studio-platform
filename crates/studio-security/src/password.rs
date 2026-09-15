//! Host-owned salted password verification.
//!
//! Passwords are accepted only for the duration of hashing or verification. A
//! [`PasswordVerifier`] contains a random salt and a derived digest, never the
//! password itself, and is safe to persist in the host's local identity record.

use std::{error::Error, fmt};

use getrandom::fill;
use pbkdf2::pbkdf2_hmac;
use serde::{Deserialize, Serialize};
use sha2_10::Sha256;
use zeroize::Zeroize;

const SALT_BYTES: usize = 16;
const DIGEST_BYTES: usize = 32;
/// OWASP's current PBKDF2-HMAC-SHA-256 work-factor recommendation for a
/// password verifier. The value is deliberately stored with each verifier so
/// a future migration can increase it without changing old records.
const DEFAULT_ITERATIONS: u32 = 600_000;
const MAX_PASSWORD_BYTES: usize = 4096;

/// Stable rejection family for password verifier construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PasswordErrorCode {
    /// The supplied password was empty or too large for the host entry surface.
    InvalidInput,
    /// The operating system could not provide a cryptographically secure salt.
    EntropyUnavailable,
}

/// Password verifier construction failure without retaining password material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PasswordError {
    code: PasswordErrorCode,
}

impl PasswordError {
    const fn invalid_input() -> Self {
        Self {
            code: PasswordErrorCode::InvalidInput,
        }
    }

    const fn entropy_unavailable() -> Self {
        Self {
            code: PasswordErrorCode::EntropyUnavailable,
        }
    }

    /// Stable error code suitable for host diagnostics.
    #[must_use]
    pub const fn code(self) -> PasswordErrorCode {
        self.code
    }
}

impl fmt::Display for PasswordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            PasswordErrorCode::InvalidInput => "password input invalid",
            PasswordErrorCode::EntropyUnavailable => "secure entropy unavailable",
        })
    }
}

impl Error for PasswordError {}

/// A serializable, salted PBKDF2-HMAC-SHA-256 password verifier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasswordVerifier {
    algorithm: String,
    iterations: u32,
    salt: [u8; SALT_BYTES],
    digest: [u8; DIGEST_BYTES],
}

impl PasswordVerifier {
    /// Derive a verifier from a password without retaining the password.
    ///
    /// # Errors
    ///
    /// Returns a closed error for empty/oversized input or unavailable secure
    /// randomness. Password bytes are never formatted or serialized.
    pub fn derive(password: &[u8]) -> Result<Self, PasswordError> {
        validate_password(password)?;
        let mut salt = [0; SALT_BYTES];
        fill(&mut salt).map_err(|_| PasswordError::entropy_unavailable())?;
        Ok(Self::from_salt(password, salt, DEFAULT_ITERATIONS))
    }

    /// Verify a password against the stored salt and work factor.
    #[must_use]
    pub fn verify(&self, password: &[u8]) -> bool {
        if validate_password(password).is_err()
            || self.algorithm != "pbkdf2-sha256"
            || self.iterations == 0
        {
            return false;
        }
        let mut derived = [0; DIGEST_BYTES];
        pbkdf2_hmac::<Sha256>(password, &self.salt, self.iterations, &mut derived);
        let matches = constant_time_eq(&derived, &self.digest);
        derived.zeroize();
        matches
    }

    /// Name of the derivation scheme used by this verifier.
    #[must_use]
    pub fn algorithm(&self) -> &str {
        &self.algorithm
    }

    /// Number of PBKDF2 iterations used by this verifier.
    #[must_use]
    pub const fn iterations(&self) -> u32 {
        self.iterations
    }

    /// Borrow the public salt for migration and diagnostics.
    #[must_use]
    pub const fn salt(&self) -> &[u8; SALT_BYTES] {
        &self.salt
    }

    fn from_salt(password: &[u8], salt: [u8; SALT_BYTES], iterations: u32) -> Self {
        let mut digest = [0; DIGEST_BYTES];
        pbkdf2_hmac::<Sha256>(password, &salt, iterations, &mut digest);
        Self {
            algorithm: "pbkdf2-sha256".to_owned(),
            iterations,
            salt,
            digest,
        }
    }
}

fn validate_password(password: &[u8]) -> Result<(), PasswordError> {
    if password.is_empty() || password.len() > MAX_PASSWORD_BYTES {
        Err(PasswordError::invalid_input())
    } else {
        Ok(())
    }
}

fn constant_time_eq(left: &[u8; DIGEST_BYTES], right: &[u8; DIGEST_BYTES]) -> bool {
    let mut difference = 0_u8;
    for (&left, &right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_never_contains_the_password_and_round_trips() {
        let verifier = PasswordVerifier::derive(b"correct horse battery staple")
            .expect("salted verifier derives");
        assert!(verifier.verify(b"correct horse battery staple"));
        assert!(!verifier.verify(b"wrong password"));
        assert_eq!(verifier.algorithm(), "pbkdf2-sha256");
        assert_eq!(verifier.iterations(), DEFAULT_ITERATIONS);
        assert!(!format!("{verifier:?}").contains("correct horse"));
        let encoded = serde_json::to_string(&verifier).expect("verifier serializes");
        assert!(!encoded.contains("correct horse"));
        let decoded: PasswordVerifier = serde_json::from_str(&encoded).expect("verifier decodes");
        assert!(decoded.verify(b"correct horse battery staple"));
    }

    #[test]
    fn invalid_passwords_fail_closed() {
        assert_eq!(
            PasswordVerifier::derive(&[]).unwrap_err().code(),
            PasswordErrorCode::InvalidInput
        );
        let verifier = PasswordVerifier::derive(b"valid password").expect("derives");
        assert!(!verifier.verify(&[]));
    }
}
