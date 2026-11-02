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
    session: InteractionSession,
    registry: UiRegistry,
    owner: InstanceId,
    assets: BTreeMap<String, Vec<u8>>,
}

pub(crate) enum Replacement {
    Ir {
        module: Arc<StudioModule>,
        manifest: Vec<u8>,
        assets: BTreeMap<String, Vec<u8>>,
        source_path: Option<String>,
    },
    Wasm(Box<PreparedPlugin>),
    Assets(BTreeMap<String, Vec<u8>>),
}

/// Source and identity metadata for one prepared preview replacement.
pub(crate) struct SurfaceMetadata {
    pub component_id: Option<String>,
    pub screen_id: Option<String>,
    pub source_locations: BTreeMap<String, DevSourceLocation>,
}

impl Replacement {
    /// Resolve identity and source links before the replacement is consumed.
    pub(crate) fn metadata(&self) -> Option<SurfaceMetadata> {
        match self {
            Self::Assets(_) => None,
            Self::Wasm(plugin) => Some(SurfaceMetadata {
                component_id: Some(plugin.application_id().to_owned()),
                screen_id: Some(plugin.application_id().to_owned()),
                source_locations: BTreeMap::new(),
            }),
            Self::Ir {
                module,
                source_path,
                ..
            } => {
                let component = module.components.first()?;
                let mut source_locations = BTreeMap::new();
                if let Some(path) = source_path {
                    collect_source_locations(&component.template, path, &mut source_locations);
                }
                Some(SurfaceMetadata {
                    component_id: Some(component.id.clone()),
                    // `studio dev` currently mounts the module's sole entry component as its
                    // only screen identity. Multi-screen routing is not represented by this IR.
                    screen_id: Some(component.id.clone()),
                    source_locations,
                })
            }
        }
    }
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
                source_path: _,
            } => {
                let manifest = parse_manifest(&manifest, ManifestPolicy::default())
                    .map_err(|error| error.to_string())?;
                let owner = InstanceId::new(manifest.id).map_err(|error| error.to_string())?;
                let component = module
                    .components
                    .first()
                    .ok_or("IR module has no entry component")?;
                let (mount, session) = match &self {
                    Self::Ir(old) if !reset && old.owner == owner => {
                        old.session.hot_swap(&old.module, &module, &component.id)
                    }
                    _ => InteractionSession::mount(&module, component, &BTreeMap::new()),
                }
                .map_err(|error| format!("{}: {}", error.code, error.message))?;
                let registry = admitted_registry(owner.clone(), mount)?;
                *self = Self::Ir(Box::new(IrSurface {
                    module,
                    session,
                    registry,
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
            Self::Ir(surface) => surface
                .registry
                .root_id()
                .and_then(|id| render_node(&surface.registry, &surface.owner, id)),
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
                let mut dispatcher = HostEventDispatcher::new(surface.owner.clone());
                register_tree(
                    &surface.registry,
                    &surface.owner,
                    surface.registry.root_id().ok_or("no root")?,
                    &mut dispatcher,
                )?;
                let HostEvent::Ui(event) = dispatcher
                    .dispatch(&surface.owner, node_id, action)
                    .map_err(|error| error.to_string())?
                else {
                    return Ok(());
                };
                let component = surface
                    .module
                    .components
                    .first()
                    .ok_or("no entry component")?;
                if !component.handlers.iter().any(|handler| {
                    handler.node.as_deref() == Some(node_id) && handler.event == event.event
                }) {
                    return Ok(());
                }
                let mut candidate = surface.session.clone();
                let message = candidate
                    .apply(component, node_id, &event.event)
                    .map_err(|error| error.message)?;
                match message {
                    SessionMessage::Patch(batch) => {
                        surface
                            .registry
                            .apply_patch(&surface.owner, batch)
                            .map_err(|error| error.to_string())?;
                    }
                    SessionMessage::Event(event) => println!("[dev] {}", event.to_json()),
                }
                surface.session = candidate;
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
            source_path: Some("app.studio".to_owned()),
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
        let replacement = update(SOURCE);
        let metadata = replacement.metadata().unwrap();
        assert_eq!(metadata.component_id.as_deref(), Some("app"));
        assert_eq!(metadata.screen_id.as_deref(), Some("app"));
        let content = metadata.source_locations.get("content").unwrap();
        assert_eq!(content.path, "app.studio");
        assert_eq!(content.line, 5);
        assert!(metadata.source_locations.contains_key("quantity"));
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
            source_path,
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
                    source_path,
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
