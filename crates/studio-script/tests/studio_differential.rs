//! US3 differential contract: the Rust evaluator and the compiled Wasm
//! artifact emit byte-identical observable protocol sequences for shared
//! fixtures, and both match the checked-in `protocol.jsonl` goldens.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::Command;

use studio_script::eval::{EmittedEvent, dispatch_event, mount_tree};
use studio_script::ir::StudioModule;

fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ir-v2")
        .join(name)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compile_fixture(name: &str) -> StudioModule {
    let source =
        std::fs::read_to_string(fixture_dir(name).join("source.studio")).expect("fixture readable");
    studio_script::compile_studio(&source, &format!("{name}.studio"), name).expect("compiles")
}

/// Scripted event sequences per fixture: (node id, trigger family).
fn scripted_events(name: &str) -> Vec<(String, String)> {
    match name {
        "typed-events" => vec![("events-add".to_owned(), "pressed".to_owned())],
        _ => Vec::new(),
    }
}

/// The evaluator leg: mount message plus one canonical emission per
/// scripted event.
fn eval_sequence(name: &str) -> Vec<String> {
    let module = compile_fixture(name);
    let component = &module.components[0];
    let (tree, scope) = mount_tree(&module, component, &BTreeMap::new()).unwrap();
    let mount = serde_json::to_string(&studio_protocol::GuestMessage::Mount(tree)).unwrap();
    let mut sequence = vec![mount];
    for (node, event) in scripted_events(name) {
        let emission: EmittedEvent =
            dispatch_event(component, &scope, &node, &event, &component.handlers)
                .unwrap()
                .expect("scripted event fires");
        sequence.push(emission.to_json());
    }
    sequence
}

/// The Wasm leg: emit `AssemblyScript`, compile with ASC, drive the module,
/// and capture every hostEmit call.
fn wasm_sequence(name: &str) -> Vec<String> {
    let module = compile_fixture(name);
    let emitted = studio_script::assemblyscript::emit_studio(&module).expect("emits");
    let work = tempfile::tempdir().expect("scratch dir");
    let assembly = work.path().join("module.ts");
    let wasm = work.path().join("module.wasm");
    std::fs::write(&assembly, &emitted.assembly_source).expect("write assembly");
    let asc = repo_root().join("sdk/assemblyscript/node_modules/assemblyscript/bin/asc.js");
    assert!(
        asc.is_file(),
        "asc compiler must be installed (bun install)"
    );
    let status = Command::new("bun")
        .arg(&asc)
        .arg(&assembly)
        .arg("--outFile")
        .arg(&wasm)
        .arg("--runtime")
        .arg("stub")
        .status()
        .expect("spawn asc");
    assert!(status.success(), "asc compiles generated module for {name}");
    let driver = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/drive-wasm.ts");
    let mut command = Command::new("bun");
    command.arg(&driver).arg(&wasm);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    let mut child = command.spawn().expect("spawn wasm driver");
    {
        let stdin = child.stdin.as_mut().expect("driver stdin");
        for (node, event) in scripted_events(name) {
            let line = serde_json::json!({
                "type": "ui",
                "payload": {"node_id": node, "event": event, "payload": {}},
            });
            writeln!(stdin, "{line}").expect("write event");
        }
    }
    let output = child.wait_with_output().expect("driver output");
    assert!(output.status.success(), "wasm driver runs for {name}");
    String::from_utf8(output.stdout)
        .expect("driver output is UTF-8")
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn evaluator_and_wasm_emit_identical_sequences() {
    for name in ["typed-events", "conditional", "keyed-each"] {
        let eval = eval_sequence(name);
        let wasm = wasm_sequence(name);
        assert_eq!(
            eval, wasm,
            "differential divergence for {name}: eval {eval:?} vs wasm {wasm:?}"
        );
    }
}

#[test]
fn sequences_match_checked_in_goldens() {
    for name in ["typed-events", "conditional", "keyed-each"] {
        let sequence = eval_sequence(name);
        let golden_path = fixture_dir(name).join("protocol.jsonl");
        let rendered = sequence.join("\n") + "\n";
        if std::env::var("BLESS").is_ok() {
            std::fs::write(&golden_path, &rendered).expect("bless golden");
            continue;
        }
        let golden = std::fs::read_to_string(&golden_path).unwrap_or_else(|_| {
            panic!("missing golden for {name}; run with BLESS=1, review, commit")
        });
        assert_eq!(rendered, golden, "protocol drift for {name}");
    }
}
