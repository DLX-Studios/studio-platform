//! Instance-owned, bounded, atomic navigation stack.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::{
    error::{StackError, StackErrorCode},
    guard::{GuardDecision, NavigationGuard},
    route::parse_route,
};

const MAX_DEPTH: usize = 32;
const GUARD_BUDGET: Duration = Duration::from_millis(50);

/// Unforgeable-at-the-API-boundary identity of the owning plugin instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StackOwner([u8; 16]);

impl StackOwner {
    /// Construct an owner identity from host-issued bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}

/// Closed set of atomic navigation commands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationOperation<'a> {
    /// Add a route above the current entry.
    Push(&'a str),
    /// Replace the current entry.
    Replace(&'a str),
    /// Remove the current entry.
    Pop,
    /// Remove entries through a prior route.
    PopTo(&'a str),
    /// Replace the entire stack with one route.
    Reset(&'a str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StackEntry {
    route: String,
    state: BTreeMap<String, String>,
}

/// Host-owned navigation history for one plugin instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavigationStack {
    owner: StackOwner,
    entries: Vec<StackEntry>,
    pending_payment: bool,
}

impl NavigationStack {
    /// Create a stack with one valid concrete route.
    ///
    /// # Errors
    ///
    /// Returns `RouteInvalid` when the initial concrete route is malformed.
    pub fn new(owner: StackOwner, route: impl Into<String>) -> Result<Self, StackError> {
        let route = route.into();
        validate_route(&route)?;
        Ok(Self {
            owner,
            entries: vec![StackEntry {
                route,
                state: BTreeMap::new(),
            }],
            pending_payment: false,
        })
    }

    /// Apply one command atomically after owner and protected-exit checks.
    ///
    /// # Errors
    ///
    /// Returns a stable stack error for invalid ownership, routes, bounds, targets, or guards.
    pub fn apply(
        &mut self,
        owner: &StackOwner,
        operation: NavigationOperation<'_>,
        guard: &mut impl NavigationGuard,
    ) -> Result<(), StackError> {
        self.check_owner(owner)?;
        let destination = self.destination(operation)?;
        if self.pending_payment {
            let response = guard.evaluate(self.current_route(), &destination, true);
            if response.elapsed > GUARD_BUDGET {
                return Err(StackError::new(StackErrorCode::GuardTimeout));
            }
            if response.decision != GuardDecision::Confirmed {
                return Err(StackError::new(StackErrorCode::GuardDenied));
            }
        }

        let mut staged = self.entries.clone();
        match operation {
            NavigationOperation::Push(route) => {
                if staged.len() == MAX_DEPTH {
                    return Err(StackError::new(StackErrorCode::StackOverflow));
                }
                staged.push(new_entry(route)?);
            }
            NavigationOperation::Replace(route) => {
                if let Some(current) = staged.last_mut() {
                    *current = new_entry(route)?;
                }
            }
            NavigationOperation::Pop => {
                if staged.len() == 1 {
                    return Err(StackError::new(StackErrorCode::RootPop));
                }
                staged.pop();
            }
            NavigationOperation::PopTo(route) => {
                validate_route(route)?;
                let index = staged
                    .iter()
                    .rposition(|entry| entry.route == route)
                    .ok_or_else(|| StackError::new(StackErrorCode::TargetNotFound))?;
                staged.truncate(index + 1);
            }
            NavigationOperation::Reset(route) => staged = vec![new_entry(route)?],
        }
        self.entries = staged;
        self.pending_payment = false;
        Ok(())
    }

    /// Save route-local state on the current entry.
    ///
    /// # Errors
    ///
    /// Returns `OwnerMismatch` when called by another plugin instance.
    pub fn set_local_state(
        &mut self,
        owner: &StackOwner,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), StackError> {
        self.check_owner(owner)?;
        if let Some(current) = self.entries.last_mut() {
            current.state.insert(key.into(), value.into());
        }
        Ok(())
    }

    /// Read route-local state from the current entry.
    #[must_use]
    pub fn local_state(&self, key: &str) -> Option<&str> {
        self.entries.last()?.state.get(key).map(String::as_str)
    }

    /// Set whether a protected payment may be abandoned.
    ///
    /// # Errors
    ///
    /// Returns `OwnerMismatch` when called by another plugin instance.
    pub fn set_pending_payment(
        &mut self,
        owner: &StackOwner,
        pending: bool,
    ) -> Result<(), StackError> {
        self.check_owner(owner)?;
        self.pending_payment = pending;
        Ok(())
    }

    /// Return whether protected payment is pending.
    #[must_use]
    pub const fn pending_payment(&self) -> bool {
        self.pending_payment
    }

    /// Return the active route.
    #[must_use]
    pub fn current_route(&self) -> &str {
        self.entries.last().map_or("", |entry| entry.route.as_str())
    }

    /// Return the number of stack entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Navigation stacks are never empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }

    fn check_owner(&self, owner: &StackOwner) -> Result<(), StackError> {
        if self.owner == *owner {
            Ok(())
        } else {
            Err(StackError::new(StackErrorCode::OwnerMismatch))
        }
    }

    fn destination(&self, operation: NavigationOperation<'_>) -> Result<String, StackError> {
        match operation {
            NavigationOperation::Push(route)
            | NavigationOperation::Replace(route)
            | NavigationOperation::Reset(route)
            | NavigationOperation::PopTo(route) => validate_route(route).map(|()| route.to_owned()),
            NavigationOperation::Pop => self
                .entries
                .get(self.entries.len().saturating_sub(2))
                .map(|entry| entry.route.clone())
                .ok_or_else(|| StackError::new(StackErrorCode::RootPop)),
        }
    }
}

fn new_entry(route: &str) -> Result<StackEntry, StackError> {
    validate_route(route)?;
    Ok(StackEntry {
        route: route.to_owned(),
        state: BTreeMap::new(),
    })
}

fn validate_route(route: &str) -> Result<(), StackError> {
    parse_route(route)
        .map(|_| ())
        .map_err(|_| StackError::new(StackErrorCode::RouteInvalid))
}
