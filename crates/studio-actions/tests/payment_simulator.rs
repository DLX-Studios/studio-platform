#![allow(missing_docs)]

use studio_actions::{
    Checkout, Money, PaymentAuthorization, PaymentErrorCode, PaymentOutcome, PaymentRequest,
    PaymentSimulator,
};
use studio_security::{PluginPrincipal, TrustMode};

fn principal(instance: u8) -> PluginPrincipal {
    PluginPrincipal::new(
        "publisher-key-1",
        "com.example.pos",
        [7; 32],
        [instance; 16],
        TrustMode::Production,
    )
    .unwrap()
}

fn request(key: &str, minor: i64, owner: PluginPrincipal, authorized: bool) -> PaymentRequest {
    let now = std::time::Instant::now();
    let mut confirmation = Checkout::new(
        "checkout-1",
        "Nova Barbers",
        "Example Publisher",
        Money::new("USD", minor).unwrap(),
    )
    .unwrap()
    .begin_confirmation(now + std::time::Duration::from_mins(2))
    .unwrap();
    PaymentRequest::new(
        key,
        owner,
        confirmation.confirm(now).unwrap(),
        authorized.then(PaymentAuthorization::host_verified),
    )
    .unwrap()
}

#[test]
fn new_payments_require_host_verified_authorization() {
    let mut simulator = PaymentSimulator::new();
    let error = simulator
        .charge(request("sale-1", 8_500, principal(1), false), 1_000)
        .unwrap_err();
    assert_eq!(error.code(), PaymentErrorCode::AuthorizationRequired);
    assert_eq!(simulator.transaction_count(), 0);
}

#[test]
fn exact_minor_unit_suffixes_select_all_documented_offline_outcomes() {
    let mut simulator = PaymentSimulator::new();
    for (index, minor, expected, code, retryable) in [
        (0, 8_500, PaymentOutcome::Approved, None, false),
        (
            1,
            8_501,
            PaymentOutcome::Declined,
            Some("payment_declined"),
            false,
        ),
        (
            2,
            8_502,
            PaymentOutcome::Timeout,
            Some("payment_timeout"),
            true,
        ),
        (
            3,
            8_503,
            PaymentOutcome::Unavailable,
            Some("terminal_unavailable"),
            true,
        ),
    ] {
        let result = simulator
            .charge(
                request(&format!("sale-{index}"), minor, principal(1), true),
                1_000 + index,
            )
            .unwrap();
        assert_eq!(result.outcome(), expected);
        assert_eq!(result.code(), code);
        assert_eq!(result.retryable(), retryable);
        assert_eq!(result.amount().minor(), minor);
    }
    assert_eq!(simulator.transaction_count(), 4);
    assert_eq!(simulator.network_attempts(), 0);
}

#[test]
fn retained_keys_replay_exactly_and_conflicting_reuse_is_rejected() {
    let mut simulator = PaymentSimulator::new();
    let first = simulator
        .charge(request("same-key", 8_500, principal(1), true), 1_000)
        .unwrap();
    let replay = simulator
        .charge(request("same-key", 8_500, principal(1), false), 9_999)
        .unwrap();
    assert_eq!(replay, first);
    assert_eq!(simulator.transaction_count(), 1);

    for conflicting in [
        request("same-key", 8_501, principal(1), true),
        request("same-key", 8_500, principal(2), true),
    ] {
        assert_eq!(
            simulator.charge(conflicting, 2_000).unwrap_err().code(),
            PaymentErrorCode::IdempotencyConflict
        );
    }
}

#[test]
fn capacity_never_evicts_terminal_records_and_replays_remain_available() {
    assert_eq!(PaymentSimulator::new().capacity(), 10_000);
    let mut simulator = PaymentSimulator::with_capacity(2).unwrap();
    let first = simulator
        .charge(request("sale-a", 8_500, principal(1), true), 1)
        .unwrap();
    simulator
        .charge(request("sale-b", 8_501, principal(1), true), 2)
        .unwrap();

    assert_eq!(
        simulator
            .charge(request("sale-c", 8_502, principal(1), true), 3)
            .unwrap_err()
            .code(),
        PaymentErrorCode::IdempotencyCapacityExhausted
    );
    assert_eq!(simulator.transaction_count(), 2);
    assert_eq!(
        simulator
            .charge(request("sale-a", 8_500, principal(1), false), 4)
            .unwrap(),
        first
    );
}
