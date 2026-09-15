//! Formatting goldens: deterministic output, idempotence, and degraded
//! behavior for rejected sources.

use studio_language_server::{LanguageServer, Workspace};

const CARD: &str = "<script lang=\"ts\">\n  let { title = \"Hi\" } = $props();\n</script>\n<Card id=\"a\" title={title}>\n  <Text id=\"b\">{title}</Text>\n</Card>\n";

fn server_with(source: &str) -> LanguageServer {
    let mut server = LanguageServer::new(Workspace::new());
    server.open_document("file:///Card.studio", source);
    server
}

#[test]
fn formatting_is_deterministic_and_idempotent() {
    let server = server_with(CARD);
    let first = server.format_document("file:///Card.studio").unwrap();
    assert!(first.contains("<Card"));
    let second = server.format_document("file:///Card.studio").unwrap();
    assert_eq!(first, second, "formatting is deterministic");
    // Idempotence through the module helper as well.
    let path = std::path::PathBuf::from("/tmp/studio-format-idempotence.svelte");
    assert!(studio_language_server::format::is_stable(&first, &path));
}

#[test]
fn rejected_sources_degrade_to_a_message() {
    let server = server_with("<Card");
    assert!(server.format_document("file:///Card.studio").is_err());
}

#[test]
fn unknown_documents_report_cleanly() {
    let server = LanguageServer::new(Workspace::new());
    assert_eq!(
        server.format_document("file:///missing.studio"),
        Err("unknown document".to_owned())
    );
}
