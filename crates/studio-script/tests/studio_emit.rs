//! US3 backend contract: generated `AssemblyScript` is deterministic,
//! handler-complete, and embeds a valid mount payload.

use studio_script::assemblyscript::emit_studio;
use studio_script::ir::StudioModule;

fn compile_fixture(name: &str) -> StudioModule {
    let path = format!(
        "{}/tests/fixtures/ir-v2/{name}/source.studio",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(&path).expect("fixture readable");
    studio_script::compile_studio(&source, &format!("{name}.studio"), name).expect("compiles")
}

#[test]
fn emission_is_deterministic_and_handler_complete() {
    let module = compile_fixture("typed-events");
    let first = emit_studio(&module).expect("emits");
    let second = emit_studio(&module).expect("emits");
    assert_eq!(
        first.assembly_source, second.assembly_source,
        "identical IR produces identical AssemblyScript"
    );

    let component = &module.components[0];
    for handler in &component.handlers {
        let node = handler.node.as_deref().expect("handler bound to a node");
        assert!(
            first.assembly_source.contains(node),
            "branch matches node {node}"
        );
        assert!(
            first.assembly_source.contains(&handler.emit),
            "branch returns the emission"
        );
    }

    let mount: serde_json::Value =
        serde_json::from_str(&first.mount_payload).expect("mount payload is JSON");
    assert_eq!(mount["type"], "mount");
    assert!(mount["payload"]["root"].is_object());
}

#[test]
fn handlerless_modules_emit_a_quiet_dispatcher() {
    let module = compile_fixture("conditional");
    let emitted = emit_studio(&module).expect("emits");
    assert!(emitted.assembly_source.contains("return 0;"));
    let mount: serde_json::Value =
        serde_json::from_str(&emitted.mount_payload).expect("mount payload is JSON");
    assert_eq!(mount["type"], "mount");
}

#[test]
fn multi_component_modules_need_the_graph() {
    let mut module = compile_fixture("typed-events");
    module.components.push(module.components[0].clone());
    let error = emit_studio(&module).expect_err("multi-component must fail");
    assert_eq!(error.code, "STUDIO340");
}
