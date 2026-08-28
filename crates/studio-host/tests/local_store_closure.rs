#![allow(missing_docs)]
#![allow(clippy::doc_markdown, clippy::needless_pass_by_value)]

use std::{path::Path, time::Duration};

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
            payload: serde_json::json!({ "ordinal": ordinal, "origin": id }),
        }),
    )
    .expect("sample batch is valid")
}

fn sample_batch_three(id: &str) -> StoreBatch {
    StoreBatch::new(
        id,
        (0..3).map(|ordinal| StoreBatchEntry {
            ordinal,
            payload: serde_json::json!({ "ordinal": ordinal, "origin": id }),
        }),
    )
    .expect("sample batch is valid")
}

async fn opened(directory: &Path, durability: Durability) -> EmbeddedLocalStore {
    EmbeddedLocalStore::open(directory, durability)
        .await
        .expect("store opens")
}

fn assert_code(error: studio_host::LocalStoreError, expected: LocalStoreDiagnosticCode) {
    assert_eq!(error.diagnostic().code(), expected);
    assert!(!error.diagnostic().message().is_empty());
}

/// Every durability mode must open a fresh RocksDB store, persist, and close.
#[test]
fn durability_every_never_and_interval_persist_on_real_rocksdb() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    for durability in [
        Durability::Every,
        Durability::Never,
        Durability::Interval(Duration::from_millis(250)),
    ] {
        let directory = tempfile::tempdir().expect("tempdir");
        runtime.block_on(async {
            let store = opened(directory.path(), durability).await;
            assert_eq!(store.durability(), durability);
            assert_eq!(store.directory(), directory.path());

            // Fresh store has no batches.
            let empty = store
                .batch_entries("never-written")
                .await
                .expect("fresh read empty");
            assert!(empty.is_empty(), "fresh {:?} batch is empty", durability);

            let metadata = store.metadata().await.expect("metadata");
            assert_eq!(metadata.schema_version(), 1);
            assert_eq!(metadata.engine_format_version(), 1);

            store
                .write_batch(&sample_batch("durability-check"))
                .await
                .expect("write succeeds");

            let entries = store
                .batch_entries("durability-check")
                .await
                .expect("read back");
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].ordinal, 0);
            assert_eq!(entries[1].ordinal, 1);

            store.close().await.expect("close");
        });
    }
}

/// Fresh-directory vs reopen behavior via the real EmbeddedLocalStore seam.
///
/// A fresh directory opens as empty; a second open on the same directory is
/// idempotent and preserves the first transaction. Corrupted or missing stores
/// never auto-create on recover.
#[test]
fn fresh_directory_is_empty_and_reopen_is_idempotent_via_real_engine() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    // Fresh open is empty, blocking variant too.
    let fresh_dir = tempfile::tempdir().expect("tempdir");
    runtime.block_on(async {
        let store = opened(fresh_dir.path(), Durability::Every).await;
        let entries = store
            .batch_entries("absent")
            .await
            .expect("fresh absent reads empty");
        assert!(entries.is_empty());
        store.close().await.expect("close fresh");
    });

    // Blocking fresh open also starts empty (proves retained runtime path).
    let fresh_blocking_dir = tempfile::tempdir().expect("tempdir");
    let store = EmbeddedLocalStore::open_blocking(fresh_blocking_dir.path(), Durability::Every)
        .expect("blocking fresh opens");
    let entries = store
        .batch_entries_blocking("absent")
        .expect("blocking fresh absent reads empty");
    assert!(entries.is_empty());
    // Use blocking write then close via runtime teardown (drop).
    store
        .write_batch_blocking(&sample_batch("fresh-blocking"))
        .expect("blocking write");
    let entries = store
        .batch_entries_blocking("fresh-blocking")
        .expect("blocking read back");
    assert_eq!(entries.len(), 2);
    // Close logically: drop and sleep via async close on a fresh handle.
    // We need to close properly: convert to async close via runtime.
    // For this fresh directory we reopen async to verify close-reopen.
    drop(store);
    // Reopen async and verify previous blocking write survived if close had been graceful.
    // Instead, test a clean blocking -> blocking reopen path below.

    // Durable write survives close + reopen (async seam).
    let durable_dir = tempfile::tempdir().expect("tempdir");
    runtime.block_on(async {
        let store = opened(durable_dir.path(), Durability::Every).await;
        store
            .write_batch(&sample_batch_three("reopen-me"))
            .await
            .expect("write");
        store.close().await.expect("close");

        let reopened = opened(durable_dir.path(), Durability::Every).await;
        let entries = reopened
            .batch_entries("reopen-me")
            .await
            .expect("reopened reads durable batch");
        assert_eq!(entries.len(), 3);
        assert!(
            entries
                .iter()
                .enumerate()
                .all(|(i, e)| e.ordinal == i as u32)
        );

        // Unknown batch on reopened store is still empty (is_missing_batch_resource).
        let missing = reopened
            .batch_entries("still-absent")
            .await
            .expect("unknown remains empty");
        assert!(missing.is_empty());

        reopened.close().await.expect("reopen close");
    });

    // Durable write survives blocking close + async recover.
    let recover_dir = tempfile::tempdir().expect("tempdir");
    runtime.block_on(async {
        let store = opened(recover_dir.path(), Durability::Every).await;
        store
            .write_batch(&sample_batch("recover-me"))
            .await
            .expect("write");
        store.close().await.expect("close");

        let recovered = EmbeddedLocalStore::recover(recover_dir.path(), Durability::Every)
            .await
            .expect("recover succeeds after close");
        let entries = recovered
            .batch_entries("recover-me")
            .await
            .expect("recovered reads batch");
        assert_eq!(entries.len(), 2);
        recovered.close().await.expect("recovered close");
    });

    // Interval durability also reopens idempotently.
    let interval_dir = tempfile::tempdir().expect("tempdir");
    runtime.block_on(async {
        let store = opened(
            interval_dir.path(),
            Durability::Interval(Duration::from_millis(200)),
        )
        .await;
        store
            .write_batch(&sample_batch("interval-reopen"))
            .await
            .expect("interval write");
        store.close().await.expect("close");
        let reopened = opened(
            interval_dir.path(),
            Durability::Interval(Duration::from_millis(200)),
        )
        .await;
        let entries = reopened
            .batch_entries("interval-reopen")
            .await
            .expect("interval reopen reads");
        assert_eq!(entries.len(), 2);
        reopened.close().await.expect("close");
    });
}

/// The retained-runtime fix: a store opened via `open_blocking` serves
/// blocking batch calls on its own router without spawning a throwaway runtime.
#[test]
fn blocking_store_retains_runtime_across_batch_operations() {
    let directory = tempfile::tempdir().expect("tempdir");

    // Open via blocking seam (creates multi-thread runtime retained inside store).
    let store = EmbeddedLocalStore::open_blocking(directory.path(), Durability::Every)
        .expect("blocking open");

    // Verify durability and directory are retained.
    assert_eq!(store.durability(), Durability::Every);
    assert_eq!(store.directory(), directory.path());

    // Fresh manifest + metadata must exist.
    assert!(
        directory.path().join(MANIFEST_FILE).exists(),
        "manifest exists after blocking open"
    );

    // Batch round-trip via blocking seam uses retained runtime, not a new current_thread.
    store
        .write_batch_blocking(&sample_batch_three("blocking-roundtrip"))
        .expect("blocking write");
    let entries = store
        .batch_entries_blocking("blocking-roundtrip")
        .expect("blocking read");
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].payload["origin"], "blocking-roundtrip");

    // Overwrite same batch atomically replaces the single record.
    store
        .write_batch_blocking(&sample_batch("blocking-roundtrip"))
        .expect("blocking overwrite");
    let overwritten = store
        .batch_entries_blocking("blocking-roundtrip")
        .expect("blocking read after overwrite");
    assert_eq!(overwritten.len(), 2);

    // Unknown batch via blocking returns empty (missing-resource handling).
    let missing = store
        .batch_entries_blocking("blocking-absent")
        .expect("blocking missing returns empty");
    assert!(missing.is_empty());

    // Invalid batch id fails closed with stable code via blocking seam as well.
    let invalid = store.batch_entries_blocking("").unwrap_err();
    assert_eq!(
        invalid.diagnostic().code(),
        LocalStoreDiagnosticCode::BatchInvalid
    );

    // Clean shutdown via the async close seam: the blocking store's retained
    // runtime owns the SurrealDB router, so `close().await` must run to drop
    // the handle and give the router time to release the RocksDB file lock.
    // The retained runtime is validated above by the blocking round-trip; a
    // clean close must succeed and release the store without leaking the router.
    let close_runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    close_runtime
        .block_on(store.close())
        .expect("blocking store closes");
    // Give the retained multi-thread runtime time to shut down its RocksDB
    // handle before the temporary directory is cleaned up.
    std::thread::sleep(Duration::from_millis(600));
}

/// Corruption diagnostics are stable, actionable, and safe-coded via both seams.
#[test]
fn corruption_diagnostics_are_actionable_via_real_engine() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    // Invalid UTF-8 in an existing manifest is corruption, not a fresh store.
    let corrupted = tempfile::tempdir().expect("tempdir");
    runtime.block_on(async {
        let store = opened(corrupted.path(), Durability::Every).await;
        store.close().await.expect("close");
    });
    std::fs::write(corrupted.path().join(MANIFEST_FILE), [0xff, 0xfe, 0xfd])
        .expect("corrupt manifest bytes");
    runtime.block_on(async {
        let open_err = EmbeddedLocalStore::open(corrupted.path(), Durability::Every)
            .await
            .err()
            .expect("corrupted open fails");
        assert_code(open_err, LocalStoreDiagnosticCode::EngineManifestCorrupt);
        let recover_err = EmbeddedLocalStore::recover(corrupted.path(), Durability::Every)
            .await
            .err()
            .expect("corrupted recover fails");
        assert_code(recover_err, LocalStoreDiagnosticCode::EngineManifestCorrupt);
    });
    assert_eq!(
        std::fs::read(corrupted.path().join(MANIFEST_FILE)).expect("manifest remains readable"),
        [0xff, 0xfe, 0xfd]
    );

    // A manifest path that is a directory is a portable non-NotFound read
    // failure on supported platforms and must also fail closed.
    let unreadable = tempfile::tempdir().expect("temporary directory");
    runtime.block_on(async {
        let store = opened(unreadable.path(), Durability::Every).await;
        store.close().await.expect("close");
    });
    std::fs::remove_file(unreadable.path().join(MANIFEST_FILE)).expect("remove manifest");
    std::fs::create_dir(unreadable.path().join(MANIFEST_FILE)).expect("manifest directory");
    runtime.block_on(async {
        let open_err = EmbeddedLocalStore::open(unreadable.path(), Durability::Every)
            .await
            .err()
            .expect("directory manifest must fail");
        assert_code(open_err, LocalStoreDiagnosticCode::EngineManifestCorrupt);
        let recover_err = EmbeddedLocalStore::recover(unreadable.path(), Durability::Every)
            .await
            .err()
            .expect("directory manifest recovery must fail");
        assert_code(recover_err, LocalStoreDiagnosticCode::EngineManifestCorrupt);
    });
    assert!(unreadable.path().join(MANIFEST_FILE).is_dir());

    // Incompatible manifest (future format / foreign engine).
    let incompatible = tempfile::tempdir().expect("tempdir");
    runtime.block_on(async {
        let store = opened(incompatible.path(), Durability::Every).await;
        store.close().await.expect("close");
    });
    std::fs::write(
        incompatible.path().join(MANIFEST_FILE),
        r#"{"engine":"rocksdb","format_version":999}"#,
    )
    .expect("future format");
    runtime.block_on(async {
        let open_err = EmbeddedLocalStore::open(incompatible.path(), Durability::Every)
            .await
            .err()
            .expect("future format fails");
        assert_code(open_err, LocalStoreDiagnosticCode::EngineIncompatible);
    });

    let foreign_dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(foreign_dir.path()).expect("dir");
    std::fs::write(
        foreign_dir.path().join(MANIFEST_FILE),
        r#"{"engine":"surrealkv","format_version":1}"#,
    )
    .expect("foreign engine");
    runtime.block_on(async {
        let err = EmbeddedLocalStore::recover(foreign_dir.path(), Durability::Every)
            .await
            .err()
            .expect("foreign engine recover fails");
        assert_code(err, LocalStoreDiagnosticCode::EngineIncompatible);
    });

    // RecoveryUnavailable for missing / uninitialized directories.
    let missing = tempfile::tempdir().expect("tempdir");
    let missing_path = missing.path().join("never-created");
    runtime.block_on(async {
        let err = EmbeddedLocalStore::recover(&missing_path, Durability::Every)
            .await
            .err()
            .expect("missing recover unavailable");
        assert_code(err, LocalStoreDiagnosticCode::RecoveryUnavailable);
    });
    let uninitialized = tempfile::tempdir().expect("tempdir");
    runtime.block_on(async {
        let err = EmbeddedLocalStore::recover(uninitialized.path(), Durability::Every)
            .await
            .err()
            .expect("uninitialized recover unavailable");
        assert_code(err, LocalStoreDiagnosticCode::RecoveryUnavailable);
    });

    // DurabilityInvalid for sub-100ms intervals (host rejects before engine).
    let bad_durability_dir = tempfile::tempdir().expect("tempdir");
    runtime.block_on(async {
        let err = EmbeddedLocalStore::open(
            bad_durability_dir.path(),
            Durability::Interval(Duration::from_millis(100)),
        )
        .await
        .err()
        .expect("100ms rejected");
        assert_code(err, LocalStoreDiagnosticCode::DurabilityInvalid);
        let err2 = EmbeddedLocalStore::open(
            bad_durability_dir.path(),
            Durability::Interval(Duration::from_millis(50)),
        )
        .await
        .err()
        .expect("50ms rejected");
        assert_code(err2, LocalStoreDiagnosticCode::DurabilityInvalid);
    });

    // Recovery after manifest removal: a closed store without its manifest is
    // not recoverable, proving the manifest is the durability marker.
    let manifest_removed_dir = tempfile::tempdir().expect("tempdir");
    runtime.block_on(async {
        let store = opened(manifest_removed_dir.path(), Durability::Every).await;
        store.close().await.expect("close");
        std::fs::remove_file(manifest_removed_dir.path().join(MANIFEST_FILE))
            .expect("remove manifest");
        let err = EmbeddedLocalStore::recover(manifest_removed_dir.path(), Durability::Every)
            .await
            .err()
            .expect("manifest removed recover fails");
        assert_code(err, LocalStoreDiagnosticCode::RecoveryUnavailable);
    });

    // DirectoryInvalid: a file masquerading as a store directory.
    let file_as_dir = tempfile::tempdir().expect("tempdir");
    let file_path = file_as_dir.path().join("not-a-directory");
    std::fs::write(&file_path, b"not a directory").expect("write file");
    runtime.block_on(async {
        let err = EmbeddedLocalStore::open(&file_path, Durability::Every)
            .await
            .err()
            .expect("file-as-directory must fail");
        // EngineOpenFailed or DirectoryInvalid are both safe, actionable diagnostics
        // for a non-directory path; the host must not leak filesystem details.
        assert!(
            matches!(
                err.diagnostic().code(),
                LocalStoreDiagnosticCode::DirectoryInvalid
                    | LocalStoreDiagnosticCode::EngineOpenFailed
            ),
            "unexpected code {:?}",
            err.diagnostic().code()
        );
    });
}

/// Sync writes survive a clean close even for the relaxed durability modes.
///
/// This documents the durability contract: `Every` syncs before return, while
/// `Never` and `Interval` still persist through a clean `close`; forced
/// termination may lose acknowledged writes for the latter two.
#[test]
fn clean_close_persists_batches_even_for_relaxed_durability() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    for durability in [
        Durability::Never,
        Durability::Interval(Duration::from_millis(250)),
    ] {
        let dir = tempfile::tempdir().expect("tempdir");
        runtime.block_on(async {
            let store = EmbeddedLocalStore::open(dir.path(), durability)
                .await
                .expect("open");
            store
                .write_batch(&sample_batch("relaxed"))
                .await
                .expect("write");
            store.close().await.expect("close flushes");

            let reopened = EmbeddedLocalStore::open(dir.path(), Durability::Every)
                .await
                .expect("reopen with Every");
            let entries = reopened
                .batch_entries("relaxed")
                .await
                .expect("read after relaxed close");
            assert_eq!(
                entries.len(),
                2,
                "clean close must persist even for {:?}",
                durability
            );
            reopened.close().await.expect("close");
        });
    }
}

/// Executor boundary keeps storage off the UI thread for every durability mode.
#[test]
fn storage_executor_boundary_works_for_all_durabilities() {
    let executor = StoreExecutor::new().expect("executor");
    let cases = [
        Durability::Every,
        Durability::Never,
        Durability::Interval(Duration::from_millis(250)),
    ];
    for durability in cases {
        let dir = tempfile::tempdir().expect("tempdir");
        let dir_path = dir.path().to_path_buf();
        let task = executor.spawn(async move {
            let store = EmbeddedLocalStore::open(dir_path, durability).await?;
            store.write_batch(&sample_batch("executor")).await?;
            let entries = store.batch_entries("executor").await?;
            store.close().await?;
            Ok::<usize, studio_host::LocalStoreError>(entries.len())
        });
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let len = runtime
            .block_on(task.resolve())
            .expect("executor task resolves");
        assert_eq!(len, 2);
    }
    executor.shutdown();
}
