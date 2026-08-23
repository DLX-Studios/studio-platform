use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ProtocolError, ProtocolLimits, validate_bounded_string};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LifecycleEvent {
    pub state: LifecycleState,
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Loading,
    Running,
    Trapped,
    Stopped,
}

pub(crate) fn validate_lifecycle_event(
    event: &LifecycleEvent,
    limits: ProtocolLimits,
) -> Result<(), ProtocolError> {
    if let Some(message) = &event.message {
        validate_bounded_string(message, limits.max_string_bytes)?;
    }
    Ok(())
}
