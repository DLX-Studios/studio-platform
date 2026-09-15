//! Host-owned payment confirmation and immutable execution snapshot.

use std::{error::Error, fmt, time::Instant};

use crate::Money;

/// Stable trusted-confirmation failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfirmationErrorCode {
    /// The operator explicitly cancelled the trusted surface.
    ConfirmationCancelled,
    /// Authorization expired before confirmation was accepted.
    AuthorizationExpired,
    /// The confirmation was already accepted.
    ConfirmationComplete,
}

/// Non-sensitive confirmation-state failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfirmationError {
    code: ConfirmationErrorCode,
}

impl ConfirmationError {
    const fn new(code: ConfirmationErrorCode) -> Self {
        Self { code }
    }

    /// Stable code suitable for a host-owned surface.
    #[must_use]
    pub const fn code(&self) -> ConfirmationErrorCode {
        self.code
    }
}

impl fmt::Display for ConfirmationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            ConfirmationErrorCode::ConfirmationCancelled => "confirmation cancelled",
            ConfirmationErrorCode::AuthorizationExpired => "authorization expired",
            ConfirmationErrorCode::ConfirmationComplete => "confirmation already completed",
        })
    }
}

impl Error for ConfirmationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfirmationState {
    AwaitingOperator,
    Cancelled,
    Expired,
    Confirmed,
}

/// Read-only content rendered by the trusted Studio confirmation surface.
#[derive(Clone, Copy, Debug)]
pub struct ConfirmationView<'a> {
    /// Verified merchant display identity.
    pub merchant: &'a str,
    /// Verified publisher display identity.
    pub publisher: &'a str,
    /// Exact frozen amount and currency.
    pub amount: &'a Money,
    /// Explicit reminder that no real provider is contacted.
    pub simulator_status: &'static str,
}

/// Host-owned confirmation state created from a checkout snapshot.
#[derive(Clone, Debug)]
pub struct TrustedPaymentConfirmation {
    session_id: String,
    merchant: String,
    publisher: String,
    amount: Money,
    authorization_expires_at: Instant,
    state: ConfirmationState,
}

impl TrustedPaymentConfirmation {
    pub(crate) const fn new(
        session_id: String,
        merchant: String,
        publisher: String,
        amount: Money,
        authorization_expires_at: Instant,
    ) -> Self {
        Self {
            session_id,
            merchant,
            publisher,
            amount,
            authorization_expires_at,
            state: ConfirmationState::AwaitingOperator,
        }
    }

    /// Content that Studio, rather than the plugin, renders for operator review.
    #[must_use]
    pub fn view(&self) -> ConfirmationView<'_> {
        ConfirmationView {
            merchant: &self.merchant,
            publisher: &self.publisher,
            amount: &self.amount,
            simulator_status: "SIMULATOR — no real charge",
        }
    }

    /// Accept the trusted surface and return an immutable execution snapshot.
    ///
    /// # Errors
    ///
    /// Rejects cancelled, expired, or already-completed confirmation state.
    pub fn confirm(&mut self, now: Instant) -> Result<ConfirmedPayment, ConfirmationError> {
        match self.state {
            ConfirmationState::Cancelled => {
                return Err(ConfirmationError::new(
                    ConfirmationErrorCode::ConfirmationCancelled,
                ));
            }
            ConfirmationState::Expired => {
                return Err(ConfirmationError::new(
                    ConfirmationErrorCode::AuthorizationExpired,
                ));
            }
            ConfirmationState::Confirmed => {
                return Err(ConfirmationError::new(
                    ConfirmationErrorCode::ConfirmationComplete,
                ));
            }
            ConfirmationState::AwaitingOperator => {}
        }
        if now >= self.authorization_expires_at {
            self.state = ConfirmationState::Expired;
            return Err(ConfirmationError::new(
                ConfirmationErrorCode::AuthorizationExpired,
            ));
        }
        self.state = ConfirmationState::Confirmed;
        Ok(ConfirmedPayment {
            session_id: self.session_id.clone(),
            merchant: self.merchant.clone(),
            publisher: self.publisher.clone(),
            amount: self.amount.clone(),
        })
    }

    /// Cancel a pending trusted confirmation.
    ///
    /// # Errors
    ///
    /// Rejects an already terminal confirmation.
    pub fn cancel(&mut self) -> Result<(), ConfirmationError> {
        if self.state != ConfirmationState::AwaitingOperator {
            return Err(ConfirmationError::new(
                ConfirmationErrorCode::ConfirmationComplete,
            ));
        }
        self.state = ConfirmationState::Cancelled;
        Ok(())
    }
}

/// Immutable values authorized by the operator for payment execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfirmedPayment {
    session_id: String,
    merchant: String,
    publisher: String,
    amount: Money,
}

impl ConfirmedPayment {
    /// Bound checkout session.
    #[must_use]
    pub fn checkout_session_id(&self) -> &str {
        &self.session_id
    }

    /// Verified merchant identity.
    #[must_use]
    pub fn merchant(&self) -> &str {
        &self.merchant
    }

    /// Verified publisher identity.
    #[must_use]
    pub fn publisher(&self) -> &str {
        &self.publisher
    }

    /// Exact confirmed amount and currency.
    #[must_use]
    pub const fn amount(&self) -> &Money {
        &self.amount
    }
}
