//! Checkout action dispatch across trusted input, simulator, receipt, and preview services.

use std::time::Instant;

use studio_actions::{
    Checkout, Money, PaymentOutcome, PaymentResult, PrintError, PrintPreviewRequest,
    PrinterSimulator, Receipt, ReceiptError, ReceiptLine,
};
use studio_security::PluginPrincipal;
use studio_ui::InstanceId;
use thiserror::Error;

use crate::{
    CheckoutRouter, HostPreferences, PrintPreviewSurface, ProtectedConfirmationView,
    ProtectedPaymentError, ProtectedPaymentSession,
};

/// Host-owned checkout orchestration failure.
#[derive(Debug, Error)]
pub enum NativeCheckoutError {
    /// Trusted input, confirmation, or payment execution failed.
    #[error(transparent)]
    Payment(#[from] ProtectedPaymentError),
    /// Receipt construction failed.
    #[error(transparent)]
    Receipt(#[from] ReceiptError),
    /// Structured print-preview admission failed.
    #[error(transparent)]
    Print(#[from] PrintError),
    /// Native route transition failed.
    #[error(transparent)]
    Navigation(#[from] studio_navigation::StackError),
    /// A required approved result or confirmation snapshot is unavailable.
    #[error("checkout action state invalid")]
    StateInvalid,
}

/// Native shell state joining plugin UI with trusted checkout overlays and host actions.
pub struct NativeCheckoutShell {
    owner: InstanceId,
    router: CheckoutRouter,
    payment: ProtectedPaymentSession,
    printer: PrinterSimulator,
    result: Option<PaymentResult>,
    receipt: Option<Receipt>,
}

impl NativeCheckoutShell {
    /// Create one instance-owned shell from verified identity and checkout data.
    ///
    /// # Errors
    ///
    /// Rejects invalid protected-input metadata or initial native navigation state.
    pub fn new(
        owner: InstanceId,
        principal: PluginPrincipal,
        checkout: Checkout,
        reduced_motion: bool,
    ) -> Result<Self, NativeCheckoutError> {
        let stack_owner = studio_navigation::StackOwner::new(*principal.instance_id());
        let router = CheckoutRouter::new(stack_owner, HostPreferences::new(reduced_motion))?;
        let payment = ProtectedPaymentSession::new(owner.clone(), principal, checkout)?;
        Ok(Self {
            owner,
            router,
            payment,
            printer: PrinterSimulator::new(),
            result: None,
            receipt: None,
        })
    }

    /// Capture a host-owned PIN and return only its opaque authorization reference.
    ///
    /// # Errors
    ///
    /// Returns a safe payment/state error without exposing captured bytes.
    pub fn capture_pin(
        &mut self,
        owner: &InstanceId,
        pin: &[u8],
        now: Instant,
    ) -> Result<String, NativeCheckoutError> {
        let event = self.payment.capture_pin(owner, pin, now)?;
        serde_json::to_value(event)
            .ok()
            .and_then(|value| {
                value["payload"]["payload"]["authorization_ref"]
                    .as_str()
                    .map(str::to_owned)
            })
            .ok_or(NativeCheckoutError::StateInvalid)
    }

    /// Show the trusted confirmation and enter the protected payment route.
    ///
    /// # Errors
    ///
    /// Requires owned, ready authorization and valid navigation.
    pub fn begin_confirmation(
        &mut self,
        owner: &InstanceId,
        now: Instant,
    ) -> Result<ProtectedConfirmationView, NativeCheckoutError> {
        let view = self.payment.begin_confirmation(owner, now)?;
        self.router.push("/checkout/payment", false)?;
        self.router.set_pending_payment(true)?;
        Ok(view)
    }

    /// Confirm and execute the frozen amount through the offline simulator.
    ///
    /// # Errors
    ///
    /// Rejects invalid ownership, authorization, confirmation, or idempotency state.
    pub fn confirm_and_charge(
        &mut self,
        owner: &InstanceId,
        authorization_ref: &str,
        idempotency_key: &str,
        now: Instant,
        host_timestamp_millis: u64,
    ) -> Result<PaymentResult, NativeCheckoutError> {
        let result = self.payment.confirm_and_charge(
            owner,
            authorization_ref,
            idempotency_key,
            now,
            host_timestamp_millis,
        )?;
        self.router.set_pending_payment(false)?;
        self.result = Some(result.clone());
        Ok(result)
    }

    /// Create an immutable receipt and navigate to it after approval.
    ///
    /// # Errors
    ///
    /// Requires an approved result and exact values agreeing with the confirmation.
    pub fn create_receipt(
        &mut self,
        lines: Vec<ReceiptLine>,
        subtotal: Money,
        discount: Money,
        tax: Money,
    ) -> Result<Receipt, NativeCheckoutError> {
        let result = self
            .result
            .as_ref()
            .ok_or(NativeCheckoutError::StateInvalid)?;
        if result.outcome() != PaymentOutcome::Approved {
            return Err(NativeCheckoutError::StateInvalid);
        }
        let confirmed = self
            .payment
            .confirmed_snapshot()
            .ok_or(NativeCheckoutError::StateInvalid)?;
        let receipt = Receipt::from_approved(
            self.payment.principal().clone(),
            confirmed,
            result,
            lines,
            subtotal,
            discount,
            tax,
        )?;
        self.printer.register(receipt.clone())?;
        self.router
            .push(&format!("/receipts/{}", receipt.id()), false)?;
        self.receipt = Some(receipt.clone());
        Ok(receipt)
    }

    /// Create the single structured host preview for the current receipt.
    ///
    /// # Errors
    ///
    /// Requires a current owned receipt and a unique request identity.
    pub fn print_preview(
        &mut self,
        request_id: &str,
    ) -> Result<PrintPreviewSurface, NativeCheckoutError> {
        let receipt = self
            .receipt
            .as_ref()
            .ok_or(NativeCheckoutError::StateInvalid)?;
        let request = PrintPreviewRequest::new(request_id, receipt.id())?;
        let job = self.printer.preview(self.payment.principal(), request)?;
        Ok(PrintPreviewSurface::new(job))
    }

    /// Current native route.
    #[must_use]
    pub fn current_route(&self) -> &str {
        self.router.current_route()
    }

    /// Offline invariant across all terminal outcomes.
    #[must_use]
    pub const fn network_attempts(&self) -> usize {
        self.payment.network_attempts()
    }

    /// Owning native instance.
    #[must_use]
    pub const fn owner(&self) -> &InstanceId {
        &self.owner
    }
}
