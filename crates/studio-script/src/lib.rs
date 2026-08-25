//! The parser of record and canonical printer for Studio Script.
//!
//! Studio Script is the canonical, deliberately closed textual form of a
//! Studio Design screen.  This crate owns the syntax and semantic model used
//! by the Designer, the compiler boundary, and the command-line tools.  It
//! does not depend on GPUI, `SurrealDB`, Tree-sitter, or a compiler backend.
//!
//! # Grammar
//!
//! A source file is UTF-8 and starts with the version header `studio 1`.
//! Whitespace and HTML comments may appear around the header.  The canonical
//! printer emits two-space indentation and normalizes all non-comment
//! whitespace.
//!
//! ```text
//! document       = trivia* header trivia* script? trivia* element* trivia* ;
//! header         = "studio" wsp "1" line_end ;
//! script         = "<script" wsp script_attr+ ">" script_body "</script>" ;
//! script_attr    = "lang" "=" string
//!                | "context" "=" ("instance" | "module") ;
//! element        = "<" name attribute* ("/>" | ">" content "</" name ">") ;
//! content        = (trivia | comment | element | text)* ;
//! attribute      = name ("=" (string | expression))? ;
//! expression     = "{" binding | token | literal "}" ;
//! binding        = "$item." path ;
//! token          = ("token." | "$token." | "@") path ;
//! literal        = boolean | number | quoted_string ;
//! comment        = "<!--" (!"-->")* "-->" ;
//! text           = (!"<")+ ;
//! name           = name_start name_continue* ;
//! path           = segment ("." segment)* ;
//! ```
//!
//! The script block uses `lang="studio"`; this is the Studio Script language
//! itself, not TypeScript.  Its body is intentionally opaque until the typed
//! IR/lowering seam consumes it.
//!
//! `id` is a reserved attribute and is mandatory on every element.  A node
//! identity is canonical lower kebab case (`hero`, `checkout-submit`, or
//! `item-2`); duplicate, missing, or non-canonical identities are errors.
//! Element names and attribute names are intentionally lexical rather than a
//! hard-coded component catalog so plugin-provided catalog kinds can use the
//! same parser.  Catalog/schema admission is a later semantic seam.
//!
//! Attribute expressions are bounded.  Binding paths must start with
//! `$item.` and contain at most [`ParseOptions::max_path_segments`] segments;
//! token references use `token.foo.bar`, `$token.foo.bar`, or `@foo.bar`.
//! Arbitrary JavaScript, CSS, HTML entities, interpolation, and database
//! expressions are not part of this grammar.  Script content is opaque to the
//! parser but is bounded and only the `studio` language is admitted in v1.
//!
//! Comments are stored as trivia on the following element or text node.  A
//! comment immediately before a closing tag is retained as trailing trivia on
//! that element; comments at document boundaries are retained on the document.
//! The printer emits comments in their stored order and canonicalizes every
//! other whitespace choice.
//!
//! The parser applies resource limits before allocating untrusted structures.
//! [`ParseOptions`] can be tightened by callers processing hostile or
//! user-supplied fixtures.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The only Studio Script syntax version currently admitted by this crate.
pub const STUDIO_SCRIPT_VERSION: u16 = 1;

/// The default source-size limit in bytes.
pub const DEFAULT_MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;

/// The default maximum number of element nodes in a document.
pub const DEFAULT_MAX_NODES: usize = 10_000;

/// The default maximum element nesting depth.
pub const DEFAULT_MAX_DEPTH: usize = 128;

/// The default maximum script body size in bytes.
pub const DEFAULT_MAX_SCRIPT_BYTES: usize = 512 * 1024;

/// Stable diagnostic code for a malformed or unsupported version header.
pub const CODE_VERSION: &str = "STUDIO001";
/// Stable diagnostic code for a missing node identity.
pub const CODE_MISSING_NODE_ID: &str = "STUDIO002";
/// Stable diagnostic code for a duplicate node identity.
pub const CODE_DUPLICATE_NODE_ID: &str = "STUDIO003";
/// Stable diagnostic code for a non-canonical node identity.
pub const CODE_NON_CANONICAL_NODE_ID: &str = "STUDIO004";
/// Stable diagnostic code for malformed source syntax.
pub const CODE_SYNTAX: &str = "STUDIO005";
/// Stable diagnostic code for a closed-grammar construct that is not known.
pub const CODE_UNKNOWN_CONSTRUCT: &str = "STUDIO006";
/// Stable diagnostic code for an invalid bounded binding path.
pub const CODE_BINDING: &str = "STUDIO007";
/// Stable diagnostic code for an invalid token reference.
pub const CODE_TOKEN: &str = "STUDIO008";
/// Stable diagnostic code for invalid or duplicated script blocks.
pub const CODE_SCRIPT: &str = "STUDIO009";
/// Stable diagnostic code for a parser resource limit.
pub const CODE_LIMIT: &str = "STUDIO010";
/// Stable diagnostic code for mismatched element tags.
pub const CODE_TAG_MISMATCH: &str = "STUDIO011";
/// Stable diagnostic code for a duplicate attribute.
pub const CODE_DUPLICATE_ATTRIBUTE: &str = "STUDIO012";
/// Stable diagnostic code for an empty document with no element roots.
pub const CODE_NO_ROOT: &str = "STUDIO013";
/// Stable diagnostic code for source that is valid but not canonically printed.
pub const CODE_NON_CANONICAL_FORMAT: &str = "STUDIO014";

/// Errors reported while preparing a source file for an older compiler
/// adapter boundary.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    /// The source contains more than one supported script block.
    #[error("{filename} contains more than one <script> block")]
    MultipleScriptBlocks {
        /// The source file containing the duplicate blocks.
        filename: PathBuf,
    },

    /// The closing script tag is missing.
    #[error("{filename} contains an unterminated <script> block")]
    UnterminatedScriptBlock {
        /// The source file containing the unterminated block.
        filename: PathBuf,
    },

    /// A closing script tag appears without an opening script tag.
    #[error("{filename} contains a closing </script> without an opening <script>")]
    UnexpectedScriptClose {
        /// The source file containing the unexpected closing tag.
        filename: PathBuf,
    },
}

/// The intended output backend for a future Studio Script transform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Development or web output executed by a JavaScript runtime.
    JavaScript,
    /// Production output that will be lowered to `AssemblyScript` and Wasm.
    AssemblyScript,
}

/// The source blocks extracted from a `.studio` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBlocks {
    /// The contents of the optional `<script>` block.
    pub script: Option<String>,
    /// Markup outside the `<script>` block.
    pub markup: String,
}

/// A source file prepared for a compiler backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSource {
    /// The source filename used for diagnostics and module identity.
    pub filename: PathBuf,
    /// The selected output target.
    pub target: Target,
    /// The separated source blocks.
    pub blocks: SourceBlocks,
}

/// Resource limits for parsing untrusted Studio Script input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseOptions {
    /// Maximum source length in bytes.
    pub max_source_bytes: usize,
    /// Maximum number of element nodes.
    pub max_nodes: usize,
    /// Maximum nesting depth.
    pub max_depth: usize,
    /// Maximum number of attributes on one element.
    pub max_attributes: usize,
    /// Maximum UTF-8 bytes in one attribute value.
    pub max_attribute_bytes: usize,
    /// Maximum UTF-8 bytes in one script body.
    pub max_script_bytes: usize,
    /// Maximum UTF-8 bytes in one text node.
    pub max_text_bytes: usize,
    /// Maximum UTF-8 bytes in one comment.
    pub max_comment_bytes: usize,
    /// Maximum path segments after `$item.` or a token prefix.
    pub max_path_segments: usize,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            max_source_bytes: DEFAULT_MAX_SOURCE_BYTES,
            max_nodes: DEFAULT_MAX_NODES,
            max_depth: DEFAULT_MAX_DEPTH,
            max_attributes: 64,
            max_attribute_bytes: 16 * 1024,
            max_script_bytes: DEFAULT_MAX_SCRIPT_BYTES,
            max_text_bytes: 256 * 1024,
            max_comment_bytes: 64 * 1024,
            max_path_segments: 8,
        }
    }
}

/// A source location using one-based line and column numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    /// One-based line number.
    pub line: usize,
    /// One-based UTF-8 character column number.
    pub column: usize,
    /// Zero-based byte offset from the beginning of the source.
    pub offset: usize,
}

/// A source span associated with a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Inclusive start location.
    pub start: Location,
    /// Exclusive end location.
    pub end: Location,
}

impl Span {
    const fn point(location: Location) -> Self {
        Self {
            start: location,
            end: location,
        }
    }
}

/// Severity of a parser-of-record diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The source cannot be accepted.
    Error,
    /// A non-fatal diagnostic (reserved for future lint policy layers).
    Warning,
}

/// A stable, source-linked diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Stable machine-readable code.
    pub code: &'static str,
    /// Severity of the diagnostic.
    pub severity: Severity,
    /// Human-readable safe message.
    pub message: String,
    /// Source span, when a source location exists.
    pub span: Span,
}

impl Diagnostic {
    fn error(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            span,
        }
    }
}

/// A parse or semantic-validation failure containing all diagnostics found.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Studio Script contains {count} diagnostic(s)", count = diagnostics.len())]
pub struct ParseError {
    /// Stable diagnostics, ordered by source discovery.
    pub diagnostics: Vec<Diagnostic>,
}

impl ParseError {
    /// Return the diagnostics without exposing parser internals.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// A comment retained as source trivia.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Comment {
    /// The comment body without `<!--` and `-->` delimiters.
    pub text: String,
}

impl Comment {
    /// Construct a comment trivia value.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// A bounded `$item.foo.bar` binding path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingPath {
    /// The canonical path including the `$item.` prefix.
    pub path: String,
}

/// A bounded token reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenRef {
    /// The canonical path including the `token.` prefix.
    pub path: String,
}

/// A closed Studio Script attribute value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    /// A quoted UTF-8 string.
    String(String),
    /// A bare boolean attribute or `{true}`/`{false}`.
    Boolean(bool),
    /// A bounded decimal integer or floating-point literal, retained in
    /// canonical lexical form.
    Number(String),
    /// A bounded repeated-content binding.
    Binding(BindingPath),
    /// A design-token reference.
    Token(TokenRef),
}

/// A Studio Script `<script>` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptBlock {
    /// Trivia immediately before this block.
    pub leading_comments: Vec<Comment>,
    /// The admitted script language (`studio` in v1).
    pub lang: String,
    /// Whether the block is an instance or module script.
    pub context: ScriptContext,
    /// Opaque script source, normalized for line endings and outer newlines.
    pub content: String,
}

/// Context of a Studio Script block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptContext {
    /// Component instance script (the default).
    Instance,
    /// Module-level script.
    Module,
}

/// A parsed text leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextNode {
    /// Trivia immediately before this text leaf.
    pub leading_comments: Vec<Comment>,
    /// Canonicalized text content.
    pub text: String,
}

/// A parsed nested child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// A nested element.
    Element(Element),
    /// A text leaf.
    Text(TextNode),
}

impl Node {
    /// Construct an element child.
    #[must_use]
    pub fn element(element: Element) -> Self {
        Self::Element(element)
    }

    /// Construct a text child with no comments.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(TextNode {
            leading_comments: Vec::new(),
            text: text.into(),
        })
    }
}

/// A parsed Studio Script element.  `id` is intentionally separate from the
/// attribute map because it is the semantic identity of the element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    /// Trivia immediately before this element.
    pub leading_comments: Vec<Comment>,
    /// Catalog or plugin component kind.
    pub kind: String,
    /// Stable node identity.
    pub id: String,
    /// Non-identity attributes in deterministic key order.
    pub attributes: BTreeMap<String, AttributeValue>,
    /// Ordered nested content.
    pub children: Vec<Node>,
    /// Trivia found before this element's closing tag.
    pub trailing_comments: Vec<Comment>,
}

impl Element {
    /// Construct an element with no attributes or children.
    #[must_use]
    pub fn new(kind: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            leading_comments: Vec::new(),
            kind: kind.into(),
            id: id.into(),
            attributes: BTreeMap::new(),
            children: Vec::new(),
            trailing_comments: Vec::new(),
        }
    }

    /// Add or replace a non-identity attribute.
    pub fn set_attribute(&mut self, name: impl Into<String>, value: AttributeValue) {
        self.attributes.insert(name.into(), value);
    }
}

/// The semantic model produced by the parser of record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioDocument {
    /// Closed grammar version.
    pub version: u16,
    /// Comments before the first document item or before the version header.
    pub leading_comments: Vec<Comment>,
    /// Optional typed script block.
    pub script: Option<ScriptBlock>,
    /// Ordered top-level elements.
    pub nodes: Vec<Element>,
    /// Comments at end of the document with no following node.
    pub trailing_comments: Vec<Comment>,
}

impl Default for StudioDocument {
    fn default() -> Self {
        Self {
            version: STUDIO_SCRIPT_VERSION,
            leading_comments: Vec::new(),
            script: None,
            nodes: Vec::new(),
            trailing_comments: Vec::new(),
        }
    }
}

impl StudioDocument {
    /// Construct an empty v1 document.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

/// Short alias for callers that prefer the grammar name.
pub type Document = StudioDocument;

/// Prepare a Studio Script source file for a compiler backend.
///
/// This legacy source-block splitter remains intentionally independent of the
/// parser of record.  Compiler adapters that only need to inspect a script
/// header can continue to use it while the full frontend uses [`parse`].
///
/// # Errors
///
/// Returns [`Error`] when the source contains malformed, duplicate, or
/// unexpected script tags.
pub fn prepare(
    path: impl AsRef<Path>,
    source: &str,
    target: Target,
) -> Result<PreparedSource, Error> {
    let filename = path.as_ref().to_path_buf();
    let blocks = split_source(&filename, source)?;

    Ok(PreparedSource {
        filename,
        target,
        blocks,
    })
}

/// Parse a bounded v1 Studio Script source file into its semantic model.
///
/// # Errors
///
/// Returns [`ParseError`] with stable, source-linked diagnostics when the
/// source is syntactically invalid, violates the closed grammar, or exceeds a
/// resource limit.
pub fn parse(source: &str) -> Result<StudioDocument, ParseError> {
    parse_with_options(source, ParseOptions::default())
}

/// Alias for [`parse`] for call sites that use the grammar terminology.
///
/// # Errors
///
/// Returns the same bounded parser diagnostics as [`parse`].
pub fn parse_document(source: &str) -> Result<StudioDocument, ParseError> {
    parse(source)
}

/// Parse Studio Script with explicit hostile-input limits.
///
/// # Errors
///
/// As with [`parse`], all returned diagnostics are safe to display and use
/// one-based line and column locations.
pub fn parse_with_options(
    source: &str,
    options: ParseOptions,
) -> Result<StudioDocument, ParseError> {
    let mut parser = Parser::new(source, options);
    parser.parse_document()
}

/// Validate a manually constructed semantic model.
///
/// Parsed source is already validated.  This function is useful to callers
/// that construct a model through commands or tests before printing it.
#[must_use]
pub fn validate(document: &StudioDocument) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let point = Span::point(Location {
        line: 1,
        column: 1,
        offset: 0,
    });

    if document.version != STUDIO_SCRIPT_VERSION {
        diagnostics.push(Diagnostic::error(
            CODE_VERSION,
            format!("unsupported Studio Script version {}", document.version),
            point,
        ));
    }

    let mut ids = BTreeSet::new();
    for element in &document.nodes {
        validate_element(element, &mut ids, &mut diagnostics, point);
    }
    diagnostics
}

/// Print a semantic model in deterministic canonical Studio Script form.
///
/// Models are validated before printing.  Invalid manually constructed models
/// are still printed deterministically; callers that need rejection should
/// call [`validate`] first.
#[must_use]
pub fn print(document: &StudioDocument) -> String {
    let mut output = String::new();
    let version = if document.version == 0 {
        STUDIO_SCRIPT_VERSION
    } else {
        document.version
    };
    write_comments(&mut output, 0, &document.leading_comments);
    let _ = writeln!(output, "studio {version}");

    if let Some(script) = &document.script {
        write_comments(&mut output, 0, &script.leading_comments);
        write_script(&mut output, script);
    }
    for element in &document.nodes {
        write_element(&mut output, element, 0);
    }
    write_comments(&mut output, 0, &document.trailing_comments);
    output
}

/// Alias for [`print`] that reads naturally at call sites.
#[must_use]
pub fn print_document(document: &StudioDocument) -> String {
    print(document)
}

/// Alias for [`print`] emphasizing that the result is canonical source.
#[must_use]
pub fn canonical_print(document: &StudioDocument) -> String {
    print(document)
}

/// Parse and print a source file in canonical form.
///
/// # Errors
///
/// Returns the parser diagnostics when the source is not accepted.
pub fn format(source: &str) -> Result<String, ParseError> {
    Ok(print(&parse(source)?))
}

/// Alias for [`format`].
///
/// # Errors
///
/// Returns parser diagnostics when `source` is not valid Studio Script.
pub fn canonicalize(source: &str) -> Result<String, ParseError> {
    format(source)
}

fn validate_element(
    element: &Element,
    ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    point: Span,
) {
    if element.id.is_empty() {
        diagnostics.push(Diagnostic::error(
            CODE_MISSING_NODE_ID,
            format!("element <{}> is missing a stable node id", element.kind),
            point,
        ));
    } else {
        if !is_canonical_id(&element.id) {
            diagnostics.push(Diagnostic::error(
                CODE_NON_CANONICAL_NODE_ID,
                format!("node id {:?} is not canonical lower kebab case", element.id),
                point,
            ));
        }
        if !ids.insert(element.id.clone()) {
            diagnostics.push(Diagnostic::error(
                CODE_DUPLICATE_NODE_ID,
                format!("node id {:?} is duplicated", element.id),
                point,
            ));
        }
    }
    for child in &element.children {
        if let Node::Element(child) = child {
            validate_element(child, ids, diagnostics, point);
        }
    }
}

fn write_comments(output: &mut String, indent: usize, comments: &[Comment]) {
    for comment in comments {
        write_indent(output, indent);
        let text = normalize_comment(&comment.text);
        output.push_str("<!--");
        output.push_str(&text);
        let _ = writeln!(output, "-->");
    }
}

fn write_script(output: &mut String, script: &ScriptBlock) {
    write!(output, "<script lang={}", quote(&script.lang)).expect("writing to String cannot fail");
    if script.context == ScriptContext::Module {
        output.push_str(" context=\"module\"");
    }
    output.push_str(">\n");
    let content = normalize_script(&script.content);
    if !content.is_empty() {
        output.push_str(&content);
        output.push('\n');
    }
    output.push_str("</script>\n");
}

fn write_element(output: &mut String, element: &Element, indent: usize) {
    write_comments(output, indent, &element.leading_comments);
    write_indent(output, indent);
    write!(output, "<{} id={}", element.kind, quote(&element.id))
        .expect("writing to String cannot fail");
    for (name, value) in &element.attributes {
        write!(output, " {name}={}", print_value(value)).expect("writing to String cannot fail");
    }

    if element.children.is_empty() && element.trailing_comments.is_empty() {
        output.push_str(" />\n");
        return;
    }

    output.push_str(">\n");
    for child in &element.children {
        match child {
            Node::Element(child) => write_element(output, child, indent + 1),
            Node::Text(text) => {
                write_comments(output, indent + 1, &text.leading_comments);
                write_indent(output, indent + 1);
                let _ = writeln!(output, "{}", normalize_text(&text.text));
            }
        }
    }
    write_comments(output, indent + 1, &element.trailing_comments);
    write_indent(output, indent);
    let _ = writeln!(output, "</{}>", element.kind);
}

fn write_indent(output: &mut String, indent: usize) {
    for _ in 0..indent {
        output.push_str("  ");
    }
}

fn print_value(value: &AttributeValue) -> String {
    match value {
        AttributeValue::String(value) => quote(value),
        AttributeValue::Boolean(value) => format!("{{{value}}}"),
        AttributeValue::Number(value) => format!("{{{value}}}"),
        AttributeValue::Binding(binding) => format!("{{{}}}", binding.path),
        AttributeValue::Token(token) => format!("{{{}}}", token.path),
    }
}

fn quote(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for character in value.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            character if character.is_control() => quoted.push('\u{fffd}'),
            character => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}

fn normalize_comment(comment: &str) -> String {
    comment.replace("\r\n", "\n").replace('\r', "\n")
}

fn normalize_script(script: &str) -> String {
    script
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_matches('\n')
        .to_owned()
}

fn normalize_text(text: &str) -> String {
    let mut normalized = String::new();
    let mut pending_space = false;
    for character in text.chars() {
        if character.is_whitespace() {
            pending_space = !normalized.is_empty();
            continue;
        }
        if pending_space && !normalized.is_empty() {
            normalized.push(' ');
        }
        pending_space = false;
        normalized.push(character);
    }
    normalized
}

fn is_canonical_id(id: &str) -> bool {
    let mut previous_dash = false;
    let mut segment_has_character = false;
    for (index, character) in id.chars().enumerate() {
        match character {
            'a'..='z' | '0'..='9' if index > 0 || character.is_ascii_lowercase() => {
                segment_has_character = true;
                previous_dash = false;
            }
            '-' if segment_has_character && !previous_dash => {
                previous_dash = true;
                segment_has_character = false;
            }
            _ => return false,
        }
    }
    segment_has_character && !previous_dash
}

fn is_name_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

fn is_name_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | ':' | '.')
}

fn is_path_segment(segment: &str) -> bool {
    let mut characters = segment.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn canonical_number(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let mut index = usize::from(bytes[0] == b'-');
    if index == 1 && bytes.len() == 1 {
        return None;
    }
    let mut digits_before = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        digits_before += 1;
        index += 1;
    }
    let mut digits_after = 0;
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            digits_after += 1;
            index += 1;
        }
    }
    if index != bytes.len() || (digits_before == 0 && digits_after == 0) {
        return None;
    }
    let mut result = value.to_owned();
    if let Some(dot) = result.find('.') {
        while result.ends_with('0') {
            result.pop();
        }
        if result.ends_with('.') {
            result.pop();
        }
        if dot == 0 {
            result.insert(0, '0');
        } else if dot == 1 && result.starts_with('-') {
            result.insert(1, '0');
        }
    }
    if result.starts_with('-') {
        let digits_start = 1;
        let mut end = digits_start;
        while end < result.len() && result.as_bytes()[end].is_ascii_digit() {
            end += 1;
        }
        let digits = result[digits_start..end].trim_start_matches('0');
        let digits = if digits.is_empty() { "0" } else { digits };
        result = format!("-{digits}{}", &result[end..]);
    } else {
        let mut end = 0;
        while end < result.len() && result.as_bytes()[end].is_ascii_digit() {
            end += 1;
        }
        let digits = result[..end].trim_start_matches('0');
        let digits = if digits.is_empty() { "0" } else { digits };
        result = format!("{digits}{}", &result[end..]);
    }
    Some(result)
}

struct Parser<'a> {
    source: &'a str,
    bytes: &'a [u8],
    position: usize,
    options: ParseOptions,
    nodes: usize,
    diagnostics: Vec<Diagnostic>,
    ids: BTreeSet<String>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, options: ParseOptions) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            position: 0,
            options,
            nodes: 0,
            diagnostics: Vec::new(),
            ids: BTreeSet::new(),
        }
    }

    fn parse_document(&mut self) -> Result<StudioDocument, ParseError> {
        if self.source.len() > self.options.max_source_bytes {
            self.push_error(
                CODE_LIMIT,
                format!(
                    "source exceeds the {} byte parser limit",
                    self.options.max_source_bytes
                ),
                0,
            );
            return Err(self.finish_error());
        }

        let leading_comments = self.parse_boundary_comments();
        self.skip_whitespace();
        let version = self.parse_header();

        let mut document = StudioDocument {
            version,
            leading_comments,
            script: None,
            nodes: Vec::new(),
            trailing_comments: Vec::new(),
        };
        let mut pending_comments = Vec::new();

        while !self.at_end() {
            if self.skip_whitespace() {
                continue;
            }
            if self.starts_with("<!--") {
                pending_comments.push(self.parse_comment());
                continue;
            }
            if self.starts_with("<script") {
                if document.script.is_some() {
                    self.push_error(
                        CODE_SCRIPT,
                        "a document may contain only one <script> block",
                        self.position,
                    );
                    self.skip_until_tag_end();
                } else {
                    document.script =
                        Some(self.parse_script(std::mem::take(&mut pending_comments)));
                }
                continue;
            }
            if self.starts_with("</") {
                self.push_error(CODE_SYNTAX, "unexpected closing element", self.position);
                self.skip_until_tag_end();
                continue;
            }
            if self.starts_with("<") {
                let element = self.parse_element(std::mem::take(&mut pending_comments), 0);
                document.nodes.push(element);
                continue;
            }
            self.push_error(
                CODE_SYNTAX,
                "unexpected content outside an element",
                self.position,
            );
            self.skip_until_markup();
        }
        document.trailing_comments = pending_comments;

        if document.nodes.is_empty() && document.script.is_none() {
            self.push_error(
                CODE_NO_ROOT,
                "document must contain an element or script block",
                self.position,
            );
        }

        if self.diagnostics.is_empty() {
            Ok(document)
        } else {
            Err(self.finish_error())
        }
    }

    fn parse_header(&mut self) -> u16 {
        let header_start = self.position;
        if !self.consume_word("studio") {
            self.push_error(
                CODE_VERSION,
                "Studio Script must start with the `studio 1` version header",
                header_start,
            );
            return STUDIO_SCRIPT_VERSION;
        }
        if !self.consume_space() {
            self.push_error(
                CODE_VERSION,
                "expected a version after `studio`",
                header_start,
            );
            return STUDIO_SCRIPT_VERSION;
        }

        let version_start = self.position;
        let version = if self.consume_word("version") {
            self.skip_tag_whitespace();
            if self.consume_byte(b'=') {
                self.skip_spaces();
                self.parse_quoted_raw()
                    .and_then(|value| value.parse::<u16>().ok())
            } else {
                self.push_error(CODE_VERSION, "expected `=` after `version`", version_start);
                None
            }
        } else {
            self.parse_digits()
        };
        self.skip_spaces();
        if !self.consume_line_end_or_eof() {
            self.push_error(
                CODE_VERSION,
                "unexpected text in Studio Script version header",
                self.position,
            );
            self.skip_until_line_end();
        }
        match version {
            Some(STUDIO_SCRIPT_VERSION) => STUDIO_SCRIPT_VERSION,
            Some(version) => {
                self.push_error(
                    CODE_VERSION,
                    format!("unsupported Studio Script version {version}"),
                    version_start,
                );
                version
            }
            None => {
                self.push_error(
                    CODE_VERSION,
                    "Studio Script version must be an integer",
                    version_start,
                );
                STUDIO_SCRIPT_VERSION
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn parse_script(&mut self, leading_comments: Vec<Comment>) -> ScriptBlock {
        let script_start = self.position;
        self.position += "<script".len();
        let mut attrs = BTreeMap::new();
        loop {
            self.skip_tag_whitespace();
            if self.consume_byte(b'>') {
                break;
            }
            if self.at_end() {
                self.push_error(
                    CODE_SCRIPT,
                    "unterminated <script> opening tag",
                    script_start,
                );
                return ScriptBlock {
                    leading_comments,
                    lang: "studio".to_owned(),
                    context: ScriptContext::Instance,
                    content: String::new(),
                };
            }
            if self.starts_with("/>") {
                self.push_error(
                    CODE_SCRIPT,
                    "<script> cannot be self-closing",
                    self.position,
                );
                self.position += 2;
                break;
            }
            let Some(name) = self.parse_name() else {
                self.push_error(
                    CODE_SCRIPT,
                    "invalid <script> header attribute",
                    self.position,
                );
                self.skip_until_tag_end();
                break;
            };
            self.skip_tag_whitespace();
            if !self.consume_byte(b'=') {
                self.push_error(
                    CODE_SCRIPT,
                    "script attributes require a value",
                    self.position,
                );
                continue;
            }
            self.skip_tag_whitespace();
            let Some(value) = self.parse_quoted_raw() else {
                self.push_error(
                    CODE_SCRIPT,
                    "script attributes must use quoted values",
                    self.position,
                );
                self.skip_until_tag_end();
                break;
            };
            if attrs.insert(name.clone(), value).is_some() {
                self.push_error(
                    CODE_DUPLICATE_ATTRIBUTE,
                    format!("duplicate script attribute `{name}`"),
                    self.position,
                );
            }
        }

        let content_start = self.position;
        let Some(close_relative) = self.source[content_start..].find("</script>") else {
            self.push_error(CODE_SCRIPT, "unterminated <script> block", script_start);
            self.position = self.bytes.len();
            return ScriptBlock {
                leading_comments,
                lang: attrs.remove("lang").unwrap_or_else(|| "studio".to_owned()),
                context: parse_script_context(
                    attrs.get("context").map(String::as_str),
                    self,
                    script_start,
                ),
                content: normalize_script(&self.source[content_start..]),
            };
        };
        let close_start = content_start + close_relative;
        let script_body = &self.source[content_start..close_start];
        if script_body.len() > self.options.max_script_bytes {
            self.push_error(
                CODE_LIMIT,
                "script body exceeds the parser limit",
                content_start,
            );
        }
        self.position = close_start + "</script>".len();

        let lang = attrs.remove("lang");
        let raw_context = attrs.remove("context");
        if !attrs.is_empty() {
            for name in attrs.keys() {
                self.push_error(
                    CODE_UNKNOWN_CONSTRUCT,
                    format!("unknown <script> attribute `{name}`"),
                    script_start,
                );
            }
        }
        let lang = match lang {
            Some(lang) if lang == "studio" => lang,
            Some(lang) => {
                self.push_error(
                    CODE_SCRIPT,
                    format!("unsupported script language `{lang}`; v1 admits `studio`"),
                    script_start,
                );
                lang
            }
            None => {
                self.push_error(
                    CODE_SCRIPT,
                    "<script> requires lang=\"studio\"",
                    script_start,
                );
                "studio".to_owned()
            }
        };
        let context = parse_script_context(raw_context.as_deref(), self, script_start);
        ScriptBlock {
            leading_comments,
            lang,
            context,
            content: normalize_script(script_body),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn parse_element(&mut self, leading_comments: Vec<Comment>, depth: usize) -> Element {
        let element_start = self.position;
        self.position += 1;
        let kind = self.parse_name().unwrap_or_else(|| {
            self.push_error(CODE_SYNTAX, "expected an element name", element_start);
            "Invalid".to_owned()
        });
        if matches!(kind.as_str(), "script" | "studio") {
            self.push_error(
                CODE_UNKNOWN_CONSTRUCT,
                format!("`{kind}` is reserved and cannot be an element"),
                element_start,
            );
        }

        self.nodes += 1;
        if self.nodes > self.options.max_nodes {
            self.push_error(
                CODE_LIMIT,
                "document exceeds the element-node limit",
                element_start,
            );
        }
        if depth >= self.options.max_depth {
            self.push_error(
                CODE_LIMIT,
                "document exceeds the element nesting limit",
                element_start,
            );
        }

        let mut attributes = BTreeMap::new();
        let mut id: Option<(String, usize)> = None;
        let mut attribute_count = 0;
        let self_closing = loop {
            self.skip_tag_whitespace();
            if self.consume_byte(b'>') {
                break false;
            }
            if self.starts_with("/>") {
                self.position += 2;
                break true;
            }
            if self.at_end() {
                self.push_error(
                    CODE_SYNTAX,
                    format!("unterminated <{kind}> opening tag"),
                    element_start,
                );
                break true;
            }
            let attribute_start = self.position;
            let Some(name) = self.parse_name() else {
                self.push_error(CODE_SYNTAX, "expected an attribute name", self.position);
                self.skip_until_tag_end();
                break true;
            };
            attribute_count += 1;
            if attribute_count > self.options.max_attributes {
                self.push_error(
                    CODE_LIMIT,
                    "element exceeds the attribute limit",
                    attribute_start,
                );
            }
            self.skip_tag_whitespace();
            let value = if self.consume_byte(b'=') {
                self.skip_tag_whitespace();
                self.parse_attribute_value(attribute_start)
            } else {
                AttributeValue::Boolean(true)
            };
            if name == "id" {
                match value {
                    AttributeValue::String(value) => {
                        if id.is_some() {
                            self.push_error(
                                CODE_DUPLICATE_ATTRIBUTE,
                                "duplicate `id` attribute",
                                attribute_start,
                            );
                        } else {
                            id = Some((value, attribute_start));
                        }
                    }
                    _ => self.push_error(
                        CODE_MISSING_NODE_ID,
                        "node `id` must be a quoted string",
                        attribute_start,
                    ),
                }
            } else if attributes.insert(name.clone(), value).is_some() {
                self.push_error(
                    CODE_DUPLICATE_ATTRIBUTE,
                    format!("duplicate attribute `{name}`"),
                    attribute_start,
                );
            }
        };

        let (id, id_start) = id.unwrap_or_else(|| {
            self.push_error(
                CODE_MISSING_NODE_ID,
                format!("element <{kind}> is missing a stable node id"),
                element_start,
            );
            (format!("invalid-node-{}", self.nodes), element_start)
        });
        if !is_canonical_id(&id) {
            self.push_error(
                CODE_NON_CANONICAL_NODE_ID,
                format!("node id `{id}` is not canonical lower kebab case"),
                id_start,
            );
        }
        if !self.ids.insert(id.clone()) {
            self.push_error(
                CODE_DUPLICATE_NODE_ID,
                format!("node id `{id}` is duplicated"),
                id_start,
            );
        }

        let mut element = Element {
            leading_comments,
            kind: kind.clone(),
            id,
            attributes,
            children: Vec::new(),
            trailing_comments: Vec::new(),
        };
        if self_closing {
            return element;
        }
        if depth >= self.options.max_depth || self.nodes > self.options.max_nodes {
            // The opening tag has already been consumed and the diagnostic was
            // emitted above.  Stop scanning this subtree rather than recursing
            // through hostile input until the process stack is exhausted.
            self.position = self.bytes.len();
            return element;
        }

        let mut pending_comments = Vec::new();
        while !self.at_end() {
            if self.skip_whitespace() {
                continue;
            }
            if self.starts_with("<!--") {
                pending_comments.push(self.parse_comment());
                continue;
            }
            if self.starts_with("</") {
                if !pending_comments.is_empty() {
                    element.trailing_comments = std::mem::take(&mut pending_comments);
                }
                self.position += 2;
                let close_start = self.position;
                let close_name = self.parse_name().unwrap_or_default();
                self.skip_tag_whitespace();
                if !self.consume_byte(b'>') {
                    self.push_error(
                        CODE_SYNTAX,
                        "expected `>` after closing element",
                        self.position,
                    );
                    self.skip_until_tag_end();
                }
                if close_name != kind {
                    self.push_error(
                        CODE_TAG_MISMATCH,
                        format!("closing tag </{close_name}> does not match <{kind}>"),
                        close_start,
                    );
                }
                return element;
            }
            if self.starts_with("<") {
                let child = self.parse_element(std::mem::take(&mut pending_comments), depth + 1);
                element.children.push(Node::Element(child));
                continue;
            }
            let text_start = self.position;
            self.skip_until_markup();
            let raw_text = &self.source[text_start..self.position];
            if raw_text.len() > self.options.max_text_bytes {
                self.push_error(CODE_LIMIT, "text node exceeds the parser limit", text_start);
            }
            let text = normalize_text(raw_text);
            if !text.is_empty() {
                element.children.push(Node::Text(TextNode {
                    leading_comments: std::mem::take(&mut pending_comments),
                    text,
                }));
            }
        }
        self.push_error(
            CODE_SYNTAX,
            format!("element <{kind}> is missing a closing tag"),
            element_start,
        );
        element
    }

    fn parse_attribute_value(&mut self, start: usize) -> AttributeValue {
        if self.peek_byte() == Some(b'"') {
            return AttributeValue::String(self.parse_quoted(start));
        }
        if self.consume_byte(b'{') {
            let expression_start = self.position;
            let close = self.source[self.position..].find('}');
            let Some(close) = close else {
                self.push_error(
                    CODE_SYNTAX,
                    "unterminated attribute expression",
                    expression_start,
                );
                return AttributeValue::String(String::new());
            };
            let raw = self.source[self.position..self.position + close].trim();
            self.position += close + 1;
            if raw.len() > self.options.max_attribute_bytes {
                self.push_error(
                    CODE_LIMIT,
                    "attribute expression exceeds the parser limit",
                    start,
                );
            }
            return self.parse_expression(raw, expression_start);
        }
        self.push_error(
            CODE_SYNTAX,
            "attribute values must be quoted or bounded expressions",
            start,
        );
        self.skip_until_attribute_end();
        AttributeValue::String(String::new())
    }

    fn parse_expression(&mut self, raw: &str, start: usize) -> AttributeValue {
        if raw == "true" {
            return AttributeValue::Boolean(true);
        }
        if raw == "false" {
            return AttributeValue::Boolean(false);
        }
        if let Some(number) = canonical_number(raw) {
            return AttributeValue::Number(number);
        }
        if raw.starts_with("$item.") {
            let path = raw.to_owned();
            if valid_path(
                raw.strip_prefix("$item.").unwrap_or_default(),
                self.options.max_path_segments,
            ) {
                return AttributeValue::Binding(BindingPath { path });
            }
            self.push_error(
                CODE_BINDING,
                format!("invalid bounded binding path `{raw}`"),
                start,
            );
            return AttributeValue::String(String::new());
        }
        let token_path = if let Some(path) = raw.strip_prefix("token.") {
            Some(path)
        } else if let Some(path) = raw.strip_prefix("$token.") {
            Some(path)
        } else {
            raw.strip_prefix('@')
        };
        if let Some(path) = token_path {
            if valid_path(path, self.options.max_path_segments) {
                return AttributeValue::Token(TokenRef {
                    path: format!("token.{path}"),
                });
            }
            self.push_error(
                CODE_TOKEN,
                format!("invalid bounded token reference `{raw}`"),
                start,
            );
            return AttributeValue::String(String::new());
        }
        if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
            return AttributeValue::String(unescape_string(&raw[1..raw.len() - 1]));
        }
        self.push_error(
            CODE_UNKNOWN_CONSTRUCT,
            format!("unsupported attribute expression `{raw}`"),
            start,
        );
        AttributeValue::String(String::new())
    }

    fn parse_comment(&mut self) -> Comment {
        let start = self.position;
        self.position += "<!--".len();
        let Some(close) = self.source[self.position..].find("-->") else {
            self.push_error(CODE_SYNTAX, "unterminated comment", start);
            self.position = self.bytes.len();
            return Comment::default();
        };
        let content = &self.source[self.position..self.position + close];
        if content.len() > self.options.max_comment_bytes {
            self.push_error(CODE_LIMIT, "comment exceeds the parser limit", start);
        }
        self.position += close + "-->".len();
        Comment::new(normalize_comment(content))
    }

    fn parse_boundary_comments(&mut self) -> Vec<Comment> {
        let mut comments = Vec::new();
        loop {
            self.skip_whitespace();
            if self.starts_with("<!--") {
                comments.push(self.parse_comment());
            } else {
                break;
            }
        }
        comments
    }

    fn parse_name(&mut self) -> Option<String> {
        let start = self.position;
        let first = self.peek_char()?;
        if !is_name_start(first) {
            return None;
        }
        self.position += first.len_utf8();
        while let Some(character) = self.peek_char() {
            if !is_name_continue(character) {
                break;
            }
            self.position += character.len_utf8();
        }
        Some(self.source[start..self.position].to_owned())
    }

    fn parse_quoted(&mut self, start: usize) -> String {
        let Some(raw) = self.parse_quoted_raw() else {
            self.push_error(CODE_SYNTAX, "unterminated quoted attribute value", start);
            return String::new();
        };
        if raw.len() > self.options.max_attribute_bytes {
            self.push_error(
                CODE_LIMIT,
                "attribute value exceeds the parser limit",
                start,
            );
        }
        raw
    }

    fn parse_quoted_raw(&mut self) -> Option<String> {
        if !self.consume_byte(b'"') {
            return None;
        }
        let mut value = String::new();
        while let Some(character) = self.peek_char() {
            self.position += character.len_utf8();
            match character {
                '"' => return Some(value),
                '\\' => {
                    let escaped = self.peek_char()?;
                    self.position += escaped.len_utf8();
                    match escaped {
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),
                        '"' => value.push('"'),
                        '\\' => value.push('\\'),
                        _ => {
                            self.push_error(
                                CODE_SYNTAX,
                                "unsupported string escape",
                                self.position - escaped.len_utf8(),
                            );
                            value.push(escaped);
                        }
                    }
                }
                character if character.is_control() => {
                    self.push_error(
                        CODE_SYNTAX,
                        "control characters are not allowed in strings",
                        self.position - character.len_utf8(),
                    );
                }
                character => value.push(character),
            }
        }
        None
    }

    fn parse_digits(&mut self) -> Option<u16> {
        let start = self.position;
        while self.peek_byte().is_some_and(|byte| byte.is_ascii_digit()) {
            self.position += 1;
        }
        (self.position > start)
            .then(|| self.source[start..self.position].parse().ok())
            .flatten()
    }

    fn consume_word(&mut self, word: &str) -> bool {
        self.starts_with(word)
            .then(|| self.position += word.len())
            .is_some()
    }

    fn consume_space(&mut self) -> bool {
        let start = self.position;
        self.skip_spaces();
        self.position > start
    }

    fn skip_spaces(&mut self) {
        while matches!(self.peek_byte(), Some(b' ' | b'\t')) {
            self.position += 1;
        }
    }

    fn skip_tag_whitespace(&mut self) {
        while matches!(self.peek_byte(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.position += 1;
        }
    }

    fn skip_whitespace(&mut self) -> bool {
        let start = self.position;
        while self.peek_char().is_some_and(char::is_whitespace) {
            self.position += self.peek_char().map_or(0, char::len_utf8);
        }
        self.position > start
    }

    fn consume_line_end_or_eof(&mut self) -> bool {
        if self.at_end() {
            return true;
        }
        if self.consume_byte(b'\n') {
            return true;
        }
        if self.consume_byte(b'\r') {
            let _ = self.consume_byte(b'\n');
            return true;
        }
        false
    }

    fn skip_until_line_end(&mut self) {
        while let Some(byte) = self.peek_byte() {
            self.position += 1;
            if byte == b'\n' {
                break;
            }
        }
    }

    fn skip_until_markup(&mut self) {
        while !self.at_end() && !self.starts_with("<") {
            self.position += self.peek_char().map_or(1, char::len_utf8);
        }
    }

    fn skip_until_tag_end(&mut self) {
        while !self.at_end() {
            let byte = self.peek_byte().unwrap_or_default();
            self.position += 1;
            if byte == b'>' {
                break;
            }
        }
    }

    fn skip_until_attribute_end(&mut self) {
        while !self.at_end() {
            match self.peek_byte() {
                Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/') | None => break,
                _ => self.position += self.peek_char().map_or(1, char::len_utf8),
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.source[self.position..].chars().next()
    }

    fn peek_byte(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    fn consume_byte(&mut self, byte: u8) -> bool {
        if self.peek_byte() == Some(byte) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn starts_with(&self, value: &str) -> bool {
        self.source[self.position..].starts_with(value)
    }

    fn at_end(&self) -> bool {
        self.position >= self.bytes.len()
    }

    fn location(&self, offset: usize) -> Location {
        let bounded = offset.min(self.source.len());
        let prefix = &self.source[..bounded];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix
            .rsplit('\n')
            .next()
            .map_or(1, |line| line.chars().count() + 1);
        Location {
            line,
            column,
            offset: bounded,
        }
    }

    fn push_error(&mut self, code: &'static str, message: impl Into<String>, offset: usize) {
        let location = self.location(offset);
        self.diagnostics
            .push(Diagnostic::error(code, message, Span::point(location)));
    }

    fn finish_error(&mut self) -> ParseError {
        ParseError {
            diagnostics: std::mem::take(&mut self.diagnostics),
        }
    }
}

fn parse_script_context(
    raw: Option<&str>,
    parser: &mut Parser<'_>,
    offset: usize,
) -> ScriptContext {
    match raw {
        None | Some("instance") => ScriptContext::Instance,
        Some("module") => ScriptContext::Module,
        Some(value) => {
            parser.push_error(
                CODE_SCRIPT,
                format!("unsupported script context `{value}`"),
                offset,
            );
            ScriptContext::Instance
        }
    }
}

fn valid_path(path: &str, max_segments: usize) -> bool {
    let segments: Vec<_> = path.split('.').collect();
    !segments.is_empty()
        && segments.len() <= max_segments
        && segments.iter().all(|segment| is_path_segment(segment))
}

fn unescape_string(value: &str) -> String {
    let mut result = String::new();
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            result.push(match character {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            result.push(character);
        }
    }
    if escaped {
        result.push('\\');
    }
    result
}

fn split_source(filename: &Path, source: &str) -> Result<SourceBlocks, Error> {
    let Some(open_start) = source.find("<script") else {
        if source.contains("</script>") {
            return Err(Error::UnexpectedScriptClose {
                filename: filename.to_path_buf(),
            });
        }

        return Ok(SourceBlocks {
            script: None,
            markup: source.to_owned(),
        });
    };

    let open_end = source[open_start..]
        .find('>')
        .map(|offset| open_start + offset)
        .ok_or_else(|| Error::UnterminatedScriptBlock {
            filename: filename.to_path_buf(),
        })?;
    let content_start = open_end + 1;
    let close_relative = source[content_start..].find("</script>").ok_or_else(|| {
        Error::UnterminatedScriptBlock {
            filename: filename.to_path_buf(),
        }
    })?;
    let close_start = content_start + close_relative;
    let after_close = close_start + "</script>".len();

    if source[after_close..].contains("<script") {
        return Err(Error::MultipleScriptBlocks {
            filename: filename.to_path_buf(),
        });
    }

    let mut markup = String::with_capacity(source.len() - (open_end - open_start + 1));
    markup.push_str(&source[..open_start]);
    markup.push_str(&source[after_close..]);

    Ok(SourceBlocks {
        script: Some(source[content_start..close_start].to_owned()),
        markup,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        AttributeValue, BindingPath, Comment, Element, Error, Node, ParseOptions, ScriptBlock,
        ScriptContext, StudioDocument, Target, TokenRef, canonicalize, format, parse,
        parse_with_options, prepare, print, validate,
    };
    use std::path::Path;

    #[test]
    fn splits_script_and_markup() {
        let prepared = prepare(
            "Counter.studio",
            "<script lang=\"ts\">let count = $state(0)</script><button>{count}</button>",
            Target::JavaScript,
        )
        .expect("source should split");

        assert_eq!(prepared.filename, Path::new("Counter.studio"));
        assert_eq!(
            prepared.blocks.script.as_deref(),
            Some("let count = $state(0)")
        );
        assert_eq!(prepared.blocks.markup, "<button>{count}</button>");
    }

    #[test]
    fn supports_markup_without_script() {
        let prepared = prepare("Card.studio", "<Card />", Target::AssemblyScript)
            .expect("source should split");

        assert_eq!(prepared.blocks.script, None);
        assert_eq!(prepared.blocks.markup, "<Card />");
    }

    #[test]
    fn rejects_multiple_script_blocks() {
        let error = prepare(
            "Broken.studio",
            "<script>one</script><Card /><script>two</script>",
            Target::JavaScript,
        )
        .expect_err("multiple scripts should fail");

        assert_eq!(
            error,
            Error::MultipleScriptBlocks {
                filename: "Broken.studio".into()
            }
        );
    }

    #[test]
    fn parses_nested_v1_document_and_bounded_values() {
        let source = "studio 1\n<script lang=\"studio\">\nlet title = 'Home'\n</script>\n<!-- screen comment -->\n<Screen id=\"home\" title=\"Home\">\n  <List id=\"product-list\" items={$item.products} tone={token.colors.primary}>\n    <Text id=\"title\">\n      Hello world\n    </Text>\n  </List>\n</Screen>\n";
        let document = parse(source).expect("source should parse");
        assert_eq!(document.version, 1);
        assert_eq!(document.nodes.len(), 1);
        assert_eq!(document.nodes[0].id, "home");
        assert_eq!(
            document.nodes[0].leading_comments[0],
            Comment::new(" screen comment ")
        );
        let list = match &document.nodes[0].children[0] {
            Node::Element(element) => element,
            Node::Text(_) => panic!("expected nested element"),
        };
        assert_eq!(
            list.attributes.get("items"),
            Some(&AttributeValue::Binding(BindingPath {
                path: "$item.products".to_owned()
            }))
        );
        assert_eq!(
            list.attributes.get("tone"),
            Some(&AttributeValue::Token(TokenRef {
                path: "token.colors.primary".to_owned()
            }))
        );
    }

    #[test]
    fn accepts_whitespace_between_tag_attributes() {
        let document = parse("studio 1\n<Screen\n  id=\"home\"\n  title=\"Home\"\n/>\n")
            .expect("tag whitespace should be accepted");
        assert_eq!(document.nodes[0].id, "home");
        assert_eq!(document.nodes[0].attributes.len(), 1);
    }

    #[test]
    fn canonical_print_is_idempotent_and_round_trips() {
        let source = "studio version=\"1\"\n\n<Screen   title=\"Home\" id=\"home\">\n<!-- keep -->\n<Text id=\"title\">  Hello   world </Text>\n</Screen>\n";
        let model = parse(source).expect("source should parse");
        let printed = print(&model);
        assert_eq!(
            printed,
            print(&parse(&printed).expect("printed source should parse"))
        );
        assert_eq!(model, parse(&printed).expect("model should round trip"));
        assert_eq!(
            printed,
            format(&printed).expect("format should be idempotent")
        );
        assert_eq!(
            printed,
            canonicalize(source).expect("canonicalize should print")
        );
    }

    #[test]
    fn comments_anchor_to_following_nodes() {
        let document = parse(
            "studio 1\n<Screen id=\"home\">\n<!-- before text -->\n<Text id=\"copy\">Hello</Text>\n<!-- trailing -->\n</Screen>\n",
        )
        .expect("source should parse");
        let screen = &document.nodes[0];
        assert_eq!(screen.children.len(), 1);
        let child = match &screen.children[0] {
            Node::Element(child) => child,
            Node::Text(_) => panic!("expected element"),
        };
        assert_eq!(child.leading_comments[0].text, " before text ");
        assert_eq!(screen.trailing_comments[0].text, " trailing ");
        let printed = print(&document);
        assert!(printed.contains("<!-- before text -->"));
        assert!(printed.contains("<!-- trailing -->"));
    }

    #[test]
    fn document_boundary_comments_round_trip() {
        let document = parse(
            "<!-- before header -->\nstudio 1\n<Screen id=\"home\" />\n<!-- after root -->\n",
        )
        .expect("source should parse");
        let printed = print(&document);
        assert!(printed.starts_with("<!-- before header -->\nstudio 1\n"));
        assert_eq!(
            document,
            parse(&printed).expect("printed source should parse")
        );
    }

    #[test]
    fn reports_stable_identity_diagnostics_with_locations() {
        let error = parse(
            "studio 1\n<Screen id=\"Hero\"><Text></Text><Box id=\"hero\" /><Box id=\"hero\" /></Screen>\n",
        )
        .expect_err("invalid IDs should fail");
        let codes: Vec<_> = error
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect();
        assert!(codes.contains(&super::CODE_NON_CANONICAL_NODE_ID));
        assert!(codes.contains(&super::CODE_MISSING_NODE_ID));
        assert!(codes.contains(&super::CODE_DUPLICATE_NODE_ID));
        assert!(
            error
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.span.start.line >= 2)
        );
    }

    #[test]
    fn rejects_unknown_expressions_and_unsupported_versions() {
        let error = parse("studio 2\n<Box id=\"box\" value={window.alert()} />\n")
            .expect_err("closed grammar should reject input");
        let codes: Vec<_> = error
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect();
        assert!(codes.contains(&super::CODE_VERSION));
        assert!(codes.contains(&super::CODE_UNKNOWN_CONSTRUCT));
    }

    #[test]
    fn applies_hostile_input_limits_before_deep_parse() {
        let options = ParseOptions {
            max_source_bytes: 16,
            ..ParseOptions::default()
        };
        let error = parse_with_options("studio 1\n<Box id=\"box\" />", options)
            .expect_err("source should exceed explicit limit");
        assert_eq!(error.diagnostics[0].code, super::CODE_LIMIT);
    }

    #[test]
    fn manually_constructed_models_print_deterministically() {
        let mut screen = Element::new("Screen", "home");
        screen.set_attribute("z-index", AttributeValue::Number("2".to_owned()));
        screen.set_attribute("title", AttributeValue::String("Home".to_owned()));
        screen.children.push(Node::text("Hello"));
        let mut document = StudioDocument::new();
        document.script = Some(ScriptBlock {
            leading_comments: Vec::new(),
            lang: "studio".to_owned(),
            context: ScriptContext::Module,
            content: "export const title = 'Home';".to_owned(),
        });
        document.nodes.push(screen);
        let first = print(&document);
        let second = print(&document);
        assert_eq!(first, second);
        assert!(first.contains("context=\"module\""));
        assert!(validate(&document).is_empty());
    }
}
