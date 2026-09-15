//! Deterministic offline payment simulator.

use studio_security::PluginPrincipal;

use crate::{
    ConfirmedPayment, Money, PaymentError, PaymentErrorCode,
    idempotency::{IdempotencyRegistry, Lookup, PaymentFingerprint},
};

const PROCESS_RECORD_CAPACITY: usize = 10_000;

/// Proof that the host resolved a valid authorization reference for this execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaymentAuthorization(());

impl PaymentAuthorization {
    /// Construct proof after host-owned secret resolution.
    #[must_use]
    pub const fn host_verified() -> Self {
        Self(())
    }
}

/// Fully host-bound request for one deterministic simulated charge.
#[derive(Clone, Debug)]
pub struct PaymentRequest {
    idempotency_key: String,
    owner: PluginPrincipal,
    confirmation: ConfirmedPayment,
    authorization: Option<PaymentAuthorization>,
}

impl PaymentRequest {
    /// Construct a request from an immutable confirmation and optional host authorization proof.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, or control-bearing idempotency keys.
    pub fn new(
        idempotency_key: impl Into<String>,
        owner: PluginPrincipal,
        confirmation: ConfirmedPayment,
        authorization: Option<PaymentAuthorization>,
    ) -> Result<Self, PaymentError> {
        let idempotency_key = idempotency_key.into();
        if idempotency_key.is_empty()
            || idempotency_key.len() > 128
            || idempotency_key.chars().any(char::is_control)
        {
            return Err(PaymentError::new(PaymentErrorCode::ActionInvalid));
        }
        Ok(Self {
            idempotency_key,
            owner,
            confirmation,
            authorization,
        })
    }
}

/// Closed simulator outcome catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaymentOutcome {
    /// Simulated approval.
    Approved,
    /// Simulated issuer decline.
    Declined,
    /// Simulated provider timeout.
    Timeout,
    /// Simulated terminal unavailability.
    Unavailable,
}

/// Immutable terminal simulator result retained for exact replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentResult {
    outcome: PaymentOutcome,
    code: Option<&'static str>,
    retryable: bool,
    amount: Money,
    result_reference: String,
    host_timestamp_millis: u64,
}

impl PaymentResult {
    /// Terminal simulator outcome.
    #[must_use]
    pub const fn outcome(&self) -> PaymentOutcome {
        self.outcome
    }

    /// Stable failure code, absent only for approval.
    #[must_use]
    pub const fn code(&self) -> Option<&'static str> {
        self.code
    }

    /// Whether the operator may start a documented recovery flow.
    #[must_use]
    pub const fn retryable(&self) -> bool {
        self.retryable
    }

    /// Exact confirmed money used by execution.
    #[must_use]
    pub const fn amount(&self) -> &Money {
        &self.amount
    }

    /// Non-sensitive simulator transaction reference.
    #[must_use]
    pub fn result_reference(&self) -> &str {
        &self.result_reference
    }

    /// Host timestamp supplied to the deterministic simulator boundary.
    #[must_use]
    pub const fn host_timestamp_millis(&self) -> u64 {
        self.host_timestamp_millis
    }
}

/// Offline simulator with bounded, no-eviction, process-lifetime idempotency state.
pub struct PaymentSimulator {
    idempotency: IdempotencyRegistry,
    next_reference: u64,
}

impl PaymentSimulator {
    /// Create the milestone simulator with its required 10,000-record ceiling.
    #[must_use]
    pub fn new() -> Self {
        Self {
            idempotency: IdempotencyRegistry::new(PROCESS_RECORD_CAPACITY),
            next_reference: 1,
        }
    }

    /// Create a smaller registry for bounded deterministic testing.
    ///
    /// # Errors
    ///
    /// Rejects zero or values above the host ceiling.
    pub fn with_capacity(capacity: usize) -> Result<Self, PaymentError> {
        if capacity == 0 || capacity > PROCESS_RECORD_CAPACITY {
            return Err(PaymentError::new(PaymentErrorCode::ActionInvalid));
        }
        Ok(Self {
            idempotency: IdempotencyRegistry::new(capacity),
            next_reference: 1,
        })
    }

    /// Execute or replay one deterministic terminal charge.
    ///
    /// # Errors
    ///
    /// Rejects conflicting keys, missing authorization for new keys, and new keys at capacity.
    pub fn charge(
        &mut self,
        request: PaymentRequest,
        host_timestamp_millis: u64,
    ) -> Result<PaymentResult, PaymentError> {
        let fingerprint = PaymentFingerprint::new(
            request.owner,
            request.confirmation.checkout_session_id().to_owned(),
            request.confirmation.amount().clone(),
        );
        match self
            .idempotency
            .lookup(&request.idempotency_key, &fingerprint)?
        {
            Lookup::Replay(result) => return Ok(result),
            Lookup::Missing => {}
        }
        if request.authorization.is_none() {
            return Err(PaymentError::new(PaymentErrorCode::AuthorizationRequired));
        }
        if self.idempotency.len() >= self.idempotency.capacity() {
            return Err(PaymentError::new(
                PaymentErrorCode::IdempotencyCapacityExhausted,
            ));
        }
        let (outcome, code, retryable) = outcome(request.confirmation.amount().minor());
        let result = PaymentResult {
            outcome,
            code,
            retryable,
            amount: request.confirmation.amount().clone(),
            result_reference: format!("sim-{:08}", self.next_reference),
            host_timestamp_millis,
        };
        self.next_reference = self.next_reference.saturating_add(1);
        self.idempotency
            .insert(request.idempotency_key, fingerprint, result.clone())?;
        Ok(result)
    }

    /// Number of retained terminal transactions.
    #[must_use]
    pub fn transaction_count(&self) -> usize {
        self.idempotency.len()
    }

    /// Configured terminal record ceiling.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.idempotency.capacity()
    }

    /// Simulator invariant: this implementation has no network path.
    #[must_use]
    pub const fn network_attempts(&self) -> usize {
        0
    }
}

impl Default for PaymentSimulator {
    fn default() -> Self {
        Self::new()
    }
}

fn outcome(minor: i64) -> (PaymentOutcome, Option<&'static str>, bool) {
    match minor % 100 {
        1 => (PaymentOutcome::Declined, Some("payment_declined"), false),
        2 => (PaymentOutcome::Timeout, Some("payment_timeout"), true),
        3 => (
            PaymentOutcome::Unavailable,
            Some("terminal_unavailable"),
            true,
        ),
        _ => (PaymentOutcome::Approved, None, false),
    }
}
