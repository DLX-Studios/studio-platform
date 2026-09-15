//! Template bundle contract: deterministic builds, structural inspection,
//! and fail-closed layout violations.

#![allow(missing_docs)]

use std::collections::BTreeMap;

use studio_package::{ArchivePolicy, TemplateFiles, inspect_template, pack_template};

fn files() -> TemplateFiles {
    TemplateFiles {
        template_json: br#"{"id":"blank","name":"Blank"}"#.to_vec(),
        manifest: br#"{"schemaVersion":1}"#.to_vec(),
        entry_source: b"<Card id=\"a\" />".to_vec(),
        assets: BTreeMap::from([("assets/icon.svg".to_owned(), b"<svg />".to_vec())]),
    }
}

#[test]
fn round_trip_preserves_every_file() {
    let policy = ArchivePolicy::default();
    let bytes = pack_template(&files(), policy).expect("packs");
    let inspected = inspect_template(&bytes, policy).expect("inspects");
    assert_eq!(inspected.template_json, files().template_json);
    assert_eq!(inspected.manifest, files().manifest);
    assert_eq!(inspected.entry_source, files().entry_source);
    assert_eq!(inspected.assets, files().assets);
}

#[test]
fn identical_trees_pack_byte_identically() {
    let policy = ArchivePolicy::default();
    let first = pack_template(&files(), policy).expect("packs");
    let second = pack_template(&files(), policy).expect("packs again");
    assert_eq!(first, second);
}

#[test]
fn foreign_layouts_fail_closed() {
    let policy = ArchivePolicy::default();
    // Missing template identity.
    let mut missing_id = files();
    missing_id.template_json = br#"{"name":"No Id"}"#.to_vec();
    assert!(pack_template(&missing_id, policy).is_err());

    // Compiled modules never ride in template bundles.
    let mut with_wasm = files();
    with_wasm
        .assets
        .insert("module.wasm".to_owned(), b"wasm".to_vec());
    assert!(pack_template(&with_wasm, policy).is_err());

    // Build outputs never ride in template bundles.
    let mut with_build = files();
    with_build
        .assets
        .insert("build/app.studio".to_owned(), b"stale".to_vec());
    assert!(pack_template(&with_build, policy).is_err());
}
