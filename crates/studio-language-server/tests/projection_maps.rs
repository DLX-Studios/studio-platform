//! Projection mappings: definition, rename, and references over the
//! adapter symbol index, plus generated↔source round-trips.

use studio_language_server::{LanguageServer, Position, Workspace};

const CARD: &str = "<script lang=\"ts\">\n  let { title = \"Hi\" } = $props();\n  let quantity = $state(1);\n  function add() {\n    emit(\"add-one\", { quantity });\n  }\n</script>\n<Card id=\"a\">\n  <Column id=\"col\">\n    <Text id=\"t\">{title}</Text>\n    <Text id=\"u\">{title}</Text>\n    <Text id=\"b\">{quantity}</Text>\n    <Button id=\"c\" onclick={add}>Go</Button>\n  </Column>\n</Card>";

fn server_with(source: &str) -> LanguageServer {
    let mut server = LanguageServer::new(Workspace::new());
    server.open_document("file:///Card.studio", source);
    server
}

fn position_of(source: &str, needle: &str) -> Position {
    // Position inside the identifier (past any opening brace), so the lookup
    // lands on the name rather than surrounding syntax.
    let mut offset = source.find(needle).expect("needle in source");
    while source
        .as_bytes()
        .get(offset)
        .is_some_and(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_' && *byte != b'$')
    {
        offset += 1;
    }
    let line = source[..offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count();
    let character = source[..offset]
        .rsplit('\n')
        .next()
        .unwrap_or_default()
        .chars()
        .count();
    Position {
        line: u32::try_from(line).unwrap_or(u32::MAX),
        character: u32::try_from(character).unwrap_or(u32::MAX),
    }
}

#[test]
fn definition_jumps_to_declarations() {
    let server = server_with(CARD);
    // `quantity` use inside the template resolves to its $state declaration.
    let use_site = position_of(CARD, "{quantity}");
    let definitions = server.definition("file:///Card.studio", use_site);
    assert_eq!(definitions.len(), 1);
    assert!(definitions[0].uri.ends_with("Card.studio"));
    // `add` use resolves to the function declaration.
    let use_site = position_of(CARD, "{add}");
    let definitions = server.definition("file:///Card.studio", use_site);
    assert_eq!(definitions.len(), 1);
}

#[test]
fn rename_lists_every_occurrence() {
    let server = server_with(CARD);
    let use_site = position_of(CARD, "{quantity}");
    let edits = server.rename("file:///Card.studio", use_site, "amount");
    // Declaration, template use, and emission payload: at least three sites.
    assert!(edits.len() >= 3, "rename covers all sites: {}", edits.len());
    // Invalid rename targets are refused, not half-applied.
    let edits = server.rename("file:///Card.studio", use_site, "not a name");
    assert!(edits.is_empty());
}

#[test]
fn references_cover_declaration_and_uses() {
    let server = server_with(CARD);
    let use_site = position_of(CARD, "{title}");
    let locations = server.references("file:///Card.studio", use_site);
    // $props declaration, Card title attribute, Text interpolation.
    assert!(locations.len() >= 3, "references cover all sites");
}

#[test]
fn unknown_positions_resolve_to_nothing() {
    let server = server_with(CARD);
    let missing = server.definition(
        "file:///Card.studio",
        Position {
            line: 100,
            character: 0,
        },
    );
    assert!(missing.is_empty());
    assert!(
        server
            .rename(
                "file:///missing.studio",
                Position {
                    line: 0,
                    character: 0
                },
                "x"
            )
            .is_empty()
    );
}
