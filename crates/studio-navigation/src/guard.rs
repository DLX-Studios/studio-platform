//! Synchronous, host-budgeted navigation guards.

use std::time::Duration;

/// Closed outcomes returned by a navigation guard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuardDecision {
    /// Permit ordinary navigation.
    Allow,
    /// Reject navigation.
    Deny,
    /// Permit navigation after explicit protected-flow confirmation.
    Confirmed,
}

/// A guard outcome paired with host-observed execution time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuardResponse {
    pub(crate) decision: GuardDecision,
    pub(crate) elapsed: Duration,
}

impl GuardResponse {
    /// Construct a response from the decision and host-observed duration.
    #[must_use]
    pub const fn new(decision: GuardDecision, elapsed: Duration) -> Self {
        Self { decision, elapsed }
    }
}

/// Plugin navigation guard evaluated by the host before a protected exit.
pub trait NavigationGuard {
    /// Evaluate a route change. Implementations cannot mutate the staged stack.
    fn evaluate(&mut self, from: &str, to: &str, pending_payment: bool) -> GuardResponse;
}
