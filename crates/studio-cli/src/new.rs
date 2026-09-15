//! `studio new`: scaffold modern-Script projects from the template gallery.
//!
//! Templates are data, not code: `templates/<name>/` carries `template.json`
//! plus project files, shared with the designer IDE's new-project flow.
//! Remote templates arrive over HTTPS (GitHub shorthand, git URLs, direct
//! `.zip`/`.studio` links) and verify identically to local ones before any
//! file lands in the destination.

use std::path::{Path, PathBuf};

use crate::build::{BuildDiagnostic, BuildFailure, BuildPhase};

/// Template kinds: sandboxes for learning, or working bases to extend.
pub const KIND_PLAYGROUND: &str = "playground";
pub const KIND_LAYOUT: &str = "layout";

/// Gallery entry descriptor.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct TemplateMeta {
    /// Gallery id (also the directory name for local templates).
    pub id: String,
    /// Display name.
    pub name: String,
    /// One-line description for `--list-templates`.
    pub description: String,
    /// `playground` or `layout`.
    pub kind: String,
}

/// Repository template gallery root.
#[must_use]
pub fn gallery_root() -> PathBuf {
    crate::build::repo_root().join("templates")
}

/// List gallery templates in directory order.
pub fn list_templates() -> Result<Vec<(String, TemplateMeta)>, BuildFailure> {
    let root = gallery_root();
    let mut entries: Vec<(String, TemplateMeta)> = Vec::new();
    let read = std::fs::read_dir(&root).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Io,
            vec![BuildDiagnostic::error(
                BuildPhase::Io,
                "BUILD_NEW_GALLERY_MISSING",
                format!("read template gallery {}: {error}", root.display()),
            )],
        )
    })?;
    for entry in read.filter_map(Result::ok) {
        let manifest = entry.path().join("template.json");
        if !manifest.is_file() {
            continue;
        }
        let bytes = std::fs::read(&manifest).map_err(|error| io_failure(&manifest, &error))?;
        let meta: TemplateMeta = serde_json::from_slice(&bytes).map_err(|error| {
            BuildFailure::new(
                BuildPhase::Usage,
                vec![BuildDiagnostic::error(
                    BuildPhase::Usage,
                    "BUILD_NEW_TEMPLATE_INVALID",
                    format!("parse {}: {error}", manifest.display()),
                )],
            )
        })?;
        entries.push((entry.file_name().to_string_lossy().to_string(), meta));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(entries)
}

/// Validate a project name: lowercase alphanumeric plus interior `-`,
/// starting and ending alphanumeric, at most 64 bytes.
#[must_use]
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && name
            .bytes()
            .next_back()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// Scaffold a project: resolve the template, expand into a staging
/// directory, self-check, then publish to the destination.
///
/// # Errors
///
/// Returns usage failures for bad names, occupied destinations, unknown
/// templates, and remote errors; validation failures for templates that
/// do not check clean.
pub fn run(name: &str, template: &str, force: bool) -> Result<PathBuf, BuildFailure> {
    if !valid_name(name) {
        return Err(BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_NEW_NAME_INVALID",
                format!("invalid project name {name:?}: use lowercase alphanumeric plus `-`"),
            )],
        ));
    }
    let destination = std::env::current_dir()
        .map_err(|error| {
            BuildFailure::new(
                BuildPhase::Io,
                vec![BuildDiagnostic::error(
                    BuildPhase::Io,
                    "BUILD_NEW_CWD_MISSING",
                    format!("resolve working directory: {error}"),
                )],
            )
        })?
        .join(name);
    if destination.exists() {
        let occupied =
            std::fs::read_dir(&destination).map_or(true, |mut read| read.next().is_some());
        if occupied && !force {
            return Err(BuildFailure::new(
                BuildPhase::Usage,
                vec![BuildDiagnostic::error(
                    BuildPhase::Usage,
                    "BUILD_NEW_DESTINATION_OCCUPIED",
                    format!(
                        "destination {} exists and is not empty (use --force)",
                        destination.display()
                    ),
                )],
            ));
        }
        if force {
            std::fs::remove_dir_all(&destination).map_err(|error| {
                BuildFailure::new(
                    BuildPhase::Io,
                    vec![BuildDiagnostic::error(
                        BuildPhase::Io,
                        "BUILD_NEW_DESTINATION_UNWRITABLE",
                        format!("clear {}: {error}", destination.display()),
                    )],
                )
            })?;
        }
    }
    let staging = destination.with_extension("studio-new-tmp");
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|error| {
            BuildFailure::new(
                BuildPhase::Io,
                vec![BuildDiagnostic::error(
                    BuildPhase::Io,
                    "BUILD_NEW_DESTINATION_UNWRITABLE",
                    format!("clear {}: {error}", staging.display()),
                )],
            )
        })?;
    }
    let outcome = expand_template(template, &staging)
        .and_then(|()| rewrite_manifest(&staging, name).and_then(|()| self_check(&staging)));
    match outcome {
        Ok(()) => {
            std::fs::rename(&staging, &destination).map_err(|error| {
                BuildFailure::new(
                    BuildPhase::Io,
                    vec![BuildDiagnostic::error(
                        BuildPhase::Io,
                        "BUILD_NEW_DESTINATION_UNWRITABLE",
                        format!("publish {}: {error}", destination.display()),
                    )],
                )
            })?;
            Ok(destination)
        }
        Err(failure) => {
            let _ = std::fs::remove_dir_all(&staging);
            Err(failure)
        }
    }
}

/// Resolve a template reference into a staged tree.
fn expand_template(template: &str, staging: &Path) -> Result<(), BuildFailure> {
    if let Some(local) = gallery_template(template) {
        copy_template_tree(&local, staging, true)?;
        verify_template_id(staging, Some(template))?;
        return Ok(());
    }
    if template.contains("://") {
        fetch_remote_template(template, staging)?;
    } else if is_github_shorthand(template) {
        fetch_github_template(template, staging)?;
    } else {
        return Err(BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_NEW_TEMPLATE_UNKNOWN",
                format!("unknown template {template:?}: use `studio new --list-templates`"),
            )],
        ));
    }
    verify_template_id(staging, None)?;
    Ok(())
}

/// Gallery directory for a template name, if present with a manifest.
fn gallery_template(name: &str) -> Option<PathBuf> {
    let candidate = gallery_root().join(name);
    if candidate.join("template.json").is_file() {
        Some(candidate)
    } else {
        None
    }
}

/// Copy a template tree, skipping the gallery manifest itself when asked.
fn copy_template_tree(
    source: &Path,
    staging: &Path,
    skip_manifest: bool,
) -> Result<(), BuildFailure> {
    std::fs::create_dir_all(staging).map_err(|error| io_failure(staging, &error))?;
    let mut entries: Vec<PathBuf> = std::fs::read_dir(source)
        .map_err(|error| io_failure(source, &error))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().map(|name| name.to_string_lossy());
        if skip_manifest && name.as_deref() == Some("template.json") {
            continue;
        }
        let destination = staging.join(path.file_name().expect("entry has a name"));
        if path.is_dir() {
            copy_template_tree(&path, &destination, false)?;
        } else {
            std::fs::copy(&path, &destination).map_err(|error| io_failure(&path, &error))?;
        }
    }
    Ok(())
}

/// Verify the staged tree carries a valid template manifest.
fn verify_template_id(staging: &Path, expected: Option<&str>) -> Result<(), BuildFailure> {
    let manifest = staging.join("template.json");
    // Gallery trees skip their manifest on copy; remote trees must carry one.
    if expected.is_some() {
        return Ok(());
    }
    let bytes = std::fs::read(&manifest).map_err(|_| {
        BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_NEW_TEMPLATE_INVALID",
                "remote template has no template.json at its root".to_owned(),
            )],
        )
    })?;
    let meta: TemplateMeta = serde_json::from_slice(&bytes).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_NEW_TEMPLATE_INVALID",
                format!("parse remote template.json: {error}"),
            )],
        )
    })?;
    if meta.id.trim().is_empty() {
        return Err(BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_NEW_TEMPLATE_INVALID",
                "remote template.json has no id".to_owned(),
            )],
        ));
    }
    // The manifest is gallery metadata, not project content.
    let _ = std::fs::remove_file(&manifest);
    Ok(())
}

/// Rewrite the staged manifest identity for the new project.
fn rewrite_manifest(staging: &Path, name: &str) -> Result<(), BuildFailure> {
    let path = staging.join("manifest.json");
    let bytes = std::fs::read(&path).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_NEW_TEMPLATE_INVALID",
                format!("template has no manifest.json: {error}"),
            )],
        )
    })?;
    let mut manifest: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_NEW_TEMPLATE_INVALID",
                format!("parse template manifest.json: {error}"),
            )],
        )
    })?;
    manifest["id"] = serde_json::Value::String(format!("com.studio.{}", name.replace('-', ".")));
    manifest["name"] = serde_json::Value::String(pretty_name(name));
    manifest["version"] = serde_json::Value::String("0.1.0".to_owned());
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&manifest).expect("manifest serializes"),
    )
    .map_err(|error| io_failure(&path, &error))?;
    Ok(())
}

/// Human display name from a project slug.
fn pretty_name(name: &str) -> String {
    name.split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Compile the staged entry through the real pipeline: templates that do
/// not lower are never published.
fn self_check(staging: &Path) -> Result<(), BuildFailure> {
    let entry = staging.join("app.studio");
    let source = std::fs::read_to_string(&entry).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_NEW_TEMPLATE_INVALID",
                format!("template has no app.studio: {error}"),
            )],
        )
    })?;
    studio_script::compile_studio(&source, "app.studio", "app").map_err(|diagnostics| {
        BuildFailure::new(
            BuildPhase::Validate,
            diagnostics
                .into_iter()
                .map(|diagnostic| {
                    let mut record = BuildDiagnostic::error(
                        BuildPhase::Validate,
                        diagnostic.code,
                        diagnostic.message,
                    );
                    record.line =
                        Some(u64::try_from(diagnostic.span.start.line).unwrap_or(u64::MAX));
                    record.column =
                        Some(u64::try_from(diagnostic.span.start.column).unwrap_or(u64::MAX));
                    record
                })
                .collect(),
        )
    })?;
    Ok(())
}

/// Whether a template reference is `owner/repo` shorthand.
fn is_github_shorthand(template: &str) -> bool {
    let mut parts = template.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(repo), None) => {
            !owner.is_empty()
                && !repo.is_empty()
                && owner
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
                && repo
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        }
        _ => false,
    }
}

/// Fetch a GitHub shorthand template through codeload zips (main, then
/// master), without needing a git binary.
fn fetch_github_template(shorthand: &str, staging: &Path) -> Result<(), BuildFailure> {
    let mut failure = None;
    for branch in ["main", "master"] {
        let url = format!("https://codeload.github.com/{shorthand}/zip/refs/heads/{branch}");
        match download_bytes(&url) {
            Ok(bytes) => {
                return unpack_zip(&bytes, staging);
            }
            Err(error) => {
                failure = Some(error);
            }
        }
    }
    Err(failure.expect("branch loop always fails or returns"))
}

/// Fetch a direct remote template: git URLs clone, `.zip` links unpack,
/// `.studio` links expand as template bundles.
fn fetch_remote_template(url: &str, staging: &Path) -> Result<(), BuildFailure> {
    if url.starts_with("git@") || url.ends_with(".git") {
        return clone_git_template(url, staging);
    }
    if let Some(https) = url.strip_prefix("https://") {
        // Bare `github.com/owner/repo` links behave like shorthand.
        if let Some(path) = https.strip_prefix("github.com/") {
            let shorthand = path.trim_end_matches(".git").trim_end_matches('/');
            if is_github_shorthand(shorthand) {
                return fetch_github_template(shorthand, staging);
            }
        }
    }
    if !(url.starts_with("https://") || is_loopback_http(url)) {
        return Err(BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_NEW_REMOTE_REJECTED",
                "remote templates require https (http is allowed for loopback only)".to_owned(),
            )],
        ));
    }
    let bytes = download_bytes(url)?;
    if url.ends_with(".zip") {
        unpack_zip(&bytes, staging)
    } else if url.ends_with(".studio") {
        unpack_template_bundle(&bytes, staging)
    } else {
        Err(BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_NEW_REMOTE_REJECTED",
                "direct links must end in .zip or .studio".to_owned(),
            )],
        ))
    }
}

/// Plain-HTTP allowance for loopback test and template servers only.
fn is_loopback_http(url: &str) -> bool {
    url.starts_with("http://127.0.0.1:")
        || url.starts_with("http://localhost:")
        || url.starts_with("http://[::1]:")
}

/// Download URL bytes over HTTPS through `curl`, failing closed.
fn download_bytes(url: &str) -> Result<Vec<u8>, BuildFailure> {
    let scratch = std::env::temp_dir().join(format!("studio-new-{}.tmp", std::process::id()));
    // Plain HTTP only leaves the box for loopback template servers;
    // downgrades anywhere else fail closed.
    let protocols = if is_loopback_http(url) {
        "http,https"
    } else {
        "https"
    };
    let status = std::process::Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--proto",
            protocols,
            "--proto-redir",
            protocols,
            "--max-time",
            "90",
            "--output",
        ])
        .arg(&scratch)
        .arg(url)
        .status()
        .map_err(|error| {
            BuildFailure::new(
                BuildPhase::Usage,
                vec![BuildDiagnostic::error(
                    BuildPhase::Usage,
                    "BUILD_NEW_REMOTE_UNAVAILABLE",
                    format!("remote fetch needs curl: {error}"),
                )],
            )
        })?;
    if !status.success() {
        let _ = std::fs::remove_file(&scratch);
        return Err(BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_NEW_REMOTE_UNAVAILABLE",
                format!("download {url} failed"),
            )],
        ));
    }
    let bytes = std::fs::read(&scratch).map_err(|error| io_failure(&scratch, &error))?;
    let _ = std::fs::remove_file(&scratch);
    Ok(bytes)
}

/// Clone a git template shallowly.
fn clone_git_template(url: &str, staging: &Path) -> Result<(), BuildFailure> {
    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "1", url])
        .arg(staging)
        .status()
        .map_err(|error| {
            BuildFailure::new(
                BuildPhase::Usage,
                vec![BuildDiagnostic::error(
                    BuildPhase::Usage,
                    "BUILD_NEW_REMOTE_UNAVAILABLE",
                    format!("git template clone needs git: {error}"),
                )],
            )
        })?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(staging);
        return Err(BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_NEW_REMOTE_UNAVAILABLE",
                format!("clone {url} failed"),
            )],
        ));
    }
    let _ = std::fs::remove_dir_all(staging.join(".git"));
    Ok(())
}

/// Unpack a template zip, collapsing a single top-level directory.
fn unpack_zip(bytes: &[u8], staging: &Path) -> Result<(), BuildFailure> {
    use std::io::Read as _;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_NEW_REMOTE_INVALID",
                format!("remote template is not a zip: {error}"),
            )],
        )
    })?;
    let mut names: Vec<String> = Vec::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index).map_err(|error| {
            BuildFailure::new(
                BuildPhase::Usage,
                vec![BuildDiagnostic::error(
                    BuildPhase::Usage,
                    "BUILD_NEW_REMOTE_INVALID",
                    format!("read remote template entry: {error}"),
                )],
            )
        })?;
        if let Some(name) = file.enclosed_name() {
            names.push(name.to_string_lossy().to_string());
        }
    }
    let prefix = single_top_dir(&names);
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| {
            BuildFailure::new(
                BuildPhase::Usage,
                vec![BuildDiagnostic::error(
                    BuildPhase::Usage,
                    "BUILD_NEW_REMOTE_INVALID",
                    format!("read remote template entry: {error}"),
                )],
            )
        })?;
        let Some(safe) = file.enclosed_name() else {
            continue;
        };
        let relative = match &prefix {
            Some(prefix) => match safe.strip_prefix(prefix) {
                Ok(rest) if !rest.as_os_str().is_empty() => rest.to_path_buf(),
                _ => continue,
            },
            None => safe.to_path_buf(),
        };
        let destination = staging.join(&relative);
        if file.is_dir() {
            std::fs::create_dir_all(&destination)
                .map_err(|error| io_failure(&destination, &error))?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| io_failure(parent, &error))?;
        }
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).map_err(|error| {
            BuildFailure::new(
                BuildPhase::Usage,
                vec![BuildDiagnostic::error(
                    BuildPhase::Usage,
                    "BUILD_NEW_REMOTE_INVALID",
                    format!("read remote template entry: {error}"),
                )],
            )
        })?;
        std::fs::write(&destination, contents).map_err(|error| io_failure(&destination, &error))?;
    }
    Ok(())
}

/// Expand a `.studio` template bundle: manifest, entry source, and assets.
/// Compiled modules never ship into the new tree; the first build emits them.
fn unpack_template_bundle(bytes: &[u8], staging: &Path) -> Result<(), BuildFailure> {
    use std::io::Read as _;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|error| {
        BuildFailure::new(
            BuildPhase::Usage,
            vec![BuildDiagnostic::error(
                BuildPhase::Usage,
                "BUILD_NEW_REMOTE_INVALID",
                format!("remote template is not a bundle: {error}"),
            )],
        )
    })?;
    let mut names = Vec::new();
    for index in 0..archive.len() {
        if let Ok(file) = archive.by_index(index) {
            if let Some(name) = file.enclosed_name() {
                names.push(name.to_string_lossy().to_string());
            }
        }
    }
    for required in ["template.json", "manifest.json", "app.studio"] {
        if !names.iter().any(|name| name == required) {
            return Err(BuildFailure::new(
                BuildPhase::Usage,
                vec![BuildDiagnostic::error(
                    BuildPhase::Usage,
                    "BUILD_NEW_REMOTE_INVALID",
                    format!("template bundle has no {required}"),
                )],
            ));
        }
    }
    std::fs::create_dir_all(staging).map_err(|error| io_failure(staging, &error))?;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| {
            BuildFailure::new(
                BuildPhase::Usage,
                vec![BuildDiagnostic::error(
                    BuildPhase::Usage,
                    "BUILD_NEW_REMOTE_INVALID",
                    format!("read template bundle entry: {error}"),
                )],
            )
        })?;
        let Some(safe) = file.enclosed_name() else {
            continue;
        };
        let relative = safe.to_string_lossy().to_string();
        if relative.ends_with(".wasm") || relative.starts_with("build/") {
            continue;
        }
        let destination = staging.join(safe);
        if file.is_dir() {
            std::fs::create_dir_all(&destination)
                .map_err(|error| io_failure(&destination, &error))?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| io_failure(parent, &error))?;
        }
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).map_err(|error| {
            BuildFailure::new(
                BuildPhase::Usage,
                vec![BuildDiagnostic::error(
                    BuildPhase::Usage,
                    "BUILD_NEW_REMOTE_INVALID",
                    format!("read template bundle entry: {error}"),
                )],
            )
        })?;
        std::fs::write(&destination, contents).map_err(|error| io_failure(&destination, &error))?;
    }
    Ok(())
}

/// Single shared top-level directory, if every entry lives under one.
fn single_top_dir(names: &[String]) -> Option<PathBuf> {
    let mut tops: Vec<&str> = Vec::new();
    for name in names {
        let top = name.split('/').next().unwrap_or_default();
        if top.is_empty() {
            continue;
        }
        if !tops.contains(&top) {
            tops.push(top);
        }
    }
    if tops.len() == 1 && names.iter().any(|name| name.contains('/')) {
        Some(PathBuf::from(tops[0]))
    } else {
        None
    }
}

fn io_failure(path: &Path, error: &dyn std::fmt::Display) -> BuildFailure {
    BuildFailure::new(
        BuildPhase::Io,
        vec![BuildDiagnostic::error(
            BuildPhase::Io,
            "BUILD_NEW_IO",
            format!("{}: {error}", path.display()),
        )],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_accept_slugs_and_reject_the_rest() {
        for valid in ["shop", "pos-clothing-store", "a1", "x-9-y"] {
            assert!(valid_name(valid), "{valid} is a valid name");
        }
        for invalid in [
            "",
            "Bad Name",
            "UPPER",
            "-lead",
            "trail-",
            "under_score",
            "with.dot",
            "with/slash",
            &"x".repeat(65),
        ] {
            assert!(!valid_name(invalid), "{invalid} is rejected");
        }
    }

    #[test]
    fn shorthand_needs_owner_and_repo() {
        assert!(is_github_shorthand("owner/repo"));
        assert!(is_github_shorthand("my-org/my.repo_2"));
        for invalid in [
            "",
            "noslash",
            "a/b/c",
            "/repo",
            "owner/",
            "owner/repo/extra",
        ] {
            assert!(!is_github_shorthand(invalid), "{invalid} is not shorthand");
        }
    }

    #[test]
    fn loopback_http_covers_local_servers() {
        assert!(is_loopback_http("http://127.0.0.1:8080/t.zip"));
        assert!(is_loopback_http("http://localhost:3000/t.zip"));
        assert!(is_loopback_http("http://[::1]:9000/t.zip"));
        assert!(!is_loopback_http("https://example.com/t.zip"));
        assert!(!is_loopback_http("http://example.com/t.zip"));
        assert!(!is_loopback_http("http://192.168.1.2/t.zip"));
    }

    #[test]
    fn pretty_names_title_case_words() {
        assert_eq!(pretty_name("pos-clothing-store"), "Pos Clothing Store");
        assert_eq!(pretty_name("shop"), "Shop");
    }
}
