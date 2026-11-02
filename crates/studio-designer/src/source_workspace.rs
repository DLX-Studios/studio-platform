//! Bounded, project-confined access to source files for Designer integrations.
//!
//! This module is transport-independent. Callers must bind a workspace to an
//! already-open project root and use the returned content revision when they
//! make an edit. Writes are staged beside the target and atomically renamed
//! into place so `studio dev` observes either the old file or the complete new
//! file.

use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Maximum size of a project file that this source workspace will read or edit.
pub const MAX_SOURCE_FILE_BYTES: usize = 4 * 1024 * 1024;

/// Maximum number of files returned by one project listing.
pub const MAX_PROJECT_SOURCE_FILES: usize = 4_096;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Metadata for one source file returned by [`ProjectSourceWorkspace::list_files`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileInfo {
    /// Project-relative path using `/` separators.
    pub path: String,
    /// SHA-256 revision of the exact file bytes.
    pub revision: String,
    /// Number of bytes in the file.
    pub bytes: u64,
}

/// Text content and revision for one project source file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileSnapshot {
    /// Project-relative path using `/` separators.
    pub path: String,
    /// SHA-256 revision of the exact UTF-8 file bytes.
    pub revision: String,
    /// UTF-8 file content.
    pub content: String,
}

/// One UTF-8 byte-range replacement in an existing file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatchEdit {
    /// Inclusive start byte offset in the original file.
    pub start: usize,
    /// Exclusive end byte offset in the original file.
    pub end: usize,
    /// Replacement text.
    pub replacement: String,
}

impl PatchEdit {
    /// Construct one byte-range replacement.
    #[must_use]
    pub fn replace(start: usize, end: usize, replacement: impl Into<String>) -> Self {
        Self {
            start,
            end,
            replacement: replacement.into(),
        }
    }
}

/// Failure while opening or using a project source workspace.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProjectSourceError {
    /// The selected project root is missing or is not a directory.
    #[error("project source root is not a directory")]
    InvalidRoot,
    /// A path is not a normalized project-relative file path.
    #[error("invalid project-relative source path: {0}")]
    InvalidPath(String),
    /// The path targets a generated file or an excluded project directory.
    #[error("source path is excluded from Designer file access: {0}")]
    ExcludedPath(String),
    /// A symbolic link resolves outside the project root.
    #[error("source path escapes the project through a symbolic link: {0}")]
    SymlinkEscape(String),
    /// The requested file does not exist.
    #[error("source file does not exist: {0}")]
    NotFound(String),
    /// The requested path does not identify a regular file.
    #[error("source path is not a regular file: {0}")]
    NotFile(String),
    /// A source file cannot be represented as UTF-8 text.
    #[error("source file is not valid UTF-8: {0}")]
    InvalidUtf8(String),
    /// A source file exceeds the per-file size limit.
    #[error("source file exceeds the {max_bytes}-byte limit: {path}")]
    ContentTooLarge {
        /// Project-relative file path.
        path: String,
        /// Maximum allowed size.
        max_bytes: usize,
    },
    /// A project listing would exceed the bounded file count.
    #[error("project contains more than {max_files} accessible source files")]
    TooManyFiles {
        /// Maximum allowed file count.
        max_files: usize,
    },
    /// The file changed after the caller read its revision.
    #[error("source file revision mismatch: expected {expected}, found {actual}")]
    RevisionMismatch {
        /// Revision supplied by the caller.
        expected: String,
        /// Current file revision, or `missing` if the file disappeared.
        actual: String,
    },
    /// A create-only write targeted a path that already exists.
    #[error("source file already exists: {0}")]
    AlreadyExists(String),
    /// A patch contains an invalid, overlapping, or non-UTF-8 byte range.
    #[error("invalid source patch: {0}")]
    InvalidPatch(String),
    /// A filesystem operation failed. The path is project-relative when possible.
    #[error("source workspace I/O error for {path}: {message}")]
    Io {
        /// Project-relative path when available.
        path: String,
        /// Safe filesystem error description.
        message: String,
    },
}

impl ProjectSourceError {
    /// Stable machine-readable failure code for local agent protocols.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidRoot => "PROJECT_ROOT_INVALID",
            Self::InvalidPath(_) => "PATH_INVALID",
            Self::ExcludedPath(_) => "PATH_EXCLUDED",
            Self::SymlinkEscape(_) => "PATH_ESCAPE",
            Self::NotFound(_) => "FILE_NOT_FOUND",
            Self::NotFile(_) => "NOT_A_FILE",
            Self::InvalidUtf8(_) => "FILE_NOT_UTF8",
            Self::ContentTooLarge { .. } => "FILE_TOO_LARGE",
            Self::TooManyFiles { .. } => "TOO_MANY_FILES",
            Self::RevisionMismatch { .. } => "REVISION_CONFLICT",
            Self::AlreadyExists(_) => "FILE_ALREADY_EXISTS",
            Self::InvalidPatch(_) => "PATCH_INVALID",
            Self::Io { .. } => "FILE_IO",
        }
    }
}

/// Read and atomically edit files below one canonical project root.
#[derive(Clone, Debug)]
pub struct ProjectSourceWorkspace {
    root: PathBuf,
}

impl ProjectSourceWorkspace {
    /// Open a project root for bounded source-file access.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, ProjectSourceError> {
        let root = fs::canonicalize(root.as_ref()).map_err(|error| ProjectSourceError::Io {
            path: String::new(),
            message: error.to_string(),
        })?;
        if !root.is_dir() {
            return Err(ProjectSourceError::InvalidRoot);
        }
        Ok(Self { root })
    }

    /// Return the canonical root this workspace is confined to.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// List accessible files in deterministic path order.
    ///
    /// File contents are read one at a time to compute their SHA-256
    /// revisions. The returned records contain metadata only.
    pub fn list_files(&self) -> Result<Vec<FileInfo>, ProjectSourceError> {
        let mut paths = Vec::new();
        let mut visited_directories = HashSet::new();
        self.collect_files(&self.root, &mut visited_directories, &mut paths)?;
        paths.sort();
        if paths.len() > MAX_PROJECT_SOURCE_FILES {
            return Err(ProjectSourceError::TooManyFiles {
                max_files: MAX_PROJECT_SOURCE_FILES,
            });
        }

        let mut files = Vec::with_capacity(paths.len());
        for path in paths {
            let target = self.existing_file_path(Path::new(&path), &path)?;
            let bytes = match read_limited(&target, &path, self) {
                Ok(bytes) => bytes,
                Err(ProjectSourceError::ContentTooLarge { .. }) => continue,
                Err(error) => return Err(error),
            };
            if std::str::from_utf8(&bytes).is_err() {
                continue;
            }
            files.push(FileInfo {
                path,
                revision: revision(&bytes),
                bytes: bytes.len() as u64,
            });
        }
        Ok(files)
    }

    /// Read one project-relative UTF-8 file and its content revision.
    pub fn read_file(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<FileSnapshot, ProjectSourceError> {
        let path = normalized_relative_path(relative_path.as_ref())?;
        self.ensure_allowed_path(&path)?;
        let target = self.existing_file_path(relative_path.as_ref(), &path)?;
        let bytes = read_limited(&target, &path, self)?;
        let content =
            String::from_utf8(bytes).map_err(|_| ProjectSourceError::InvalidUtf8(path.clone()))?;
        let revision = revision(content.as_bytes());
        Ok(FileSnapshot {
            path,
            revision,
            content,
        })
    }

    /// Create or replace one project-relative UTF-8 file.
    ///
    /// `expected_revision: None` is create-only. Replacements must include
    /// the revision returned by a prior read or listing.
    pub fn write_file(
        &self,
        relative_path: impl AsRef<Path>,
        content: &str,
        expected_revision: Option<&str>,
    ) -> Result<FileSnapshot, ProjectSourceError> {
        let requested = relative_path.as_ref();
        let path = normalized_relative_path(requested)?;
        self.ensure_allowed_path(&path)?;
        if content.len() > MAX_SOURCE_FILE_BYTES {
            return Err(ProjectSourceError::ContentTooLarge {
                path,
                max_bytes: MAX_SOURCE_FILE_BYTES,
            });
        }

        let (target, existed, original_permissions) = self.write_target(requested, &path)?;
        match (expected_revision, existed) {
            (None, true) => return Err(ProjectSourceError::AlreadyExists(path)),
            (Some(expected), false) => {
                return Err(ProjectSourceError::RevisionMismatch {
                    expected: expected.to_owned(),
                    actual: String::from("missing"),
                });
            }
            (Some(expected), true) => {
                let current = read_limited(&target, &path, self)?;
                let actual = revision(&current);
                if actual != expected {
                    return Err(ProjectSourceError::RevisionMismatch {
                        expected: expected.to_owned(),
                        actual,
                    });
                }
            }
            (None, false) => {}
        }

        self.atomic_replace(
            &target,
            &path,
            content.as_bytes(),
            original_permissions,
            expected_revision,
        )?;
        Ok(FileSnapshot {
            path,
            revision: revision(content.as_bytes()),
            content: content.to_owned(),
        })
    }

    /// Apply UTF-8 byte-range replacements against one expected file revision.
    pub fn apply_patch(
        &self,
        relative_path: impl AsRef<Path>,
        edits: &[PatchEdit],
        expected_revision: &str,
    ) -> Result<FileSnapshot, ProjectSourceError> {
        let path = relative_path.as_ref();
        let current = self.read_file(path)?;
        if current.revision != expected_revision {
            return Err(ProjectSourceError::RevisionMismatch {
                expected: expected_revision.to_owned(),
                actual: current.revision,
            });
        }

        let mut ordered = edits.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|edit| (edit.start, edit.end));
        let mut previous_start = None;
        let mut previous_end = 0;
        for edit in &ordered {
            if edit.start > edit.end
                || edit.end > current.content.len()
                || !current.content.is_char_boundary(edit.start)
                || !current.content.is_char_boundary(edit.end)
            {
                return Err(ProjectSourceError::InvalidPatch(format!(
                    "range {}..{} is outside the UTF-8 source boundaries",
                    edit.start, edit.end
                )));
            }
            if previous_start == Some(edit.start) || edit.start < previous_end {
                return Err(ProjectSourceError::InvalidPatch(
                    "edit ranges overlap or share an insertion point".to_owned(),
                ));
            }
            previous_start = Some(edit.start);
            previous_end = edit.end;
        }

        let replacement_bytes = ordered.iter().fold(0usize, |total, edit| {
            total.saturating_add(edit.replacement.len())
        });
        let removed_bytes: usize = ordered.iter().map(|edit| edit.end - edit.start).sum();
        let resulting_len = current
            .content
            .len()
            .saturating_sub(removed_bytes)
            .saturating_add(replacement_bytes);
        if resulting_len > MAX_SOURCE_FILE_BYTES {
            return Err(ProjectSourceError::ContentTooLarge {
                path: current.path,
                max_bytes: MAX_SOURCE_FILE_BYTES,
            });
        }

        let mut updated = current.content.clone();
        for edit in ordered.into_iter().rev() {
            updated.replace_range(edit.start..edit.end, &edit.replacement);
        }
        self.write_file(path, &updated, Some(expected_revision))
    }

    fn collect_files(
        &self,
        directory: &Path,
        visited_directories: &mut HashSet<PathBuf>,
        files: &mut Vec<String>,
    ) -> Result<(), ProjectSourceError> {
        let canonical_directory =
            fs::canonicalize(directory).map_err(|error| self.io_error("", error))?;
        if !canonical_directory.starts_with(&self.root) {
            return Ok(());
        }
        if !visited_directories.insert(canonical_directory.clone()) {
            return Ok(());
        }

        for entry in fs::read_dir(directory).map_err(|error| self.io_error("", error))? {
            let entry = entry.map_err(|error| self.io_error("", error))?;
            let entry_path = entry.path();
            let relative = entry_path
                .strip_prefix(&self.root)
                .map_err(|_| ProjectSourceError::SymlinkEscape(String::new()))?;
            let Some(relative_text) = relative.to_str() else {
                continue;
            };
            let relative_text = relative_text.replace(std::path::MAIN_SEPARATOR, "/");

            let metadata = fs::symlink_metadata(&entry_path)
                .map_err(|error| self.io_error(&relative_text, error))?;
            let actual_path = if metadata.file_type().is_symlink() {
                let Ok(actual_path) = fs::canonicalize(&entry_path) else {
                    continue;
                };
                if !actual_path.starts_with(&self.root) {
                    continue;
                }
                let actual_relative = actual_path
                    .strip_prefix(&self.root)
                    .unwrap_or(Path::new(""));
                if self.ensure_allowed_path(actual_relative).is_err() {
                    continue;
                }
                actual_path
            } else {
                entry_path.clone()
            };
            let actual_metadata =
                fs::metadata(&actual_path).map_err(|error| self.io_error(&relative_text, error))?;
            if actual_metadata.is_dir() {
                let actual_relative = actual_path
                    .strip_prefix(&self.root)
                    .unwrap_or(Path::new(""));
                if is_excluded_directory(&relative_text)
                    || is_excluded_directory(&path_to_slashes(actual_relative))
                {
                    continue;
                }
                self.collect_files(&entry_path, visited_directories, files)?;
            } else if actual_metadata.is_file() {
                if self.ensure_allowed_path(&relative_text).is_err() {
                    continue;
                }
                let actual_relative = actual_path
                    .strip_prefix(&self.root)
                    .unwrap_or(Path::new(""));
                if self.ensure_allowed_path(actual_relative).is_err() {
                    continue;
                }
                files.push(relative_text);
                if files.len() > MAX_PROJECT_SOURCE_FILES {
                    return Err(ProjectSourceError::TooManyFiles {
                        max_files: MAX_PROJECT_SOURCE_FILES,
                    });
                }
            }
        }
        Ok(())
    }

    fn ensure_allowed_path(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<(), ProjectSourceError> {
        let relative = normalized_relative_path(relative_path.as_ref())?;
        if is_excluded(&relative) {
            return Err(ProjectSourceError::ExcludedPath(relative));
        }
        Ok(())
    }

    fn existing_file_path(
        &self,
        requested: &Path,
        relative: &str,
    ) -> Result<PathBuf, ProjectSourceError> {
        let candidate = self.root.join(requested);
        let actual = fs::canonicalize(&candidate).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ProjectSourceError::NotFound(relative.to_owned())
            } else {
                self.io_error(relative, error)
            }
        })?;
        if !actual.starts_with(&self.root) {
            return Err(ProjectSourceError::SymlinkEscape(relative.to_owned()));
        }
        let actual_relative = actual
            .strip_prefix(&self.root)
            .map_err(|_| ProjectSourceError::SymlinkEscape(relative.to_owned()))?;
        self.ensure_allowed_path(actual_relative)?;
        if !actual.is_file() {
            return Err(ProjectSourceError::NotFile(relative.to_owned()));
        }
        Ok(actual)
    }

    fn write_target(
        &self,
        requested: &Path,
        relative: &str,
    ) -> Result<(PathBuf, bool, Option<fs::Permissions>), ProjectSourceError> {
        let Some(file_name) = requested.file_name() else {
            return Err(ProjectSourceError::InvalidPath(relative.to_owned()));
        };
        let parent_relative = requested.parent().unwrap_or_else(|| Path::new(""));
        let parent = fs::canonicalize(self.root.join(parent_relative)).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ProjectSourceError::NotFound(relative.to_owned())
            } else {
                self.io_error(relative, error)
            }
        })?;
        if !parent.starts_with(&self.root) {
            return Err(ProjectSourceError::SymlinkEscape(relative.to_owned()));
        }
        let parent_actual_relative = parent
            .strip_prefix(&self.root)
            .map_err(|_| ProjectSourceError::SymlinkEscape(relative.to_owned()))?;
        if !parent_actual_relative.as_os_str().is_empty() {
            self.ensure_allowed_path(parent_actual_relative)?;
        }
        if !parent.is_dir() {
            return Err(ProjectSourceError::NotFile(relative.to_owned()));
        }
        let candidate = parent.join(file_name);
        match fs::symlink_metadata(&candidate) {
            Ok(_) => {
                let target = self.existing_file_path(
                    &candidate.strip_prefix(&self.root).unwrap_or(&candidate),
                    relative,
                )?;
                let permissions = fs::metadata(&target)
                    .map_err(|error| self.io_error(relative, error))?
                    .permissions();
                Ok((target, true, Some(permissions)))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Ok((candidate, false, None))
            }
            Err(error) => Err(self.io_error(relative, error)),
        }
    }

    fn atomic_replace(
        &self,
        target: &Path,
        relative: &str,
        bytes: &[u8],
        permissions: Option<fs::Permissions>,
        expected_revision: Option<&str>,
    ) -> Result<(), ProjectSourceError> {
        let parent = target
            .parent()
            .ok_or_else(|| ProjectSourceError::InvalidPath(relative.to_owned()))?;
        let (temporary, mut file) = create_temporary_file(parent, relative, self)?;
        let result = (|| {
            file.write_all(bytes)
                .map_err(|error| self.io_error(relative, error))?;
            if let Some(permissions) = permissions {
                file.set_permissions(permissions)
                    .map_err(|error| self.io_error(relative, error))?;
            }
            file.sync_all()
                .map_err(|error| self.io_error(relative, error))?;
            drop(file);

            if let Some(expected) = expected_revision {
                let current = read_limited(target, relative, self)?;
                let actual = revision(&current);
                if actual != expected {
                    return Err(ProjectSourceError::RevisionMismatch {
                        expected: expected.to_owned(),
                        actual,
                    });
                }
            }

            if expected_revision.is_some() {
                fs::rename(&temporary, target).map_err(|error| self.io_error(relative, error))?;
            } else {
                fs::hard_link(&temporary, target).map_err(|error| {
                    if error.kind() == std::io::ErrorKind::AlreadyExists {
                        ProjectSourceError::AlreadyExists(relative.to_owned())
                    } else {
                        self.io_error(relative, error)
                    }
                })?;
                let _ = fs::remove_file(&temporary);
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    fn io_error(&self, path: &str, error: std::io::Error) -> ProjectSourceError {
        ProjectSourceError::Io {
            path: path.to_owned(),
            message: error.to_string(),
        }
    }
}

fn normalized_relative_path(path: &Path) -> Result<String, ProjectSourceError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(ProjectSourceError::InvalidPath(path.display().to_string()));
    }
    let raw = path
        .to_str()
        .ok_or_else(|| ProjectSourceError::InvalidPath(path.display().to_string()))?;
    if raw.contains('\\') || raw.contains(':') || raw.chars().any(char::is_control) {
        return Err(ProjectSourceError::InvalidPath(raw.to_owned()));
    }
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(name) => {
                let Some(name) = name.to_str() else {
                    return Err(ProjectSourceError::InvalidPath(raw.to_owned()));
                };
                if name.is_empty() || name == "." || name == ".." {
                    return Err(ProjectSourceError::InvalidPath(raw.to_owned()));
                }
                components.push(name);
            }
            _ => return Err(ProjectSourceError::InvalidPath(raw.to_owned())),
        }
    }
    if components.is_empty() {
        return Err(ProjectSourceError::InvalidPath(raw.to_owned()));
    }
    Ok(components.join("/"))
}

fn is_excluded(path: &str) -> bool {
    let mut components = path.split('/').peekable();
    while let Some(component) = components.next() {
        let is_directory = components.peek().is_some();
        if is_sensitive_path_component(component) {
            return true;
        }
        if is_directory
            && matches!(
                component,
                "build" | ".git" | "node_modules" | ".studio-dev" | "target"
            )
        {
            return true;
        }
        if !is_directory && is_generated_file(component) {
            return true;
        }
    }
    false
}

fn is_excluded_directory(path: &str) -> bool {
    path.split('/').any(|component| {
        is_sensitive_path_component(component)
            || matches!(
                component,
                "build" | ".git" | "node_modules" | ".studio-dev" | "target"
            )
    })
}

fn is_sensitive_path_component(component: &str) -> bool {
    let component = component.to_ascii_lowercase();
    if matches!(
        component.as_str(),
        ".env.example" | ".env.sample" | ".env.template"
    ) {
        return false;
    }
    component == ".env"
        || component.starts_with(".env.")
        || matches!(
            component.as_str(),
            ".npmrc"
                | ".netrc"
                | ".pypirc"
                | ".ssh"
                | ".aws"
                | ".secrets"
                | "secrets"
                | ".credentials"
                | "credentials"
                | "id_rsa"
                | "id_dsa"
                | "id_ecdsa"
                | "id_ed25519"
                | "id_ecdsa_sk"
                | "id_ed25519_sk"
        )
        || component.ends_with(".pem")
        || component.ends_with(".key")
        || component.ends_with(".p12")
        || component.ends_with(".pfx")
        || component.ends_with(".p8")
        || component.starts_with("service-account.")
}

fn path_to_slashes(path: &Path) -> String {
    path.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn is_generated_file(name: &str) -> bool {
    name.contains(".generated.")
        || name == "generated"
        || name.starts_with("generated.")
        || name.starts_with(".studio-write-")
}

fn revision(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(7 + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn read_limited(
    path: &Path,
    relative: &str,
    workspace: &ProjectSourceWorkspace,
) -> Result<Vec<u8>, ProjectSourceError> {
    let file = File::open(path).map_err(|error| workspace.io_error(relative, error))?;
    let mut bytes = Vec::new();
    file.take((MAX_SOURCE_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| workspace.io_error(relative, error))?;
    if bytes.len() > MAX_SOURCE_FILE_BYTES {
        return Err(ProjectSourceError::ContentTooLarge {
            path: relative.to_owned(),
            max_bytes: MAX_SOURCE_FILE_BYTES,
        });
    }
    Ok(bytes)
}

fn create_temporary_file(
    parent: &Path,
    relative: &str,
    workspace: &ProjectSourceWorkspace,
) -> Result<(PathBuf, File), ProjectSourceError> {
    for _ in 0..32 {
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".studio-write-{}-{sequence}.generated.tmp",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(workspace.io_error(relative, error)),
        }
    }
    Err(ProjectSourceError::Io {
        path: relative.to_owned(),
        message: "could not allocate a unique same-directory temporary file".to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> (tempfile::TempDir, ProjectSourceWorkspace) {
        let directory = tempfile::tempdir().expect("temporary project root");
        let workspace = ProjectSourceWorkspace::open(directory.path()).expect("open workspace");
        (directory, workspace)
    }

    #[test]
    fn create_replace_and_list_use_sha256_content_revisions() {
        let (directory, workspace) = workspace();
        fs::create_dir(directory.path().join("assembly")).unwrap();
        let created = workspace
            .write_file("assembly/index.ts", "export const value = 1;\n", None)
            .expect("create file");
        assert!(created.revision.starts_with("sha256:"));
        assert_eq!(created.content, "export const value = 1;\n");

        let listed = workspace.list_files().expect("list files");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].path, "assembly/index.ts");
        assert_eq!(listed[0].revision, created.revision);
        assert_eq!(listed[0].bytes, created.content.len() as u64);

        let replaced = workspace
            .write_file(
                "assembly/index.ts",
                "export const value = 2;\n",
                Some(&created.revision),
            )
            .expect("replace matching revision");
        assert_ne!(created.revision, replaced.revision);
        assert_eq!(
            fs::read_to_string(directory.path().join("assembly/index.ts")).unwrap(),
            replaced.content
        );
    }

    #[test]
    fn create_only_and_revision_preconditions_prevent_overwrites() {
        let (_directory, workspace) = workspace();
        let created = workspace
            .write_file("app.studio", "<Text>Hello</Text>", None)
            .unwrap();

        assert_eq!(
            workspace.write_file("app.studio", "overwrite", None),
            Err(ProjectSourceError::AlreadyExists("app.studio".to_owned()))
        );
        assert!(matches!(
            workspace.write_file("app.studio", "overwrite", Some("sha256:stale")),
            Err(ProjectSourceError::RevisionMismatch { .. })
        ));
        assert_eq!(
            workspace.read_file("app.studio").unwrap().revision,
            created.revision
        );
    }

    #[test]
    fn patches_apply_non_overlapping_utf8_byte_ranges_against_one_revision() {
        let (_directory, workspace) = workspace();
        let created = workspace.write_file("app.studio", "abcdef", None).unwrap();
        let updated = workspace
            .apply_patch(
                "app.studio",
                &[
                    PatchEdit::replace(4, 6, "YZ"),
                    PatchEdit::replace(1, 2, "X"),
                ],
                &created.revision,
            )
            .unwrap();
        assert_eq!(updated.content, "aXcdYZ");
    }

    #[test]
    fn patches_reject_overlapping_ranges_and_non_character_boundaries() {
        let (_directory, workspace) = workspace();
        let created = workspace.write_file("app.studio", "abcdef", None).unwrap();
        assert!(matches!(
            workspace.apply_patch(
                "app.studio",
                &[PatchEdit::replace(0, 3, "x"), PatchEdit::replace(2, 4, "y"),],
                &created.revision,
            ),
            Err(ProjectSourceError::InvalidPatch(_))
        ));

        let unicode = workspace.write_file("unicode.studio", "aéz", None).unwrap();
        assert!(matches!(
            workspace.apply_patch(
                "unicode.studio",
                &[PatchEdit::replace(2, 3, "x")],
                &unicode.revision,
            ),
            Err(ProjectSourceError::InvalidPatch(_))
        ));
    }

    #[test]
    fn traversal_absolute_and_generated_paths_are_rejected() {
        let (_directory, workspace) = workspace();
        for path in [
            "../outside.ts",
            "/etc/passwd",
            "routes.generated.ts",
            "build/app.ts",
        ] {
            assert!(workspace.read_file(path).is_err(), "accepted path {path}");
            assert!(workspace.write_file(path, "source", None).is_err());
        }
    }

    #[test]
    fn secret_files_and_directories_are_not_exposed_to_agent_tools() {
        let (directory, workspace) = workspace();
        fs::create_dir(directory.path().join("secrets")).unwrap();
        for path in [
            ".env",
            ".env.local",
            ".npmrc",
            "secrets/token.json",
            "tls/private.key",
            "service-account.json",
        ] {
            let file = directory.path().join(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(&file, "secret").unwrap();
            assert!(workspace.read_file(path).is_err(), "read allowed {path}");
            assert!(workspace.write_file(path, "changed", None).is_err());
        }
        fs::write(
            directory.path().join(".env.example"),
            "PUBLIC_VALUE=example\n",
        )
        .unwrap();
        fs::write(directory.path().join("app.studio"), "<Text/>\n").unwrap();

        let paths: Vec<_> = workspace
            .list_files()
            .unwrap()
            .into_iter()
            .map(|file| file.path)
            .collect();
        assert_eq!(paths, [".env.example", "app.studio"]);
    }

    #[test]
    fn listing_omits_excluded_directories_and_generated_files() {
        let (directory, workspace) = workspace();
        for excluded in ["build", ".git", "node_modules", ".studio-dev", "target"] {
            fs::create_dir_all(directory.path().join(excluded)).unwrap();
            fs::write(directory.path().join(excluded).join("hidden.ts"), "hidden").unwrap();
        }
        fs::write(directory.path().join("view.generated.ts"), "generated").unwrap();
        fs::write(directory.path().join("app.studio"), "<Text/> ").unwrap();

        let files = workspace.list_files().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "app.studio");
    }

    #[test]
    fn listing_skips_binary_assets_that_text_tools_cannot_edit() {
        let (directory, workspace) = workspace();
        fs::create_dir(directory.path().join("assets")).unwrap();
        fs::write(directory.path().join("assets/logo.png"), [0xff, 0xd8, 0xff]).unwrap();
        fs::write(directory.path().join("app.studio"), "<Text/>\n").unwrap();

        let files = workspace.list_files().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "app.studio");
        assert!(matches!(
            workspace.read_file("assets/logo.png"),
            Err(ProjectSourceError::InvalidUtf8(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_rejected_and_not_listed() {
        use std::os::unix::fs::symlink;

        let (directory, workspace) = workspace();
        let external = tempfile::NamedTempFile::new().unwrap();
        fs::write(external.path(), "outside source").unwrap();
        symlink(external.path(), directory.path().join("outside.ts")).unwrap();

        assert!(matches!(
            workspace.read_file("outside.ts"),
            Err(ProjectSourceError::SymlinkEscape(_))
        ));
        assert!(matches!(
            workspace.write_file("outside.ts", "replacement", Some("sha256:any")),
            Err(ProjectSourceError::SymlinkEscape(_))
        ));
        assert!(workspace.list_files().unwrap().is_empty());
    }

    #[test]
    fn source_file_size_is_bounded() {
        let (directory, workspace) = workspace();
        let content = "x".repeat(MAX_SOURCE_FILE_BYTES + 1);
        assert!(matches!(
            workspace.write_file("large.ts", &content, None),
            Err(ProjectSourceError::ContentTooLarge { .. })
        ));
        fs::write(directory.path().join("large.ts"), content).unwrap();
        assert!(matches!(
            workspace.read_file("large.ts"),
            Err(ProjectSourceError::ContentTooLarge { .. })
        ));
    }

    #[test]
    fn project_listing_is_bounded_by_file_count() {
        let (directory, workspace) = workspace();
        for index in 0..=MAX_PROJECT_SOURCE_FILES {
            fs::write(directory.path().join(format!("{index}.ts")), b"x").unwrap();
        }
        assert_eq!(
            workspace.list_files(),
            Err(ProjectSourceError::TooManyFiles {
                max_files: MAX_PROJECT_SOURCE_FILES,
            })
        );
    }
}
