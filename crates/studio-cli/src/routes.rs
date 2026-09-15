//! File-based route registry: scan `routes/` into entries, validate shapes,
//! render the deterministic module, and match paths with one shared semantic.
//!
//! File conventions: `index.ts` → `/`, nesting joins segments, `[name]` →
//! `:name` parameters, `[...name]` → trailing wildcard, leading-`_` files are
//! helper modules (never routes). Titles come from `export const title`
//! string literals, defaulting to the humanized last segment.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Route shape class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteKind {
    /// No parameters or wildcards.
    Static,
    /// At least one `:param` segment.
    Param,
    /// Trailing wildcard.
    Wildcard,
}

/// One registry entry.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteEntry {
    /// Path pattern (for example `/orders/:id`).
    pub path: String,
    /// Segments in order: static text, `:param`, or trailing `*`.
    pub segments: Vec<String>,
    /// Parameter names in order.
    pub params: Vec<String>,
    /// Title from the `title` export or the default.
    pub title: String,
    /// Project-relative source file.
    pub file: String,
    /// Shape class.
    pub kind: RouteKind,
}

/// A registry diagnostic with implicated files.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteDiagnostic {
    /// Stable code (`ROUTE_DUPLICATE_*`, `ROUTE_AMBIGUOUS_*`,
    /// `ROUTE_MALFORMED_*`, `ROUTE_MISMATCH_*`).
    pub code: &'static str,
    /// Safe message.
    pub message: String,
    /// Implicated files, sorted.
    pub files: Vec<String>,
}

/// A built registry: sorted entries plus optional not-found entry.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteRegistry {
    /// Entries sorted by path.
    pub entries: Vec<RouteEntry>,
    /// Catch-all path, if declared.
    pub not_found: Option<String>,
}

/// Scan `routes/` under a project into a validated registry.
///
/// # Errors
///
/// Returns every diagnostic when files duplicate, collide, or malform.
pub fn build_registry(project: &Path) -> Result<RouteRegistry, Vec<RouteDiagnostic>> {
    let routes_dir = project.join("routes");
    let mut files = Vec::new();
    if routes_dir.is_dir() {
        let mut stack = vec![routes_dir.clone()];
        while let Some(dir) = stack.pop() {
            let Ok(read) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in read.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|extension| extension == "ts") {
                    files.push(path);
                }
            }
        }
    }
    files.sort();
    let mut entries = Vec::new();
    let mut diagnostics = Vec::new();
    for file in &files {
        let Ok(relative) = file.strip_prefix(&routes_dir) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        let Some(entry) = convert_file(project, &routes_dir, &relative, file, &mut diagnostics)
        else {
            continue;
        };
        entries.push(entry);
    }
    validate_registry(&mut entries, &mut diagnostics);
    if diagnostics.is_empty() {
        let not_found = entries
            .iter()
            .find(|entry| entry.kind == RouteKind::Wildcard)
            .map(|entry| entry.path.clone());
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(RouteRegistry { entries, not_found })
    } else {
        diagnostics.sort_by(|left, right| {
            left.code
                .cmp(right.code)
                .then_with(|| left.files.cmp(&right.files))
        });
        Err(diagnostics)
    }
}

fn convert_file(
    project: &Path,
    routes_dir: &Path,
    relative: &str,
    file: &Path,
    diagnostics: &mut Vec<RouteDiagnostic>,
) -> Option<RouteEntry> {
    let stem = relative.strip_suffix(".ts").unwrap_or(relative).to_owned();
    if stem
        .split('/')
        .next()
        .is_some_and(|first| first.starts_with('_'))
    {
        return None;
    }
    if stem.split('/').any(|segment| segment.starts_with('_')) {
        // Nested underscore segments are helper modules, not routes.
        return None;
    }
    let mut segments = Vec::new();
    let mut params = Vec::new();
    let mut wildcard = false;
    for segment in stem.split('/') {
        if segment.is_empty() {
            diagnostics.push(malformed(
                file,
                project,
                format!("empty segment in route file `{relative}`"),
            ));
            return None;
        }
        if let Some(inner) = segment
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            if let Some(name) = inner.strip_prefix("...") {
                if name.is_empty() || !is_identifier(name) {
                    diagnostics.push(malformed(
                        file,
                        project,
                        format!("bad wildcard name in `{relative}`"),
                    ));
                    return None;
                }
                wildcard = true;
                segments.push("*".to_owned());
            } else {
                if !is_identifier(inner) {
                    diagnostics.push(malformed(
                        file,
                        project,
                        format!("bad parameter name in `{relative}`"),
                    ));
                    return None;
                }
                params.push(inner.to_owned());
                segments.push(format!(":{inner}"));
            }
        } else {
            if !is_path_segment(segment) {
                diagnostics.push(malformed(
                    file,
                    project,
                    format!("bad characters in `{relative}`"),
                ));
                return None;
            }
            segments.push(segment.to_owned());
        }
    }
    if wildcard {
        let position = segments.iter().position(|segment| segment == "*");
        if position != Some(segments.len() - 1) {
            diagnostics.push(malformed(
                file,
                project,
                format!("wildcard must be last in `{relative}`"),
            ));
            return None;
        }
    }
    let mut path = format!("/{}", segments.join("/"));
    if path == "/index" {
        path = "/".to_owned();
        segments.clear();
    } else if let Some(rest) = path.strip_suffix("/index") {
        path = if rest.is_empty() {
            "/".to_owned()
        } else {
            rest.to_owned()
        };
        if path == "/" {
            segments.clear();
        } else {
            segments.pop();
        }
    }
    let content = std::fs::read_to_string(file).unwrap_or_default();
    let declared = export_const_string(&content, "route");
    if let Some(declared) = declared {
        if declared != path {
            diagnostics.push(RouteDiagnostic {
                code: "ROUTE_MISMATCH_PATH",
                message: format!(
                    "`route` export {declared:?} disagrees with file path {path:?}; the file path wins"
                ),
                files: vec![display_path(project, file)],
            });
        }
    }
    let title = export_const_string(&content, "title").unwrap_or_else(|| default_title(&path));
    let _ = routes_dir;
    let kind = if wildcard {
        RouteKind::Wildcard
    } else if params.is_empty() {
        RouteKind::Static
    } else {
        RouteKind::Param
    };
    Some(RouteEntry {
        path,
        segments,
        params,
        title,
        file: display_path(project, file),
        kind,
    })
}

fn malformed(file: &Path, project: &Path, message: String) -> RouteDiagnostic {
    let code = if message.contains("wildcard") {
        "ROUTE_MALFORMED_WILDCARD"
    } else if message.contains("empty segment") {
        "ROUTE_MALFORMED_EMPTY"
    } else if message.contains("parameter") || message.contains("wildcard name") {
        "ROUTE_MALFORMED_PARAM"
    } else {
        "ROUTE_MALFORMED_CHARS"
    };
    RouteDiagnostic {
        code,
        message,
        files: vec![display_path(project, file)],
    }
}

fn display_path(project: &Path, file: &Path) -> String {
    file.strip_prefix(project)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| file.to_string_lossy().to_string())
}

fn is_identifier(name: &str) -> bool {
    let mut characters = name.chars();
    match characters.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn is_path_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

fn default_title(path: &str) -> String {
    if path == "/" {
        return "Home".to_owned();
    }
    if path.contains('*') {
        return "Not Found".to_owned();
    }
    let last = path.rsplit('/').next().unwrap_or(path);
    let clean = last.trim_start_matches(':');
    let mut title = String::new();
    for (index, word) in clean.split(['-', '_']).enumerate() {
        if index > 0 {
            title.push(' ');
        }
        let mut characters = word.chars();
        if let Some(first) = characters.next() {
            title.extend(first.to_uppercase());
            title.push_str(characters.as_str());
        }
    }
    if title.is_empty() {
        "Route".to_owned()
    } else {
        title
    }
}

/// Read `export const <name> = "..."` string literals without executing the
/// module. Only plain string literals are recognized.
fn export_const_string(content: &str, name: &str) -> Option<String> {
    let marker = format!("export const {name}");
    let start = content.find(&marker)?;
    let rest = content[start + marker.len()..].trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let quote = rest.chars().next()?;
    if !matches!(quote, '"' | '\'') {
        return None;
    }
    let mut value = String::new();
    let mut escaped = false;
    for character in rest[1..].chars() {
        if escaped {
            value.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == quote {
            return Some(value);
        } else {
            value.push(character);
        }
    }
    None
}

fn validate_registry(entries: &mut [RouteEntry], diagnostics: &mut Vec<RouteDiagnostic>) {
    let mut by_path: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for entry in entries.iter() {
        by_path
            .entry(entry.path.as_str())
            .or_default()
            .push(entry.file.as_str());
    }
    for (path, files) in &by_path {
        if files.len() > 1 {
            let mut files: Vec<String> = files.iter().map(|file| (*file).to_owned()).collect();
            files.sort();
            diagnostics.push(RouteDiagnostic {
                code: "ROUTE_DUPLICATE_PATH",
                message: format!("duplicate route path `{path}`"),
                files,
            });
        }
    }
    // Ambiguity: same segment shape with different parameter names.
    let mut by_shape: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut shape_files: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in entries.iter() {
        let shape = entry
            .segments
            .iter()
            .map(|segment| {
                if segment == "*" {
                    "*".to_owned()
                } else if segment.starts_with(':') {
                    ":param".to_owned()
                } else {
                    segment.clone()
                }
            })
            .collect::<Vec<_>>()
            .join("/");
        by_shape
            .entry(shape.clone())
            .or_default()
            .insert(entry.params.join(","));
        shape_files
            .entry(shape)
            .or_default()
            .push(entry.file.clone());
    }
    for (shape, param_sets) in &by_shape {
        if param_sets.len() > 1 {
            let mut files = shape_files[shape].clone();
            files.sort();
            files.dedup();
            diagnostics.push(RouteDiagnostic {
                code: "ROUTE_AMBIGUOUS_SHAPE",
                message: format!("ambiguous parameter names for shape `{shape}`"),
                files,
            });
        }
    }
    // Two catch-alls is malformed.
    let wildcards: Vec<&RouteEntry> = entries
        .iter()
        .filter(|entry| entry.kind == RouteKind::Wildcard)
        .collect();
    if wildcards.len() > 1 {
        let mut files: Vec<String> = wildcards.iter().map(|entry| entry.file.clone()).collect();
        files.sort();
        diagnostics.push(RouteDiagnostic {
            code: "ROUTE_MALFORMED_WILDCARD",
            message: "at most one catch-all route is allowed".to_owned(),
            files,
        });
    }
    // Duplicate titles are fine; duplicate explicit ids are a separate gate.
}

/// Match one concrete path against the registry.
///
/// Returns the entry plus extracted parameters, or `None` on a miss.
/// Normalizes trailing slashes (except root), strips query/fragment, and
/// matches case-sensitively. First registry-order match wins: statics before
/// params before wildcard at each level (callers sort accordingly).
#[must_use]
pub fn match_route<'a>(
    registry: &'a RouteRegistry,
    path: &str,
) -> Option<(&'a RouteEntry, Vec<(String, String)>)> {
    let path = normalize_path(path);
    let request: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let mut statics = Vec::new();
    let mut params = Vec::new();
    let mut wildcards = Vec::new();
    for entry in &registry.entries {
        match entry.kind {
            RouteKind::Static => statics.push(entry),
            RouteKind::Param => params.push(entry),
            RouteKind::Wildcard => wildcards.push(entry),
        }
    }
    for entry in statics.into_iter().chain(params).chain(wildcards) {
        if let Some(bound) = match_entry(entry, &request) {
            return Some((entry, bound));
        }
    }
    None
}

fn match_entry(entry: &RouteEntry, request: &[&str]) -> Option<Vec<(String, String)>> {
    let mut bound = Vec::new();
    let mut index = 0usize;
    for segment in &entry.segments {
        if segment == "*" {
            return Some(bound);
        }
        let piece = *request.get(index)?;
        if let Some(name) = segment.strip_prefix(':') {
            bound.push((name.to_owned(), piece.to_owned()));
        } else if segment != piece {
            return None;
        }
        index += 1;
    }
    if index == request.len() {
        Some(bound)
    } else {
        None
    }
}

fn normalize_path(path: &str) -> String {
    let path = path.split(['?', '#']).next().unwrap_or_default();
    if path.len() > 1 {
        path.trim_end_matches('/').to_owned()
    } else if path.is_empty() {
        "/".to_owned()
    } else {
        path.to_owned()
    }
}

/// Render the deterministic route module, preserving the legacy constants.
#[must_use]
pub fn render_module(registry: &RouteRegistry) -> String {
    let mut source = String::from("// Generated from routes/ — do not edit\n");
    for entry in &registry.entries {
        if entry.kind != RouteKind::Wildcard {
            let slug = entry.path.replace(['/', '-', ':'], "_");
            let slug = slug.trim_matches('_');
            let slug = if slug.is_empty() { "root" } else { slug };
            source.push_str(&format!(
                "export const route_{slug} = \"{}\";\n",
                entry.path
            ));
        }
    }
    let paths: Vec<String> = registry
        .entries
        .iter()
        .filter(|entry| entry.kind != RouteKind::Wildcard)
        .map(|entry| format!("\"{}\"", entry.path))
        .collect();
    source.push_str(&format!(
        "export const declaredRoutes = [{}];\n",
        paths.join(", ")
    ));
    source.push_str("export interface RouteEntry {\n");
    source.push_str("  path: string;\n");
    source.push_str("  params: string[];\n");
    source.push_str("  title: string;\n");
    source.push_str("  file: string;\n");
    source.push_str("  kind: \"static\" | \"param\" | \"wildcard\";\n");
    source.push_str("}\n");
    source.push_str("export const routeTable: RouteEntry[] = [\n");
    for entry in &registry.entries {
        let params = entry
            .params
            .iter()
            .map(|param| format!("\"{param}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let kind = match entry.kind {
            RouteKind::Static => "static",
            RouteKind::Param => "param",
            RouteKind::Wildcard => "wildcard",
        };
        source.push_str(&format!(
            "  {{ path: \"{}\", params: [{}], title: \"{}\", file: \"{}\", kind: \"{}\" }},\n",
            entry.path, params, entry.title, entry.file, kind
        ));
    }
    source.push_str("];\n");
    match &registry.not_found {
        Some(path) => source.push_str(&format!(
            "export const notFoundRoute: string = \"{path}\";\n"
        )),
        None => source.push_str("export const notFoundRoute: string | null = null;\n"),
    }
    source
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_is_exact_and_case_sensitive() {
        assert_eq!(normalize_path("/pos/"), "/pos");
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path(""), "/");
        assert_eq!(normalize_path("/pos?x=1#top"), "/pos");
        assert_eq!(normalize_path("/POS"), "/POS");
    }
}
