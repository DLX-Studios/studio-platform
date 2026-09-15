//! Host-private native identity and interaction state preservation.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use serde_json::Value;
use studio_protocol::NodeKind;
use studio_ui::{CommittedChange, InstanceId, PatchCommit, UiRegistry};

use crate::{UpdateError, UpdateReport};

#[derive(Clone, Debug, PartialEq)]
struct NativeNode {
    identity: u64,
    kind: NodeKind,
    props: BTreeMap<String, Value>,
}

/// Comparable host-private interaction snapshot used by rollback tests and diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeStateSnapshot {
    nodes: BTreeMap<String, NativeNode>,
    focused: Option<String>,
    scroll_offsets: BTreeMap<String, f32>,
    input_buffers: BTreeMap<String, String>,
    component_values: BTreeMap<String, Value>,
}

/// Native state keyed by stable protocol IDs while native widget identities remain host-private.
#[derive(Debug)]
pub struct NativeStateStore {
    owner: InstanceId,
    nodes: BTreeMap<String, NativeNode>,
    focused: Option<String>,
    scroll_offsets: BTreeMap<String, f32>,
    input_buffers: BTreeMap<String, String>,
    component_values: BTreeMap<String, Value>,
    next_identity: u64,
}

impl NativeStateStore {
    /// Create native state for a completely mounted retained registry.
    ///
    /// # Errors
    ///
    /// Returns [`UpdateError::OwnerMismatch`] when registry ownership differs.
    pub fn from_registry(owner: &InstanceId, registry: &UiRegistry) -> Result<Self, UpdateError> {
        let mut next_identity = 1_u64;
        let nodes = registry
            .retained_nodes(owner)
            .map_err(|_| UpdateError::OwnerMismatch)?
            .map(|node| {
                let identity = next_identity;
                next_identity += 1;
                (
                    node.id.clone(),
                    NativeNode {
                        identity,
                        kind: node.kind,
                        props: node.props.clone(),
                    },
                )
            })
            .collect();
        Ok(Self {
            owner: owner.clone(),
            nodes,
            focused: None,
            scroll_offsets: BTreeMap::new(),
            input_buffers: BTreeMap::new(),
            component_values: BTreeMap::new(),
            next_identity,
        })
    }

    /// Apply one committed retained-tree delta without rebuilding unrelated native nodes.
    ///
    /// # Errors
    ///
    /// Returns owner or missing-target failures before changing native state.
    pub fn apply_commit(
        &mut self,
        owner: &InstanceId,
        registry: &UiRegistry,
        commit: &PatchCommit,
    ) -> Result<UpdateReport, UpdateError> {
        self.check_owner(owner)?;
        let retained = registry
            .retained_nodes(owner)
            .map_err(|_| UpdateError::OwnerMismatch)?
            .map(|node| (node.id.clone(), node))
            .collect::<BTreeMap<_, _>>();
        let mut staged = self.snapshot(owner)?;
        let mut staged_next_identity = self.next_identity;
        let mut invalidated = Vec::new();
        let mut unique = BTreeSet::new();
        for change in &commit.changes {
            match change {
                CommittedChange::Property { node_id, .. } => {
                    let source = retained
                        .get(node_id)
                        .ok_or_else(|| UpdateError::NodeNotFound(node_id.clone()))?;
                    let target = staged
                        .nodes
                        .get_mut(node_id)
                        .ok_or_else(|| UpdateError::NodeNotFound(node_id.clone()))?;
                    target.props.clone_from(&source.props);
                    push_unique(&mut invalidated, &mut unique, node_id);
                }
                CommittedChange::Inserted { root_id, .. } => {
                    push_unique(&mut invalidated, &mut unique, root_id);
                }
                CommittedChange::Removed {
                    root_id,
                    removed_ids,
                }
                | CommittedChange::Replaced {
                    root_id,
                    removed_ids,
                } => {
                    for removed in removed_ids {
                        staged.nodes.remove(removed);
                        staged.scroll_offsets.remove(removed);
                        staged.input_buffers.remove(removed);
                        staged
                            .component_values
                            .retain(|key, _| !key.starts_with(&format!("{removed}\u{0}")));
                        if staged.focused.as_deref() == Some(removed) {
                            staged.focused = None;
                        }
                    }
                    push_unique(&mut invalidated, &mut unique, root_id);
                }
            }
        }
        for (node_id, node) in retained {
            if let Entry::Vacant(entry) = staged.nodes.entry(node_id) {
                entry.insert(NativeNode {
                    identity: staged_next_identity,
                    kind: node.kind,
                    props: node.props.clone(),
                });
                staged_next_identity = staged_next_identity.saturating_add(1);
            }
        }
        staged
            .nodes
            .retain(|node_id, _| registry.get(owner, node_id).is_ok());
        self.nodes = staged.nodes;
        self.focused = staged.focused;
        self.scroll_offsets = staged.scroll_offsets;
        self.input_buffers = staged.input_buffers;
        self.component_values = staged.component_values;
        self.next_identity = staged_next_identity;
        Ok(UpdateReport {
            invalidated_nodes: invalidated,
        })
    }

    /// Focus one retained native node.
    ///
    /// # Errors
    ///
    /// Returns owner or missing-node errors.
    pub fn focus(&mut self, owner: &InstanceId, node_id: &str) -> Result<(), UpdateError> {
        self.node(owner, node_id)?;
        self.focused = Some(node_id.to_owned());
        Ok(())
    }

    /// Set host-private scroll state on a scrollable native node.
    ///
    /// # Errors
    ///
    /// Returns owner, node, kind, or non-finite/negative offset errors.
    pub fn set_scroll_offset(
        &mut self,
        owner: &InstanceId,
        node_id: &str,
        offset: f32,
    ) -> Result<(), UpdateError> {
        let kind = self.node(owner, node_id)?.kind;
        if !matches!(kind, NodeKind::ScrollView | NodeKind::ListView)
            || !offset.is_finite()
            || offset < 0.0
        {
            return Err(UpdateError::StateInvalid);
        }
        self.scroll_offsets.insert(node_id.to_owned(), offset);
        Ok(())
    }

    /// Set a host-private non-secret text input buffer.
    ///
    /// # Errors
    ///
    /// Returns owner, node, or kind errors; secret inputs are deliberately excluded.
    pub fn set_input_buffer(
        &mut self,
        owner: &InstanceId,
        node_id: &str,
        value: impl Into<String>,
    ) -> Result<(), UpdateError> {
        if self.node(owner, node_id)?.kind != NodeKind::TextInput {
            return Err(UpdateError::StateInvalid);
        }
        self.input_buffers.insert(node_id.to_owned(), value.into());
        Ok(())
    }

    /// Borrow one protocol property mirrored by a native node.
    ///
    /// # Errors
    ///
    /// Returns owner, node, or property lookup errors.
    pub fn property(
        &self,
        owner: &InstanceId,
        node_id: &str,
        property: &str,
    ) -> Result<&Value, UpdateError> {
        self.node(owner, node_id)?
            .props
            .get(property)
            .ok_or_else(|| UpdateError::NodeNotFound(format!("{node_id}.{property}")))
    }

    /// Return the opaque host-native identity for a stable protocol node.
    ///
    /// # Errors
    ///
    /// Returns owner or missing-node errors.
    pub fn native_identity(&self, owner: &InstanceId, node_id: &str) -> Result<u64, UpdateError> {
        Ok(self.node(owner, node_id)?.identity)
    }

    /// Return the stable focused node identity.
    ///
    /// # Errors
    ///
    /// Returns an ownership mismatch.
    pub fn focused_id(&self, owner: &InstanceId) -> Result<Option<&str>, UpdateError> {
        self.check_owner(owner)?;
        Ok(self.focused.as_deref())
    }

    /// Return host-private scroll state, defaulting to zero.
    ///
    /// # Errors
    ///
    /// Returns owner or missing-node errors.
    pub fn scroll_offset(&self, owner: &InstanceId, node_id: &str) -> Result<f32, UpdateError> {
        self.node(owner, node_id)?;
        Ok(self.scroll_offsets.get(node_id).copied().unwrap_or(0.0))
    }

    /// Return the host-private non-secret input buffer.
    ///
    /// # Errors
    ///
    /// Returns owner or missing-node errors.
    pub fn input_buffer(&self, owner: &InstanceId, node_id: &str) -> Result<&str, UpdateError> {
        self.node(owner, node_id)?;
        Ok(self.input_buffers.get(node_id).map_or("", String::as_str))
    }

    /// Store a host-owned state value for a stateful gpui-component node.
    ///
    /// # Errors
    ///
    /// Returns `UpdateError` if the owner, node, or key is invalid.
    pub fn set_component_value(
        &mut self,
        owner: &InstanceId,
        node_id: &str,
        key: &str,
        value: Value,
    ) -> Result<(), UpdateError> {
        self.node(owner, node_id)?;
        if key.is_empty() || key.len() > 128 {
            return Err(UpdateError::StateInvalid);
        }
        self.component_values
            .insert(format!("{node_id}\u{0}{key}"), value);
        Ok(())
    }

    /// Read a host-owned state value for a stateful gpui-component node.
    ///
    /// # Errors
    ///
    /// Returns `UpdateError` if the owner, node, or key is invalid.
    pub fn component_value(
        &self,
        owner: &InstanceId,
        node_id: &str,
        key: &str,
    ) -> Result<Option<&Value>, UpdateError> {
        self.node(owner, node_id)?;
        Ok(self.component_values.get(&format!("{node_id}\u{0}{key}")))
    }

    /// Clone a comparable snapshot after checking ownership.
    ///
    /// # Errors
    ///
    /// Returns an ownership mismatch.
    pub fn snapshot(&self, owner: &InstanceId) -> Result<NativeStateSnapshot, UpdateError> {
        self.check_owner(owner)?;
        Ok(NativeStateSnapshot {
            nodes: self.nodes.clone(),
            focused: self.focused.clone(),
            scroll_offsets: self.scroll_offsets.clone(),
            input_buffers: self.input_buffers.clone(),
            component_values: self.component_values.clone(),
        })
    }

    fn node(&self, owner: &InstanceId, node_id: &str) -> Result<&NativeNode, UpdateError> {
        self.check_owner(owner)?;
        self.nodes
            .get(node_id)
            .ok_or_else(|| UpdateError::NodeNotFound(node_id.to_owned()))
    }

    fn check_owner(&self, owner: &InstanceId) -> Result<(), UpdateError> {
        if owner != &self.owner {
            return Err(UpdateError::OwnerMismatch);
        }
        Ok(())
    }
}

fn push_unique(output: &mut Vec<String>, seen: &mut BTreeSet<String>, node_id: &str) {
    if seen.insert(node_id.to_owned()) {
        output.push(node_id.to_owned());
    }
}
