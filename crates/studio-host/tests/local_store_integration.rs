#![allow(missing_docs)]
#![allow(
    clippy::doc_markdown,
    clippy::needless_pass_by_value,
    clippy::err_expect
)]

//! Integration coverage for the embedded LocalStore boundary against
//! temporary RocksDB directories: idempotent reopen, durability validation,
//! safe diagnostics for damaged fixtures, and the async executor boundary.

use std::path::Path;

use studio_host::{
    Durability, EmbeddedLocalStore, LocalStore, LocalStoreDiagnosticCode, StoreBatch,
    StoreBatchEntry, StoreExecutor,
};
use tokio::runtime::Builder;

const MANIFEST_FILE: &str = ".studio-localstore-engine.json";

fn sample_batch(id: &str) -> StoreBatch {
    StoreBatch::new(
        id,
        (0..2).map(|ordinal| StoreBatchEntry {
            ordinal,
            payload: serde_json::json!({ "ordinal": ordinal }),
        }),
    )
    .expect("sample batch is valid")
}

async fn opened(directory: &Path) -> EmbeddedLocalStore {
    EmbeddedLocalStore::open(directory, Durability::Every)
        .await
        .expect("store opens")
}

fn assert_code(error: studio_host::LocalStoreError, expected: LocalStoreDiagnosticCode) {
    assert_eq!(error.diagnostic().code(), expected);
    assert!(!error.diagnostic().message().is_empty());
}

#[test]
fn store_initializes_schema_metadata_and_reopens_idempotently() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime starts");

    runtime.block_on(async {
        let store = opened(directory.path()).await;
        assert_eq!(store.directory(), directory.path());
        assert_eq!(store.durability(), Durability::Every);

        let metadata = store.metadata().await.expect("metadata initializes");
        assert_eq!(metadata.schema_version(), 1);
        assert_eq!(metadata.engine_format_version(), 1);

        store
            .write_batch(&sample_batch("reopen"))
            .await
            .expect("batch commits");
        store.close().await.expect("store closes");

        // Idempotent reopen across a simulated process restart.
        let reopened = opened(directory.path()).await;
        let metadata = reopened.metadata().await.expect("metadata re-reads");
        assert_eq!(metadata.schema_version(), 1);
        let entries = reopened
            .batch_entries("reopen")
            .await
            .expect("committed batch re-reads");
        assert_eq!(entries.len(), 2);
        reopened.close().await.expect("reopened store closes");
    });
}

#[test]
fn unknown_batches_read_back_empty() {
    let directory = tempfile::tempdir().expect("temporary directory");
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            let store = opened(directory.path()).await;
            let entries = store
                .batch_entries("never-written")
                .await
                .expect("unknown batch reads safely");
            assert!(entries.is_empty());
            store.close().await.expect("closes");
        });
}

#[test]
fn durability_and_batches_fail_with_stable_safe_codes() {
    let directory = tempfile::tempdir().expect("temporary directory");
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            let error = EmbeddedLocalStore::open(
                directory.path(),
                Durability::Interval(std::time::Duration::from_millis(50)),
            )
            .await
            .err()
            .expect("sub-100 ms interval is rejected");
            assert_code(error, LocalStoreDiagnosticCode::DurabilityInvalid);

            let store = opened(directory.path()).await;
            let empty = StoreBatch::new("empty", std::iter::empty::<StoreBatchEntry>());
            match empty {
                Err(error) => assert_code(error, LocalStoreDiagnosticCode::BatchInvalid),
                Ok(_) => panic!("empty batch must be rejected"),
            }
            let gap = StoreBatch::new(
                "gap",
                [StoreBatchEntry {
                    ordinal: 4,
                    payload: serde_json::json!({}),
                }],
            );
            match gap {
                Err(error) => assert_code(error, LocalStoreDiagnosticCode::BatchInvalid),
                Ok(_) => panic!("ordinal gaps must be rejected"),
            }
            let invalid_id_error = store.batch_entries("").await.err().expect("invalid id");
            assert_code(invalid_id_error, LocalStoreDiagnosticCode::BatchInvalid);
            store.close().await.expect("closes");
        });
}

#[test]
fn recovery_requires_a_fully_initialized_store() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    // A missing directory has nothing to recover.
    let missing = tempfile::tempdir().expect("temporary directory");
    let missing_path = missing.path().join("never-created");
    runtime.block_on(async {
        let error = EmbeddedLocalStore::recover(&missing_path, Durability::Every)
            .await
            .err()
            .expect("missing store cannot recover");
        assert_code(error, LocalStoreDiagnosticCode::RecoveryUnavailable);
    });

    // An existing but uninitialized directory is not a Studio store.
    let uninitialized = tempfile::tempdir().expect("temporary directory");
    runtime.block_on(async {
        let error = EmbeddedLocalStore::recover(uninitialized.path(), Durability::Every)
            .await
            .err()
            .expect("uninitialized store cannot recover");
        assert_code(error, LocalStoreDiagnosticCode::RecoveryUnavailable);
    });

    // After open + close, recovery succeeds.
    let initialized = tempfile::tempdir().expect("temporary directory");
    runtime.block_on(async {
        let store = opened(initialized.path()).await;
        store.close().await.expect("closes");
        EmbeddedLocalStore::recover(initialized.path(), Durability::Every)
            .await
            .expect("initialized store recovers");
    });
}

#[test]
fn corrupted_manifest_fails_safely() {
    let directory = tempfile::tempdir().expect("temporary directory");
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            let store = opened(directory.path()).await;
            store.close().await.expect("closes");
        });

    std::fs::write(directory.path().join(MANIFEST_FILE), "{not json").expect("corrupts manifest");

    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            let open_error = EmbeddedLocalStore::open(directory.path(), Durability::Every)
                .await
                .err()
                .expect("corrupted manifest must fail");
            assert_code(open_error, LocalStoreDiagnosticCode::EngineManifestCorrupt);

            let recover_error = EmbeddedLocalStore::recover(directory.path(), Durability::Every)
                .await
                .err()
                .expect("corrupted manifest must fail recovery");
            assert_code(
                recover_error,
                LocalStoreDiagnosticCode::EngineManifestCorrupt,
            );
        });
}

#[test]
fn incompatible_manifest_fails_safely() {
    let directory = tempfile::tempdir().expect("temporary directory");
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            let store = opened(directory.path()).await;
            store.close().await.expect("closes");
        });

    std::fs::write(
        directory.path().join(MANIFEST_FILE),
        r#"{"engine":"rocksdb","format_version":999}"#,
    )
    .expect("rewrites manifest with future format");

    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(async {
            let error = EmbeddedLocalStore::open(directory.path(), Durability::Every)
                .await
                .err()
                .expect("future format must be rejected");
            assert_code(error, LocalStoreDiagnosticCode::EngineIncompatible);

            let foreign_directory = tempfile::tempdir().expect("temporary directory");
            std::fs::create_dir_all(foreign_directory.path()).expect("directory exists");
            std::fs::write(
                foreign_directory.path().join(MANIFEST_FILE),
                r#"{"engine":"surrealkv","format_version":1}"#,
            )
            .expect("writes foreign engine manifest");
            let foreign_error =
                EmbeddedLocalStore::recover(foreign_directory.path(), Durability::Every)
                    .await
                    .err()
                    .expect("foreign engine must be rejected");
            assert_code(foreign_error, LocalStoreDiagnosticCode::EngineIncompatible);
        });
}

#[test]
fn storage_runs_through_the_off_ui_thread_executor_boundary() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let executor = StoreExecutor::new().expect("executor starts");

    let operation_directory = directory.path().to_path_buf();
    let task = executor.spawn(async move {
        let store = EmbeddedLocalStore::open(operation_directory, Durability::Every).await?;
        store.write_batch(&sample_batch("boundary")).await?;
        let metadata = store.metadata().await?;
        let entries = store.batch_entries("boundary").await?;
        store.close().await?;
        Ok((metadata.schema_version(), entries.len()))
    });

    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let (schema_version, entry_count) = runtime
        .block_on(task.resolve())
        .expect("storage task resolves off-thread");
    assert_eq!(schema_version, 1);
    assert_eq!(entry_count, 2);
    executor.shutdown();
}
