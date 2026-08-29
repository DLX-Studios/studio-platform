//! Lowering from the Studio Script semantic model into the typed IR.
//!
//! The lowerer walks the parser-of-record document, keeps the static subset
//! (literal properties, text leaves, catalog elements), parses the declarative
//! behavior grammar from the `<script lang="studio">` block, and rejects every
//! construct outside the v1 skeleton subset with a stable, source-linked
//! diagnostic.  Nothing is silently omitted.
//!
//! # Behavior grammar
//!
//! The script block is line-oriented and closed:
//!
//! ```text
//! line = blank | comment | statement ;
//! statement = "on" wsp event wsp node-id wsp action [ ";" ] ;
//! event = "pressed" | "changed" | "submitted" ;
//! action = "push" "(" route ")" | "replace" "(" route ")"
//!        | "pop-to" "(" route ")" | "reset" "(" route ")"
//!        | "pop" "(" ")" ;
//! route = "/" segment ( "/" segment )* ;
//! comment = "#" (!newline)* ;
//! ```
//!
//! Statements execute in authored order; duplicate triggers for one node and
//! event pair are rejected.

use std::collections::BTreeSet;

use crate::{AttributeValue, Diagnostic, Element, Location, Node, Severity, Span, StudioDocument};
use studio_protocol::NodeKind;

use crate::ir::{
    IrElement, IrNavigationAction, IrNavigationOperation, IrNode, IrProperty, IrScreen, IrText,
    IrTriggerEvent,
};

/// Stable diagnostic code for a `$item.*` binding outside the static subset.
pub const CODE_IR_UNSUPPORTED_BINDING: &str = "STUDIO201";
/// Stable diagnostic code for a malformed behavior statement.
pub const CODE_IR_BEHAVIOR_SYNTAX: &str = "STUDIO202";
/// Stable diagnostic code for an unknown trigger-event or action keyword.
pub const CODE_IR_UNKNOWN_KEYWORD: &str = "STUDIO203";
/// Stable diagnostic code for a duplicate trigger node/event pair.
pub const CODE_IR_DUPLICATE_TRIGGER: &str = "STUDIO204";
/// Stable diagnostic code for a navigation target with no matching screen.
pub const CODE_IR_UNKNOWN_TARGET: &str = "STUDIO205";
/// Stable diagnostic code for a kind outside the v1 runtime catalog.
pub const CODE_IR_UNKNOWN_KIND: &str = "STUDIO206";
/// Stable diagnostic code for a token reference that needs Library resolution.
pub const CODE_IR_UNRESOLVED_TOKEN: &str = "STUDIO207";
/// Stable diagnostic code for a trigger naming no known node identity.
pub const CODE_IR_UNKNOWN_TRIGGER_NODE: &str = "STUDIO208";

const MAX_ROUTE_BYTES: usize = 128;
const DOCUMENT_POINT: Location = Location {
    line: 1,
    column: 1,
    offset: 0,
};

/// A lowering failure containing all diagnostics found, ordered by source
/// discovery: document-tree diagnostics first, then behavior-statement ones.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "Studio Script lowering contains {count} diagnostic(s)",
    count = diagnostics.len()
)]
pub struct LowerError {
    /// Stable diagnostics collected during lowering.
    pub diagnostics: Vec<Diagnostic>,
}

impl LowerError {
    /// Return the diagnostics without exposing lowering internals.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// Lower a parsed Studio Script document into the typed IR.
///
/// `source` is the original text the document was parsed from; it backs the
/// source spans of element and behavior diagnostics.  Behavior statements are
/// parsed from the raw `<script>` region so their spans are exact.
///
/// # Errors
///
/// Returns [`LowerError`] when any construct falls outside the supported
/// subset, references an unknown identity or screen, or duplicates a trigger.
pub fn lower_document(
    document: &StudioDocument,
    source: &str,
) -> Result<crate::ir::StudioIrModule, LowerError> {
    let mut diagnostics = Vec::new();
    let mut screens = Vec::new();
    let mut routes = BTreeSet::new();

    let mut ids = BTreeSet::new();
    for root in &document.nodes {
        collect_ids_from_element(root, &mut ids);
    }

    for root in &document.nodes {
        let screen = lower_screen(root, source, &mut diagnostics);
        routes.insert(screen.route.clone());
        screens.push(screen);
    }

    let actions = if document.script.is_some() {
        lower_behaviors(source, &ids, &routes, &mut diagnostics)
    } else {
        Vec::new()
    };

    if diagnostics.is_empty() {
        Ok(crate::ir::StudioIrModule {
            version: crate::ir::STUDIO_IR_VERSION,
            screens,
            actions,
        })
    } else {
        Err(LowerError { diagnostics })
    }
}

fn collect_ids(node: &Node, ids: &mut BTreeSet<String>) {
    if let Node::Element(element) = node {
        collect_ids_from_element(element, ids);
    }
}

fn collect_ids_from_element(element: &Element, ids: &mut BTreeSet<String>) {
    ids.insert(element.id.clone());
    for child in &element.children {
        collect_ids(child, ids);
    }
}

fn lower_screen(root: &Element, source: &str, diagnostics: &mut Vec<Diagnostic>) -> IrScreen {
    let root_node = lower_node(root, source, diagnostics);
    IrScreen {
        id: root.id.clone(),
        route: format!("/{}", root.id),
        root: root_node,
    }
}

fn lower_node(element: &Element, source: &str, diagnostics: &mut Vec<Diagnostic>) -> IrNode {
    let span = locate_element(source, &element.id);
    validate_kind(&element.kind, span, diagnostics);

    let mut properties = Vec::new();
    for (name, value) in &element.attributes {
        match value {
            AttributeValue::String(value) => properties.push((
                name.clone(),
                IrProperty::String(value.clone()),
            )),
            AttributeValue::Boolean(value) => {
                properties.push((name.clone(), IrProperty::Boolean(*value)));
            }
            AttributeValue::Number(value) => properties.push((
                name.clone(),
                IrProperty::Number(value.clone()),
            )),
            AttributeValue::Binding(binding) => diagnostics.push(Diagnostic {
                code: CODE_IR_UNSUPPORTED_BINDING,
                severity: Severity::Error,
                message: format!(
                    "binding `{}` on node `{}` is outside the static wasm subset",
                    binding.path, element.id
                ),
                span,
            }),
            AttributeValue::Token(token) => diagnostics.push(Diagnostic {
                code: CODE_IR_UNRESOLVED_TOKEN,
                severity: Severity::Error,
                message: format!(
                    "token reference `{}` on node `{}` requires Library resolution beyond this skeleton",
                    token.path, element.id
                ),
                span,
            }),
        }
    }

    let mut children = Vec::new();
    let mut text_ordinal = 0;
    for child in &element.children {
        match child {
            Node::Element(child) if child.kind.eq_ignore_ascii_case("text") => {
                validate_kind(&child.kind, locate_element(source, &child.id), diagnostics);
                text_ordinal += 1;
                let text = child
                    .children
                    .iter()
                    .find_map(|nested| match nested {
                        Node::Text(text) => Some(text.text.clone()),
                        Node::Element(_) => None,
                    })
                    .unwrap_or_default();
                children.push(IrNode::Text(IrText {
                    id: format!("{}-text-{text_ordinal}", element.id),
                    span: locate_text(source, &text),
                    text,
                }));
            }
            Node::Element(child) => children.push(lower_node(child, source, diagnostics)),
            Node::Text(text) => {
                text_ordinal += 1;
                children.push(IrNode::Text(IrText {
                    id: format!("{}-text-{text_ordinal}", element.id),
                    text: text.text.clone(),
                    span: locate_text(source, &text.text),
                }));
            }
        }
    }
    // Preserve authored child order: it is observable in a static screen
    // tree, while property order is normalized below for deterministic output.
    properties.sort_by(|left, right| left.0.cmp(&right.0));

    IrNode::Element(IrElement {
        id: element.id.clone(),
        kind: catalog_kind_name(&element.kind),
        properties,
        children,
        span,
    })
}

fn validate_kind(kind: &str, span: Span, diagnostics: &mut Vec<Diagnostic>) {
    if kind.eq_ignore_ascii_case("screen") {
        return;
    }
    let quoted = format!("\"{}\"", catalog_kind_name(kind));
    if serde_json::from_str::<NodeKind>(&quoted).is_err() {
        diagnostics.push(Diagnostic {
            code: CODE_IR_UNKNOWN_KIND,
            severity: Severity::Error,
            message: format!("element kind `{kind}` is not in the v1 runtime catalog"),
            span,
        });
    }
}

fn catalog_kind_name(kind: &str) -> String {
    if kind.eq_ignore_ascii_case("screen") {
        return "screen".to_owned();
    }
    if kind.eq_ignore_ascii_case("list") {
        return "list_view".to_owned();
    }
    let mut name = String::with_capacity(kind.len() + 4);
    for (index, character) in kind.chars().enumerate() {
        if character.is_ascii_uppercase() && index != 0 {
            name.push('_');
        }
        name.push(character.to_ascii_lowercase());
    }
    name
}

struct RawBehavior {
    node_id: String,
    event: IrTriggerEvent,
    operation: IrNavigationOperation,
    span: Span,
}

#[allow(clippy::too_many_lines)]
fn lower_behaviors(
    source: &str,
    ids: &BTreeSet<String>,
    routes: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<IrNavigationAction> {
    let Some((region_start, region)) = script_region(source) else {
        // UNVERIFIED: quoted attribute values containing `>` could shift the
        // located opening-tag end; the parser model itself carries no offsets,
        // so relocation failure is reported instead of skipped silently.
        diagnostics.push(Diagnostic {
            code: CODE_IR_BEHAVIOR_SYNTAX,
            severity: Severity::Error,
            message: "the <script> block could not be relocated in the source; \
                      behavior statements were not lowered"
                .to_owned(),
            span: Span::point(DOCUMENT_POINT),
        });
        return Vec::new();
    };

    let mut actions = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for (line_index, raw_line) in region.split('\n').enumerate() {
        let line_start = if line_index == 0 {
            region_start
        } else {
            region_start + cumulative_line_bytes(region, line_index)
        };

        let statement = match parse_statement(raw_line, line_start, source) {
            Ok(Some(behavior)) => behavior,
            Ok(None) => continue,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };

        if !ids.contains(&statement.node_id) {
            diagnostics.push(Diagnostic {
                code: CODE_IR_UNKNOWN_TRIGGER_NODE,
                severity: Severity::Error,
                message: format!(
                    "behavior triggers node `{}`, which no element defines",
                    statement.node_id
                ),
                span: statement.span,
            });
            continue;
        }
        if let Some(route) = statement.operation.route()
            && !routes.contains(route)
        {
            diagnostics.push(Diagnostic {
                code: CODE_IR_UNKNOWN_TARGET,
                severity: Severity::Error,
                message: format!("navigation target `{route}` matches no screen"),
                span: statement.span,
            });
            continue;
        }
        let key = format!(
            "{event}|{node}",
            event = statement.event.as_str(),
            node = statement.node_id
        );
        if !seen.insert(key) {
            diagnostics.push(Diagnostic {
                code: CODE_IR_DUPLICATE_TRIGGER,
                severity: Severity::Error,
                message: format!(
                    "node `{}` already declares a `{}` behavior",
                    statement.node_id,
                    statement.event.as_str()
                ),
                span: statement.span,
            });
            continue;
        }

        actions.push(IrNavigationAction {
            trigger_node_id: statement.node_id,
            trigger_event: statement.event,
            operation: statement.operation,
            span: statement.span,
        });
    }

    actions
}

/// Sum the byte length of every line before `line_index`, including the
/// newline separators that precede it.
fn cumulative_line_bytes(region: &str, line_index: usize) -> usize {
    region
        .split('\n')
        .take(line_index)
        .map(|line| line.len() + 1)
        .sum()
}

type StatementResult = Result<Option<RawBehavior>, Diagnostic>;

#[allow(clippy::too_many_lines)]
fn parse_statement(raw_line: &str, line_start: usize, source: &str) -> StatementResult {
    let without_comment = match raw_line.find('#') {
        Some(hash) => &raw_line[..hash],
        None => raw_line,
    };
    let trimmed = without_comment.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let trimmed = trimmed.strip_suffix(';').unwrap_or(trimmed).trim_end();

    let leading = raw_line.len() - raw_line.trim_start().len();
    let statement_start = line_start + leading;
    let statement_span = Span::point(location_at_offset_in(source, statement_start));

    let mut parts = trimmed.split_whitespace();
    let Some(keyword) = parts.next() else {
        return Ok(None);
    };
    if keyword != "on" {
        return Err(Diagnostic {
            code: CODE_IR_BEHAVIOR_SYNTAX,
            severity: Severity::Error,
            message: format!("behavior statements start with `on`, found `{keyword}`"),
            span: statement_span,
        });
    }

    let error = |code: &'static str, message: String| {
        Err(Diagnostic {
            code,
            severity: Severity::Error,
            message,
            span: statement_span,
        })
    };

    let Some(event_keyword) = parts.next() else {
        return error(
            CODE_IR_BEHAVIOR_SYNTAX,
            "behavior statements need a trigger event after `on`".to_owned(),
        );
    };
    let Some(event) = IrTriggerEvent::parse(event_keyword) else {
        return error(
            CODE_IR_UNKNOWN_KEYWORD,
            format!("unknown trigger event `{event_keyword}`"),
        );
    };

    let Some(node_id) = parts.next() else {
        return error(
            CODE_IR_BEHAVIOR_SYNTAX,
            "behavior statements need a node identity after the event".to_owned(),
        );
    };

    let action = parts.collect::<Vec<_>>().join(" ");
    let Some(operation) = parse_action(&action) else {
        return error(
            CODE_IR_UNKNOWN_KEYWORD,
            format!("unknown navigation action `{action}`"),
        );
    };

    Ok(Some(RawBehavior {
        node_id: node_id.to_owned(),
        event,
        operation,
        span: statement_span,
    }))
}

fn parse_action(action: &str) -> Option<IrNavigationOperation> {
    let open = action.find('(')?;
    if !action.ends_with(')') {
        return None;
    }
    let name = &action[..open];
    let argument = action[open + 1..action.len() - 1].trim();
    let route = |argument: &str| -> Option<String> {
        if is_valid_route(argument) {
            Some(argument.to_owned())
        } else {
            None
        }
    };
    match name {
        "pop" if argument.is_empty() => Some(IrNavigationOperation::Pop),
        "push" => route(argument).map(|route| IrNavigationOperation::Push { route }),
        "replace" => route(argument).map(|route| IrNavigationOperation::Replace { route }),
        "pop-to" => route(argument).map(|route| IrNavigationOperation::PopTo { route }),
        "reset" => route(argument).map(|route| IrNavigationOperation::Reset { route }),
        _ => None,
    }
}

fn is_valid_route(route: &str) -> bool {
    route.len() <= MAX_ROUTE_BYTES
        && route.starts_with('/')
        && route[1..].split('/').all(is_route_segment)
}

fn is_route_segment(segment: &str) -> bool {
    let mut characters = segment.chars();
    matches!(characters.next(), Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit())
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

/// Locate the `<script>` body region as absolute `(start_offset, text)`.
///
/// The parser model intentionally carries no offsets, so the region is
/// relocated by scanning for the opening tag.  Quoted attribute values
/// containing `>` are the known approximation seam.
fn script_region(source: &str) -> Option<(usize, &str)> {
    let open = source.find("<script")?;
    let tag_end = open + source[open..].find('>')?;
    let close = source[tag_end..].find("</script>")?;
    let start = tag_end + '>'.len_utf8();
    Some((start, &source[start..tag_end + close]))
}

fn locate_element(source: &str, id: &str) -> Span {
    let needle = format!("id=\"{id}\"");
    match source.find(&needle) {
        Some(offset) => Span::point(location_at_offset_in(source, offset)),
        // Manually constructed models may not appear in any source text.
        None => Span::point(DOCUMENT_POINT),
    }
}

fn locate_text(source: &str, text: &str) -> Span {
    match source.find(text) {
        Some(offset) => Span::point(location_at_offset_in(source, offset)),
        None => Span::point(DOCUMENT_POINT),
    }
}

fn location_at_offset_in(source: &str, offset: usize) -> Location {
    let bounded = offset.min(source.len());
    let prefix = &source[..bounded];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit('\n')
        .next()
        .map_or(1, |tail| tail.chars().count() + 1);
    Location {
        line,
        column,
        offset: bounded,
    }
}
