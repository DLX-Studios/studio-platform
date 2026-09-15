#![no_main]
use libfuzzer_sys::fuzz_target;
use studio_package::{ArchivePolicy, ManifestPolicy, inspect_archive, parse_manifest};

fuzz_target!(|data: &[u8]| {
    let _ = inspect_archive(data, ArchivePolicy::default());
    let _ = parse_manifest(data, ManifestPolicy::default());
});
