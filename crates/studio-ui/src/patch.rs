//! Stable atomic patch commit records and rejection families.

use thiserror::Error;

/// Stable retained-tree patch rejection family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchErrorCode {
    /// Caller does not own this tree namespace.
    OwnerMismatch,
    /// Batch sequence is zero, replayed, or decreasing.
    SequenceInvalid,
    /// Target node does not exist.
    TargetInvalid,
    /// Child insertion index is outside the current ordered children.
    IndexInvalid,
    /// Operation illegally removes the mounted root.
    RootInvalid,
    /// Property is not valid for the target kind.
    PropertyInvalid,
    /// Resulting identity, depth, count, or child contract is invalid.
    TreeInvalid,
    /// Batch operation count is outside the fixed limit.
    BatchInvalid,
}

/// Detailed atomic patch rejection.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PatchError {
    /// Ownership check failed.
    #[error("patch owner mismatch")]
    OwnerMismatch,
    /// Sequence ordering failed.
    #[error("invalid patch sequence")]
    SequenceInvalid,
    /// Target identity was not retained.
    #[error("patch target not found: {0}")]
    TargetInvalid(String),
    /// Ordered child index was invalid.
    #[error("patch child index is invalid")]
    IndexInvalid,
    /// Mounted root cannot be removed.
    #[error("mounted root cannot be removed")]
    RootInvalid,
    /// Updated property failed its kind contract.
    #[error("patch property is invalid")]
    PropertyInvalid,
    /// Combined resulting tree failed validation.
    #[error("patched tree is invalid: {0}")]
    TreeInvalid(&'static str),
    /// Batch count failed the protocol ceiling.
    #[error("patch batch operation count is invalid")]
    BatchInvalid,
}

impl PatchError {
    /// Return the stable family for this rejection.
    #[must_use]
    pub const fn code(&self) -> PatchErrorCode {
        match self {
            Self::OwnerMismatch => PatchErrorCode::OwnerMismatch,
            Self::SequenceInvalid => PatchErrorCode::SequenceInvalid,
            Self::TargetInvalid(_) => PatchErrorCode::TargetInvalid,
            Self::IndexInvalid => PatchErrorCode::IndexInvalid,
            Self::RootInvalid => PatchErrorCode::RootInvalid,
            Self::PropertyInvalid => PatchErrorCode::PropertyInvalid,
            Self::TreeInvalid(_) => PatchErrorCode::TreeInvalid,
            Self::BatchInvalid => PatchErrorCode::BatchInvalid,
        }
    }

    /// Convert this rejection to bounded developer guidance without property values.
    #[must_use]
    pub const fn diagnostic(&self) -> crate::UiDiagnostic {
        match self.code() {
            PatchErrorCode::OwnerMismatch => {
                crate::UiDiagnostic::new("owner_mismatch", "patch owner is invalid")
            }
            PatchErrorCode::SequenceInvalid => {
                crate::UiDiagnostic::new("sequence_invalid", "patch sequence must increase")
            }
            PatchErrorCode::TargetInvalid => {
                crate::UiDiagnostic::new("target_invalid", "patch target does not exist")
            }
            PatchErrorCode::IndexInvalid => {
                crate::UiDiagnostic::new("index_invalid", "child index is invalid")
            }
            PatchErrorCode::RootInvalid => {
                crate::UiDiagnostic::new("root_invalid", "mounted root cannot be removed")
            }
            PatchErrorCode::PropertyInvalid => crate::UiDiagnostic::new(
                "property_invalid",
                "property is not valid for this component",
            ),
            PatchErrorCode::TreeInvalid => {
                crate::UiDiagnostic::new("tree_invalid", "resulting UI tree is invalid")
            }
            PatchErrorCode::BatchInvalid => {
                crate::UiDiagnostic::new("batch_invalid", "patch operation count is invalid")
            }
        }
    }
}

/// One successfully committed targeted retained-tree change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommittedChange {
    /// One property changed on one stable node.
    Property {
        /// Stable target identity.
        node_id: String,
        /// Changed property name.
        property: String,
    },
    /// A subtree was inserted under a stable parent.
    Inserted {
        /// Stable parent identity.
        parent_id: String,
        /// New subtree root identity.
        root_id: String,
    },
    /// A retained subtree was removed.
    Removed {
        /// Removed root identity.
        root_id: String,
        /// Every removed local identity.
        removed_ids: Vec<String>,
    },
    /// A retained subtree was reconciled with a replacement.
    Replaced {
        /// Target/replacement root identity.
        root_id: String,
        /// Every identity removed before replacement.
        removed_ids: Vec<String>,
    },
}

/// Audit record returned only after an all-or-none commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchCommit {
    /// Committed batch sequence.
    pub sequence: u64,
    /// Ordered changes corresponding to ordered wire operations.
    pub changes: Vec<CommittedChange>,
}
