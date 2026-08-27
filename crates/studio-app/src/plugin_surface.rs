//! Prepared mounted plugin surface retained for the complete launch lifetime.

use serde_json::Value;
use studio_components::{
    ComponentCatalog, DispatchError, HostEventDispatcher, InputAction, NativeStateStore,
    RuntimeControl, UpdateError, UpdateReport,
};
use studio_protocol::{
    GuestMessage, HostEvent, NodeKind, ProtocolError, ProtocolLimits, UiNode, decode_guest_message,
};
use studio_ui::{PatchError, PatchMetrics, UiRegistry};
use studio_wasm::{PluginInstance, RuntimeError};
use studio_package::ProviderAdmissionPlan;
use thiserror::Error;

use crate::cli::LaunchMode;

/// Persistent host-owned indicator required for explicitly untrusted development launches.
pub const DEVELOPMENT_WARNING: &str = "Development mode: unsigned plugin bundle";

/// Immutable retained-node copy used by the GPUI render pass.
#[derive(Clone, Debug)]
pub(crate) struct PluginRenderNode {
    pub id: String,
    pub kind: NodeKind,
    pub control: Option<RuntimeControl>,
    pub props: std::collections::BTreeMap<String, Value>,
    pub children: Vec<Self>,
}

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
    guest_event_calls: u64,
    guest_patch_messages: u64,
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
    ) -> Self {
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
            guest_event_calls: 0,
            guest_patch_messages: 0,
        }
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
        let mut invalidated_nodes = Vec::new();
        for emission in outcome.emissions {
            let GuestMessage::Patch(batch) = decode_guest_message(&emission, self.protocol_limits)?
            else {
                return Err(SurfaceError::UnexpectedGuestMessage);
            };
            self.guest_patch_messages = self.guest_patch_messages.saturating_add(1);
            let commit = self.registry.apply_patch(&owner, batch)?;
            let report = self
                .native_state
                .apply_commit(&owner, &self.registry, &commit)?;
            for node_id in report.invalidated_nodes {
                if !invalidated_nodes.contains(&node_id) {
                    invalidated_nodes.push(node_id);
                }
            }
        }
        Ok(UpdateReport { invalidated_nodes })
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

    /// Build a host-owned checkout shell bound to this mounted surface's principal.
    ///
    /// # Errors
    ///
    /// Rejects invalid checkout or navigation initialization while retaining the
    /// existing mounted UI state.
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

fn collect_focusable(node: &PluginRenderNode, order: &mut Vec<(String, String)>) {
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
    #[error("guest event call emitted a non-patch message")]
    UnexpectedGuestMessage,
    /// Retained transaction validation failed.
    #[error(transparent)]
    Patch(#[from] PatchError),
    /// Native reconciliation failed.
    #[error(transparent)]
    Update(#[from] UpdateError),
}
