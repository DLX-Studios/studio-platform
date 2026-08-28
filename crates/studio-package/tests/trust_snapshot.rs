#![allow(missing_docs)]
#![allow(
    clippy::needless_raw_string_hashes,
    clippy::format_collect,
    clippy::needless_pass_by_value
)]

use ed25519_dalek::SigningKey;
use serde_json::json;
use studio_package::{TrustStore, TrustStoreErrorCode};

fn public_key_hex() -> String {
    SigningKey::from_bytes(&[7; 32])
        .verifying_key()
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn snapshot(revocations: serde_json::Value, second_key: Option<serde_json::Value>) -> Vec<u8> {
    let mut keys = vec![json!({
        "publisherId": "example",
        "keyId": "key-old",
        "publicKey": public_key_hex(),
        "validFrom": 100,
        "expiresAt": 300,
        "enabled": true
    })];
    if let Some(key) = second_key {
        keys.push(key);
    }
    serde_json::to_vec(&json!({
        "schemaVersion": 1,
        "snapshotId": "production-42",
        "version": 42,
        "validFrom": 100,
        "expiresAt": 300,
        "keys": keys,
        "revocations": revocations
    }))
    .unwrap()
}

#[test]
fn accepts_active_snapshot_and_retains_safe_release_evidence() {
    let store = TrustStore::from_json_at(&snapshot(json!([]), None), 200).unwrap();
    let evidence = store.evidence().unwrap();
    assert_eq!(evidence.snapshot_id, "production-42");
    assert_eq!(evidence.version, 42);
    assert_eq!(evidence.key_count, 1);
}

#[test]
fn rejects_missing_expired_and_malformed_snapshots_without_raw_details() {
    assert_eq!(
        TrustStore::from_json_at(br#"{}"#, 200).unwrap_err().code(),
        TrustStoreErrorCode::Malformed
    );
    assert_eq!(
        TrustStore::from_json_at(&snapshot(json!([]), None), 300)
            .unwrap_err()
            .code(),
        TrustStoreErrorCode::Expired
    );
    let malformed = snapshot(
        json!([]),
        Some(json!({
            "publisherId": "example",
            "keyId": "key-new",
            "publicKey": "not-a-key",
            "validFrom": 100,
            "expiresAt": 300
        })),
    );
    assert_eq!(
        TrustStore::from_json_at(&malformed, 200)
            .unwrap_err()
            .code(),
        TrustStoreErrorCode::Malformed
    );
}

#[test]
fn revocation_blocks_a_key_while_overlapping_rotation_remains_active() {
    let rotated = json!({
        "publisherId": "example",
        "keyId": "key-new",
        "publicKey": public_key_hex(),
        "validFrom": 150,
        "expiresAt": 400,
        "enabled": true
    });
    let store = TrustStore::from_json_at(
        &snapshot(
            json!([{ "publisherId": "example", "keyId": "key-old" }]),
            Some(rotated),
        ),
        200,
    )
    .unwrap();
    assert!(!store.is_empty());
}

#[test]
fn revoked_only_snapshot_fails_closed() {
    assert_eq!(
        TrustStore::from_json_at(
            &snapshot(
                json!([{ "publisherId": "example", "keyId": "key-old" }]),
                None
            ),
            200,
        )
        .unwrap_err()
        .code(),
        TrustStoreErrorCode::NoActiveKeys
    );
}
