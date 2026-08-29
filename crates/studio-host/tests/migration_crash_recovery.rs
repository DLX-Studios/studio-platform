#![allow(missing_docs)]

//! Signed migration v1→v2 crash rehearsal on the real RocksDB engine.
//!
//! The worker applies migration 1 inside a signed bundle and parks forever
//! while in `Applying` (after the durable recovery point). The parent
//! force-kills it, reopens the same RocksDB directory, and proves:
//! - no half-migrated state is visible
//! - the prior state is recoverable or the migration completes idempotently
//! - a guest would never launch from the half-migrated store
//! - engine upgrades remain separate (engine_format_version unchanged)

use std::{
    collections::BTreeMap,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use ed25519_dalek::{Signer, SigningKey};
use serde_json::json;
use studio_host::{
    Durability, EmbeddedLocalStore, LocalStore, MigrationErrorCode, MigrationLifecycle,
    MigrationRunner, MigrationState, MigrationStepError,
};
use studio_package::{
    ArchiveFiles, ArchivePolicy, CanonicalBundleInput, ManifestPolicy, TrustStore,
    TrustedPublisherKey, VerifiedMigrationBundle, build_archive, canonical_bundle_document,
    inspect_archive,
};
use tokio::runtime::Builder;

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

#[test]
fn signed_migration_v1_to_v2_crash_rehearsal_restores_prior_or_completes_atomically() {
    let directory = tempfile::tempdir().expect("temporary RocksDB directory");
    let marker = directory.path().join(PAUSE_MARKER_FILE);
    let worker = std::env::var("CARGO_BIN_EXE_migration_crash_worker")
        .expect("Cargo provides the migration crash worker path");
    let mut child = Command::new(worker)
        .arg(directory.path())
        .spawn()
        .expect("migration crash worker starts");

    let deadline = Instant::now() + Duration::from_secs(20);
    while !marker.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        marker.exists(),
        "worker must pause inside the Applying migration step"
    );
    child.kill().expect("forced termination succeeds");
    child.wait().expect("worker reaped");

    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("recovery runtime starts");
    runtime.block_on(async {
        let recovered = EmbeddedLocalStore::open(directory.path(), Durability::Every)
            .await
            .expect("RocksDB reopens after forced termination");
        let metadata_before = recovered
            .metadata()
            .await
            .expect("engine metadata survives forced termination");
        // Engine upgrade path is separate: engine_format_version must not have
        // been changed by the application migration attempt.
        assert_eq!(
            metadata_before.engine_format_version(),
            1,
            "migration must never change engine_format_version"
        );

        let runner = MigrationRunner::new(&recovered);
        let state_after_crash = runner.state().await.expect("migration state is readable");
        // Never half-migrated: either still at v1 with prior data, or already at
        // a clean Committed v2. The data must never contain a partial candidate.
        assert_ne!(
            state_after_crash.data(),
            &json!({"stable": true, "half": true}),
            "no half-migrated candidate may be persisted"
        );
        // Recovery point must exist if we did not reach Committed; it is the
        // prior-or-new guarantee (restore to prior).
        if !matches!(state_after_crash.lifecycle(), MigrationLifecycle::Committed) {
            assert!(
                state_after_crash.recovery_point().is_some(),
                "interrupted migration must retain its durable recovery point"
            );
            // Prior data is still intact via the recovery point / state
            assert_eq!(state_after_crash.schema_version(), 1);
            assert_eq!(state_after_crash.data(), &json!({"stable": true}));
            // Lifecycle must be recoverable (RecoveryPointCreated/Applying/Validating), not Committed
            assert!(
                matches!(
                    state_after_crash.lifecycle(),
                    MigrationLifecycle::RecoveryPointCreated { .. }
                        | MigrationLifecycle::Applying { .. }
                        | MigrationLifecycle::Validating { .. }
                ),
                "interrupted lifecycle must be recoverable, got {:?}",
                state_after_crash.lifecycle()
            );
        }

        // Reopen must not launch a guest from a half-migrated store; retrying
        // the migration deterministically completes atomically.
        let package = signed_package();
        let report = runner
            .run(&package, |_migration, asset, data| {
                assert!(!asset.is_empty());
                // Recovery must have restored prior data before this retry
                assert_eq!(data, &json!({"stable": true}));
                data["schema"] = json!(2);
                Ok(())
            })
            .await
            .expect("retry recovers and applies atomically");
        assert_eq!(report.initial_version, 1);
        assert_eq!(report.final_version, 2);
        assert_eq!(report.applied, vec!["v1-to-v2".to_owned()]);
        assert!(report.recovery_point_id.is_some());

        let final_state = runner.state().await.expect("final state");
        assert_eq!(final_state.schema_version(), 2);
        assert_eq!(
            final_state.data(),
            &json!({"stable": true, "schema": 2}),
            "final state is exactly prior plus migration, never half"
        );
        assert!(matches!(
            final_state.lifecycle(),
            MigrationLifecycle::Committed
        ));

        // Idempotent replay: second run is a no-op
        let replay = runner
            .run(&package, |_migration, _asset, _data| {
                panic!("completed migration must not run again");
            })
            .await
            .expect("replay is a no-op");
        assert!(replay.applied.is_empty());
        assert_eq!(replay.final_version, 2);

        // Engine version still unchanged after committed migration
        let metadata_after = recovered
            .metadata()
            .await
            .expect("metadata after migration");
        assert_eq!(metadata_after.engine_format_version(), 1);
        assert_eq!(metadata_after.schema_version(), 1);

        recovered.close().await.expect("recovered store closes");
    });
}

#[tokio::test]
async fn signed_migration_quarantine_survives_reopen_and_requires_explicit_restore() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let runner = MigrationRunner::new(&store);
    runner
        .initialize(MigrationState::new(1, json!({"stable": true})).expect("state"))
        .await
        .expect("initial state persists");
    let package = signed_package();

    // Action rejection quarantines with a safe diagnostic, never persisting half
    assert_eq!(
        runner
            .run(&package, |_migration, _asset, _data| Err(
                MigrationStepError::Rejected
            ))
            .await
            .unwrap_err()
            .code(),
        MigrationErrorCode::ActionFailed
    );
    let quarantined = runner.state().await.expect("quarantine persists");
    assert!(matches!(
        quarantined.lifecycle(),
        MigrationLifecycle::Quarantined { .. }
    ));
    assert_eq!(quarantined.schema_version(), 1);
    assert_eq!(quarantined.data(), &json!({"stable": true}));
    assert!(
        quarantined.recovery_point().is_some(),
        "recovery point retained"
    );
    // Safe diagnostic: no storage/package/application-data details in Display
    let err = MigrationRunner::new(&store)
        .run(&package, |_, _, _| Ok(()))
        .await
        .unwrap_err();
    assert_eq!(err.code(), MigrationErrorCode::Quarantined);
    assert_eq!(
        err.to_string(),
        "application migration is quarantined; restore the recovery point before retrying"
    );

    // Persisted quarantine survives reopen (never launches guest from half state)
    store.close().await.expect("store closes");
    let reopened = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store reopens");
    let runner = MigrationRunner::new(&reopened);
    let after_reopen = runner.state().await.expect("state after reopen");
    assert!(matches!(
        after_reopen.lifecycle(),
        MigrationLifecycle::Quarantined { .. }
    ));
    assert_eq!(after_reopen.data(), &json!({"stable": true}));
    // Still quarantined without explicit restore
    assert_eq!(
        runner
            .run(&package, |_, _, _| Ok(()))
            .await
            .unwrap_err()
            .code(),
        MigrationErrorCode::Quarantined
    );
    // Explicit restore recovers prior data and clears quarantine
    let restored = runner
        .restore_recovery_point()
        .await
        .expect("recovery point restores");
    assert!(matches!(restored.lifecycle(), MigrationLifecycle::Idle));
    assert_eq!(restored.data(), &json!({"stable": true}));
    // After restore, migration can complete
    let report = runner
        .run(&package, |_, _, data| {
            data["schema"] = json!(2);
            Ok(())
        })
        .await
        .expect("migration succeeds after restore");
    assert_eq!(report.final_version, 2);
    reopened.close().await.expect("store closes");
}

#[tokio::test]
async fn signed_migration_validator_failure_never_persists_half_migrated_data_on_rocksdb() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let runner = MigrationRunner::new(&store);
    runner
        .initialize(MigrationState::new(1, json!({"stable": true})).expect("state"))
        .await
        .expect("initial state persists");
    let package = signed_package();

    assert_eq!(
        runner
            .run_with_validator(
                &package,
                |_m, _asset, data| {
                    data["half"] = json!(true);
                    Ok(())
                },
                |_m, _data| Err(MigrationStepError::Rejected),
            )
            .await
            .unwrap_err()
            .code(),
        MigrationErrorCode::ValidationFailed
    );
    let state = runner.state().await.expect("quarantine persists");
    assert_eq!(state.schema_version(), 1);
    assert_eq!(state.data(), &json!({"stable": true}));

    // Survives reopen: still no half data, still quarantined
    store.close().await.expect("store closes");
    let reopened = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store reopens");
    let runner = MigrationRunner::new(&reopened);
    let after = runner.state().await.expect("state after reopen");
    assert_eq!(after.data(), &json!({"stable": true}));
    assert!(matches!(
        after.lifecycle(),
        MigrationLifecycle::Quarantined { .. }
    ));
    reopened.close().await.expect("store closes");
}
