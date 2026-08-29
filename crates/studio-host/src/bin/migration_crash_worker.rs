//! Child process for the signed-migration crash rehearsal.
//!
//! Sequence: open the store with `Durability::Every`, initialize the
//! application-data migration envelope at schema version 1 with
//! `{"stable": true}`, then begin a signed v1→v2 migration whose
//! action parks forever inside the `Applying` lifecycle. The parent
//! force-kills this process at the marker and proves that the next
//! open recovers to the prior durable state or completes idempotently,
//! never exposing a half-migrated store.

use std::{collections::BTreeMap, env, path::PathBuf, thread, time::Duration};

use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use studio_host::{Durability, EmbeddedLocalStore, MigrationRunner, MigrationState};
use studio_package::{
    ArchiveFiles, ArchivePolicy, CanonicalBundleInput, ManifestPolicy, TrustStore,
    TrustedPublisherKey, VerifiedMigrationBundle, build_archive, canonical_bundle_document,
    inspect_archive,
};

const PAUSE_MARKER_FILE: &str = ".studio-migration-test-paused";

fn signed_package() -> VerifiedMigrationBundle {
    let migration_entry = "assets/migrations/v1-to-v2.json";
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
        "assets": [migration_entry],
        "migrations": [{
            "id": "v1-to-v2",
            "fromVersion": 1,
            "toVersion": 2,
            "entry": migration_entry
        }]
    });
    let module = vec![0, 97, 115, 109];
    let migration = br#"{"operation":"add-version"}"#.to_vec();
    let assets = BTreeMap::from([(migration_entry.to_owned(), migration)]);
    let input = CanonicalBundleInput {
        manifest: manifest.clone(),
        module_path: "module.wasm".to_owned(),
        module: module.clone(),
        assets: assets.clone(),
    };
    let signing_key = SigningKey::from_bytes(&[9; 32]);
    let signature = signing_key
        .sign(&canonical_bundle_document(&input).expect("canonical document"))
        .to_bytes();
    let archive = build_archive(
        &ArchiveFiles {
            manifest: serde_json::to_vec(&manifest).expect("manifest JSON"),
            module,
            signature: signature.to_vec(),
            assets,
        },
        ArchivePolicy::default(),
    )
    .expect("archive builds");
    let inspected = inspect_archive(&archive, ArchivePolicy::default()).expect("archive inspects");
    let trust = TrustStore::from_keys([TrustedPublisherKey {
        publisher_id: "example".to_owned(),
        key_id: "key-1".to_owned(),
        verifying_key: signing_key.verifying_key().to_bytes(),
        enabled: true,
    }])
    .expect("trust store");
    VerifiedMigrationBundle::admit(&inspected, ManifestPolicy::default(), &trust)
        .expect("migration bundle admits")
}

fn main() {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let Some(directory) = arguments.next() else {
        std::process::exit(2);
    };
    if arguments.next().is_some() {
        std::process::exit(2);
    }
    let directory = PathBuf::from(directory);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("test runtime starts");
    runtime.block_on(async move {
        let store = EmbeddedLocalStore::open(&directory, Durability::Every)
            .await
            .expect("worker opens LocalStore");
        let runner = MigrationRunner::new(&store);
        // Fresh temp directory: initialize v1 state. Ignore if already present
        // (worker is only launched once per rehearsal, but keep it idempotent).
        let initial = MigrationState::new(1, json!({"stable": true})).expect("initial state");
        let _ = runner.initialize(initial).await;

        let package = signed_package();
        let marker = directory.join(PAUSE_MARKER_FILE);
        // `run` persists RecoveryPointCreated then Applying before invoking this
        // closure. Parking here leaves a durable recovery point and an
        // `Applying` lifecycle on disk, exactly the mid-migration crash the
        // rehearsal must recover from without exposing half-migrated data.
        let _ = runner
            .run(&package, |_migration, _asset, _candidate: &mut Value| {
                let _ = std::fs::write(&marker, b"paused");
                loop {
                    thread::sleep(Duration::from_millis(50));
                }
            })
            .await;
        unreachable!("worker is force-terminated while paused inside migration");
    });
}
