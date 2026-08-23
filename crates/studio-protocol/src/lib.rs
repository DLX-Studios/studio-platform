//! Authoritative, closed protocol-v1 messages crossing the Studio host–guest boundary.
#![allow(
    missing_docs,
    reason = "wire fields and variants are documented in the checked-in protocol-v1 contracts"
)]

pub mod actions;
mod error;
pub mod lifecycle;
pub mod navigation;
mod properties;
pub mod ui;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use actions::{
    ActionRequest, ActionResult, MoneyPayload, PrintPreviewPayload, ReceiptLinePayload,
    ReceiptPayload,
};
pub use error::{DeveloperDiagnostic, ErrorCode, ProtocolError};
pub use lifecycle::{LifecycleEvent, LifecycleState};
pub use navigation::{NavigationCommand, NavigationEvent};
pub use properties::validate_node_contract;
pub use ui::{MountTree, NodeKind, PatchBatch, PatchOp, UiEvent, UiNode};

/// Protocol version implemented by this crate.
pub const PROTOCOL_VERSION: u16 = 1;

/// Defensive limits applied before an untrusted message may affect host state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolLimits {
    pub max_message_bytes: usize,
    pub max_mount_bytes: usize,
    pub max_patch_bytes: usize,
    pub max_patch_operations: usize,
    pub max_nodes: usize,
    pub max_tree_depth: usize,
    pub max_node_id_bytes: usize,
    pub max_string_bytes: usize,
    pub max_navigation_depth: usize,
    pub max_pending_actions: usize,
}

impl Default for ProtocolLimits {
    fn default() -> Self {
        Self {
            max_message_bytes: 64 * 1024,
            max_mount_bytes: 1024 * 1024,
            max_patch_bytes: 256 * 1024,
            max_patch_operations: 512,
            max_nodes: 5_000,
            max_tree_depth: 64,
            max_node_id_bytes: 128,
            max_string_bytes: 64 * 1024,
            max_navigation_depth: 32,
            max_pending_actions: 16,
        }
    }
}

/// A closed message emitted by a guest plugin.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum GuestMessage {
    Mount(MountTree),
    Patch(PatchBatch),
    Navigate(NavigationCommand),
    Action(ActionRequest),
    Log(GuestLog),
}

/// A closed event delivered by the host to a guest plugin.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum HostEvent {
    Ui(UiEvent),
    Navigation(NavigationEvent),
    ActionResult(ActionResult),
    Lifecycle(LifecycleEvent),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuestLog {
    pub level: LogLevel,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

/// Decode and structurally validate one bounded, untrusted guest envelope.
///
/// # Errors
///
/// Returns a stable [`ProtocolError`] for size, JSON, version, or structural violations.
pub fn decode_guest_message(
    bytes: &[u8],
    limits: ProtocolLimits,
) -> Result<GuestMessage, ProtocolError> {
    ensure_size(
        "guest",
        bytes.len(),
        limits.max_mount_bytes.max(limits.max_patch_bytes),
    )?;
    let message: GuestMessage = decode_json(bytes)?;
    let limit = match &message {
        GuestMessage::Mount(_) => limits.max_mount_bytes,
        GuestMessage::Patch(_) => limits.max_patch_bytes,
        _ => limits.max_message_bytes,
    };
    ensure_size("guest", bytes.len(), limit)?;
    validate_guest_message(&message, limits)?;
    Ok(message)
}

/// Decode and structurally validate one bounded host event before guest delivery.
///
/// # Errors
///
/// Returns a stable [`ProtocolError`] for size, JSON, or structural violations.
pub fn decode_host_event(bytes: &[u8], limits: ProtocolLimits) -> Result<HostEvent, ProtocolError> {
    ensure_size("host", bytes.len(), limits.max_message_bytes)?;
    let event: HostEvent = decode_json(bytes)?;
    validate_host_event(&event, limits)?;
    Ok(event)
}

/// Validate an already-decoded guest message against protocol-v1 invariants.
///
/// # Errors
///
/// Returns a stable [`ProtocolError`] when any invariant is violated.
pub fn validate_guest_message(
    message: &GuestMessage,
    limits: ProtocolLimits,
) -> Result<(), ProtocolError> {
    match message {
        GuestMessage::Mount(mount) => ui::validate_mount(mount, limits),
        GuestMessage::Patch(batch) => ui::validate_patch(batch, limits),
        GuestMessage::Navigate(command) => navigation::validate_navigation_command(command),
        GuestMessage::Action(action) => actions::validate_action_request(action, limits),
        GuestMessage::Log(log) => validate_bounded_string(&log.message, limits.max_string_bytes),
    }
}

/// Validate an already-decoded host event against protocol-v1 invariants.
///
/// # Errors
///
/// Returns a stable [`ProtocolError`] when any invariant is violated.
pub fn validate_host_event(event: &HostEvent, limits: ProtocolLimits) -> Result<(), ProtocolError> {
    match event {
        HostEvent::Ui(event) => ui::validate_ui_event(event, limits),
        HostEvent::Navigation(event) => navigation::validate_navigation_event(event),
        HostEvent::ActionResult(result) => actions::validate_action_result(result, limits),
        HostEvent::Lifecycle(event) => lifecycle::validate_lifecycle_event(event, limits),
    }
}

/// Require a non-zero patch sequence strictly greater than the last committed sequence.
///
/// # Errors
///
/// Returns [`ProtocolError::InvalidPatchSequence`] for zero, replayed, or decreasing sequences.
pub fn validate_patch_sequence(
    batch: &PatchBatch,
    previous: Option<u64>,
) -> Result<(), ProtocolError> {
    let previous = previous.unwrap_or(0);
    if batch.sequence <= previous {
        return Err(ProtocolError::InvalidPatchSequence {
            previous,
            received: batch.sequence,
        });
    }
    Ok(())
}

fn decode_json<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, ProtocolError> {
    serde_json::from_slice(bytes).map_err(|error| ProtocolError::InvalidJson(error.to_string()))
}

fn ensure_size(kind: &'static str, actual: usize, limit: usize) -> Result<(), ProtocolError> {
    if actual > limit {
        return Err(ProtocolError::MessageTooLarge {
            kind,
            actual,
            limit,
        });
    }
    Ok(())
}

pub(crate) fn validate_bounded_string(value: &str, limit: usize) -> Result<(), ProtocolError> {
    if value.len() > limit {
        return Err(ProtocolError::MessageTooLarge {
            kind: "string",
            actual: value.len(),
            limit,
        });
    }
    Ok(())
}
