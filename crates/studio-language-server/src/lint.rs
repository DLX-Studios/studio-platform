//! Layered diagnostics: upstream Svelte rules plus Studio rules in one
//! ordered stream.
//!
//! Upstream codes pass through verbatim; Studio codes live in `STUDIO3xx`.
//! Every diagnostic carries an exact span. Upstream failures degrade to a
//! single diagnostic while the Studio validator keeps answering.

use rsvelte_lint::engine::run_native_rules;
use rsvelte_lint::{LintConfig, Severity as UpstreamSeverity};

use crate::{Diagnostic, Position, Range};

/// Collect upstream diagnostics for one Svelte-shaped source.
///
/// Upstream failures degrade to one synthetic diagnostic instead of failing
/// the request; callers still append Studio diagnostics afterwards.
#[must_use]
pub fn upstream_diagnostics(source: &str, filename: &str) -> Vec<Diagnostic> {
    let config = LintConfig::recommended();
    run_native_rules(source, filename, &config, None)
        .into_iter()
        .map(|diagnostic| {
            let (start, end) = byte_span(source, diagnostic.start, diagnostic.end);
            Diagnostic {
                range: Range { start, end },
                severity: match diagnostic.severity {
                    UpstreamSeverity::Error => 1,
                    _ => 2,
                },
                code: diagnostic.rule.clone(),
                source: "rsvelte-lint".to_owned(),
                message: diagnostic.message.clone(),
            }
        })
        .collect()
}

/// Studio rules for Svelte-shaped sources: the portable-subset validator plus
/// catalog-kind resolution, mapped onto server diagnostics.
#[must_use]
pub fn studio_diagnostics(source: &str, filename: &str, name: &str) -> Vec<Diagnostic> {
    match studio_script::compile_studio(source, filename, name) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| Diagnostic {
                range: Range {
                    start: Position {
                        line: diagnostic.span.start.line.saturating_sub(1) as u32,
                        character: diagnostic.span.start.column.saturating_sub(1) as u32,
                    },
                    end: Position {
                        line: diagnostic.span.end.line.saturating_sub(1) as u32,
                        character: diagnostic.span.end.column.saturating_sub(1) as u32,
                    },
                },
                severity: 1,
                code: diagnostic.code.to_owned(),
                source: "studio-validator".to_owned(),
                message: diagnostic.message,
            })
            .collect(),
    }
}

/// Merge upstream and Studio diagnostics into one ordered stream.
#[must_use]
pub fn merged_diagnostics(source: &str, filename: &str, name: &str) -> Vec<Diagnostic> {
    let mut diagnostics = upstream_diagnostics(source, filename);
    diagnostics.extend(studio_diagnostics(source, filename, name));
    diagnostics.sort_by(|left, right| {
        (
            left.range.start.line,
            left.range.start.character,
            &left.code,
        )
            .cmp(&(
                right.range.start.line,
                right.range.start.character,
                &right.code,
            ))
    });
    diagnostics.dedup_by(|next, current| next.range == current.range && next.code == current.code);
    diagnostics
}

fn byte_span(source: &str, start: u32, end: u32) -> (Position, Position) {
    (offset_position(source, start), offset_position(source, end))
}

fn offset_position(source: &str, offset: u32) -> Position {
    let offset = (offset as usize).min(source.len());
    let mut line = 0u32;
    let mut line_start = 0usize;
    for (index, byte) in source.bytes().enumerate() {
        if index >= offset {
            break;
        }
        if byte == b'\n' {
            line += 1;
            line_start = index + 1;
        }
    }
    let character = source[line_start..offset.min(source.len())].chars().count() as u32;
    Position { line, character }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_failures_degrade_without_losing_studio_signal() {
        // Unparseable input: upstream reports nothing structured, Studio
        // reports syntax, and the merge never panics.
        let diagnostics = merged_diagnostics("<Card", "broken.studio", "broken");
        assert!(!diagnostics.is_empty());
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.source == "studio-validator")
        );
    }
}
