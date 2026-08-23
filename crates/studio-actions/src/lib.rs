//! Host-mediated checkout, payment, receipt, and printer simulator services.

use std::collections::HashSet;

mod checkout;
mod confirmation;
mod error;
mod idempotency;
mod payment;
mod printer;
mod receipt;

pub use checkout::{Checkout, CheckoutError, Money};
pub use confirmation::{
    ConfirmationError, ConfirmationErrorCode, ConfirmationView, ConfirmedPayment,
    TrustedPaymentConfirmation,
};
pub use error::{PaymentError, PaymentErrorCode};
pub use payment::{
    PaymentAuthorization, PaymentOutcome, PaymentRequest, PaymentResult, PaymentSimulator,
};
pub use printer::{
    PrintError, PrintErrorCode, PrintJob, PrintPreview, PrintPreviewRequest, PrinterSimulator,
};
pub use receipt::{Receipt, ReceiptError, ReceiptErrorCode, ReceiptLine};

/// Bounded host-owned set of non-terminal action identities.
#[derive(Default)]
pub struct PendingActionSet {
    requests: HashSet<String>,
}

impl PendingActionSet {
    /// Admit one pending action under the protocol ceiling.
    ///
    /// # Errors
    ///
    /// Rejects malformed, duplicate, or seventeenth pending requests.
    pub fn begin(&mut self, request_id: impl Into<String>) -> Result<(), PaymentError> {
        let request_id = request_id.into();
        if request_id.is_empty()
            || request_id.len() > 128
            || request_id.chars().any(char::is_control)
            || self.requests.len() >= 16
            || !self.requests.insert(request_id)
        {
            return Err(PaymentError::new(PaymentErrorCode::ActionInvalid));
        }
        Ok(())
    }

    /// Cancel all non-terminal actions and return the number cancelled.
    pub fn cancel_all(&mut self) -> usize {
        let count = self.requests.len();
        self.requests.clear();
        count
    }

    /// Number of non-terminal actions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Whether no action is pending.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }
}
