//! Project navigation must survive the real generated-Wasm path.
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use studio_navigation::project::{ProjectRoute, RoutingManifest};
use studio_script::assemblyscript::emit_routed_studio;
use studio_script::eval::EmittedEvent;
use studio_script::types::StudioValue;

fn project() -> (studio_script::ir::StudioModule, RoutingManifest) {
    let mut home = studio_script::compile_studio(
        r#"<script lang="ts">
let count = $state(0);
let target = $state("/");
function add() { count += 1; }
function live() { target = "/live"; emit("navigate", { operation: "push", route: target }); }
</script>
<Column id="home"><Text id="count">{count}</Text><Button id="add" onclick={add}>Add</Button><Button id="live" onclick={live}>Go live</Button></Column>"#,
        "routes/+page.studio", "home",
    ).unwrap();
    let live = studio_script::compile_studio(
        r#"<script lang="ts">function back() { emit("navigate", { operation: "pop" }); }</script><Column id="live-page"><Text id="live-title">Live</Text><Button id="back" onclick={back}>Back</Button></Column>"#,
        "routes/live/+page.studio", "live",
    ).unwrap();
    home.components.extend(live.components);
    let routing = RoutingManifest {
        schema_version: 1,
        routes: vec![
            ProjectRoute {
                id: "home".into(),
                path: "/".into(),
                source: "routes/+page.studio".into(),
            },
            ProjectRoute {
                id: "live".into(),
                path: "/live".into(),
                source: "routes/live/+page.studio".into(),
            },
        ],
    };
    (home, routing)
}

#[test]
fn navigation_emission_uses_the_existing_closed_protocol_envelope() {
    let event = EmittedEvent {
        name: "navigate".into(),
        payload: [
            ("operation".into(), StudioValue::String("push".into())),
            ("route".into(), StudioValue::String("/live".into())),
        ]
        .into(),
    };
    let emitted: studio_protocol::GuestMessage = serde_json::from_str(&event.to_json()).unwrap();
    assert!(matches!(
        emitted,
        studio_protocol::GuestMessage::Navigate(_)
    ));
}

#[test]
fn emitted_project_navigates_and_remounts_retained_page_state() {
    let (module, routing) = project();
    let emitted = emit_routed_studio(&module, &routing).unwrap();
    assert_eq!(emitted, emit_routed_studio(&module, &routing).unwrap());
    let events = [
        serde_json::json!({"type":"ui","payload":{"node_id":"add","event":"pressed","payload":{}}}),
        serde_json::json!({"type":"ui","payload":{"node_id":"live","event":"pressed","payload":{}}}),
        serde_json::json!({"type":"navigation","payload":{"route":"/live","accepted":true,"error_code":null}}),
        serde_json::json!({"type":"navigation","payload":{"route":"/live","accepted":false,"error_code":"navigation_denied"}}),
        serde_json::json!({"type":"ui","payload":{"node_id":"back","event":"pressed","payload":{}}}),
        serde_json::json!({"type":"navigation","payload":{"route":"/","accepted":true,"error_code":null}}),
    ];
    let messages = compile_and_drive(&emitted.assembly_source, &events);
    assert_eq!(messages.len(), 6, "{messages:?}");
    assert_eq!(messages[0]["payload"]["route"], "/");
    assert_eq!(messages[2]["type"], "navigate");
    assert_eq!(messages[3]["payload"]["route"], "/live");
    assert_eq!(messages[4]["payload"]["operation"], "pop");
    assert_eq!(messages[5]["payload"]["route"], "/");
    assert_eq!(
        messages[5]["payload"]["root"]["children"][0]["children"][0]["props"]["text"],
        "1"
    );
}

#[test]
fn invalid_navigation_operations_are_rejected_by_both_emission_paths() {
    let module = studio_script::compile_studio(
        r#"<script lang="ts">function bad() { emit("navigate", { operation: "erase", route: "/" }); }</script><Button id="bad" onclick={bad}>Bad</Button>"#,
        "bad.studio", "bad",
    ).unwrap();
    assert!(studio_script::assemblyscript::emit_studio(&module).is_err());
    let routing = RoutingManifest {
        schema_version: 1,
        routes: vec![ProjectRoute {
            id: "bad".into(),
            path: "/".into(),
            source: "routes/+page.studio".into(),
        }],
    };
    assert!(emit_routed_studio(&module, &routing).is_err());
}

fn compile_and_drive(
    assembly_source: &str,
    events: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    let scratch = tempfile::tempdir().unwrap();
    let assembly = scratch.path().join("project.ts");
    let wasm = scratch.path().join("project.wasm");
    std::fs::write(&assembly, assembly_source).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("bun")
        .arg(root.join("sdk/assemblyscript/node_modules/assemblyscript/bin/asc.js"))
        .arg(&assembly)
        .args(["--outFile"])
        .arg(&wasm)
        .args(["--runtime", "stub"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut driver = Command::new("bun")
        .arg(root.join("crates/studio-script/tests/fixtures/drive-wasm.ts"))
        .arg(&wasm)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    for event in events {
        writeln!(driver.stdin.as_mut().unwrap(), "{event}").unwrap();
    }
    let output = driver.wait_with_output().unwrap();
    assert!(output.status.success());
    let messages: Vec<serde_json::Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for message in &messages {
        let decoded: studio_protocol::GuestMessage =
            serde_json::from_value(message.clone()).unwrap();
        studio_protocol::validate_guest_message(
            &decoded,
            studio_protocol::ProtocolLimits::default(),
        )
        .unwrap();
    }
    messages
}

#[test]
fn compiled_page_selection_agrees_with_the_host_for_params_and_catch_all_precedence() {
    let declarations = [
        ("home", "/", r#"<Text id="home-title">Home</Text>"#),
        (
            "streams_exact",
            "/streams",
            r#"<Text id="streams-title">Streams</Text>"#,
        ),
        (
            "streams_rest",
            "/streams/*",
            r#"<Text id="rest-title">Stream catch-all</Text>"#,
        ),
        (
            "section_new",
            "/:section/new",
            r#"<script lang="ts">let { section } = $props();</script><Text id="section-title">{section}</Text>"#,
        ),
        (
            "game",
            "/games/:id",
            r#"<script lang="ts">let { id } = $props();</script><Text id="game-title">{id}</Text>"#,
        ),
    ];
    let mut module =
        studio_script::compile_studio(declarations[0].2, "home.studio", "home").unwrap();
    for (id, _, source) in declarations.iter().skip(1) {
        module.components.extend(
            studio_script::compile_studio(source, &format!("{id}.studio"), id)
                .unwrap()
                .components,
        );
    }
    let routing = RoutingManifest {
        schema_version: 1,
        routes: declarations
            .iter()
            .map(|(id, path, _)| ProjectRoute {
                id: (*id).into(),
                path: (*path).into(),
                source: format!("routes/{id}.studio"),
            })
            .collect(),
    };
    let emitted = emit_routed_studio(&module, &routing).unwrap();
    let mut host = studio_navigation::project::ProjectRouter::new(&routing, "/").unwrap();
    let destinations = [
        "/streams",
        "/streams/new",
        "/news/new",
        "/games/42",
        "/games/7",
    ];
    let selections: Vec<_> = destinations
        .iter()
        .map(|path| {
            host.navigate(&studio_protocol::NavigationCommand::Push {
                route: (*path).into(),
            })
            .unwrap()
        })
        .collect();
    let events:Vec<_>=selections.iter().map(|selection|serde_json::json!({"type":"navigation","payload":{"route":selection.route,"accepted":true,"error_code":null}})).collect();
    let messages = compile_and_drive(&emitted.assembly_source, &events);
    for (message, selection) in messages.iter().skip(1).zip(&selections) {
        assert_eq!(message["payload"]["route"], selection.route);
        assert_eq!(
            message["payload"]["root"]["id"],
            format!("{}#root", selection.id)
        );
    }
    assert_eq!(selections[1].id, "streams_rest");
    assert_eq!(
        messages[3]["payload"]["root"]["children"][0]["props"]["text"],
        "news"
    );
    assert_eq!(
        messages[4]["payload"]["root"]["children"][0]["props"]["text"],
        "42"
    );
    assert_eq!(
        messages[5]["payload"]["root"]["children"][0]["props"]["text"],
        "7"
    );
}

#[test]
fn evaluator_parameter_remount_retains_page_state_and_refreshes_props() {
    use std::collections::BTreeMap;
    use studio_script::eval::InteractionSession;
    let module=studio_script::compile_studio(
        r#"<script lang="ts">let { id } = $props(); let count = $state(0); function add() { count += 1; }</script><Column id="page"><Text id="id">{id}</Text><Text id="count">{count}</Text><Button id="add" onclick={add}>Add</Button></Column>"#,
        "page.studio","page",
    ).unwrap();
    let component = &module.components[0];
    let first: BTreeMap<_, _> = [("id".into(), StudioValue::String("42".into()))].into();
    let (_, mut session) = InteractionSession::mount(&module, component, &first).unwrap();
    session.apply(component, "add", "pressed").unwrap();
    let second: BTreeMap<_, _> = [("id".into(), StudioValue::String("7".into()))].into();
    let (mount, _) = session
        .remount_with_props(&module, component, &second)
        .unwrap();
    assert_eq!(mount.root.children[0].children[0].props["text"], "7");
    assert_eq!(mount.root.children[0].children[1].props["text"], "1");
    assert!(
        session
            .remount_with_props(&module, component, &BTreeMap::new())
            .is_err()
    );
}

#[test]
fn rejected_navigation_rolls_back_page_state_before_the_next_input() {
    let module=studio_script::compile_studio(
        r#"<script lang="ts">let count = $state(0); function reject() { count += 10; emit("navigate", { operation: "push", route: "/missing" }); } function add() { count += 1; }</script><Column id="home"><Text id="count">{count}</Text><Button id="reject" onclick={reject}>Reject</Button><Button id="add" onclick={add}>Add</Button></Column>"#,
        "home.studio","home",
    ).unwrap();
    let routing = RoutingManifest {
        schema_version: 1,
        routes: vec![ProjectRoute {
            id: "home".into(),
            path: "/".into(),
            source: "routes/+page.studio".into(),
        }],
    };
    let emitted = emit_routed_studio(&module, &routing).unwrap();
    let messages = compile_and_drive(
        &emitted.assembly_source,
        &[
            serde_json::json!({"type":"ui","payload":{"node_id":"reject","event":"pressed","payload":{}}}),
            serde_json::json!({"type":"navigation","payload":{"route":"/","accepted":false,"error_code":"navigation_denied"}}),
            serde_json::json!({"type":"ui","payload":{"node_id":"add","event":"pressed","payload":{}}}),
        ],
    );
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[1]["type"], "navigate");
    assert_eq!(
        messages[2]["payload"]["operations"][0]["node"]["children"][0]["children"][0]["props"]["text"],
        "1"
    );
}

#[test]
fn routed_dynamic_strings_preserve_unicode_and_escape_json_control_characters() {
    let module=studio_script::compile_studio(
        r#"<script lang="ts">let title = $state("Ready"); function change() { title = "café\n\u263a\b"; }</script><Column id="home"><Text id="title">{title}</Text><Button id="change" onclick={change}>Change</Button></Column>"#,
        "home.studio","home",
    ).unwrap();
    let routing = RoutingManifest {
        schema_version: 1,
        routes: vec![ProjectRoute {
            id: "home".into(),
            path: "/".into(),
            source: "routes/+page.studio".into(),
        }],
    };
    let emitted = emit_routed_studio(&module, &routing).unwrap();
    let messages = compile_and_drive(
        &emitted.assembly_source,
        &[
            serde_json::json!({"type":"ui","payload":{"node_id":"change","event":"pressed","payload":{}}}),
        ],
    );
    assert_eq!(
        messages[1]["payload"]["operations"][0]["node"]["children"][0]["children"][0]["props"]["text"],
        "café\n☺\u{8}"
    );
}

#[test]
fn routed_countdown_exit_opacity_is_reprojected_from_conditional_state() {
    let module=studio_script::compile_studio(
        r#"<script lang="ts">let remaining = $state(1); function tick() { remaining = remaining > 0 ? remaining - 1 : 0; }</script><Column id="clock" opacity={remaining === 0 ? 0 : 1}><Text id="seconds">{remaining}</Text><Button id="tick" onclick={tick}>Tick</Button></Column>"#,
        "home.studio","home",
    ).unwrap();
    let routing = RoutingManifest {
        schema_version: 1,
        routes: vec![ProjectRoute {
            id: "home".into(),
            path: "/".into(),
            source: "routes/+page.studio".into(),
        }],
    };
    let emitted = emit_routed_studio(&module, &routing).unwrap();
    let messages = compile_and_drive(
        &emitted.assembly_source,
        &[
            serde_json::json!({"type":"ui","payload":{"node_id":"tick","event":"pressed","payload":{}}}),
        ],
    );
    assert_eq!(
        messages[0]["payload"]["root"]["children"][0]["props"]["opacity"],
        1
    );
    assert_eq!(
        messages[1]["payload"]["operations"][0]["node"]["children"][0]["props"]["opacity"],
        0
    );
}

#[test]
fn conditional_attribute_source_reaches_typed_ir() {
    let source = r#"<script lang="ts">let remaining = $state(1);</script><Column id="clock" opacity={remaining === 0 ? 0 : 1}><Text id="time">{remaining}</Text></Column>"#;
    studio_script::compile_studio(source, "conditional.studio", "conditional").unwrap();
}

#[test]
fn transition_object_attributes_preserve_fields_and_pass_wire_validation() {
    let source = r#"<Column id="clock" transition={{ duration_ms: 700, curve: 'ease_out' }}><Text id="time">10:00</Text></Column>"#;
    let module = studio_script::compile_studio(source, "transition.studio", "transition").unwrap();
    let (mount, _) = studio_script::eval::mount_tree(
        &module,
        &module.components[0],
        &std::collections::BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(
        mount.root.children[0].props["transition"],
        serde_json::json!({"duration_ms":700,"curve":"ease_out"})
    );
    studio_protocol::validate_guest_message(
        &studio_protocol::GuestMessage::Mount(mount),
        studio_protocol::ProtocolLimits::default(),
    )
    .unwrap();
}

#[test]
fn routed_string_props_can_select_header_artwork_branches() {
    let module=studio_script::compile_studio(
        r#"<script lang="ts">let { title = "STREAM STARTING", showStarting = true } = $props();</script>{#if title === "STREAM STARTING" && showStarting === true}<Text id="starting">Starting</Text>{:else}<Text id="live">Live</Text>{/if}"#,
        "header.studio","header",
    ).unwrap();
    let routing = RoutingManifest {
        schema_version: 1,
        routes: vec![ProjectRoute {
            id: "header".into(),
            path: "/".into(),
            source: "routes/+page.studio".into(),
        }],
    };
    let emitted = emit_routed_studio(&module, &routing).unwrap();
    let messages = compile_and_drive(&emitted.assembly_source, &[]);
    let expected: serde_json::Value = serde_json::from_str(&emitted.mount_payload).unwrap();
    assert_eq!(
        messages[0], expected,
        "routed initial projection must preserve conditional leaf content"
    );
    assert_eq!(
        messages[0]["payload"]["root"]["children"][0]["children"][0]["props"]["text"],
        "Starting"
    );
}
