#![allow(missing_docs)]

use std::time::{Duration, Instant};

use studio_actions::{Checkout, Money, PaymentOutcome};
use studio_app::{ProtectedPaymentErrorCode, StudioHost};
use studio_security::{PluginPrincipal, TrustMode};
use studio_ui::InstanceId;

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

fn session(minor: i64) -> studio_app::ProtectedPaymentSession {
    StudioHost::protected_payment_session(
        InstanceId::new("instance-a").unwrap(),
        principal(1),
        Checkout::new(
            "checkout-1",
            "Nova Barbers",
            "Example Publisher",
            Money::new("USD", minor).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn token(event: &studio_protocol::HostEvent) -> String {
    let encoded = serde_json::to_value(event).unwrap();
    encoded["payload"]["payload"]["authorization_ref"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn no_pin_is_recoverable_and_ready_event_contains_only_an_opaque_reference() {
    let owner = InstanceId::new("instance-a").unwrap();
    let now = Instant::now();
    let mut payment = session(8_500);
    assert_eq!(
        payment.begin_confirmation(&owner, now).unwrap_err().code(),
        ProtectedPaymentErrorCode::AuthorizationRequired
    );

    let event = payment.capture_pin(&owner, b"8642", now).unwrap();
    let serialized = serde_json::to_string(&event).unwrap();
    assert!(!serialized.contains("8642"));
    assert_eq!(token(&event).len(), 64);
}

#[test]
fn trusted_confirmation_rejects_invalid_references_and_charges_the_frozen_amount() {
    let owner = InstanceId::new("instance-a").unwrap();
    let now = Instant::now();
    let mut payment = session(8_500);
    let authorization = token(&payment.capture_pin(&owner, b"8642", now).unwrap());
    let view = payment.begin_confirmation(&owner, now).unwrap();
    assert_eq!(view.merchant, "Nova Barbers");
    assert_eq!(view.publisher, "Example Publisher");
    assert_eq!(view.amount.minor(), 8_500);
    assert_eq!(view.simulator_status, "SIMULATOR — no real charge");

    payment
        .set_cart_total(Money::new("USD", 99_900).unwrap())
        .unwrap();
    assert_eq!(
        payment
            .confirm_and_charge(&owner, &"00".repeat(32), "sale-1", now, 1_000)
            .unwrap_err()
            .code(),
        ProtectedPaymentErrorCode::AuthorizationInvalid
    );
    assert_eq!(
        payment
            .confirm_and_charge(
                &InstanceId::new("instance-b").unwrap(),
                &authorization,
                "sale-1",
                now,
                1_000,
            )
            .unwrap_err()
            .code(),
        ProtectedPaymentErrorCode::OwnerMismatch
    );

    let result = payment
        .confirm_and_charge(&owner, &authorization, "sale-1", now, 1_000)
        .unwrap();
    assert_eq!(result.outcome(), PaymentOutcome::Approved);
    assert_eq!(result.amount().minor(), 8_500);
    assert_eq!(payment.cart_total().minor(), 99_900);
    assert_eq!(payment.replay("sale-1", 9_999).unwrap(), result);
    assert_eq!(payment.transaction_count(), 1);
}

#[test]
fn protected_flow_produces_every_deterministic_outcome_and_expires_authorization() {
    let owner = InstanceId::new("instance-a").unwrap();
    let now = Instant::now();
    for (minor, expected) in [
        (8_500, PaymentOutcome::Approved),
        (8_501, PaymentOutcome::Declined),
        (8_502, PaymentOutcome::Timeout),
        (8_503, PaymentOutcome::Unavailable),
    ] {
        let mut payment = session(minor);
        let authorization = token(&payment.capture_pin(&owner, b"2468", now).unwrap());
        payment.begin_confirmation(&owner, now).unwrap();
        assert_eq!(
            payment
                .confirm_and_charge(&owner, &authorization, &format!("sale-{minor}"), now, 1_000,)
                .unwrap()
                .outcome(),
            expected
        );
    }

    let mut expired = session(8_500);
    let authorization = token(&expired.capture_pin(&owner, b"1357", now).unwrap());
    expired.begin_confirmation(&owner, now).unwrap();
    assert_eq!(
        expired
            .confirm_and_charge(
                &owner,
                &authorization,
                "expired",
                now + Duration::from_mins(2),
                1_000,
            )
            .unwrap_err()
            .code(),
        ProtectedPaymentErrorCode::AuthorizationInvalid
    );
}
