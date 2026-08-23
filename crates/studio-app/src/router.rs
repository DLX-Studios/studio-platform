//! Native-shell navigation composition for protected checkout screens.

use std::time::Duration;

use studio_navigation::{
    GuardDecision, GuardResponse, HostClock, NavigationGuard, NavigationOperation, NavigationStack,
    StackError, StackOwner, TransitionController, TransitionKind,
};

use crate::HostPreferences;

struct FixedClock(Duration);
impl HostClock for FixedClock {
    fn elapsed(&self) -> Duration {
        self.0
    }
}

struct ShellGuard(bool);
impl NavigationGuard for ShellGuard {
    fn evaluate(&mut self, _: &str, _: &str, _: bool) -> GuardResponse {
        GuardResponse::new(
            if self.0 {
                GuardDecision::Confirmed
            } else {
                GuardDecision::Deny
            },
            Duration::ZERO,
        )
    }
}

/// Instance-owned router used by the native checkout shell.
pub struct CheckoutRouter {
    owner: StackOwner,
    stack: NavigationStack,
    transitions: TransitionController,
}

impl CheckoutRouter {
    /// Create a router at the cart screen.
    ///
    /// # Errors
    ///
    /// Returns a navigation error if the initial route is invalid.
    pub fn new(owner: StackOwner, preferences: HostPreferences) -> Result<Self, StackError> {
        Ok(Self {
            owner,
            stack: NavigationStack::new(owner, "/cart")?,
            transitions: TransitionController::new(preferences.motion()),
        })
    }

    /// Push a route, optionally carrying trusted confirmation for a protected exit.
    ///
    /// # Errors
    ///
    /// Returns a stable stack/guard error without changing the route.
    pub fn push(&mut self, route: &str, confirmed_exit: bool) -> Result<(), StackError> {
        let from = self.stack.current_route().to_owned();
        self.stack.apply(
            &self.owner,
            NavigationOperation::Push(route),
            &mut ShellGuard(confirmed_exit),
        )?;
        let clock = FixedClock(Duration::ZERO);
        self.transitions
            .begin(TransitionKind::Push, &from, route, &clock);
        let _ = self.transitions.sample(&clock);
        Ok(())
    }

    /// Mark whether leaving the current payment screen requires confirmation.
    ///
    /// # Errors
    ///
    /// This internally owned operation fails only if the owner invariant is broken.
    pub fn set_pending_payment(&mut self, pending: bool) -> Result<(), StackError> {
        self.stack.set_pending_payment(&self.owner, pending)
    }

    /// Current route committed by the host stack.
    #[must_use]
    pub fn current_route(&self) -> &str {
        self.stack.current_route()
    }
}
