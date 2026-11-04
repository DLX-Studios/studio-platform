//! Project composition contracts shared by development IR and Wasm lowering.
use std::collections::BTreeMap;
use std::path::Path;

use studio_script::eval::{InteractionSession, SessionMessage, mount_tree};
use studio_script::project::compile_component;

fn write(root: &Path, path: &str, source: &str) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, source).unwrap();
}

fn text<'a>(node: &'a studio_protocol::UiNode, id: &str) -> Option<&'a str> {
    if node.id == id {
        return node.props.get("text").and_then(serde_json::Value::as_str);
    }
    node.children.iter().find_map(|child| text(child, id))
}

#[test]
fn imported_component_instances_have_independent_state_and_source_locations() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "components/Counter.studio",
        r#"<script lang="ts">
let { label = "Count" } = $props();
let count = $state(0);
function increment() { count += 1; }
</script>
<Column id="counter"><Text id="label">{label}</Text><Text id="value">{count}</Text><Button id="add" onclick={increment}>Add</Button></Column>"#,
    );
    write(
        dir.path(),
        "routes/+page.studio",
        r#"<script lang="ts">
import Counter from '../components/Counter.studio';
</script><Column id="page"><Counter id="first" label="One" /><Counter id="second" label="Two" /></Column>"#,
    );
    let project =
        compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home").unwrap();
    let component = &project.module.components[0];
    let (mount, mut session) =
        InteractionSession::mount(&project.module, component, &BTreeMap::new()).unwrap();
    assert_eq!(text(&mount.root, "first--label"), Some("One"));
    assert_eq!(text(&mount.root, "second--value"), Some("0"));
    assert_eq!(
        project.source_locations["first--value"].path,
        "components/Counter.studio"
    );
    let SessionMessage::Patch(patch) = session.apply(component, "first--add", "pressed").unwrap()
    else {
        panic!("state handler must patch")
    };
    let json = serde_json::to_string(&patch).unwrap();
    assert!(json.contains("first--value"));
    assert!(json.contains("\"value\":\"1\""));
    let SessionMessage::Patch(patch) = session.apply(component, "second--add", "pressed").unwrap()
    else {
        panic!("state handler must patch")
    };
    let values: Vec<_> = patch
        .operations
        .iter()
        .filter_map(|operation| {
            let json = serde_json::to_value(operation).unwrap();
            (json["property"] == "text"
                && json["node_id"]
                    .as_str()
                    .is_some_and(|id| id.ends_with("--value")))
            .then(|| json["value"].clone())
        })
        .collect();
    assert_eq!(values, vec![serde_json::json!("1"), serde_json::json!("1")]);
}

#[test]
fn imported_props_follow_parent_reactive_state() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "components/Value.studio",
        r#"<script lang="ts">let { value = 0 } = $props();</script><Text id="value">{value}</Text>"#,
    );
    write(
        dir.path(),
        "routes/+page.studio",
        r#"<script lang="ts">
import Value from '../components/Value.studio';
let count = $state(2);
function increment() { count += 1; }
</script><Column id="page"><Value id="child" value={count} /><Button id="add" onclick={increment}>Add</Button></Column>"#,
    );
    let project =
        compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home").unwrap();
    let component = &project.module.components[0];
    let (_, mut session) =
        InteractionSession::mount(&project.module, component, &BTreeMap::new()).unwrap();
    let message = session
        .apply(component, "add", "pressed")
        .unwrap()
        .to_json();
    assert!(message.contains("child--value"));
    assert!(message.contains("\"value\":\"3\""));
}

#[test]
fn ancestor_layouts_wrap_page_in_order_and_dependencies_are_recorded() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "routes/+layout.studio",
        r#"<Column id="outer"><Text id="brand" text="Vice City" /><Slot /></Column>"#,
    );
    write(
        dir.path(),
        "routes/live/+layout.studio",
        r#"<Box id="inner"><Slot /></Box>"#,
    );
    write(
        dir.path(),
        "routes/live/+page.studio",
        r#"<Text id="live" text="WE ARE LIVE" />"#,
    );
    let layouts = vec![
        "routes/+layout.studio".into(),
        "routes/live/+layout.studio".into(),
    ];
    let project = compile_component(
        dir.path(),
        Path::new("routes/live/+page.studio"),
        &layouts,
        "live",
    )
    .unwrap();
    let (mount, _) = mount_tree(
        &project.module,
        &project.module.components[0],
        &BTreeMap::new(),
    )
    .unwrap();
    let json = serde_json::to_value(&mount.root).unwrap();
    assert_eq!(json["children"][0]["id"], "layout0--outer");
    assert_eq!(json["children"][0]["children"][1]["id"], "layout1--inner");
    assert_eq!(text(&mount.root, "live"), Some("WE ARE LIVE"));
    assert!(
        project
            .module
            .dependencies
            .contains(&"routes/+layout.studio".to_owned())
    );
}

#[test]
fn import_cycles_missing_sources_and_invalid_props_are_source_linked() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "components/Required.studio",
        r#"<script lang="ts">let { title } = $props();</script><Text id="text">{title}</Text>"#,
    );
    for (source, expected) in [
        (
            r#"<script lang="ts">import Missing from '../components/Missing.studio';</script><Missing />"#,
            "Missing.studio",
        ),
        (
            r#"<script lang="ts">import Required from '../components/Required.studio';</script><Required />"#,
            "required prop",
        ),
        (
            r#"<script lang="ts">import Required from '../components/Required.studio';</script><Required title="Ok" extra="bad" />"#,
            "unknown prop",
        ),
    ] {
        write(dir.path(), "routes/+page.studio", source);
        let errors = compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home")
            .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.diagnostic.message.contains(expected)),
            "{errors:?}"
        );
        assert!(errors.iter().all(|error| !error.path.is_empty()));
    }
    write(
        dir.path(),
        "components/A.studio",
        r#"<script lang="ts">import B from './B.studio';</script><B />"#,
    );
    write(
        dir.path(),
        "components/B.studio",
        r#"<script lang="ts">import A from './A.studio';</script><A />"#,
    );
    write(
        dir.path(),
        "routes/+page.studio",
        r#"<script lang="ts">import A from '../components/A.studio';</script><A />"#,
    );
    let errors =
        compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.diagnostic.message.contains("cycle"))
    );
}

#[cfg(unix)]
#[test]
fn symlink_import_escape_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(
        outside.path(),
        "External.studio",
        r#"<Text id="external" text="No" />"#,
    );
    std::fs::create_dir_all(dir.path().join("components")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("External.studio"),
        dir.path().join("components/External.studio"),
    )
    .unwrap();
    write(
        dir.path(),
        "routes/+page.studio",
        r#"<script lang="ts">import External from '../components/External.studio';</script><External />"#,
    );
    let errors =
        compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.diagnostic.message.contains("outside project"))
    );
}

#[test]
fn svelte_children_render_outlet_composes_without_serializable_snippet_props() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "routes/+layout.studio",
        r#"<script lang="ts">let { children } = $props();</script><Column id="shell">{@render children()}</Column>"#,
    );
    write(
        dir.path(),
        "routes/+page.studio",
        r#"<Text id="page" text="Hello" />"#,
    );
    let project = compile_component(
        dir.path(),
        Path::new("routes/+page.studio"),
        &["routes/+layout.studio".into()],
        "home",
    )
    .unwrap();
    let (mount, _) = mount_tree(
        &project.module,
        &project.module.components[0],
        &BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(text(&mount.root, "page"), Some("Hello"));
    assert!(project.module.components[0].props.is_empty());
}

#[test]
fn composition_rejects_colliding_authored_and_generated_node_ids() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "components/Value.studio",
        r#"<Text id="value" text="Child" />"#,
    );
    write(
        dir.path(),
        "routes/+page.studio",
        r#"<script lang="ts">import Value from '../components/Value.studio';</script><Column id="page"><Value id="child" /><Text id="child--value" text="Collision" /></Column>"#,
    );
    let errors =
        compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.diagnostic.message.contains("duplicate composed node"))
    );
}

#[test]
fn component_props_are_type_checked_before_mounting() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "components/Value.studio",
        r#"<script lang="ts">let { value = 0 } = $props();</script><Text id="value">{value}</Text>"#,
    );
    write(
        dir.path(),
        "routes/+page.studio",
        r#"<script lang="ts">import Value from '../components/Value.studio';</script><Value value="wrong" />"#,
    );
    let errors =
        compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.diagnostic.message.contains("incompatible type"))
    );
}

#[test]
fn composed_derived_dependencies_follow_bound_parent_state() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "components/Double.studio",
        r#"<script lang="ts">let { value = 0 } = $props();let doubled = $derived(value * 2);</script><Text id="value">{doubled}</Text>"#,
    );
    write(
        dir.path(),
        "routes/+page.studio",
        r#"<script lang="ts">import Double from '../components/Double.studio';let count = $state(2);function add() { count += 1; }</script><Column id="page"><Double id="child" value={count} /><Button id="add" onclick={add}>Add</Button></Column>"#,
    );
    let project =
        compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home").unwrap();
    let component = &project.module.components[0];
    assert_eq!(component.derived[0].dependencies, vec!["count"]);
    let (_, mut session) =
        InteractionSession::mount(&project.module, component, &BTreeMap::new()).unwrap();
    let patch = session
        .apply(component, "add", "pressed")
        .unwrap()
        .to_json();
    assert!(patch.contains("\"value\":\"6\""));
}

#[test]
fn project_handlers_accept_single_quoted_navigation_event_names() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "routes/+page.studio",
        r#"<script lang="ts">function go() { emit('navigate', { operation: 'push', route: '/live' }); }</script><Button id="go" onclick={go}>Go</Button>"#,
    );
    let project =
        compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home").unwrap();
    assert_eq!(project.module.components[0].handlers[0].emit, "navigate");
}

#[test]
fn layout_scalar_route_parameters_are_exposed_on_the_composed_page() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "routes/+layout.studio",
        r#"<script lang="ts">let { id } = $props();</script><Column id="shell"><Text id="parameter">{id}</Text><Slot /></Column>"#,
    );
    write(
        dir.path(),
        "routes/+page.studio",
        r#"<Text id="page" text="Hello" />"#,
    );
    let project = compile_component(
        dir.path(),
        Path::new("routes/+page.studio"),
        &["routes/+layout.studio".into()],
        "home",
    )
    .unwrap();
    let component = &project.module.components[0];
    assert!(
        component
            .props
            .iter()
            .any(|prop| prop.name == "id" && prop.required)
    );
    let props = [(
        "id".to_owned(),
        studio_script::types::StudioValue::String("stream-42".to_owned()),
    )]
    .into_iter()
    .collect();
    let (mount, _) = mount_tree(&project.module, component, &props).unwrap();
    assert_eq!(text(&mount.root, "layout0--parameter"), Some("stream-42"));
}

#[test]
fn source_timer_lowers_to_hidden_native_targets_and_scoped_metadata() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "routes/+page.studio",
        r#"<script lang="ts">
let remaining = $state(2);
function tick() { remaining = remaining > 0 ? remaining - 1 : 0; }
function complete() { emit('navigate', { operation: 'replace', route: '/live' }); }
</script>
<Column id="page"><Text id="value">{remaining}</Text><Timer id="stream-timer" interval_ms={1000} until={remaining === 0} completion_delay_ms={700} ontick={tick} oncomplete={complete} /></Column>"#,
    );
    let project =
        compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home").unwrap();
    assert_eq!(project.timers.len(), 1);
    let timer = &project.timers[0];
    assert_eq!(timer.id, "stream-timer");
    assert_eq!(timer.tick_node, "stream-timer--tick");
    assert_eq!(timer.complete_node, "stream-timer--complete");
    assert_eq!(timer.status_node, "stream-timer--status");
    assert_eq!(timer.interval_ms, 1000);
    assert_eq!(timer.completion_delay_ms, 700);
    let component = &project.module.components[0];
    let (mount, mut session) =
        InteractionSession::mount(&project.module, component, &BTreeMap::new()).unwrap();
    assert_eq!(text(&mount.root, &timer.status_node), Some("running"));
    session
        .apply(component, &timer.tick_node, "pressed")
        .unwrap();
    let completed = session
        .apply(component, &timer.tick_node, "pressed")
        .unwrap()
        .to_json();
    assert!(completed.contains("\"value\":\"complete\""));
    let message = session
        .apply(component, &timer.complete_node, "pressed")
        .unwrap()
        .to_json();
    assert!(message.contains("\"type\":\"navigate\""));
    assert!(
        !serde_json::to_string(&mount)
            .unwrap()
            .contains("\"kind\":\"timer\"")
    );
}

#[test]
fn imported_timer_instances_get_independent_targets_and_state() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "components/Timed.studio",
        r#"<script lang="ts">
let remaining = $state(2);
function tick() { remaining = remaining > 0 ? remaining - 1 : 0; }
function complete() { remaining = 0; }
</script><Column id="content"><Text id="value">{remaining}</Text><Timer id="timer" interval_ms={1000} until={remaining === 0} ontick={tick} oncomplete={complete} /></Column>"#,
    );
    write(
        dir.path(),
        "routes/+page.studio",
        r#"<script lang="ts">import Timed from '../components/Timed.studio';</script><Column id="page"><Timed id="first" /><Timed id="second" /></Column>"#,
    );
    let project =
        compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home").unwrap();
    assert_eq!(project.timers.len(), 2);
    assert_eq!(project.timers[0].tick_node, "first--timer--tick");
    assert_eq!(project.timers[1].tick_node, "second--timer--tick");
    let component = &project.module.components[0];
    let (_, mut session) =
        InteractionSession::mount(&project.module, component, &BTreeMap::new()).unwrap();
    let patch = session
        .apply(component, &project.timers[0].tick_node, "pressed")
        .unwrap()
        .to_json();
    assert!(patch.contains("first--value"));
    assert!(patch.contains("\"value\":\"1\""));
    assert!(patch.contains("\"value\":\"2\""));
}

#[test]
fn timer_declarations_reject_unstable_ids_invalid_bounds_and_nonboolean_status() {
    let dir = tempfile::tempdir().unwrap();
    for (declaration, expected) in [
        (
            r"<Timer interval_ms={1000} until={remaining === 0} ontick={tick} oncomplete={complete} />",
            "explicit",
        ),
        (
            r#"<Timer id="timer" interval_ms={10} until={remaining === 0} ontick={tick} oncomplete={complete} />"#,
            "interval",
        ),
        (
            r#"<Timer id="timer" interval_ms={1000.5} until={remaining === 0} ontick={tick} oncomplete={complete} />"#,
            "integer",
        ),
        (
            r#"<Timer id="timer" interval_ms={1000} until={remaining} ontick={tick} oncomplete={complete} />"#,
            "boolean",
        ),
        (
            r#"<Timer id="timer" interval_ms={1000} until={remaining === 0} completion_delay_ms={60001} ontick={tick} oncomplete={complete} />"#,
            "completion_delay_ms",
        ),
        (
            r#"<Timer id="timer" interval_ms={1000} until={remaining === 0} surprise={1} ontick={tick} oncomplete={complete} />"#,
            "unknown",
        ),
    ] {
        let source = format!(
            r#"<script lang="ts">let remaining = $state(2);function tick() {{ remaining -= 1; }}function complete() {{ remaining = 0; }}</script>{declaration}"#
        );
        write(dir.path(), "routes/+page.studio", &source);
        let errors = compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home")
            .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.diagnostic.message.contains(expected)),
            "{declaration}: {errors:?}"
        );
    }
}

#[test]
fn stateful_imported_components_in_each_are_rejected_without_per_key_ownership() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "components/Counter.studio",
        r#"<script lang="ts">let count = $state(0);function add() { count += 1; }</script><Column id="counter"><Text id="count">{count}</Text><Button id="add" onclick={add}>Add</Button></Column>"#,
    );
    write(
        dir.path(),
        "routes/+page.studio",
        r#"<script lang="ts">import Counter from '../components/Counter.studio';let rows = $state([{ id: 'a' }, { id: 'b' }]);</script><Column id="page">{#each rows as row (row.id)}<Counter id="counter-instance" />{/each}</Column>"#,
    );
    let errors =
        compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home").unwrap_err();
    assert!(
        errors.iter().any(|error| error
            .diagnostic
            .message
            .contains("stateful imported components in each")),
        "{errors:?}"
    );
}

#[test]
fn authored_and_namespaced_node_ids_obey_protocol_byte_bounds_before_mount() {
    let dir = tempfile::tempdir().unwrap();
    let oversized = "x".repeat(129);
    write(
        dir.path(),
        "routes/+page.studio",
        &format!(r#"<Text id="{oversized}" text="Too long" />"#),
    );
    let errors =
        compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.diagnostic.message.contains("node id")
                && error.diagnostic.message.contains("128")),
        "{errors:?}"
    );

    write(
        dir.path(),
        "components/Value.studio",
        r#"<Text id="value-with-a-long-name" text="Too long when scoped" />"#,
    );
    let prefix = "x".repeat(120);
    write(
        dir.path(),
        "routes/+page.studio",
        &format!(
            r#"<script lang="ts">import Value from '../components/Value.studio';</script><Value id="{prefix}" />"#
        ),
    );
    let errors =
        compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home").unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.diagnostic.message.contains("node id")
                && error.diagnostic.message.contains("128")),
        "{errors:?}"
    );
}

#[test]
fn scalar_derived_boolean_and_string_values_are_valid_imported_props() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "components/Status.studio",
        r#"<script lang="ts">let { active = false, label = '' } = $props();</script><Text id="status">{label}: {active ? 'active' : 'idle'}</Text>"#,
    );
    write(
        dir.path(),
        "routes/+page.studio",
        r#"<script lang="ts">
import Status from '../components/Status.studio';
let count = $state(1);
let active = $derived(count > 0);
let label = $derived('Stream ' + count);
function add() { count += 1; }
</script><Column id="page"><Status id="child" active={active} label={label} /><Button id="add" onclick={add}>Add</Button></Column>"#,
    );
    let project =
        compile_component(dir.path(), Path::new("routes/+page.studio"), &[], "home").unwrap();
    let component = &project.module.components[0];
    assert_eq!(
        component.derived[0].ty,
        studio_script::types::StudioType::Boolean
    );
    assert_eq!(
        component.derived[1].ty,
        studio_script::types::StudioType::String
    );
    let (mount, mut session) =
        InteractionSession::mount(&project.module, component, &BTreeMap::new()).unwrap();
    assert_eq!(text(&mount.root, "child--status"), Some("Stream 1: active"));
    let message = session
        .apply(component, "add", "pressed")
        .unwrap()
        .to_json();
    assert!(message.contains("Stream 2: active"));
    studio_script::assemblyscript::emit_studio(&project.module).unwrap();
}
