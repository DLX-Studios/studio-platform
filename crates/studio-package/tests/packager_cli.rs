#![allow(missing_docs)]

use std::{
    fs,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use studio_package::{ArchivePolicy, inspect_archive};

fn fixture() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("studio-pack-test-{}-{nonce}", std::process::id()));
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::write(root.join("module.wasm"), b"\0asm\x01\0\0\0").unwrap();
    fs::write(root.join("assets/catalog.json"), br#"{"items":[]}"#).unwrap();
    fs::write(root.join("signing.key"), [7_u8; 32]).unwrap();
    fs::write(
        root.join("manifest.json"),
        br#"{
      "schemaVersion":1,"id":"com.example.pack","name":"Pack Test","version":"0.1.0",
      "publisher":{"id":"example","keyId":"key-1"},"entry":"module.wasm",
      "sdkVersion":"^0.1.0","protocolVersion":1,"capabilities":[],
      "limits":{"memoryMiB":16,"eventFuel":1000000},"assets":["assets/catalog.json"]
    }"#,
    )
    .unwrap();
    root
}

fn run(root: &std::path::Path, output: &str, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_studio-pack"))
        .args([
            "--manifest",
            "manifest.json",
            "--module",
            "module.wasm",
            "--output",
            output,
        ])
        .args(extra)
        .current_dir(root)
        .output()
        .unwrap()
}

#[test]
fn signed_cli_is_byte_deterministic_rfc8785_and_raw_signature_compatible() {
    let root = fixture();
    for output in ["first.studio", "second.studio"] {
        let result = run(&root, output, &["--signing-key", "signing.key"]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let first = fs::read(root.join("first.studio")).unwrap();
    assert_eq!(first, fs::read(root.join("second.studio")).unwrap());
    let archive = inspect_archive(&first, ArchivePolicy::default()).unwrap();
    assert_eq!(archive.signature.len(), 64);
    assert_eq!(
        archive.manifest,
        studio_package::canonicalize_json(&serde_json::from_slice(&archive.manifest).unwrap(),)
            .unwrap()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unsigned_output_requires_explicit_development_mode_and_diagnostics_are_safe() {
    let root = fixture();
    let missing_mode = run(&root, "missing.studio", &[]);
    assert!(!missing_mode.status.success());
    let dev = run(&root, "dev.studio", &["--dev"]);
    assert!(dev.status.success());
    let archive = inspect_archive(
        &fs::read(root.join("dev.studio")).unwrap(),
        ArchivePolicy::default(),
    )
    .unwrap();
    assert_eq!(archive.signature, vec![0; 64]);
    assert!(String::from_utf8_lossy(&dev.stderr).contains("unsigned development bundle"));

    fs::write(root.join("signing.key"), b"super-secret-but-wrong").unwrap();
    let bad = run(&root, "bad.studio", &["--signing-key", "signing.key"]);
    let diagnostic = String::from_utf8_lossy(&bad.stderr);
    assert!(!bad.status.success());
    assert!(!diagnostic.contains("super-secret"));
    fs::remove_dir_all(root).unwrap();
}
