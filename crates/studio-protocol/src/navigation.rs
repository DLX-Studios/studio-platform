use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use crate::ProtocolError;

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum NavigationCommand {
    Push { route: String },
    Replace { route: String },
    Pop,
    PopTo { route: String },
    Reset { route: String },
}

impl<'de> Deserialize<'de> for NavigationCommand {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
        enum ClosedNavigationCommand {
            Push { route: String },
            Replace { route: String },
            Pop {},
            PopTo { route: String },
            Reset { route: String },
        }

        Ok(match ClosedNavigationCommand::deserialize(deserializer)? {
            ClosedNavigationCommand::Push { route } => Self::Push { route },
            ClosedNavigationCommand::Replace { route } => Self::Replace { route },
            ClosedNavigationCommand::Pop {} => Self::Pop,
            ClosedNavigationCommand::PopTo { route } => Self::PopTo { route },
            ClosedNavigationCommand::Reset { route } => Self::Reset { route },
        })
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NavigationEvent {
    pub route: String,
    pub accepted: bool,
    pub error_code: Option<String>,
}

/// Validate route-bearing fields in a navigation command.
///
/// # Errors
///
/// Returns [`ProtocolError::InvalidRoute`] when a route is not an absolute safe v1 route.
pub fn validate_navigation_command(command: &NavigationCommand) -> Result<(), ProtocolError> {
    match command {
        NavigationCommand::Push { route }
        | NavigationCommand::Replace { route }
        | NavigationCommand::PopTo { route }
        | NavigationCommand::Reset { route } => validate_route(route),
        NavigationCommand::Pop => Ok(()),
    }
}

pub(crate) fn validate_navigation_event(event: &NavigationEvent) -> Result<(), ProtocolError> {
    validate_route(&event.route)?;
    if event.accepted && event.error_code.is_some() {
        return Err(ProtocolError::InvalidLifecycle(
            "accepted navigation event cannot contain an error",
        ));
    }
    if !event.accepted && event.error_code.as_deref().is_none_or(str::is_empty) {
        return Err(ProtocolError::InvalidLifecycle(
            "rejected navigation event requires an error",
        ));
    }
    Ok(())
}

pub(crate) fn validate_route(route: &str) -> Result<(), ProtocolError> {
    if !route.starts_with('/')
        || route.contains("//")
        || route.contains(['?', '#', '\\'])
        || route.chars().any(char::is_control)
    {
        return Err(ProtocolError::InvalidRoute(route.to_owned()));
    }
    Ok(())
}
