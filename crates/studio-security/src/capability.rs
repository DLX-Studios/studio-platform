//! Deny-by-default capability and pending-action admission.

use std::{collections::HashSet, error::Error, fmt};

use crate::PluginPrincipal;

/// Closed milestone-one capability catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CapabilityId {
    /// Deterministic payment simulator.
    PaymentSimulate,
    /// Structured printer-preview simulator.
    PrinterSimulate,
    /// Bounded host-mediated SurrealQL over application data.
    DataSurrealQuery,
}

impl CapabilityId {
    /// Parse the closed capability catalog without falling back to an open string.
    ///
    /// # Errors
    ///
    /// Unknown capability names are denied.
    pub fn parse(value: &str) -> Result<Self, SecurityError> {
        match value {
            "payment.simulate" => Ok(Self::PaymentSimulate),
            "printer.simulate" => Ok(Self::PrinterSimulate),
            "data.surreal.query" => Ok(Self::DataSurrealQuery),
            _ => Err(SecurityError::capability_denied()),
        }
    }
}

/// Stable, non-sensitive security decision code.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecurityErrorCode {
    /// Principal, declaration, policy, or operation authorization failed.
    CapabilityDenied,
    /// Pending action capacity has been reached.
    QueueFull,
    /// A request identifier is malformed, duplicated, or unknown.
    RequestInvalid,
}

/// Safe capability-admission failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityError {
    code: SecurityErrorCode,
}

impl SecurityError {
    pub(crate) const fn capability_denied() -> Self {
        Self {
            code: SecurityErrorCode::CapabilityDenied,
        }
    }

    pub(crate) const fn queue_full() -> Self {
        Self {
            code: SecurityErrorCode::QueueFull,
        }
    }

    pub(crate) const fn request_invalid() -> Self {
        Self {
            code: SecurityErrorCode::RequestInvalid,
        }
    }

    /// Stable code suitable for host-owned diagnostics and action results.
    #[must_use]
    pub const fn code(&self) -> SecurityErrorCode {
        self.code
    }
}

impl fmt::Display for SecurityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            SecurityErrorCode::CapabilityDenied => "capability denied",
            SecurityErrorCode::QueueFull => "pending action queue full",
            SecurityErrorCode::RequestInvalid => "action request invalid",
        })
    }
}

impl Error for SecurityError {}

/// Instance-owned action admission gate with a fixed host pending-request ceiling.
#[derive(Clone, Debug)]
pub struct ActionGate {
    owner: PluginPrincipal,
    declared: HashSet<CapabilityId>,
    allowed: HashSet<CapabilityId>,
    maximum_pending: usize,
    pending: HashSet<String>,
}

impl ActionGate {
    /// Create a gate from signed manifest declarations and independent host policy.
    pub fn new(
        owner: PluginPrincipal,
        declared: impl IntoIterator<Item = CapabilityId>,
        allowed: impl IntoIterator<Item = CapabilityId>,
        maximum_pending: usize,
    ) -> Self {
        Self {
            owner,
            declared: declared.into_iter().collect(),
            allowed: allowed.into_iter().collect(),
            maximum_pending: maximum_pending.min(16),
            pending: HashSet::new(),
        }
    }

    /// Validate and reserve one pending action request.
    ///
    /// # Errors
    ///
    /// Denies mismatched owners, undeclared/disallowed capabilities, unknown operations,
    /// malformed or duplicate request IDs, and requests above the host ceiling.
    pub fn begin(
        &mut self,
        caller: &PluginPrincipal,
        capability: CapabilityId,
        operation: &str,
        request_id: &str,
    ) -> Result<(), SecurityError> {
        if request_id.is_empty()
            || request_id.len() > 128
            || request_id.chars().any(char::is_control)
            || self.pending.contains(request_id)
        {
            return Err(SecurityError::request_invalid());
        }
        if caller != &self.owner
            || !self.declared.contains(&capability)
            || !self.allowed.contains(&capability)
            || !valid_operation(capability, operation)
        {
            return Err(SecurityError::capability_denied());
        }
        if self.pending.len() >= self.maximum_pending {
            return Err(SecurityError::queue_full());
        }
        self.pending.insert(request_id.to_owned());
        Ok(())
    }

    /// Complete and release one pending request ID.
    ///
    /// # Errors
    ///
    /// Returns [`SecurityErrorCode::RequestInvalid`] when the ID is not pending.
    pub fn complete(&mut self, request_id: &str) -> Result<(), SecurityError> {
        if self.pending.remove(request_id) {
            Ok(())
        } else {
            Err(SecurityError::request_invalid())
        }
    }

    /// Number of requests currently admitted for this instance.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

fn valid_operation(capability: CapabilityId, operation: &str) -> bool {
    matches!(
        (capability, operation),
        (CapabilityId::PaymentSimulate, "charge") | (CapabilityId::PrinterSimulate, "preview")
            | (CapabilityId::DataSurrealQuery, "query")
    )
}
