//! Stable, non-oracular protocol validation failures.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable protocol error families exposed across subsystem boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    MessageInvalid,
    MessageTooLarge,
    ProtocolUnsupported,
    LifecycleInvalid,
    TreeInvalid,
    PatchInvalid,
    SequenceInvalid,
    RouteNotFound,
    NavigationDenied,
    NavigationTimeout,
    CapabilityDenied,
    ActionInvalid,
    ResourceExhausted,
    GuestTerminated,
}

/// Detailed internal protocol validation error.
#[derive(Debug, Error, PartialEq)]
pub enum ProtocolError {
    #[error("{kind} envelope is {actual} bytes; limit is {limit}")]
    MessageTooLarge {
        kind: &'static str,
        actual: usize,
        limit: usize,
    },
    #[error("invalid protocol JSON: {0}")]
    InvalidJson(String),
    #[error("unsupported protocol version {0}")]
    UnsupportedVersion(u16),
    #[error("invalid node id: {0}")]
    InvalidNodeId(String),
    #[error("duplicate node id: {0}")]
    DuplicateNodeId(String),
    #[error("UI tree exceeds {0} nodes")]
    TooManyNodes(usize),
    #[error("UI tree exceeds depth {0}")]
    TreeTooDeep(usize),
    #[error("invalid property {property} on node {node_id}")]
    InvalidNodeProperty { node_id: String, property: String },
    #[error("invalid child count {actual} for node {node_id}")]
    InvalidChildCount { node_id: String, actual: usize },
    #[error("patch must contain between 1 and {0} operations")]
    InvalidPatchOperationCount(usize),
    #[error("patch sequence {received} must be greater than {previous}")]
    InvalidPatchSequence { previous: u64, received: u64 },
    #[error("invalid route: {0}")]
    InvalidRoute(String),
    #[error("invalid action field: {0}")]
    InvalidAction(&'static str),
    #[error("invalid lifecycle event: {0}")]
    InvalidLifecycle(&'static str),
}

/// Stable developer-facing diagnostic with no untrusted payload context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeveloperDiagnostic {
    code: &'static str,
    message: &'static str,
}

impl DeveloperDiagnostic {
    /// Stable machine-readable code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        self.code
    }

    /// Safe corrective summary.
    #[must_use]
    pub const fn message(self) -> &'static str {
        self.message
    }
}

impl ProtocolError {
    /// Return the stable family for this detailed rejection.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::MessageTooLarge { .. } => ErrorCode::MessageTooLarge,
            Self::InvalidJson(_) => ErrorCode::MessageInvalid,
            Self::UnsupportedVersion(_) => ErrorCode::ProtocolUnsupported,
            Self::InvalidNodeId(_)
            | Self::DuplicateNodeId(_)
            | Self::TooManyNodes(_)
            | Self::TreeTooDeep(_)
            | Self::InvalidNodeProperty { .. }
            | Self::InvalidChildCount { .. } => ErrorCode::TreeInvalid,
            Self::InvalidPatchOperationCount(_) => ErrorCode::PatchInvalid,
            Self::InvalidPatchSequence { .. } => ErrorCode::SequenceInvalid,
            Self::InvalidRoute(_) => ErrorCode::RouteNotFound,
            Self::InvalidAction(_) => ErrorCode::ActionInvalid,
            Self::InvalidLifecycle(_) => ErrorCode::LifecycleInvalid,
        }
    }

    /// Convert a detailed decoder error into bounded, non-sensitive developer guidance.
    #[must_use]
    pub const fn diagnostic(&self) -> DeveloperDiagnostic {
        match self.code() {
            ErrorCode::MessageInvalid => DeveloperDiagnostic {
                code: "message_invalid",
                message: "message does not match the closed protocol schema",
            },
            ErrorCode::MessageTooLarge => DeveloperDiagnostic {
                code: "message_too_large",
                message: "message exceeds its host limit",
            },
            ErrorCode::ProtocolUnsupported => DeveloperDiagnostic {
                code: "protocol_unsupported",
                message: "protocol version is unsupported",
            },
            ErrorCode::LifecycleInvalid => DeveloperDiagnostic {
                code: "lifecycle_invalid",
                message: "lifecycle transition is invalid",
            },
            ErrorCode::TreeInvalid => DeveloperDiagnostic {
                code: "tree_invalid",
                message: "UI tree or property is invalid",
            },
            ErrorCode::PatchInvalid => DeveloperDiagnostic {
                code: "patch_invalid",
                message: "patch batch is invalid",
            },
            ErrorCode::SequenceInvalid => DeveloperDiagnostic {
                code: "sequence_invalid",
                message: "patch sequence is not increasing",
            },
            ErrorCode::RouteNotFound => DeveloperDiagnostic {
                code: "route_not_found",
                message: "route is invalid or undeclared",
            },
            ErrorCode::NavigationDenied => DeveloperDiagnostic {
                code: "navigation_denied",
                message: "navigation was denied",
            },
            ErrorCode::NavigationTimeout => DeveloperDiagnostic {
                code: "navigation_timeout",
                message: "navigation guard exceeded its budget",
            },
            ErrorCode::CapabilityDenied => DeveloperDiagnostic {
                code: "capability_denied",
                message: "capability is unavailable",
            },
            ErrorCode::ActionInvalid => DeveloperDiagnostic {
                code: "action_invalid",
                message: "action request is invalid",
            },
            ErrorCode::ResourceExhausted => DeveloperDiagnostic {
                code: "resource_exhausted",
                message: "plugin exceeded a host resource limit",
            },
            ErrorCode::GuestTerminated => DeveloperDiagnostic {
                code: "guest_terminated",
                message: "plugin instance has terminated",
            },
        }
    }
}
