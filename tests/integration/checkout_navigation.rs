#![allow(missing_docs)]

use std::time::{Duration, Instant};

use studio_actions::{
    Checkout, Money, PaymentAuthorization, PaymentOutcome, PaymentRequest, PaymentSimulator,
    PrintPreviewRequest, PrinterSimulator, Receipt, ReceiptLine,
};
use studio_app::HostPreferences;
use studio_navigation::{
    GuardDecision, GuardResponse, HostClock, NavigationGuard, NavigationOperation, NavigationStack,
    StackOwner, TransitionController, TransitionKind, TransitionState,
};
use studio_security::{PluginPrincipal, TrustMode};

struct ConfirmExit;
impl NavigationGuard for ConfirmExit {
    fn evaluate(&mut self, _: &str, _: &str, _: bool) -> GuardResponse {
        GuardResponse::new(GuardDecision::Confirmed, Duration::from_millis(2))
    }
}

#[derive(Default)]
struct Clock(Duration);
impl HostClock for Clock {
    fn elapsed(&self) -> Duration {
        self.0
    }
}

fn principal() -> PluginPrincipal {
    PluginPrincipal::new("publisher", "pos", [7; 32], [8; 16], TrustMode::Production).unwrap()
}

#[test]
fn navigation_recovery_receipt_and_preview_form_one_coherent_flow() {
    let owner = StackOwner::new([8; 16]);
    let mut stack = NavigationStack::new(owner, "/catalog").unwrap();
    let mut guard = ConfirmExit;
    stack.set_local_state(&owner, "search", "beard").unwrap();
    stack
        .apply(&owner, NavigationOperation::Push("/cart"), &mut guard)
        .unwrap();
    stack
        .apply(&owner, NavigationOperation::Pop, &mut guard)
        .unwrap();
    assert_eq!(stack.local_state("search"), Some("beard"));
    stack
        .apply(&owner, NavigationOperation::Push("/payment"), &mut guard)
        .unwrap();
    stack.set_pending_payment(&owner, true).unwrap();
    stack
        .apply(&owner, NavigationOperation::Push("/catalog"), &mut guard)
        .unwrap();

    let checkout = Checkout::new(
        "sale-1",
        "Studio Barber",
        "Verified POS",
        Money::new("USD", 3_780).unwrap(),
    )
    .unwrap();
    let now = Instant::now();
    let mut confirmation = checkout
        .begin_confirmation(now + Duration::from_secs(30))
        .unwrap();
    let confirmed = confirmation.confirm(now).unwrap();
    let request = PaymentRequest::new(
        "payment-approved",
        principal(),
        confirmed.clone(),
        Some(PaymentAuthorization::host_verified()),
    )
    .unwrap();
    let result = PaymentSimulator::new().charge(request, 9_000).unwrap();
    assert_eq!(result.outcome(), PaymentOutcome::Approved);
    let receipt = Receipt::from_approved(
        principal(),
        &confirmed,
        &result,
        vec![ReceiptLine::new("Classic Cut", 1, Money::new("USD", 3_500).unwrap()).unwrap()],
        Money::new("USD", 3_500).unwrap(),
        Money::new("USD", 0).unwrap(),
        Money::new("USD", 280).unwrap(),
    )
    .unwrap();
    let mut printer = PrinterSimulator::new();
    printer.register(receipt.clone()).unwrap();
    let job = printer
        .preview(
            &principal(),
            PrintPreviewRequest::new("preview-1", receipt.id()).unwrap(),
        )
        .unwrap();
    assert_eq!(job.preview().total().minor(), 3_780);

    let clock = Clock::default();
    let mut reduced = TransitionController::new(HostPreferences::new(true).motion());
    reduced.begin(TransitionKind::Push, "/payment", "/receipt", &clock);
    assert_eq!(reduced.sample(&clock), TransitionState::Completed);
    assert_eq!(reduced.current_route(), "/receipt");
}

#[test]
fn declined_timeout_and_unavailable_results_remain_recoverable() {
    let now = Instant::now();
    for (minor, expected, retryable) in [
        (3_801, PaymentOutcome::Declined, false),
        (3_802, PaymentOutcome::Timeout, true),
        (3_803, PaymentOutcome::Unavailable, true),
    ] {
        let checkout = Checkout::new(
            format!("sale-{minor}"),
            "Studio Barber",
            "Verified POS",
            Money::new("USD", minor).unwrap(),
        )
        .unwrap();
        let mut surface = checkout
            .begin_confirmation(now + Duration::from_secs(30))
            .unwrap();
        let confirmed = surface.confirm(now).unwrap();
        let request = PaymentRequest::new(
            format!("payment-{minor}"),
            principal(),
            confirmed,
            Some(PaymentAuthorization::host_verified()),
        )
        .unwrap();
        let result = PaymentSimulator::new().charge(request, 9_000).unwrap();
        assert_eq!(result.outcome(), expected);
        assert_eq!(result.retryable(), retryable);
    }
}
