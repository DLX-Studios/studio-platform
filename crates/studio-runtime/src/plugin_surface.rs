//! Prepared mounted plugin surface retained for the complete launch lifetime.

use std::collections::BTreeSet;
#[cfg(feature = "services")]
use std::sync::Arc;
use std::sync::mpsc;
#[cfg(feature = "services")]
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
#[cfg(any(feature = "services", test))]
use serde_json::json;
use studio_components::{
    ComponentCatalog, DispatchError, HostEventDispatcher, InputAction, NativeStateStore,
    RuntimeControl, UpdateError, UpdateReport,
};
#[cfg(feature = "services")]
use studio_github::GithubClient;
use studio_navigation::project::{ProjectRouter, ROUTING_ASSET, RouteSelection, RoutingManifest};
use studio_navigation::timers::{
    MAX_TIMER_EVENTS, TIMER_ASSET, TimerManifest, TimerSchedule, TimerSpec,
};
#[cfg(feature = "services")]
use studio_net::RestBroker;
#[cfg(feature = "services")]
use studio_oauth::OAuthManager;
use studio_package::ProviderAdmissionPlan;
use studio_protocol::{
    ActionRequest, ActionResult, GuestMessage, HostEvent, MountTree, NavigationCommand,
    NavigationEvent, ProtocolError, ProtocolLimits, UiNode, decode_guest_message,
};
use studio_ui::{PatchError, PatchMetrics, UiRegistry};
use studio_wasm::{PluginInstance, RuntimeError};
use thiserror::Error;

use crate::cli::LaunchMode;
#[cfg(feature = "services")]
use crate::host::is_oauth_provider;

/// Persistent host-owned indicator required for explicitly untrusted development launches.
pub const DEVELOPMENT_WARNING: &str = "Development mode: unsigned plugin bundle";

use studio_components::native_view::{NativeRenderNode as PluginRenderNode, NativeSurface};

/// Fully validated, instantiated, and atomically mounted plugin surface.
#[derive(Debug)]
pub struct PluginSurface {
    mode: LaunchMode,
    warning: Option<&'static str>,
    registry: UiRegistry,
    native_state: NativeStateStore,
    dispatcher: HostEventDispatcher,
    instance: PluginInstance,
    assets: std::collections::BTreeMap<String, Vec<u8>>,
    protocol_limits: ProtocolLimits,
    provider_plan: ProviderAdmissionPlan,
    #[cfg(feature = "services")]
    oauth_manager: Option<Arc<OAuthManager>>,
    #[cfg(feature = "services")]
    rest_broker: Option<Arc<RestBroker<'static>>>,
    action_sender: mpsc::Sender<studio_protocol::ActionResult>,
    action_receiver: mpsc::Receiver<studio_protocol::ActionResult>,
    guest_event_calls: u64,
    guest_patch_messages: u64,
    routing: Option<ProjectRouter>,
    timer_manifest: Option<TimerManifest>,
    timer_schedule: Option<TimerSchedule>,
    timer_origin: Instant,
    timer_now: Duration,
    timer_generation: u64,
}

impl PluginSurface {
    pub(crate) fn new(
        mode: LaunchMode,
        registry: UiRegistry,
        native_state: NativeStateStore,
        dispatcher: HostEventDispatcher,
        instance: PluginInstance,
        assets: std::collections::BTreeMap<String, Vec<u8>>,
        protocol_limits: ProtocolLimits,
        provider_plan: ProviderAdmissionPlan,
        provider_services: crate::host::ProviderServices,
        routing: Option<ProjectRouter>,
    ) -> Self {
        let (action_sender, action_receiver) = mpsc::channel();
        #[cfg(not(feature = "services"))]
        let _ = provider_services;
        #[cfg(feature = "services")]
        let (oauth_manager, rest_broker) = provider_services.map_or((None, None), |services| {
            (Some(services.0), Some(services.1))
        });
        Self {
            mode,
            warning: (mode == LaunchMode::Development).then_some(DEVELOPMENT_WARNING),
            registry,
            native_state,
            dispatcher,
            instance,
            assets,
            protocol_limits,
            provider_plan,
            #[cfg(feature = "services")]
            oauth_manager,
            #[cfg(feature = "services")]
            rest_broker,
            action_sender,
            action_receiver,
            guest_event_calls: 0,
            guest_patch_messages: 0,
            routing,
            timer_manifest: None,
            timer_schedule: None,
            timer_origin: Instant::now(),
            timer_now: Duration::ZERO,
            timer_generation: 0,
        }
    }

    /// Capture the SDK's opt-in state document for a development module replacement.
    ///
    /// # Errors
    /// Returns a guest failure if its snapshot hook violates the bounded ABI.
    #[cfg(feature = "development")]
    pub fn snapshot_development_state(&mut self) -> Result<Option<Vec<u8>>, SurfaceError> {
        self.instance
            .snapshot_development_state()
            .map_err(SurfaceError::from)
    }

    /// Refresh admitted asset bytes without restarting an unchanged guest.
    #[cfg(feature = "development")]
    pub fn replace_development_assets(
        &mut self,
        assets: std::collections::BTreeMap<String, Vec<u8>>,
    ) {
        self.assets = assets;
    }

    /// Returns bytes for an asset admitted with the mounted bundle.
    #[must_use]
    pub(crate) fn asset(&self, path: &str) -> Option<&[u8]> {
        self.assets.get(path).map(Vec::as_slice)
    }

    /// Active trust mode.
    #[must_use]
    pub const fn mode(&self) -> LaunchMode {
        self.mode
    }

    /// Persistent host-owned warning for development mode.
    #[must_use]
    pub const fn warning(&self) -> Option<&'static str> {
        self.warning
    }

    /// Resolved provider capability plan admitted before guest startup.
    #[must_use]
    pub const fn provider_plan(&self) -> &ProviderAdmissionPlan {
        &self.provider_plan
    }

    /// Atomically committed retained tree.
    #[must_use]
    pub const fn registry(&self) -> &UiRegistry {
        &self.registry
    }

    /// Dispatch a native action from this surface's host-owned instance context.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError`] for an unknown target or kind-incompatible native action.
    pub fn dispatch_input(
        &self,
        node_id: &str,
        action: InputAction,
    ) -> Result<HostEvent, DispatchError> {
        self.dispatcher
            .dispatch(self.dispatcher_owner(), node_id, action)
    }

    /// Deliver a native input event, then atomically apply the guest's targeted patch output.
    ///
    /// # Errors
    ///
    /// Returns a stable surface error for dispatch, serialization, guest, protocol, patch, or
    /// native reconciliation failures.
    pub fn process_input(
        &mut self,
        node_id: &str,
        action: InputAction,
    ) -> Result<UpdateReport, SurfaceError> {
        let owner = self.dispatcher_owner().clone();
        let event = self.dispatcher.dispatch(&owner, node_id, action.clone())?;
        if let InputAction::TextChanged { value } = action {
            self.native_state.set_input_buffer(&owner, node_id, value)?;
        }
        let encoded = serde_json::to_vec(&event)
            .map_err(|error| SurfaceError::Serialization(error.to_string()))?;
        self.guest_event_calls = self.guest_event_calls.saturating_add(1);
        let outcome = self.instance.invoke_event_bytes(&encoded)?;
        self.apply_guest_emissions(outcome.emissions)
    }

    fn apply_guest_emissions(
        &mut self,
        emissions: Vec<Vec<u8>>,
    ) -> Result<UpdateReport, SurfaceError> {
        let owner = self.dispatcher_owner().clone();
        let mut invalidated_nodes = Vec::new();
        let emission_count = emissions.len();
        for emission in emissions {
            match decode_guest_message(&emission, self.protocol_limits)? {
                GuestMessage::Patch(batch) => {
                    let structural = batch.operations.iter().any(|operation| {
                        !matches!(operation, studio_protocol::PatchOp::UpdateProp { .. })
                    });
                    self.guest_patch_messages = self.guest_patch_messages.saturating_add(1);
                    let commit = self.registry.apply_patch(&owner, batch)?;
                    let report = self
                        .native_state
                        .apply_commit(&owner, &self.registry, &commit)?;
                    if structural {
                        self.dispatcher = dispatcher_from_registry(&owner, &self.registry)?;
                    }
                    for node_id in report.invalidated_nodes {
                        if !invalidated_nodes.contains(&node_id) {
                            invalidated_nodes.push(node_id);
                        }
                    }
                }
                GuestMessage::Action(action) => self.queue_action(action),
                GuestMessage::Navigate(command) if emission_count == 1 => {
                    return self.navigate(&command);
                }
                _ => return Err(SurfaceError::UnexpectedGuestMessage),
            }
        }
        Ok(UpdateReport { invalidated_nodes })
    }

    fn navigate(&mut self, command: &NavigationCommand) -> Result<UpdateReport, SurfaceError> {
        let mut candidate = self.routing.clone().ok_or_else(|| {
            SurfaceError::Navigation("the package does not declare routed pages".to_owned())
        })?;
        let selection = match candidate.navigate(command) {
            Ok(selection) => selection,
            Err(_) => {
                let event = HostEvent::Navigation(NavigationEvent {
                    route: candidate.current().route,
                    accepted: false,
                    error_code: Some("navigation_denied".to_owned()),
                });
                let encoded = serde_json::to_vec(&event)
                    .map_err(|error| SurfaceError::Serialization(error.to_string()))?;
                self.guest_event_calls = self.guest_event_calls.saturating_add(1);
                let outcome = self.instance.invoke_event_bytes(&encoded)?;
                if !outcome.emissions.is_empty() {
                    return Err(SurfaceError::UnexpectedGuestMessage);
                }
                return Ok(UpdateReport {
                    invalidated_nodes: Vec::new(),
                });
            }
        };
        let event = HostEvent::Navigation(NavigationEvent {
            route: selection.route.clone(),
            accepted: true,
            error_code: None,
        });
        let encoded = serde_json::to_vec(&event)
            .map_err(|error| SurfaceError::Serialization(error.to_string()))?;
        self.guest_event_calls = self.guest_event_calls.saturating_add(1);
        let outcome = self.instance.invoke_event_bytes(&encoded)?;
        let [bytes] = outcome.emissions.as_slice() else {
            return Err(SurfaceError::UnexpectedGuestMessage);
        };
        let GuestMessage::Mount(mount) = decode_guest_message(bytes, self.protocol_limits)? else {
            return Err(SurfaceError::UnexpectedGuestMessage);
        };
        validate_routed_mount(&selection, &mount).map_err(SurfaceError::Navigation)?;
        let owner = self.dispatcher_owner().clone();
        let mut dispatcher = HostEventDispatcher::new(owner.clone());
        let mut pending = vec![&mount.root];
        let mut invalidated_nodes = Vec::new();
        while let Some(node) = pending.pop() {
            dispatcher.register(&node.id, node.kind)?;
            invalidated_nodes.push(node.id.clone());
            pending.extend(node.children.iter());
        }
        let mut registry = UiRegistry::new(owner.clone(), self.protocol_limits);
        registry
            .mount(mount)
            .map_err(|error| SurfaceError::Navigation(error.to_string()))?;
        let native_state = NativeStateStore::from_registry(&owner, &registry)?;
        let timer_now = self.timer_now.max(self.timer_origin.elapsed());
        let timer_schedule = self
            .timer_manifest
            .as_ref()
            .map(|manifest| {
                timer_schedule_for_registry(manifest, &selection.id, &owner, &registry, timer_now)
            })
            .transpose()
            .map_err(SurfaceError::Navigation)?
            .flatten();
        self.registry = registry;
        self.native_state = native_state;
        self.dispatcher = dispatcher;
        self.routing = Some(candidate);
        self.timer_schedule = timer_schedule;
        self.timer_now = timer_now;
        self.timer_generation = self.timer_generation.saturating_add(1);
        Ok(UpdateReport { invalidated_nodes })
    }

    pub(crate) fn configure_timers(&mut self, manifest: TimerManifest) -> Result<(), SurfaceError> {
        validate_timer_manifest_routes(&manifest, &self.assets, self.routing.as_ref())
            .map_err(SurfaceError::Navigation)?;
        let page = self
            .routing
            .as_ref()
            .ok_or_else(|| {
                SurfaceError::Navigation("timers require declared project routes".to_owned())
            })?
            .current()
            .id;
        let schedule = timer_schedule_for_registry(
            &manifest,
            &page,
            self.dispatcher_owner(),
            &self.registry,
            self.timer_now,
        )
        .map_err(SurfaceError::Navigation)?;
        self.timer_manifest = Some(manifest);
        self.timer_schedule = schedule;
        self.timer_generation = self.timer_generation.saturating_add(1);
        Ok(())
    }

    fn current_timer_specs(&self) -> &[TimerSpec] {
        let Some(router) = &self.routing else {
            return &[];
        };
        let selection = router.current();
        self.timer_manifest
            .as_ref()
            .and_then(|manifest| manifest.pages.iter().find(|page| page.id == selection.id))
            .map_or(&[], |page| page.timers.as_slice())
    }

    fn completed_timers(&self) -> Result<BTreeSet<String>, SurfaceError> {
        let mut completed = BTreeSet::new();
        for timer in self.current_timer_specs() {
            let status = self
                .registry
                .get(self.dispatcher_owner(), &timer.status_node)
                .map_err(|error| SurfaceError::Navigation(error.to_string()))?;
            match status.props.get("text").and_then(Value::as_str) {
                Some("complete") => {
                    completed.insert(timer.id.clone());
                }
                Some("running") => {}
                _ => {
                    return Err(SurfaceError::Navigation(
                        "timer status must project running or complete".to_owned(),
                    ));
                }
            }
        }
        Ok(completed)
    }

    fn poll_timers_at(&mut self, now: Duration) -> Result<bool, SurfaceError> {
        self.timer_now = self.timer_now.max(now);
        if self.timer_schedule.is_none() {
            return Ok(false);
        }
        let generation = self.timer_generation;
        let mut changed = false;
        for _ in 0..MAX_TIMER_EVENTS {
            if generation != self.timer_generation {
                break;
            }
            let completed = self.completed_timers()?;
            let event = self
                .timer_schedule
                .as_mut()
                .and_then(|schedule| schedule.poll_one(self.timer_now, &completed));
            let Some(event) = event else {
                break;
            };
            let Some(timer) = self
                .current_timer_specs()
                .iter()
                .find(|timer| {
                    timer.tick_node == event.node_id || timer.complete_node == event.node_id
                })
                .cloned()
            else {
                return Err(SurfaceError::Navigation(
                    "timer event does not belong to the active page".to_owned(),
                ));
            };
            if event.node_id == timer.tick_node && completed.contains(&timer.id) {
                continue;
            }
            if event.event != "pressed" {
                return Err(SurfaceError::Navigation(
                    "timer event must be pressed".to_owned(),
                ));
            }
            self.process_input(&event.node_id, InputAction::PointerClick)?;
            changed = true;
            if generation != self.timer_generation {
                break;
            }
            let completed = self.completed_timers()?;
            if let Some(schedule) = &mut self.timer_schedule {
                schedule.observe(self.timer_now, &completed);
            }
        }
        Ok(changed)
    }

    #[cfg(feature = "services")]
    fn queue_action(&self, request: ActionRequest) {
        let sender = self.action_sender.clone();
        let oauth_manager = self.oauth_manager.clone();
        let rest_broker = self.rest_broker.clone();
        let action_id = request.request_id.clone();
        if thread::Builder::new()
            .name(String::from("studio-provider-action"))
            .spawn(move || {
                let result = run_provider_action(request, oauth_manager, rest_broker);
                let _ = sender.send(result);
            })
            .is_err()
        {
            let _ = self.action_sender.send(ActionResult::Failure {
                request_id: action_id,
                code: String::from("host_action_unavailable"),
                message: String::from("Host action could not start"),
                retryable: true,
            });
        }
    }

    #[cfg(not(feature = "services"))]
    fn queue_action(&self, request: ActionRequest) {
        let _ = self.action_sender.send(ActionResult::Failure {
            request_id: request.request_id,
            code: "capability_denied".into(),
            message: "This runtime was built without host service modules".into(),
            retryable: false,
        });
    }

    /// Deliver completed host actions to the guest on the UI thread.
    pub fn poll_action_results(&mut self) -> Result<bool, SurfaceError> {
        let mut changed = false;
        while let Ok(result) = self.action_receiver.try_recv() {
            let event = HostEvent::ActionResult(result);
            let encoded = serde_json::to_vec(&event)
                .map_err(|error| SurfaceError::Serialization(error.to_string()))?;
            self.guest_event_calls = self.guest_event_calls.saturating_add(1);
            let outcome = self.instance.invoke_event_bytes(&encoded)?;
            let report = self.apply_guest_emissions(outcome.emissions)?;
            changed |= !report.invalidated_nodes.is_empty();
        }
        changed |= self.poll_timers_at(self.timer_origin.elapsed())?;
        Ok(changed)
    }

    /// Focus one native node.
    ///
    /// # Errors
    ///
    /// Returns an update error for an unknown node.
    pub fn focus(&mut self, node_id: &str) -> Result<(), UpdateError> {
        let owner = self.dispatcher_owner().clone();
        self.native_state.focus(&owner, node_id)
    }

    /// Set retained scroll state on a native scroll container.
    ///
    /// # Errors
    ///
    /// Returns an update error for an unknown or incompatible node.
    pub fn set_scroll_offset(&mut self, node_id: &str, offset: f32) -> Result<(), UpdateError> {
        let owner = self.dispatcher_owner().clone();
        self.native_state.set_scroll_offset(&owner, node_id, offset)
    }

    /// Borrow a current retained native property.
    ///
    /// # Errors
    ///
    /// Returns an update error for an unknown node or property.
    pub fn property(&self, node_id: &str, property: &str) -> Result<&Value, UpdateError> {
        self.native_state
            .property(self.dispatcher_owner(), node_id, property)
    }

    /// Return the opaque native identity for a stable node ID.
    ///
    /// # Errors
    ///
    /// Returns an update error for an unknown node.
    pub fn native_identity(&self, node_id: &str) -> Result<u64, UpdateError> {
        self.native_state
            .native_identity(self.dispatcher_owner(), node_id)
    }

    /// Return the focused stable node ID.
    ///
    /// # Errors
    ///
    /// Returns an ownership error if surface state is inconsistent.
    pub fn focused_id(&self) -> Result<Option<&str>, UpdateError> {
        self.native_state.focused_id(self.dispatcher_owner())
    }

    /// Return retained scroll state for a native node.
    ///
    /// # Errors
    ///
    /// Returns an update error for an unknown node.
    pub fn scroll_offset(&self, node_id: &str) -> Result<f32, UpdateError> {
        self.native_state
            .scroll_offset(self.dispatcher_owner(), node_id)
    }

    /// Return retained non-secret input state for a native node.
    ///
    /// # Errors
    ///
    /// Returns an update error for an unknown node.
    pub fn input_buffer(&self, node_id: &str) -> Result<&str, UpdateError> {
        self.native_state
            .input_buffer(self.dispatcher_owner(), node_id)
    }

    /// Return monotonic guest and retained-patch instrumentation.
    #[must_use]
    pub const fn metrics(&self) -> SurfaceMetrics {
        SurfaceMetrics {
            guest_event_calls: self.guest_event_calls,
            guest_patch_messages: self.guest_patch_messages,
            patches: self.registry.patch_metrics(),
        }
    }

    /// Accessibility labels in the native keyboard-focusable component catalog.
    #[must_use]
    pub fn keyboard_focusable_labels(&self) -> Vec<String> {
        self.keyboard_focus_order()
            .into_iter()
            .map(|(_, label)| label)
            .collect()
    }

    /// Stable preorder keyboard traversal with meaningful host accessibility labels.
    #[must_use]
    pub fn keyboard_focus_order(&self) -> Vec<(String, String)> {
        let mut order = Vec::new();
        if let Some(root) = self.render_tree() {
            collect_focusable(&root, &mut order);
        }
        order
    }

    /// Advance native focus through the retained preorder, wrapping at the end.
    ///
    /// # Errors
    ///
    /// Returns an update error only if the retained/native state became inconsistent.
    pub fn focus_next(&mut self) -> Result<Option<&str>, UpdateError> {
        let order = self.keyboard_focus_order();
        if order.is_empty() {
            return Ok(None);
        }
        let current = self.focused_id()?;
        let next = current
            .and_then(|current| order.iter().position(|(id, _)| id == current))
            .map_or(0, |index| (index + 1) % order.len());
        let owner = self.dispatcher_owner().clone();
        self.native_state.focus(&owner, &order[next].0)?;
        self.focused_id()
    }

    /// Clone the currently committed retained hierarchy for one native render pass.
    pub(crate) fn render_tree(&self) -> Option<PluginRenderNode> {
        let root = self.registry.root_id()?;
        self.render_node(root)
    }

    fn render_node(&self, node_id: &str) -> Option<PluginRenderNode> {
        let node = self.registry.get(self.dispatcher_owner(), node_id).ok()?;
        Some(PluginRenderNode {
            id: node.id.clone(),
            kind: node.kind,
            control: RuntimeControl::from_node_kind(node.kind),
            props: node.props.clone(),
            children: node
                .children
                .iter()
                .filter_map(|child| self.render_node(child))
                .collect(),
        })
    }

    fn dispatcher_owner(&self) -> &studio_ui::InstanceId {
        self.dispatcher.owner()
    }

    /// Stable host-owned identifier that owns this mounted surface.
    #[must_use]
    pub fn owner_id(&self) -> &studio_ui::InstanceId {
        self.dispatcher.owner()
    }

    /// Mount route of the committed tree, for reload route resolution.
    #[must_use]
    pub fn mount_route(&self) -> Option<String> {
        self.registry
            .snapshot(self.owner_id())
            .ok()
            .map(|snapshot| snapshot.route().to_owned())
    }

    /// Build a host-owned checkout shell bound to this mounted surface's principal.
    ///
    /// # Errors
    ///
    /// Rejects invalid checkout or navigation initialization while retaining the
    /// existing mounted UI state.
    #[cfg(feature = "services")]
    pub fn checkout_shell(
        &self,
        checkout: studio_actions::Checkout,
        reduced_motion: bool,
    ) -> Result<crate::NativeCheckoutShell, crate::NativeCheckoutError> {
        let owner = self.dispatcher.owner().clone();
        // Derive a stable 16-byte instance id from the verified owner string.
        let mut instance_bytes = [0u8; 16];
        for (index, byte) in owner.as_str().bytes().enumerate() {
            instance_bytes[index % 16] = instance_bytes[index % 16].wrapping_add(byte);
        }
        if instance_bytes == [0; 16] {
            instance_bytes[0] = 1;
        }
        let trust = match self.mode {
            crate::cli::LaunchMode::Production => studio_security::TrustMode::Production,
            crate::cli::LaunchMode::Development => studio_security::TrustMode::Development,
        };
        let principal = studio_security::PluginPrincipal::new(
            owner.as_str(),
            owner.as_str(),
            [7u8; 32],
            instance_bytes,
            trust,
        )
        .map_err(|_| crate::NativeCheckoutError::StateInvalid)?;
        crate::NativeCheckoutShell::new(owner, principal, checkout, reduced_motion)
    }
}

/// Resolve the provider a host action capability names.
///
/// The host exposes one capability per provider, named `{provider}.oauth`. A capability that
/// names no provider the host can sign in with is not a capability this host offers: a
/// maintained first-party provider always is one, and a declared issuer is one once the launch
/// has enabled it on the manager.
#[cfg(feature = "services")]
fn provider_from_capability<'a>(
    capability: &'a str,
    oauth_manager: Option<&Arc<OAuthManager>>,
) -> Option<&'a str> {
    let provider = capability.strip_suffix(".oauth")?;
    if is_oauth_provider(provider) {
        return Some(provider);
    }
    oauth_manager
        .is_some_and(|manager| manager.supports(provider))
        .then_some(provider)
}

/// Run one provider host action.
///
/// The capability names the provider, the operation is always `sign_in`, and the approved
/// claims of that provider are returned to the guest. GitHub additionally lists the
/// repositories the signed-in account can reach, because that application reads them.
#[cfg(feature = "services")]
fn run_provider_action(
    request: ActionRequest,
    oauth_manager: Option<Arc<OAuthManager>>,
    rest_broker: Option<Arc<RestBroker<'static>>>,
) -> ActionResult {
    let request_id = request.request_id;
    let Some(provider) = provider_from_capability(&request.capability, oauth_manager.as_ref())
    else {
        return ActionResult::Failure {
            request_id,
            code: String::from("capability_denied"),
            message: String::from("Requested host action is not available"),
            retryable: false,
        };
    };
    if request.operation != "sign_in" {
        return ActionResult::Failure {
            request_id,
            code: String::from("capability_denied"),
            message: String::from("Requested host action is not available"),
            retryable: false,
        };
    }
    if let Some(requested) = request.payload.get("provider").and_then(Value::as_str)
        && requested != provider
    {
        return ActionResult::Failure {
            request_id,
            code: String::from("oauth.provider.unknown"),
            message: String::from("Requested provider is unavailable"),
            retryable: false,
        };
    }
    let Some(oauth_manager) = oauth_manager else {
        return ActionResult::Failure {
            request_id,
            code: String::from("oauth.provider.unavailable"),
            message: String::from("Sign-in is unavailable on this host"),
            retryable: false,
        };
    };
    let result = oauth_manager.sign_in(provider);
    if !result.is_success() {
        return ActionResult::Failure {
            request_id,
            code: result.code_str().to_owned(),
            message: String::from("Sign-in failed"),
            retryable: true,
        };
    }
    let Some(claims) = result.claims else {
        return ActionResult::Failure {
            request_id,
            code: String::from("oauth.claims.invalid"),
            message: String::from("Profile could not be verified"),
            retryable: false,
        };
    };
    if provider == studio_oauth::GITHUB_PROVIDER_ID {
        let Some(rest_broker) = rest_broker else {
            return ActionResult::Failure {
                request_id,
                code: String::from("oauth.provider.unavailable"),
                message: String::from("Sign-in is unavailable on this host"),
                retryable: false,
            };
        };
        let api = rest_broker.guest_api();
        let client = GithubClient::new(&api);
        let repositories = match client.repositories() {
            Ok(repositories) => repositories,
            Err(_) => {
                return ActionResult::Failure {
                    request_id,
                    code: String::from("github.repositories.failed"),
                    message: String::from("GitHub repositories could not be loaded"),
                    retryable: true,
                };
            }
        };
        let repository_names = repositories
            .into_iter()
            .map(|repository| repository.full_name)
            .collect::<Vec<_>>();
        let repository_count = repository_names.len();
        return ActionResult::Success {
            request_id,
            payload: json!({
                "login": claims.login,
                "email": claims.email,
                "repositoryNames": repository_names,
                "repositoryCount": repository_count
            }),
        };
    }
    ActionResult::Success {
        request_id,
        payload: json!({
            "provider": provider,
            "subject": claims.subject,
            "login": claims.login,
            "displayName": claims.display_name,
            "avatarUrl": claims.avatar_url,
            "profileUrl": claims.profile_url,
            "email": claims.email
        }),
    }
}

fn collect_focusable(node: &PluginRenderNode, order: &mut Vec<(String, String)>) {
    if node.props.get("visible") == Some(&Value::Bool(false)) {
        return;
    }
    let component = ComponentCatalog::default().map(&UiNode {
        id: node.id.clone(),
        kind: node.kind,
        props: node.props.clone(),
        children: Vec::new(),
    });
    if let Ok(component) = component
        && component.focusable
        && let Some(label) = component.accessibility_label
    {
        order.push((node.id.clone(), label));
    }
    for child in &node.children {
        collect_focusable(child, order);
    }
}

fn dispatcher_from_registry(
    owner: &studio_ui::InstanceId,
    registry: &UiRegistry,
) -> Result<HostEventDispatcher, SurfaceError> {
    let mut dispatcher = HostEventDispatcher::new(owner.clone());
    let mut pending = vec![
        registry
            .root_id()
            .ok_or(SurfaceError::UnexpectedGuestMessage)?,
    ];
    while let Some(id) = pending.pop() {
        let node = registry
            .get(owner, id)
            .map_err(|error| SurfaceError::Navigation(error.to_string()))?;
        dispatcher.register(&node.id, node.kind)?;
        pending.extend(node.children.iter().map(String::as_str));
    }
    Ok(dispatcher)
}

/// Validate the signed page declaration before executing guest initialization.
pub(crate) fn project_router_from_assets(
    assets: &std::collections::BTreeMap<String, Vec<u8>>,
) -> Result<Option<ProjectRouter>, String> {
    let Some(bytes) = assets.get(ROUTING_ASSET) else {
        return Ok(None);
    };
    let manifest = RoutingManifest::from_bytes(bytes)?;
    let initial = manifest.initial_route()?;
    ProjectRouter::new(&manifest, &initial).map(Some)
}

/// Validate signed timer metadata and its page identities before executing guest code.
pub(crate) fn project_timer_manifest_from_assets(
    assets: &std::collections::BTreeMap<String, Vec<u8>>,
    routing: Option<&ProjectRouter>,
) -> Result<Option<TimerManifest>, String> {
    let Some(bytes) = assets.get(TIMER_ASSET) else {
        return Ok(None);
    };
    let manifest = TimerManifest::from_bytes(bytes)?;
    validate_timer_manifest_routes(&manifest, assets, routing)?;
    Ok(Some(manifest))
}

fn validate_timer_manifest_routes(
    manifest: &TimerManifest,
    assets: &std::collections::BTreeMap<String, Vec<u8>>,
    routing: Option<&ProjectRouter>,
) -> Result<(), String> {
    manifest.validate()?;
    if routing.is_none() {
        return Err("timers require declared project routes".to_owned());
    }
    let routes = assets
        .get(ROUTING_ASSET)
        .ok_or("timer package is missing its routing declaration")?;
    let routes = RoutingManifest::from_bytes(routes)?;
    if manifest
        .pages
        .iter()
        .any(|page| !routes.routes.iter().any(|route| route.id == page.id))
    {
        return Err("timer metadata references an undeclared page".to_owned());
    }
    Ok(())
}

fn timer_schedule_for_registry(
    manifest: &TimerManifest,
    page_id: &str,
    owner: &studio_ui::InstanceId,
    registry: &UiRegistry,
    now: Duration,
) -> Result<Option<TimerSchedule>, String> {
    let Some(page) = manifest.pages.iter().find(|page| page.id == page_id) else {
        return Ok(None);
    };
    for timer in &page.timers {
        let wrapper = registry
            .get(owner, &timer.id)
            .map_err(|error| error.to_string())?;
        if wrapper.kind != studio_protocol::NodeKind::Box
            || wrapper.props.get("visible") != Some(&Value::Bool(false))
        {
            return Err("timer wrapper must be an invisible native Box".to_owned());
        }
        let mut descendants = BTreeSet::new();
        let mut pending: Vec<_> = wrapper.children.iter().map(String::as_str).collect();
        while let Some(id) = pending.pop() {
            let node = registry.get(owner, id).map_err(|error| error.to_string())?;
            descendants.insert(id);
            pending.extend(node.children.iter().map(String::as_str));
        }
        for (id, kind) in [
            (&timer.tick_node, studio_protocol::NodeKind::Button),
            (&timer.complete_node, studio_protocol::NodeKind::Button),
            (&timer.status_node, studio_protocol::NodeKind::Text),
        ] {
            if !descendants.contains(id.as_str()) {
                return Err("timer targets must belong to their invisible wrapper".to_owned());
            }
            let node = registry.get(owner, id).map_err(|error| error.to_string())?;
            if node.kind != kind {
                return Err("timer target has the wrong native component kind".to_owned());
            }
            if kind == studio_protocol::NodeKind::Text
                && !matches!(
                    node.props.get("text").and_then(Value::as_str),
                    Some("running" | "complete")
                )
            {
                return Err("timer status must project running or complete".to_owned());
            }
        }
    }
    if page.timers.is_empty() {
        return Ok(None);
    }
    TimerSchedule::new(&page.timers, now).map(Some)
}

/// A guest may mount only the concrete route and page the host just admitted.
pub(crate) fn validate_routed_mount(
    selection: &RouteSelection,
    mount: &MountTree,
) -> Result<(), String> {
    if mount.route != selection.route || mount.root.id != format!("{}#root", selection.id) {
        return Err("guest mount does not match the host-authorized route and page".to_owned());
    }
    Ok(())
}

/// Monotonic mounted-surface work counters used for diagnostics and idle assertions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurfaceMetrics {
    /// Host events delivered into the guest.
    pub guest_event_calls: u64,
    /// Patch envelopes emitted by completed guest event calls.
    pub guest_patch_messages: u64,
    /// Successfully committed retained patch work.
    pub patches: PatchMetrics,
}

/// Stable processing failures after a mounted plugin receives native input.
#[derive(Debug, Error)]
pub enum SurfaceError {
    /// Native event dispatch failed.
    #[error(transparent)]
    Dispatch(#[from] DispatchError),
    /// Host event JSON encoding failed.
    #[error("host event serialization failed: {0}")]
    Serialization(String),
    /// Guest execution failed terminally.
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    /// Guest message validation failed.
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    /// Guest emitted an envelope that is invalid during event processing.
    #[error("guest event call emitted an unexpected message")]
    UnexpectedGuestMessage,
    /// A routed package requested or mounted an inadmissible destination.
    #[error("project navigation failed: {0}")]
    Navigation(String),
    /// Retained transaction validation failed.
    #[error(transparent)]
    Patch(#[from] PatchError),
    /// Native reconciliation failed.
    #[error(transparent)]
    Update(#[from] UpdateError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_protocol::ActionRequest;

    #[test]
    fn structural_patches_refresh_input_targets_and_remove_stale_controls() {
        use std::collections::BTreeMap;
        use studio_protocol::{NodeKind, PatchBatch, PatchOp};
        let owner = studio_ui::InstanceId::new("com.studio.routing-test").unwrap();
        let button = |id: &str| UiNode {
            id: id.to_owned(),
            kind: NodeKind::Button,
            props: BTreeMap::from([("label".to_owned(), json!("Activate"))]),
            children: Vec::new(),
        };
        let root = |child: UiNode| UiNode {
            id: "page-root".to_owned(),
            kind: NodeKind::Box,
            props: BTreeMap::new(),
            children: vec![child],
        };
        let mut registry = UiRegistry::new(owner.clone(), ProtocolLimits::default());
        registry
            .mount(MountTree {
                protocol_version: studio_protocol::PROTOCOL_VERSION,
                route: "/".to_owned(),
                root: root(button("before")),
            })
            .unwrap();
        let before = dispatcher_from_registry(&owner, &registry).unwrap();
        registry
            .apply_patch(
                &owner,
                PatchBatch {
                    sequence: 1,
                    operations: vec![PatchOp::ReplaceNode {
                        node_id: "page-root".to_owned(),
                        node: root(button("after")),
                    }],
                },
            )
            .unwrap();
        assert!(
            before
                .dispatch(&owner, "after", InputAction::PointerClick)
                .is_err()
        );
        let after = dispatcher_from_registry(&owner, &registry).unwrap();
        assert!(
            after
                .dispatch(&owner, "after", InputAction::PointerClick)
                .is_ok()
        );
        assert!(
            after
                .dispatch(&owner, "before", InputAction::PointerClick)
                .is_err()
        );
    }

    fn routed_guest_surface(unauthorized_mount: bool) -> PluginSurface {
        use std::collections::BTreeMap;
        use studio_protocol::NodeKind;
        let mount = |route: &str, page: &str, button: &str| {
            GuestMessage::Mount(MountTree {
                protocol_version: studio_protocol::PROTOCOL_VERSION,
                route: route.to_owned(),
                root: UiNode {
                    id: format!("{page}#root"),
                    kind: NodeKind::Box,
                    props: BTreeMap::new(),
                    children: vec![UiNode {
                        id: button.to_owned(),
                        kind: NodeKind::Button,
                        props: BTreeMap::from([("label".to_owned(), json!(button))]),
                        children: Vec::new(),
                    }],
                },
            })
        };
        let messages = [
            mount("/", "home", "go-live"),
            GuestMessage::Navigate(NavigationCommand::Push {
                route: "/live".to_owned(),
            }),
            mount(
                "/live",
                if unauthorized_mount { "home" } else { "live" },
                "back",
            ),
            GuestMessage::Navigate(NavigationCommand::Pop),
            mount("/", "home", "go-live"),
        ];
        let documents: Vec<_> = messages
            .iter()
            .map(|message| serde_json::to_string(message).unwrap())
            .collect();
        fixture_surface(&documents, routed_assets())
    }

    fn fixture_surface(
        documents: &[String],
        assets: std::collections::BTreeMap<String, Vec<u8>>,
    ) -> PluginSurface {
        let data = documents
            .iter()
            .enumerate()
            .map(|(index, document)| {
                format!(
                    "(data (i32.const {}) {})",
                    index * 1024,
                    serde_json::to_string(document).unwrap()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut branches = "i32.const 0".to_owned();
        for index in (1..documents.len()).rev() {
            branches = format!(
                "global.get $step i32.const {index} i32.eq (if (result i32) (then i32.const {offset} i32.const {length} call $emit) (else {branches}))",
                offset = index * 1024,
                length = documents[index].len()
            );
        }
        let wasm = wat::parse_str(format!(r#"(module
            (import "studio_host" "emit" (func $emit (param i32 i32) (result i32)))
            (memory (export "memory") 1 1) (table 1 1 funcref)
            (global $step (mut i32) (i32.const 0))
            {data}
            (func (export "studio_alloc") (param i32) (result i32) i32.const 8192)
            (func (export "studio_dealloc") (param i32 i32))
            (func (export "studio_init") (param i32 i32) (result i32) i32.const 0 i32.const {initial_length} call $emit)
            (func (export "studio_event") (param i32 i32) (result i32)
                global.get $step i32.const 1 i32.add global.set $step {branches}))"#,initial_length=documents[0].len())).unwrap();
        let engine = studio_wasm::SandboxEngine::new().unwrap();
        let validated = studio_wasm::ModulePolicy::default()
            .validate(&engine, &wasm)
            .unwrap();
        let mut instance =
            PluginInstance::instantiate(engine, validated, studio_wasm::RuntimeBudgets::default())
                .unwrap();
        let outcome = instance.invoke_init(0, 0).unwrap();
        let GuestMessage::Mount(mount) =
            decode_guest_message(&outcome.emissions[0], ProtocolLimits::default()).unwrap()
        else {
            panic!("fixture did not mount");
        };
        let owner = studio_ui::InstanceId::new("com.studio.starter").unwrap();
        let mut registry = UiRegistry::new(owner.clone(), ProtocolLimits::default());
        registry.mount(mount).unwrap();
        let dispatcher = dispatcher_from_registry(&owner, &registry).unwrap();
        let native = NativeStateStore::from_registry(&owner, &registry).unwrap();
        let manifest = studio_package::parse_manifest(
            include_bytes!("../../../examples/starter/manifest.json"),
            studio_package::ManifestPolicy::default(),
        )
        .unwrap();
        let plan = studio_package::ProviderRegistry::maintained()
            .admit_manifest(&manifest, &studio_net::limits::BrokerLimits::default())
            .unwrap();
        let routing = project_router_from_assets(&assets).unwrap();
        PluginSurface::new(
            LaunchMode::Development,
            registry,
            native,
            dispatcher,
            instance,
            assets,
            ProtocolLimits::default(),
            plan,
            None,
            routing,
        )
    }

    #[test]
    fn real_guest_navigation_is_host_admitted_and_rebuilds_the_mounted_page() {
        let mut surface = routed_guest_surface(false);
        assert_eq!(surface.mount_route().as_deref(), Some("/"));
        surface
            .process_input("go-live", InputAction::PointerClick)
            .unwrap();
        assert_eq!(surface.mount_route().as_deref(), Some("/live"));
        assert!(
            surface
                .dispatch_input("go-live", InputAction::PointerClick)
                .is_err()
        );
        surface
            .process_input("back", InputAction::PointerClick)
            .unwrap();
        assert_eq!(surface.mount_route().as_deref(), Some("/"));
        assert!(
            surface
                .dispatch_input("go-live", InputAction::PointerClick)
                .is_ok()
        );
    }

    #[test]
    fn guest_cannot_commit_a_different_page_after_the_host_accepts_navigation() {
        let mut surface = routed_guest_surface(true);
        assert!(matches!(
            surface.process_input("go-live", InputAction::PointerClick),
            Err(SurfaceError::Navigation(_))
        ));
        assert_eq!(surface.mount_route().as_deref(), Some("/"));
        assert_eq!(surface.routing.as_ref().unwrap().current().route, "/");
        assert!(
            surface
                .dispatch_input("go-live", InputAction::PointerClick)
                .is_ok()
        );
    }

    fn timer_guest_surface() -> PluginSurface {
        use std::collections::BTreeMap;
        use studio_protocol::{NodeKind, PatchBatch, PatchOp};
        let node = |id: &str,
                    kind: NodeKind,
                    props: BTreeMap<String, Value>,
                    children: Vec<UiNode>| UiNode {
            id: id.to_owned(),
            kind,
            props,
            children,
        };
        let initial = GuestMessage::Mount(MountTree {
            protocol_version: studio_protocol::PROTOCOL_VERSION,
            route: "/".to_owned(),
            root: node(
                "home#root",
                NodeKind::Box,
                BTreeMap::new(),
                vec![
                    node(
                        "remaining",
                        NodeKind::Text,
                        BTreeMap::from([("text".to_owned(), json!("2"))]),
                        Vec::new(),
                    ),
                    node(
                        "go-live",
                        NodeKind::Button,
                        BTreeMap::from([("label".to_owned(), json!("Go live"))]),
                        Vec::new(),
                    ),
                    node(
                        "stream-timer",
                        NodeKind::Box,
                        BTreeMap::from([("visible".to_owned(), json!(false))]),
                        vec![
                            node(
                                "timer-status",
                                NodeKind::Text,
                                BTreeMap::from([("text".to_owned(), json!("running"))]),
                                Vec::new(),
                            ),
                            node(
                                "timer-tick",
                                NodeKind::Button,
                                BTreeMap::from([("label".to_owned(), json!("Tick"))]),
                                Vec::new(),
                            ),
                            node(
                                "timer-complete",
                                NodeKind::Button,
                                BTreeMap::from([("label".to_owned(), json!("Complete"))]),
                                Vec::new(),
                            ),
                        ],
                    ),
                ],
            ),
        });
        let property = |id: &str, text: &str| PatchOp::UpdateProp {
            node_id: id.to_owned(),
            property: "text".to_owned(),
            value: json!(text),
        };
        let messages = [
            initial,
            GuestMessage::Patch(PatchBatch {
                sequence: 1,
                operations: vec![property("remaining", "1")],
            }),
            GuestMessage::Patch(PatchBatch {
                sequence: 2,
                operations: vec![
                    property("remaining", "0"),
                    property("timer-status", "complete"),
                ],
            }),
            GuestMessage::Navigate(NavigationCommand::Push {
                route: "/live".to_owned(),
            }),
            GuestMessage::Mount(MountTree {
                protocol_version: studio_protocol::PROTOCOL_VERSION,
                route: "/live".to_owned(),
                root: node("live#root", NodeKind::Box, BTreeMap::new(), Vec::new()),
            }),
        ];
        let documents: Vec<_> = messages
            .iter()
            .map(|message| serde_json::to_string(message).unwrap())
            .collect();
        let mut assets = routed_assets();
        assets.insert("assets/studio/timers.json".to_owned(), br#"{"schemaVersion":1,"pages":[{"id":"home","timers":[{"id":"stream-timer","tickNode":"timer-tick","completeNode":"timer-complete","statusNode":"timer-status","intervalMs":1000,"completionDelayMs":700}]}]}"#.to_vec());
        let mut surface = fixture_surface(&documents, assets);
        let manifest =
            project_timer_manifest_from_assets(&surface.assets, surface.routing.as_ref())
                .unwrap()
                .unwrap();
        surface.configure_timers(manifest).unwrap();
        surface
    }

    #[test]
    fn host_timer_rechecks_other_timer_reset_before_delivering_its_due_completion() {
        use std::collections::BTreeMap;
        use studio_protocol::{NodeKind, PatchBatch, PatchOp};
        let hidden = |id: &str, status: &str| UiNode {
            id: id.to_owned(),
            kind: NodeKind::Box,
            props: BTreeMap::from([("visible".to_owned(), json!(false))]),
            children: vec![
                UiNode {
                    id: format!("{id}-status"),
                    kind: NodeKind::Text,
                    props: BTreeMap::from([("text".to_owned(), json!(status))]),
                    children: Vec::new(),
                },
                UiNode {
                    id: format!("{id}-tick"),
                    kind: NodeKind::Button,
                    props: BTreeMap::from([("label".to_owned(), json!("Tick"))]),
                    children: Vec::new(),
                },
                UiNode {
                    id: format!("{id}-complete"),
                    kind: NodeKind::Button,
                    props: BTreeMap::from([("label".to_owned(), json!("Complete"))]),
                    children: Vec::new(),
                },
            ],
        };
        let property = |id: &str, text: &str| PatchOp::UpdateProp {
            node_id: id.to_owned(),
            property: "text".to_owned(),
            value: json!(text),
        };
        let messages = [
            GuestMessage::Mount(MountTree {
                protocol_version: studio_protocol::PROTOCOL_VERSION,
                route: "/".to_owned(),
                root: UiNode {
                    id: "home#root".to_owned(),
                    kind: NodeKind::Box,
                    props: BTreeMap::new(),
                    children: vec![
                        UiNode {
                            id: "observed".to_owned(),
                            kind: NodeKind::Text,
                            props: BTreeMap::from([("text".to_owned(), json!("ready"))]),
                            children: Vec::new(),
                        },
                        hidden("a", "running"),
                        hidden("b", "complete"),
                    ],
                },
            }),
            GuestMessage::Patch(PatchBatch {
                sequence: 1,
                operations: vec![property("observed", "one tick")],
            }),
            GuestMessage::Patch(PatchBatch {
                sequence: 2,
                operations: vec![
                    property("a-status", "complete"),
                    property("b-status", "running"),
                    property("observed", "reset"),
                ],
            }),
            GuestMessage::Patch(PatchBatch {
                sequence: 3,
                operations: vec![
                    property("b-status", "complete"),
                    property("observed", "fresh tick"),
                ],
            }),
        ];
        let documents: Vec<_> = messages
            .iter()
            .map(|message| serde_json::to_string(message).unwrap())
            .collect();
        let mut assets = routed_assets();
        let spec = |id: &str, delay: u64| json!({"id":id,"tickNode":format!("{id}-tick"),"completeNode":format!("{id}-complete"),"statusNode":format!("{id}-status"),"intervalMs":1000,"completionDelayMs":delay});
        assets.insert(TIMER_ASSET.to_owned(), serde_json::to_vec(&json!({"schemaVersion":1,"pages":[{"id":"home","timers":[spec("a",60000),spec("b",2000)]}]})).unwrap());
        let mut surface = fixture_surface(&documents, assets);
        let manifest =
            project_timer_manifest_from_assets(&surface.assets, surface.routing.as_ref())
                .unwrap()
                .unwrap();
        surface.configure_timers(manifest).unwrap();
        assert!(!surface.poll_timers_at(Duration::ZERO).unwrap());
        assert!(surface.poll_timers_at(Duration::from_secs(1)).unwrap());
        assert!(surface.poll_timers_at(Duration::from_secs(2)).unwrap());
        assert_eq!(
            surface.metrics().guest_event_calls,
            2,
            "timer a reset b before b's queued completion, so b must not fire yet"
        );
        assert_eq!(
            surface
                .registry
                .get(surface.dispatcher_owner(), "observed")
                .unwrap()
                .props["text"],
            "reset"
        );
        assert!(surface.poll_timers_at(Duration::from_secs(3)).unwrap());
        assert_eq!(surface.metrics().guest_event_calls, 3);
        assert_eq!(
            surface
                .registry
                .get(surface.dispatcher_owner(), "observed")
                .unwrap()
                .props["text"],
            "fresh tick"
        );
    }

    #[test]
    fn host_timer_internal_callbacks_are_excluded_from_keyboard_focus() {
        let surface = timer_guest_surface();
        let focusable: Vec<_> = surface
            .keyboard_focus_order()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(focusable, vec!["go-live"]);
    }

    #[test]
    fn host_timer_delivers_due_guest_ticks_then_one_delayed_completion_without_idle_calls() {
        use std::time::Duration;
        let mut surface = timer_guest_surface();
        assert!(!surface.poll_timers_at(Duration::from_millis(999)).unwrap());
        assert_eq!(surface.metrics().guest_event_calls, 0);
        assert!(surface.poll_timers_at(Duration::from_secs(1)).unwrap());
        assert_eq!(
            surface
                .registry()
                .get(surface.dispatcher_owner(), "remaining")
                .unwrap()
                .props["text"],
            "1"
        );
        assert!(surface.poll_timers_at(Duration::from_secs(2)).unwrap());
        assert_eq!(
            surface
                .registry()
                .get(surface.dispatcher_owner(), "remaining")
                .unwrap()
                .props["text"],
            "0"
        );
        assert!(!surface.poll_timers_at(Duration::from_secs(2)).unwrap());
        assert!(!surface.poll_timers_at(Duration::from_millis(2699)).unwrap());
        assert!(surface.poll_timers_at(Duration::from_millis(2700)).unwrap());
        assert_eq!(surface.mount_route().as_deref(), Some("/live"));
        let calls = surface.metrics().guest_event_calls;
        assert!(!surface.poll_timers_at(Duration::from_secs(60)).unwrap());
        assert_eq!(surface.metrics().guest_event_calls, calls);
    }

    #[test]
    fn host_timer_route_exit_cancels_pending_completion_before_its_deadline() {
        let mut surface = timer_guest_surface();
        surface.poll_timers_at(Duration::from_secs(1)).unwrap();
        surface.poll_timers_at(Duration::from_secs(2)).unwrap();
        surface
            .process_input("go-live", InputAction::PointerClick)
            .unwrap();
        assert_eq!(surface.mount_route().as_deref(), Some("/live"));
        let calls = surface.metrics().guest_event_calls;
        assert!(!surface.poll_timers_at(Duration::from_secs(60)).unwrap());
        assert_eq!(surface.metrics().guest_event_calls, calls);
    }

    #[test]
    fn timer_callbacks_cannot_target_controls_outside_the_invisible_timer_wrapper() {
        let mut surface = timer_guest_surface();
        let mut manifest =
            project_timer_manifest_from_assets(&surface.assets, surface.routing.as_ref())
                .unwrap()
                .unwrap();
        manifest.pages[0].timers[0].tick_node = "remaining".to_owned();
        assert!(surface.configure_timers(manifest).is_err());
        assert!(
            surface
                .poll_timers_at(std::time::Duration::from_secs(1))
                .unwrap()
        );
        assert_eq!(
            surface
                .registry()
                .get(surface.dispatcher_owner(), "remaining")
                .unwrap()
                .props["text"],
            "1"
        );
    }

    #[test]
    fn host_timer_catch_up_stops_at_completion_and_rejects_foreign_page_metadata() {
        use std::time::Duration;
        let mut surface = timer_guest_surface();
        assert!(surface.poll_timers_at(Duration::from_secs(10)).unwrap());
        assert_eq!(
            surface.metrics().guest_event_calls,
            2,
            "remaining due ticks stop after the completion status changes"
        );
        let mut assets = routed_assets();
        assets.insert(
            "assets/studio/timers.json".to_owned(),
            br#"{"schemaVersion":1,"pages":[{"id":"foreign","timers":[]}]}"#.to_vec(),
        );
        let router = project_router_from_assets(&assets).unwrap();
        assert!(project_timer_manifest_from_assets(&assets, router.as_ref()).is_err());
    }

    fn routed_assets() -> std::collections::BTreeMap<String, Vec<u8>> {
        [(ROUTING_ASSET.to_owned(), br#"{"schemaVersion":1,"routes":[{"id":"home","path":"/","source":"routes/+page.studio"},{"id":"live","path":"/live","source":"routes/live/+page.studio"}]}"#.to_vec())].into()
    }

    #[test]
    fn signed_routing_metadata_is_admitted_and_unknown_commands_are_atomic() {
        let mut router = project_router_from_assets(&routed_assets())
            .unwrap()
            .unwrap();
        assert_eq!(router.current().id, "home");
        assert!(
            router
                .navigate(&NavigationCommand::Push {
                    route: "/unknown".to_owned()
                })
                .is_err()
        );
        assert_eq!(router.current().route, "/");
        assert!(router.navigate(&NavigationCommand::Pop).is_err());
        assert_eq!(router.current().route, "/");
        router
            .navigate(&NavigationCommand::Push {
                route: "/live".to_owned(),
            })
            .unwrap();
        assert_eq!(router.current().id, "live");
        router.navigate(&NavigationCommand::Pop).unwrap();
        assert_eq!(router.current().route, "/");
    }

    #[test]
    fn malformed_signed_routing_metadata_fails_and_legacy_assets_are_unchanged() {
        assert!(
            project_router_from_assets(&std::collections::BTreeMap::new())
                .unwrap()
                .is_none()
        );
        for bytes in [br#"{"schemaVersion":2,"routes":[]}"#.as_slice(), br#"{"schemaVersion":1,"routes":[],"unknown":true}"#.as_slice(), br#"{"schemaVersion":1,"routes":[{"id":"bad","path":"/","source":"../outside.studio"}]}"#.as_slice()] {
            let assets = [(ROUTING_ASSET.to_owned(), bytes.to_vec())].into();
            assert!(project_router_from_assets(&assets).is_err());
        }
    }

    #[test]
    fn only_the_host_selected_page_and_concrete_route_can_remount() {
        let router = project_router_from_assets(&routed_assets())
            .unwrap()
            .unwrap();
        let selection = router.current();
        let mut mount = MountTree {
            protocol_version: studio_protocol::PROTOCOL_VERSION,
            route: "/".to_owned(),
            root: UiNode {
                id: "home#root".to_owned(),
                kind: studio_protocol::NodeKind::Box,
                props: std::collections::BTreeMap::new(),
                children: Vec::new(),
            },
        };
        assert!(validate_routed_mount(&selection, &mount).is_ok());
        mount.route = "/live".to_owned();
        assert!(validate_routed_mount(&selection, &mount).is_err());
        mount.route = "/".to_owned();
        mount.root.id = "live#root".to_owned();
        assert!(validate_routed_mount(&selection, &mount).is_err());
    }

    #[test]
    #[cfg(not(feature = "services"))]
    fn core_surface_returns_a_denial_for_unavailable_host_actions() {
        let surface = timer_guest_surface();
        surface.queue_action(ActionRequest {
            request_id: "unavailable-1".into(),
            capability: "github.oauth".into(),
            operation: "sign_in".into(),
            payload: json!({}),
        });
        assert!(matches!(
            surface.action_receiver.try_recv().unwrap(),
            ActionResult::Failure { request_id, code, retryable: false, .. }
                if request_id == "unavailable-1" && code == "capability_denied"
        ));
    }

    /// One sign-in request for the provider the capability names.
    #[cfg(feature = "services")]
    fn sign_in_request(capability: &str, provider: Option<&str>) -> ActionRequest {
        ActionRequest {
            request_id: String::from("sign-in-1"),
            capability: capability.to_owned(),
            operation: String::from("sign_in"),
            payload: match provider {
                Some(provider) => json!({"provider": provider}),
                None => json!({}),
            },
        }
    }

    /// The failure code one action answered with.
    #[cfg(feature = "services")]
    fn failure_code(result: ActionResult) -> String {
        match result {
            ActionResult::Failure { code, .. } => code,
            ActionResult::Success { .. } => panic!("the action was not refused"),
        }
    }

    #[test]
    #[cfg(feature = "services")]
    fn a_capability_naming_no_provider_the_host_offers_is_denied() {
        let result = run_provider_action(sign_in_request("storage.oauth", None), None, None);
        assert_eq!(failure_code(result), "capability_denied");
    }

    #[test]
    #[cfg(feature = "services")]
    fn a_payload_naming_another_provider_than_the_capability_is_unknown() {
        let result =
            run_provider_action(sign_in_request("github.oauth", Some("discord")), None, None);
        assert_eq!(failure_code(result), "oauth.provider.unknown");
    }
}

impl NativeSurface for PluginSurface {
    type Error = SurfaceError;
    fn render_tree(&self) -> Option<PluginRenderNode> {
        self.render_tree()
    }
    fn process_input(&mut self, node_id: &str, action: InputAction) -> Result<(), Self::Error> {
        self.process_input(node_id, action).map(|_| ())
    }
    fn poll_action_results(&mut self) -> Result<bool, Self::Error> {
        self.poll_action_results()
    }
    fn asset(&self, path: &str) -> Option<&[u8]> {
        self.asset(path)
    }
    fn warning(&self) -> Option<&str> {
        self.warning()
    }
}
