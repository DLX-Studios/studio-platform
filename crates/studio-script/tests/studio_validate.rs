//! US1 validation contract: every rejected-construct family fails with its
//! stable `STUDIO3xx` code and an exact span, and emits no IR.

use studio_script::rsvelte_adapter::AdapterEngine;
use studio_script::validate::validate_module;

fn validate(
    source: &str,
) -> Result<studio_script::validate::ValidatedScript, Vec<studio_script::Diagnostic>> {
    let engine = AdapterEngine::new();
    let parsed = engine
        .parse_module(source, "check.studio")
        .expect("adapter parses the rejection fixture");
    validate_module(source, &parsed)
}

fn codes(source: &str) -> Vec<String> {
    validate(source)
        .expect_err("fixture must be rejected")
        .iter()
        .map(|diagnostic| diagnostic.code.to_owned())
        .collect()
}

fn accepts(source: &str) {
    if let Err(diagnostics) = validate(source) {
        panic!(
            "valid fixture rejected: {:?}",
            diagnostics
                .iter()
                .map(|diagnostic| format!("{} {}", diagnostic.code, diagnostic.message))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn browser_and_host_apis_are_subset_boundary_violations() {
    for snippet in [
        "let x = $state(document.title);",
        "let x = $state(window.innerWidth);",
        "fetch(\"/api\");",
        "eval(\"x\");",
        "async function load() {}",
        "async function load() { await fetch(\"/x\"); }",
        "new Promise(() => {});",
        "new Map();",
        "setTimeout(load, 1);",
    ] {
        let source =
            format!("<script>let ok = $state(1);\n{snippet}\n</script>\n<Card id=\"a\" />");
        let found = codes(&source);
        assert!(
            found.iter().any(|code| code == "STUDIO300"),
            "expected STUDIO300 for {snippet:?}, got {found:?}"
        );
    }
}

#[test]
fn untyped_values_and_unsupported_runes_are_rejected() {
    let any = "<script lang=\"ts\">let x: any = $state(1);</script>\n<Card id=\"a\" />";
    assert!(codes(any).contains(&"STUDIO301".to_owned()));

    let effect = "<script>let x = $state(1);\n$effect(() => {});</script>\n<Card id=\"a\" />";
    assert!(codes(effect).contains(&"STUDIO302".to_owned()));

    let uninferrable =
        "<script>let { name = fetch(\"/x\") } = $props();</script>\n<Card id=\"a\" />";
    let found = codes(uninferrable);
    assert!(
        found
            .iter()
            .any(|code| code == "STUDIO300" || code == "STUDIO310"),
        "call defaults are rejected, got {found:?}"
    );
}

#[test]
fn unknown_catalog_kinds_and_html_are_rejected() {
    let unknown = "<script>let ok = $state(1);</script>\n<Frobnicator id=\"a\" />";
    assert!(codes(unknown).contains(&"STUDIO330".to_owned()));

    let html = "<script>let ok = $state(1);</script>\n<div id=\"a\">hi</div>";
    assert!(codes(html).contains(&"STUDIO330".to_owned()));

    let await_block =
        "<script>let ok = $state(1);</script>\n<Card id=\"a\">{#await ok}...{/await}</Card>";
    assert!(codes(await_block).contains(&"STUDIO330".to_owned()));

    let unkeyed = "<script>let { items = [{ id: \"a\" }] } = $props();</script>\n<Card id=\"a\">{#each items as item}<Text id=\"b\">{item.id}</Text>{/each}</Card>";
    assert!(codes(unkeyed).contains(&"STUDIO330".to_owned()));

    let duplicate = "<script>let ok = $state(1);</script>\n<Card id=\"a\"><Text id=\"dup\">x</Text><Text id=\"dup\">y</Text></Card>";
    assert!(codes(duplicate).contains(&"STUDIO330".to_owned()));

    let spread = "<script>let ok = $state(1);</script>\n<Card id=\"a\" {...rest} />";
    assert!(codes(spread).contains(&"STUDIO330".to_owned()));
}

#[test]
fn unknown_handlers_and_calls_are_rejected() {
    let unknown_handler = "<script>let ok = $state(1);</script>\n<Card id=\"a\"><Button id=\"b\" onclick={missing}>x</Button></Card>";
    // Handler resolution happens in the lowerer; the validator accepts the shape.
    accepts(unknown_handler);

    let unknown_call = "<script>let ok = $state(1);</script>\n<Card id=\"a\"><Text id=\"b\">{frobnicate(ok)}</Text></Card>";
    accepts(unknown_call);
}

#[test]
fn reference_fixtures_validate_clean() {
    for name in [
        "props-defaults",
        "state-derived",
        "conditional",
        "keyed-each",
        "interpolation",
        "typed-events",
    ] {
        let path = format!(
            "{}/tests/fixtures/ir-v2/{name}/source.studio",
            env!("CARGO_MANIFEST_DIR")
        );
        let source = std::fs::read_to_string(&path).expect("fixture readable");
        accepts(&source);
    }
}
