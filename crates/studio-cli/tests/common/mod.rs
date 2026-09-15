//! Shared helpers for studio-cli integration tests.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The compiled `studio` binary under test.
#[must_use]
pub fn studio_binary() -> &'static str {
    env!("CARGO_BIN_EXE_studio")
}

/// Run the studio binary with arguments, capturing output.
pub fn run_studio(args: &[&str]) -> Output {
    Command::new(studio_binary())
        .args(args)
        .output()
        .expect("spawn studio binary")
}

/// Copy a minimal starter-style project into a temporary directory.
///
/// The starter project is self-contained: one AssemblyScript entry, a closed
/// manifest, and an asconfig that writes `build/<name>.wasm`.
pub fn seed_project(root: &Path, name: &str) -> PathBuf {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let starter = repo.join("examples/starter");
    let project = root.join(name);
    copy_dir(&starter.join("assembly"), &project.join("assembly"));
    std::fs::create_dir_all(&project).expect("create project root");
    std::fs::copy(starter.join("manifest.json"), project.join("manifest.json"))
        .expect("copy manifest");
    std::fs::copy(starter.join("asconfig.json"), project.join("asconfig.json"))
        .expect("copy asconfig");
    project
}

fn copy_dir(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("create destination dir");
    for entry in walkdir::WalkDir::new(source) {
        let entry = entry.expect("walk source");
        let relative = entry.path().strip_prefix(source).expect("strip prefix");
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).expect("create dir");
        } else {
            std::fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

/// Extract every JSON object line from output that carries the given field
/// value, parsed as serde_json values.
pub fn json_lines_with(output: &[u8], field: &str, value: &str) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|record| record[field].as_str() == Some(value))
        .collect()
}
