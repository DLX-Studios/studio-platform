//! Complete pre-commit mount validation.

use std::collections::{BTreeMap, HashSet};

use studio_protocol::{MountTree, NodeKind, PROTOCOL_VERSION, ProtocolLimits, UiNode};
use thiserror::Error;

use crate::RetainedNode;

/// Stable retained-tree mount rejection family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MountErrorCode {
    /// Instance identity is malformed.
    OwnerInvalid,
    /// Registry access used a different instance identity.
    OwnerMismatch,
    /// The instance already owns a mounted tree.
    AlreadyMounted,
    /// A requested retained node does not exist.
    NodeNotFound,
    /// Tree identity, shape, route, version, count, or depth is invalid.
    TreeInvalid,
    /// A property crosses the native semantic property boundary.
    PropertyInvalid,
}

/// Detailed retained-tree rejection.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum MountError {
    /// Instance identity cannot be admitted.
    #[error("invalid instance identity")]
    OwnerInvalid,
    /// Registry ownership check failed.
    #[error("UI registry owner mismatch")]
    OwnerMismatch,
    /// Initial mount may occur only once.
    #[error("UI tree is already mounted")]
    AlreadyMounted,
    /// Node lookup failed.
    #[error("retained node not found: {0}")]
    NodeNotFound(String),
    /// Complete tree validation failed.
    #[error("invalid mount tree: {0}")]
    TreeInvalid(&'static str),
    /// Node property validation failed.
    #[error("invalid property {property} on node {node_id}")]
    PropertyInvalid {
        /// Target node identity.
        node_id: String,
        /// Rejected property name.
        property: String,
    },
}

impl MountError {
    /// Return the stable family for this mount rejection.
    #[must_use]
    pub const fn code(&self) -> MountErrorCode {
        match self {
            Self::OwnerInvalid => MountErrorCode::OwnerInvalid,
            Self::OwnerMismatch => MountErrorCode::OwnerMismatch,
            Self::AlreadyMounted => MountErrorCode::AlreadyMounted,
            Self::NodeNotFound(_) => MountErrorCode::NodeNotFound,
            Self::TreeInvalid(_) => MountErrorCode::TreeInvalid,
            Self::PropertyInvalid { .. } => MountErrorCode::PropertyInvalid,
        }
    }
}

pub(crate) struct StagedMount {
    pub(crate) route: String,
    pub(crate) root_id: String,
    pub(crate) nodes: BTreeMap<String, RetainedNode>,
}

pub(crate) fn stage_mount(
    mount: MountTree,
    limits: ProtocolLimits,
) -> Result<StagedMount, MountError> {
    if mount.protocol_version != PROTOCOL_VERSION {
        return Err(MountError::TreeInvalid("protocol version"));
    }
    if !valid_route(&mount.route) {
        return Err(MountError::TreeInvalid("route"));
    }

    let root_id = mount.root.id.clone();
    let mut ids = HashSet::new();
    let mut nodes = BTreeMap::new();
    let mut stack = vec![(mount.root, None, 1_usize)];
    while let Some((node, parent, depth)) = stack.pop() {
        if depth > limits.max_tree_depth || nodes.len() >= limits.max_nodes {
            return Err(MountError::TreeInvalid("tree budget"));
        }
        validate_node(&node, limits)?;
        if !ids.insert(node.id.clone()) {
            return Err(MountError::TreeInvalid("duplicate node identity"));
        }

        let child_ids = node.children.iter().map(|child| child.id.clone()).collect();
        let node_id = node.id.clone();
        for child in node.children.into_iter().rev() {
            stack.push((child, Some(node_id.clone()), depth + 1));
        }
        nodes.insert(
            node_id.clone(),
            RetainedNode {
                id: node_id,
                kind: node.kind,
                props: node.props,
                children: child_ids,
                parent,
            },
        );
    }
    Ok(StagedMount {
        route: mount.route,
        root_id,
        nodes,
    })
}

fn validate_node(node: &UiNode, limits: ProtocolLimits) -> Result<(), MountError> {
    if node.id.is_empty() || node.id.len() > limits.max_node_id_bytes {
        return Err(MountError::TreeInvalid("node identity"));
    }
    for (property, value) in &node.props {
        if property.is_empty()
            || property.len() > 128
            || forbidden_property(property)
            || (node.kind == NodeKind::SecretInput && property == "value")
            || json_string_exceeds(value, limits.max_string_bytes)
        {
            return Err(MountError::PropertyInvalid {
                node_id: node.id.clone(),
                property: property.clone(),
            });
        }
    }
    Ok(())
}

fn forbidden_property(property: &str) -> bool {
    matches!(
        property,
        "html"
            | "css"
            | "class"
            | "class_name"
            | "native_class"
            | "raw_draw"
            | "shader"
            | "device_control"
    )
}

fn json_string_exceeds(value: &serde_json::Value, limit: usize) -> bool {
    match value {
        serde_json::Value::String(value) => value.len() > limit,
        serde_json::Value::Array(values) => {
            values.iter().any(|value| json_string_exceeds(value, limit))
        }
        serde_json::Value::Object(values) => values
            .iter()
            .any(|(key, value)| key.len() > limit || json_string_exceeds(value, limit)),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            false
        }
    }
}

fn valid_route(route: &str) -> bool {
    route.starts_with('/')
        && !route.contains(['\\', '\0', '?', '#'])
        && !route.chars().any(char::is_control)
        && (route == "/" || !route.ends_with('/'))
        && !route
            .split('/')
            .any(|segment| matches!(segment, "." | ".."))
}
