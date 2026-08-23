#![allow(missing_docs)]

use std::time::{Duration, Instant};

use studio_actions::{
    Checkout, Money, PaymentAuthorization, PaymentRequest, PaymentSimulator, PrintErrorCode,
    PrintPreviewRequest, PrinterSimulator, Receipt, ReceiptLine,
};
use studio_security::{PluginPrincipal, TrustMode};

fn owner(instance: u8) -> PluginPrincipal {
    PluginPrincipal::new(
        "publisher",
        "pos",
        [3; 32],
        [instance; 16],
        TrustMode::Production,
    )
    .unwrap()
}

fn receipt() -> Receipt {
    let principal = owner(4);
    let checkout = Checkout::new(
        "sale-1",
        "Studio Barber",
        "Verified POS",
        Money::new("USD", 3_780).unwrap(),
    )
    .unwrap();
    let mut surface = checkout
        .begin_confirmation(Instant::now() + Duration::from_secs(30))
        .unwrap();
    let confirmed = surface.confirm(Instant::now()).unwrap();
    let payment = PaymentRequest::new(
        "pay-1",
        principal.clone(),
        confirmed.clone(),
        Some(PaymentAuthorization::host_verified()),
    )
    .unwrap();
    let result = PaymentSimulator::new().charge(payment, 500).unwrap();
    Receipt::from_approved(
        principal,
        &confirmed,
        &result,
        vec![ReceiptLine::new("Classic Cut", 1, Money::new("USD", 3_500).unwrap()).unwrap()],
        Money::new("USD", 3_500).unwrap(),
        Money::new("USD", 0).unwrap(),
        Money::new("USD", 280).unwrap(),
    )
    .unwrap()
}

#[test]
fn one_owned_structured_request_creates_one_host_preview_job() {
    let receipt = receipt();
    let mut printer = PrinterSimulator::new();
    printer.register(receipt.clone()).unwrap();
    let request = PrintPreviewRequest::new("print-1", receipt.id()).unwrap();
    let job = printer.preview(&owner(4), request.clone()).unwrap();
    assert_eq!(job.receipt_id(), receipt.id());
    assert_eq!(job.preview().merchant(), "Studio Barber");
    assert_eq!(job.preview().lines()[0].label(), "Classic Cut");
    assert_eq!(printer.job_count(), 1);

    assert_eq!(
        printer.preview(&owner(4), request).unwrap_err().code(),
        PrintErrorCode::DuplicateRequest
    );
    assert_eq!(printer.job_count(), 1);
}

#[test]
fn foreign_owners_and_unstructured_device_channels_are_rejected() {
    let receipt = receipt();
    let mut printer = PrinterSimulator::new();
    printer.register(receipt.clone()).unwrap();
    assert_eq!(
        printer
            .preview(
                &owner(9),
                PrintPreviewRequest::new("print-foreign", receipt.id()).unwrap(),
            )
            .unwrap_err()
            .code(),
        PrintErrorCode::OwnerMismatch
    );
    for raw in [
        br#"{"request_id":"raw","receipt_id":"x","escpos":"\u001b@"}"#.as_slice(),
        br#"{"request_id":"raw","receipt_id":"x","device":"/dev/usb/lp0"}"#.as_slice(),
        &[0x1b, 0x40][..],
    ] {
        assert_eq!(
            PrintPreviewRequest::decode(raw).unwrap_err().code(),
            PrintErrorCode::InvalidRequest
        );
    }
}
