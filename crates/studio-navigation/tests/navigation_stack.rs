#![allow(missing_docs)]

use std::time::Duration;

use studio_navigation::{
    GuardDecision, GuardResponse, NavigationGuard, NavigationOperation, NavigationStack,
    StackErrorCode, StackOwner,
};

struct Guard {
    response: GuardResponse,
    calls: usize,
}

impl NavigationGuard for Guard {
    fn evaluate(&mut self, _from: &str, _to: &str, _pending_payment: bool) -> GuardResponse {
        self.calls += 1;
        self.response
    }
}

fn allow() -> Guard {
    Guard {
        response: GuardResponse::new(GuardDecision::Allow, Duration::ZERO),
        calls: 0,
    }
}

#[test]
fn push_replace_pop_pop_to_and_reset_are_atomic_and_restore_local_state() {
    let owner = StackOwner::new([1; 16]);
    let mut stack = NavigationStack::new(owner, "/catalog").unwrap();
    let mut guard = allow();
    stack.set_local_state(&owner, "search", "beard").unwrap();
    stack
        .apply(&owner, NavigationOperation::Push("/cart"), &mut guard)
        .unwrap();
    stack
        .apply(&owner, NavigationOperation::Push("/checkout"), &mut guard)
        .unwrap();
    stack
        .apply(
            &owner,
            NavigationOperation::Replace("/checkout/payment"),
            &mut guard,
        )
        .unwrap();
    assert_eq!(stack.current_route(), "/checkout/payment");
    stack
        .apply(&owner, NavigationOperation::PopTo("/catalog"), &mut guard)
        .unwrap();
    assert_eq!(stack.current_route(), "/catalog");
    assert_eq!(stack.local_state("search"), Some("beard"));

    stack
        .apply(&owner, NavigationOperation::Push("/cart"), &mut guard)
        .unwrap();
    stack
        .apply(&owner, NavigationOperation::Pop, &mut guard)
        .unwrap();
    stack
        .apply(
            &owner,
            NavigationOperation::Reset("/catalog/new"),
            &mut guard,
        )
        .unwrap();
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.current_route(), "/catalog/new");
    assert_eq!(stack.local_state("search"), None);
}

#[test]
fn ownership_and_depth_32_are_enforced_without_partial_mutation() {
    let owner = StackOwner::new([1; 16]);
    let foreign = StackOwner::new([2; 16]);
    let mut stack = NavigationStack::new(owner, "/root").unwrap();
    let mut guard = allow();
    assert_eq!(
        stack
            .apply(&foreign, NavigationOperation::Push("/foreign"), &mut guard,)
            .unwrap_err()
            .code(),
        StackErrorCode::OwnerMismatch
    );
    assert_eq!(stack.current_route(), "/root");

    for index in 1..32 {
        stack
            .apply(
                &owner,
                NavigationOperation::Push(&format!("/route-{index}")),
                &mut guard,
            )
            .unwrap();
    }
    assert_eq!(stack.len(), 32);
    assert_eq!(
        stack
            .apply(&owner, NavigationOperation::Push("/overflow"), &mut guard,)
            .unwrap_err()
            .code(),
        StackErrorCode::StackOverflow
    );
    assert_eq!(stack.len(), 32);
}

#[test]
fn pending_payment_requires_confirmation_and_guard_timeout_blocks_navigation() {
    let owner = StackOwner::new([1; 16]);
    let mut stack = NavigationStack::new(owner, "/checkout/payment").unwrap();
    stack.set_pending_payment(&owner, true).unwrap();

    for (response, expected) in [
        (
            GuardResponse::new(GuardDecision::Deny, Duration::ZERO),
            StackErrorCode::GuardDenied,
        ),
        (
            GuardResponse::new(GuardDecision::Allow, Duration::ZERO),
            StackErrorCode::GuardDenied,
        ),
        (
            GuardResponse::new(GuardDecision::Confirmed, Duration::from_millis(51)),
            StackErrorCode::GuardTimeout,
        ),
    ] {
        let mut guard = Guard { response, calls: 0 };
        assert_eq!(
            stack
                .apply(&owner, NavigationOperation::Push("/catalog"), &mut guard)
                .unwrap_err()
                .code(),
            expected
        );
        assert_eq!(guard.calls, 1);
        assert_eq!(stack.current_route(), "/checkout/payment");
    }

    let mut confirmed = Guard {
        response: GuardResponse::new(GuardDecision::Confirmed, Duration::from_millis(50)),
        calls: 0,
    };
    stack
        .apply(
            &owner,
            NavigationOperation::Push("/catalog"),
            &mut confirmed,
        )
        .unwrap();
    assert_eq!(stack.current_route(), "/catalog");
    assert!(!stack.pending_payment());
}
