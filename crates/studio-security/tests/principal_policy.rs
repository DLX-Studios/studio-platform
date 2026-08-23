#![allow(missing_docs)]

use studio_security::{ActionGate, CapabilityId, PluginPrincipal, SecurityErrorCode, TrustMode};

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
fn principal_identity_compares_every_verified_and_runtime_dimension() {
    let expected = principal(1);
    assert_eq!(expected, principal(1));
    assert_ne!(expected, principal(2));
    assert_ne!(
        expected,
        PluginPrincipal::new(
            "publisher-key-2",
            "com.example.pos",
            [7; 32],
            [1; 16],
            TrustMode::Production,
        )
        .unwrap()
    );
    assert_ne!(
        expected,
        PluginPrincipal::new(
            "publisher-key-1",
            "com.example.other",
            [7; 32],
            [1; 16],
            TrustMode::Production,
        )
        .unwrap()
    );
    assert_ne!(
        expected,
        PluginPrincipal::new(
            "publisher-key-1",
            "com.example.pos",
            [8; 32],
            [1; 16],
            TrustMode::Production,
        )
        .unwrap()
    );
    assert_ne!(
        expected,
        PluginPrincipal::new(
            "publisher-key-1",
            "com.example.pos",
            [7; 32],
            [1; 16],
            TrustMode::Development,
        )
        .unwrap()
    );
}

#[test]
fn action_gate_requires_owner_declaration_policy_and_closed_operation() {
    let owner = principal(1);
    let mut gate = ActionGate::new(
        owner.clone(),
        [CapabilityId::PaymentSimulate],
        [CapabilityId::PaymentSimulate, CapabilityId::PrinterSimulate],
        16,
    );

    gate.begin(&owner, CapabilityId::PaymentSimulate, "charge", "request-1")
        .unwrap();
    assert_eq!(gate.pending_len(), 1);

    assert_eq!(
        gate.begin(
            &principal(2),
            CapabilityId::PaymentSimulate,
            "charge",
            "foreign",
        )
        .unwrap_err()
        .code(),
        SecurityErrorCode::CapabilityDenied
    );
    assert_eq!(
        gate.begin(
            &owner,
            CapabilityId::PrinterSimulate,
            "preview",
            "undeclared",
        )
        .unwrap_err()
        .code(),
        SecurityErrorCode::CapabilityDenied
    );
    assert_eq!(
        gate.begin(
            &owner,
            CapabilityId::PaymentSimulate,
            "refund",
            "unknown-operation",
        )
        .unwrap_err()
        .code(),
        SecurityErrorCode::CapabilityDenied
    );

    let mut policy_denied = ActionGate::new(owner.clone(), [CapabilityId::PaymentSimulate], [], 16);
    assert_eq!(
        policy_denied
            .begin(&owner, CapabilityId::PaymentSimulate, "charge", "denied",)
            .unwrap_err()
            .code(),
        SecurityErrorCode::CapabilityDenied
    );
}

#[test]
fn pending_action_ids_are_unique_and_the_host_ceiling_is_enforced() {
    let owner = principal(1);
    let mut gate = ActionGate::new(
        owner.clone(),
        [CapabilityId::PaymentSimulate],
        [CapabilityId::PaymentSimulate],
        16,
    );

    for index in 0..16 {
        gate.begin(
            &owner,
            CapabilityId::PaymentSimulate,
            "charge",
            &format!("request-{index}"),
        )
        .unwrap();
    }
    assert_eq!(gate.pending_len(), 16);
    assert_eq!(
        gate.begin(
            &owner,
            CapabilityId::PaymentSimulate,
            "charge",
            "request-16",
        )
        .unwrap_err()
        .code(),
        SecurityErrorCode::QueueFull
    );

    gate.complete("request-0").unwrap();
    gate.begin(
        &owner,
        CapabilityId::PaymentSimulate,
        "charge",
        "request-16",
    )
    .unwrap();
    assert_eq!(
        gate.begin(
            &owner,
            CapabilityId::PaymentSimulate,
            "charge",
            "request-16",
        )
        .unwrap_err()
        .code(),
        SecurityErrorCode::RequestInvalid
    );
}
