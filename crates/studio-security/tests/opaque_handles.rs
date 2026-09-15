#![allow(missing_docs)]

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use studio_security::{
    OpaqueHandle, PluginPrincipal, SecretErrorCode, SecretPurpose, SecretRegistry, TrustMode,
};

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
fn handles_are_random_256_bit_values_and_debug_output_is_redacted() {
    let owner = principal(1);
    let now = Instant::now();
    let mut registry = SecretRegistry::new();
    let handles = (0..256)
        .map(|_| {
            registry
                .capture_at(
                    owner.clone(),
                    SecretPurpose::PaymentPin,
                    "checkout-1",
                    b"1234",
                    now,
                )
                .unwrap()
        })
        .collect::<HashSet<OpaqueHandle>>();

    assert_eq!(handles.len(), 256);
    assert!(handles.iter().all(|handle| handle.as_bytes().len() == 32));
    assert!(handles.iter().all(|handle| handle.as_bytes() != &[0; 32]));
    assert_eq!(
        format!("{:?}", handles.iter().next().unwrap()),
        "OpaqueHandle(REDACTED)"
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one lifecycle test keeps all non-oracular cases comparable"
)]
fn resolution_is_exactly_scoped_expires_at_120_seconds_and_is_single_use() {
    let owner = principal(1);
    let foreign = principal(2);
    let now = Instant::now();
    let mut registry = SecretRegistry::new();
    let handle = registry
        .capture_at(
            owner.clone(),
            SecretPurpose::PaymentPin,
            "checkout-1",
            b"2468",
            now,
        )
        .unwrap();

    for result in [
        registry.consume_at(
            &handle,
            &foreign,
            SecretPurpose::PaymentPin,
            "checkout-1",
            now,
            |_| (),
        ),
        registry.consume_at(
            &handle,
            &owner,
            SecretPurpose::DevicePassword,
            "checkout-1",
            now,
            |_| (),
        ),
        registry.consume_at(
            &handle,
            &owner,
            SecretPurpose::PaymentPin,
            "checkout-2",
            now,
            |_| (),
        ),
        registry.consume_at(
            &OpaqueHandle::from_bytes([9; 32]),
            &owner,
            SecretPurpose::PaymentPin,
            "checkout-1",
            now,
            |_| (),
        ),
    ] {
        assert_eq!(
            result.unwrap_err().code(),
            SecretErrorCode::AuthorizationInvalid
        );
    }

    let observed = registry
        .consume_at(
            &handle,
            &owner,
            SecretPurpose::PaymentPin,
            "checkout-1",
            now + Duration::from_secs(119),
            <[u8]>::to_vec,
        )
        .unwrap();
    assert_eq!(observed, b"2468");
    assert_eq!(registry.cleared_count(), 1);
    assert_eq!(
        registry
            .consume_at(
                &handle,
                &owner,
                SecretPurpose::PaymentPin,
                "checkout-1",
                now + Duration::from_secs(119),
                |_| (),
            )
            .unwrap_err()
            .code(),
        SecretErrorCode::AuthorizationInvalid
    );

    let expired = registry
        .capture_at(
            owner.clone(),
            SecretPurpose::PaymentPin,
            "checkout-1",
            b"0000",
            now,
        )
        .unwrap();
    assert_eq!(
        registry
            .consume_at(
                &expired,
                &owner,
                SecretPurpose::PaymentPin,
                "checkout-1",
                now + Duration::from_mins(2),
                |_| (),
            )
            .unwrap_err()
            .code(),
        SecretErrorCode::AuthorizationInvalid
    );
    assert_eq!(registry.cleared_count(), 2);
}

#[test]
fn explicit_and_owner_revocation_clear_records_without_an_oracle() {
    let owner = principal(1);
    let other = principal(2);
    let now = Instant::now();
    let mut registry = SecretRegistry::new();
    let first = registry
        .capture_at(owner.clone(), SecretPurpose::PaymentPin, "a", b"1111", now)
        .unwrap();
    let second = registry
        .capture_at(owner.clone(), SecretPurpose::PaymentPin, "b", b"2222", now)
        .unwrap();
    let other_handle = registry
        .capture_at(other.clone(), SecretPurpose::PaymentPin, "c", b"3333", now)
        .unwrap();

    registry.revoke(&first);
    assert_eq!(registry.revoke_owner(&owner), 1);
    assert_eq!(registry.active_len(), 1);
    assert_eq!(registry.cleared_count(), 2);

    for (handle, principal, session) in [(&first, &owner, "a"), (&second, &owner, "b")] {
        assert_eq!(
            registry
                .consume_at(
                    handle,
                    principal,
                    SecretPurpose::PaymentPin,
                    session,
                    now,
                    |_| (),
                )
                .unwrap_err()
                .code(),
            SecretErrorCode::AuthorizationInvalid
        );
    }
    registry
        .consume_at(
            &other_handle,
            &other,
            SecretPurpose::PaymentPin,
            "c",
            now,
            |_| (),
        )
        .unwrap();
    assert_eq!(registry.cleared_count(), 3);
}
