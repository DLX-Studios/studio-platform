//! Composition of host-owned PIN capture, confirmation, and offline payment execution.

use std::{
    error::Error,
    fmt,
    time::{Duration, Instant},
};

use studio_actions::{
    Checkout, CheckoutError, ConfirmationError, ConfirmedPayment, Money, PaymentAuthorization,
    PaymentError, PaymentRequest, PaymentResult, PaymentSimulator, TrustedPaymentConfirmation,
};
use studio_components::{HostSecretInput, SecretInputError, SecretInputErrorCode};
use studio_protocol::HostEvent;
use studio_security::{PluginPrincipal, SecretPurpose, SecretRegistry};
use studio_ui::InstanceId;

/// Stable protected-payment orchestration failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedPaymentErrorCode {
    /// Caller does not own the host session.
    OwnerMismatch,
    /// No captured PIN is ready.
    AuthorizationRequired,
    /// Supplied authorization could not be resolved without revealing why.
    AuthorizationInvalid,
    /// Trusted confirmation has not begun or cannot complete.
    ConfirmationInvalid,
    /// Simulator or idempotency admission failed.
    PaymentInvalid,
    /// Checkout input was malformed.
    CheckoutInvalid,
}

/// Safe host-owned protected-payment error.
#[derive(Debug)]
pub struct ProtectedPaymentError {
    code: ProtectedPaymentErrorCode,
}

impl ProtectedPaymentError {
    const fn new(code: ProtectedPaymentErrorCode) -> Self {
        Self { code }
    }

    /// Stable code safe for operator and guest-facing results.
    #[must_use]
    pub const fn code(&self) -> ProtectedPaymentErrorCode {
        self.code
    }
}

impl fmt::Display for ProtectedPaymentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            ProtectedPaymentErrorCode::OwnerMismatch => "payment owner mismatch",
            ProtectedPaymentErrorCode::AuthorizationRequired => "authorization required",
            ProtectedPaymentErrorCode::AuthorizationInvalid => "authorization invalid",
            ProtectedPaymentErrorCode::ConfirmationInvalid => "confirmation invalid",
            ProtectedPaymentErrorCode::PaymentInvalid => "payment invalid",
            ProtectedPaymentErrorCode::CheckoutInvalid => "checkout invalid",
        })
    }
}

impl Error for ProtectedPaymentError {}

/// Owned content displayed by the trusted native confirmation surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedConfirmationView {
    /// Verified merchant identity.
    pub merchant: String,
    /// Verified publisher identity.
    pub publisher: String,
    /// Frozen exact payment money.
    pub amount: Money,
    /// Explicit offline simulator status.
    pub simulator_status: &'static str,
}

/// One instance-owned protected checkout/payment lifecycle.
pub struct ProtectedPaymentSession {
    owner: InstanceId,
    checkout: Checkout,
    secret_input: HostSecretInput,
    secrets: SecretRegistry,
    confirmation: Option<TrustedPaymentConfirmation>,
    confirmed: Option<ConfirmedPayment>,
    simulator: PaymentSimulator,
}

impl ProtectedPaymentSession {
    /// Compose the host services for one principal and checkout session.
    ///
    /// # Errors
    ///
    /// Rejects invalid host-owned input metadata.
    pub fn new(
        owner: InstanceId,
        principal: PluginPrincipal,
        checkout: Checkout,
    ) -> Result<Self, ProtectedPaymentError> {
        let secret_input = HostSecretInput::new(
            owner.clone(),
            principal,
            "payment-pin",
            SecretPurpose::PaymentPin,
            "checkout-1",
        )
        .map_err(map_secret_error)?;
        Ok(Self {
            owner,
            checkout,
            secret_input,
            secrets: SecretRegistry::new(),
            confirmation: None,
            confirmed: None,
            simulator: PaymentSimulator::new(),
        })
    }

    /// Capture PIN bytes solely in host memory and return a safe readiness event.
    ///
    /// # Errors
    ///
    /// Rejects owner or capture failures without including the PIN.
    pub fn capture_pin(
        &mut self,
        owner: &InstanceId,
        pin: &[u8],
        now: Instant,
    ) -> Result<HostEvent, ProtectedPaymentError> {
        self.secret_input
            .capture_at(owner, &mut self.secrets, pin, now)
            .map_err(map_secret_error)
    }

    /// Begin trusted confirmation for the current cart snapshot.
    ///
    /// # Errors
    ///
    /// Requires exact ownership and a currently ready PIN reference.
    pub fn begin_confirmation(
        &mut self,
        owner: &InstanceId,
        now: Instant,
    ) -> Result<ProtectedConfirmationView, ProtectedPaymentError> {
        self.check_owner(owner)?;
        if !self.secret_input.snapshot().ready {
            return Err(ProtectedPaymentError::new(
                ProtectedPaymentErrorCode::AuthorizationRequired,
            ));
        }
        let confirmation = self
            .checkout
            .begin_confirmation(now + Duration::from_mins(2))
            .map_err(map_checkout_error)?;
        let view = confirmation.view();
        let display = ProtectedConfirmationView {
            merchant: view.merchant.to_owned(),
            publisher: view.publisher.to_owned(),
            amount: view.amount.clone(),
            simulator_status: view.simulator_status,
        };
        self.confirmation = Some(confirmation);
        Ok(display)
    }

    /// Confirm, consume the submitted opaque authorization, and execute the frozen charge.
    ///
    /// # Errors
    ///
    /// Rejects invalid ownership, authorization, confirmation state, or simulator admission.
    pub fn confirm_and_charge(
        &mut self,
        owner: &InstanceId,
        authorization_ref: &str,
        idempotency_key: &str,
        now: Instant,
        host_timestamp_millis: u64,
    ) -> Result<PaymentResult, ProtectedPaymentError> {
        self.check_owner(owner)?;
        let authorization = self
            .secret_input
            .consume_reference_at(owner, &mut self.secrets, authorization_ref, now, |_| {
                PaymentAuthorization::host_verified()
            })
            .map_err(map_secret_error)?;
        let confirmation = self.confirmation.as_mut().ok_or_else(|| {
            ProtectedPaymentError::new(ProtectedPaymentErrorCode::ConfirmationInvalid)
        })?;
        let confirmed = confirmation.confirm(now).map_err(map_confirmation_error)?;
        let request = PaymentRequest::new(
            idempotency_key,
            self.principal().clone(),
            confirmed.clone(),
            Some(authorization),
        )
        .map_err(map_payment_error)?;
        let result = self
            .simulator
            .charge(request, host_timestamp_millis)
            .map_err(map_payment_error)?;
        self.confirmed = Some(confirmed);
        Ok(result)
    }

    /// Replay the retained terminal result without requiring a consumed authorization.
    ///
    /// # Errors
    ///
    /// Requires one previously confirmed snapshot and a matching retained key.
    pub fn replay(
        &mut self,
        idempotency_key: &str,
        host_timestamp_millis: u64,
    ) -> Result<PaymentResult, ProtectedPaymentError> {
        let confirmed = self.confirmed.clone().ok_or_else(|| {
            ProtectedPaymentError::new(ProtectedPaymentErrorCode::ConfirmationInvalid)
        })?;
        let request =
            PaymentRequest::new(idempotency_key, self.principal().clone(), confirmed, None)
                .map_err(map_payment_error)?;
        self.simulator
            .charge(request, host_timestamp_millis)
            .map_err(map_payment_error)
    }

    /// Mutate current cart state without affecting an in-progress confirmation snapshot.
    ///
    /// # Errors
    ///
    /// Rejects currency changes.
    pub fn set_cart_total(&mut self, total: Money) -> Result<(), ProtectedPaymentError> {
        self.checkout.set_total(total).map_err(map_checkout_error)
    }

    /// Current mutable cart total.
    #[must_use]
    pub const fn cart_total(&self) -> &Money {
        self.checkout.total()
    }

    /// Count of retained terminal simulator records.
    #[must_use]
    pub fn transaction_count(&self) -> usize {
        self.simulator.transaction_count()
    }

    /// Immutable confirmed values retained after terminal execution.
    #[must_use]
    pub const fn confirmed_snapshot(&self) -> Option<&ConfirmedPayment> {
        self.confirmed.as_ref()
    }

    /// Exact verified principal bound to this session.
    #[must_use]
    pub fn principal(&self) -> &PluginPrincipal {
        self.secret_input.principal()
    }

    /// Offline simulator invariant.
    #[must_use]
    pub const fn network_attempts(&self) -> usize {
        self.simulator.network_attempts()
    }

    fn check_owner(&self, owner: &InstanceId) -> Result<(), ProtectedPaymentError> {
        if owner == &self.owner {
            Ok(())
        } else {
            Err(ProtectedPaymentError::new(
                ProtectedPaymentErrorCode::OwnerMismatch,
            ))
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Result::map_err passes the owned source error"
)]
fn map_secret_error(error: SecretInputError) -> ProtectedPaymentError {
    let code = match error.code() {
        SecretInputErrorCode::OwnerMismatch => ProtectedPaymentErrorCode::OwnerMismatch,
        SecretInputErrorCode::AuthorizationInvalid => {
            ProtectedPaymentErrorCode::AuthorizationInvalid
        }
        SecretInputErrorCode::InputInvalid => ProtectedPaymentErrorCode::CheckoutInvalid,
    };
    ProtectedPaymentError::new(code)
}

fn map_checkout_error(_error: CheckoutError) -> ProtectedPaymentError {
    ProtectedPaymentError::new(ProtectedPaymentErrorCode::CheckoutInvalid)
}

fn map_confirmation_error(_error: ConfirmationError) -> ProtectedPaymentError {
    ProtectedPaymentError::new(ProtectedPaymentErrorCode::ConfirmationInvalid)
}

fn map_payment_error(_error: PaymentError) -> ProtectedPaymentError {
    ProtectedPaymentError::new(ProtectedPaymentErrorCode::PaymentInvalid)
}
