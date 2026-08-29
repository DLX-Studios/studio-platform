//! A small, headless language-server seam for Studio Script.
//!
//! The server owns no Designer state.  [`Workspace`] is an in-memory index that can be
//! populated from files, editor buffers, or a test fixture.  A deliberately cheap lexical
//! index provides keystroke-latency answers while the parser-of-record boundary is consulted
//! for source diagnostics.  The lexical index is intentionally Tree-sitter-compatible: it
//! stores source spans and never becomes a second semantic model, so a Tree-sitter adapter
//! or the full Studio Script parser can be substituted without changing the LSP surface.

#![allow(missing_docs)]
#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::uninlined_format_args,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::collapsible_if,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::manual_pattern_char_comparison,
    clippy::single_char_pattern
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A UTF-16 LSP position.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// Compatibility alias for editor adapters that use an explicit LSP prefix.
pub type LspPosition = Position;

/// An LSP source range.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// Compatibility alias for editor adapters that use an explicit LSP prefix.
pub type LspRange = Range;

/// A source location used by go-to-definition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

/// An LSP diagnostic. `code` values are stable Studio-owned strings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: u8,
    pub code: String,
    pub source: String,
    pub message: String,
}

/// A completion entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<u8>,
}

/// A completion response.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompletionList {
    pub is_incomplete: bool,
    pub items: Vec<CompletionItem>,
}

/// Hover markdown returned by the server.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Hover {
    pub contents: HoverContents,
    pub range: Option<Range>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HoverContents {
    pub kind: String,
    pub value: String,
}

#[derive(Clone, Debug)]
struct Definition {
    name: String,
    location: Location,
    detail: String,
    documentation: Option<String>,
}

#[derive(Clone, Debug)]
struct SchemaField {
    name: String,
    ty: String,
    location: Location,
}

#[derive(Clone, Debug)]
struct ResponseSchema {
    name: String,
    fields: BTreeMap<String, SchemaField>,
}

/// A source-backed workspace index. It is independent of Designer process and storage.
#[derive(Clone, Debug)]
pub struct Workspace {
    files: BTreeMap<String, String>,
    components: BTreeMap<String, Definition>,
    tokens: BTreeMap<String, Definition>,
    plugins: BTreeMap<String, Definition>,
    schemas: BTreeMap<String, ResponseSchema>,
    declared_schemas: BTreeMap<String, ResponseSchema>,
    catalog: BTreeSet<String>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspace {
    /// Create an empty workspace populated with the first-party component catalog.
    #[must_use]
    pub fn new() -> Self {
        let mut workspace = Self {
            files: BTreeMap::new(),
            components: BTreeMap::new(),
            tokens: BTreeMap::new(),
            plugins: BTreeMap::new(),
            schemas: BTreeMap::new(),
            declared_schemas: BTreeMap::new(),
            catalog: BTreeSet::new(),
        };
        workspace
            .catalog
            .extend(CATALOG_COMPONENTS.iter().map(|name| (*name).to_owned()));
        for name in &workspace.catalog {
            workspace.components.insert(
                name.clone(),
                Definition {
                    name: name.clone(),
                    location: Location {
                        uri: format!("studio://component/{name}"),
                        range: Range::default(),
                    },
                    detail: "Studio component".to_owned(),
                    documentation: None,
                },
            );
        }
        workspace
    }

    /// Index a source buffer under an editor URI.
    pub fn add_file(&mut self, uri: impl Into<String>, source: impl Into<String>) {
        self.files.insert(uri.into(), source.into());
        self.rebuild_index();
    }

    /// Alias used by programmatic integrations.
    pub fn with_file(mut self, uri: impl Into<String>, source: impl Into<String>) -> Self {
        self.add_file(uri, source);
        self
    }

    /// Add a first-party or plugin component to the catalog.
    pub fn add_component(&mut self, name: impl Into<String>, documentation: Option<String>) {
        let name = name.into();
        let uri = format!("studio://component/{name}");
        self.components.insert(
            name.clone(),
            Definition {
                name,
                location: Location {
                    uri,
                    range: Range::default(),
                },
                detail: "Studio component".to_owned(),
                documentation,
            },
        );
    }

    /// Add a design token definition to the workspace.
    pub fn add_token(
        &mut self,
        name: impl Into<String>,
        ty: impl Into<String>,
        documentation: Option<String>,
    ) {
        let name = normalize_token_name(&name.into());
        self.tokens.insert(
            name.clone(),
            Definition {
                name,
                location: Location {
                    uri: "studio://tokens".to_owned(),
                    range: Range::default(),
                },
                detail: ty.into(),
                documentation,
            },
        );
    }

    /// Add a plugin SDK surface. `detail` is shown in completion and hover.
    pub fn add_plugin_surface(
        &mut self,
        name: impl Into<String>,
        detail: impl Into<String>,
        documentation: Option<String>,
    ) {
        let name = name.into();
        self.plugins.insert(
            name.clone(),
            Definition {
                name,
                location: Location {
                    uri: "studio://plugin-sdk".to_owned(),
                    range: Range::default(),
                },
                detail: detail.into(),
                documentation,
            },
        );
    }

    /// Declare a response schema used by `$item` bindings.
    pub fn add_response_schema<I, N, T>(&mut self, name: impl Into<String>, fields: I)
    where
        I: IntoIterator<Item = (N, T)>,
        N: Into<String>,
        T: Into<String>,
    {
        let name = name.into();
        let fields = fields
            .into_iter()
            .map(|(field, ty)| {
                let field = field.into();
                (
                    field.clone(),
                    SchemaField {
                        name: field,
                        ty: ty.into(),
                        location: Location {
                            uri: format!("studio://schema/{name}"),
                            range: Range::default(),
                        },
                    },
                )
            })
            .collect();
        let schema = ResponseSchema { name, fields };
        self.declared_schemas.insert(schema.name.clone(), schema);
        self.rebuild_index();
    }

    /// Load all source-like files below a project root without consulting Designer state.
    pub fn from_root(root: impl AsRef<Path>) -> io::Result<Self> {
        let mut workspace = Self::new();
        let root = root.as_ref().to_path_buf();
        let mut files = Vec::new();
        collect_files(&root, &mut files)?;
        files.sort();
        for path in files {
            let source = fs::read_to_string(&path)?;
            workspace.files.insert(path_to_uri(&path), source);
        }
        workspace.rebuild_index();
        Ok(workspace)
    }

    /// Return the indexed source, useful for editor adapters.
    #[must_use]
    pub fn source(&self, uri: &str) -> Option<&str> {
        self.files.get(uri).map(String::as_str)
    }

    fn rebuild_index(&mut self) {
        self.components.retain(|_, definition| {
            definition.location.uri.starts_with("studio://")
                && !definition.location.uri.starts_with("studio://source/")
        });
        self.tokens
            .retain(|_, definition| definition.location.uri == "studio://tokens");
        self.plugins
            .retain(|_, definition| definition.location.uri == "studio://plugin-sdk");
        self.schemas = self.declared_schemas.clone();
        for (uri, source) in &self.files {
            scan_source(
                uri,
                source,
                &mut self.components,
                &mut self.tokens,
                &mut self.plugins,
                &mut self.schemas,
            );
        }
    }
}

/// A headless server over an indexed [`Workspace`].
#[derive(Clone, Debug)]
pub struct LanguageServer {
    workspace: Workspace,
    documents: BTreeMap<String, String>,
    shutdown: bool,
}

/// Short alias used by embedders that call the implementation a server seam.
pub type Server = LanguageServer;

impl LanguageServer {
    /// Construct a server with an in-memory workspace.
    #[must_use]
    pub fn new(workspace: Workspace) -> Self {
        Self {
            workspace,
            documents: BTreeMap::new(),
            shutdown: false,
        }
    }

    /// Construct a server from a project tree.
    pub fn from_root(root: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self::new(Workspace::from_root(root)?))
    }

    /// Access the current workspace.
    #[must_use]
    pub const fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// Open or replace a document buffer.
    pub fn open_document(&mut self, uri: impl Into<String>, source: impl Into<String>) {
        let uri = uri.into();
        let source = source.into();
        self.documents.insert(uri.clone(), source.clone());
        self.workspace.add_file(uri, source);
    }

    /// Return parser and Studio semantic diagnostics for a document.
    #[must_use]
    pub fn diagnostics(&self, uri: &str) -> Vec<Diagnostic> {
        let Some(source) = self.source(uri) else {
            return Vec::new();
        };
        let mut diagnostics = parser_diagnostics(uri, source);
        diagnostics.extend(semantic_diagnostics(&self.workspace, uri, source));
        diagnostics.sort_by_key(|diagnostic| {
            (
                diagnostic.range.start.line,
                diagnostic.range.start.character,
                diagnostic.code.clone(),
            )
        });
        diagnostics
    }

    /// Return deterministic completion items at an LSP position.
    #[must_use]
    pub fn completion(&self, uri: &str, position: Position) -> CompletionList {
        let Some(source) = self.source(uri) else {
            return CompletionList {
                is_incomplete: false,
                items: Vec::new(),
            };
        };
        let offset = offset_for_position(source, position);
        let prefix = completion_prefix(source, offset);
        let mut items = BTreeMap::<String, CompletionItem>::new();
        let tag_context = source[..offset].rfind('<').is_some_and(|start| {
            start > source[..offset].rfind('>').unwrap_or(0) && !source[start..offset].contains('>')
        });
        if tag_context {
            for name in self
                .workspace
                .catalog
                .iter()
                .chain(self.workspace.components.keys())
            {
                insert_completion(&mut items, name, "Studio component", None, 7);
            }
        }
        if prefix.starts_with("$item.") {
            for schema in self.workspace.schemas.values() {
                for field in schema.fields.values() {
                    insert_completion(
                        &mut items,
                        &field.name,
                        &field.ty,
                        Some(format!("{} response field", schema.name)),
                        5,
                    );
                }
            }
        }
        if prefix.starts_with("token.") || prefix.starts_with("$token.") || prefix.starts_with('@')
        {
            for definition in self.workspace.tokens.values() {
                insert_completion(
                    &mut items,
                    &definition.name,
                    &definition.detail,
                    definition.documentation.clone(),
                    21,
                );
            }
        }
        if prefix.starts_with("plugin.")
            || prefix.starts_with("sdk.")
            || prefix == "plugin"
            || prefix == "sdk"
        {
            for definition in self.workspace.plugins.values() {
                insert_completion(
                    &mut items,
                    &definition.name,
                    &definition.detail,
                    definition.documentation.clone(),
                    3,
                );
            }
        }
        if !tag_context && prefix.is_empty() {
            for definition in self.workspace.plugins.values() {
                insert_completion(
                    &mut items,
                    &definition.name,
                    &definition.detail,
                    definition.documentation.clone(),
                    3,
                );
            }
        }
        CompletionList {
            is_incomplete: false,
            items: items.into_values().collect(),
        }
    }

    /// Return hover information for a component, token, plugin surface, or `$item` field.
    #[must_use]
    pub fn hover(&self, uri: &str, position: Position) -> Option<Hover> {
        let source = self.source(uri)?;
        let offset = offset_for_position(source, position);
        let (symbol, range) = symbol_at(source, offset)?;
        if let Some(field) = symbol.strip_prefix("$item.") {
            for schema in self.workspace.schemas.values() {
                if let Some(field) = schema.fields.get(field) {
                    return Some(hover_for(
                        &field.name,
                        &format!("{} — `$item` response field", field.ty),
                        None,
                        range,
                    ));
                }
            }
        }
        let name = normalize_token_name(&symbol);
        let definition = self
            .workspace
            .tokens
            .get(&name)
            .or_else(|| self.workspace.components.get(&symbol))
            .or_else(|| self.workspace.plugins.get(&symbol));
        definition.map(|definition| {
            hover_for(
                &definition.name,
                &definition.detail,
                definition.documentation.clone(),
                range,
            )
        })
    }

    /// Resolve a symbol across all indexed project and plugin files.
    #[must_use]
    pub fn definition(&self, uri: &str, position: Position) -> Vec<Location> {
        let Some(source) = self.source(uri) else {
            return Vec::new();
        };
        let offset = offset_for_position(source, position);
        let Some((symbol, _)) = symbol_at(source, offset) else {
            return Vec::new();
        };
        if let Some(field) = symbol.strip_prefix("$item.") {
            return self
                .workspace
                .schemas
                .values()
                .filter_map(|schema| schema.fields.get(field).map(|field| field.location.clone()))
                .collect();
        }
        let normalized = normalize_token_name(&symbol);
        if let Some(definition) = self.workspace.tokens.get(&normalized) {
            return vec![definition.location.clone()];
        }
        if let Some(definition) = self.workspace.components.get(&symbol) {
            return vec![definition.location.clone()];
        }
        if let Some(definition) = self.workspace.plugins.get(&symbol) {
            return vec![definition.location.clone()];
        }
        Vec::new()
    }

    /// Run a framed JSON-RPC/LSP session over any stdio-like reader and writer.
    pub fn serve<R: BufRead, W: Write>(&mut self, mut reader: R, mut writer: W) -> io::Result<()> {
        while let Some(message) = read_message(&mut reader)? {
            let (response, notifications) = self.handle_message(message);
            for notification in notifications {
                write_message(&mut writer, &notification)?;
            }
            if let Some(response) = response {
                write_message(&mut writer, &response)?;
            }
            writer.flush()?;
            if self.shutdown {
                break;
            }
        }
        Ok(())
    }

    fn handle_message(&mut self, message: Value) -> (Option<Value>, Vec<Value>) {
        let id = message.get("id").cloned();
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let mut notifications = Vec::new();
        let result = match method {
            "initialize" => json!({
                "capabilities": {
                    "textDocumentSync": 1,
                    "completionProvider": {"triggerCharacters": ["<", ".", "$", "@"]},
                    "hoverProvider": true,
                    "definitionProvider": true
                },
                "serverInfo": {"name": "studio-language-server", "version": env!("CARGO_PKG_VERSION")}
            }),
            "initialized" => return (None, notifications),
            "shutdown" => { self.shutdown = true; Value::Null },
            "exit" => { self.shutdown = true; return (None, notifications); },
            "textDocument/didOpen" => {
                if let Some(document) = params.get("textDocument") {
                    if let (Some(uri), Some(text)) = (document.get("uri").and_then(Value::as_str), document.get("text").and_then(Value::as_str)) {
                        self.open_document(uri, text);
                        notifications.push(publish_diagnostics(uri, &self.diagnostics(uri)));
                    }
                }
                return (None, notifications);
            }
            "textDocument/didChange" => {
                if let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) {
                    if let Some(text) = params.pointer("/contentChanges/0/text").and_then(Value::as_str) {
                        self.open_document(uri, text);
                        notifications.push(publish_diagnostics(uri, &self.diagnostics(uri)));
                    }
                }
                return (None, notifications);
            }
            "textDocument/completion" => {
                let (uri, position) = match request_document_position(&params) {
                    Ok(value) => value,
                    Err(error) => return (id.map(|id| rpc_error(id, error)), notifications),
                };
                serde_json::to_value(self.completion(&uri, position)).unwrap_or(Value::Null)
            }
            "textDocument/hover" => {
                let (uri, position) = match request_document_position(&params) {
                    Ok(value) => value,
                    Err(error) => return (id.map(|id| rpc_error(id, error)), notifications),
                };
                serde_json::to_value(self.hover(&uri, position)).unwrap_or(Value::Null)
            }
            "textDocument/definition" => {
                let (uri, position) = match request_document_position(&params) {
                    Ok(value) => value,
                    Err(error) => return (id.map(|id| rpc_error(id, error)), notifications),
                };
                serde_json::to_value(self.definition(&uri, position)).unwrap_or(Value::Array(Vec::new()))
            }
            "textDocument/diagnostic" => {
                let uri = params.pointer("/textDocument/uri").and_then(Value::as_str).unwrap_or_default();
                json!({"kind": "full", "items": self.diagnostics(uri)})
            }
            _ => return (id.map(|id| json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"method not found"}})), notifications),
        };
        (
            id.map(|id| json!({"jsonrpc": "2.0", "id": id, "result": result})),
            notifications,
        )
    }

    fn source(&self, uri: &str) -> Option<&str> {
        self.documents
            .get(uri)
            .map(String::as_str)
            .or_else(|| self.workspace.source(uri))
    }
}

/// Start a server over process stdin/stdout.
pub fn run_stdio(workspace: Workspace) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    LanguageServer::new(workspace).serve(stdin.lock(), stdout.lock())
}

fn parser_diagnostics(uri: &str, source: &str) -> Vec<Diagnostic> {
    let mut output = Vec::new();
    if let Err(error) = studio_script::prepare(uri, source, studio_script::Target::JavaScript) {
        let code = match &error {
            studio_script::Error::MultipleScriptBlocks { .. } => "STUDIO-SCRIPT-MULTIPLE",
            studio_script::Error::UnterminatedScriptBlock { .. } => "STUDIO-SCRIPT-UNTERMINATED",
            studio_script::Error::UnexpectedScriptClose { .. } => "STUDIO-SCRIPT-UNEXPECTED-CLOSE",
        };
        output.push(Diagnostic {
            range: Range {
                start: Position::default(),
                end: Position {
                    line: 0,
                    character: 1,
                },
            },
            severity: 1,
            code: code.to_owned(),
            source: "studio-script".to_owned(),
            message: error.to_string(),
        });
    }
    output
}

fn semantic_diagnostics(workspace: &Workspace, uri: &str, source: &str) -> Vec<Diagnostic> {
    let mut output = Vec::new();
    let known_components: BTreeSet<&str> = workspace
        .catalog
        .iter()
        .chain(workspace.components.keys())
        .map(String::as_str)
        .collect();
    let tags = scan_tags(source);
    let mut stack: Vec<(String, usize)> = Vec::new();
    for tag in tags {
        if tag.name == "script" {
            continue;
        }
        if tag.closing {
            if let Some((open, start)) = stack.pop() {
                if open != tag.name {
                    output.push(source_diagnostic(
                        uri,
                        source,
                        tag.start,
                        tag.end,
                        "STUDIO-TAG-MISMATCH",
                        format!("closing </{}> does not match <{}>", tag.name, open),
                    ));
                    stack.push((open, start));
                }
            } else {
                output.push(source_diagnostic(
                    uri,
                    source,
                    tag.start,
                    tag.end,
                    "STUDIO-TAG-UNEXPECTED-CLOSE",
                    format!("closing </{}> has no matching opening tag", tag.name),
                ));
            }
            continue;
        }
        if is_component_name(&tag.name) && !known_components.contains(tag.name.as_str()) {
            output.push(source_diagnostic(
                uri,
                source,
                tag.start,
                tag.end,
                "STUDIO-COMPONENT-UNKNOWN",
                format!("unknown Studio component <{}>", tag.name),
            ));
        }
        if !tag.self_closing {
            stack.push((tag.name.clone(), tag.start));
        }
    }
    for (name, start) in stack {
        output.push(source_diagnostic(
            uri,
            source,
            start,
            start + name.len() + 1,
            "STUDIO-TAG-UNTERMINATED",
            format!("element <{}> is not closed", name),
        ));
    }
    for (name, start, end) in scan_references(source, "$item.") {
        let field = name.trim_start_matches("$item.");
        if !workspace.schemas.is_empty()
            && !workspace
                .schemas
                .values()
                .any(|schema| schema.fields.contains_key(field))
        {
            output.push(source_diagnostic(
                uri,
                source,
                start,
                end,
                "STUDIO-ITEM-FIELD-UNKNOWN",
                format!("$item field {} is not declared by a response schema", field),
            ));
        }
    }
    for (name, start, end) in scan_token_references(source) {
        let name = normalize_token_name(&name);
        if !workspace.tokens.contains_key(&name) && !name.is_empty() {
            output.push(source_diagnostic(
                uri,
                source,
                start,
                end,
                "STUDIO-TOKEN-UNKNOWN",
                format!("unknown design token {}", name),
            ));
        }
    }
    output
}

fn scan_token_references(source: &str) -> Vec<(String, usize, usize)> {
    let mut output = scan_references(source, "$token.");
    output.extend(
        scan_references(source, "token.")
            .into_iter()
            .filter(|(_, start, _)| *start == 0 || source.as_bytes()[*start - 1] != b'$'),
    );
    output.extend(
        scan_references(source, "@")
            .into_iter()
            .filter(|(_, start, _)| {
                *start == 0 || !source.as_bytes()[*start - 1].is_ascii_alphanumeric()
            }),
    );
    output.sort_by_key(|(_, start, _)| *start);
    output
}

fn scan_source(
    uri: &str,
    source: &str,
    components: &mut BTreeMap<String, Definition>,
    tokens: &mut BTreeMap<String, Definition>,
    plugins: &mut BTreeMap<String, Definition>,
    schemas: &mut BTreeMap<String, ResponseSchema>,
) {
    let path = uri_to_path(uri);
    let is_component_file = path
        .as_ref()
        .is_some_and(|path| path.to_string_lossy().contains("/components/"));
    for tag in scan_tags(source) {
        if !tag.closing && is_component_name(&tag.name) && is_component_file {
            let location = Location {
                uri: uri.to_owned(),
                range: span(source, tag.start, tag.end),
            };
            components.entry(tag.name.clone()).or_insert(Definition {
                name: tag.name,
                location,
                detail: "Project component".to_owned(),
                documentation: None,
            });
        }
    }
    for (name, start, end) in scan_token_declarations(source) {
        tokens.insert(
            name.clone(),
            Definition {
                name: name.clone(),
                location: Location {
                    uri: uri.to_owned(),
                    range: span(source, start, end),
                },
                detail: "design token".to_owned(),
                documentation: None,
            },
        );
    }
    for (name, start, end, detail) in scan_plugin_declarations(source) {
        plugins.insert(
            name.clone(),
            Definition {
                name,
                location: Location {
                    uri: uri.to_owned(),
                    range: span(source, start, end),
                },
                detail,
                documentation: None,
            },
        );
    }
    if uri.ends_with(".json") {
        for name in scan_plugin_json_names(source) {
            plugins.entry(name.clone()).or_insert(Definition {
                name,
                location: Location {
                    uri: uri.to_owned(),
                    range: Range::default(),
                },
                detail: "Plugin contribution".to_owned(),
                documentation: None,
            });
        }
        for schema in scan_json_response_schemas(uri, source) {
            schemas.insert(schema.name.clone(), schema);
        }
    }
    for schema in scan_schemas(uri, source) {
        schemas.insert(schema.name.clone(), schema);
    }
}

fn scan_plugin_declarations(source: &str) -> Vec<(String, usize, usize, String)> {
    let mut output = Vec::new();
    for (line_start, line) in lines_with_offsets(source) {
        let trimmed = line.trim_start();
        let Some(keyword) = [
            "export function ",
            "export class ",
            "export const ",
            "export interface ",
            "export type ",
        ]
        .iter()
        .find(|keyword| trimmed.starts_with(**keyword)) else {
            continue;
        };
        let name_start = line_start + line.find(keyword).unwrap_or(0) + keyword.len();
        let name = identifier_at(source, name_start);
        if !name.is_empty() {
            output.push((
                name.clone(),
                name_start,
                name_start + name.len(),
                "Plugin SDK surface".to_owned(),
            ));
        }
    }
    output
}

fn scan_plugin_json_names(source: &str) -> Vec<String> {
    fn visit(value: &Value, names: &mut BTreeSet<String>) {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    if matches!(key.as_str(), "id" | "name" | "command" | "action") {
                        if let Some(name) = value.as_str() {
                            if is_identifier_path(name) || name.contains('.') {
                                names.insert(name.to_owned());
                            }
                        }
                    }
                    visit(value, names);
                }
            }
            Value::Array(array) => {
                for value in array {
                    visit(value, names);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    let Ok(value) = serde_json::from_str(source) else {
        return Vec::new();
    };
    let mut names = BTreeSet::new();
    visit(&value, &mut names);
    names.into_iter().collect()
}

/// Index top-level fields from plugin route response schemas.
///
/// Plugin manifests are the source of truth for host-mediated response
/// shapes.  They use JSON Schema under `routes[].responseSchema`; keeping the
/// extraction here intentionally shallow exposes the fields that can be used
/// directly as `$item.field` bindings without pretending to type arbitrary
/// nested JSON Schema expressions.
fn scan_json_response_schemas(uri: &str, source: &str) -> Vec<ResponseSchema> {
    let Ok(value) = serde_json::from_str::<Value>(source) else {
        return Vec::new();
    };
    let Some(routes) = value.get("routes").and_then(Value::as_array) else {
        return Vec::new();
    };

    routes
        .iter()
        .filter_map(|route| {
            let name = route.get("id").and_then(Value::as_str)?;
            let response_schema = route.get("responseSchema")?;
            let schema = if response_schema.get("type").and_then(Value::as_str) == Some("array") {
                response_schema.get("items")?
            } else {
                response_schema
            };
            let properties = schema.get("properties").and_then(Value::as_object)?;
            let fields = properties
                .iter()
                .map(|(field, definition)| {
                    (
                        field.clone(),
                        SchemaField {
                            name: field.clone(),
                            ty: json_schema_type(definition),
                            location: Location {
                                uri: format!(
                                    "{uri}#routes/{name}/responseSchema/properties/{field}"
                                ),
                                range: Range::default(),
                            },
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();
            (!fields.is_empty()).then_some(ResponseSchema {
                name: name.to_owned(),
                fields,
            })
        })
        .collect()
}

fn json_schema_type(definition: &Value) -> String {
    definition
        .get("type")
        .and_then(Value::as_str)
        .map_or_else(|| "unknown".to_owned(), ToOwned::to_owned)
}

fn scan_schemas(uri: &str, source: &str) -> Vec<ResponseSchema> {
    let mut output = Vec::new();
    let lines: Vec<(usize, &str)> = lines_with_offsets(source).collect();
    for (index, (line_start, line)) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let keyword = if trimmed.starts_with("interface ") {
            Some("interface ")
        } else if trimmed.starts_with("type ") {
            Some("type ")
        } else {
            None
        };
        let Some(keyword) = keyword else {
            continue;
        };
        let name_start = *line_start + line.find(keyword).unwrap_or(0) + keyword.len();
        let name = identifier_at(source, name_start);
        if name.is_empty() {
            continue;
        }
        let mut body_lines = Vec::new();
        let mut depth = 0_i32;
        for (next_start, next_line) in lines.iter().skip(index) {
            depth += next_line.matches('{').count() as i32;
            depth -= next_line.matches('}').count() as i32;
            body_lines.push((*next_start, *next_line));
            if depth <= 0 {
                break;
            }
        }
        let mut fields = BTreeMap::new();
        for (field_line_start, field_line) in body_lines {
            let body_start = field_line.find('{').map_or(0, |offset| offset + 1);
            let body_end = field_line[body_start..]
                .find('}')
                .map_or(field_line.len(), |offset| body_start + offset);
            let body = &field_line[body_start..body_end];
            for (fragment_offset, fragment) in body
                .split_inclusive(|character: char| character == ';' || character == ',')
                .scan(0, |offset, fragment| {
                    let start = *offset;
                    *offset += fragment.len();
                    Some((start, fragment))
                })
            {
                let trimmed = fragment
                    .trim()
                    .trim_end_matches(|character: char| character == ';' || character == ',')
                    .trim();
                let Some(colon) = trimmed.find(':') else {
                    continue;
                };
                let field = trimmed[..colon].trim().trim_end_matches('?').trim();
                if !is_identifier(field) {
                    continue;
                }
                let ty = trimmed[colon + 1..].trim().to_owned();
                let field_offset = field_line_start
                    + body_start
                    + fragment_offset
                    + fragment.find(field).unwrap_or(0);
                fields.insert(
                    field.to_owned(),
                    SchemaField {
                        name: field.to_owned(),
                        ty,
                        location: Location {
                            uri: uri.to_owned(),
                            range: span(source, field_offset, field_offset + field.len()),
                        },
                    },
                );
            }
        }
        if !fields.is_empty() {
            output.push(ResponseSchema { name, fields });
        }
    }
    output
}

fn scan_token_declarations(source: &str) -> Vec<(String, usize, usize)> {
    let mut output = Vec::new();
    for (line_start, line) in lines_with_offsets(source) {
        let trimmed = line.trim();
        let candidate = if let Some(rest) = trimmed.strip_prefix("token ") {
            rest
        } else if let Some(rest) = trimmed.strip_prefix("token.") {
            rest
        } else if let Some(rest) = trimmed.strip_prefix("--") {
            rest
        } else {
            continue;
        };
        let name = candidate
            .split(|character: char| matches!(character, ' ' | '=' | ':' | ';'))
            .next()
            .unwrap_or_default();
        if is_identifier_path(name) {
            let name = normalize_token_name(name);
            let start = line_start
                + line
                    .find(name.strip_prefix("token.").unwrap_or(&name))
                    .unwrap_or(0);
            output.push((name, start, start + candidate.len().min(64)));
        }
    }
    output
}

#[derive(Clone, Debug)]
struct Tag {
    name: String,
    start: usize,
    end: usize,
    closing: bool,
    self_closing: bool,
}

fn scan_tags(source: &str) -> Vec<Tag> {
    let mut output = Vec::new();
    let bytes = source.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let Some(relative) = source[cursor..].find('<') else {
            break;
        };
        let start = cursor + relative;
        let mut index = start + 1;
        let closing = bytes.get(index) == Some(&b'/');
        if closing {
            index += 1;
        }
        if bytes
            .get(index)
            .is_some_and(|byte| *byte == b'!' || *byte == b'?')
        {
            cursor = index + 1;
            continue;
        }
        let name_start = index;
        while bytes.get(index).is_some_and(|byte| {
            byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b'_' || *byte == b'.'
        }) {
            index += 1;
        }
        if index == name_start {
            cursor = start + 1;
            continue;
        }
        let name = source[name_start..index].to_owned();
        let Some(close_relative) = source[index..].find('>') else {
            break;
        };
        let end = index + close_relative + 1;
        let self_closing = source[name_start..end].trim_end().ends_with("/");
        output.push(Tag {
            name,
            start,
            end,
            closing,
            self_closing,
        });
        if !closing && source[name_start..index].eq_ignore_ascii_case("script") {
            if let Some(close_relative) = source[end..].find("</script>") {
                let close_start = end + close_relative;
                let close_end = close_start + "</script>".len();
                output.push(Tag {
                    name: "script".to_owned(),
                    start: close_start,
                    end: close_end,
                    closing: true,
                    self_closing: false,
                });
                cursor = close_end;
                continue;
            }
        }
        cursor = end;
    }
    output
}

fn scan_references(source: &str, prefix: &str) -> Vec<(String, usize, usize)> {
    let mut output = Vec::new();
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find(prefix) {
        let start = cursor + relative;
        let mut end = start + prefix.len();
        while end < source.len()
            && (source.as_bytes()[end].is_ascii_alphanumeric()
                || source.as_bytes()[end] == b'_'
                || source.as_bytes()[end] == b'-'
                || source.as_bytes()[end] == b'.')
        {
            end += 1;
        }
        output.push((source[start..end].to_owned(), start, end));
        cursor = end;
    }
    output
}

fn source_diagnostic(
    _uri: &str,
    source: &str,
    start: usize,
    end: usize,
    code: &str,
    message: String,
) -> Diagnostic {
    Diagnostic {
        range: span(source, start, end),
        severity: 1,
        code: code.to_owned(),
        source: "studio-language-server".to_owned(),
        message,
    }
}

fn insert_completion(
    items: &mut BTreeMap<String, CompletionItem>,
    label: &str,
    detail: &str,
    documentation: Option<String>,
    kind: u8,
) {
    items.entry(label.to_owned()).or_insert(CompletionItem {
        label: label.to_owned(),
        detail: Some(detail.to_owned()),
        documentation,
        kind: Some(kind),
    });
}

fn hover_for(name: &str, detail: &str, documentation: Option<String>, range: Range) -> Hover {
    let mut value = format!("**{name}**\\n\\n`{detail}`");
    if let Some(documentation) = documentation {
        value.push_str("\\n\\n");
        value.push_str(&documentation);
    }
    Hover {
        contents: HoverContents {
            kind: "markdown".to_owned(),
            value,
        },
        range: Some(range),
    }
}

fn publish_diagnostics(uri: &str, diagnostics: &[Diagnostic]) -> Value {
    json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":diagnostics}})
}

fn request_document_position(params: &Value) -> Result<(String, Position), Value> {
    let Some(uri) = params
        .pointer("/textDocument/uri")
        .and_then(Value::as_str)
        .filter(|uri| !uri.trim().is_empty() && uri.contains("://"))
    else {
        return Err(json!({
            "code": -32602,
            "message": "textDocument.uri must be a non-empty URI",
            "data": {"field": "textDocument.uri"}
        }));
    };
    let Some(position_value) = params.pointer("/position") else {
        return Err(json!({
            "code": -32602,
            "message": "position is required",
            "data": {"field": "position"}
        }));
    };
    let position: Position = serde_json::from_value(position_value.clone()).map_err(|_| {
        json!({
            "code": -32602,
            "message": "position.line and position.character must be unsigned integers",
            "data": {"field": "position"}
        })
    })?;
    Ok((uri.to_owned(), position))
}

fn rpc_error(id: Value, error: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": error})
}

fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Value>> {
    let mut header = String::new();
    let mut length = None;
    loop {
        header.clear();
        if reader.read_line(&mut header)? == 0 {
            return Ok(None);
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
            );
        }
    }
    let length = length
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length"))?;
    let mut payload = vec![0; length];
    reader.read_exact(&mut payload)?;
    serde_json::from_slice(&payload)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn write_message<W: Write>(writer: &mut W, message: &Value) -> io::Result<()> {
    let payload = serde_json::to_vec(message).map_err(io::Error::other)?;
    write!(writer, "Content-Length: {}\r\n\r\n", payload.len())?;
    writer.write_all(&payload)
}

fn completion_prefix(source: &str, offset: usize) -> String {
    let mut start = offset;
    while start > 0 {
        let byte = source.as_bytes()[start - 1];
        if byte.is_ascii_alphanumeric()
            || byte == b'_'
            || byte == b'-'
            || byte == b'.'
            || byte == b'$'
            || byte == b'@'
        {
            start -= 1;
        } else {
            break;
        }
    }
    source[start..offset].to_owned()
}

fn symbol_at(source: &str, offset: usize) -> Option<(String, Range)> {
    if source.is_empty() {
        return None;
    }
    let mut start = offset.min(source.len());
    while start > 0 && is_symbol_byte(source.as_bytes()[start - 1]) {
        start -= 1;
    }
    let mut end = offset.min(source.len());
    while end < source.len() && is_symbol_byte(source.as_bytes()[end]) {
        end += 1;
    }
    if start == end {
        return None;
    }
    Some((source[start..end].to_owned(), span(source, start, end)))
}

fn is_symbol_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'$' | b'@')
}
fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}
fn is_identifier_path(value: &str) -> bool {
    !value.is_empty() && value.split('.').all(is_identifier)
}
fn is_component_name(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}
fn normalize_token_name(value: &str) -> String {
    value
        .trim_start_matches('$')
        .trim_start_matches('@')
        .strip_prefix("token.")
        .unwrap_or(value)
        .to_owned()
}
fn identifier_at(source: &str, offset: usize) -> String {
    let mut end = offset;
    while end < source.len()
        && (source.as_bytes()[end].is_ascii_alphanumeric() || source.as_bytes()[end] == b'_')
    {
        end += 1;
    }
    source[offset..end].to_owned()
}

fn offset_for_position(source: &str, position: Position) -> usize {
    let mut offset = 0;
    for (line, text) in source.split_inclusive('\n').enumerate() {
        if line == position.line as usize {
            let mut units = 0;
            for (index, character) in text.trim_end_matches('\n').char_indices() {
                if units >= position.character {
                    return offset + index;
                }
                units += character.len_utf16() as u32;
            }
            return offset + text.trim_end_matches('\n').len();
        }
        offset += text.len();
    }
    source.len()
}

fn position_for_offset(source: &str, offset: usize) -> Position {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let column = prefix
        .rsplit('\n')
        .next()
        .unwrap_or_default()
        .encode_utf16()
        .count() as u32;
    Position {
        line,
        character: column,
    }
}

fn span(source: &str, start: usize, end: usize) -> Range {
    Range {
        start: position_for_offset(source, start),
        end: position_for_offset(source, end),
    }
}
fn lines_with_offsets(source: &str) -> impl Iterator<Item = (usize, &str)> {
    source.split_inclusive('\n').scan(0, |offset, line| {
        let start = *offset;
        *offset += line.len();
        Some((start, line.trim_end_matches('\n')))
    })
}

fn collect_files(root: &Path, output: &mut Vec<PathBuf>) -> io::Result<()> {
    if root.is_dir() {
        for entry in fs::read_dir(root)? {
            let path = entry?.path();
            if path.is_dir() {
                collect_files(&path, output)?;
            } else if path.extension().is_some_and(|extension| {
                matches!(extension.to_str(), Some("studio" | "ts" | "json"))
            }) {
                output.push(path);
            }
        }
    }
    Ok(())
}

fn path_to_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
}
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    uri.strip_prefix("file://").map(PathBuf::from)
}

const CATALOG_COMPONENTS: &[&str] = &[
    "Box",
    "Column",
    "Row",
    "Stack",
    "Grid",
    "ScrollView",
    "ListView",
    "Spacer",
    "Divider",
    "Text",
    "Icon",
    "Image",
    "Card",
    "Badge",
    "Tag",
    "Avatar",
    "Empty",
    "Skeleton",
    "ProgressIndicator",
    "ProgressCircle",
    "Spinner",
    "Button",
    "IconButton",
    "Checkbox",
    "Radio",
    "Switch",
    "Toggle",
    "ButtonGroup",
    "Slider",
    "RangeSlider",
    "Select",
    "Combobox",
    "NumberInput",
    "TextInput",
    "TextArea",
    "Field",
    "InputGroup",
    "OtpInput",
    "SecretInput",
    "Dialog",
    "AlertDialog",
    "Popover",
    "Sheet",
    "BottomSheet",
    "Drawer",
    "Toast",
    "Notification",
    "Banner",
    "ContextMenu",
    "CommandPalette",
    "Tooltip",
    "Scaffold",
    "AppBar",
    "Sidebar",
    "NavigationBar",
    "NavigationRail",
    "Tabs",
    "Breadcrumb",
    "Stepper",
    "Pagination",
    "ListTile",
    "SearchableList",
    "VirtualList",
    "DataTable",
    "Tree",
    "DescriptionList",
    "Calendar",
    "DatePicker",
    "TimePicker",
    "Separator",
    "Accordion",
    "Collapsible",
    "HoverCard",
    "MenuBar",
    "StatusBar",
    "KeyboardShortcuts",
    "Kbd",
    "ColorPicker",
    "Rating",
    "Resizable",
    "Dock",
    "Chart",
    "Editor",
    "RichText",
    "Carousel",
    "DragDrop",
    "Theme",
    "AspectRatio",
    "Alert",
    "Attachment",
    "Bubble",
    "Command",
    "NativeSelect",
    "NavigationMenu",
    "ScrollArea",
    "Item",
    "Message",
    "MessageScroller",
    "ToggleGroup",
    "Sonner",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_and_navigation_are_deterministic() {
        let source = "<Button id=\"save\" />\n<Text value={token.brand.primary} />";
        let mut workspace = Workspace::new();
        workspace.add_token(
            "brand.primary",
            "Color",
            Some("Primary brand color".to_owned()),
        );
        workspace.add_file("file:///main.studio", source);
        let server = LanguageServer::new(workspace);
        let completion = server.completion(
            "file:///main.studio",
            Position {
                line: 1,
                character: 31,
            },
        );
        assert!(
            completion
                .items
                .iter()
                .any(|item| item.label == "brand.primary")
        );
        assert_eq!(
            server
                .definition(
                    "file:///main.studio",
                    Position {
                        line: 1,
                        character: 28
                    }
                )
                .len(),
            1
        );
    }
}
