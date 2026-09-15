//! Bounded ZIP inspection and byte-deterministic stored archive construction.

use std::{
    collections::{BTreeMap, HashSet},
    io::{Cursor, Read, Write},
};

use thiserror::Error;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Host archive, entry, and aggregate byte ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchivePolicy {
    /// Maximum encoded `.studio` archive bytes.
    pub max_archive_bytes: usize,
    /// Maximum uncompressed `module.wasm` bytes.
    pub max_module_bytes: usize,
    /// Maximum aggregate uncompressed declared asset bytes.
    pub max_asset_bytes: usize,
    /// Maximum central-directory entry count.
    pub max_entries: usize,
}

impl Default for ArchivePolicy {
    fn default() -> Self {
        Self {
            max_archive_bytes: 16 * 1024 * 1024,
            max_module_bytes: 8 * 1024 * 1024,
            max_asset_bytes: 1024 * 1024,
            max_entries: 1024,
        }
    }
}

/// Complete files used to build one deterministic bundle archive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveFiles {
    /// Exact manifest bytes.
    pub manifest: Vec<u8>,
    /// Exact wasm module bytes.
    pub module: Vec<u8>,
    /// Exact raw Ed25519 signature bytes.
    pub signature: Vec<u8>,
    /// Normalized asset path to exact bytes, sorted by path.
    pub assets: BTreeMap<String, Vec<u8>>,
}

/// Owned archive contents after complete structural validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectedArchive {
    /// Exact manifest bytes.
    pub manifest: Vec<u8>,
    /// Exact wasm module bytes.
    pub module: Vec<u8>,
    /// Exact raw signature bytes.
    pub signature: Vec<u8>,
    /// Sorted normalized assets.
    pub assets: BTreeMap<String, Vec<u8>>,
}

/// Inspect an untrusted archive without extracting to disk.
///
/// # Errors
///
/// Returns [`ArchiveError`] for malformed ZIP, path, type, metadata, ordering, layout, or limit
/// violations.
pub fn inspect_archive(
    bytes: &[u8],
    policy: ArchivePolicy,
) -> Result<InspectedArchive, ArchiveError> {
    if bytes.len() > policy.max_archive_bytes {
        return Err(ArchiveError::SizeLimit("archive"));
    }
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| ArchiveError::InvalidZip(error.to_string()))?;
    if classic_central_directory_entry_count(bytes).is_some_and(|count| count != archive.len()) {
        // `zip` indexes files by raw name and therefore collapses exact duplicate names. Compare
        // its visible entries with the independently declared central-directory count before any
        // layout checks so an attacker cannot hide a duplicate required entry.
        return Err(ArchiveError::DuplicatePath("central directory".to_owned()));
    }
    if archive.len() > policy.max_entries {
        return Err(ArchiveError::SizeLimit("entry count"));
    }
    if !archive.comment().is_empty() {
        return Err(ArchiveError::MetadataInvalid("archive comment"));
    }

    let mut exact_paths = HashSet::new();
    let mut folded_paths = HashSet::new();
    let mut previous_path: Option<String> = None;
    let mut manifest = None;
    let mut module = None;
    let mut signature = None;
    let mut assets = BTreeMap::new();
    let mut asset_bytes = 0_usize;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| ArchiveError::InvalidZip(error.to_string()))?;
        let path = entry.name().to_owned();
        validate_path(&path)?;
        if !exact_paths.insert(path.clone()) || !folded_paths.insert(path.to_ascii_lowercase()) {
            return Err(ArchiveError::DuplicatePath(path));
        }
        if previous_path
            .as_deref()
            .is_some_and(|previous| previous >= path.as_str())
        {
            return Err(ArchiveError::OrderInvalid);
        }
        previous_path = Some(path.clone());
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

        let declared_size =
            usize::try_from(entry.size()).map_err(|_| ArchiveError::SizeLimit("entry"))?;
        let limit = match path.as_str() {
            "manifest.json" => 64 * 1024,
            "module.wasm" => policy.max_module_bytes,
            "signature.ed25519" => 64,
            _ if path.starts_with("assets/") => policy.max_asset_bytes,
            _ => return Err(ArchiveError::LayoutInvalid(path)),
        };
        if declared_size > limit {
            return Err(ArchiveError::SizeLimit("entry"));
        }
        let mut contents = Vec::with_capacity(declared_size);
        entry
            .by_ref()
            .take(u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1))
            .read_to_end(&mut contents)
            .map_err(|error| ArchiveError::InvalidZip(error.to_string()))?;
        if contents.len() != declared_size || contents.len() > limit {
            return Err(ArchiveError::SizeLimit("entry"));
        }
        match path.as_str() {
            "manifest.json" => manifest = Some(contents),
            "module.wasm" => module = Some(contents),
            "signature.ed25519" => signature = Some(contents),
            _ => {
                asset_bytes = asset_bytes
                    .checked_add(contents.len())
                    .ok_or(ArchiveError::SizeLimit("assets"))?;
                if asset_bytes > policy.max_asset_bytes {
                    return Err(ArchiveError::SizeLimit("assets"));
                }
                assets.insert(path, contents);
            }
        }
    }

    Ok(InspectedArchive {
        manifest: manifest
            .ok_or_else(|| ArchiveError::LayoutInvalid("manifest.json".to_owned()))?,
        module: module.ok_or_else(|| ArchiveError::LayoutInvalid("module.wasm".to_owned()))?,
        signature: signature
            .ok_or_else(|| ArchiveError::LayoutInvalid("signature.ed25519".to_owned()))?,
        assets,
    })
}

/// Build a byte-deterministic stored ZIP archive.
///
/// # Errors
///
/// Returns [`ArchiveError`] for invalid inputs, limits, ZIP writing, or failed self-inspection.
pub fn build_archive(files: &ArchiveFiles, policy: ArchivePolicy) -> Result<Vec<u8>, ArchiveError> {
    if files.module.len() > policy.max_module_bytes
        || files.signature.len() != 64
        || files.assets.len().saturating_add(3) > policy.max_entries
    {
        return Err(ArchiveError::SizeLimit("bundle files"));
    }
    let mut entries = BTreeMap::from([
        ("manifest.json".to_owned(), files.manifest.as_slice()),
        ("module.wasm".to_owned(), files.module.as_slice()),
        ("signature.ed25519".to_owned(), files.signature.as_slice()),
    ]);
    for (path, contents) in &files.assets {
        validate_path(path)?;
        if !path.starts_with("assets/") || entries.insert(path.clone(), contents).is_some() {
            return Err(ArchiveError::LayoutInvalid(path.clone()));
        }
    }

    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::DEFAULT
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::DEFAULT)
        .unix_permissions(0o644);
    for (path, contents) in entries {
        writer
            .start_file(path, options)
            .map_err(|error| ArchiveError::InvalidZip(error.to_string()))?;
        writer
            .write_all(contents)
            .map_err(|error| ArchiveError::InvalidZip(error.to_string()))?;
    }
    let bytes = writer
        .finish()
        .map_err(|error| ArchiveError::InvalidZip(error.to_string()))?
        .into_inner();
    inspect_archive(&bytes, policy)?;
    Ok(bytes)
}

fn validate_path(path: &str) -> Result<(), ArchiveError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains(['\\', '\0'])
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(ArchiveError::PathInvalid(path.to_owned()));
    }
    Ok(())
}

fn classic_central_directory_entry_count(bytes: &[u8]) -> Option<usize> {
    const EOCD_SIGNATURE: &[u8; 4] = b"PK\x05\x06";
    const EOCD_FIXED_LEN: usize = 22;
    const MAX_COMMENT_LEN: usize = u16::MAX as usize;

    let search_start = bytes.len().saturating_sub(EOCD_FIXED_LEN + MAX_COMMENT_LEN);
    let signature_offset = bytes[search_start..]
        .windows(EOCD_SIGNATURE.len())
        .rposition(|window| window == EOCD_SIGNATURE)
        .map(|relative| search_start + relative)?;
    let record = bytes.get(signature_offset..signature_offset + EOCD_FIXED_LEN)?;
    let comment_len = usize::from(u16::from_le_bytes([record[20], record[21]]));
    if signature_offset + EOCD_FIXED_LEN + comment_len != bytes.len() {
        return None;
    }
    let count = u16::from_le_bytes([record[10], record[11]]);
    (count != u16::MAX).then_some(usize::from(count))
}

/// Stable archive rejection family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveErrorCode {
    /// ZIP framing or content stream is malformed.
    InvalidZip,
    /// Entry path is unsafe or non-normalized.
    PathInvalid,
    /// Exact or case-folded entry path is duplicated.
    DuplicatePath,
    /// Entry is not a regular file with fixed permissions.
    EntryTypeInvalid,
    /// Entry uses unsupported compression.
    CompressionInvalid,
    /// Timestamp, permissions, extra fields, or comments differ from the fixed form.
    MetadataInvalid,
    /// Central-directory path order is not lexicographic.
    OrderInvalid,
    /// Archive, entry, aggregate, or count limit exceeded.
    SizeLimit,
    /// Required or allowed archive layout was violated.
    LayoutInvalid,
}

/// Detailed archive validation failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ArchiveError {
    /// ZIP parser or writer failure.
    #[error("invalid ZIP: {0}")]
    InvalidZip(String),
    /// Unsafe entry path.
    #[error("invalid archive path: {0}")]
    PathInvalid(String),
    /// Exact or case-folded collision.
    #[error("duplicate archive path: {0}")]
    DuplicatePath(String),
    /// Unsupported filesystem object or mode.
    #[error("archive entry is not a fixed regular file: {0}")]
    EntryTypeInvalid(String),
    /// Unsupported compression.
    #[error("archive entry must be stored: {0}")]
    CompressionInvalid(String),
    /// Non-deterministic metadata.
    #[error("non-deterministic archive metadata: {0}")]
    MetadataInvalid(&'static str),
    /// Nonlexicographic ordering.
    #[error("archive entries are not in lexicographic order")]
    OrderInvalid,
    /// Fixed resource ceiling exceeded.
    #[error("archive size limit exceeded: {0}")]
    SizeLimit(&'static str),
    /// Missing or undeclared layout entry.
    #[error("invalid archive layout: {0}")]
    LayoutInvalid(String),
}

impl ArchiveError {
    /// Return the stable family for this detailed archive failure.
    #[must_use]
    pub const fn code(&self) -> ArchiveErrorCode {
        match self {
            Self::InvalidZip(_) => ArchiveErrorCode::InvalidZip,
            Self::PathInvalid(_) => ArchiveErrorCode::PathInvalid,
            Self::DuplicatePath(_) => ArchiveErrorCode::DuplicatePath,
            Self::EntryTypeInvalid(_) => ArchiveErrorCode::EntryTypeInvalid,
            Self::CompressionInvalid(_) => ArchiveErrorCode::CompressionInvalid,
            Self::MetadataInvalid(_) => ArchiveErrorCode::MetadataInvalid,
            Self::OrderInvalid => ArchiveErrorCode::OrderInvalid,
            Self::SizeLimit(_) => ArchiveErrorCode::SizeLimit,
            Self::LayoutInvalid(_) => ArchiveErrorCode::LayoutInvalid,
        }
    }
}
