#![allow(missing_docs)]

use std::fs;
use std::path::Path;

use studio_script::{format, parse};

fn fixture_files(directory: &str) -> Vec<std::path::PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(directory);
    let mut files: Vec<_> = fs::read_dir(&root)
        .expect("fixture directory should exist")
        .map(|entry| entry.expect("fixture entry should be readable").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "studio"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "fixture directory {} should contain at least one .studio file",
        root.display()
    );
    files
}

#[test]
fn every_valid_fixture_parses_and_formats_idempotently() {
    for path in fixture_files("valid") {
        let source = fs::read_to_string(&path).expect("valid fixture should be UTF-8");
        let canonical = format(&source)
            .unwrap_or_else(|error| panic!("valid fixture {} failed: {error:?}", path.display()));
        assert_eq!(
            canonical,
            format(&canonical).expect("canonical valid fixture should format twice"),
            "{} should be idempotent",
            path.display()
        );
        assert_eq!(
            parse(&canonical).expect("canonical valid fixture should parse"),
            parse(&format(&canonical).expect("canonical fixture should format"))
                .expect("canonical fixture should parse after formatting")
        );
    }
}

#[test]
fn every_invalid_fixture_has_a_stable_diagnostic() {
    for path in fixture_files("invalid") {
        let source = fs::read_to_string(&path).expect("invalid fixture should be UTF-8");
        let error = parse(&source).expect_err("invalid fixture should fail");
        assert!(
            !error.diagnostics.is_empty(),
            "{} had no diagnostics",
            path.display()
        );
        assert!(
            error
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code.starts_with("STUDIO")),
            "{} had an unstable diagnostic code",
            path.display()
        );
    }
}
