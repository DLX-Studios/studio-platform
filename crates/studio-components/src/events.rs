//! Owner-checked conversion from native input into typed host events.

use std::collections::BTreeMap;

use serde_json::json;
use studio_protocol::{HostEvent, NodeKind, UiEvent};
use studio_ui::InstanceId;
use thiserror::Error;

/// Native input accepted by Studio-owned component wrappers.
#[derive(Clone, Debug, PartialEq)]
pub enum InputAction {
    /// Primary pointer activation.
    PointerClick,
    /// Keyboard activation through Enter or Space.
    KeyboardActivate,
    /// Touch activation.
    TouchActivate,
    /// Native scroll delta in logical pixels.
    Scroll {
        /// Horizontal delta.
        delta_x: f32,
        /// Vertical delta.
        delta_y: f32,
    },
    /// Slider drag resulting in a semantic numeric value.
    SliderDrag {
        /// New slider value.
        value: f64,
    },
    /// Non-secret host text input update.
    TextChanged {
        /// New non-secret text.
        value: String,
    },
    /// A value selected from a native selection control.
    SelectionChanged {
        /// Selected non-secret value.
        value: String,
    },
}

/// Stable host-event dispatch rejection family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchErrorCode {
    /// Caller does not own the dispatcher namespace.
    OwnerMismatch,
    /// Node identity is already registered.
    DuplicateNode,
    /// Node identity is unknown.
    NodeNotFound,
    /// Input action is invalid for the registered kind.
    ActionInvalid,
}

/// Detailed host-event dispatch rejection.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DispatchError {
    /// Instance ownership check failed.
    #[error("event owner mismatch")]
    OwnerMismatch,
    /// Registration would create an ambiguous local identity.
    #[error("event node is already registered: {0}")]
    DuplicateNode(String),
    /// Dispatch target is not registered.
    #[error("event node not found: {0}")]
    NodeNotFound(String),
    /// Native action is incompatible with the retained node kind.
    #[error("native action is invalid for node kind")]
    ActionInvalid,
}

impl DispatchError {
    /// Return the stable dispatch error family.
    #[must_use]
    pub const fn code(&self) -> DispatchErrorCode {
        match self {
            Self::OwnerMismatch => DispatchErrorCode::OwnerMismatch,
            Self::DuplicateNode(_) => DispatchErrorCode::DuplicateNode,
            Self::NodeNotFound(_) => DispatchErrorCode::NodeNotFound,
            Self::ActionInvalid => DispatchErrorCode::ActionInvalid,
        }
    }
}

/// Native event source registry bound to exactly one plugin instance.
#[derive(Debug)]
pub struct HostEventDispatcher {
    owner: InstanceId,
    nodes: BTreeMap<String, NodeKind>,
}

impl HostEventDispatcher {
    /// Create an empty dispatcher for one active instance.
    #[must_use]
    pub fn new(owner: InstanceId) -> Self {
        Self {
            owner,
            nodes: BTreeMap::new(),
        }
    }

    /// Borrow the host-owned instance identity used for all dispatches.
    #[must_use]
    pub const fn owner(&self) -> &InstanceId {
        &self.owner
    }

    /// Register one retained node as a native event source.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError::DuplicateNode`] for repeated local identities.
    pub fn register(
        &mut self,
        node_id: impl Into<String>,
        kind: NodeKind,
    ) -> Result<(), DispatchError> {
        let node_id = node_id.into();
        if self.nodes.insert(node_id.clone(), kind).is_some() {
            return Err(DispatchError::DuplicateNode(node_id));
        }
        Ok(())
    }

    /// Convert native input into a typed guest-facing host event after owner/kind checks.
    ///
    /// # Errors
    ///
    /// Returns owner, target, or incompatible-action errors. Secret input has no action carrying
    /// raw text, so entered secret bytes cannot enter the protocol event.
    pub fn dispatch(
        &self,
        owner: &InstanceId,
        node_id: &str,
        action: InputAction,
    ) -> Result<HostEvent, DispatchError> {
        if owner != &self.owner {
            return Err(DispatchError::OwnerMismatch);
        }
        let kind = self
            .nodes
            .get(node_id)
            .copied()
            .ok_or_else(|| DispatchError::NodeNotFound(node_id.to_owned()))?;
        let (event, payload) = event_payload(kind, action)?;
        Ok(HostEvent::Ui(UiEvent {
            node_id: node_id.to_owned(),
            event: event.to_owned(),
            payload,
        }))
    }
}

fn event_payload(
    kind: NodeKind,
    action: InputAction,
) -> Result<(&'static str, serde_json::Value), DispatchError> {
    match (kind, action) {
        (
            NodeKind::Button | NodeKind::IconButton | NodeKind::ButtonGroup,
            InputAction::PointerClick | InputAction::KeyboardActivate | InputAction::TouchActivate,
        ) => Ok(("pressed", json!({}))),
        (NodeKind::ScrollView | NodeKind::ListView, InputAction::Scroll { delta_x, delta_y }) => {
            Ok(("scrolled", json!({"delta_x": delta_x, "delta_y": delta_y})))
        }
        (
            NodeKind::Slider | NodeKind::RangeSlider | NodeKind::NumberInput,
            InputAction::SliderDrag { value },
        ) if value.is_finite() => Ok(("changed", json!({"value": value}))),
        (
            NodeKind::TextInput
            | NodeKind::TextArea
            | NodeKind::Field
            | NodeKind::InputGroup
            | NodeKind::OtpInput,
            InputAction::TextChanged { value },
        )
        | (
            NodeKind::Select
            | NodeKind::Combobox
            | NodeKind::Checkbox
            | NodeKind::Radio
            | NodeKind::Switch
            | NodeKind::Toggle,
            InputAction::SelectionChanged { value },
        ) => Ok(("changed", json!({"value": value}))),
        _ => Err(DispatchError::ActionInvalid),
    }
}

pub(crate) fn secret_ready_event(node_id: &str, authorization_ref: &str) -> HostEvent {
    HostEvent::Ui(UiEvent {
        node_id: node_id.to_owned(),
        event: "ready".to_owned(),
        payload: json!({
            "ready": true,
            "kind": "payment_pin",
            "expires_in_seconds": 120,
            "authorization_ref": authorization_ref,
        }),
    })
}
