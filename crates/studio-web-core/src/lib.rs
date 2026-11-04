//! Closed browser bridge for Studio routing, retained UI and host timers.
//!
//! The browser worker owns guest execution and asset admission. This bridge owns
//! route history, protocol validation, atomic retained-tree admission and timers.

use std::collections::BTreeSet;
use std::time::Duration;

use serde_json::{Value, json};
use studio_navigation::project::{ProjectRouter, RouteSelection, RoutingManifest};
use studio_navigation::timers::{MAX_PAGE_TIMERS, TimerManifest, TimerSchedule, TimerSpec};
use studio_protocol::{
    GuestMessage, MountTree, NavigationCommand, NodeKind, PROTOCOL_VERSION, ProtocolLimits, UiNode,
    decode_guest_message, validate_guest_message,
};
use studio_ui::{InstanceId, UiRegistry};

struct PreviousNavigation {
    router: ProjectRouter,
    registry: Option<UiRegistry>,
    schedule: TimerSchedule,
}

/// Browser host with authoritative Rust routing and retained-tree admission.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
pub struct WebHost {
    router: ProjectRouter,
    timers: TimerManifest,
    owner: InstanceId,
    registry: Option<UiRegistry>,
    schedule: TimerSchedule,
    now: Duration,
    previous: Option<PreviousNavigation>,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen::prelude::wasm_bindgen)]
impl WebHost {
    /// Admit closed project metadata and select the initial concrete route.
    ///
    /// # Errors
    /// Rejects invalid metadata, unknown timer pages, unknown routes and unsafe clocks.
    #[cfg_attr(
        target_arch = "wasm32",
        wasm_bindgen::prelude::wasm_bindgen(constructor)
    )]
    pub fn new(
        routes_json: &str,
        timers_json: &str,
        initial_route: &str,
        now_ms: f64,
    ) -> Result<Self, String> {
        let now = checked_time(now_ms)?;
        let declaration = RoutingManifest::from_bytes(routes_json.as_bytes())?;
        let timers = TimerManifest::from_bytes(timers_json.as_bytes())?;
        for page in &timers.pages {
            if !declaration.routes.iter().any(|route| route.id == page.id) {
                return Err("timer metadata names an undeclared page".to_owned());
            }
        }
        let router = ProjectRouter::new(&declaration, initial_route)?;
        let schedule = TimerSchedule::new(page_specs(&timers, &router.current().id), now)?;
        Ok(Self {
            router,
            timers,
            owner: InstanceId::new("web-host").map_err(|error| error.to_string())?,
            registry: None,
            schedule,
            now,
            previous: None,
        })
    }

    /// Return the selected concrete route, page identity and parameters as JSON.
    ///
    /// # Errors
    /// Returns a serialization error if a selection cannot be encoded.
    pub fn current(&self) -> Result<String, String> {
        selection_json(&self.router.current())
    }

    /// Stage an admitted route/history change and fresh page timer intervals.
    ///
    /// The worker must deliver an accepted navigation event and admit the resulting
    /// mount. If guest execution fails, [`Self::cancel_navigation`] restores the
    /// preceding tree, history and timer deadlines. Input is blocked while pending.
    ///
    /// # Errors
    /// Rejects malformed commands, unsafe clocks, unresolved routes and overlapping
    /// navigation transactions without changing the selected page.
    pub fn navigate(&mut self, command_json: &str, now_ms: f64) -> Result<String, String> {
        let now = self.now.max(checked_time(now_ms)?);
        if self.previous.is_some() {
            return Err("navigation is awaiting its admitted page mount".to_owned());
        }
        if command_json.len() > ProtocolLimits::default().max_message_bytes {
            return Err("navigation command exceeds the message budget".to_owned());
        }
        let command: NavigationCommand =
            serde_json::from_str(command_json).map_err(|error| error.to_string())?;
        let mut router = self.router.clone();
        let selection = router.navigate(&command)?;
        let schedule = TimerSchedule::new(page_specs(&self.timers, &selection.id), now)?;
        let encoded = selection_json(&selection)?;
        self.previous = Some(PreviousNavigation {
            router: std::mem::replace(&mut self.router, router),
            registry: self.registry.take(),
            schedule: std::mem::replace(&mut self.schedule, schedule),
        });
        self.now = now;
        Ok(encoded)
    }

    /// Restore a pending navigation after failed guest execution or page admission.
    /// Returns whether a transaction was restored. Host time never moves backward.
    pub fn cancel_navigation(&mut self) -> bool {
        let Some(previous) = self.previous.take() else {
            return false;
        };
        self.router = previous.router;
        self.registry = previous.registry;
        self.schedule = previous.schedule;
        true
    }

    /// Validate and atomically admit one guest mount or patch, returning root JSON.
    ///
    /// # Errors
    /// Rejects invalid protocol messages, unauthorized roots/routes, unsupported web
    /// kinds, stale sequences and broken timer ownership while preserving the tree.
    pub fn accept(&mut self, message_json: &str) -> Result<String, String> {
        let limits = ProtocolLimits::default();
        let message = decode_guest_message(message_json.as_bytes(), limits)
            .map_err(|error| error.to_string())?;
        let mut candidate = UiRegistry::new(self.owner.clone(), limits);
        let is_mount = matches!(&message, GuestMessage::Mount(_));
        match message {
            GuestMessage::Mount(mount) => {
                self.validate_mount_owner(&mount)?;
                candidate.mount(mount).map_err(|error| error.to_string())?;
            }
            GuestMessage::Patch(batch) => {
                let registry = self.registry.as_ref().ok_or("page is not mounted")?;
                if registry
                    .last_sequence()
                    .is_some_and(|sequence| batch.sequence <= sequence)
                {
                    return Err("patch sequence must increase".to_owned());
                }
                candidate
                    .mount(self.retained_mount(registry)?)
                    .map_err(|error| error.to_string())?;
                candidate
                    .apply_patch(&self.owner, batch)
                    .map_err(|error| error.to_string())?;
            }
            _ => return Err("tree admission accepts only mount and patch messages".to_owned()),
        }
        let mount = self.retained_mount(&candidate)?;
        self.validate_mount_owner(&mount)?;
        validate_guest_message(&GuestMessage::Mount(mount.clone()), limits)
            .map_err(|error| error.to_string())?;
        validate_web_kinds(&mount.root)?;
        self.validate_timers(&candidate)?;
        let encoded = serde_json::to_string(&mount.root).map_err(|error| error.to_string())?;
        if encoded.len() > limits.max_mount_bytes {
            return Err("retained tree exceeds the web mount budget".to_owned());
        }
        self.registry = Some(candidate);
        if is_mount {
            self.previous = None;
        }
        Ok(encoded)
    }

    /// Return the committed root tree as JSON.
    ///
    /// # Errors
    /// Returns an error before an admitted mount or while navigation is pending.
    pub fn tree(&self) -> Result<String, String> {
        let registry = self.registry.as_ref().ok_or("page is not mounted")?;
        serde_json::to_string(&self.retained_mount(registry)?.root)
            .map_err(|error| error.to_string())
    }

    /// Admit a pressed event only for a visible, enabled active-page Button.
    /// Button events use the existing default pressed contract; guest code owns its handler.
    #[must_use]
    pub fn can_input(&self, node_id: &str, event: &str) -> bool {
        if !self.enabled_button(node_id, event) {
            return false;
        }
        let Some(registry) = &self.registry else {
            return false;
        };
        let mut current = Some(node_id);
        while let Some(id) = current {
            let Ok(node) = registry.get(&self.owner, id) else {
                return false;
            };
            if node.props.get("visible") == Some(&Value::Bool(false)) {
                return false;
            }
            let Ok(parent) = registry.parent_id(&self.owner, id) else {
                return false;
            };
            current = parent;
        }
        true
    }

    /// Admit a compiler-declared internal timer Button despite its invisible wrapper.
    /// Only a due event returned by [`Self::poll_one`] may use this internal path.
    #[must_use]
    pub fn timer_allowed(&self, node_id: &str, event: &str) -> bool {
        self.enabled_button(node_id, event)
            && self
                .active_specs()
                .iter()
                .any(|spec| spec.tick_node == node_id || spec.complete_node == node_id)
    }

    /// Observe checked timer completion/reset states without consuming due events.
    ///
    /// # Errors
    /// Rejects unsafe clocks and malformed, duplicate, unknown or forged status IDs.
    pub fn observe(&mut self, now_ms: f64, completed_json: &str) -> Result<(), String> {
        let now = self.now.max(checked_time(now_ms)?);
        let completed = self.completed(completed_json)?;
        self.schedule.observe(now, &completed);
        self.now = now;
        Ok(())
    }

    /// Return JSON null or one due `{node_id,event}` event in deterministic order.
    /// Hosts must dispatch and observe each result before asking for another, and
    /// bound each poll loop by `studio_navigation::timers::MAX_TIMER_EVENTS`.
    ///
    /// # Errors
    /// Rejects unsafe clocks and malformed or forged completion state atomically.
    pub fn poll_one(&mut self, now_ms: f64, completed_json: &str) -> Result<String, String> {
        let now = self.now.max(checked_time(now_ms)?);
        let completed = self.completed(completed_json)?;
        self.now = now;
        if self.registry.is_none() {
            return Ok("null".to_owned());
        }
        let event = self.schedule.poll_one(now, &completed);
        Ok(event.map_or_else(
            || "null".to_owned(),
            |event| json!({"node_id":event.node_id,"event":event.event}).to_string(),
        ))
    }
}

impl WebHost {
    fn active_specs(&self) -> &[TimerSpec] {
        page_specs(&self.timers, &self.router.current().id)
    }

    fn validate_mount_owner(&self, mount: &MountTree) -> Result<(), String> {
        let selection = self.router.current();
        if mount.route != selection.route || mount.root.id != format!("{}#root", selection.id) {
            return Err("mount does not belong to the host-selected page and route".to_owned());
        }
        Ok(())
    }

    fn retained_mount(&self, registry: &UiRegistry) -> Result<MountTree, String> {
        let root_id = registry.root_id().ok_or("page is not mounted")?;
        Ok(MountTree {
            protocol_version: PROTOCOL_VERSION,
            route: self.router.current().route,
            root: retained_tree(registry, &self.owner, root_id)?,
        })
    }

    fn validate_timers(&self, registry: &UiRegistry) -> Result<(), String> {
        for spec in self.active_specs() {
            let wrapper = registry
                .get(&self.owner, &spec.id)
                .map_err(|error| error.to_string())?;
            if wrapper.kind != NodeKind::Box
                || wrapper.props.get("visible") != Some(&Value::Bool(false))
            {
                return Err("timer wrapper must be an invisible Box".to_owned());
            }
            for (id, kind) in [
                (&spec.tick_node, NodeKind::Button),
                (&spec.complete_node, NodeKind::Button),
                (&spec.status_node, NodeKind::Text),
            ] {
                let node = registry
                    .get(&self.owner, id)
                    .map_err(|error| error.to_string())?;
                if node.kind != kind || !descends_from(registry, &self.owner, id, &spec.id)? {
                    return Err("timer target kind or wrapper ownership is invalid".to_owned());
                }
                if kind == NodeKind::Text
                    && !matches!(
                        node.props.get("text").and_then(Value::as_str),
                        Some("running" | "complete")
                    )
                {
                    return Err("timer status must project running or complete".to_owned());
                }
            }
        }
        Ok(())
    }

    fn enabled_button(&self, node_id: &str, event: &str) -> bool {
        event == "pressed"
            && self.registry.as_ref().is_some_and(|registry| {
                registry.get(&self.owner, node_id).is_ok_and(|node| {
                    node.kind == NodeKind::Button
                        && node.props.get("enabled") != Some(&Value::Bool(false))
                })
            })
    }

    fn completed(&self, encoded: &str) -> Result<BTreeSet<String>, String> {
        if encoded.len() > ProtocolLimits::default().max_message_bytes {
            return Err("timer completion state exceeds the message budget".to_owned());
        }
        let ids: Vec<String> = serde_json::from_str(encoded).map_err(|error| error.to_string())?;
        if ids.len() > MAX_PAGE_TIMERS {
            return Err("too many completed timer IDs".to_owned());
        }
        let completed: BTreeSet<String> = ids.iter().cloned().collect();
        if completed.len() != ids.len() {
            return Err("duplicate completed timer IDs".to_owned());
        }
        let mut actual = BTreeSet::new();
        if let Some(registry) = &self.registry {
            for spec in self.active_specs() {
                let status = registry
                    .get(&self.owner, &spec.status_node)
                    .map_err(|error| error.to_string())?;
                if status.props.get("text").and_then(Value::as_str) == Some("complete") {
                    actual.insert(spec.id.clone());
                }
            }
        }
        if completed != actual {
            return Err("completion IDs must match admitted timer status nodes".to_owned());
        }
        Ok(completed)
    }
}

fn page_specs<'a>(timers: &'a TimerManifest, page_id: &str) -> &'a [TimerSpec] {
    timers
        .pages
        .iter()
        .find(|page| page.id == page_id)
        .map_or(&[], |page| page.timers.as_slice())
}

fn selection_json(selection: &RouteSelection) -> Result<String, String> {
    serde_json::to_string(
        &json!({"route":selection.route,"id":selection.id,"params":selection.params}),
    )
    .map_err(|error| error.to_string())
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "finite nonnegative safe integer milliseconds are checked before conversion"
)]
fn checked_time(now_ms: f64) -> Result<Duration, String> {
    if !now_ms.is_finite()
        || !(0.0..=9_007_199_254_740_991.0).contains(&now_ms)
        || now_ms.fract() != 0.0
    {
        return Err("host clock must be nonnegative safe integer milliseconds".to_owned());
    }
    Ok(Duration::from_millis(now_ms as u64))
}

fn retained_tree(registry: &UiRegistry, owner: &InstanceId, id: &str) -> Result<UiNode, String> {
    let node = registry.get(owner, id).map_err(|error| error.to_string())?;
    Ok(UiNode {
        id: node.id.clone(),
        kind: node.kind,
        props: node.props.clone(),
        children: node
            .children
            .iter()
            .map(|id| retained_tree(registry, owner, id))
            .collect::<Result<_, _>>()?,
    })
}

fn validate_web_kinds(root: &UiNode) -> Result<(), String> {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if !matches!(
            node.kind,
            NodeKind::Box
                | NodeKind::Scaffold
                | NodeKind::Column
                | NodeKind::Row
                | NodeKind::Text
                | NodeKind::Image
                | NodeKind::Button
                | NodeKind::Tag
        ) {
            return Err(format!("unsupported web node kind: {:?}", node.kind));
        }
        pending.extend(node.children.iter());
    }
    Ok(())
}

fn descends_from(
    registry: &UiRegistry,
    owner: &InstanceId,
    id: &str,
    ancestor: &str,
) -> Result<bool, String> {
    let mut parent = registry
        .parent_id(owner, id)
        .map_err(|error| error.to_string())?;
    while let Some(id) = parent {
        if id == ancestor {
            return Ok(true);
        }
        parent = registry
            .parent_id(owner, id)
            .map_err(|error| error.to_string())?;
    }
    Ok(false)
}
