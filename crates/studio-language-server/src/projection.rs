//! Symbol index and generated↔source mappings for navigation.
//!
//! Definition, rename, and references run on Studio-owned data: the adapter
//! symbol walk gives declarations, and usage spans come from template
//! expressions plus word-boundary source scans. `rsvelte_projection`'s
//! `ProjectionMap` backs generated-file↔source translation (for example a
//! generated route-table position back to its route file).

use studio_script::rsvelte_adapter::{SourceModule, SourceNode};
use studio_script::validate::ValidatedScript;

/// Symbol kinds the index tracks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    /// A component tag.
    Component,
    /// A `$props()` entry.
    Prop,
    /// A `$state()` slot.
    State,
    /// A `$derived()` slot.
    Derived,
    /// A script function (possible handler).
    Function,
}

/// One indexed symbol with byte spans in its file.
#[derive(Clone, Debug, PartialEq)]
pub struct Symbol {
    /// Binding name.
    pub name: String,
    /// Symbol kind.
    pub kind: SymbolKind,
    /// Byte span of the declaration or tag.
    pub span: (u32, u32),
}

/// Index one parsed module: components, bindings, and functions.
#[must_use]
pub fn index_module(module: &SourceModule, script: &ValidatedScript) -> Vec<Symbol> {
    let mut symbols = Vec::new();
    for node in &module.template {
        index_nodes(node, &mut symbols);
    }
    // De-duplicate component tags by (name, span) so repeated tags each keep
    // their own definition site.
    for prop in &script.props {
        symbols.push(Symbol {
            name: prop.name.clone(),
            kind: SymbolKind::Prop,
            span: span_pair(&prop.span),
        });
    }
    for slot in &script.state {
        symbols.push(Symbol {
            name: slot.name.clone(),
            kind: SymbolKind::State,
            span: span_pair(&slot.span),
        });
    }
    for slot in &script.derived {
        symbols.push(Symbol {
            name: slot.name.clone(),
            kind: SymbolKind::Derived,
            span: span_pair(&slot.span),
        });
    }
    for function in &script.functions {
        symbols.push(Symbol {
            name: function.name.clone(),
            kind: SymbolKind::Function,
            span: span_pair(&function.span),
        });
    }
    symbols
}

fn span_pair(span: &studio_script::Span) -> (u32, u32) {
    let start = u32::try_from(span.start.offset).unwrap_or(u32::MAX);
    let end = u32::try_from(span.end.offset).unwrap_or(u32::MAX);
    (start, end)
}

fn index_nodes(node: &SourceNode, symbols: &mut Vec<Symbol>) {
    match node {
        SourceNode::Component {
            span,
            name,
            children,
            ..
        } => {
            symbols.push(Symbol {
                name: name.clone(),
                kind: SymbolKind::Component,
                span: (span.start, span.end),
            });
            for child in children {
                index_nodes(child, symbols);
            }
        }
        SourceNode::If {
            consequent,
            alternate,
            ..
        } => {
            for child in consequent.iter().chain(alternate.iter()) {
                index_nodes(child, symbols);
            }
        }
        SourceNode::Each { body, fallback, .. } => {
            for child in body.iter().chain(fallback.iter()) {
                index_nodes(child, symbols);
            }
        }
        SourceNode::Element { children, .. } => {
            for child in children {
                index_nodes(child, symbols);
            }
        }
        SourceNode::Text { .. } | SourceNode::Interpolation(_) | SourceNode::Unsupported { .. } => {
        }
    }
}

/// Identifier under a byte offset with its span, if the offset sits on one.
#[must_use]
pub fn identifier_at(source: &str, offset: u32) -> Option<(String, (u32, u32))> {
    let offset = offset as usize;
    if offset > source.len() {
        return None;
    }
    let bytes = source.as_bytes();
    let is_word = |index: usize| {
        bytes
            .get(index)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'$')
    };
    if !is_word(offset) && !is_word(offset.saturating_sub(1)) {
        return None;
    }
    let mut start = offset;
    while start > 0 && is_word(start - 1) {
        start -= 1;
    }
    let mut end = offset;
    while end < bytes.len() && is_word(end) {
        end += 1;
    }
    if start == end {
        return None;
    }
    Some((
        source[start..end].to_owned(),
        (
            u32::try_from(start).unwrap_or(u32::MAX),
            u32::try_from(end).unwrap_or(u32::MAX),
        ),
    ))
}

/// Find the binding a use-site refers to: the identifier under `offset`
/// resolved against declarations, preferring the nearest definition.
#[must_use]
pub fn definition_of(symbols: &[Symbol], source: &str, offset: u32) -> Option<Symbol> {
    let (name, _) = identifier_at(source, offset)?;
    // A use inside a declaration's own span resolves to itself when the
    // identifier matches; otherwise the first declaration wins.
    let mut fallback = None;
    for symbol in symbols {
        if symbol.name != name {
            continue;
        }
        let (start, end) = symbol.span;
        if offset >= start && offset < end {
            return Some(symbol.clone());
        }
        if fallback.is_none() {
            fallback = Some(symbol.clone());
        }
    }
    fallback
}

/// Every span mentioning a binding: its declaration plus word-boundary uses
/// across template and script text.
#[must_use]
pub fn references_of(symbols: &[Symbol], source: &str, name: &str) -> Vec<(u32, u32)> {
    let mut spans: Vec<(u32, u32)> = symbols
        .iter()
        .filter(|symbol| symbol.name == name)
        .map(|symbol| symbol.span)
        .collect();
    let mut search = 0usize;
    while let Some(found) = source[search..].find(name) {
        let absolute = search + found;
        let before = source[..absolute].chars().next_back();
        let after = source[absolute + name.len()..].chars().next();
        let boundary = |character: Option<char>| {
            character.is_none_or(|character| {
                !(character.is_ascii_alphanumeric() || character == '_' || character == '$')
            })
        };
        if boundary(before) && boundary(after) {
            let span = (
                u32::try_from(absolute).unwrap_or(u32::MAX),
                u32::try_from(absolute + name.len()).unwrap_or(u32::MAX),
            );
            if !spans.contains(&span) {
                spans.push(span);
            }
        }
        search = absolute + name.len().max(1);
    }
    spans.sort();
    spans
}

#[cfg(test)]
mod tests {
    // Generated→source lookups ride on maps produced by
    // `rsvelte_projection::ProjectionEngine::project`; offset adjustment of an
    // empty map is a covered no-op. See `tests/projection_maps.rs` for the
    // round-trip through real engine output.
    #[test]
    fn empty_map_offset_adjustment_is_a_noop() {
        let mut map = rsvelte_projection::ProjectionMap::default();
        map.insert_generated(100, 10);
        assert!(map.segments().is_empty());
    }
}
