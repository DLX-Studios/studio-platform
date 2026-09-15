//! Failure isolation: upstream tooling failures degrade to diagnostics
//! while the Studio validator, IR pipeline, and release builds behave
//! identically with or without tooling.

use studio_language_server::{LanguageServer, Workspace};

fn server_with(source: &str) -> LanguageServer {
    let mut server = LanguageServer::new(Workspace::new());
    server.open_document("file:///Iso.studio", source);
    server
}

#[test]
fn malformed_inputs_never_crash_requests() {
    for source in [
        "",
        "<",
        "<Card",
        "<script>",
        "{#if}",
        "\u{0}",
        &"x".repeat(100_000),
    ] {
        let server = server_with(source);
        let _ = server.diagnostics("file:///Iso.studio");
        let _ = server.format_document("file:///Iso.studio");
        let _ = server.completion(
            "file:///Iso.studio",
            studio_language_server::Position {
                line: 0,
                character: 0,
            },
        );
        let _ = server.hover(
            "file:///Iso.studio",
            studio_language_server::Position {
                line: 0,
                character: 0,
            },
        );
        let _ = server.definition(
            "file:///Iso.studio",
            studio_language_server::Position {
                line: 0,
                character: 0,
            },
        );
        let _ = server.rename(
            "file:///Iso.studio",
            studio_language_server::Position {
                line: 0,
                character: 0,
            },
            "x",
        );
        let _ = server.references(
            "file:///Iso.studio",
            studio_language_server::Position {
                line: 0,
                character: 0,
            },
        );
    }
}

#[test]
fn formatter_rejection_leaves_sources_untouched() {
    // Unparseable input: formatting fails and the source is returned
    // unchanged by the caller contract (Err, never partial output).
    let server = server_with("<Card");
    assert!(server.format_document("file:///Iso.studio").is_err());
}

#[test]
fn compiler_answers_identically_with_tooling_present() {
    // The validator path used by builds does not consult tooling state:
    // identical sources validate identically regardless of server activity.
    let source = "<script lang=\"ts\">\n  let { title = \"Hi\" } = $props();\n</script>\n<Card id=\"a\" title={title} />";
    let server = server_with(source);
    let diagnostics = server.diagnostics("file:///Iso.studio");
    let direct = studio_script::compile_studio(source, "Iso.studio", "Iso");
    assert_eq!(
        direct.is_ok(),
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.source == "studio-validator")
    );
}
