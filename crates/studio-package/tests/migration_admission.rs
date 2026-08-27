#![allow(missing_docs)]

use std::collections::BTreeMap;

use ed25519_dalek::{Signer, SigningKey};
use serde_json::json;
use studio_package::{
    ArchiveFiles, ArchivePolicy, CanonicalBundleInput, ManifestPolicy, MigrationAdmissionError,
    TrustStore, TrustedPublisherKey, VerifiedMigrationBundle, build_archive,
    canonical_bundle_document, inspect_archive,
};

fn archive(signature: Vec<u8>) -> Vec<u8> {
    let manifest = json!({
        "schemaVersion": 1,
        "id": "com.example.app",
        "name": "Example App",
        "version": "1.0.0",
        "publisher": {"id": "example", "keyId": "key-1"},
        "entry": "module.wasm",
        "sdkVersion": "^0.1.0",
        "protocolVersion": 1,
        "capabilities": [],
        "limits": {"memoryMiB": 16, "eventFuel": 10000000},
        "assets": ["assets/migrations/v1-to-v2.json"],
        "migrations": [{
            "id": "v1-to-v2",
            "fromVersion": 1,
            "toVersion": 2,
            "entry": "assets/migrations/v1-to-v2.json"
        }]
    });
    build_archive(
        &ArchiveFiles {
            manifest: serde_json::to_vec(&manifest).unwrap(),
            module: vec![0, 97, 115, 109],
            signature,
            assets: BTreeMap::from([(
                "assets/migrations/v1-to-v2.json".to_owned(),
                br#"{"operation":"add-version"}"#.to_vec(),
            )]),
        },
        ArchivePolicy::default(),
    )
    .unwrap()
}

fn signed_archive(signing_key: &SigningKey) -> Vec<u8> {
    let value = json!({
        "schemaVersion": 1,
        "id": "com.example.app",
        "name": "Example App",
        "version": "1.0.0",
        "publisher": {"id": "example", "keyId": "key-1"},
        "entry": "module.wasm",
        "sdkVersion": "^0.1.0",
        "protocolVersion": 1,
        "capabilities": [],
        "limits": {"memoryMiB": 16, "eventFuel": 10000000},
        "assets": ["assets/migrations/v1-to-v2.json"],
        "migrations": [{"id":"v1-to-v2","fromVersion":1,"toVersion":2,"entry":"assets/migrations/v1-to-v2.json"}]
    });
    let module = vec![0, 97, 115, 109];
    let assets = BTreeMap::from([(
        "assets/migrations/v1-to-v2.json".to_owned(),
        br#"{"operation":"add-version"}"#.to_vec(),
    )]);
    let document = canonical_bundle_document(&CanonicalBundleInput {
        manifest: value.clone(),
        module_path: "module.wasm".to_owned(),
        module: module.clone(),
        assets: assets.clone(),
    })
    .unwrap();
    archive(signing_key.sign(&document).to_bytes().to_vec())
}

fn trust(signing_key: &SigningKey) -> TrustStore {
    TrustStore::from_keys([TrustedPublisherKey {
        publisher_id: "example".to_owned(),
        key_id: "key-1".to_owned(),
        verifying_key: signing_key.verifying_key().to_bytes(),
        enabled: true,
    }])
    .unwrap()
}

#[test]
fn migration_admission_requires_a_valid_publisher_signature() {
    let signing_key = SigningKey::from_bytes(&[11; 32]);
    let inspected = inspect_archive(&signed_archive(&signing_key), ArchivePolicy::default()).unwrap();
    let admitted = VerifiedMigrationBundle::admit(
        &inspected,
        ManifestPolicy::default(),
        &trust(&signing_key),
    )
    .unwrap();
    assert_eq!(admitted.manifest().migrations.len(), 1);
    assert!(admitted.migration_asset("assets/migrations/v1-to-v2.json").is_some());

    let unsigned = inspect_archive(&archive(vec![0; 64]), ArchivePolicy::default()).unwrap();
    assert!(matches!(
        VerifiedMigrationBundle::admit(&unsigned, ManifestPolicy::default(), &trust(&signing_key)),
        Err(MigrationAdmissionError::Integrity(_))
    ));
}
