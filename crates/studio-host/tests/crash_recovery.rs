#![allow(missing_docs)]

//! Deterministic forced-termination harness.
//!
//! A child process commits one durable batch, then pauses inside an open,
//! uncommitted client transaction. The parent force-kills it at the marker and
//! proves recovery to the last durable transaction with no partial batch.

use std::{
    process::Command,
    thread,
    time::{Duration, Instant},
};

use studio_host::{Durability, EmbeddedLocalStore, LocalStore};
use tokio::runtime::Builder;

const PAUSE_MARKER_FILE: &str = ".studio-localstore-test-paused";
const DURABLE_BATCH_ID: &str = "durable-before-termination";
const INTERRUPTED_BATCH_ID: &str = "forced-termination";

#[test]
fn forced_termination_recovers_the_last_durable_transaction_without_partial_batch() {
    let directory = tempfile::tempdir().expect("temporary RocksDB directory");
    let marker = directory.path().join(PAUSE_MARKER_FILE);
    let mut child = Command::new(env!("CARGO_BIN_EXE_localstore-crash-worker"))
        .arg(directory.path())
        .spawn()
        .expect("crash worker starts");

    let deadline = Instant::now() + Duration::from_secs(20);
    while !marker.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        marker.exists(),
        "worker must pause inside the uncommitted database transaction"
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
            .expect("RocksDB recovers after forced termination");
        recovered
            .metadata()
            .await
            .expect("schema metadata survives forced termination");

        let durable = recovered
            .batch_entries(DURABLE_BATCH_ID)
            .await
            .expect("last durable batch is readable");
        assert_eq!(durable.len(), 3, "the committed batch survives verbatim");
        assert!(
            durable
                .iter()
                .enumerate()
                .all(|(index, entry)| u32::try_from(index) == Ok(entry.ordinal)),
            "committed entries keep their host-defined order"
        );

        let interrupted = recovered
            .batch_entries(INTERRUPTED_BATCH_ID)
            .await
            .expect("interrupted batch is queryable");
        assert!(
            interrupted.is_empty(),
            "the uncommitted transaction must expose no partial batch"
        );

        recovered.close().await.expect("recovered store closes");
    });
}
