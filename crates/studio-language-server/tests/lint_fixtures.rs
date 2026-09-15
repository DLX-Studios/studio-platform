//! Diagnostic fixtures: upstream Svelte rules and Studio rules merge into
//! one ordered, spanned stream with stable codes.

use studio_language_server::{LanguageServer, Workspace};

fn server_with(source: &str) -> LanguageServer {
    let mut server = LanguageServer::new(Workspace::new());
    server.open_document("file:///Check.studio", source);
    server
}

#[test]
fn studio_subset_violations_carry_codes_and_spans() {
    let server = server_with("<script>let x = $state(document.title);</script>\n<Card id=\"a\" />");
    let diagnostics = server.diagnostics("file:///Check.studio");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "STUDIO300"),
        "browser API violation reported, got {:?}",
        diagnostics
            .iter()
            .map(|diagnostic| &diagnostic.code)
            .collect::<Vec<_>>()
    );
    for diagnostic in &diagnostics {
        assert!(
            diagnostic.range.end.line > diagnostic.range.start.line
                || diagnostic.range.end.character > diagnostic.range.start.character,
            "spans are non-empty"
        );
    }
}

#[test]
fn unknown_catalog_kinds_are_studio_diagnostics() {
    let server = server_with("<script>let ok = $state(1);</script>\n<Frobnicator id=\"a\" />");
    let diagnostics = server.diagnostics("file:///Check.studio");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "STUDIO330")
    );
}

#[test]
fn valid_sources_produce_no_studio_errors() {
    let server = server_with(
        "<script lang=\"ts\">\n  let { title = \"Hi\" } = $props();\n</script>\n<Card id=\"a\">\n  <Column id=\"col\">\n    <Text id=\"b\">{title}</Text>\n  </Column>\n</Card>",
    );
    let studio_errors: Vec<_> = server
        .diagnostics("file:///Check.studio")
        .into_iter()
        .filter(|diagnostic| diagnostic.source == "studio-validator")
        .collect();
    assert!(
        studio_errors.is_empty(),
        "valid source is clean: {studio_errors:?}"
    );
}

#[test]
fn legacy_sources_keep_the_legacy_path() {
    let server = server_with("studio 1\nscreen home {\n}");
    // Legacy path: parser-of-record diagnostics, no Studio-validator codes.
    let diagnostics = server.diagnostics("file:///Check.studio");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.source != "studio-validator")
    );
}
