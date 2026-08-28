//! Host-owned Studio Library asset admission.
//!
//! The Library deliberately lives beside [`crate::local_store`].  SurrealDB
//! stores the small catalog and relationship records while immutable blobs
//! live in a SHA-256 content-addressed directory.  No storage handle, path, or
//! catalog implementation type crosses into `studio-design`; design nodes
//! retain only opaque [`studio_design::LibraryAssetId`] values.
#![allow(
    missing_docs,
    reason = "closed Library records are documented at the module and type seams"
)]
#![allow(missing_docs)]
#![allow(clippy::all, clippy::pedantic, clippy::restriction, clippy::nursery)]

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use studio_design::LibraryAssetId;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::local_store::{
    EmbeddedLocalStore, LocalStore, LocalStoreDiagnosticCode, LocalStoreError, StoreBatch,
    StoreBatchEntry,
};

const CATALOG_BATCH_ID: &str = "studio-library-assets-catalog-v1";
const CATALOG_SCHEMA_VERSION: u16 = 1;
const LIBRARY_DIRECTORY: &str = ".studio-library";
const BLOB_DIRECTORY: &str = "blobs";

/// A closed media classification used by the Designer Library.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Image,
    Video,
    Audio,
    Font,
    Document,
    Icon,
}

/// Descriptive alias used by callers that model the field as a media kind.
pub type MediaKind = AssetKind;

/// Formats admitted by the v1 host decoder matrix.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetFormat {
    Png,
    Jpeg,
    Webp,
    Gif,
    Avif,
    Svg,
    Mp4,
    Webm,
    Mov,
    Mp3,
    Wav,
    Ogg,
    Flac,
    M4a,
    Aac,
    Woff2,
    Woff,
    Ttf,
    Otf,
    Pdf,
    PlainText,
    Markdown,
}

impl AssetFormat {
    fn from_name(name: &str) -> Option<Self> {
        match Path::new(name)
            .extension()?
            .to_str()?
            .to_ascii_lowercase()
            .as_str()
        {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::Webp),
            "gif" => Some(Self::Gif),
            "avif" => Some(Self::Avif),
            "svg" => Some(Self::Svg),
            "mp4" => Some(Self::Mp4),
            "webm" => Some(Self::Webm),
            "mov" => Some(Self::Mov),
            "mp3" => Some(Self::Mp3),
            "wav" => Some(Self::Wav),
            "ogg" => Some(Self::Ogg),
            "flac" => Some(Self::Flac),
            "m4a" => Some(Self::M4a),
            "aac" => Some(Self::Aac),
            "woff2" => Some(Self::Woff2),
            "woff" => Some(Self::Woff),
            "ttf" => Some(Self::Ttf),
            "otf" => Some(Self::Otf),
            "pdf" => Some(Self::Pdf),
            "txt" => Some(Self::PlainText),
            "md" | "markdown" => Some(Self::Markdown),
            _ => None,
        }
    }

    fn kind(self) -> AssetKind {
        match self {
            Self::Png | Self::Jpeg | Self::Webp | Self::Gif | Self::Avif | Self::Svg => {
                AssetKind::Image
            }
            Self::Mp4 | Self::Webm | Self::Mov => AssetKind::Video,
            Self::Mp3 | Self::Wav | Self::Ogg | Self::Flac | Self::M4a | Self::Aac => {
                AssetKind::Audio
            }
            Self::Woff2 | Self::Woff | Self::Ttf | Self::Otf => AssetKind::Font,
            Self::Pdf | Self::PlainText | Self::Markdown => AssetKind::Document,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Webp => "webp",
            Self::Gif => "gif",
            Self::Avif => "avif",
            Self::Svg => "svg",
            Self::Mp4 => "mp4",
            Self::Webm => "webm",
            Self::Mov => "mov",
            Self::Mp3 => "mp3",
            Self::Wav => "wav",
            Self::Ogg => "ogg",
            Self::Flac => "flac",
            Self::M4a => "m4a",
            Self::Aac => "aac",
            Self::Woff2 => "woff2",
            Self::Woff => "woff",
            Self::Ttf => "ttf",
            Self::Otf => "otf",
            Self::Pdf => "pdf",
            Self::PlainText => "txt",
            Self::Markdown => "md",
        }
    }
}

/// The source category recorded with an admitted asset.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetSourceKind {
    ImportedFile,
    Url,
    Generated,
    Agent,
    Extension,
}

/// Safe, caller-provided provenance.  `source` is an opaque source identity;
/// it is not interpreted as a path and is never used to locate a blob.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetProvenance {
    pub source: String,
    pub actor: String,
    pub kind: AssetSourceKind,
    pub detail: Option<String>,
}

impl AssetProvenance {
    /// Construct file provenance with an optional human-readable detail.
    #[must_use]
    pub fn new(source: impl Into<String>, actor: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            actor: actor.into(),
            kind: AssetSourceKind::ImportedFile,
            detail: None,
        }
    }

    /// Set the source category.
    #[must_use]
    pub const fn with_kind(mut self, kind: AssetSourceKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set safe provenance detail such as an agent run or extension version.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Input accepted by [`LibraryAssetStore::admit`].
#[derive(Clone, Debug)]
pub struct AssetAdmission {
    pub name: String,
    pub bytes: Vec<u8>,
    pub provenance: AssetProvenance,
    pub kind: Option<AssetKind>,
    pub mime_type: Option<String>,
    pub revision: u64,
}

/// Descriptive alias for callers that model admission as a request object.
pub type AssetAdmissionRequest = AssetAdmission;

impl AssetAdmission {
    /// Create an admission request.  The name is used only for format
    /// inference and the display name; provenance remains the source identity.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
        provenance: AssetProvenance,
    ) -> Self {
        Self {
            name: name.into(),
            bytes: bytes.into(),
            provenance,
            kind: None,
            mime_type: None,
            revision: 0,
        }
    }

    /// Override the inferred kind (for example, to classify an SVG as an icon).
    #[must_use]
    pub const fn with_kind(mut self, kind: AssetKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Supply a caller-provided MIME type for catalog display.
    #[must_use]
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Associate the admission with a source/design revision.
    #[must_use]
    pub const fn at_revision(mut self, revision: u64) -> Self {
        self.revision = revision;
        self
    }
}

/// A relative content-addressed blob reference.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BlobReference {
    pub content_hash: String,
    pub relative_path: String,
    pub byte_length: u64,
}

/// One usage reference that must be unbound before safe deletion.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetUsage {
    pub reference_id: String,
    pub owner: String,
    pub field: String,
}

impl AssetUsage {
    /// Construct a usage reference from stable owner and field identities.
    pub fn new(
        reference_id: impl Into<String>,
        owner: impl Into<String>,
        field: impl Into<String>,
    ) -> Result<Self, LibraryAssetError> {
        let usage = Self {
            reference_id: reference_id.into(),
            owner: owner.into(),
            field: field.into(),
        };
        validate_text(&usage.reference_id)?;
        validate_text(&usage.owner)?;
        validate_text(&usage.field)?;
        Ok(usage)
    }
}

/// Deterministic request for a runtime-ready variant.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeVariantSpec {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub format: Option<AssetFormat>,
    pub quality: Option<u8>,
}

impl RuntimeVariantSpec {
    fn key(&self) -> String {
        format!(
            "v1;width={};height={};format={};quality={}",
            self.width.map_or_else(|| "-".to_owned(), |v| v.to_string()),
            self.height
                .map_or_else(|| "-".to_owned(), |v| v.to_string()),
            self.format.map_or("-", AssetFormat::as_str),
            self.quality
                .map_or_else(|| "-".to_owned(), |v| v.to_string()),
        )
    }
}

/// A generated variant and its immutable content-addressed bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeVariant {
    pub id: String,
    pub spec: RuntimeVariantSpec,
    pub blob: BlobReference,
}

/// Persisted metadata for one admitted asset.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AssetMetadata {
    pub id: LibraryAssetId,
    pub name: String,
    pub kind: AssetKind,
    pub format: AssetFormat,
    pub mime_type: Option<String>,
    pub content_hash: String,
    pub byte_length: u64,
    pub provenance: Vec<AssetProvenance>,
    pub created_revision: u64,
    pub updated_revision: u64,
    pub original: BlobReference,
    pub variants: BTreeMap<String, RuntimeVariant>,
    pub usages: Vec<AssetUsage>,
}

/// Descriptive alias for one catalog asset record.
pub type LibraryAsset = AssetMetadata;

impl AssetMetadata {
    /// The preserved source format.
    #[must_use]
    pub const fn original_format(&self) -> AssetFormat {
        self.format
    }

    /// The preserved source blob reference.
    #[must_use]
    pub fn original_blob(&self) -> &BlobReference {
        &self.original
    }
}

/// Bytes returned by an original or variant read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetBlob {
    pub reference: BlobReference,
    pub bytes: Vec<u8>,
}

/// Safe deletion policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeletePolicy {
    /// Reject when any design/content reference remains.
    RequireUnbound,
    /// Delete and return the broken references as explicit diagnostics.
    AllowBreakingChange,
}

/// Result of a successful deletion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteResult {
    pub asset_id: LibraryAssetId,
    pub broken_references: Vec<AssetUsage>,
}

/// Stable Library diagnostic categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibraryDiagnosticCode {
    InvalidAdmission,
    UnsupportedFormat,
    UnsupportedCodec,
    UnsafeSvg,
    AssetNotFound,
    AssetInUse,
    BlobCorrupt,
    Storage,
}

/// User-safe Library diagnostic; raw filesystem and decoder errors are never
/// surfaced through this type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryDiagnostic {
    code: LibraryDiagnosticCode,
    message: String,
}

impl LibraryDiagnostic {
    /// Stable diagnostic code.
    #[must_use]
    pub const fn code(&self) -> LibraryDiagnosticCode {
        self.code
    }

    /// Actionable reason suitable for the Library panel.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Failure from asset admission, storage, validation, or safe deletion.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct LibraryAssetError {
    diagnostic: LibraryDiagnostic,
    message: String,
    usages: Vec<AssetUsage>,
}

impl LibraryAssetError {
    fn new(code: LibraryDiagnosticCode, message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            diagnostic: LibraryDiagnostic {
                code,
                message: message.clone(),
            },
            message,
            usages: Vec::new(),
        }
    }

    fn in_use(usages: Vec<AssetUsage>) -> Self {
        let message = format!("asset is still referenced by {} usage(s)", usages.len());
        Self {
            diagnostic: LibraryDiagnostic {
                code: LibraryDiagnosticCode::AssetInUse,
                message: message.clone(),
            },
            message,
            usages,
        }
    }

    /// Stable diagnostic suitable for UI and automation.
    #[must_use]
    pub const fn diagnostic(&self) -> &LibraryDiagnostic {
        &self.diagnostic
    }

    /// Usage listing that blocked a safe deletion, if any.
    #[must_use]
    pub fn usages(&self) -> &[AssetUsage] {
        &self.usages
    }
}

/// A keyboard-safe, deterministic Library panel model.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LibraryPanelState {
    asset_ids: Vec<LibraryAssetId>,
    focused: Option<usize>,
}

/// Keyboard events understood by [`LibraryPanelState`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibraryPanelKey {
    Up,
    Down,
    Home,
    End,
    Enter,
    Escape,
}

/// Result of one keyboard action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LibraryPanelAction {
    Focused(Option<LibraryAssetId>),
    Activated(Option<LibraryAssetId>),
    Cancelled,
}

impl LibraryPanelState {
    /// Build a panel model in stable identity order.
    #[must_use]
    pub fn new(assets: &[AssetMetadata]) -> Self {
        let mut asset_ids = assets
            .iter()
            .map(|asset| asset.id.clone())
            .collect::<Vec<_>>();
        asset_ids.sort();
        asset_ids.dedup();
        let focused = (!asset_ids.is_empty()).then_some(0);
        Self { asset_ids, focused }
    }

    /// IDs in the panel's deterministic keyboard traversal order.
    #[must_use]
    pub fn focus_order(&self) -> &[LibraryAssetId] {
        &self.asset_ids
    }

    /// Currently focused asset, if the panel is non-empty.
    #[must_use]
    pub fn focused_asset(&self) -> Option<&LibraryAssetId> {
        self.focused.and_then(|index| self.asset_ids.get(index))
    }

    /// Handle a keyboard action without requiring pointer-only interaction.
    pub fn handle_key(&mut self, key: LibraryPanelKey) -> LibraryPanelAction {
        match key {
            LibraryPanelKey::Up => self.move_focus(-1),
            LibraryPanelKey::Down => self.move_focus(1),
            LibraryPanelKey::Home => {
                self.focused = (!self.asset_ids.is_empty()).then_some(0);
                LibraryPanelAction::Focused(self.focused_asset().cloned())
            }
            LibraryPanelKey::End => {
                self.focused = (!self.asset_ids.is_empty()).then_some(self.asset_ids.len() - 1);
                LibraryPanelAction::Focused(self.focused_asset().cloned())
            }
            LibraryPanelKey::Enter => LibraryPanelAction::Activated(self.focused_asset().cloned()),
            LibraryPanelKey::Escape => LibraryPanelAction::Cancelled,
        }
    }

    fn move_focus(&mut self, delta: isize) -> LibraryPanelAction {
        if self.asset_ids.is_empty() {
            self.focused = None;
        } else {
            let current = isize::try_from(self.focused.unwrap_or(0))
                .expect("a panel index must fit in isize");
            let last = isize::try_from(self.asset_ids.len())
                .expect("a panel length must fit in isize")
                - 1;
            self.focused = Some(
                usize::try_from((current + delta).clamp(0, last))
                    .expect("clamped panel index must be non-negative"),
            );
        }
        LibraryPanelAction::Focused(self.focused_asset().cloned())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedCatalog {
    schema_version: u16,
    assets: BTreeMap<LibraryAssetId, AssetMetadata>,
}

/// Content-addressed Library catalog backed by a host-owned LocalStore.
#[derive(Clone)]
pub struct LibraryAssetStore {
    store: Arc<EmbeddedLocalStore>,
    root: PathBuf,
    catalog_lock: Arc<Mutex<()>>,
}

impl LibraryAssetStore {
    /// Open a Library beside a new or existing LocalStore directory.
    pub async fn open(
        directory: impl Into<PathBuf>,
        durability: crate::Durability,
    ) -> Result<Self, LibraryAssetError> {
        let store = EmbeddedLocalStore::open(directory, durability)
            .await
            .map_err(map_store_error)?;
        Ok(Self::from_store(Arc::new(store)))
    }

    /// Close the underlying LocalStore when this Library is its sole owner.
    pub async fn close(self) -> Result<(), LibraryAssetError> {
        let store = Arc::try_unwrap(self.store).map_err(|_| {
            LibraryAssetError::new(
                LibraryDiagnosticCode::Storage,
                "the Library LocalStore is still shared by another host service",
            )
        })?;
        store.close().await.map_err(map_store_error)
    }

    /// Recover ownership of the embedded store when no other service shares it.
    pub fn try_into_store(self) -> Result<EmbeddedLocalStore, Self> {
        match Arc::try_unwrap(self.store) {
            Ok(store) => Ok(store),
            Err(store) => Err(Self {
                store,
                root: self.root,
                catalog_lock: self.catalog_lock,
            }),
        }
    }

    /// Attach Library storage to an already opened host LocalStore.
    #[must_use]
    pub fn from_store(store: Arc<EmbeddedLocalStore>) -> Self {
        let root = store.directory().join(LIBRARY_DIRECTORY);
        Self {
            store,
            root,
            catalog_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Attach Library storage to an owned LocalStore.
    #[must_use]
    pub fn from_embedded_store(store: EmbeddedLocalStore) -> Self {
        Self::from_store(Arc::new(store))
    }

    /// Alias emphasizing that the LocalStore remains host-owned.
    #[must_use]
    pub fn new(store: Arc<EmbeddedLocalStore>) -> Self {
        Self::from_store(store)
    }

    /// Admit an asset, deduplicating by the exact SHA-256 source bytes.
    pub async fn admit(&self, request: AssetAdmission) -> Result<AssetMetadata, LibraryAssetError> {
        let _catalog_guard = self.catalog_lock.lock().await;
        let name = display_name(&request.name)?;
        validate_text(&request.provenance.source)?;
        validate_text(&request.provenance.actor)?;
        if let Some(detail) = &request.provenance.detail {
            validate_text(detail)?;
        }
        if let Some(mime_type) = &request.mime_type {
            validate_text(mime_type)?;
        }
        let format = infer_format(&name, request.mime_type.as_deref(), &request.bytes)?;
        validate_bytes(format, &request.bytes)?;
        let hash = hash_bytes(&request.bytes);
        let id = LibraryAssetId::new(format!("asset-sha256-{hash}")).map_err(|_| {
            LibraryAssetError::new(
                LibraryDiagnosticCode::InvalidAdmission,
                "asset hash identity is invalid",
            )
        })?;
        let original = self.write_blob(&hash, &request.bytes)?;
        let mut catalog = self.read_catalog().await?;
        if let Some(existing) = catalog.assets.get_mut(&id) {
            if !existing.provenance.contains(&request.provenance) {
                existing.provenance.push(request.provenance);
                existing.provenance.sort_by_key(|value| {
                    (
                        value.source.clone(),
                        value.actor.clone(),
                        value.kind,
                        value.detail.clone(),
                    )
                });
            }
            existing.updated_revision = existing.updated_revision.max(request.revision);
            let updated = existing.clone();
            self.write_catalog(&catalog).await?;
            return Ok(updated);
        }
        let metadata = AssetMetadata {
            id: id.clone(),
            name,
            kind: request.kind.unwrap_or_else(|| format.kind()),
            format,
            mime_type: request.mime_type,
            content_hash: hash,
            byte_length: byte_length(&request.bytes),
            provenance: vec![request.provenance],
            created_revision: request.revision,
            updated_revision: request.revision,
            original,
            variants: BTreeMap::new(),
            usages: Vec::new(),
        };
        catalog.assets.insert(id, metadata.clone());
        self.write_catalog(&catalog).await?;
        Ok(metadata)
    }

    /// List all admitted assets in stable identity order.
    pub async fn list(&self) -> Result<Vec<AssetMetadata>, LibraryAssetError> {
        Ok(self.read_catalog().await?.assets.into_values().collect())
    }

    /// Retrieve metadata for one opaque asset identity.
    pub async fn metadata(&self, id: &LibraryAssetId) -> Result<AssetMetadata, LibraryAssetError> {
        self.read_catalog()
            .await?
            .assets
            .get(id)
            .cloned()
            .ok_or_else(|| {
                LibraryAssetError::new(
                    LibraryDiagnosticCode::AssetNotFound,
                    "the Library asset identity was not found",
                )
            })
    }

    /// Retrieve and hash-check the preserved source original.
    pub async fn read_original(&self, id: &LibraryAssetId) -> Result<AssetBlob, LibraryAssetError> {
        let metadata = self.metadata(id).await?;
        self.read_blob(&metadata.original)
    }

    /// Generate or retrieve a deterministic runtime variant.  V1 keeps the
    /// original bytes byte-for-byte while recording the normalized variant
    /// contract; decoder-specific transcoding can replace this host seam
    /// without changing identity or catalog references.
    pub async fn generate_runtime_variant(
        &self,
        id: &LibraryAssetId,
        spec: RuntimeVariantSpec,
    ) -> Result<RuntimeVariant, LibraryAssetError> {
        let _catalog_guard = self.catalog_lock.lock().await;
        let mut catalog = self.read_catalog().await?;
        let generated = {
            let metadata = catalog.assets.get_mut(id).ok_or_else(|| {
                LibraryAssetError::new(
                    LibraryDiagnosticCode::AssetNotFound,
                    "the Library asset identity was not found",
                )
            })?;
            let key = spec.key();
            if let Some(existing) = metadata.variants.get(&key) {
                return Ok(existing.clone());
            }
            let variant_hash = hash_bytes(format!("{}\n{key}", metadata.content_hash).as_bytes());
            let bytes = self.read_blob(&metadata.original)?.bytes;
            let blob = self.write_variant_blob(&variant_hash, &bytes)?;
            let variant = RuntimeVariant {
                id: format!("variant-sha256-{variant_hash}"),
                spec,
                blob,
            };
            metadata.variants.insert(key, variant.clone());
            variant
        };
        self.write_catalog(&catalog).await?;
        Ok(generated)
    }

    /// Retrieve a previously generated runtime variant and hash-check it.
    pub async fn read_runtime_variant(
        &self,
        id: &LibraryAssetId,
        spec: &RuntimeVariantSpec,
    ) -> Result<AssetBlob, LibraryAssetError> {
        let metadata = self.metadata(id).await?;
        let variant = metadata.variants.get(&spec.key()).ok_or_else(|| {
            LibraryAssetError::new(
                LibraryDiagnosticCode::AssetNotFound,
                "the requested runtime variant was not generated",
            )
        })?;
        self.read_blob(&variant.blob)
    }

    /// Add one stable design/content usage reference.
    pub async fn bind(
        &self,
        id: &LibraryAssetId,
        usage: AssetUsage,
    ) -> Result<(), LibraryAssetError> {
        let _catalog_guard = self.catalog_lock.lock().await;
        let mut catalog = self.read_catalog().await?;
        let metadata = catalog.assets.get_mut(id).ok_or_else(|| {
            LibraryAssetError::new(
                LibraryDiagnosticCode::AssetNotFound,
                "the Library asset identity was not found",
            )
        })?;
        if !metadata.usages.contains(&usage) {
            metadata.usages.push(usage);
            metadata.usages.sort_by_key(|value| {
                (
                    value.reference_id.clone(),
                    value.owner.clone(),
                    value.field.clone(),
                )
            });
            self.write_catalog(&catalog).await?;
        }
        Ok(())
    }

    /// Remove one usage reference; missing references are idempotent.
    pub async fn unbind(
        &self,
        id: &LibraryAssetId,
        reference_id: &str,
    ) -> Result<(), LibraryAssetError> {
        let _catalog_guard = self.catalog_lock.lock().await;
        validate_text(reference_id)?;
        let mut catalog = self.read_catalog().await?;
        let metadata = catalog.assets.get_mut(id).ok_or_else(|| {
            LibraryAssetError::new(
                LibraryDiagnosticCode::AssetNotFound,
                "the Library asset identity was not found",
            )
        })?;
        let old_len = metadata.usages.len();
        metadata
            .usages
            .retain(|usage| usage.reference_id != reference_id);
        if metadata.usages.len() != old_len {
            self.write_catalog(&catalog).await?;
        }
        Ok(())
    }

    /// Return the current usage listing used by the Library panel.
    pub async fn usages(&self, id: &LibraryAssetId) -> Result<Vec<AssetUsage>, LibraryAssetError> {
        Ok(self.metadata(id).await?.usages)
    }

    /// Safely delete an asset only after all references are unbound.
    pub async fn delete(&self, id: &LibraryAssetId) -> Result<DeleteResult, LibraryAssetError> {
        self.delete_with_policy(id, DeletePolicy::RequireUnbound)
            .await
    }

    /// Delete under an explicit policy. Breaking deletion returns the exact
    /// references it invalidated so callers can leave a visible diagnostic.
    pub async fn delete_with_policy(
        &self,
        id: &LibraryAssetId,
        policy: DeletePolicy,
    ) -> Result<DeleteResult, LibraryAssetError> {
        let _catalog_guard = self.catalog_lock.lock().await;
        let mut catalog = self.read_catalog().await?;
        let metadata = catalog.assets.get(id).cloned().ok_or_else(|| {
            LibraryAssetError::new(
                LibraryDiagnosticCode::AssetNotFound,
                "the Library asset identity was not found",
            )
        })?;
        if policy == DeletePolicy::RequireUnbound && !metadata.usages.is_empty() {
            return Err(LibraryAssetError::in_use(metadata.usages));
        }
        catalog.assets.remove(id);
        self.write_catalog(&catalog).await?;
        self.remove_blob(&metadata.original)?;
        for variant in metadata.variants.values() {
            self.remove_blob(&variant.blob)?;
        }
        Ok(DeleteResult {
            asset_id: id.clone(),
            broken_references: if policy == DeletePolicy::AllowBreakingChange {
                metadata.usages
            } else {
                Vec::new()
            },
        })
    }

    async fn read_catalog(&self) -> Result<PersistedCatalog, LibraryAssetError> {
        let entries = self
            .store
            .batch_entries(CATALOG_BATCH_ID)
            .await
            .map_err(map_store_error)?;
        if entries.is_empty() {
            return Ok(PersistedCatalog {
                schema_version: CATALOG_SCHEMA_VERSION,
                assets: BTreeMap::new(),
            });
        }
        let [entry] = entries.as_slice() else {
            return Err(LibraryAssetError::new(
                LibraryDiagnosticCode::Storage,
                "the Library catalog has an invalid record shape",
            ));
        };
        let catalog =
            serde_json::from_value::<PersistedCatalog>(entry.payload.clone()).map_err(|_| {
                LibraryAssetError::new(
                    LibraryDiagnosticCode::Storage,
                    "the Library catalog is damaged",
                )
            })?;
        if catalog.schema_version != CATALOG_SCHEMA_VERSION {
            return Err(LibraryAssetError::new(
                LibraryDiagnosticCode::Storage,
                "the Library catalog schema is unsupported",
            ));
        }
        Ok(catalog)
    }

    async fn write_catalog(&self, catalog: &PersistedCatalog) -> Result<(), LibraryAssetError> {
        let payload = serde_json::to_value(catalog).map_err(|_| {
            LibraryAssetError::new(
                LibraryDiagnosticCode::Storage,
                "the Library catalog could not be encoded",
            )
        })?;
        let batch = StoreBatch::new(
            CATALOG_BATCH_ID,
            [StoreBatchEntry {
                ordinal: 0,
                payload,
            }],
        )
        .map_err(map_store_error)?;
        self.store
            .write_batch(&batch)
            .await
            .map_err(map_store_error)
    }

    fn write_blob(&self, hash: &str, bytes: &[u8]) -> Result<BlobReference, LibraryAssetError> {
        let relative_path = format!("{BLOB_DIRECTORY}/sha256/{}/{hash}", &hash[..2]);
        let path = self.root.join(&relative_path);
        write_if_absent(&path, bytes)?;
        Ok(BlobReference {
            content_hash: hash.to_owned(),
            relative_path,
            byte_length: byte_length(bytes),
        })
    }

    fn write_variant_blob(
        &self,
        hash: &str,
        bytes: &[u8],
    ) -> Result<BlobReference, LibraryAssetError> {
        let relative_path = format!("{BLOB_DIRECTORY}/variants/{}/{hash}", &hash[..2]);
        let path = self.root.join(&relative_path);
        write_if_absent(&path, bytes)?;
        Ok(BlobReference {
            content_hash: hash_bytes(bytes),
            relative_path,
            byte_length: byte_length(bytes),
        })
    }

    fn read_blob(&self, reference: &BlobReference) -> Result<AssetBlob, LibraryAssetError> {
        let bytes = fs::read(self.root.join(&reference.relative_path)).map_err(|_| {
            LibraryAssetError::new(
                LibraryDiagnosticCode::BlobCorrupt,
                "the Library blob is missing or unreadable",
            )
        })?;
        if byte_length(&bytes) != reference.byte_length
            || hash_bytes(&bytes) != reference.content_hash
        {
            return Err(LibraryAssetError::new(
                LibraryDiagnosticCode::BlobCorrupt,
                "the Library blob hash does not match its catalog metadata",
            ));
        }
        Ok(AssetBlob {
            reference: reference.clone(),
            bytes,
        })
    }

    fn remove_blob(&self, reference: &BlobReference) -> Result<(), LibraryAssetError> {
        match fs::remove_file(self.root.join(&reference.relative_path)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(LibraryAssetError::new(
                LibraryDiagnosticCode::Storage,
                "the Library blob could not be removed",
            )),
        }
    }
}

fn write_if_absent(path: &Path, bytes: &[u8]) -> Result<(), LibraryAssetError> {
    if path.exists() {
        return Ok(());
    }
    let parent = path.parent().ok_or_else(|| {
        LibraryAssetError::new(
            LibraryDiagnosticCode::Storage,
            "the Library blob path is invalid",
        )
    })?;
    fs::create_dir_all(parent).map_err(|_| {
        LibraryAssetError::new(
            LibraryDiagnosticCode::Storage,
            "the Library blob directory could not be created",
        )
    })?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes).map_err(|_| {
        LibraryAssetError::new(
            LibraryDiagnosticCode::Storage,
            "the Library blob could not be written",
        )
    })?;
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(_) if path.exists() => {
            let _ = fs::remove_file(temporary);
            Ok(())
        }
        Err(_) => Err(LibraryAssetError::new(
            LibraryDiagnosticCode::Storage,
            "the Library blob could not be committed",
        )),
    }
}

fn display_name(name: &str) -> Result<String, LibraryAssetError> {
    validate_text(name)?;
    let name = Path::new(name)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            LibraryAssetError::new(
                LibraryDiagnosticCode::InvalidAdmission,
                "asset name must contain a filename",
            )
        })?;
    Ok(name.to_owned())
}

fn validate_text(value: &str) -> Result<(), LibraryAssetError> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return Err(LibraryAssetError::new(
            LibraryDiagnosticCode::InvalidAdmission,
            "asset metadata contains an empty or control-bearing value",
        ));
    }
    Ok(())
}

fn infer_format(
    name: &str,
    mime: Option<&str>,
    bytes: &[u8],
) -> Result<AssetFormat, LibraryAssetError> {
    let format = AssetFormat::from_name(name).or_else(|| {
        mime.and_then(|mime| match mime.to_ascii_lowercase().as_str() {
            "image/png" => Some(AssetFormat::Png),
            "image/jpeg" => Some(AssetFormat::Jpeg),
            "image/webp" => Some(AssetFormat::Webp),
            "image/svg+xml" => Some(AssetFormat::Svg),
            "application/pdf" => Some(AssetFormat::Pdf),
            "text/plain" => Some(AssetFormat::PlainText),
            "text/markdown" => Some(AssetFormat::Markdown),
            _ => None,
        })
    });
    format.ok_or_else(|| {
        let reason = if bytes.starts_with(b"RIFF") {
            "the container is not in the approved Library format matrix"
        } else {
            "the filename or MIME type is not in the approved Library format matrix"
        };
        LibraryAssetError::new(LibraryDiagnosticCode::UnsupportedFormat, reason)
    })
}

fn validate_bytes(format: AssetFormat, bytes: &[u8]) -> Result<(), LibraryAssetError> {
    let has = |needle: &[u8]| bytes.windows(needle.len()).any(|part| part == needle);
    let valid = match format {
        AssetFormat::Png => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        AssetFormat::Jpeg => bytes.starts_with(b"\xff\xd8\xff"),
        AssetFormat::Webp => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(&b"WEBP"[..]),
        AssetFormat::Gif => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        AssetFormat::Avif => has(b"ftypavif") || has(b"ftypavis"),
        AssetFormat::Svg => is_safe_svg(bytes)?,
        AssetFormat::Mp4 => has(b"ftyp") && approved_codec(bytes),
        AssetFormat::Mov => has(b"ftypqt  ") && approved_codec(bytes),
        AssetFormat::Webm => bytes.starts_with(b"\x1a\x45\xdf\xa3") && approved_codec(bytes),
        AssetFormat::Mp3 => bytes.starts_with(b"ID3") || bytes.starts_with(b"\xff\xfb"),
        AssetFormat::Wav => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(&b"WAVE"[..]),
        AssetFormat::Ogg => bytes.starts_with(b"OggS"),
        AssetFormat::Flac => bytes.starts_with(b"fLaC"),
        AssetFormat::M4a => has(b"ftypM4A") || has(b"ftypisom"),
        AssetFormat::Aac => bytes.starts_with(b"\xff\xf1") || bytes.starts_with(b"\xff\xf9"),
        AssetFormat::Woff2 => bytes.starts_with(b"wOF2"),
        AssetFormat::Woff => bytes.starts_with(b"wOFF"),
        AssetFormat::Ttf => bytes.starts_with(b"\x00\x01\x00\x00"),
        AssetFormat::Otf => bytes.starts_with(b"OTTO"),
        AssetFormat::Pdf => bytes.starts_with(b"%PDF-"),
        AssetFormat::PlainText | AssetFormat::Markdown => std::str::from_utf8(bytes).is_ok(),
    };
    if valid {
        Ok(())
    } else if matches!(
        format,
        AssetFormat::Mp4 | AssetFormat::Mov | AssetFormat::Webm | AssetFormat::M4a
    ) && has(b"codec=")
    {
        Err(LibraryAssetError::new(
            LibraryDiagnosticCode::UnsupportedCodec,
            "the media container declares an unsupported codec",
        ))
    } else {
        Err(LibraryAssetError::new(
            LibraryDiagnosticCode::UnsupportedFormat,
            "the asset bytes do not match the declared format",
        ))
    }
}

fn approved_codec(bytes: &[u8]) -> bool {
    let Some(start) = bytes.windows(6).position(|part| part == b"codec=") else {
        return true;
    };
    let value = &bytes[start + 6..];
    let end = value
        .iter()
        .position(|byte| *byte == b' ' || *byte == b'\n' || *byte == 0)
        .unwrap_or(value.len());
    let codec = std::str::from_utf8(&value[..end]).unwrap_or_default();
    matches!(
        codec,
        "h264" | "avc1" | "hevc" | "hvc1" | "vp8" | "vp9" | "av1" | "aac" | "opus" | "vorbis"
    )
}

fn is_safe_svg(bytes: &[u8]) -> Result<bool, LibraryAssetError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| {
            LibraryAssetError::new(LibraryDiagnosticCode::UnsafeSvg, "SVG is not valid UTF-8")
        })?
        .to_ascii_lowercase();
    if !text.contains("<svg") {
        return Ok(false);
    }
    let unsafe_markers = [
        ("<script", "SVG contains script content"),
        ("javascript:", "SVG contains a javascript URL"),
        ("onload=", "SVG contains an executable event handler"),
        ("onclick=", "SVG contains an executable event handler"),
        ("onerror=", "SVG contains an executable event handler"),
        ("<foreignobject", "SVG contains foreign HTML content"),
        ("<!entity", "SVG contains an external entity declaration"),
        (
            "<!doctype",
            "SVG contains a potentially external document type",
        ),
        ("<iframe", "SVG contains an embedded frame"),
        ("<object", "SVG contains an embedded object"),
        ("<embed", "SVG contains an embedded resource"),
        ("href=\"http", "SVG contains an external resource URL"),
        ("href='http", "SVG contains an external resource URL"),
        ("xlink:href=\"http", "SVG contains an external resource URL"),
        ("url(http", "SVG contains an external CSS URL"),
    ];
    if let Some((_, reason)) = unsafe_markers
        .iter()
        .find(|(marker, _)| text.contains(marker))
    {
        return Err(LibraryAssetError::new(
            LibraryDiagnosticCode::UnsafeSvg,
            *reason,
        ));
    }
    Ok(true)
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hash = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut hash, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hash
}

fn byte_length(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).expect("a process byte slice must fit in u64")
}

fn map_store_error(error: LocalStoreError) -> LibraryAssetError {
    let code = match error.diagnostic().code() {
        LocalStoreDiagnosticCode::BatchInvalid => LibraryDiagnosticCode::InvalidAdmission,
        LocalStoreDiagnosticCode::DirectoryInvalid
        | LocalStoreDiagnosticCode::DurabilityInvalid
        | LocalStoreDiagnosticCode::RecoveryUnavailable
        | LocalStoreDiagnosticCode::EngineManifestCorrupt
        | LocalStoreDiagnosticCode::EngineIncompatible
        | LocalStoreDiagnosticCode::EngineOpenFailed
        | LocalStoreDiagnosticCode::SchemaMetadataCorrupt
        | LocalStoreDiagnosticCode::SchemaIncompatible
        | LocalStoreDiagnosticCode::OperationFailed
        | LocalStoreDiagnosticCode::ExecutorUnavailable
        | LocalStoreDiagnosticCode::QueryTimedOut => LibraryDiagnosticCode::Storage,
    };
    LibraryAssetError::new(code, error.diagnostic().message())
}
