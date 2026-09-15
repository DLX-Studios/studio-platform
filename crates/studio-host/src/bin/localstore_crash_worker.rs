//! Child process used only by the `LocalStore` forced-termination integration test.
//!
//! Sequence: open the store with `Durability::Every`, commit a durable batch,
//! then enter an uncommitted transaction with a second batch, signal the pause
//! marker, and park until the parent force-kills this process. Recovery must
//! show the durable batch and no trace of the interrupted one.

use std::{env, path::PathBuf};

use serde_json::json;
use studio_host::{Durability, EmbeddedLocalStore, LocalStore, StoreBatch, StoreBatchEntry};

const PAUSE_MARKER_FILE: &str = ".studio-localstore-test-paused";
const DURABLE_BATCH_ID: &str = "durable-before-termination";

fn sample_entries(prefix: &'static str) -> impl IntoIterator<Item = StoreBatchEntry> {
    (0..3).map(move |ordinal| StoreBatchEntry {
        ordinal,
        payload: json!({ "ordinal": ordinal, "source": prefix }),
    })
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
        let durable = StoreBatch::new(DURABLE_BATCH_ID, sample_entries("durable"))
            .expect("worker batch is valid");
        store
            .write_batch(&durable)
            .await
            .expect("worker commits durable batch");

        let interrupted = StoreBatch::new("forced-termination", sample_entries("interrupted"))
            .expect("worker batch is valid");
        store
            .test_pause_inside_uncommitted_transaction(
                &interrupted,
                &directory.join(PAUSE_MARKER_FILE),
            )
            .await;
        unreachable!("worker is force-terminated while paused inside the transaction");
    });
}
