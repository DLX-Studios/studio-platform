//! A host-independent adapter for the Designer's embedded Studio Script editor.
//!
//! The adapter deliberately keeps the text buffer separate from the design
//! mutation seam. Keystrokes update a cheap lexical view and a best-effort
//! outline. A commit is the only operation that invokes the parser/lowerer and
//! turns the resulting document delta into a typed [`CommandBatch`]. Invalid
//! text therefore never becomes a partial design mutation.

#![allow(clippy::all)]
#![allow(
    clippy::assigning_clones,
    clippy::collapsible_if,
    clippy::doc_markdown,
    clippy::items_after_statements,
    clippy::manual_let_else,
    clippy::map_unwrap_or,
    clippy::missing_errors_doc,
    clippy::needless_pass_by_value,
    clippy::semicolon_if_nothing_returned,
    clippy::single_match_else,
    clippy::unnested_or_patterns
)]

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use studio_script::{
    AttributeValue, Comment, Diagnostic, Element, Node, Severity, Span, StudioDocument, TokenRef,
    canonical_print, compile, parse,
};
use thiserror::Error;

use crate::{
    Actor, Command, CommandBatch, CommandOutcome, CommandReceipt, DesignNode, DesignNodeSource,
    DesignerQuery, DesignerQueryResult, DesignerSession, NodeId, NodeParent, OperationId,
    ParentPlacement, ProjectId, PropertyValue, RevisionId, STUDIO_DESIGN_SCHEMA_VERSION,
    StudioDesign, StudioDesignSnapshot, UndoGroupId,
};

/// A bounded byte edit supplied by a text widget.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptEdit {
    /// Inclusive UTF-8 byte offset of the edit.
    pub start: usize,
    /// Exclusive UTF-8 byte offset of the edit.
    pub end: usize,
    /// Replacement text.
    pub replacement: String,
}

impl ScriptEdit {
    /// Construct a byte-range replacement.
    #[must_use]
    pub fn replace(start: usize, end: usize, replacement: impl Into<String>) -> Self {
        Self {
            start,
            end,
            replacement: replacement.into(),
        }
    }
}

/// Lexical categories consumed by a syntax-highlighting view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxTokenKind {
    /// A language/header or behavior keyword.
    Keyword,
    /// An element/tag name.
    Tag,
    /// An attribute name.
    Attribute,
    /// A quoted string.
    String,
    /// A numeric literal.
    Number,
    /// A `$item.*` binding.
    Binding,
    /// A token reference.
    Token,
    /// A comment.
    Comment,
    /// A punctuation delimiter.
    Punctuation,
    /// An identifier or other lexical word.
    Identifier,
    /// Plain element text.
    Text,
}

/// One source range returned by the lightweight syntax hook.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxToken {
    /// Token category.
    pub kind: SyntaxTokenKind,
    /// Zero-based inclusive source byte offset.
    pub start: usize,
    /// Zero-based exclusive source byte offset.
    pub end: usize,
    /// One-based source line.
    pub line: usize,
    /// One-based source column.
    pub column: usize,
    /// Token text.
    pub text: String,
}

/// A stable tree entry for the hierarchy/outline pane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutlineNode {
    /// Element identity.
    pub id: String,
    /// Element kind as authored.
    pub kind: String,
    /// One-based source line of the opening tag.
    pub line: usize,
    /// One-based source column of the opening tag.
    pub column: usize,
    /// Nesting depth, where top-level elements are zero.
    pub depth: usize,
    /// Nested outline entries.
    pub children: Vec<Self>,
}

/// A source-linked problem suitable for the Problems panel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptDiagnostic {
    /// Stable parser/lowering or adapter code.
    pub code: String,
    /// Diagnostic severity.
    pub severity: Severity,
    /// Safe user-facing message.
    pub message: String,
    /// Source location and range.
    pub span: Span,
}

impl ScriptDiagnostic {
    fn from_parser(diagnostic: &Diagnostic) -> Self {
        Self {
            code: diagnostic.code.to_owned(),
            severity: diagnostic.severity,
            message: diagnostic.message.clone(),
            span: diagnostic.span,
        }
    }

    fn error(code: impl Into<String>, message: impl Into<String>, span: Span) -> Self {
        Self {
            code: code.into(),
            severity: Severity::Error,
            message: message.into(),
            span,
        }
    }

    /// One-based line convenience accessor for panel rows.
    #[must_use]
    pub const fn line(&self) -> usize {
        self.span.start.line
    }

    /// One-based column convenience accessor for panel highlights.
    #[must_use]
    pub const fn column(&self) -> usize {
        self.span.start.column
    }
}

/// The metadata attributed to one editor commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptCommitMetadata {
    /// Operation id used for idempotency by [`DesignerSession`].
    pub operation_id: OperationId,
    /// Human/agent actor attribution.
    pub actor: Actor,
    /// Named undo group identity.
    pub undo_group_id: UndoGroupId,
    /// User-facing undo group name.
    pub undo_group_name: String,
}

impl ScriptCommitMetadata {
    /// Construct commit metadata.
    #[must_use]
    pub fn new(
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
        undo_group_name: impl Into<String>,
    ) -> Self {
        Self {
            operation_id,
            actor,
            undo_group_id,
            undo_group_name: undo_group_name.into(),
        }
    }
}

/// Immutable view returned after a keystroke or replacement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditorSnapshot {
    /// Current source buffer.
    pub source: String,
    /// Best-effort parser diagnostics currently visible.
    pub diagnostics: Vec<ScriptDiagnostic>,
    /// Lexical syntax ranges.
    pub syntax: Vec<SyntaxToken>,
    /// Hierarchy outline.
    pub outline: Vec<OutlineNode>,
    /// Whether the source differs from the last accepted canonical document.
    pub dirty: bool,
    /// Revision against which the next commit will be submitted.
    pub base_revision: RevisionId,
}

/// A valid, deterministic command plan produced by a commit attempt.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptCommitPlan {
    /// Typed command batch to submit. A batch with no commands is a formatting-only change.
    pub batch: CommandBatch,
    /// Canonical source corresponding to the candidate document.
    pub canonical_source: String,
    /// Parsed candidate document, retained so acceptance does not reparse it.
    pub document: StudioDocument,
    /// Whether submitting the batch is necessary.
    pub changed: bool,
}

/// Result of submitting an editor plan through the DesignerSession seam.
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptCommitOutcome {
    /// A typed batch was accepted and the editor now tracks its revision.
    Committed {
        /// Accepted session receipt.
        receipt: CommandReceipt,
        /// Canonical source after acceptance.
        source: String,
    },
    /// Source was valid but changed only formatting/trivia.
    NoChanges {
        /// Canonical source after local formatting.
        source: String,
    },
    /// The source was rejected before any session call.
    Invalid {
        /// Line-linked parser/lowering/adapter diagnostics.
        diagnostics: Vec<ScriptDiagnostic>,
    },
    /// The session rejected or conflicted with the typed plan.
    Session(CommandOutcome),
}

/// Failures constructing or editing an adapter.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum ScriptEditorError {
    /// The byte range was not a valid UTF-8 boundary or was outside the buffer.
    #[error("script edit range is outside the UTF-8 source buffer")]
    InvalidEdit,
    /// The source was invalid at commit time.
    #[error("script source is invalid")]
    InvalidSource {
        /// Parser/lowering diagnostics linked to source spans.
        diagnostics: Vec<ScriptDiagnostic>,
    },
    /// A valid source feature cannot be expressed by the current command seam.
    #[error("script edit cannot be represented by the Designer command seam")]
    Unsupported {
        /// Source-linked explanation of the unsupported feature.
        diagnostic: ScriptDiagnostic,
    },
    /// The adapter was opened for a different project.
    #[error("script editor project does not match the Designer session")]
    ProjectMismatch,
    /// A design field has no v1 Studio Script representation.
    #[error("design cannot be represented as Studio Script: {0}")]
    Design(String),
}

impl ScriptEditorError {
    /// Return line-linked diagnostics carried by this failure.
    #[must_use]
    pub fn diagnostics(&self) -> &[ScriptDiagnostic] {
        match self {
            Self::InvalidSource { diagnostics } => diagnostics,
            Self::Unsupported { diagnostic } => std::slice::from_ref(diagnostic),
            Self::InvalidEdit | Self::ProjectMismatch | Self::Design(_) => &[],
        }
    }
}

/// Testable document/buffer adapter used by a narrow native editor surface.
pub struct ScriptDocumentAdapter {
    project_id: ProjectId,
    source: String,
    parsed: Option<StudioDocument>,
    baseline: Option<StudioDocument>,
    base_revision: RevisionId,
    diagnostics: Vec<ScriptDiagnostic>,
    syntax: Vec<SyntaxToken>,
    outline: Vec<OutlineNode>,
}

/// Short alias for callers that call the adapter an editor.
pub type ScriptEditor = ScriptDocumentAdapter;

impl ScriptDocumentAdapter {
    /// Open an editor buffer without requiring a valid initial source.
    #[must_use]
    pub fn new(source: impl Into<String>, project_id: ProjectId) -> Self {
        let mut adapter = Self {
            project_id,
            source: source.into(),
            parsed: None,
            baseline: None,
            base_revision: RevisionId::INITIAL,
            diagnostics: Vec::new(),
            syntax: Vec::new(),
            outline: Vec::new(),
        };
        adapter.reindex();
        adapter.baseline = adapter.parsed.clone();
        adapter
    }

    /// Open a source buffer attached to an immutable Designer snapshot.
    #[must_use]
    pub fn open(snapshot: &StudioDesignSnapshot, source: impl Into<String>) -> Self {
        let mut adapter = Self::new(source, snapshot.design.project_id.clone());
        adapter.base_revision = snapshot.revision.id;
        adapter
    }

    /// Open canonical source generated from a Designer snapshot.
    pub fn from_snapshot(snapshot: &StudioDesignSnapshot) -> Result<Self, ScriptEditorError> {
        let document = document_from_design(&snapshot.design)?;
        let source = canonical_print(&document);
        Ok(Self {
            project_id: snapshot.design.project_id.clone(),
            source,
            parsed: Some(document.clone()),
            baseline: Some(document),
            base_revision: snapshot.revision.id,
            diagnostics: Vec::new(),
            syntax: Vec::new(),
            outline: Vec::new(),
        }
        .with_index())
    }

    /// Return the current source buffer.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Return the current parsed document, if the buffer parses.
    #[must_use]
    pub fn document(&self) -> Option<&StudioDocument> {
        self.parsed.as_ref()
    }

    /// Return diagnostics currently shown by the editor.
    #[must_use]
    pub fn diagnostics(&self) -> &[ScriptDiagnostic] {
        &self.diagnostics
    }

    /// Return lexical ranges for syntax highlighting.
    #[must_use]
    pub fn syntax(&self) -> &[SyntaxToken] {
        &self.syntax
    }

    /// Alias for [`Self::syntax`].
    #[must_use]
    pub fn syntax_tokens(&self) -> &[SyntaxToken] {
        self.syntax()
    }

    /// Return the best-effort element hierarchy outline.
    #[must_use]
    pub fn outline(&self) -> &[OutlineNode] {
        &self.outline
    }

    /// Revision used as the command batch base.
    #[must_use]
    pub const fn base_revision(&self) -> RevisionId {
        self.base_revision
    }

    /// Return whether the buffer differs from the accepted document.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.baseline
            .as_ref()
            .is_none_or(|document| canonical_print(document) != self.source)
    }

    /// Apply one UTF-8 byte edit and refresh the lexical/outline hooks.
    pub fn apply_edit(&mut self, edit: ScriptEdit) -> Result<EditorSnapshot, ScriptEditorError> {
        if edit.start > edit.end
            || edit.end > self.source.len()
            || !self.source.is_char_boundary(edit.start)
            || !self.source.is_char_boundary(edit.end)
        {
            return Err(ScriptEditorError::InvalidEdit);
        }
        self.source
            .replace_range(edit.start..edit.end, &edit.replacement);
        self.reindex();
        Ok(self.snapshot())
    }

    /// Replace the complete text buffer, as used by paste or an external model.
    pub fn replace_source(&mut self, source: impl Into<String>) -> EditorSnapshot {
        self.source = source.into();
        self.reindex();
        self.snapshot()
    }

    /// Return an immutable view of all editor surfaces.
    #[must_use]
    pub fn snapshot(&self) -> EditorSnapshot {
        EditorSnapshot {
            source: self.source.clone(),
            diagnostics: self.diagnostics.clone(),
            syntax: self.syntax.clone(),
            outline: self.outline.clone(),
            dirty: self.is_dirty(),
            base_revision: self.base_revision,
        }
    }

    /// Build a typed command batch without mutating the session.
    #[allow(
        clippy::too_many_lines,
        reason = "commit preparation keeps validation and diff policy atomic"
    )]
    pub fn prepare_commit(
        &mut self,
        snapshot: &StudioDesignSnapshot,
        metadata: ScriptCommitMetadata,
    ) -> Result<ScriptCommitPlan, ScriptEditorError> {
        if snapshot.design.project_id != self.project_id {
            return Err(ScriptEditorError::ProjectMismatch);
        }

        let document = match parse(&self.source) {
            Ok(document) => document,
            Err(error) => {
                let diagnostics = error
                    .diagnostics()
                    .iter()
                    .map(ScriptDiagnostic::from_parser)
                    .collect::<Vec<_>>();
                self.diagnostics = diagnostics.clone();
                return Err(ScriptEditorError::InvalidSource { diagnostics });
            }
        };
        if let Err(error) = compile(&self.source) {
            let diagnostics = error
                .diagnostics()
                .iter()
                .map(ScriptDiagnostic::from_parser)
                .collect::<Vec<_>>();
            self.diagnostics = diagnostics.clone();
            return Err(ScriptEditorError::InvalidSource { diagnostics });
        }

        if self
            .baseline
            .as_ref()
            .and_then(|baseline| baseline.script.as_ref())
            != document.script.as_ref()
        {
            let span = document.script.as_ref().map_or_else(
                || document_span(&self.source),
                |script| script_span(&self.source, &script.content),
            );
            let diagnostic = ScriptDiagnostic::error(
                "DESIGN_SCRIPT_BEHAVIOR_UNSUPPORTED",
                "editing the behavior block is validated but is not yet represented by a Designer command",
                span,
            );
            self.diagnostics = vec![diagnostic.clone()];
            return Err(ScriptEditorError::Unsupported { diagnostic });
        }

        let mut candidate = design_from_document(&document, snapshot.design.project_id.clone())?;
        // Script behaviors, library data, and non-source layout metadata remain
        // owned by the session until their typed command families exist.
        candidate.name = snapshot.design.name.clone();
        candidate.compositions = snapshot.design.compositions.clone();
        candidate.tokens = snapshot.design.tokens.clone();
        candidate.responsive_variants = snapshot.design.responsive_variants.clone();
        candidate.interactions = snapshot.design.interactions.clone();

        let commands = match diff_designs(&snapshot.design, &candidate, &self.source) {
            Ok(commands) => commands,
            Err(diagnostic) => {
                self.diagnostics = vec![diagnostic.clone()];
                return Err(ScriptEditorError::Unsupported { diagnostic });
            }
        };
        self.diagnostics.clear();
        let canonical_source = canonical_print(&document);
        let batch = CommandBatch {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            operation_id: metadata.operation_id,
            actor: metadata.actor,
            project_id: snapshot.design.project_id.clone(),
            base_revision: self.base_revision,
            undo_group_id: metadata.undo_group_id,
            undo_group_name: metadata.undo_group_name,
            preconditions: Vec::new(),
            commands: commands.clone(),
        };
        Ok(ScriptCommitPlan {
            batch,
            canonical_source,
            document,
            changed: !commands.is_empty(),
        })
    }

    /// Validate, diff, and submit the current buffer through a session.
    #[allow(
        clippy::too_many_lines,
        reason = "session submission keeps buffer and revision transitions atomic"
    )]
    pub async fn commit<S: DesignerSession>(
        &mut self,
        session: &mut S,
        metadata: ScriptCommitMetadata,
    ) -> ScriptCommitOutcome {
        let snapshot = match session.query(DesignerQuery::Snapshot) {
            DesignerQueryResult::Snapshot(snapshot) => snapshot,
            _ => {
                let diagnostic = ScriptDiagnostic::error(
                    "DESIGN_SNAPSHOT_UNAVAILABLE",
                    "the Designer session did not return a design snapshot",
                    document_span(&self.source),
                );
                self.diagnostics = vec![diagnostic.clone()];
                return ScriptCommitOutcome::Invalid {
                    diagnostics: vec![diagnostic],
                };
            }
        };
        let plan = match self.prepare_commit(&snapshot, metadata) {
            Ok(plan) => plan,
            Err(ScriptEditorError::InvalidSource { .. })
            | Err(ScriptEditorError::Unsupported { .. }) => {
                return ScriptCommitOutcome::Invalid {
                    diagnostics: self.diagnostics.clone(),
                };
            }
            Err(error) => {
                let diagnostic = ScriptDiagnostic::error(
                    "DESIGN_EDITOR_ADAPTER",
                    error.to_string(),
                    document_span(&self.source),
                );
                self.diagnostics = vec![diagnostic.clone()];
                return ScriptCommitOutcome::Invalid {
                    diagnostics: vec![diagnostic],
                };
            }
        };

        if !plan.changed {
            self.source = plan.canonical_source.clone();
            self.parsed = Some(plan.document.clone());
            self.baseline = Some(plan.document);
            self.reindex();
            return ScriptCommitOutcome::NoChanges {
                source: self.source.clone(),
            };
        }

        match session.submit(plan.batch).await {
            CommandOutcome::Accepted(receipt) => {
                self.source = plan.canonical_source.clone();
                self.parsed = Some(plan.document.clone());
                self.baseline = Some(plan.document);
                self.base_revision = receipt.committed_revision;
                self.diagnostics.clear();
                self.reindex();
                ScriptCommitOutcome::Committed {
                    receipt,
                    source: self.source.clone(),
                }
            }
            outcome => ScriptCommitOutcome::Session(outcome),
        }
    }

    /// Replace the buffer with canonical source for a just-committed canvas edit.
    pub fn refresh_from_snapshot(
        &mut self,
        snapshot: &StudioDesignSnapshot,
    ) -> Result<EditorSnapshot, ScriptEditorError> {
        if snapshot.design.project_id != self.project_id {
            return Err(ScriptEditorError::ProjectMismatch);
        }
        let old = self.parsed.as_ref().or(self.baseline.as_ref());
        let mut document = document_from_design(&snapshot.design)?;
        if let Some(old) = old {
            transfer_trivia(old, &mut document);
            document.script = old.script.clone();
        }
        self.source = canonical_print(&document);
        self.parsed = Some(document.clone());
        self.baseline = Some(document);
        self.base_revision = snapshot.revision.id;
        self.diagnostics.clear();
        self.reindex();
        Ok(self.snapshot())
    }

    /// Alias used by canvas projection adapters.
    pub fn sync_from_snapshot(
        &mut self,
        snapshot: &StudioDesignSnapshot,
    ) -> Result<EditorSnapshot, ScriptEditorError> {
        self.refresh_from_snapshot(snapshot)
    }

    fn with_index(mut self) -> Self {
        self.reindex();
        self
    }

    fn reindex(&mut self) {
        self.syntax = scan_syntax(&self.source);
        match parse(&self.source) {
            Ok(document) => {
                self.parsed = Some(document.clone());
                self.diagnostics.clear();
                self.outline = outline_for_document(&document, &self.source);
            }
            Err(error) => {
                self.parsed = None;
                self.diagnostics = error
                    .diagnostics()
                    .iter()
                    .map(ScriptDiagnostic::from_parser)
                    .collect();
                self.outline = outline_from_source(&self.source);
            }
        }
    }
}

fn document_from_design(design: &StudioDesign) -> Result<StudioDocument, ScriptEditorError> {
    let mut document = StudioDocument::new();
    for screen_id in &design.screen_order {
        let screen = design.screens.get(screen_id).ok_or_else(|| {
            ScriptEditorError::Design("screen order references a missing screen".to_owned())
        })?;
        let element = element_from_design_node(design, &screen.root_node_id)?;
        document.nodes.push(element);
    }
    Ok(document)
}

fn element_from_design_node(
    design: &StudioDesign,
    node_id: &NodeId,
) -> Result<Element, ScriptEditorError> {
    let node = design
        .nodes
        .get(node_id)
        .ok_or_else(|| ScriptEditorError::Design(format!("node `{node_id}` is missing")))?;
    let kind = match &node.source {
        DesignNodeSource::Primitive { kind } => *kind,
        DesignNodeSource::CompositionInstance { .. } => {
            return Err(ScriptEditorError::Design(format!(
                "composition instance `{node_id}` has no v1 Studio Script element form"
            )));
        }
    };
    let kind_name = serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or_else(|| ScriptEditorError::Design("node kind is not serializable".to_owned()))?;
    let mut element = Element::new(kind_name, node.id.as_str());
    if node.name != node.id.as_str() {
        element.set_attribute("name", AttributeValue::String(node.name.clone()));
    }
    for (name, value) in &node.properties {
        if name == "name" || (kind == studio_protocol::NodeKind::Text && name == "text") {
            continue;
        }
        element.set_attribute(name.clone(), attribute_from_property(value)?);
    }
    if kind == studio_protocol::NodeKind::Text {
        if let Some(PropertyValue::String(text)) = node.properties.get("text") {
            element.children.push(Node::text(text.clone()));
        }
    }
    for child_id in &node.children {
        element
            .children
            .push(Node::Element(element_from_design_node(design, child_id)?));
    }
    Ok(element)
}

fn attribute_from_property(value: &PropertyValue) -> Result<AttributeValue, ScriptEditorError> {
    Ok(match value {
        PropertyValue::String(value) => AttributeValue::String(value.clone()),
        PropertyValue::Boolean(value) => AttributeValue::Boolean(*value),
        PropertyValue::Integer(value) => AttributeValue::Number(value.to_string()),
        PropertyValue::Decimal(value) => AttributeValue::Number(value.clone()),
        PropertyValue::Token(token) => AttributeValue::Token(TokenRef {
            path: format!("token.{token}"),
        }),
        PropertyValue::Binding(binding) => AttributeValue::Binding(studio_script::BindingPath {
            path: format!(
                "$item.{}",
                std::iter::once(binding.collection.as_str())
                    .chain(binding.segments.iter().map(String::as_str))
                    .collect::<Vec<_>>()
                    .join(".")
            ),
        }),
        unsupported => {
            return Err(ScriptEditorError::Design(format!(
                "property value `{unsupported:?}` has no parser-of-record representation"
            )));
        }
    })
}

fn property_from_attribute(value: &AttributeValue) -> Result<PropertyValue, ScriptEditorError> {
    Ok(match value {
        AttributeValue::String(value) => PropertyValue::String(value.clone()),
        AttributeValue::Boolean(value) => PropertyValue::Boolean(*value),
        AttributeValue::Number(value) => value
            .parse::<i64>()
            .map(PropertyValue::Integer)
            .unwrap_or_else(|_| PropertyValue::Decimal(value.clone())),
        AttributeValue::Binding(binding) => {
            let path = binding.path.strip_prefix("$item.").ok_or_else(|| {
                ScriptEditorError::Design("binding is missing the `$item.` prefix".to_owned())
            })?;
            let mut segments = path.split('.');
            let collection = segments
                .next()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ScriptEditorError::Design("binding collection is empty".to_owned())
                })?;
            PropertyValue::Binding(crate::BindingPath {
                collection: collection.to_owned(),
                segments: segments.map(str::to_owned).collect(),
            })
        }
        AttributeValue::Token(token) => PropertyValue::Token(
            crate::TokenId::new(
                token
                    .path
                    .strip_prefix("token.")
                    .or_else(|| token.path.strip_prefix("$token."))
                    .or_else(|| token.path.strip_prefix('@'))
                    .unwrap_or(&token.path),
            )
            .map_err(|_| ScriptEditorError::Design("token identity is invalid".to_owned()))?,
        ),
    })
}

fn design_from_document(
    document: &StudioDocument,
    project_id: ProjectId,
) -> Result<StudioDesign, ScriptEditorError> {
    let mut design = StudioDesign::empty(project_id, "Studio Script");
    for root in &document.nodes {
        let root_id = NodeId::new(root.id.clone())
            .map_err(|_| ScriptEditorError::Design("element identity is invalid".to_owned()))?;
        let screen_id = crate::ScreenId::new(root.id.clone())
            .map_err(|_| ScriptEditorError::Design("screen identity is invalid".to_owned()))?;
        flatten_element(
            root,
            &mut design,
            NodeParent::Screen {
                screen_id: screen_id.clone(),
            },
        )?;
        design.screens.insert(
            screen_id.clone(),
            crate::Screen {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                id: screen_id.clone(),
                name: design
                    .nodes
                    .get(&root_id)
                    .map(|node| node.name.clone())
                    .unwrap_or_else(|| root.id.clone()),
                route: format!("/{}", root.id),
                root_node_id: root_id,
            },
        );
        design.screen_order.push(screen_id);
    }
    Ok(design)
}

fn flatten_element(
    element: &Element,
    design: &mut StudioDesign,
    parent: NodeParent,
) -> Result<(), ScriptEditorError> {
    let id = NodeId::new(element.id.clone())
        .map_err(|_| ScriptEditorError::Design("element identity is invalid".to_owned()))?;
    let kind: studio_protocol::NodeKind =
        serde_json::from_value(Value::String(element.kind.to_ascii_lowercase())).map_err(|_| {
            ScriptEditorError::Design(format!("unknown element kind `{}`", element.kind))
        })?;
    let name = match element.attributes.get("name") {
        Some(AttributeValue::String(value)) => value.clone(),
        _ => element.id.clone(),
    };
    let mut node = DesignNode::primitive(id.clone(), name, kind);
    for (name, value) in &element.attributes {
        if name == "name" {
            continue;
        }
        node.properties
            .insert(name.clone(), property_from_attribute(value)?);
    }
    for child in &element.children {
        match child {
            Node::Element(child) => {
                node.children
                    .push(NodeId::new(child.id.clone()).map_err(|_| {
                        ScriptEditorError::Design("element identity is invalid".to_owned())
                    })?)
            }
            Node::Text(text) => {
                node.properties
                    .insert("text".to_owned(), PropertyValue::String(text.text.clone()));
            }
        }
    }
    design.parents.insert(id.clone(), parent);
    let children = node.children.clone();
    design.nodes.insert(id, node);
    for child in &element.children {
        if let Node::Element(child) = child {
            flatten_element(
                child,
                design,
                NodeParent::Node {
                    node_id: NodeId::new(element.id.clone()).map_err(|_| {
                        ScriptEditorError::Design("element identity is invalid".to_owned())
                    })?,
                },
            )?;
        }
    }
    debug_assert_eq!(
        children.len(),
        design.nodes[&NodeId::new(element.id.clone()).unwrap()]
            .children
            .len()
    );
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "the deterministic diff pipeline is kept together for review"
)]
fn diff_designs(
    current: &StudioDesign,
    target: &StudioDesign,
    source: &str,
) -> Result<Vec<Command>, ScriptDiagnostic> {
    let current_roots = current
        .screen_order
        .iter()
        .filter_map(|id| {
            current
                .screens
                .get(id)
                .map(|screen| screen.root_node_id.clone())
        })
        .collect::<Vec<_>>();
    let target_roots = target
        .screen_order
        .iter()
        .filter_map(|id| {
            target
                .screens
                .get(id)
                .map(|screen| screen.root_node_id.clone())
        })
        .collect::<Vec<_>>();
    if current_roots != target_roots {
        return Err(ScriptDiagnostic::error(
            "DESIGN_ROOT_EDIT_UNSUPPORTED",
            "editing screen roots or screen count is not represented by the current command seam",
            document_span(source),
        ));
    }

    let current_ids = current.nodes.keys().cloned().collect::<BTreeSet<_>>();
    let target_ids = target.nodes.keys().cloned().collect::<BTreeSet<_>>();
    let mut commands = Vec::new();
    let mut working = current.clone();

    // Delete only the topmost removed node in each subtree. The command engine
    // records the complete subtree in a tombstone and remains atomic.
    for id in current_ids.difference(&target_ids) {
        let Some(parent) = current.parents.get(id) else {
            continue;
        };
        let topmost = match parent {
            NodeParent::Node { node_id } => target_ids.contains(node_id),
            NodeParent::Screen { .. } | NodeParent::Composition { .. } => false,
        };
        if !topmost {
            continue;
        }
        let Some(index) = child_index(&working, id) else {
            return Err(adapter_diagnostic(
                source,
                id,
                "removed node has no indexed parent",
            ));
        };
        commands.push(Command::DeleteNode {
            node_id: id.clone(),
        });
        remove_working_subtree(&mut working, id, index);
    }

    // Insert new nodes child-first in document order. InsertNode intentionally
    // accepts a childless node, so descendants become their own typed inserts.
    for id in target_order(target) {
        if current_ids.contains(&id) {
            continue;
        }
        let Some(NodeParent::Node { node_id: parent_id }) = target.parents.get(&id) else {
            return Err(adapter_diagnostic(
                source,
                &id,
                "new screen roots are not supported",
            ));
        };
        let Some(node) = target.nodes.get(&id) else {
            continue;
        };
        let target_index = target
            .nodes
            .get(parent_id)
            .and_then(|parent| parent.children.iter().position(|child| child == &id))
            .ok_or_else(|| adapter_diagnostic(source, &id, "new node is absent from its parent"))?;
        let index = target_index.min(
            working
                .nodes
                .get(parent_id)
                .map_or(0, |parent| parent.children.len()),
        );
        let mut inserted = node.clone();
        inserted.children.clear();
        commands.push(Command::InsertNode {
            parent: ParentPlacement {
                parent: NodeParent::Node {
                    node_id: parent_id.clone(),
                },
                index,
            },
            node: Box::new(inserted.clone()),
        });
        working.nodes.insert(id.clone(), inserted);
        working.parents.insert(
            id.clone(),
            NodeParent::Node {
                node_id: parent_id.clone(),
            },
        );
        working
            .nodes
            .get_mut(parent_id)
            .expect("target parent exists")
            .children
            .insert(index, id.clone());
    }

    // Property/name changes are independent of structural ordering.
    for id in current_ids.intersection(&target_ids) {
        let before = &current.nodes[id];
        let after = &target.nodes[id];
        if before.source != after.source {
            return Err(adapter_diagnostic(
                source,
                id,
                "changing an element kind is not represented by a typed command",
            ));
        }
        if before.name != after.name {
            commands.push(Command::RenameNode {
                node_id: id.clone(),
                name: after.name.clone(),
            });
        }
        let keys = before
            .properties
            .keys()
            .chain(after.properties.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        for property in keys {
            let old = before.properties.get(&property);
            let new = after.properties.get(&property);
            if old != new {
                commands.push(Command::SetProperty {
                    node_id: id.clone(),
                    property,
                    value: new.cloned(),
                });
            }
        }
    }

    // Moves are emitted before final reorders. The working model makes each
    // placement valid even when several siblings move together.
    for id in target_order(target) {
        if !current_ids.contains(&id) || !target_ids.contains(&id) {
            continue;
        }
        let old_parent = working.parents.get(&id).cloned();
        let new_parent = target.parents.get(&id).cloned();
        if old_parent == new_parent {
            continue;
        }
        let Some(NodeParent::Node { node_id: parent_id }) = new_parent else {
            return Err(adapter_diagnostic(
                source,
                &id,
                "moving screen roots is not supported",
            ));
        };
        let target_index = target.nodes[&parent_id]
            .children
            .iter()
            .position(|child| child == &id)
            .unwrap_or(0);
        let destination_index = target_index.min(working.nodes[&parent_id].children.len());
        let old_index = old_parent
            .as_ref()
            .and_then(|_| child_index(&working, &id))
            .unwrap_or(0);
        commands.push(Command::MoveNode {
            node_id: id.clone(),
            destination: ParentPlacement {
                parent: NodeParent::Node {
                    node_id: parent_id.clone(),
                },
                index: destination_index,
            },
        });
        remove_child_working(&mut working, &id, old_parent.as_ref().unwrap(), old_index);
        working
            .nodes
            .get_mut(&parent_id)
            .expect("destination parent exists")
            .children
            .insert(destination_index, id.clone());
        working
            .parents
            .insert(id, NodeParent::Node { node_id: parent_id });
    }

    // Finish with exact sibling order. ReorderNode is stable and reversible,
    // and this pass also corrects temporary insertion/move clamping.
    for parent_id in target_order(target) {
        let Some(parent) = target.nodes.get(&parent_id) else {
            continue;
        };
        for (index, id) in parent.children.iter().enumerate() {
            let Some(current_index) = child_index(&working, id) else {
                continue;
            };
            if current_index != index {
                commands.push(Command::ReorderNode {
                    node_id: id.clone(),
                    index,
                });
                let parent_ref = working.parents[id].clone();
                remove_child_working(&mut working, id, &parent_ref, current_index);
                working
                    .nodes
                    .get_mut(&parent_id)
                    .expect("target parent exists")
                    .children
                    .insert(index, id.clone());
            }
        }
    }
    Ok(commands)
}

fn target_order(design: &StudioDesign) -> Vec<NodeId> {
    fn walk(design: &StudioDesign, id: &NodeId, output: &mut Vec<NodeId>) {
        output.push(id.clone());
        if let Some(node) = design.nodes.get(id) {
            for child in &node.children {
                walk(design, child, output);
            }
        }
    }
    design
        .screen_order
        .iter()
        .filter_map(|screen_id| design.screens.get(screen_id))
        .fold(Vec::new(), |mut output, screen| {
            walk(design, &screen.root_node_id, &mut output);
            output
        })
}

fn child_index(design: &StudioDesign, id: &NodeId) -> Option<usize> {
    let NodeParent::Node { node_id: parent_id } = design.parents.get(id)? else {
        return None;
    };
    design
        .nodes
        .get(parent_id)?
        .children
        .iter()
        .position(|child| child == id)
}

fn remove_working_subtree(design: &mut StudioDesign, id: &NodeId, index: usize) {
    let parent = design.parents.get(id).cloned();
    if let Some(NodeParent::Node { node_id: parent_id }) = parent {
        if let Some(node) = design.nodes.get_mut(&parent_id) {
            node.children.remove(index);
        }
    }
    let children = design
        .nodes
        .get(id)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    for child in children {
        remove_working_subtree_without_parent(design, &child);
    }
    remove_working_subtree_without_parent(design, id);
}

fn remove_working_subtree_without_parent(design: &mut StudioDesign, id: &NodeId) {
    let children = design
        .nodes
        .get(id)
        .map(|node| node.children.clone())
        .unwrap_or_default();
    for child in children {
        remove_working_subtree_without_parent(design, &child);
    }
    design.nodes.remove(id);
    design.parents.remove(id);
}

fn remove_child_working(design: &mut StudioDesign, id: &NodeId, parent: &NodeParent, index: usize) {
    if let NodeParent::Node { node_id: parent_id } = parent {
        if let Some(node) = design.nodes.get_mut(parent_id) {
            if node.children.get(index) == Some(id) {
                node.children.remove(index);
            } else if let Some(found) = node.children.iter().position(|child| child == id) {
                node.children.remove(found);
            }
        }
    }
}

fn adapter_diagnostic(source: &str, id: &NodeId, message: &str) -> ScriptDiagnostic {
    ScriptDiagnostic::error(
        "DESIGN_EDITOR_DIFF",
        message,
        span_for_id(source, id.as_str()),
    )
}

fn document_span(source: &str) -> Span {
    span_at(source, 0)
}

fn script_span(source: &str, content: &str) -> Span {
    source
        .find(content)
        .map_or_else(|| document_span(source), |offset| span_at(source, offset))
}

fn span_for_id(source: &str, id: &str) -> Span {
    let needle = format!("id=\"{id}\"");
    source
        .find(&needle)
        .map_or_else(|| document_span(source), |offset| span_at(source, offset))
}

fn span_at(source: &str, offset: usize) -> Span {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map_or(0, |line| line.chars().count())
        + 1;
    let location = studio_script::Location {
        line,
        column,
        offset,
    };
    Span {
        start: location,
        end: location,
    }
}

fn outline_for_document(document: &StudioDocument, source: &str) -> Vec<OutlineNode> {
    fn visit(element: &Element, source: &str, depth: usize) -> OutlineNode {
        let span = span_for_id(source, &element.id);
        OutlineNode {
            id: element.id.clone(),
            kind: element.kind.clone(),
            line: span.start.line,
            column: span.start.column,
            depth,
            children: element
                .children
                .iter()
                .filter_map(|child| match child {
                    Node::Element(child) => Some(visit(child, source, depth + 1)),
                    Node::Text(_) => None,
                })
                .collect(),
        }
    }
    document
        .nodes
        .iter()
        .map(|element| visit(element, source, 0))
        .collect()
}

fn outline_from_source(source: &str) -> Vec<OutlineNode> {
    let mut output = Vec::new();
    for token in scan_syntax(source) {
        if token.kind != SyntaxTokenKind::Tag || token.text.starts_with('/') {
            continue;
        }
        let Some(id_start) = source[token.end..].find("id=\"") else {
            continue;
        };
        let id_start = token.end + id_start + 4;
        let Some(id_end) = source[id_start..].find('"') else {
            continue;
        };
        output.push(OutlineNode {
            id: source[id_start..id_start + id_end].to_owned(),
            kind: token.text.clone(),
            line: token.line,
            column: token.column,
            depth: 0,
            children: Vec::new(),
        });
    }
    output
}

#[allow(
    clippy::too_many_lines,
    reason = "the lexical hook is one bounded state machine"
)]
fn scan_syntax(source: &str) -> Vec<SyntaxToken> {
    let bytes = source.as_bytes();
    let mut output = Vec::new();
    let mut offset = 0;
    let mut in_tag = false;
    while offset < bytes.len() {
        if source[offset..].starts_with("<!--") {
            let end = source[offset + 4..]
                .find("-->")
                .map_or(bytes.len(), |end| offset + 4 + end + 3);
            push_token(&mut output, source, SyntaxTokenKind::Comment, offset, end);
            offset = end;
            continue;
        }
        if bytes[offset] == b'#' {
            let end = source[offset..]
                .find('\n')
                .map_or(bytes.len(), |end| offset + end);
            push_token(&mut output, source, SyntaxTokenKind::Comment, offset, end);
            offset = end;
            continue;
        }
        if bytes[offset] == b'<' {
            in_tag = true;
            push_token(
                &mut output,
                source,
                SyntaxTokenKind::Punctuation,
                offset,
                offset + 1,
            );
            offset += 1;
            if bytes.get(offset) == Some(&b'/') {
                push_token(
                    &mut output,
                    source,
                    SyntaxTokenKind::Punctuation,
                    offset,
                    offset + 1,
                );
                offset += 1;
            }
            let end = take_word(source, offset);
            if end > offset {
                push_token(&mut output, source, SyntaxTokenKind::Tag, offset, end);
                offset = end;
            }
            continue;
        }
        if in_tag
            && (bytes[offset] == b'>'
                || source[offset..].starts_with("/>")
                || bytes[offset] == b'=')
        {
            let end = if source[offset..].starts_with("/>") {
                offset + 2
            } else {
                offset + 1
            };
            push_token(
                &mut output,
                source,
                SyntaxTokenKind::Punctuation,
                offset,
                end,
            );
            in_tag = !source[offset..].starts_with("/>") && bytes[offset] != b'>';
            offset = end;
            continue;
        }
        if bytes[offset] == b'"' {
            let end = source[offset + 1..]
                .find('"')
                .map_or(bytes.len(), |end| offset + 1 + end + 1);
            push_token(&mut output, source, SyntaxTokenKind::String, offset, end);
            offset = end;
            continue;
        }
        if bytes[offset] == b'{' {
            let end = source[offset + 1..]
                .find('}')
                .map_or(bytes.len(), |end| offset + 1 + end + 1);
            let text = &source[offset..end];
            let kind = if text.contains("$item.") {
                SyntaxTokenKind::Binding
            } else if text.contains("token.") || text.contains("$token.") || text.contains('@') {
                SyntaxTokenKind::Token
            } else if text.chars().any(|character| character.is_ascii_digit()) {
                SyntaxTokenKind::Number
            } else {
                SyntaxTokenKind::Identifier
            };
            push_token(&mut output, source, kind, offset, end);
            offset = end;
            continue;
        }
        if bytes[offset].is_ascii_whitespace() {
            offset += 1;
            continue;
        }
        let end = take_word(source, offset);
        if end > offset {
            let text = &source[offset..end];
            let kind = if matches!(
                text,
                "studio"
                    | "script"
                    | "lang"
                    | "context"
                    | "on"
                    | "pressed"
                    | "changed"
                    | "submitted"
                    | "push"
                    | "replace"
                    | "pop"
                    | "pop-to"
                    | "reset"
            ) {
                SyntaxTokenKind::Keyword
            } else if in_tag {
                SyntaxTokenKind::Attribute
            } else {
                SyntaxTokenKind::Text
            };
            push_token(&mut output, source, kind, offset, end);
            offset = end;
        } else {
            let width = source[offset..].chars().next().map_or(1, char::len_utf8);
            push_token(
                &mut output,
                source,
                SyntaxTokenKind::Punctuation,
                offset,
                offset + width,
            );
            offset += width;
        }
    }
    output
}

fn take_word(source: &str, start: usize) -> usize {
    source[start..]
        .char_indices()
        .take_while(|(_, character)| {
            character.is_alphanumeric() || *character == '_' || *character == '-'
        })
        .last()
        .map_or(start, |(offset, character)| {
            start + offset + character.len_utf8()
        })
}

fn push_token(
    output: &mut Vec<SyntaxToken>,
    source: &str,
    kind: SyntaxTokenKind,
    start: usize,
    end: usize,
) {
    if start >= end || start >= source.len() {
        return;
    }
    let span = span_at(source, start);
    output.push(SyntaxToken {
        kind,
        start,
        end: end.min(source.len()),
        line: span.start.line,
        column: span.start.column,
        text: source[start..end.min(source.len())].to_owned(),
    });
}

fn transfer_trivia(old: &StudioDocument, new: &mut StudioDocument) {
    new.leading_comments = old.leading_comments.clone();
    new.trailing_comments = old.trailing_comments.clone();
    let mut old_elements = BTreeMap::new();
    fn collect<'a>(
        elements: &'a [Element],
        map: &mut BTreeMap<&'a str, (&'a [Comment], &'a [Comment])>,
    ) {
        for element in elements {
            map.insert(
                &element.id,
                (&element.leading_comments, &element.trailing_comments),
            );
            for child in &element.children {
                if let Node::Element(child) = child {
                    collect(std::slice::from_ref(child), map);
                }
            }
        }
    }
    collect(&old.nodes, &mut old_elements);
    fn apply(element: &mut Element, map: &BTreeMap<&str, (&[Comment], &[Comment])>) {
        if let Some((leading, trailing)) = map.get(element.id.as_str()) {
            element.leading_comments = leading.to_vec();
            element.trailing_comments = trailing.to_vec();
        }
        for child in &mut element.children {
            if let Node::Element(child) = child {
                apply(child, map);
            }
        }
    }
    for element in &mut new.nodes {
        apply(element, &old_elements);
    }
}
