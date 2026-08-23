#![allow(missing_docs)]

use std::time::Instant;

use studio_components::{HostSecretInput, SecretInputErrorCode};
use studio_security::{PluginPrincipal, SecretPurpose, SecretRegistry, TrustMode};
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

#[test]
fn secret_capture_projects_only_readiness_and_an_opaque_reference() {
    let owner = InstanceId::new("instance-a").unwrap();
    let mut input = HostSecretInput::new(
        owner.clone(),
        principal(1),
        "payment-pin",
        SecretPurpose::PaymentPin,
        "checkout-1",
    )
    .unwrap();
    let mut registry = SecretRegistry::new();
    let raw_pin = b"8642";

    let event = input
        .capture_at(&owner, &mut registry, raw_pin, Instant::now())
        .unwrap();
    let encoded = serde_json::to_vec(&event).unwrap();
    let encoded_text = String::from_utf8(encoded.clone()).unwrap();

    assert!(
        !encoded
            .windows(raw_pin.len())
            .any(|window| window == raw_pin)
    );
    assert!(!encoded_text.contains("8642"));
    assert!(encoded_text.contains("authorization_ref"));
    assert!(encoded_text.contains("payment_pin"));
    assert!(encoded_text.contains("expires_in_seconds"));
    assert_eq!(input.snapshot().node_id, "payment-pin");
    assert!(input.snapshot().ready);
    assert!(!format!("{:?}", input.snapshot()).contains("8642"));

    let captured = input
        .consume_at(&owner, &mut registry, Instant::now(), <[u8]>::to_vec)
        .unwrap();
    assert_eq!(captured, raw_pin);
    assert!(!input.snapshot().ready);
}

#[test]
fn secret_input_rejects_foreign_owners_and_teardown_revokes_without_leaking() {
    let owner = InstanceId::new("instance-a").unwrap();
    let foreign = InstanceId::new("instance-b").unwrap();
    let mut input = HostSecretInput::new(
        owner.clone(),
        principal(1),
        "payment-pin",
        SecretPurpose::PaymentPin,
        "checkout-1",
    )
    .unwrap();
    let mut registry = SecretRegistry::new();
    let error = input
        .capture_at(&foreign, &mut registry, b"9999", Instant::now())
        .unwrap_err();
    assert_eq!(error.code(), SecretInputErrorCode::OwnerMismatch);
    assert!(!error.to_string().contains("9999"));

    input
        .capture_at(&owner, &mut registry, b"9999", Instant::now())
        .unwrap();
    assert_eq!(registry.active_len(), 1);
    input.teardown(&mut registry);
    assert_eq!(registry.active_len(), 0);
    assert!(!input.snapshot().ready);
    assert_eq!(
        input
            .consume_at(&owner, &mut registry, Instant::now(), |_| ())
            .unwrap_err()
            .code(),
        SecretInputErrorCode::AuthorizationInvalid
    );
}
