#![allow(missing_docs)]

use std::time::{Duration, Instant};

use studio_actions::{Checkout, ConfirmationErrorCode, Money, TrustedPaymentConfirmation};

fn checkout() -> Checkout {
    Checkout::new(
        "checkout-1",
        "Nova Barbers",
        "Example Publisher",
        Money::new("USD", 8_500).unwrap(),
    )
    .unwrap()
}

#[test]
fn trusted_surface_displays_verified_identity_exact_money_and_simulator_status() {
    let now = Instant::now();
    let confirmation = checkout()
        .begin_confirmation(now + Duration::from_mins(2))
        .unwrap();
    let view = confirmation.view();

    assert_eq!(view.merchant, "Nova Barbers");
    assert_eq!(view.publisher, "Example Publisher");
    assert_eq!(view.amount.currency(), "USD");
    assert_eq!(view.amount.minor(), 8_500);
    assert_eq!(view.simulator_status, "SIMULATOR — no real charge");
}

#[test]
fn confirmation_freezes_amount_currency_and_identity_against_later_cart_changes() {
    let now = Instant::now();
    let mut checkout = checkout();
    let mut confirmation = checkout
        .begin_confirmation(now + Duration::from_mins(2))
        .unwrap();

    checkout
        .set_total(Money::new("USD", 99_999).unwrap())
        .unwrap();
    let snapshot = confirmation.confirm(now).unwrap();

    assert_eq!(snapshot.checkout_session_id(), "checkout-1");
    assert_eq!(snapshot.merchant(), "Nova Barbers");
    assert_eq!(snapshot.publisher(), "Example Publisher");
    assert_eq!(snapshot.amount().currency(), "USD");
    assert_eq!(snapshot.amount().minor(), 8_500);
    assert_eq!(checkout.total().minor(), 99_999);
}

#[test]
fn cancellation_and_authorization_expiry_cannot_produce_a_payment_snapshot() {
    let now = Instant::now();
    let mut cancelled = checkout()
        .begin_confirmation(now + Duration::from_mins(2))
        .unwrap();
    cancelled.cancel().unwrap();
    assert_eq!(
        cancelled.confirm(now).unwrap_err().code(),
        ConfirmationErrorCode::ConfirmationCancelled
    );

    let mut expired = checkout()
        .begin_confirmation(now + Duration::from_secs(30))
        .unwrap();
    assert_eq!(
        expired
            .confirm(now + Duration::from_secs(30))
            .unwrap_err()
            .code(),
        ConfirmationErrorCode::AuthorizationExpired
    );
    assert_eq!(
        expired.confirm(now).unwrap_err().code(),
        ConfirmationErrorCode::AuthorizationExpired
    );
}

#[test]
fn money_rejects_floating_or_implicit_currency_shapes_at_the_type_boundary() {
    assert!(Money::new("", 100).is_err());
    assert!(Money::new("usd", 100).is_err());
    assert!(Money::new("USDX", 100).is_err());
    assert!(Money::new("USD", -1).is_err());
    let _: TrustedPaymentConfirmation = checkout()
        .begin_confirmation(Instant::now() + Duration::from_mins(2))
        .unwrap();
}
