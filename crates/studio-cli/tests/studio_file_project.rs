//! File routes compose through the same project compiler used by dev and build.
use std::fs;
use std::path::Path;
use studio_cli::studio_project::{compile_routes, has_file_routes};

fn write(root: &Path, path: &str, source: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
}

#[test]
fn discovers_pages_and_composes_layouts_and_imported_components() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "routes/+layout.studio",
        "<Column id=\"shell\"><Text id=\"brand\">Stream</Text><Slot /></Column>",
    );
    write(root, "components/Heading.studio", "<script lang=\"ts\">let { title = \"Ready\" } = $props();</script><Text id=\"heading\">{title}</Text>");
    write(root, "routes/+page.studio", "<script lang=\"ts\">import Heading from '../components/Heading.studio';</script><Heading id=\"hero\" title=\"Starting\" />");
    write(
        root,
        "routes/live/+page.studio",
        "<Text id=\"live\">Live</Text>",
    );
    assert!(has_file_routes(root));
    let project = compile_routes(root).unwrap().unwrap();
    assert_eq!(
        project
            .routing
            .routes
            .iter()
            .map(|route| route.path.as_str())
            .collect::<Vec<_>>(),
        ["/", "/live"]
    );
    assert_eq!(project.module.components.len(), 2);
    let route = &project.routing.routes[0];
    assert_eq!(route.source, "routes/+page.studio");
    let component = project
        .module
        .components
        .iter()
        .find(|c| c.id == route.id)
        .unwrap();
    let (mount, _) =
        studio_script::eval::mount_tree(&project.module, component, &Default::default()).unwrap();
    let json = serde_json::to_string(&mount).unwrap();
    assert!(json.contains("Starting"));
    assert!(json.contains("Stream"));
    assert!(project.source_locations[&route.id]
        .values()
        .any(|location| location.path == "components/Heading.studio"));
}

#[test]
fn page_changes_and_added_layouts_change_composed_ir() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "routes/+page.studio",
        "<Text id=\"title\">Before</Text>",
    );
    let first = compile_routes(root).unwrap().unwrap();
    write(
        root,
        "routes/+layout.studio",
        "<Card id=\"layout\"><Slot /></Card>",
    );
    write(
        root,
        "routes/+page.studio",
        "<Text id=\"title\">After</Text>",
    );
    let second = compile_routes(root).unwrap().unwrap();
    assert_ne!(first.module, second.module);
    assert_eq!(first.routing.routes[0].id, second.routing.routes[0].id);
}

#[test]
fn legacy_typescript_projects_do_not_enter_studio_composition() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "routes/index.ts", "export const route = '/';");
    assert!(!has_file_routes(dir.path()));
    assert!(compile_routes(dir.path()).unwrap().is_none());
}

#[test]
fn broken_components_fail_before_replacing_the_compiled_project() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "routes/+page.studio", "<script lang=\"ts\">import Missing from '../components/Missing.studio';</script><Missing />");
    let error = compile_routes(dir.path())
        .err()
        .expect("missing import must fail");
    assert!(error
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("Missing.studio")));
}

#[test]
fn compiler_owned_asset_keys_cannot_be_supplied_by_authored_sources() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "routes/+page.studio",
        "<Text id=\"title\">Hello</Text>",
    );
    write(dir.path(), "assets/studio/routes.json", "{}");
    assert!(
        compile_routes(dir.path()).is_err(),
        "reserved compiler metadata must not be silently overwritten"
    );
}

#[test]
fn explicit_screen_projects_keep_precedence_over_automatic_file_routes() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "studio.project.json",
        r#"{"schemaVersion":1,"screens":[{"id":"home","source":"routes/home.studio"}]}"#,
    );
    write(
        dir.path(),
        "routes/home.studio",
        "<Text id=\"title\">Home</Text>",
    );
    assert!(
        compile_routes(dir.path()).unwrap().is_none(),
        "explicit screen graph owns discovery"
    );
}

#[test]
fn composed_routed_wasm_is_admitted_and_navigates_in_the_real_host() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "manifest.json",
        include_str!("../../../examples/starter/manifest.json"),
    );
    write(
        root,
        "routes/+page.studio",
        r#"<script lang="ts">let count = $state(0); function add() { count += 1; } function live() { emit('navigate', { operation: 'push', route: '/live' }); }</script><Column id="page"><Text id="count">{count}</Text><Button id="add" onclick={add}>Add</Button><Button id="live" onclick={live}>Go live</Button></Column>"#,
    );
    write(
        root,
        "routes/live/+page.studio",
        r#"<script lang="ts">function back() { emit('navigate', { operation: 'pop' }); }</script><Column id="live"><Button id="back" onclick={back}>Back</Button></Column>"#,
    );
    let project = compile_routes(root).unwrap().unwrap();
    let mut surface = compile_host(root, &project);
    surface
        .process_input("add", studio_components::InputAction::PointerClick)
        .unwrap();
    surface
        .process_input("live", studio_components::InputAction::PointerClick)
        .unwrap();
    surface
        .process_input("back", studio_components::InputAction::PointerClick)
        .unwrap();
    let node = surface.registry().get(surface.owner_id(), "count").unwrap();
    assert_eq!(
        node.props.get("text").and_then(serde_json::Value::as_str),
        Some("1")
    );
}

fn compile_host(
    root: &Path,
    project: &studio_cli::studio_project::RoutedProject,
) -> studio_runtime::plugin_surface::PluginSurface {
    let emitted =
        studio_script::assemblyscript::emit_routed_studio(&project.module, &project.routing)
            .unwrap();
    let entry = root.join("guest.ts");
    let wasm = root.join("guest.wasm");
    fs::write(&entry, emitted.assembly_source).unwrap();
    let asc = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sdk/assemblyscript/node_modules/assemblyscript/bin/asc.js");
    let output = std::process::Command::new("bun")
        .arg(asc)
        .arg(&entry)
        .arg("--outFile")
        .arg(&wasm)
        .args(["--maximumMemory", "256"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let host = studio_runtime::StudioHost::new(
        studio_runtime::host::HostConfig::new(studio_package::TrustStore::default()),
        studio_runtime::host::NativeDisplayAvailability::Available,
    );
    let manifest = fs::read(root.join("manifest.json")).unwrap();
    let mut assets = project.assets().unwrap();
    let mut manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    for path in manifest["assets"].as_array().unwrap() {
        let path = path.as_str().unwrap();
        assets.insert(path.to_owned(), fs::read(root.join(path)).unwrap());
    }
    manifest["assets"] = serde_json::json!(assets.keys().collect::<Vec<_>>());
    host.prepare_development(
        &serde_json::to_vec(&manifest).unwrap(),
        fs::read(wasm).unwrap(),
        assets,
    )
    .unwrap()
    .mount()
    .unwrap()
}

#[test]
fn composed_timer_wasm_automatically_fades_and_navigates_through_the_real_host() {
    use std::time::{Duration, Instant};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "manifest.json",
        include_str!("../../../examples/starter/manifest.json"),
    );
    write(
        root,
        "routes/+layout.studio",
        r#"<Column id="shell"><Text id="brand">Stream</Text><Slot /></Column>"#,
    );
    write(
        root,
        "components/Clock.studio",
        r#"<script lang="ts">let { remaining = 1 } = $props();</script><Column id="clock" opacity={remaining === 0 ? 0 : 1} transition={{ duration_ms: 100, curve: 'ease_out' }}><Text id="remaining">{remaining}</Text></Column>"#,
    );
    write(
        root,
        "routes/+page.studio",
        r#"<script lang="ts">
import Clock from '../components/Clock.studio';
let remaining = $state(1);
function tick() { remaining = remaining > 0 ? remaining - 1 : 0; }
function finish() { emit('navigate', { operation: 'replace', route: '/live' }); }
</script>
<Column id="page"><Clock id="countdown" remaining={remaining} /><Timer id="timer" interval_ms={1000} until={remaining === 0} completion_delay_ms={100} ontick={tick} oncomplete={finish} /></Column>"#,
    );
    write(
        root,
        "routes/live/+page.studio",
        r#"<Text id="live-title">WE'RE LIVE</Text>"#,
    );
    let project = compile_routes(root).unwrap().unwrap();
    let home = project
        .routing
        .routes
        .iter()
        .find(|route| route.path == "/")
        .unwrap();
    let clock = project.source_locations[&home.id]
        .iter()
        .find(|(id, location)| location.path == "components/Clock.studio" && id.ends_with("clock"))
        .unwrap()
        .0
        .clone();
    let status = project
        .timers
        .pages
        .iter()
        .find(|page| page.id == home.id)
        .unwrap()
        .timers[0]
        .status_node
        .clone();
    let live = project
        .routing
        .routes
        .iter()
        .find(|route| route.path == "/live")
        .unwrap();
    let live_title = project.source_locations[&live.id]
        .keys()
        .find(|id| id.ends_with("live-title"))
        .unwrap()
        .clone();
    let mut surface = compile_host(root, &project);
    assert_eq!(surface.mount_route().as_deref(), Some("/"));
    assert!(
        surface.keyboard_focus_order().is_empty(),
        "internal Timer callbacks must stay out of keyboard navigation"
    );
    assert!(!surface.poll_action_results().unwrap());
    assert_eq!(
        surface.metrics().guest_event_calls,
        0,
        "a timer must not call the guest before its first deadline"
    );
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut saw_fade = false;
    while surface.mount_route().as_deref() != Some("/live") {
        assert!(
            Instant::now() < deadline,
            "automatic timer never reached the live route"
        );
        surface.poll_action_results().unwrap();
        if surface.mount_route().as_deref() == Some("/") {
            let clock = surface.registry().get(surface.owner_id(), &clock).unwrap();
            if clock
                .props
                .get("opacity")
                .and_then(serde_json::Value::as_u64)
                == Some(0)
            {
                saw_fade = true;
                let status = surface.registry().get(surface.owner_id(), &status).unwrap();
                assert_eq!(
                    status.props.get("text").and_then(serde_json::Value::as_str),
                    Some("complete")
                );
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        saw_fade,
        "the clock must reach opacity zero before delayed navigation"
    );
    let title = surface
        .registry()
        .get(surface.owner_id(), &live_title)
        .unwrap();
    assert_eq!(
        title.props.get("text").and_then(serde_json::Value::as_str),
        Some("WE'RE LIVE")
    );
    assert_eq!(
        surface.metrics().guest_event_calls,
        3,
        "one tick, one completion callback and one host navigation result"
    );
}

#[cfg(unix)]
#[test]
fn studio_route_mode_follows_confined_directory_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "page-sources/+page.studio",
        "<Text id=\"linked-page\">Linked</Text>",
    );
    fs::create_dir(root.join("routes")).unwrap();
    std::os::unix::fs::symlink(root.join("page-sources"), root.join("routes/live")).unwrap();
    assert!(
        has_file_routes(root),
        "a confined linked page must not select legacy compilation"
    );
    let project = compile_routes(root).unwrap().unwrap();
    assert_eq!(project.routing.routes.len(), 1);
    assert_eq!(project.routing.routes[0].path, "/live");
    assert_eq!(project.routing.routes[0].source, "routes/live/+page.studio");
}

#[cfg(unix)]
#[test]
fn studio_route_mode_rejects_escaping_directory_links_without_legacy_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(
        outside.path(),
        "+page.studio",
        "<Text id=\"outside\">External</Text>",
    );
    fs::create_dir(dir.path().join("routes")).unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("routes/external")).unwrap();
    assert!(
        has_file_routes(dir.path()),
        "unsafe route declarations must reach the scanner's rejection"
    );
    let error = compile_routes(dir.path())
        .err()
        .expect("external route directory must fail");
    assert!(error
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("escapes project root")));
}

#[cfg(unix)]
#[test]
fn studio_route_mode_keeps_linked_typescript_pages_on_the_legacy_path() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "typescript-pages/index.ts",
        "export function mount(): void {};",
    );
    fs::create_dir(dir.path().join("routes")).unwrap();
    std::os::unix::fs::symlink(
        dir.path().join("typescript-pages"),
        dir.path().join("routes/legacy"),
    )
    .unwrap();
    assert!(!has_file_routes(dir.path()));
    assert!(compile_routes(dir.path()).unwrap().is_none());
}

#[test]
fn actual_stream_example_compiles_and_mounts_with_its_declared_assets() {
    let dir = tempfile::tempdir().unwrap();
    let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/stream-countdown");
    for entry in walkdir::WalkDir::new(&example)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_file()
            && !entry
                .path()
                .components()
                .any(|part| part.as_os_str() == "build")
        {
            let target = dir
                .path()
                .join(entry.path().strip_prefix(&example).unwrap());
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::copy(entry.path(), target).unwrap();
        }
    }
    let project = compile_routes(dir.path()).unwrap().unwrap();
    let mut surface = compile_host(dir.path(), &project);
    let clock = surface
        .registry()
        .get(surface.owner_id(), "starting-clock--clock")
        .unwrap();
    assert_eq!(clock.props["text"], "10:00");
    surface
        .process_input(
            "countdown-go-live",
            studio_components::InputAction::PointerClick,
        )
        .unwrap();
    assert!(surface
        .registry()
        .get(surface.owner_id(), "live-header--live-title-art")
        .is_ok());
    surface
        .process_input("live-back", studio_components::InputAction::PointerClick)
        .unwrap();
    assert_eq!(
        surface
            .registry()
            .get(surface.owner_id(), "starting-clock--clock")
            .unwrap()
            .props["text"],
        "10:00"
    );
}
