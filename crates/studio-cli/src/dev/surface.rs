//! The development host's live state. Production surfaces contain no IR interpreter.
use std::collections::BTreeMap;
use std::sync::Arc;

use studio_components::native_view::{NativeRenderNode, NativeSurface};
use studio_components::{HostEventDispatcher, InputAction, NativeStateStore};
use studio_package::{parse_manifest, ManifestPolicy};
use studio_protocol::dev_rpc::DevSourceLocation;
use studio_protocol::{HostEvent, MountTree, ProtocolLimits, UiNode};
use studio_runtime::host::PreparedPlugin;
use studio_runtime::plugin_surface::PluginSurface;
use studio_script::eval::{InteractionSession, SessionMessage};
use studio_script::ir::{StudioModule, TemplateNode};
use studio_ui::{InstanceId, UiRegistry};

pub(crate) struct IrSurface {
    module: Arc<StudioModule>,
    screens: BTreeMap<String, IrScreenState>,
    screen_ids: Vec<String>,
    active_screen_id: String,
    screen_source_paths: BTreeMap<String, String>,
    owner: InstanceId,
    assets: BTreeMap<String, Vec<u8>>,
}

struct IrScreenState {
    session: InteractionSession,
    registry: UiRegistry,
}

pub(crate) enum Replacement {
    Ir {
        module: Arc<StudioModule>,
        manifest: Vec<u8>,
        assets: BTreeMap<String, Vec<u8>>,
        screen_source_paths: BTreeMap<String, String>,
    },
    Wasm(Box<PreparedPlugin>),
    Assets(BTreeMap<String, Vec<u8>>),
}

/// Source and identity metadata for one prepared preview replacement.
pub(crate) struct SurfaceMetadata {
    pub component_id: Option<String>,
    pub screen_id: Option<String>,
    pub screen_source_path: Option<String>,
    pub available_screen_ids: Vec<String>,
    pub source_locations: BTreeMap<String, DevSourceLocation>,
}

fn collect_source_locations(
    nodes: &[TemplateNode],
    path: &str,
    locations: &mut BTreeMap<String, DevSourceLocation>,
) {
    for node in nodes {
        match node {
            TemplateNode::Component {
                id, children, span, ..
            } => {
                locations.insert(
                    id.clone(),
                    DevSourceLocation {
                        path: path.to_owned(),
                        line: span.start.line as u64,
                        column: span.start.column as u64,
                        offset: span.start.offset as u64,
                    },
                );
                collect_source_locations(children, path, locations);
            }
            TemplateNode::Text { id, span, .. } | TemplateNode::Interpolation { id, span, .. } => {
                locations.insert(
                    id.clone(),
                    DevSourceLocation {
                        path: path.to_owned(),
                        line: span.start.line as u64,
                        column: span.start.column as u64,
                        offset: span.start.offset as u64,
                    },
                );
            }
            TemplateNode::If {
                id,
                consequent,
                alternate,
                span,
                ..
            } => {
                locations.insert(
                    id.clone(),
                    DevSourceLocation {
                        path: path.to_owned(),
                        line: span.start.line as u64,
                        column: span.start.column as u64,
                        offset: span.start.offset as u64,
                    },
                );
                collect_source_locations(consequent, path, locations);
                collect_source_locations(alternate, path, locations);
            }
            TemplateNode::Each {
                id,
                body,
                fallback,
                span,
                ..
            } => {
                locations.insert(
                    id.clone(),
                    DevSourceLocation {
                        path: path.to_owned(),
                        line: span.start.line as u64,
                        column: span.start.column as u64,
                        offset: span.start.offset as u64,
                    },
                );
                collect_source_locations(body, path, locations);
                collect_source_locations(fallback, path, locations);
            }
        }
    }
}

pub(crate) enum DevSurface {
    Empty,
    Ir(Box<IrSurface>),
    Wasm(Box<PluginSurface>),
}

impl DevSurface {
    /// Construct the complete candidate before committing; the renderer's entity survives.
    pub fn replace(&mut self, update: Replacement, reset: bool) -> Result<(), String> {
        match update {
            Replacement::Assets(assets) => match self {
                Self::Ir(surface) => surface.assets = assets,
                Self::Wasm(surface) => surface.replace_development_assets(assets),
                Self::Empty => return Err("no mounted application".into()),
            },
            Replacement::Ir {
                module,
                manifest,
                assets,
                screen_source_paths,
            } => {
                let manifest = parse_manifest(&manifest, ManifestPolicy::default())
                    .map_err(|error| error.to_string())?;
                let owner = InstanceId::new(manifest.id).map_err(|error| error.to_string())?;
                let first_screen_id = module
                    .components
                    .first()
                    .map(|component| component.id.clone())
                    .ok_or("IR module has no entry component")?;
                let mut screen_ids = Vec::with_capacity(module.components.len());
                let mut screens = BTreeMap::new();
                let previous = match &self {
                    Self::Ir(old) if !reset && old.owner == owner => Some(old.as_ref()),
                    _ => None,
                };
                for component in &module.components {
                    if screens.contains_key(&component.id) {
                        return Err(format!("duplicate screen identity `{}`", component.id));
                    }
                    screen_ids.push(component.id.clone());
                    let previous_screen = previous
                        .and_then(|old| old.screens.get(&component.id).map(|screen| (old, screen)));
                    let (mount, session) = match previous_screen {
                        Some((old, screen)) => {
                            screen.session.hot_swap(&old.module, &module, &component.id)
                        }
                        None => InteractionSession::mount(&module, component, &BTreeMap::new()),
                    }
                    .map_err(|error| format!("{}: {}", error.code, error.message))?;
                    let registry = admitted_registry(owner.clone(), mount)?;
                    screens.insert(component.id.clone(), IrScreenState { session, registry });
                }
                let active_screen_id = previous
                    .map(|old| old.active_screen_id.as_str())
                    .filter(|screen_id| screens.contains_key(*screen_id))
                    .unwrap_or(first_screen_id.as_str())
                    .to_owned();
                *self = Self::Ir(Box::new(IrSurface {
                    module,
                    screens,
                    screen_ids,
                    active_screen_id,
                    screen_source_paths,
                    owner,
                    assets,
                }));
            }
            Replacement::Wasm(plugin) => {
                let snapshot = match self {
                    Self::Wasm(old)
                        if !reset && old.owner_id().as_str() == plugin.application_id() =>
                    {
                        old.snapshot_development_state()
                            .map_err(|error| error.to_string())?
                    }
                    _ => None,
                };
                let candidate = (*plugin)
                    .mount_development(snapshot.as_deref())
                    .map_err(|error| error.to_string())?;
                if !reset && snapshot.is_none() {
                    println!(
                        "[dev] Wasm replaced; no SDK hot state registered, guest state initialized"
                    );
                }
                *self = Self::Wasm(Box::new(candidate));
            }
        }
        Ok(())
    }

    /// Switch to a declared screen while preserving its retained state.
    pub fn switch_screen(&mut self, screen_id: &str) -> Result<bool, String> {
        match self {
            Self::Ir(surface) => {
                if !surface.screens.contains_key(screen_id) {
                    return Err(format!(
                        "screen `{screen_id}` is unavailable in this dev session"
                    ));
                }
                if surface.active_screen_id == screen_id {
                    return Ok(false);
                }
                surface.active_screen_id = screen_id.to_owned();
                Ok(true)
            }
            Self::Wasm(surface) if surface.owner_id().as_str() == screen_id => Ok(false),
            Self::Wasm(_) => Err(format!(
                "screen `{screen_id}` is unavailable in this dev session"
            )),
            Self::Empty => Err("no mounted application".into()),
        }
    }

    /// Resolve active-screen identity and source links from the committed surface.
    pub fn metadata(&self) -> Option<SurfaceMetadata> {
        match self {
            Self::Empty => None,
            Self::Wasm(surface) => Some(SurfaceMetadata {
                component_id: Some(surface.owner_id().as_str().to_owned()),
                screen_id: Some(surface.owner_id().as_str().to_owned()),
                screen_source_path: None,
                available_screen_ids: vec![surface.owner_id().as_str().to_owned()],
                source_locations: BTreeMap::new(),
            }),
            Self::Ir(surface) => {
                let component = surface
                    .module
                    .components
                    .iter()
                    .find(|component| component.id == surface.active_screen_id)?;
                let mut source_locations = BTreeMap::new();
                if let Some(path) = surface.screen_source_paths.get(&surface.active_screen_id) {
                    collect_source_locations(&component.template, path, &mut source_locations);
                }
                Some(SurfaceMetadata {
                    component_id: Some(surface.active_screen_id.clone()),
                    screen_id: Some(surface.active_screen_id.clone()),
                    screen_source_path: surface
                        .screen_source_paths
                        .get(&surface.active_screen_id)
                        .cloned(),
                    available_screen_ids: surface.screen_ids.clone(),
                    source_locations,
                })
            }
        }
    }
}

fn admitted_registry(owner: InstanceId, mount: MountTree) -> Result<UiRegistry, String> {
    let mut registry = UiRegistry::new(owner.clone(), ProtocolLimits::default());
    registry.mount(mount).map_err(|error| error.to_string())?;
    NativeStateStore::from_registry(&owner, &registry).map_err(|error| error.to_string())?;
    Ok(registry)
}

impl NativeSurface for DevSurface {
    type Error = String;
    fn render_tree(&self) -> Option<NativeRenderNode> {
        match self {
            Self::Wasm(surface) => NativeSurface::render_tree(surface.as_ref()),
            Self::Ir(surface) => {
                surface
                    .screens
                    .get(&surface.active_screen_id)
                    .and_then(|screen| {
                        screen
                            .registry
                            .root_id()
                            .and_then(|id| render_node(&screen.registry, &surface.owner, id))
                    })
            }
            Self::Empty => Some(NativeRenderNode::from(&UiNode {
                id: "studio-dev-root".into(),
                kind: studio_protocol::NodeKind::Box,
                props: BTreeMap::new(),
                children: Vec::new(),
            })),
        }
    }
    fn process_input(&mut self, node_id: &str, action: InputAction) -> Result<(), String> {
        match self {
            Self::Wasm(surface) => NativeSurface::process_input(surface.as_mut(), node_id, action)
                .map_err(|error| error.to_string()),
            Self::Ir(surface) => {
                let owner = surface.owner.clone();
                let active_screen_id = surface.active_screen_id.clone();
                let module = Arc::clone(&surface.module);
                let component = module
                    .components
                    .iter()
                    .find(|component| component.id == active_screen_id)
                    .ok_or("active screen component is missing")?;
                let screen = surface
                    .screens
                    .get_mut(&active_screen_id)
                    .ok_or("active screen state is missing")?;
                let mut dispatcher = HostEventDispatcher::new(owner.clone());
                register_tree(
                    &screen.registry,
                    &owner,
                    screen.registry.root_id().ok_or("no root")?,
                    &mut dispatcher,
                )?;
                let HostEvent::Ui(event) = dispatcher
                    .dispatch(&owner, node_id, action)
                    .map_err(|error| error.to_string())?
                else {
                    return Ok(());
                };
                if !component.handlers.iter().any(|handler| {
                    handler.node.as_deref() == Some(node_id) && handler.event == event.event
                }) {
                    return Ok(());
                }
                let mut candidate = screen.session.clone();
                let message = candidate
                    .apply(component, node_id, &event.event)
                    .map_err(|error| error.message)?;
                match message {
                    SessionMessage::Patch(batch) => {
                        screen
                            .registry
                            .apply_patch(&owner, batch)
                            .map_err(|error| error.to_string())?;
                    }
                    SessionMessage::Event(event) => println!("[dev] {}", event.to_json()),
                }
                screen.session = candidate;
                Ok(())
            }
            Self::Empty => Ok(()),
        }
    }
    fn poll_action_results(&mut self) -> Result<bool, String> {
        match self {
            Self::Wasm(surface) => NativeSurface::poll_action_results(surface.as_mut())
                .map_err(|error| error.to_string()),
            _ => Ok(false),
        }
    }
    fn asset(&self, path: &str) -> Option<&[u8]> {
        match self {
            Self::Wasm(surface) => NativeSurface::asset(surface.as_ref(), path),
            Self::Ir(surface) => surface.assets.get(path).map(Vec::as_slice),
            Self::Empty => None,
        }
    }
    fn warning(&self) -> Option<&str> {
        Some("Development mode")
    }
}

fn register_tree(
    registry: &UiRegistry,
    owner: &InstanceId,
    id: &str,
    dispatcher: &mut HostEventDispatcher,
) -> Result<(), String> {
    let node = registry.get(owner, id).map_err(|error| error.to_string())?;
    dispatcher
        .register(node.id.clone(), node.kind)
        .map_err(|error| error.to_string())?;
    for child in &node.children {
        register_tree(registry, owner, child, dispatcher)?;
    }
    Ok(())
}

fn render_node(registry: &UiRegistry, owner: &InstanceId, id: &str) -> Option<NativeRenderNode> {
    let node = registry.get(owner, id).ok()?;
    Some(NativeRenderNode {
        id: node.id.clone(),
        kind: node.kind,
        control: studio_components::RuntimeControl::from_node_kind(node.kind),
        props: node.props.clone(),
        children: node
            .children
            .iter()
            .filter_map(|child| render_node(registry, owner, child))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"<script lang="ts">
let quantity = $state(1);
function add() { quantity += 1; }
</script>
<Column id="content"><Text id="quantity">{quantity}</Text><Button id="add" onclick={add}>Add</Button></Column>"#;

    fn update(source: &str) -> Replacement {
        Replacement::Ir {
            module: Arc::new(studio_script::compile_studio(source, "app.studio", "app").unwrap()),
            manifest: include_bytes!("../../../../examples/starter/manifest.json").to_vec(),
            assets: BTreeMap::new(),
            screen_source_paths: BTreeMap::from([("app".to_owned(), "app.studio".to_owned())]),
        }
    }
    fn project_update(screens: &[(&str, u8)]) -> Replacement {
        let mut components = Vec::with_capacity(screens.len());
        let mut exports = Vec::with_capacity(screens.len());
        let mut source_paths = BTreeMap::new();
        let mut version = None;
        for (id, initial) in screens {
            let path = format!("screens/{id}.studio");
            let source = format!(
                "<script lang=\"ts\">let count = $state({initial}); function add() {{ count += 1; }}</script><Column id=\"root\"><Text id=\"quantity\">{{count}}</Text><Button id=\"add\" onclick={{add}}>Add</Button></Column>"
            );
            let module = studio_script::compile_studio(&source, &path, id).unwrap();
            version = Some(module.version);
            components.push(module.components[0].clone());
            exports.push((*id).to_owned());
            source_paths.insert((*id).to_owned(), path);
        }
        Replacement::Ir {
            module: Arc::new(StudioModule {
                version: version.unwrap_or(studio_script::ir::STUDIO_IR_VERSION),
                id: "studio-project".to_owned(),
                imports: Vec::new(),
                exports,
                components,
                dependencies: Vec::new(),
            }),
            manifest: include_bytes!("../../../../examples/starter/manifest.json").to_vec(),
            assets: BTreeMap::new(),
            screen_source_paths: source_paths,
        }
    }
    fn quantity(surface: &DevSurface) -> String {
        fn find(node: &NativeRenderNode) -> Option<String> {
            if node.id == "quantity" {
                return node
                    .props
                    .get("text")
                    .and_then(|text| text.as_str())
                    .map(str::to_owned);
            }
            node.children.iter().find_map(find)
        }
        find(&surface.render_tree().unwrap()).unwrap()
    }

    #[test]
    fn ir_metadata_exposes_the_single_screen_and_template_source_spans() {
        let mut surface = DevSurface::Empty;
        surface.replace(update(SOURCE), false).unwrap();
        let metadata = surface.metadata().unwrap();
        assert_eq!(metadata.component_id.as_deref(), Some("app"));
        assert_eq!(metadata.screen_id.as_deref(), Some("app"));
        assert_eq!(metadata.screen_source_path.as_deref(), Some("app.studio"));
        let content = metadata.source_locations.get("content").unwrap();
        assert_eq!(content.path, "app.studio");
        assert_eq!(content.line, 5);
        assert!(metadata.source_locations.contains_key("quantity"));
    }

    #[test]
    fn project_metadata_tracks_the_active_screen_source_and_source_spans() {
        let home = studio_script::compile_studio(
            "<Text id=\"title\">Home</Text>",
            "screens/home.studio",
            "home",
        )
        .unwrap();
        let settings = studio_script::compile_studio(
            "<Text id=\"title\">Settings</Text>",
            "screens/settings.studio",
            "settings",
        )
        .unwrap();
        let module = StudioModule {
            version: home.version,
            id: "studio-project".to_owned(),
            imports: Vec::new(),
            exports: vec!["home".to_owned(), "settings".to_owned()],
            components: vec![home.components[0].clone(), settings.components[0].clone()],
            dependencies: Vec::new(),
        };
        let replacement = Replacement::Ir {
            module: Arc::new(module),
            manifest: include_bytes!("../../../../examples/starter/manifest.json").to_vec(),
            assets: BTreeMap::new(),
            screen_source_paths: BTreeMap::from([
                ("home".to_owned(), "screens/home.studio".to_owned()),
                ("settings".to_owned(), "screens/settings.studio".to_owned()),
            ]),
        };

        let mut surface = DevSurface::Empty;
        surface.replace(replacement, false).unwrap();
        let metadata = surface.metadata().unwrap();
        assert_eq!(metadata.screen_id.as_deref(), Some("home"));
        assert_eq!(
            metadata.screen_source_path.as_deref(),
            Some("screens/home.studio")
        );
        assert_eq!(
            metadata
                .available_screen_ids
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["home", "settings"]
        );
        assert_eq!(
            metadata
                .source_locations
                .get("title")
                .map(|location| location.path.as_str()),
            Some("screens/home.studio")
        );
        surface.switch_screen("settings").unwrap();
        let settings = surface.metadata().unwrap();
        assert_eq!(settings.screen_id.as_deref(), Some("settings"));
        assert_eq!(
            settings.screen_source_path.as_deref(),
            Some("screens/settings.studio")
        );
        assert_eq!(
            settings
                .source_locations
                .get("title")
                .map(|location| location.path.as_str()),
            Some("screens/settings.studio")
        );
    }

    #[test]
    fn screen_switching_retains_each_state_and_reload_preserves_or_falls_back_active_id() {
        let mut surface = DevSurface::Empty;
        surface
            .replace(project_update(&[("home", 1), ("settings", 50)]), false)
            .unwrap();
        surface
            .process_input("add", InputAction::PointerClick)
            .unwrap();
        assert_eq!(quantity(&surface), "2");
        surface.switch_screen("settings").unwrap();
        assert_eq!(quantity(&surface), "50");
        surface
            .process_input("add", InputAction::PointerClick)
            .unwrap();
        assert_eq!(quantity(&surface), "51");
        assert!(surface.switch_screen("missing").is_err());

        surface
            .replace(project_update(&[("settings", 50), ("home", 1)]), false)
            .unwrap();
        assert_eq!(quantity(&surface), "51");
        let metadata = surface.metadata().unwrap();
        assert_eq!(metadata.screen_id.as_deref(), Some("settings"));
        assert_eq!(
            metadata.screen_source_path.as_deref(),
            Some("screens/settings.studio")
        );
        assert_eq!(
            metadata.source_locations.get("quantity").unwrap().path,
            "screens/settings.studio"
        );

        surface.switch_screen("home").unwrap();
        assert_eq!(quantity(&surface), "2");
        surface
            .replace(project_update(&[("settings", 50), ("home", 1)]), false)
            .unwrap();
        assert_eq!(quantity(&surface), "2");

        surface.switch_screen("settings").unwrap();
        surface
            .replace(project_update(&[("home", 1)]), false)
            .unwrap();
        assert_eq!(quantity(&surface), "2");
        assert_eq!(
            surface.metadata().unwrap().screen_id.as_deref(),
            Some("home")
        );
    }

    #[test]
    fn ir_hot_reload_preserves_live_values_and_rejection_keeps_the_session_interactive() {
        let mut surface = DevSurface::Empty;
        surface.replace(update(SOURCE), false).unwrap();
        surface
            .process_input("add", InputAction::PointerClick)
            .unwrap();
        assert_eq!(quantity(&surface), "2");
        let changed = SOURCE
            .replace("$state(1)", "$state(50)")
            .replace("{quantity}</Text>", "{quantity} edited</Text>");
        surface.replace(update(&changed), false).unwrap();
        assert_eq!(quantity(&surface), "2 edited");
        let Replacement::Ir {
            module,
            manifest,
            assets,
            screen_source_paths,
        } = update(&changed)
        else {
            unreachable!()
        };
        let mut incompatible = (*module).clone();
        incompatible.version += 1;
        assert!(surface
            .replace(
                Replacement::Ir {
                    module: Arc::new(incompatible),
                    manifest,
                    assets,
                    screen_source_paths,
                },
                false
            )
            .is_err());
        surface
            .process_input("add", InputAction::KeyboardActivate)
            .unwrap();
        assert_eq!(quantity(&surface), "3 edited");
        surface.replace(update(&changed), true).unwrap();
        assert_eq!(quantity(&surface), "50 edited");
    }

    #[test]
    fn incompatible_state_types_initialize_without_discarding_the_old_surface_on_invalid_mounts() {
        let mut surface = DevSurface::Empty;
        surface.replace(update(SOURCE), false).unwrap();
        let changed = "<script lang=\"ts\">let quantity = $state(\"fresh\");</script><Text id=\"quantity\">{quantity}</Text>";
        surface.replace(update(changed), false).unwrap();
        assert_eq!(quantity(&surface), "fresh");
        let required = SOURCE.replace(
            "let quantity",
            "let { requiredText } = $props(); let quantity",
        );
        assert!(surface.replace(update(&required), false).is_err());
        assert_eq!(quantity(&surface), "fresh");
    }
}
