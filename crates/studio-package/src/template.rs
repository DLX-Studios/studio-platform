//! Deterministic template-bundle packaging: shareable `.studio` sources.
//!
//! Template bundles distribute template sources (`template.json` plus the
//! project tree), never compiled modules. The native runtime never accepts
//! this layout (`inspect_archive` fails it closed as `LayoutInvalid`);
//! `studio new` is its only reader. Archives follow the app-bundle
//! conventions (stored entries, fixed metadata, lexicographic order) so
//! identical trees pack byte-identically.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter, write::SimpleFileOptions};

use super::archive::{ArchiveError, ArchivePolicy};

/// Required template-bundle entries besides `assets/*`.
const REQUIRED_ENTRIES: &[&str] = &["template.json", "manifest.json", "app.studio"];

/// Complete files used to build one deterministic template bundle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateFiles {
    /// Exact `template.json` bytes (carries a non-empty `id`).
    pub template_json: Vec<u8>,
    /// Exact project `manifest.json` bytes.
    pub manifest: Vec<u8>,
    /// Exact `app.studio` source bytes.
    pub entry_source: Vec<u8>,
    /// Normalized asset path to exact bytes, sorted by path.
    pub assets: BTreeMap<String, Vec<u8>>,
}

/// Owned template contents after complete structural validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectedTemplate {
    /// Exact `template.json` bytes.
    pub template_json: Vec<u8>,
    /// Exact project `manifest.json` bytes.
    pub manifest: Vec<u8>,
    /// Exact `app.studio` source bytes.
    pub entry_source: Vec<u8>,
    /// Sorted normalized assets.
    pub assets: BTreeMap<String, Vec<u8>>,
}

/// Build a byte-deterministic template bundle.
///
/// # Errors
///
/// Returns [`ArchiveError`] for missing template identity, limit breaches,
/// or ZIP failures.
pub fn pack_template(
    files: &TemplateFiles,
    policy: ArchivePolicy,
) -> Result<Vec<u8>, ArchiveError> {
    template_id(&files.template_json)?;
    let mut entries = BTreeMap::from([
        ("template.json".to_owned(), files.template_json.as_slice()),
        ("manifest.json".to_owned(), files.manifest.as_slice()),
        ("app.studio".to_owned(), files.entry_source.as_slice()),
    ]);
    for (path, contents) in &files.assets {
        if !path.starts_with("assets/") || entries.insert(path.clone(), contents).is_some() {
            return Err(ArchiveError::LayoutInvalid(path.clone()));
        }
    }
    if entries.len() > policy.max_entries {
        return Err(ArchiveError::SizeLimit("entry count"));
    }
    let bytes =
        write_entries(&entries).map_err(|error| ArchiveError::InvalidZip(error.to_string()))?;
    inspect_template(&bytes, policy)?;
    Ok(bytes)
}

/// Inspect an untrusted template bundle without extracting to disk.
///
/// # Errors
///
/// Returns [`ArchiveError`] for malformed ZIP, path, layout, metadata,
/// ordering, or limit violations.
pub fn inspect_template(
    bytes: &[u8],
    policy: ArchivePolicy,
) -> Result<InspectedTemplate, ArchiveError> {
    if bytes.len() > policy.max_archive_bytes {
        return Err(ArchiveError::SizeLimit("archive"));
    }
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| ArchiveError::InvalidZip(error.to_string()))?;
    if archive.len() > policy.max_entries {
        return Err(ArchiveError::SizeLimit("entry count"));
    }
    if !archive.comment().is_empty() {
        return Err(ArchiveError::MetadataInvalid("archive comment"));
    }
    let mut seen = std::collections::HashSet::new();
    let mut previous: Option<String> = None;
    let mut found = BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| ArchiveError::InvalidZip(error.to_string()))?;
        let path = entry.name().to_owned();
        if path.is_empty()
            || path.starts_with('/')
            || path.contains(['\\', '\0'])
            || path.chars().any(char::is_control)
        {
            return Err(ArchiveError::PathInvalid(path));
        }
        if !seen.insert(path.clone()) {
            return Err(ArchiveError::DuplicatePath(path));
        }
        if previous
            .as_deref()
            .is_some_and(|previous| previous >= path.as_str())
        {
            return Err(ArchiveError::OrderInvalid);
        }
        previous = Some(path.clone());
        if !entry.is_file()
            || entry
                .unix_mode()
                .is_none_or(|mode| mode & 0o170_000 != 0o100_000)
        {
            return Err(ArchiveError::EntryTypeInvalid(path));
        }
        if entry.compression() != CompressionMethod::Stored {
            return Err(ArchiveError::CompressionInvalid(path));
        }
        if entry.unix_mode() != Some(0o100_644)
            || entry.last_modified() != Some(DateTime::DEFAULT)
            || !entry.comment().is_empty()
            || entry.extra_data().is_some_and(|extra| !extra.is_empty())
        {
            return Err(ArchiveError::MetadataInvalid("entry metadata"));
        }
        let allowed = REQUIRED_ENTRIES.contains(&path.as_str()) || path.starts_with("assets/");
        if !allowed {
            return Err(ArchiveError::LayoutInvalid(path));
        }
        let limit = if path.starts_with("assets/") {
            policy.max_asset_bytes
        } else {
            1024 * 1024
        };
        let declared_size =
            usize::try_from(entry.size()).map_err(|_| ArchiveError::SizeLimit("entry"))?;
        if declared_size > limit {
            return Err(ArchiveError::SizeLimit("entry"));
        }
        let mut contents = Vec::with_capacity(declared_size);
        entry
            .by_ref()
            .take(u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1))
            .read_to_end(&mut contents)
            .map_err(|error| ArchiveError::InvalidZip(error.to_string()))?;
        if contents.len() != declared_size {
            return Err(ArchiveError::SizeLimit("entry"));
        }
        found.insert(path, contents);
    }
    let mut take = |name: &str| {
        found
            .remove(name)
            .ok_or_else(|| ArchiveError::LayoutInvalid(name.to_owned()))
    };
    let template_json = take("template.json")?;
    let manifest = take("manifest.json")?;
    let entry_source = take("app.studio")?;
    template_id(&template_json)?;
    Ok(InspectedTemplate {
        template_json,
        manifest,
        entry_source,
        assets: found,
    })
}

/// Require a non-empty template `id`.
fn template_id(template_json: &[u8]) -> Result<(), ArchiveError> {
    let value: serde_json::Value = serde_json::from_slice(template_json)
        .map_err(|error| ArchiveError::InvalidZip(error.to_string()))?;
    if value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|id| !id.trim().is_empty())
    {
        Ok(())
    } else {
        Err(ArchiveError::LayoutInvalid("template.json".to_owned()))
    }
}

/// Write sorted stored entries with fixed metadata.
fn write_entries(entries: &BTreeMap<String, &[u8]>) -> Result<Vec<u8>, zip::result::ZipError> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::DEFAULT
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::DEFAULT)
        .unix_permissions(0o644);
    for (path, contents) in entries {
        writer.start_file(path, options)?;
        writer.write_all(contents)?;
    }
    Ok(writer.finish()?.into_inner())
}
