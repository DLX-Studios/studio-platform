//! 009 interactions: scripted event sequences agree byte for byte between
//! the evaluator leg and the compiled-Wasm leg, and every patch validates
//! under the protocol.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::PathBuf;

use studio_script::eval::InteractionSession;

fn fixture_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/interactions/counter/source.studio");
    std::fs::read_to_string(&path).expect("fixture readable")
}

fn compile_fixture() -> studio_script::ir::StudioModule {
    studio_script::compile_studio(&fixture_source(), "counter.studio", "counter").unwrap_or_else(
        |diagnostics| {
            panic!(
                "fixture failed to compile: {:?}",
                diagnostics
                    .iter()
                    .map(|diagnostic| format!("{} {}", diagnostic.code, diagnostic.message))
                    .collect::<Vec<_>>()
            )
        },
    )
}

const SCRIPT: [(&str, &str); 3] = [
    ("counter-inc", "pressed"),
    ("counter-inc", "pressed"),
    ("counter-dec", "pressed"),
];

fn eval_sequence() -> Vec<String> {
    let module = compile_fixture();
    let component = &module.components[0];
    let (tree, mut session) =
        InteractionSession::mount(&module, component, &BTreeMap::new()).expect("mounts");
    let mut sequence = vec![
        serde_json::to_string(&studio_protocol::GuestMessage::Mount(tree)).expect("mount json"),
    ];
    for (node, event) in SCRIPT {
        let message = session
            .apply(component, node, event)
            .expect("event applies");
        sequence.push(message.to_json());
    }
    sequence
}

fn wasm_sequence() -> Vec<String> {
    let module = compile_fixture();
    let emitted = studio_script::assemblyscript::emit_studio(&module).expect("emits");
    let work = tempfile::tempdir().expect("scratch dir");
    let assembly = work.path().join("module.ts");
    let wasm = work.path().join("module.wasm");
    std::fs::write(&assembly, &emitted.assembly_source).expect("write assembly");
    let asc = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../sdk/assemblyscript/node_modules/assemblyscript/bin/asc.js");
    assert!(
        asc.is_file(),
        "asc compiler must be installed (bun install)"
    );
    let status = std::process::Command::new("bun")
        .arg(&asc)
        .arg(&assembly)
        .arg("--outFile")
        .arg(&wasm)
        .arg("--runtime")
        .arg("stub")
        .status()
        .expect("spawn asc");
    assert!(status.success(), "asc compiles interaction module");
    let driver = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/drive-wasm.ts");
    let mut command = std::process::Command::new("bun");
    command.arg(&driver).arg(&wasm);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    let mut child = command.spawn().expect("spawn wasm driver");
    {
        let stdin = child.stdin.as_mut().expect("driver stdin");
        for (node, event) in SCRIPT {
            let line = serde_json::json!({
                "type": "ui",
                "payload": {"node_id": node, "event": event, "payload": {}},
            });
            writeln!(stdin, "{line}").expect("write event");
        }
    }
    let output = child.wait_with_output().expect("driver output");
    assert!(output.status.success(), "wasm driver runs");
    String::from_utf8(output.stdout)
        .expect("driver output is UTF-8")
        .lines()
        .map(str::to_owned)
        .collect()
}

fn parse_sequence(sequence: &[String]) -> Vec<serde_json::Value> {
    sequence
        .iter()
        .map(|line| serde_json::from_str(line).expect("sequence line is JSON"))
        .collect()
}

#[test]
fn interaction_sequences_agree_across_legs() {
    let eval = eval_sequence();
    let wasm = wasm_sequence();
    assert_eq!(eval, wasm, "eval and wasm legs agree byte for byte");
}

#[test]
fn patches_advance_state_with_monotonic_sequences() {
    let values = parse_sequence(&eval_sequence());
    assert_eq!(values.len(), 4, "mount plus three patches");
    assert_eq!(values[0]["type"], "mount");
    // Content props patch as strings: the host renders `text`/`label`
    // from string props only.
    let counts = ["1", "2", "1"];
    let totals = ["2999", "5998", "2999"];
    for (index, message) in values[1..].iter().enumerate() {
        assert_eq!(message["type"], "patch", "event {index} patches");
        assert_eq!(
            message["payload"]["sequence"],
            serde_json::json!((index + 1) as u64),
            "sequence advances monotonically"
        );
        let operations = message["payload"]["operations"]
            .as_array()
            .expect("patch operations");
        assert!(!operations.is_empty(), "patches carry content ops");
        for operation in operations {
            assert_eq!(operation["op"], "update_prop");
        }
        let value_of = |node: &str| {
            operations
                .iter()
                .find(|operation| operation["node_id"] == node)
                .unwrap_or_else(|| panic!("patch covers {node}"))["value"]
                .clone()
        };
        assert_eq!(value_of("counter-value"), serde_json::json!(counts[index]));
        assert_eq!(
            value_of("counter-total"),
            serde_json::json!(format!("Total ${}", totals[index]))
        );
    }
}

#[test]
fn patches_validate_under_the_protocol() {
    let values = parse_sequence(&eval_sequence());
    let limits = studio_protocol::ProtocolLimits::default();
    for message in &values {
        studio_protocol::decode_guest_message(
            &serde_json::to_vec(message).expect("message serializes"),
            limits,
        )
        .expect("every message validates");
    }
}
