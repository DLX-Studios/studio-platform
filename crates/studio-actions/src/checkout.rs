//! Exact-money checkout state used to create trusted confirmation snapshots.

use std::{error::Error, fmt, time::Instant};

use crate::TrustedPaymentConfirmation;

/// Exact non-negative monetary amount in one explicit ISO-style currency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Money {
    currency: String,
    minor: i64,
}

impl Money {
    /// Construct exact money from integer minor units.
    ///
    /// # Errors
    ///
    /// Rejects negative values and currencies other than three uppercase ASCII letters.
    pub fn new(currency: impl Into<String>, minor: i64) -> Result<Self, CheckoutError> {
        let currency = currency.into();
        if minor < 0
            || currency.len() != 3
            || !currency.bytes().all(|byte| byte.is_ascii_uppercase())
        {
            return Err(CheckoutError);
        }
        Ok(Self { currency, minor })
    }

    /// Currency code associated with the integer value.
    #[must_use]
    pub fn currency(&self) -> &str {
        &self.currency
    }

    /// Exact integer minor-unit value.
    #[must_use]
    pub const fn minor(&self) -> i64 {
        self.minor
    }
}

/// Invalid checkout identity or money transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckoutError;

impl fmt::Display for CheckoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("checkout invalid")
    }
}

impl Error for CheckoutError {}

/// Mutable cart-facing state. Confirmation copies its security-sensitive values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Checkout {
    session_id: String,
    merchant: String,
    publisher: String,
    total: Money,
}

impl Checkout {
    /// Create a checkout from host-verified display identities and exact money.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, or control-bearing identities.
    pub fn new(
        session_id: impl Into<String>,
        merchant: impl Into<String>,
        publisher: impl Into<String>,
        total: Money,
    ) -> Result<Self, CheckoutError> {
        let session_id = session_id.into();
        let merchant = merchant.into();
        let publisher = publisher.into();
        if !valid_text(&session_id) || !valid_text(&merchant) || !valid_text(&publisher) {
            return Err(CheckoutError);
        }
        Ok(Self {
            session_id,
            merchant,
            publisher,
            total,
        })
    }

    /// Begin a host-owned confirmation bound to the current values and authorization expiry.
    ///
    /// # Errors
    ///
    /// This validated checkout cannot normally fail; the result keeps the construction boundary
    /// explicit for future confirmation policy checks.
    pub fn begin_confirmation(
        &self,
        authorization_expires_at: Instant,
    ) -> Result<TrustedPaymentConfirmation, CheckoutError> {
        Ok(TrustedPaymentConfirmation::new(
            self.session_id.clone(),
            self.merchant.clone(),
            self.publisher.clone(),
            self.total.clone(),
            authorization_expires_at,
        ))
    }

    /// Replace the cart total before confirmation while preserving its currency.
    ///
    /// # Errors
    ///
    /// Rejects a currency change within one checkout session.
    pub fn set_total(&mut self, total: Money) -> Result<(), CheckoutError> {
        if total.currency != self.total.currency {
            return Err(CheckoutError);
        }
        self.total = total;
        Ok(())
    }

    /// Current mutable cart total.
    #[must_use]
    pub const fn total(&self) -> &Money {
        &self.total
    }
}

fn valid_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
