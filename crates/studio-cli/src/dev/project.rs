//! Validated source graph declared by a project's optional `studio.project.json`.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::compiler::failure;
use crate::build::BuildFailure;

const PROJECT_FILE: &str = "studio.project.json";
const MAX_PROJECT_FILE_BYTES: u64 = 1024 * 1024;
const MAX_SCREENS: usize = 64;
const MAX_SCREEN_ID_BYTES: usize = 64;
const MAX_SOURCE_PATH_BYTES: usize = 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectDeclaration {
    schema_version: u32,
    screens: Vec<ScreenDeclaration>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScreenDeclaration {
    id: String,
    source: String,
}

/// One validated screen declaration with its exact project-relative source path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ScreenEntry {
    pub id: String,
    pub source_path: String,
    pub source: String,
}

/// Load and validate the optional project declaration.
///
/// A missing file returns `Ok(None)`, allowing the caller to use the legacy
/// single-entry or AssemblyScript discovery behavior. A present but invalid
/// declaration fails closed.
pub(super) fn load_screen_entries(
    project_root: &Path,
) -> Result<Option<Vec<ScreenEntry>>, BuildFailure> {
    let root = project_root
        .canonicalize()
        .map_err(|error| failure(format!("project root {}: {error}", project_root.display())))?;
    let declaration_path = root.join(PROJECT_FILE);
    match fs::symlink_metadata(&declaration_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(failure(format!("{}: {error}", declaration_path.display())));
        }
        Ok(_) => {}
    }

    let canonical_declaration = declaration_path
        .canonicalize()
        .map_err(|error| failure(format!("{}: {error}", declaration_path.display())))?;
    if !canonical_declaration.starts_with(&root) || !canonical_declaration.is_file() {
        return Err(failure(format!(
            "{PROJECT_FILE} must be a regular file inside the project root"
        )));
    }
    let metadata = fs::metadata(&canonical_declaration)
        .map_err(|error| failure(format!("{}: {error}", canonical_declaration.display())))?;
    if metadata.len() > MAX_PROJECT_FILE_BYTES {
        return Err(failure(format!(
            "{PROJECT_FILE} exceeds the {MAX_PROJECT_FILE_BYTES}-byte limit"
        )));
    }
    let bytes = fs::read(&canonical_declaration)
        .map_err(|error| failure(format!("{}: {error}", canonical_declaration.display())))?;
    let declaration: ProjectDeclaration = serde_json::from_slice(&bytes)
        .map_err(|error| failure(format!("{PROJECT_FILE} contains invalid JSON: {error}")))?;
    if declaration.schema_version != 1 {
        return Err(failure(format!("{PROJECT_FILE} schemaVersion must be 1")));
    }
    if declaration.screens.is_empty() || declaration.screens.len() > MAX_SCREENS {
        return Err(failure(format!(
            "{PROJECT_FILE} must declare between 1 and {MAX_SCREENS} screens"
        )));
    }

    let mut ids = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut entries = Vec::with_capacity(declaration.screens.len());
    for screen in declaration.screens {
        if !is_screen_id(&screen.id) || !ids.insert(screen.id.clone()) {
            return Err(failure(format!(
                "{PROJECT_FILE} screen IDs must be unique lowercase slugs: `{}`",
                screen.id
            )));
        }
        validate_source_path(&screen.source)?;

        let unresolved_source = root.join(&screen.source);
        let canonical_source = unresolved_source.canonicalize().map_err(|error| {
            failure(format!(
                "{PROJECT_FILE} screen `{}` source `{}`: {error}",
                screen.id, screen.source
            ))
        })?;
        if !canonical_source.starts_with(&root) {
            return Err(failure(format!(
                "{PROJECT_FILE} screen `{}` source escapes the project root",
                screen.id
            )));
        }
        let metadata = fs::metadata(&canonical_source).map_err(|error| {
            failure(format!(
                "{PROJECT_FILE} screen `{}` source `{}`: {error}",
                screen.id, screen.source
            ))
        })?;
        if !metadata.is_file() {
            return Err(failure(format!(
                "{PROJECT_FILE} screen `{}` source is not a regular file",
                screen.id
            )));
        }
        let max_bytes = studio_script::DEFAULT_MAX_SOURCE_BYTES as u64;
        if metadata.len() > max_bytes {
            return Err(failure(format!(
                "{PROJECT_FILE} screen `{}` source exceeds the {max_bytes}-byte Studio source limit",
                screen.id
            )));
        }
        if !sources.insert(canonical_source.clone()) {
            return Err(failure(format!(
                "{PROJECT_FILE} declares the same source file more than once: `{}`",
                screen.source
            )));
        }
        let source_bytes = fs::read(&canonical_source).map_err(|error| {
            failure(format!(
                "{PROJECT_FILE} screen `{}` source `{}`: {error}",
                screen.id, screen.source
            ))
        })?;
        if source_bytes.len() > studio_script::DEFAULT_MAX_SOURCE_BYTES {
            return Err(failure(format!(
                "{PROJECT_FILE} screen `{}` source exceeds the {max_bytes}-byte Studio source limit",
                screen.id
            )));
        }
        let source = String::from_utf8(source_bytes).map_err(|error| {
            failure(format!(
                "{PROJECT_FILE} screen `{}` source `{}` is not valid UTF-8: {error}",
                screen.id, screen.source
            ))
        })?;
        entries.push(ScreenEntry {
            id: screen.id,
            source_path: screen.source,
            source,
        });
    }
    Ok(Some(entries))
}

fn is_screen_id(id: &str) -> bool {
    if id.is_empty() || id.len() > MAX_SCREEN_ID_BYTES {
        return false;
    }
    let mut characters = id.bytes();
    matches!(characters.next(), Some(b'a'..=b'z' | b'0'..=b'9'))
        && characters.all(|character| matches!(character, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_'))
}

fn validate_source_path(source: &str) -> Result<(), BuildFailure> {
    let invalid = source.is_empty()
        || source.len() > MAX_SOURCE_PATH_BYTES
        || source.starts_with('/')
        || source.contains('\\')
        || source.contains(':')
        || source.chars().any(char::is_control)
        || source
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
        || Path::new(source)
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("studio");
    if invalid {
        return Err(failure(format!(
            "{PROJECT_FILE} source paths must be normalized root-relative `.studio` paths: `{source}`"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_project(root: &Path, body: &str) {
        fs::write(root.join(PROJECT_FILE), body).unwrap();
    }

    #[test]
    fn parses_ordered_screen_declarations_and_retains_relative_paths() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("screens")).unwrap();
        fs::write(
            directory.path().join("screens/home.studio"),
            "<Text id=\"home-title\">Home</Text>",
        )
        .unwrap();
        fs::write(
            directory.path().join("screens/settings.studio"),
            "<Text id=\"settings-title\">Settings</Text>",
        )
        .unwrap();
        write_project(
            directory.path(),
            r#"{"schemaVersion":1,"screens":[{"id":"home","source":"screens/home.studio"},{"id":"settings","source":"screens/settings.studio"}]}"#,
        );

        let entries = load_screen_entries(directory.path()).unwrap().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "home");
        assert_eq!(entries[0].source_path, "screens/home.studio");
        assert!(directory.path().join(&entries[0].source_path).is_file());
        assert_eq!(entries[0].source, "<Text id=\"home-title\">Home</Text>");
        assert_eq!(entries[1].id, "settings");
    }

    #[test]
    fn absent_declaration_keeps_legacy_entry_discovery_available() {
        let directory = tempfile::tempdir().unwrap();
        assert!(load_screen_entries(directory.path()).unwrap().is_none());
    }

    #[test]
    fn rejects_invalid_schema_empty_screens_and_screen_count_overflow() {
        let directory = tempfile::tempdir().unwrap();
        write_project(directory.path(), r#"{"schemaVersion":2,"screens":[]}"#);
        assert!(load_screen_entries(directory.path()).is_err());

        write_project(
            directory.path(),
            r#"{"schemaVersion":1,"screens":[],"unexpected":true}"#,
        );
        assert!(load_screen_entries(directory.path()).is_err());

        write_project(directory.path(), r#"{"schemaVersion":1,"screens":[]}"#);
        assert!(load_screen_entries(directory.path()).is_err());

        let screens = (0..=MAX_SCREENS)
            .map(|index| {
                format!("{{\"id\":\"screen-{index}\",\"source\":\"screen-{index}.studio\"}}")
            })
            .collect::<Vec<_>>()
            .join(",");
        write_project(
            directory.path(),
            &format!("{{\"schemaVersion\":1,\"screens\":[{screens}]}}"),
        );
        assert!(load_screen_entries(directory.path()).is_err());
    }

    #[test]
    fn rejects_oversized_project_declarations_before_parsing() {
        let directory = tempfile::tempdir().unwrap();
        write_project(
            directory.path(),
            &" ".repeat(MAX_PROJECT_FILE_BYTES as usize + 1),
        );
        assert!(load_screen_entries(directory.path()).is_err());
    }

    #[test]
    fn rejects_duplicate_or_non_slug_ids_and_duplicate_sources() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("home.studio"),
            "<Text id=\"x\">x</Text>",
        )
        .unwrap();
        let declarations = [
            r#"{"schemaVersion":1,"screens":[{"id":"home","source":"home.studio"},{"id":"home","source":"home.studio"}]}"#.to_owned(),
            r#"{"schemaVersion":1,"screens":[{"id":"Home","source":"home.studio"}]}"#.to_owned(),
            format!(
                r#"{{"schemaVersion":1,"screens":[{{"id":"{}","source":"home.studio"}}]}}"#,
                "a".repeat(MAX_SCREEN_ID_BYTES + 1)
            ),
            r#"{"schemaVersion":1,"screens":[{"id":"home","source":"home.studio"},{"id":"other","source":"home.studio"}]}"#.to_owned(),
        ];
        for body in declarations {
            write_project(directory.path(), &body);
            assert!(load_screen_entries(directory.path()).is_err());
        }
    }

    #[test]
    fn rejects_unnormalized_absolute_and_non_studio_paths() {
        let directory = tempfile::tempdir().unwrap();
        let paths = [
            "../outside.studio".to_owned(),
            "/absolute.studio".to_owned(),
            "screens//home.studio".to_owned(),
            "screens/./home.studio".to_owned(),
            "screens\\home.studio".to_owned(),
            "home.ts".to_owned(),
            format!("{}.studio", "a".repeat(MAX_SOURCE_PATH_BYTES)),
        ];
        for source in paths {
            write_project(
                directory.path(),
                &format!(
                    "{{\"schemaVersion\":1,\"screens\":[{{\"id\":\"home\",\"source\":{}}}]}}",
                    serde_json::to_string(&source).unwrap()
                ),
            );
            assert!(load_screen_entries(directory.path()).is_err(), "{source}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape_oversized_source_and_invalid_utf8() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_source = outside.path().join("secret.studio");
        fs::write(&outside_source, "<Text id=\"secret\">secret</Text>").unwrap();
        symlink(&outside_source, directory.path().join("escape.studio")).unwrap();
        write_project(
            directory.path(),
            r#"{"schemaVersion":1,"screens":[{"id":"home","source":"escape.studio"}]}"#,
        );
        assert!(load_screen_entries(directory.path()).is_err());

        let too_large = "x".repeat(studio_script::DEFAULT_MAX_SOURCE_BYTES + 1);
        fs::write(directory.path().join("large.studio"), too_large).unwrap();
        write_project(
            directory.path(),
            r#"{"schemaVersion":1,"screens":[{"id":"home","source":"large.studio"}]}"#,
        );
        assert!(load_screen_entries(directory.path()).is_err());

        fs::write(directory.path().join("invalid.studio"), [0xff, 0xfe]).unwrap();
        write_project(
            directory.path(),
            r#"{"schemaVersion":1,"screens":[{"id":"home","source":"invalid.studio"}]}"#,
        );
        assert!(load_screen_entries(directory.path()).is_err());
    }
}
