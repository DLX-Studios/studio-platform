//! Instance identity and retained-node records.

use std::collections::BTreeMap;

use serde_json::Value;
use studio_protocol::NodeKind;

use crate::MountError;

/// Validated host identity owning exactly one plugin UI namespace.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct InstanceId(String);

impl InstanceId {
    /// Validate an instance identity for use as a registry owner.
    ///
    /// # Errors
    ///
    /// Returns [`MountError::OwnerInvalid`] for empty, oversized, or control-bearing identities.
    pub fn new(value: impl Into<String>) -> Result<Self, MountError> {
        let value = value.into();
        if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
            return Err(MountError::OwnerInvalid);
        }
        Ok(Self(value))
    }

    /// Borrow the stable identity string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One protocol node retained independently of native widget state.
#[derive(Clone, Debug, PartialEq)]
pub struct RetainedNode {
    /// Stable instance-local node identity.
    pub id: String,
    /// Closed protocol-v1 component kind.
    pub kind: NodeKind,
    /// Validated protocol properties.
    pub props: BTreeMap<String, Value>,
    /// Ordered child identities.
    pub children: Vec<String>,
    pub(crate) parent: Option<String>,
}
