//! Stable navigation-stack errors.

use std::{error::Error, fmt};

/// Stable failure codes for stack commands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StackErrorCode {
    /// The command came from a different plugin instance.
    OwnerMismatch,
    /// The bounded stack already contains 32 entries.
    StackOverflow,
    /// The root entry cannot be popped.
    RootPop,
    /// A pop-to target does not exist.
    TargetNotFound,
    /// A navigation guard rejected the command.
    GuardDenied,
    /// A navigation guard exceeded its 50 ms budget.
    GuardTimeout,
    /// A concrete route was malformed.
    RouteInvalid,
}

/// Safe navigation-stack error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StackError {
    pub(crate) code: StackErrorCode,
}

impl StackError {
    pub(crate) const fn new(code: StackErrorCode) -> Self {
        Self { code }
    }

    /// Return the stable failure code.
    #[must_use]
    pub const fn code(&self) -> StackErrorCode {
        self.code
    }
}

impl fmt::Display for StackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            StackErrorCode::OwnerMismatch => "navigation stack owner mismatch",
            StackErrorCode::StackOverflow => "navigation stack depth exceeded",
            StackErrorCode::RootPop => "navigation root cannot be popped",
            StackErrorCode::TargetNotFound => "pop-to target not found",
            StackErrorCode::GuardDenied => "navigation guard denied command",
            StackErrorCode::GuardTimeout => "navigation guard timed out",
            StackErrorCode::RouteInvalid => "navigation route invalid",
        })
    }
}

impl Error for StackError {}
