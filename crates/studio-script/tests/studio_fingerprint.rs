//! US4 fingerprint contract: identical inputs key identically, any drift
//! rekeys, and the key covers every documented toolchain input.

use studio_script::fingerprint::{ToolchainInput, fingerprint_module, studio_toolchain_inputs};
use studio_script::rsvelte_adapter::AdapterEngine;

#[test]
fn fingerprint_covers_every_documented_input() {
    let engine = AdapterEngine::new();
    let fingerprint = engine.fingerprint();
    let inputs = studio_toolchain_inputs(&fingerprint, "0.28.20", "release", "wasm32-wasip1");
    let names: Vec<&str> = inputs.iter().map(|input| input.name.as_str()).collect();
    for required in [
        "studio-ir-version",
        "studio-script-version",
        "protocol-version",
        "rsvelte-facade",
        "rsvelte-compiler",
        "rsvelte-svelte",
        "asc-version",
        "asc-options",
        "target-abi",
    ] {
        assert!(names.contains(&required), "input {required} is covered");
    }
    let keyed = fingerprint_module(b"module", &[], &inputs);
    assert_eq!(keyed.key.len(), 64);
    assert_eq!(
        fingerprint_module(b"module", &[], &inputs),
        keyed,
        "identical inputs key identically"
    );
}

#[test]
fn fingerprint_rekeys_on_any_drift() {
    let engine = AdapterEngine::new();
    let fingerprint = engine.fingerprint();
    let inputs = studio_toolchain_inputs(&fingerprint, "0.28.20", "release", "wasm32-wasip1");
    let base = fingerprint_module(b"module", &[], &inputs);
    assert_ne!(
        fingerprint_module(b"module!", &[], &inputs),
        base,
        "source drift rekeys"
    );
    assert_ne!(
        fingerprint_module(b"module", &[("dep", "other")], &inputs),
        base,
        "dependency drift rekeys"
    );
    let mut drifted = inputs.clone();
    drifted.push(ToolchainInput {
        name: "asc-version".to_owned(),
        version: "0.29.0".to_owned(),
    });
    // Duplicate names sort adjacently; the record still changes.
    assert_ne!(
        fingerprint_module(b"module", &[], &drifted),
        base,
        "toolchain drift rekeys"
    );
}
