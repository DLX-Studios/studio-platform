#![allow(missing_docs)]

use std::time::{Duration, Instant};

use studio_actions::{
    Checkout, Money, PaymentAuthorization, PaymentOutcome, PaymentRequest, PaymentSimulator,
    Receipt, ReceiptErrorCode, ReceiptLine,
};
use studio_security::{PluginPrincipal, TrustMode};

fn owner() -> PluginPrincipal {
    PluginPrincipal::new("publisher", "pos", [3; 32], [4; 16], TrustMode::Production).unwrap()
}

fn payment(
    minor: i64,
) -> (
    studio_actions::ConfirmedPayment,
    studio_actions::PaymentResult,
) {
    let checkout = Checkout::new(
        "sale-1",
        "Studio Barber",
        "Verified POS",
        Money::new("USD", minor).unwrap(),
    )
    .unwrap();
    let mut confirmation = checkout
        .begin_confirmation(Instant::now() + Duration::from_secs(30))
        .unwrap();
    let confirmed = confirmation.confirm(Instant::now()).unwrap();
    let request = PaymentRequest::new(
        "payment-1",
        owner(),
        confirmed.clone(),
        Some(PaymentAuthorization::host_verified()),
    )
    .unwrap();
    let result = PaymentSimulator::new().charge(request, 1_725_000).unwrap();
    (confirmed, result)
}

#[test]
fn approved_result_creates_an_exact_immutable_structured_receipt() {
    let (confirmed, result) = payment(10_800);
    let lines = vec![
        ReceiptLine::new("Classic Cut", 2, Money::new("USD", 3_500).unwrap()).unwrap(),
        ReceiptLine::new("Beard Trim", 1, Money::new("USD", 1_800).unwrap()).unwrap(),
    ];
    let receipt = Receipt::from_approved(
        owner(),
        &confirmed,
        &result,
        lines,
        Money::new("USD", 8_800).unwrap(),
        Money::new("USD", 0).unwrap(),
        Money::new("USD", 2_000).unwrap(),
    )
    .unwrap();

    assert_eq!(receipt.merchant(), "Studio Barber");
    assert_eq!(receipt.lines()[0].quantity(), 2);
    assert_eq!(receipt.lines()[0].line_total().minor(), 7_000);
    assert_eq!(receipt.subtotal().minor(), 8_800);
    assert_eq!(receipt.total(), confirmed.amount());
    assert_eq!(receipt.result_reference(), result.result_reference());
    assert_eq!(receipt.host_timestamp_millis(), 1_725_000);
    assert_eq!(receipt.currency(), "USD");
    assert!(!format!("{receipt:?}").contains("opaque_"));
}

#[test]
fn non_approval_or_inconsistent_confirmed_money_cannot_create_a_receipt() {
    let (confirmed, declined) = payment(10_801);
    assert_eq!(declined.outcome(), PaymentOutcome::Declined);
    let line = ReceiptLine::new("Classic Cut", 1, Money::new("USD", 10_801).unwrap()).unwrap();
    assert_eq!(
        Receipt::from_approved(
            owner(),
            &confirmed,
            &declined,
            vec![line],
            Money::new("USD", 10_801).unwrap(),
            Money::new("USD", 0).unwrap(),
            Money::new("USD", 0).unwrap(),
        )
        .unwrap_err()
        .code(),
        ReceiptErrorCode::PaymentNotApproved
    );

    let (confirmed, approved) = payment(10_800);
    let wrong = ReceiptLine::new("Wrong", 1, Money::new("USD", 9_999).unwrap()).unwrap();
    assert_eq!(
        Receipt::from_approved(
            owner(),
            &confirmed,
            &approved,
            vec![wrong],
            Money::new("USD", 9_999).unwrap(),
            Money::new("USD", 0).unwrap(),
            Money::new("USD", 0).unwrap(),
        )
        .unwrap_err()
        .code(),
        ReceiptErrorCode::TotalsMismatch
    );
}
