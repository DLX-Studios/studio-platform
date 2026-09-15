//! US3 integration contract: the documented TypeScript entry and the native
//! command produce identical bundles and identical exit codes.

mod common;

use std::process::Command;
use tempfile::TempDir;

fn run_shim(project: &std::path::Path) -> std::process::Output {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Command::new("bun")
        .arg(repo.join("scripts/build-example.ts"))
        .arg(project)
        .output()
        .expect("spawn bun shim")
}

#[test]
fn shim_and_native_command_produce_identical_bundles() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "starter");
    let bundle = project.join("build/starter.studio");

    let native = common::run_studio(&["build", project.to_str().unwrap()]);
    assert_eq!(native.status.code(), Some(0));
    let native_bytes = std::fs::read(&bundle).unwrap();

    std::fs::remove_file(&bundle).unwrap();
    let shim = run_shim(&project);
    assert_eq!(
        shim.status.code(),
        Some(0),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&shim.stdout),
        String::from_utf8_lossy(&shim.stderr)
    );
    let shim_bytes = std::fs::read(&bundle).unwrap();

    assert_eq!(
        native_bytes, shim_bytes,
        "shim delegates to the same packager"
    );
}

#[test]
fn shim_propagates_usage_failures_verb() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "starter");

    let native = common::run_studio(&["build", "definitely-not-an-example"]);
    let shim = run_shim(root.path().join("definitely-not-an-example").as_path());

    assert_eq!(native.status.code(), Some(2));
    assert_eq!(
        shim.status.code(),
        Some(2),
        "shim propagates the native exit code verbatim"
    );
    let _ = project;
}
