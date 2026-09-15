//! US1 IR contract: reference fixtures compile to byte-identical golden
//! modules with stable identities and zero diagnostics.

use std::path::PathBuf;

fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ir-v2")
        .join(name)
}

fn compile_fixture(name: &str) -> studio_script::ir::StudioModule {
    let dir = fixture_dir(name);
    let source = std::fs::read_to_string(dir.join("source.studio")).expect("fixture readable");
    studio_script::compile_studio(&source, &format!("{name}.studio"), name).unwrap_or_else(
        |diagnostics| {
            panic!(
                "fixture {name} failed to compile: {:?}",
                diagnostics
                    .iter()
                    .map(|diagnostic| format!("{} {}", diagnostic.code, diagnostic.message))
                    .collect::<Vec<_>>()
            )
        },
    )
}

fn canonical_json(module: &studio_script::ir::StudioModule) -> String {
    let value = serde_json::to_value(module).expect("IR serializes");
    serde_json::to_string_pretty(&value).expect("IR formats") + "\n"
}

#[test]
fn reference_fixtures_match_golden_modules() {
    for name in [
        "props-defaults",
        "state-derived",
        "conditional",
        "keyed-each",
        "interpolation",
        "typed-events",
    ] {
        let dir = fixture_dir(name);
        let module = compile_fixture(name);
        let golden_path = dir.join("module.json");
        let rendered = canonical_json(&module);
        if std::env::var("BLESS").is_ok() {
            std::fs::write(&golden_path, &rendered).expect("bless golden");
            continue;
        }
        let golden = std::fs::read_to_string(&golden_path).unwrap_or_else(|_| {
            panic!("missing golden for {name}; run with BLESS=1, review, commit")
        });
        assert_eq!(rendered, golden, "golden module drift for {name}");
    }
}

#[test]
fn repeated_compilations_are_byte_identical() {
    for name in ["props-defaults", "typed-events", "keyed-each"] {
        let first = canonical_json(&compile_fixture(name));
        let second = canonical_json(&compile_fixture(name));
        assert_eq!(first, second, "identical sources produce identical IR");
    }
}

#[test]
fn golden_modules_carry_stable_identities_and_version() {
    let module = compile_fixture("typed-events");
    assert_eq!(module.version, studio_script::ir::STUDIO_IR_VERSION);
    assert_eq!(module.id, "typed-events");
    assert_eq!(module.components.len(), 1);
    let component = &module.components[0];
    assert_eq!(component.props.len(), 2);
    assert_eq!(component.state.len(), 1);
    assert_eq!(component.handlers.len(), 1);
    let handler = &component.handlers[0];
    assert_eq!(handler.event, "pressed");
    assert_eq!(handler.emit, "add-to-order");
    assert_eq!(handler.node, Some("events-add".to_owned()));
}
