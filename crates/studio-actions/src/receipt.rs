//! Immutable structured receipts created from approved payment snapshots.

use std::{error::Error, fmt};

use studio_security::PluginPrincipal;

use crate::{ConfirmedPayment, Money, PaymentOutcome, PaymentResult};

/// Stable structured-receipt failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReceiptErrorCode {
    /// The payment result was not approved.
    PaymentNotApproved,
    /// Lines and exact totals do not agree with the confirmed payment.
    TotalsMismatch,
    /// A line, identity, quantity, or currency was invalid.
    InvalidReceipt,
}

/// Safe receipt construction failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReceiptError(ReceiptErrorCode);

impl ReceiptError {
    const fn new(code: ReceiptErrorCode) -> Self {
        Self(code)
    }

    /// Stable failure code.
    #[must_use]
    pub const fn code(self) -> ReceiptErrorCode {
        self.0
    }
}

impl fmt::Display for ReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.0 {
            ReceiptErrorCode::PaymentNotApproved => "payment was not approved",
            ReceiptErrorCode::TotalsMismatch => "receipt totals do not match confirmation",
            ReceiptErrorCode::InvalidReceipt => "receipt data invalid",
        })
    }
}

impl Error for ReceiptError {}

/// One ordered, exact-money receipt line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceiptLine {
    label: String,
    quantity: u32,
    unit_amount: Money,
    line_total: Money,
}

impl ReceiptLine {
    /// Create a receipt line and compute its exact integer total.
    ///
    /// # Errors
    ///
    /// Rejects invalid labels, zero/excessive quantities, or multiplication overflow.
    pub fn new(
        label: impl Into<String>,
        quantity: u32,
        unit_amount: Money,
    ) -> Result<Self, ReceiptError> {
        let label = label.into();
        if label.is_empty()
            || label.len() > 128
            || label.chars().any(char::is_control)
            || quantity == 0
            || quantity > 999
        {
            return Err(ReceiptError::new(ReceiptErrorCode::InvalidReceipt));
        }
        let line_minor = unit_amount
            .minor()
            .checked_mul(i64::from(quantity))
            .ok_or_else(|| ReceiptError::new(ReceiptErrorCode::InvalidReceipt))?;
        let line_total = Money::new(unit_amount.currency(), line_minor)
            .map_err(|_| ReceiptError::new(ReceiptErrorCode::InvalidReceipt))?;
        Ok(Self {
            label,
            quantity,
            unit_amount,
            line_total,
        })
    }

    /// Product or service label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Purchased quantity.
    #[must_use]
    pub const fn quantity(&self) -> u32 {
        self.quantity
    }

    /// Exact unit amount.
    #[must_use]
    pub const fn unit_amount(&self) -> &Money {
        &self.unit_amount
    }

    /// Exact computed line total.
    #[must_use]
    pub const fn line_total(&self) -> &Money {
        &self.line_total
    }
}

/// Immutable host-created receipt for one approved simulated payment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Receipt {
    id: String,
    owner: PluginPrincipal,
    merchant: String,
    lines: Vec<ReceiptLine>,
    subtotal: Money,
    discount: Money,
    tax: Money,
    total: Money,
    result_reference: String,
    host_timestamp_millis: u64,
}

impl Receipt {
    /// Validate and freeze a receipt from an approved result and confirmation.
    ///
    /// # Errors
    ///
    /// Rejects non-approved payments, empty lines, mixed currencies, arithmetic mismatches, or a
    /// result that differs from the immutable confirmation.
    #[allow(clippy::too_many_arguments)]
    pub fn from_approved(
        owner: PluginPrincipal,
        confirmation: &ConfirmedPayment,
        result: &PaymentResult,
        lines: Vec<ReceiptLine>,
        subtotal: Money,
        discount: Money,
        tax: Money,
    ) -> Result<Self, ReceiptError> {
        if result.outcome() != PaymentOutcome::Approved {
            return Err(ReceiptError::new(ReceiptErrorCode::PaymentNotApproved));
        }
        if lines.is_empty() || result.amount() != confirmation.amount() {
            return Err(ReceiptError::new(ReceiptErrorCode::TotalsMismatch));
        }
        let currency = confirmation.amount().currency();
        if [subtotal.currency(), discount.currency(), tax.currency()]
            .into_iter()
            .any(|candidate| candidate != currency)
            || lines.iter().any(|line| {
                line.unit_amount.currency() != currency || line.line_total.currency() != currency
            })
        {
            return Err(ReceiptError::new(ReceiptErrorCode::TotalsMismatch));
        }
        let line_sum = lines
            .iter()
            .try_fold(0_i64, |sum, line| sum.checked_add(line.line_total.minor()));
        let computed_total = subtotal
            .minor()
            .checked_sub(discount.minor())
            .and_then(|value| value.checked_add(tax.minor()));
        if line_sum != Some(subtotal.minor())
            || computed_total != Some(confirmation.amount().minor())
        {
            return Err(ReceiptError::new(ReceiptErrorCode::TotalsMismatch));
        }
        Ok(Self {
            id: format!("receipt-{}", result.result_reference()),
            owner,
            merchant: confirmation.merchant().to_owned(),
            lines,
            subtotal,
            discount,
            tax,
            total: confirmation.amount().clone(),
            result_reference: result.result_reference().to_owned(),
            host_timestamp_millis: result.host_timestamp_millis(),
        })
    }

    /// Host receipt identity.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Verified merchant identity.
    #[must_use]
    pub fn merchant(&self) -> &str {
        &self.merchant
    }

    /// Ordered receipt lines.
    #[must_use]
    pub fn lines(&self) -> &[ReceiptLine] {
        &self.lines
    }

    /// Exact pre-discount, pre-tax subtotal.
    #[must_use]
    pub const fn subtotal(&self) -> &Money {
        &self.subtotal
    }

    /// Exact discount.
    #[must_use]
    pub const fn discount(&self) -> &Money {
        &self.discount
    }

    /// Exact tax.
    #[must_use]
    pub const fn tax(&self) -> &Money {
        &self.tax
    }

    /// Exact confirmed total.
    #[must_use]
    pub const fn total(&self) -> &Money {
        &self.total
    }

    /// Currency shared by every money field.
    #[must_use]
    pub fn currency(&self) -> &str {
        self.total.currency()
    }

    /// Non-sensitive simulator result reference.
    #[must_use]
    pub fn result_reference(&self) -> &str {
        &self.result_reference
    }

    /// Host time copied from the terminal payment result.
    #[must_use]
    pub const fn host_timestamp_millis(&self) -> u64 {
        self.host_timestamp_millis
    }

    pub(crate) const fn owner(&self) -> &PluginPrincipal {
        &self.owner
    }
}
