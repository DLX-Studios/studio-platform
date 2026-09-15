#![allow(missing_docs)]

use std::collections::BTreeMap;

use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use studio_package::{
    CanonicalBundleInput, IntegrityErrorCode, TrustStore, TrustedPublisherKey,
    canonical_bundle_document, canonicalize_json, verify_bundle_signature,
};

fn manifest() -> Value {
    json!({
        "schemaVersion": 1,
        "id": "com.example.pos",
        "name": "Example POS",
        "version": "0.1.0",
        "publisher": {"id": "example", "keyId": "key-1"},
        "entry": "module.wasm",
        "sdkVersion": "^0.1.0",
        "protocolVersion": 1,
        "capabilities": [],
        "limits": {"memoryMiB": 16, "eventFuel": 10_000_000},
        "assets": ["assets/catalog.json"]
    })
}

fn input() -> CanonicalBundleInput {
    CanonicalBundleInput {
        manifest: manifest(),
        module_path: "module.wasm".to_owned(),
        module: b"module-v1".to_vec(),
        assets: BTreeMap::from([(
            "assets/catalog.json".to_owned(),
            br#"{"items":[]}"#.to_vec(),
        )]),
    }
}

fn key() -> SigningKey {
    SigningKey::from_bytes(&[7; 32])
}

fn trust_store(signing_key: &SigningKey) -> TrustStore {
    TrustStore::from_keys([TrustedPublisherKey {
        publisher_id: "example".to_owned(),
        key_id: "key-1".to_owned(),
        verifying_key: signing_key.verifying_key().to_bytes(),
        enabled: true,
    }])
    .unwrap()
}

#[test]
fn matches_rfc_8785_number_string_and_property_order_vectors() {
    let value = json!({
        "numbers": [333_333_333.333_333_3_f64, 1E30_f64, 4.50_f64, 2e-3_f64, 1e-27_f64],
        "string": "€$\u{000f}\nA'B\"\\\"/",
        "literals": [null, true, false],
        "z": 1,
        "a": 2
    });
    let canonical = String::from_utf8(canonicalize_json(&value).unwrap()).unwrap();
    assert_eq!(
        canonical,
        r#"{"a":2,"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"string":"€$\u000f\nA'B\"\\\"/","z":1}"#
    );
}

#[test]
fn signs_domain_separated_manifest_module_and_sorted_asset_digests() {
    let input = input();
    let document = canonical_bundle_document(&input).unwrap();
    let text = String::from_utf8(document.clone()).unwrap();
    assert!(text.contains("studio.bundle.signature.v1"));
    assert!(text.contains("assets/catalog.json"));
    assert!(!text.contains("module-v1"));

    let signing_key = key();
    let signature = signing_key.sign(&document).to_bytes();
    let verified = verify_bundle_signature(
        &input,
        &signature,
        "example",
        "key-1",
        &trust_store(&signing_key),
    )
    .unwrap();
    assert_eq!(verified.signed_document, document);
    assert_eq!(verified.document_sha256.len(), 32);
}

#[test]
fn rejects_manifest_module_and_asset_mutation_with_the_original_signature() {
    let original = input();
    let signing_key = key();
    let signature = signing_key
        .sign(&canonical_bundle_document(&original).unwrap())
        .to_bytes();
    let store = trust_store(&signing_key);

    let mut mutations = Vec::new();
    let mut manifest_mutation = original.clone();
    manifest_mutation.manifest["name"] = json!("Mutated");
    mutations.push(manifest_mutation);
    let mut module_mutation = original.clone();
    module_mutation.module.push(0);
    mutations.push(module_mutation);
    let mut asset_mutation = original;
    asset_mutation
        .assets
        .get_mut("assets/catalog.json")
        .unwrap()
        .push(0);
    mutations.push(asset_mutation);

    for mutation in mutations {
        assert_eq!(
            verify_bundle_signature(&mutation, &signature, "example", "key-1", &store)
                .unwrap_err()
                .code(),
            IntegrityErrorCode::SignatureInvalid
        );
    }
}

#[test]
fn rejects_wrong_publisher_key_disabled_key_and_unknown_key_without_oracles() {
    let input = input();
    let signing_key = key();
    let document = canonical_bundle_document(&input).unwrap();
    let signature = signing_key.sign(&document).to_bytes();

    let enabled = trust_store(&signing_key);
    for (publisher, key_id) in [("other", "key-1"), ("example", "unknown")] {
        assert_eq!(
            verify_bundle_signature(&input, &signature, publisher, key_id, &enabled)
                .unwrap_err()
                .code(),
            IntegrityErrorCode::TrustInvalid
        );
    }

    let disabled = TrustStore::from_keys([TrustedPublisherKey {
        publisher_id: "example".into(),
        key_id: "key-1".into(),
        verifying_key: signing_key.verifying_key().to_bytes(),
        enabled: false,
    }])
    .unwrap();
    assert_eq!(
        verify_bundle_signature(&input, &signature, "example", "key-1", &disabled)
            .unwrap_err()
            .code(),
        IntegrityErrorCode::TrustInvalid
    );
}

#[test]
fn accepts_only_exact_raw_64_byte_ed25519_signatures() {
    let input = input();
    let signing_key = key();
    let store = trust_store(&signing_key);
    for signature in [vec![0; 63], vec![0; 65], vec![0; 64]] {
        assert_eq!(
            verify_bundle_signature(&input, &signature, "example", "key-1", &store)
                .unwrap_err()
                .code(),
            IntegrityErrorCode::SignatureInvalid
        );
    }
}
