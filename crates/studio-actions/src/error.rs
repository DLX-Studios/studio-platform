//! Stable safe payment simulator errors.

use std::{error::Error, fmt};

/// Stable payment request failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaymentErrorCode {
    /// A new payment did not include host-verified authorization.
    AuthorizationRequired,
    /// A retained key was reused with different bound inputs.
    IdempotencyConflict,
    /// The terminal registry is full and cannot accept a new unique key.
    IdempotencyCapacityExhausted,
    /// Request metadata was malformed.
    ActionInvalid,
}

/// Non-sensitive simulator admission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaymentError {
    code: PaymentErrorCode,
}

impl PaymentError {
    pub(crate) const fn new(code: PaymentErrorCode) -> Self {
        Self { code }
    }

    /// Stable code suitable for an action result.
    #[must_use]
    pub const fn code(&self) -> PaymentErrorCode {
        self.code
    }
}

impl fmt::Display for PaymentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            PaymentErrorCode::AuthorizationRequired => "authorization required",
            PaymentErrorCode::IdempotencyConflict => "idempotency conflict",
            PaymentErrorCode::IdempotencyCapacityExhausted => "idempotency capacity exhausted",
            PaymentErrorCode::ActionInvalid => "payment action invalid",
        })
    }
}

impl Error for PaymentError {}
