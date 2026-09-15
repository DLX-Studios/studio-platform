//! US1–US3 icon contract: typed references only, deterministic staged
//! output, overrides, budgets, and provenance-ready behavior.

use std::collections::BTreeMap;
use std::path::PathBuf;
use studio_cli::assets::{
    collect_references, resolve_collection, stage_collection, ICON_BUDGET_BYTES,
};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/icons")
}

/// A fake lucide package with exactly the icons the matrix needs.
fn fake_package(root: &std::path::Path) -> PathBuf {
    let package = root.join("lucide-pkg/icons");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join("store.svg"), b"<svg>store</svg>").unwrap();
    std::fs::write(package.join("star.svg"), b"<svg>star</svg>").unwrap();
    package
}

#[test]
fn references_are_typed_not_textual() {
    // Copy the fixture assembly to a temp project without the override and
    // without the missing icon, so resolution succeeds for shape assertions.
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("icons");
    copy_dir(&fixture(), &project);
    std::fs::remove_file(project.join("assets/icons/store.svg")).unwrap();
    let source = std::fs::read_to_string(project.join("assembly/gallery.ts")).unwrap();
    let cleaned = source
        .lines()
        .filter(|line| !line.contains("no-such-icon-xyz"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(project.join("assembly/gallery.ts"), cleaned).unwrap();

    let references = collect_references(&project);
    let names: Vec<&str> = references.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        vec!["star", "store"],
        "exactly the called icons: {names:?}"
    );
    assert_eq!(references["store"].spans.len(), 1);
    let span = &references["store"].spans[0];
    assert!(span.line > 0 && span.column > 0);
    assert!(span.file.ends_with("assembly/gallery.ts"));
}

#[test]
fn missing_icons_fail_with_spans() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("icons");
    copy_dir(&fixture(), &project);
    let package = fake_package(root.path());

    let references = collect_references(&project);
    assert!(references.contains_key("no-such-icon-xyz"));
    let failure = resolve_collection(&project, &references, &package).unwrap_err();
    assert_eq!(failure.code, "BUILD_ASSET_ICON_MISSING");
    assert!(failure.message.contains("no-such-icon-xyz"));
    assert_eq!(failure.spans.len(), 1);
}

#[test]
fn overrides_win_silently_and_staging_is_content_stable() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("icons");
    copy_dir(&fixture(), &project);
    // Drop the missing icon so resolution succeeds.
    let source = std::fs::read_to_string(project.join("assembly/gallery.ts")).unwrap();
    let cleaned = source
        .lines()
        .filter(|line| !line.contains("no-such-icon-xyz"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(project.join("assembly/gallery.ts"), cleaned).unwrap();
    let package = fake_package(root.path());

    let references = collect_references(&project);
    let collection = resolve_collection(&project, &references, &package).unwrap();
    assert!(collection.entries["store"].overridden);
    assert_eq!(
        collection.entries["store"].bytes,
        b"<svg>override-store</svg>"
    );
    assert!(!collection.entries["star"].overridden);

    let first = stage_collection(&project, &collection).unwrap();
    let staged = first["assets/icons/store.svg"].clone();
    let before = std::fs::metadata(&staged).unwrap().modified().unwrap();
    let second = stage_collection(&project, &collection).unwrap();
    assert_eq!(first, second, "staging is content-stable");
    assert_eq!(
        std::fs::metadata(&staged).unwrap().modified().unwrap(),
        before,
        "unchanged bytes are not rewritten"
    );
}

#[test]
fn budget_overruns_fail_with_the_overage() {
    let root = tempfile::tempdir().unwrap();
    let package = root.path().join("lucide-pkg/icons");
    std::fs::create_dir_all(&package).unwrap();
    let big = vec![b'x'; (ICON_BUDGET_BYTES + 1) as usize];
    std::fs::write(package.join("huge.svg"), &big).unwrap();

    let references = BTreeMap::from([(
        "huge".to_owned(),
        studio_cli::assets::IconReference {
            name: "huge".to_owned(),
            spans: Vec::new(),
        },
    )]);
    let failure = resolve_collection(root.path(), &references, &package).unwrap_err();
    assert_eq!(failure.code, "BUILD_ASSET_BUDGET_EXCEEDED");
    assert!(failure.message.contains(&ICON_BUDGET_BYTES.to_string()));
}

#[test]
fn missing_package_fails_instead_of_silently_skipping() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("icons");
    copy_dir(&fixture(), &project);
    let references = collect_references(&project);
    assert!(!references.is_empty());
    let failure = resolve_collection(&project, &references, &root.path().join("no-such-package"))
        .unwrap_err();
    assert_eq!(failure.code, "BUILD_ASSET_PACKAGE_MISSING");
}

#[test]
#[ignore]
fn real_package_resolves_byte_identical_svgs() {
    let root = tempfile::tempdir().unwrap();
    let project = root.path().join("icons");
    copy_dir(&fixture(), &project);
    let source = std::fs::read_to_string(project.join("assembly/gallery.ts")).unwrap();
    let cleaned = source
        .lines()
        .filter(|line| !line.contains("no-such-icon-xyz"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(project.join("assembly/gallery.ts"), cleaned).unwrap();
    // Also drop the override so the package bytes are compared directly.
    std::fs::remove_file(project.join("assets/icons/store.svg")).unwrap();

    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let package = repo.join("node_modules/lucide-static/icons");
    assert!(package.is_dir(), "pinned package must be installed");

    let references = collect_references(&project);
    let collection = resolve_collection(&project, &references, &package).unwrap();
    for (name, icon) in &collection.entries {
        let expected = std::fs::read(package.join(format!("{name}.svg"))).unwrap();
        assert_eq!(icon.bytes, expected, "byte-identical {name}");
        assert!(!icon.overridden);
    }
}

fn copy_dir(source: &std::path::Path, destination: &std::path::Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in walkdir::WalkDir::new(source) {
        let entry = entry.unwrap();
        let relative = entry.path().strip_prefix(source).unwrap();
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}
