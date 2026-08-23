//! Host-private secret record types.

use std::{error::Error, fmt};

use zeroize::Zeroizing;

use crate::PluginPrincipal;

/// Closed purpose binding for sensitive values.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SecretPurpose {
    /// PIN used only to authorize a simulated payment.
    PaymentPin,
    /// Host-owned device password, distinct from payment authorization.
    DevicePassword,
}

/// A random 256-bit reference. Its debug representation never exposes its value.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct OpaqueHandle(pub(crate) [u8; 32]);

impl OpaqueHandle {
    /// Construct a reference from raw bytes for protocol decoding and adversarial tests.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrow the reference bytes for transient protocol encoding.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Encode the reference for one transient guest event without implementing `Display`.
    #[must_use]
    pub fn to_token(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut token = String::with_capacity(64);
        for byte in self.0 {
            token.push(char::from(HEX[usize::from(byte >> 4)]));
            token.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        token
    }

    /// Decode the exact lowercase hexadecimal protocol representation.
    ///
    /// # Errors
    ///
    /// Returns one non-oracular authorization error for malformed tokens.
    pub fn from_token(token: &str) -> Result<Self, SecretError> {
        if token.len() != 64 {
            return Err(SecretError::authorization_invalid());
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in token.as_bytes().chunks_exact(2).enumerate() {
            let high = decode_hex(pair[0]).ok_or_else(SecretError::authorization_invalid)?;
            let low = decode_hex(pair[1]).ok_or_else(SecretError::authorization_invalid)?;
            bytes[index] = (high << 4) | low;
        }
        Ok(Self(bytes))
    }
}

const fn decode_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

impl fmt::Debug for OpaqueHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OpaqueHandle(REDACTED)")
    }
}

/// Stable non-oracular secret-registry error code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretErrorCode {
    /// A reference could not be authorized, without revealing why.
    AuthorizationInvalid,
    /// Secure random generation failed, so no reference was created.
    EntropyUnavailable,
    /// Host-owned capture metadata was malformed.
    CaptureInvalid,
}

/// Safe secret-lifecycle failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretError {
    code: SecretErrorCode,
}

impl SecretError {
    pub(crate) const fn authorization_invalid() -> Self {
        Self {
            code: SecretErrorCode::AuthorizationInvalid,
        }
    }

    pub(crate) const fn entropy_unavailable() -> Self {
        Self {
            code: SecretErrorCode::EntropyUnavailable,
        }
    }

    pub(crate) const fn capture_invalid() -> Self {
        Self {
            code: SecretErrorCode::CaptureInvalid,
        }
    }

    /// Stable code that does not distinguish absent, expired, reused, or wrongly scoped handles.
    #[must_use]
    pub const fn code(&self) -> SecretErrorCode {
        self.code
    }
}

impl fmt::Display for SecretError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            SecretErrorCode::AuthorizationInvalid => "authorization invalid",
            SecretErrorCode::EntropyUnavailable => "secure entropy unavailable",
            SecretErrorCode::CaptureInvalid => "secret capture invalid",
        })
    }
}

impl Error for SecretError {}

pub(crate) struct SecretRecord {
    pub(crate) bytes: Zeroizing<Vec<u8>>,
    pub(crate) owner: PluginPrincipal,
    pub(crate) purpose: SecretPurpose,
    pub(crate) session_id: String,
    pub(crate) expires_at: std::time::Instant,
}
