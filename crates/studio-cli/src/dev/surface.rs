//! The development host's live state. Production surfaces contain no IR interpreter.
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use studio_navigation::timers::{TimerManifest, TimerSchedule, TimerSpec, TIMER_ASSET};

use studio_components::native_view::{NativeRenderNode, NativeSurface};
use studio_components::{HostEventDispatcher, InputAction, NativeStateStore};
use studio_navigation::project::{ProjectRouter, RoutingManifest, ROUTING_ASSET};
use studio_package::{parse_manifest, ManifestPolicy};
use studio_protocol::dev_rpc::DevSourceLocation;
use studio_protocol::NavigationCommand;
use studio_protocol::{HostEvent, MountTree, ProtocolLimits, UiNode};
use studio_runtime::host::PreparedPlugin;
use studio_runtime::plugin_surface::PluginSurface;
use studio_script::eval::{InteractionSession, SessionMessage};
use studio_script::ir::{StudioModule, TemplateNode};
use studio_script::types::StudioValue;
use studio_ui::{InstanceId, UiRegistry};

pub(crate) struct IrSurface {
    module: Arc<StudioModule>,
    screens: BTreeMap<String, IrScreenState>,
    screen_ids: Vec<String>,
    active_screen_id: String,
    router: Option<ProjectRouter>,
    timer_manifest: Option<TimerManifest>,
    timer_schedule: Option<TimerSchedule>,
    timer_origin: Instant,
    timer_elapsed: Duration,
    timer_generation: u64,
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
                let screen_ids = module
                    .components
                    .iter()
                    .map(|component| component.id.clone())
                    .collect();
                let previous = match &*self {
                    Self::Ir(previous) if !reset && previous.owner == owner => {
                        Some(previous.as_ref())
                    }
                    _ => None,
                };
                let mut router = assets
                    .get(ROUTING_ASSET)
                    .map(|bytes| {
                        let routing = RoutingManifest::from_bytes(bytes)?;
                        for route in &routing.routes {
                            if !module
                                .components
                                .iter()
                                .any(|component| component.id == route.id)
                            {
                                return Err(format!(
                                    "route `{}` has no compiled component `{}`",
                                    route.path, route.id
                                ));
                            }
                        }
                        match previous.and_then(|previous| previous.router.as_ref()) {
                            Some(old) => old.reconfigure(&routing),
                            None => ProjectRouter::new(&routing, &routing.initial_route()?),
                        }
                    })
                    .transpose()?;
                let selected = router.as_ref().map(ProjectRouter::current);
                let active_screen_id = selected
                    .as_ref()
                    .map(|selection| selection.id.clone())
                    .or_else(|| {
                        previous
                            .map(|old| old.active_screen_id.clone())
                            .filter(|id| {
                                module
                                    .components
                                    .iter()
                                    .any(|component| &component.id == id)
                            })
                    })
                    .unwrap_or(first_screen_id);
                let mut screens = BTreeMap::new();
                for component in &module.components {
                    if screens.contains_key(&component.id) {
                        return Err(format!("duplicate screen identity `{}`", component.id));
                    }
                    let prior = previous
                        .and_then(|old| old.screens.get(&component.id).map(|screen| (old, screen)));
                    if router.is_some() && prior.is_none() && component.id != active_screen_id {
                        continue;
                    }
                    let (mut mount, session) = match prior {
                        Some((old, screen)) => {
                            screen.session.hot_swap(&old.module, &module, &component.id)
                        }
                        None => {
                            let props = selected
                                .as_ref()
                                .filter(|selection| selection.id == component.id)
                                .map(|selection| {
                                    selection
                                        .params
                                        .iter()
                                        .map(|(key, value)| {
                                            (key.clone(), StudioValue::String(value.clone()))
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            InteractionSession::mount(&module, component, &props)
                        }
                    }
                    .map_err(|error| format!("{}: {}", error.code, error.message))?;
                    if let Some(selection) = &selected {
                        if selection.id == component.id {
                            mount.route.clone_from(&selection.route);
                        }
                    }
                    let registry = admitted_registry(owner.clone(), mount)?;
                    screens.insert(component.id.clone(), IrScreenState { session, registry });
                }
                let timer_manifest = assets
                    .get(TIMER_ASSET)
                    .map(|bytes| TimerManifest::from_bytes(bytes))
                    .transpose()?;
                if let Some(timers) = &timer_manifest {
                    if router.is_none() {
                        return Err("host timers require a routed Studio project".into());
                    }
                    if timers.pages.iter().any(|page| {
                        !module
                            .components
                            .iter()
                            .any(|component| component.id == page.id)
                    }) {
                        return Err("timer metadata names an undeclared page".into());
                    }
                }
                let timer_origin = previous.map_or_else(Instant::now, |old| old.timer_origin);
                let timer_elapsed = previous.map_or(Duration::ZERO, |old| old.timer_elapsed);
                let now = previous.map_or(Duration::ZERO, |old| {
                    old.timer_elapsed.max(old.timer_origin.elapsed())
                });
                let active_registry = &screens
                    .get(&active_screen_id)
                    .ok_or("active page missing")?
                    .registry;
                let prepared = timer_schedule_for_page(
                    timer_manifest.as_ref(),
                    &active_screen_id,
                    &owner,
                    active_registry,
                    now,
                )?;
                let timer_schedule = previous
                    .filter(|old| {
                        old.active_screen_id == active_screen_id
                            && old.assets.get(TIMER_ASSET) == assets.get(TIMER_ASSET)
                    })
                    .map_or(prepared, |old| old.timer_schedule.clone());
                let timer_generation =
                    previous.map_or(0, |old| old.timer_generation.saturating_add(1));
                *self = Self::Ir(Box::new(IrSurface {
                    module,
                    screens,
                    screen_ids,
                    active_screen_id,
                    router: router.take(),
                    timer_manifest,
                    timer_schedule,
                    timer_origin,
                    timer_elapsed,
                    timer_generation,
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
                if let Some(router) = &surface.router {
                    if router.current().id == screen_id {
                        return Ok(false);
                    }
                    let routing = RoutingManifest::from_bytes(
                        surface
                            .assets
                            .get(ROUTING_ASSET)
                            .ok_or("missing routing metadata")?,
                    )?;
                    let route = routing
                        .routes
                        .iter()
                        .find(|route| route.id == screen_id)
                        .ok_or("unknown page")?;
                    if route.path.contains([':', '*']) {
                        return Err(
                            "parameter pages require a concrete navigation destination".into()
                        );
                    }
                    surface.navigate(
                        &NavigationCommand::Replace {
                            route: route.path.clone(),
                        },
                        None,
                    )?;
                    return Ok(true);
                }
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

    /// Inspect identity without rebuilding expanded source maps during idle polls.
    pub fn active_screen_id(&self) -> Option<&str> {
        match self {
            Self::Ir(surface) => Some(&surface.active_screen_id),
            Self::Wasm(surface) => Some(surface.owner_id().as_str()),
            Self::Empty => None,
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
                if let Some(bytes) = surface.assets.get(crate::studio_project::SOURCE_MAP_ASSET) {
                    let maps: BTreeMap<String, BTreeMap<String, DevSourceLocation>> =
                        serde_json::from_slice(bytes).ok()?;
                    if let Some(locations) = maps.get(&surface.active_screen_id) {
                        source_locations.clone_from(locations);
                    }
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

impl IrSurface {
    fn navigate(
        &mut self,
        command: &NavigationCommand,
        source: Option<(String, InteractionSession)>,
    ) -> Result<(), String> {
        let mut router = self
            .router
            .as_ref()
            .ok_or("this project has no file-based route declaration")?
            .clone();
        let selection = router.navigate(command)?;
        let component = self
            .module
            .components
            .iter()
            .find(|component| component.id == selection.id)
            .ok_or("route component missing")?;
        let props = selection
            .params
            .iter()
            .map(|(key, value)| (key.clone(), StudioValue::String(value.clone())))
            .collect();
        let target_session = source
            .as_ref()
            .filter(|(id, _)| id == &selection.id)
            .map(|(_, session)| session)
            .or_else(|| {
                self.screens
                    .get(&selection.id)
                    .map(|screen| &screen.session)
            });
        let (mut mount, session) = match target_session {
            Some(session) => session.remount_with_props(&self.module, component, &props),
            None => InteractionSession::mount(&self.module, component, &props),
        }
        .map_err(|error| error.message)?;
        mount.route = selection.route;
        let registry = admitted_registry(self.owner.clone(), mount)?;
        let timer_schedule = timer_schedule_for_page(
            self.timer_manifest.as_ref(),
            &selection.id,
            &self.owner,
            &registry,
            self.timer_elapsed.max(self.timer_origin.elapsed()),
        )?;
        if let Some((id, session)) = source {
            if let Some(screen) = self.screens.get_mut(&id) {
                screen.session = session;
            }
        }
        self.screens
            .insert(selection.id.clone(), IrScreenState { session, registry });
        self.active_screen_id = selection.id;
        self.router = Some(router);
        self.timer_schedule = timer_schedule;
        self.timer_generation = self.timer_generation.saturating_add(1);
        Ok(())
    }

    fn apply_event(&mut self, node_id: &str, event: &str) -> Result<(), String> {
        let active_id = self.active_screen_id.clone();
        let module = Arc::clone(&self.module);
        let component = module
            .components
            .iter()
            .find(|component| component.id == active_id)
            .ok_or("active page component missing")?;
        if !component
            .handlers
            .iter()
            .any(|handler| handler.node.as_deref() == Some(node_id) && handler.event == event)
        {
            return Ok(());
        }
        let screen = self
            .screens
            .get_mut(&active_id)
            .ok_or("active page state missing")?;
        let mut candidate = screen.session.clone();
        let message = candidate
            .apply(component, node_id, event)
            .map_err(|error| error.message)?;
        match message {
            SessionMessage::Patch(batch) => {
                screen
                    .registry
                    .apply_patch(&self.owner, batch)
                    .map_err(|error| error.to_string())?;
            }
            SessionMessage::Event(event) if event.name == "navigate" => {
                let message = studio_protocol::decode_guest_message(
                    event.to_json().as_bytes(),
                    ProtocolLimits::default(),
                )
                .map_err(|error| error.to_string())?;
                let studio_protocol::GuestMessage::Navigate(command) = message else {
                    return Err("invalid navigation emission".into());
                };
                return self.navigate(&command, Some((active_id, candidate)));
            }
            SessionMessage::Event(event) => println!("[dev] {}", event.to_json()),
        }
        screen.session = candidate;
        Ok(())
    }

    fn active_timer_specs(&self) -> Vec<TimerSpec> {
        self.timer_manifest
            .as_ref()
            .and_then(|manifest| {
                manifest
                    .pages
                    .iter()
                    .find(|page| page.id == self.active_screen_id)
            })
            .map(|page| page.timers.clone())
            .unwrap_or_default()
    }

    fn completed_timers(&self, specs: &[TimerSpec]) -> Result<BTreeSet<String>, String> {
        let registry = &self
            .screens
            .get(&self.active_screen_id)
            .ok_or("active page missing")?
            .registry;
        let mut completed = BTreeSet::new();
        for spec in specs {
            let status = registry
                .get(&self.owner, &spec.status_node)
                .map_err(|error| error.to_string())?;
            match status.props.get("text").and_then(serde_json::Value::as_str) {
                Some("complete") => {
                    completed.insert(spec.id.clone());
                }
                Some("running") => {}
                _ => return Err("timer status must be running or complete".into()),
            }
        }
        Ok(completed)
    }

    fn poll_timers_at(&mut self, now: Duration) -> Result<bool, String> {
        self.timer_elapsed = self.timer_elapsed.max(now);
        let now = self.timer_elapsed;
        let specs = self.active_timer_specs();
        if specs.is_empty() {
            return Ok(false);
        }
        let generation = self.timer_generation;
        let mut changed = false;
        for _ in 0..studio_navigation::timers::MAX_TIMER_EVENTS {
            if self.timer_generation != generation {
                break;
            }
            let completed = self.completed_timers(&specs)?;
            let Some(event) = self
                .timer_schedule
                .as_mut()
                .and_then(|schedule| schedule.poll_one(now, &completed))
            else {
                break;
            };
            self.apply_event(&event.node_id, &event.event)?;
            changed = true;
            if self.timer_generation != generation {
                break;
            }
            let completed = self.completed_timers(&specs)?;
            if let Some(schedule) = self.timer_schedule.as_mut() {
                schedule.observe(now, &completed);
            }
        }
        Ok(changed)
    }
}

fn timer_schedule_for_page(
    manifest: Option<&TimerManifest>,
    id: &str,
    owner: &InstanceId,
    registry: &UiRegistry,
    now: Duration,
) -> Result<Option<TimerSchedule>, String> {
    let Some(page) = manifest.and_then(|manifest| manifest.pages.iter().find(|page| page.id == id))
    else {
        return Ok(None);
    };
    for spec in &page.timers {
        let wrapper = registry
            .get(owner, &spec.id)
            .map_err(|error| error.to_string())?;
        if wrapper.kind != studio_protocol::NodeKind::Box
            || wrapper
                .props
                .get("visible")
                .and_then(serde_json::Value::as_bool)
                != Some(false)
        {
            return Err("timer targets must belong to an invisible Timer box".into());
        }
        let mut descendants = BTreeSet::new();
        let mut pending = wrapper.children.clone();
        while let Some(node) = pending.pop() {
            let record = registry
                .get(owner, &node)
                .map_err(|error| error.to_string())?;
            descendants.insert(node);
            pending.extend(record.children.iter().cloned());
        }
        for (node, kind) in [
            (&spec.tick_node, studio_protocol::NodeKind::Button),
            (&spec.complete_node, studio_protocol::NodeKind::Button),
            (&spec.status_node, studio_protocol::NodeKind::Text),
        ] {
            if !descendants.contains(node)
                || registry
                    .get(owner, node)
                    .map_err(|error| error.to_string())?
                    .kind
                    != kind
            {
                return Err("timer target kind or ownership is invalid".into());
            }
        }
    }
    TimerSchedule::new(&page.timers, now).map(Some)
}

#[cfg(test)]
impl DevSurface {
    fn poll_timers_at(&mut self, now: Duration) -> Result<bool, String> {
        match self {
            Self::Ir(surface) => surface.poll_timers_at(now),
            _ => Ok(false),
        }
    }
}

fn admitted_registry(owner: InstanceId, mount: MountTree) -> Result<UiRegistry, String> {
    studio_protocol::validate_guest_message(
        &studio_protocol::GuestMessage::Mount(mount.clone()),
        ProtocolLimits::default(),
    )
    .map_err(|error| error.to_string())?;
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
                let screen = surface
                    .screens
                    .get(&surface.active_screen_id)
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
                surface.apply_event(node_id, &event.event)
            }
            Self::Empty => Ok(()),
        }
    }
    fn poll_action_results(&mut self) -> Result<bool, String> {
        match self {
            Self::Wasm(surface) => NativeSurface::poll_action_results(surface.as_mut())
                .map_err(|error| error.to_string()),
            Self::Ir(surface) => surface.poll_timers_at(surface.timer_origin.elapsed()),
            Self::Empty => Ok(false),
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

#[cfg(test)]
mod file_route_tests {
    use super::*;

    fn routed_update() -> Replacement {
        let sources = [
            (
                "home",
                "routes/+page.studio",
                r#"<script lang="ts">let count = $state(0); function add() { count += 1; } function live() { emit("navigate", { operation: "push", route: "/live" }); }</script><Column id="root"><Text id="count">{count}</Text><Button id="add" onclick={add}>Add</Button><Button id="live" onclick={live}>Live</Button></Column>"#,
            ),
            (
                "live",
                "routes/live/+page.studio",
                r#"<script lang="ts">function back() { emit("navigate", { operation: "pop" }); }</script><Column id="live-root"><Text id="live-title">Live</Text><Button id="back" onclick={back}>Back</Button></Column>"#,
            ),
        ];
        let mut components = Vec::new();
        let mut paths = BTreeMap::new();
        for (id, path, source) in sources {
            components.extend(
                studio_script::compile_studio(source, path, id)
                    .unwrap()
                    .components,
            );
            paths.insert(id.to_owned(), path.to_owned());
        }
        Replacement::Ir {
            module: Arc::new(StudioModule {
                version: studio_script::ir::STUDIO_IR_VERSION,
                id: "file-routes".into(), imports: Vec::new(), exports: vec!["home".into(), "live".into()],
                components, dependencies: Vec::new(),
            }),
            manifest: include_bytes!("../../../../examples/starter/manifest.json").to_vec(),
            assets: BTreeMap::from([("assets/studio/routes.json".into(), br#"{"schemaVersion":1,"routes":[{"id":"home","path":"/","source":"routes/+page.studio"},{"id":"live","path":"/live","source":"routes/live/+page.studio"}]}"#.to_vec())]),
            screen_source_paths: paths,
        }
    }

    #[test]
    fn studio_handlers_navigate_through_host_history_and_restore_state() {
        let mut surface = DevSurface::Empty;
        surface.replace(routed_update(), false).unwrap();
        surface
            .process_input("add", InputAction::PointerClick)
            .unwrap();
        surface
            .process_input("live", InputAction::PointerClick)
            .unwrap();
        assert_eq!(
            surface.metadata().unwrap().screen_id.as_deref(),
            Some("live")
        );
        surface
            .process_input("back", InputAction::PointerClick)
            .unwrap();
        assert_eq!(
            surface.metadata().unwrap().screen_id.as_deref(),
            Some("home")
        );
        fn count(node: &NativeRenderNode) -> Option<&str> {
            if node.id == "count" {
                return node.props.get("text").and_then(serde_json::Value::as_str);
            }
            node.children.iter().find_map(count)
        }
        let tree = surface.render_tree().unwrap();
        assert_eq!(count(&tree), Some("1"));
    }

    #[test]
    fn routed_reload_preserves_history_and_current_screen() {
        let mut surface = DevSurface::Empty;
        surface.replace(routed_update(), false).unwrap();
        surface
            .process_input("live", InputAction::PointerClick)
            .unwrap();
        surface.replace(routed_update(), false).unwrap();
        assert_eq!(
            surface.metadata().unwrap().screen_id.as_deref(),
            Some("live")
        );
        surface
            .process_input("back", InputAction::PointerClick)
            .unwrap();
        assert_eq!(
            surface.metadata().unwrap().screen_id.as_deref(),
            Some("home")
        );
    }
}

#[cfg(test)]
mod timer_route_tests {
    use super::*;
    use std::time::Duration;

    fn timed_update() -> Replacement {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("routes/live")).unwrap();
        std::fs::write(dir.path().join("routes/+page.studio"), r#"<script lang="ts">let remaining = $state(2); function tick() { remaining = remaining > 0 ? remaining - 1 : 0; } function complete() { emit('navigate', { operation: 'replace', route: '/live' }); }</script><Box id="clock" opacity={remaining === 0 ? 0 : 1} transition={{duration_ms: 700, curve: 'ease_out'}}><Text id="remaining">{remaining}</Text></Box><Timer id="clock-timer" interval_ms={1000} completion_delay_ms={700} until={remaining === 0} ontick={tick} oncomplete={complete} />"#).unwrap();
        std::fs::write(
            dir.path().join("routes/live/+page.studio"),
            "<Text id=\"live\">Live</Text>",
        )
        .unwrap();
        let project = crate::studio_project::compile_routes(dir.path())
            .unwrap()
            .unwrap();
        let assets = project.assets().unwrap();
        Replacement::Ir {
            module: Arc::new(project.module),
            manifest: include_bytes!("../../../../examples/starter/manifest.json").to_vec(),
            assets,
            screen_source_paths: project
                .routing
                .routes
                .into_iter()
                .map(|route| (route.id, route.source))
                .collect(),
        }
    }
    fn text(surface: &DevSurface, id: &str) -> Option<String> {
        fn find(node: &NativeRenderNode, id: &str) -> Option<String> {
            if node.id == id {
                return node
                    .props
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);
            }
            node.children.iter().find_map(|child| find(child, id))
        }
        find(&surface.render_tree().unwrap(), id)
    }

    #[test]
    fn ir_countdown_ticks_fades_then_navigates_on_the_host_clock() {
        let mut surface = DevSurface::Empty;
        surface.replace(timed_update(), false).unwrap();
        assert_eq!(text(&surface, "remaining").as_deref(), Some("2"));
        assert!(!surface.poll_timers_at(Duration::from_millis(999)).unwrap());
        assert!(surface.poll_timers_at(Duration::from_secs(2)).unwrap());
        assert_eq!(text(&surface, "remaining").as_deref(), Some("0"));
        assert!(!surface.poll_timers_at(Duration::from_millis(2699)).unwrap());
        assert!(surface.poll_timers_at(Duration::from_millis(2700)).unwrap());
        assert_eq!(
            surface.metadata().unwrap().screen_source_path.as_deref(),
            Some("routes/live/+page.studio")
        );
        assert!(!surface.poll_timers_at(Duration::from_secs(60)).unwrap());
    }

    #[test]
    fn source_reload_preserves_the_countdown_value_and_pending_deadline() {
        let mut surface = DevSurface::Empty;
        surface.replace(timed_update(), false).unwrap();
        surface.poll_timers_at(Duration::from_secs(1)).unwrap();
        surface.replace(timed_update(), false).unwrap();
        assert_eq!(text(&surface, "remaining").as_deref(), Some("1"));
        surface.poll_timers_at(Duration::from_secs(2)).unwrap();
        assert_eq!(text(&surface, "remaining").as_deref(), Some("0"));
    }
}
