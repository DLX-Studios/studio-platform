//! `AssemblyScript` backend tests: determinism, protocol-conformant mount
//! payloads, and parity between the compiled module and the reviewed
//! hand-written counterpart fixture.

use std::fs;
use std::path::Path;

use studio_protocol::{GuestMessage, MountTree};
use studio_script::ir::IrTriggerEvent;
use studio_script::{assemblyscript, compile};

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("lowering")
        .join(name);
    fs::read_to_string(path).expect("fixture should be readable")
}

fn handwritten_counterpart() -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("lowering")
            .join("nav-app.handwritten.ts"),
    )
    .expect("hand-written counterpart should be readable")
}

fn event_envelope(node_id: &str, event: IrTriggerEvent) -> String {
    format!(
        "{{\"type\":\"ui\",\"payload\":{{\"node_id\":\"{node_id}\",\"event\":\"{}\",\"payload\":{{}}}}}}",
        event.as_str()
    )
}

#[test]
fn emitted_module_is_byte_identical_across_runs_and_equivalent_sources() {
    let module = compile(&fixture("nav-app.studio")).expect("fixture should lower");
    let first = assemblyscript::emit(&module);
    let second = assemblyscript::emit(&module);
    assert_eq!(first, second);

    // The same semantics written with different whitespace, attribute order,
    // and comment placement must produce byte-identical output.
    let reformatted = "studio 1\n\n<!-- two-screen sample -->\n<Screen id=\"home\" title=\"Home\">\n<Button label=\"Open detail\" id=\"open-detail\" />\n<TextInput placeholder=\"Search\" id=\"search\" />\n<Text id=\"home-title\">Home</Text>\n</Screen>\n<Screen id=\"detail\">\n<AppBar title=\"Detail\" id=\"detail-bar\" />\n<IconButton label=\"Back\" id=\"back-button\" />\n</Screen>\n<script lang=\"studio\">\non pressed open-detail push(/detail)\non changed search replace(/detail)\non pressed back-button pop()\n</script>\n";
    let other_module = compile(reformatted).expect("reformatted source should lower");
    let other = assemblyscript::emit(&other_module);
    assert_eq!(first.assembly_source, other.assembly_source);
    assert_eq!(first.mount_payload, other.mount_payload);
}

#[test]
fn mount_payload_is_protocol_conformant() {
    let module = compile(&fixture("nav-app.studio")).expect("fixture should lower");
    let emitted = assemblyscript::emit(&module);

    let Ok(GuestMessage::Mount(tree)) =
        serde_json::from_str::<GuestMessage>(&emitted.mount_payload)
    else {
        panic!("mount payload must deserialize into a guest mount message");
    };
    let MountTree {
        protocol_version,
        route,
        root,
    } = tree;
    assert_eq!(protocol_version, 1);
    assert_eq!(route, "/home");
    assert_eq!(root.id, "home");
}

#[test]
fn compiled_observable_actions_match_the_hand_written_counterpart() {
    let module = compile(&fixture("nav-app.studio")).expect("fixture should lower");
    let emitted = assemblyscript::emit(&module);
    let handwritten = handwritten_counterpart();

    // The compiled mount payload is byte-identical to the hand-written
    // counterpart's startup emission.
    assert!(
        handwritten.contains(&emitted.mount_payload),
        "compiled mount payload must match the hand-written literal"
    );

    let cases = [
        (
            "open-detail",
            IrTriggerEvent::Pressed,
            "{\"type\":\"navigate\",\"payload\":{\"operation\":\"push\",\"route\":\"/detail\"}}",
        ),
        (
            "search",
            IrTriggerEvent::Changed,
            "{\"type\":\"navigate\",\"payload\":{\"operation\":\"replace\",\"route\":\"/detail\"}}",
        ),
        (
            "back-button",
            IrTriggerEvent::Pressed,
            "{\"type\":\"navigate\",\"payload\":{\"operation\":\"pop\"}}",
        ),
    ];
    for (node_id, event, expected) in cases {
        let envelope = event_envelope(node_id, event);
        let message = assemblyscript::simulate_event(&module, &envelope)
            .expect("event envelope should deserialize")
            .unwrap_or_else(|| panic!("dispatch should match trigger {node_id}"));
        assert_eq!(message, expected);

        // Every generated response exists verbatim in the hand-written module:
        // one observable behavior, two authoring paths.
        assert!(
            handwritten.contains(expected),
            "hand-written counterpart should declare {expected}"
        );
    }
}

#[test]
fn unmatched_or_invalid_events_are_reported_not_silently_dropped() {
    let module = compile(&fixture("nav-app.studio")).expect("fixture should lower");

    let unmatched = assemblyscript::simulate_event(
        &module,
        &event_envelope("open-detail", IrTriggerEvent::Submitted),
    )
    .expect("valid envelope should deserialize");
    assert_eq!(unmatched, None);

    let invalid = assemblyscript::simulate_event(&module, "not json");
    assert!(invalid.is_err());
}
