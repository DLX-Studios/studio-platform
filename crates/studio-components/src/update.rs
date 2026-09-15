//! Targeted native reconciliation reports and stable failures.

use thiserror::Error;

/// Stable native-state update rejection family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateErrorCode {
    /// Caller or registry belongs to another plugin instance.
    OwnerMismatch,
    /// Retained/native target does not exist.
    NodeNotFound,
    /// Requested native state is incompatible with the node kind.
    StateInvalid,
}

/// Detailed native-state update rejection.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum UpdateError {
    /// Instance ownership check failed.
    #[error("native state owner mismatch")]
    OwnerMismatch,
    /// Target identity is unknown.
    #[error("native node not found: {0}")]
    NodeNotFound(String),
    /// State operation is incompatible with the node kind.
    #[error("native state operation is invalid")]
    StateInvalid,
}

impl UpdateError {
    /// Return the stable family for this rejection.
    #[must_use]
    pub const fn code(&self) -> UpdateErrorCode {
        match self {
            Self::OwnerMismatch => UpdateErrorCode::OwnerMismatch,
            Self::NodeNotFound(_) => UpdateErrorCode::NodeNotFound,
            Self::StateInvalid => UpdateErrorCode::StateInvalid,
        }
    }
}

/// Exact nodes whose native presentation was invalidated by one commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateReport {
    /// Stable node identities in first-change order.
    pub invalidated_nodes: Vec<String>,
}
