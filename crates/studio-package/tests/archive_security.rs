#![allow(missing_docs)]

use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
};

use studio_package::{
    ArchiveErrorCode, ArchiveFiles, ArchivePolicy, build_archive, inspect_archive,
};
use zip::{CompressionMethod, DateTime, ZipWriter, write::SimpleFileOptions};

const MANIFEST: &[u8] = b"{}";
const MODULE: &[u8] = b"\0asm\x01\0\0\0";
const SIGNATURE: &[u8] = &[0; 64];

fn raw_archive(
    entries: &[(&str, &[u8])],
    options: SimpleFileOptions,
    comment: Option<&str>,
) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    if let Some(comment) = comment {
        writer.set_comment(comment).unwrap();
    }
    for (name, bytes) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn stored_options() -> SimpleFileOptions {
    SimpleFileOptions::DEFAULT.compression_method(CompressionMethod::Stored)
}

fn duplicate_named_archive() -> Vec<u8> {
    let mut archive = raw_archive(
        &[("assets/Item.json", b"x"), ("assets/item.json", b"x")],
        stored_options(),
        None,
    );
    for offset in 0..=archive.len() - b"assets/Item.json".len() {
        if &archive[offset..offset + b"assets/Item.json".len()] == b"assets/Item.json" {
            archive[offset..offset + b"assets/item.json".len()]
                .copy_from_slice(b"assets/item.json");
        }
    }
    archive
}

fn required_entries() -> [(&'static str, &'static [u8]); 3] {
    [
        ("manifest.json", MANIFEST),
        ("module.wasm", MODULE),
        ("signature.ed25519", SIGNATURE),
    ]
}

#[test]
fn rejects_traversal_absolute_backslash_empty_and_case_colliding_paths() {
    for path in [
        "../module.wasm",
        "/module.wasm",
        "assets\\item",
        "assets//item",
    ] {
        let archive = raw_archive(&[(path, b"x")], stored_options(), None);
        assert_eq!(
            inspect_archive(&archive, ArchivePolicy::default())
                .unwrap_err()
                .code(),
            ArchiveErrorCode::PathInvalid
        );
    }

    assert_eq!(
        inspect_archive(&duplicate_named_archive(), ArchivePolicy::default())
            .unwrap_err()
            .code(),
        ArchiveErrorCode::DuplicatePath
    );
    let case_collision = raw_archive(
        &[
            ("assets/Item.json", b"x".as_slice()),
            ("assets/item.json", b"x".as_slice()),
        ],
        stored_options(),
        None,
    );
    assert_eq!(
        inspect_archive(&case_collision, ArchivePolicy::default())
            .unwrap_err()
            .code(),
        ArchiveErrorCode::DuplicatePath
    );
}

#[test]
fn rejects_links_compression_nonfixed_metadata_extras_and_comments() {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .add_symlink("module.wasm", "elsewhere", stored_options())
        .unwrap();
    let linked = writer.finish().unwrap().into_inner();
    assert_eq!(
        inspect_archive(&linked, ArchivePolicy::default())
            .unwrap_err()
            .code(),
        ArchiveErrorCode::EntryTypeInvalid
    );

    let compressed = raw_archive(
        &required_entries(),
        stored_options().compression_method(CompressionMethod::Deflated),
        None,
    );
    assert_eq!(
        inspect_archive(&compressed, ArchivePolicy::default())
            .unwrap_err()
            .code(),
        ArchiveErrorCode::CompressionInvalid
    );

    let timestamp = DateTime::from_date_and_time(2026, 1, 2, 3, 4, 6).unwrap();
    for options in [
        stored_options().last_modified_time(timestamp),
        stored_options().unix_permissions(0o600),
    ] {
        let archive = raw_archive(&required_entries(), options, None);
        assert_eq!(
            inspect_archive(&archive, ArchivePolicy::default())
                .unwrap_err()
                .code(),
            ArchiveErrorCode::MetadataInvalid
        );
    }

    let commented = raw_archive(&required_entries(), stored_options(), Some("comment"));
    assert_eq!(
        inspect_archive(&commented, ArchivePolicy::default())
            .unwrap_err()
            .code(),
        ArchiveErrorCode::MetadataInvalid
    );

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let mut options = stored_options().into_full_options();
    options.add_extra_data(0xcafe, [1, 2, 3], false).unwrap();
    writer.start_file("manifest.json", options).unwrap();
    writer.write_all(MANIFEST).unwrap();
    let extra = writer.finish().unwrap().into_inner();
    assert_eq!(
        inspect_archive(&extra, ArchivePolicy::default())
            .unwrap_err()
            .code(),
        ArchiveErrorCode::MetadataInvalid
    );
}

#[test]
fn rejects_nonlexicographic_layout_and_all_input_entry_and_aggregate_limits() {
    let unordered = raw_archive(
        &[
            ("module.wasm", MODULE),
            ("manifest.json", MANIFEST),
            ("signature.ed25519", SIGNATURE),
        ],
        stored_options(),
        None,
    );
    assert_eq!(
        inspect_archive(&unordered, ArchivePolicy::default())
            .unwrap_err()
            .code(),
        ArchiveErrorCode::OrderInvalid
    );

    let tiny = ArchivePolicy {
        max_archive_bytes: 128,
        max_module_bytes: 4,
        max_asset_bytes: 4,
        max_entries: 3,
    };
    let archive = raw_archive(&required_entries(), stored_options(), None);
    assert_eq!(
        inspect_archive(&archive, tiny).unwrap_err().code(),
        ArchiveErrorCode::SizeLimit
    );

    let too_many = raw_archive(
        &[
            ("assets/a", b"a"),
            ("assets/b", b"b"),
            ("manifest.json", MANIFEST),
            ("module.wasm", MODULE),
            ("signature.ed25519", SIGNATURE),
        ],
        stored_options(),
        None,
    );
    assert_eq!(
        inspect_archive(&too_many, tiny).unwrap_err().code(),
        ArchiveErrorCode::SizeLimit
    );
}

#[test]
fn deterministic_builder_is_byte_reproducible_and_inspectable() {
    let files = ArchiveFiles {
        manifest: MANIFEST.to_vec(),
        module: MODULE.to_vec(),
        signature: SIGNATURE.to_vec(),
        assets: BTreeMap::from([
            ("assets/a.json".to_owned(), b"a".to_vec()),
            ("assets/z.json".to_owned(), b"z".to_vec()),
        ]),
    };
    let first = build_archive(&files, ArchivePolicy::default()).unwrap();
    let second = build_archive(&files, ArchivePolicy::default()).unwrap();
    assert_eq!(first, second);

    let inspected = inspect_archive(&first, ArchivePolicy::default()).unwrap();
    assert_eq!(inspected.manifest, MANIFEST);
    assert_eq!(inspected.module, MODULE);
    assert_eq!(inspected.signature, SIGNATURE);
    assert_eq!(inspected.assets, files.assets);
}
