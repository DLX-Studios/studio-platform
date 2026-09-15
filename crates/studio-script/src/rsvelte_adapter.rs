//! Fenced `rsvelte` frontend: parse facts, template tree, and fingerprints.
//!
//! This is the ONLY module that names `rsvelte` types. Everything it returns
//! is Studio-owned: spans, owned strings, `serde_json` expression shapes, and
//! `studio_script::Diagnostic`. The forbidden `compile()`/`compile_both()`
//! code-generation entry points are never called.

use rsvelte::Engine;
use rsvelte_core::ast::js::Expression;
use rsvelte_core::ast::template as template_ast;
use rsvelte_core::ast::template::{Attribute, AttributeValue, Fragment, TemplateNode};

use crate::{Diagnostic, Location, Severity, Span};

/// Half-open byte span in the original source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSpan {
    /// Inclusive start byte offset.
    pub start: u32,
    /// Exclusive end byte offset.
    pub end: u32,
}

/// Convert a byte range of `source` into a Studio [`Span`].
#[must_use]
pub fn span_for(source: &str, start: u32, end: u32) -> Span {
    LineIndex::new(source).span(source, SourceSpan { start, end })
}

/// Line index for byte-offset to line/column conversion.
struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(source: &str) -> Self {
        let mut starts = vec![0usize];
        for (index, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(index + 1);
            }
        }
        Self { starts }
    }

    fn location(&self, source: &str, offset: u32) -> Location {
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        let line = self
            .starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        let line_start = self.starts[line.min(self.starts.len() - 1)];
        let column = source[line_start..offset.min(source.len())].chars().count() + 1;
        Location {
            line: line + 1,
            column,
            offset,
        }
    }

    fn span(&self, source: &str, span: SourceSpan) -> Span {
        Span {
            start: self.location(source, span.start),
            end: self.location(source, span.end),
        }
    }
}

/// Which `<script>` region an adapter script came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceScriptKind {
    /// Instance script (component state and behavior).
    Instance,
    /// Module script (shared, runs once).
    Module,
}

/// One `<script>` block with owned content.
#[derive(Clone, Debug)]
pub struct SourceScript {
    /// Instance or module context.
    pub kind: SourceScriptKind,
    /// Span of the whole block including tags.
    pub tag: SourceSpan,
    /// Span of the script content.
    pub content_span: SourceSpan,
    /// Whether `lang="ts"` was declared.
    pub typescript: bool,
    /// Owned script content.
    pub content: String,
}

/// One `<style>` block with owned content.
#[derive(Clone, Debug)]
pub struct SourceStyle {
    /// Span of the whole block including tags.
    pub tag: SourceSpan,
    /// Span of the style content.
    pub content_span: SourceSpan,
    /// Owned style content.
    pub content: String,
}

/// One expression with its source text and Svelte JSON shape.
#[derive(Clone, Debug)]
pub struct SourceExpression {
    /// Span of the expression in the original source.
    pub span: SourceSpan,
    /// Exact source text.
    pub text: String,
    /// Svelte-compatible JSON shape (`type`, operands, literals).
    pub json: serde_json::Value,
}

/// One template attribute.
#[derive(Clone, Debug)]
pub struct SourceAttribute {
    /// Span of the whole attribute.
    pub span: SourceSpan,
    /// Attribute name.
    pub name: String,
    /// Attribute value shape.
    pub value: SourceAttributeValue,
}

/// Template attribute value shapes relevant to Studio.
#[derive(Clone, Debug)]
pub enum SourceAttributeValue {
    /// Bare attribute (no value).
    True,
    /// Single `{expression}` value.
    Expression(SourceExpression),
    /// Text, possibly mixed with `{interpolation}` parts.
    Sequence(Vec<SourceAttributePart>),
}

/// One part of a mixed attribute value.
#[derive(Clone, Debug)]
pub enum SourceAttributePart {
    /// Literal text.
    Text(String),
    /// `{interpolation}` part.
    Interpolation(SourceExpression),
}

/// Studio-owned template tree: structure, spans, and texts only.
#[derive(Clone, Debug)]
pub enum SourceNode {
    /// Capitalized tag: a catalog component candidate.
    Component {
        /// Span of the whole element.
        span: SourceSpan,
        /// Tag name as written.
        name: String,
        /// Converted attributes in source order.
        attributes: Vec<SourceAttribute>,
        /// Converted children in source order.
        children: Vec<SourceNode>,
    },
    /// Lowercase tag: an HTML element (the validator rejects it).
    Element {
        /// Span of the whole element.
        span: SourceSpan,
        /// Tag name as written.
        name: String,
        /// Converted attributes in source order.
        attributes: Vec<SourceAttribute>,
        /// Converted children in source order.
        children: Vec<SourceNode>,
    },
    /// Literal text.
    Text {
        /// Span of the text.
        span: SourceSpan,
        /// Decoded text content.
        value: String,
    },
    /// `{expression}` interpolation.
    Interpolation(SourceExpression),
    /// `{#if}` block.
    If {
        /// Span of the whole block.
        span: SourceSpan,
        /// Condition expression.
        test: SourceExpression,
        /// Nodes rendered when the condition holds.
        consequent: Vec<SourceNode>,
        /// Nodes rendered otherwise.
        alternate: Vec<SourceNode>,
    },
    /// `{#each}` block.
    Each {
        /// Span of the whole block.
        span: SourceSpan,
        /// Iterated collection expression.
        collection: SourceExpression,
        /// Item binding name.
        item: String,
        /// Index binding name, if declared.
        index: Option<String>,
        /// Key expression, if declared.
        key: Option<SourceExpression>,
        /// Nodes rendered per item.
        body: Vec<SourceNode>,
        /// Nodes rendered for an empty collection.
        fallback: Vec<SourceNode>,
    },
    /// Anything else (`{#await}`, snippets, `{@html}`, directives, spread):
    /// represented so the validator can reject it with its span.
    Unsupported {
        /// Span of the unsupported construct.
        span: SourceSpan,
        /// Stable construct kind for diagnostics.
        kind: String,
    },
}

/// Template-shape flags from analysis.
#[derive(Clone, Debug, Default)]
pub struct TemplateShape {
    /// Whether legacy `$$props`/`$$restProps`/`$$slots` forms appear.
    pub uses_legacy_forms: bool,
    /// Whether `{@render}` tags appear.
    pub uses_render_tags: bool,
    /// Whether component bindings appear.
    pub uses_component_bindings: bool,
}

/// Owned analysis facts about one `.studio` source.
#[derive(Clone, Debug, Default)]
pub struct StudioFacts {
    /// Whether Svelte 5 rune syntax is used.
    pub runes: bool,
    /// Template-shape flags.
    pub shape: TemplateShape,
    /// Declared prop names in source order.
    pub props: Vec<String>,
    /// Declared export names in source order.
    pub exports: Vec<String>,
}

/// One fully parsed `.studio` source: facts, scripts, style, template.
#[derive(Clone, Debug)]
pub struct SourceModule {
    /// Analysis facts.
    pub facts: StudioFacts,
    /// `<script>` blocks in source order.
    pub scripts: Vec<SourceScript>,
    /// `<style>` block, if present.
    pub style: Option<SourceStyle>,
    /// Template forest.
    pub template: Vec<SourceNode>,
}

/// Schema-versioned fingerprint of the pinned frontend.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct AdapterFingerprint {
    /// Facade crate version.
    pub facade_version: String,
    /// Compiler implementation version.
    pub compiler_version: String,
    /// Targeted Svelte compatibility version.
    pub svelte_version: String,
    /// Facade schema version.
    pub api_schema: u32,
    /// Runtime artifact schema version.
    pub runtime_schema: u32,
    /// Facts schema version.
    pub facts_schema: u32,
}

/// The fenced frontend. Owns the parse arena contract internally.
pub struct AdapterEngine {
    engine: Engine,
    alloc: oxc_allocator::Allocator,
}

impl AdapterEngine {
    /// Construct the frontend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: Engine::new(),
            alloc: oxc_allocator::Allocator::default(),
        }
    }

    /// Schema-versioned fingerprint of the pinned frontend for cache keys.
    #[must_use]
    pub fn fingerprint(&self) -> AdapterFingerprint {
        let fingerprint = self.engine.fingerprint();
        AdapterFingerprint {
            facade_version: fingerprint.facade_version.to_owned(),
            compiler_version: fingerprint.compiler_version.to_owned(),
            svelte_version: fingerprint.svelte_version.to_owned(),
            api_schema: fingerprint.api_schema,
            runtime_schema: fingerprint.runtime_schema,
            facts_schema: fingerprint.facts_schema,
        }
    }

    /// Parse one `.studio` source into Studio-owned facts, scripts, style, and
    /// template. Fails with spanned diagnostics; no code is generated.
    ///
    /// # Errors
    ///
    /// Returns diagnostics when parsing fails.
    pub fn parse_module(
        &self,
        source: &str,
        filename: &str,
    ) -> Result<SourceModule, Vec<Diagnostic>> {
        let lines = LineIndex::new(source);
        // Studio Script is runes-based: force Svelte 5 rune parsing so
        // `$props`/`$state`/`$derived` are syntax, not parse errors.
        let options = rsvelte::ComponentOptions::new()
            .filename(filename.to_owned())
            .runes(Some(true));
        let prepared = self
            .engine
            .prepare(source, options)
            .map_err(|failure| Self::prepare_diagnostics(&lines, source, failure))?;
        let facts = prepared.facts();
        let root = rsvelte_core::parse(source, &self.alloc, rsvelte_core::ParseOptions::default())
            .map_err(|error| Self::core_diagnostics(&lines, source, &error))?;
        let mut root = root;
        if let Some(error) = rsvelte_core::resolve_lazy_expressions(&mut root, source) {
            return Err(Self::core_diagnostics(&lines, source, &error));
        }
        Ok(SourceModule {
            facts: StudioFacts {
                runes: facts.runes,
                shape: TemplateShape {
                    uses_legacy_forms: facts.uses_legacy_props
                        || facts.uses_legacy_rest_props
                        || facts.uses_legacy_slots,
                    uses_render_tags: facts.uses_render_tags,
                    uses_component_bindings: facts.uses_component_bindings,
                },
                props: facts.props.iter().map(|prop| prop.name.clone()).collect(),
                exports: facts
                    .exports
                    .iter()
                    .map(|export| export.name.clone())
                    .collect(),
            },
            scripts: facts
                .scripts
                .iter()
                .map(|script| SourceScript {
                    kind: match script.kind {
                        rsvelte::ScriptKind::Module => SourceScriptKind::Module,
                        _ => SourceScriptKind::Instance,
                    },
                    tag: byte_span(script.tag.start(), script.tag.end()),
                    content_span: byte_span(script.content.start(), script.content.end()),
                    typescript: script.typescript,
                    content: slice(source, script.content.start(), script.content.end()),
                })
                .collect(),
            style: facts.style.as_ref().map(|style| SourceStyle {
                tag: byte_span(style.tag.start(), style.tag.end()),
                content_span: byte_span(style.content.start(), style.content.end()),
                content: slice(source, style.content.start(), style.content.end()),
            }),
            template: convert_fragment(source, &lines, &root.fragment),
        })
    }

    fn prepare_diagnostics(
        lines: &LineIndex,
        source: &str,
        failure: rsvelte::CompileFailure,
    ) -> Vec<Diagnostic> {
        let diagnostic = failure.diagnostic;
        vec![syntax_diagnostic(
            lines,
            source,
            diagnostic.message,
            diagnostic.span.map(|span| (span.start(), span.end())),
        )]
    }

    fn core_diagnostics(
        lines: &LineIndex,
        source: &str,
        error: &rsvelte_core::error::ParseError,
    ) -> Vec<Diagnostic> {
        vec![syntax_diagnostic(lines, source, error.to_string(), None)]
    }
}

impl Default for AdapterEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a [`SourceSpan`] from half-open byte offsets.
fn byte_span(start: u32, end: u32) -> SourceSpan {
    SourceSpan { start, end }
}

fn slice(source: &str, start: u32, end: u32) -> String {
    let (start, end) = (start as usize, end as usize);
    source
        .get(start..end.min(source.len()).max(start.min(source.len())))
        .unwrap_or_default()
        .to_owned()
}

fn syntax_diagnostic(
    lines: &LineIndex,
    source: &str,
    message: String,
    range: Option<(u32, u32)>,
) -> Diagnostic {
    let span = match range {
        Some((start, end)) => lines.span(
            source,
            SourceSpan {
                start: start.min(end),
                end,
            },
        ),
        None => lines.span(source, SourceSpan { start: 0, end: 0 }),
    };
    Diagnostic {
        code: crate::CODE_SYNTAX,
        severity: Severity::Error,
        message,
        span,
    }
}

fn convert_fragment(source: &str, lines: &LineIndex, fragment: &Fragment<'_>) -> Vec<SourceNode> {
    fragment
        .nodes
        .iter()
        .map(|node| convert_node(source, lines, node))
        .collect()
}

fn convert_node(source: &str, lines: &LineIndex, node: &TemplateNode<'_>) -> SourceNode {
    match node {
        TemplateNode::Component(component) => SourceNode::Component {
            span: SourceSpan {
                start: component.start,
                end: component.end,
            },
            name: component.name.to_string(),
            attributes: convert_attributes(source, &component.attributes),
            children: convert_fragment(source, lines, &component.fragment),
        },
        TemplateNode::RegularElement(element) => SourceNode::Element {
            span: SourceSpan {
                start: element.start,
                end: element.end,
            },
            name: element.name.to_string(),
            attributes: convert_attributes(source, &element.attributes),
            children: convert_fragment(source, lines, &element.fragment),
        },
        TemplateNode::Text(text) => SourceNode::Text {
            span: SourceSpan {
                start: text.start,
                end: text.end,
            },
            value: text.data.to_string(),
        },
        TemplateNode::ExpressionTag(tag) => SourceNode::Interpolation(convert_expression(
            source,
            &tag.expression,
            tag.start,
            tag.end,
        )),
        TemplateNode::IfBlock(block) => SourceNode::If {
            span: SourceSpan {
                start: block.start,
                end: block.end,
            },
            test: convert_expression(source, &block.test, block.start, block.end),
            consequent: convert_fragment(source, lines, &block.consequent),
            alternate: block
                .alternate
                .as_ref()
                .map(|fragment| convert_fragment(source, lines, fragment))
                .unwrap_or_default(),
        },
        TemplateNode::EachBlock(block) => {
            let (item, index) = each_locals(block);
            SourceNode::Each {
                span: SourceSpan {
                    start: block.start,
                    end: block.end,
                },
                collection: convert_expression(source, &block.expression, block.start, block.end),
                item,
                index,
                key: block.key.as_ref().map(|expression| {
                    convert_expression(source, expression, block.start, block.end)
                }),
                body: convert_fragment(source, lines, &block.body),
                fallback: block
                    .fallback
                    .as_ref()
                    .map(|fragment| convert_fragment(source, lines, fragment))
                    .unwrap_or_default(),
            }
        }
        other => SourceNode::Unsupported {
            span: node_span(node),
            kind: node_kind(other),
        },
    }
}

fn node_span(node: &TemplateNode<'_>) -> SourceSpan {
    let (start, end) = match node {
        TemplateNode::AwaitBlock(block) => (block.start, block.end),
        TemplateNode::KeyBlock(block) => (block.start, block.end),
        TemplateNode::SnippetBlock(block) => (block.start, block.end),
        TemplateNode::RenderTag(tag) => (tag.start, tag.end),
        TemplateNode::HtmlTag(tag) => (tag.start, tag.end),
        TemplateNode::ConstTag(tag) => (tag.start, tag.end),
        TemplateNode::DebugTag(tag) => (tag.start, tag.end),
        _ => (0, 0),
    };
    SourceSpan { start, end }
}

fn node_kind(node: &TemplateNode<'_>) -> String {
    match node {
        TemplateNode::AwaitBlock(_) => "await-block".to_owned(),
        TemplateNode::KeyBlock(_) => "key-block".to_owned(),
        TemplateNode::SnippetBlock(_) => "snippet-block".to_owned(),
        TemplateNode::RenderTag(_) => "render-tag".to_owned(),
        TemplateNode::HtmlTag(_) => "html-tag".to_owned(),
        TemplateNode::ConstTag(_) => "const-tag".to_owned(),
        TemplateNode::DebugTag(_) => "debug-tag".to_owned(),
        TemplateNode::Comment(_) => "comment".to_owned(),
        TemplateNode::TitleElement(_) => "title-element".to_owned(),
        TemplateNode::SlotElement(_) => "slot-element".to_owned(),
        _ => "unsupported".to_owned(),
    }
}

fn each_locals(block: &template_ast::EachBlock<'_>) -> (String, Option<String>) {
    let item = block
        .context
        .as_ref()
        .and_then(|context| context.as_json().get("name")?.as_str().map(str::to_owned))
        .unwrap_or_default();
    (item, block.index.as_ref().map(ToString::to_string))
}

fn convert_attributes(source: &str, attributes: &[Attribute<'_>]) -> Vec<SourceAttribute> {
    let mut converted = Vec::new();
    for attribute in attributes {
        match attribute {
            Attribute::Attribute(node) => {
                let span = SourceSpan {
                    start: node.start,
                    end: node.end,
                };
                let value = match &node.value {
                    AttributeValue::True(_) => SourceAttributeValue::True,
                    AttributeValue::Expression(tag) => SourceAttributeValue::Expression(
                        convert_expression(source, &tag.expression, tag.start, tag.end),
                    ),
                    AttributeValue::Sequence(parts) => SourceAttributeValue::Sequence(
                        parts
                            .iter()
                            .map(|part| match part {
                                template_ast::AttributeValuePart::Text(text) => {
                                    SourceAttributePart::Text(text.data.to_string())
                                }
                                template_ast::AttributeValuePart::ExpressionTag(tag) => {
                                    SourceAttributePart::Interpolation(convert_expression(
                                        source,
                                        &tag.expression,
                                        tag.start,
                                        tag.end,
                                    ))
                                }
                            })
                            .collect(),
                    ),
                };
                converted.push(SourceAttribute {
                    span,
                    name: node.name.to_string(),
                    value,
                });
            }
            Attribute::SpreadAttribute(node) => converted.push(SourceAttribute {
                span: SourceSpan {
                    start: node.start,
                    end: node.end,
                },
                name: "{...spread}".to_owned(),
                value: SourceAttributeValue::Expression(convert_expression(
                    source,
                    &node.expression,
                    node.start,
                    node.end,
                )),
            }),
            other => converted.push(SourceAttribute {
                span: directive_span(other),
                name: directive_name(other),
                value: SourceAttributeValue::Expression(SourceExpression {
                    span: directive_span(other),
                    text: String::new(),
                    json: serde_json::Value::Null,
                }),
            }),
        }
    }
    converted
}

fn directive_span(attribute: &Attribute<'_>) -> SourceSpan {
    let (start, end) = match attribute {
        Attribute::BindDirective(node) => (node.start, node.end),
        Attribute::OnDirective(node) => (node.start, node.end),
        Attribute::ClassDirective(node) => (node.start, node.end),
        Attribute::StyleDirective(node) => (node.start, node.end),
        Attribute::TransitionDirective(node) => (node.start, node.end),
        Attribute::AnimateDirective(node) => (node.start, node.end),
        Attribute::UseDirective(node) => (node.start, node.end),
        Attribute::LetDirective(node) => (node.start, node.end),
        Attribute::AttachTag(node) => (node.start, node.end),
        _ => (0, 0),
    };
    SourceSpan { start, end }
}

fn directive_name(attribute: &Attribute<'_>) -> String {
    match attribute {
        Attribute::BindDirective(_) => "bind:".to_owned(),
        Attribute::OnDirective(_) => "on:".to_owned(),
        Attribute::ClassDirective(_) => "class:".to_owned(),
        Attribute::StyleDirective(_) => "style:".to_owned(),
        Attribute::TransitionDirective(_) => "transition:".to_owned(),
        Attribute::AnimateDirective(_) => "animate:".to_owned(),
        Attribute::UseDirective(_) => "use:".to_owned(),
        Attribute::LetDirective(_) => "let:".to_owned(),
        Attribute::AttachTag(_) => "attach".to_owned(),
        _ => "directive".to_owned(),
    }
}

fn convert_expression(
    source: &str,
    expression: &Expression<'_>,
    fallback_start: u32,
    fallback_end: u32,
) -> SourceExpression {
    let (json, start, end) = match expression {
        Expression::Typed(typed) => {
            let json = typed.as_json().clone();
            let (start, end) = json_span(&json, fallback_start, fallback_end);
            (json, start, end)
        }
        Expression::Lazy { start, end, .. } => (serde_json::Value::Null, *start, *end),
    };
    SourceExpression {
        span: SourceSpan { start, end },
        text: slice(source, start, end),
        json,
    }
}

fn json_span(json: &serde_json::Value, fallback_start: u32, fallback_end: u32) -> (u32, u32) {
    let start = json
        .pointer("/loc/start/offset")
        .or_else(|| json.pointer("/start"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|offset| u32::try_from(offset).ok())
        .unwrap_or(fallback_start);
    let end = json
        .pointer("/loc/end/offset")
        .or_else(|| json.pointer("/end"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|offset| u32::try_from(offset).ok())
        .unwrap_or(fallback_end);
    (start, end)
}
