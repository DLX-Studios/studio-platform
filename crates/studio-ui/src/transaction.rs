//! Off-registry patch staging and combined-tree validation.

use std::collections::{BTreeMap, HashSet};

use studio_protocol::{PatchBatch, PatchOp, ProtocolLimits, UiNode, validate_node_contract};

use crate::{CommittedChange, InstanceId, PatchCommit, PatchError, RetainedNode, UiRegistry};

pub(crate) fn apply_transaction(
    registry: &mut UiRegistry,
    owner: &InstanceId,
    batch: PatchBatch,
) -> Result<PatchCommit, PatchError> {
    if !registry.owns(owner) {
        return Err(PatchError::OwnerMismatch);
    }
    let (limits, current_root, current_nodes, previous) = registry.transaction_parts();
    let limits = *limits;
    if batch.sequence == 0 || previous.is_some_and(|previous| batch.sequence <= previous) {
        return Err(PatchError::SequenceInvalid);
    }
    if batch.operations.is_empty() || batch.operations.len() > limits.max_patch_operations {
        return Err(PatchError::BatchInvalid);
    }
    let mut root_id = current_root
        .clone()
        .ok_or(PatchError::TreeInvalid("tree is not mounted"))?;
    let mut nodes = current_nodes.clone();
    let mut changes = Vec::with_capacity(batch.operations.len());
    let property_operations = batch
        .operations
        .iter()
        .filter(|operation| matches!(operation, PatchOp::UpdateProp { .. }))
        .count() as u64;
    let structural_operations = batch.operations.len() as u64 - property_operations;
    for operation in batch.operations {
        apply_operation(operation, &mut root_id, &mut nodes, limits, &mut changes)?;
    }
    validate_combined_tree(&root_id, &nodes, limits)?;
    registry.commit_transaction(
        root_id,
        nodes,
        batch.sequence,
        property_operations,
        structural_operations,
    );
    Ok(PatchCommit {
        sequence: batch.sequence,
        changes,
    })
}

fn apply_operation(
    operation: PatchOp,
    root_id: &mut String,
    nodes: &mut BTreeMap<String, RetainedNode>,
    limits: ProtocolLimits,
    changes: &mut Vec<CommittedChange>,
) -> Result<(), PatchError> {
    match operation {
        PatchOp::UpdateProp {
            node_id,
            property,
            value,
        } => {
            let node = nodes
                .get_mut(&node_id)
                .ok_or_else(|| PatchError::TargetInvalid(node_id.clone()))?;
            node.props.insert(property.clone(), value);
            validate_retained_node(node, limits).map_err(|_| PatchError::PropertyInvalid)?;
            changes.push(CommittedChange::Property { node_id, property });
        }
        PatchOp::InsertChild {
            parent_id,
            index,
            node,
        } => {
            let index = usize::try_from(index).map_err(|_| PatchError::IndexInvalid)?;
            let child_count = nodes
                .get(&parent_id)
                .ok_or_else(|| PatchError::TargetInvalid(parent_id.clone()))?
                .children
                .len();
            if index > child_count {
                return Err(PatchError::IndexInvalid);
            }
            let subtree_root = node.id.clone();
            insert_subtree(node, Some(parent_id.clone()), nodes, limits)?;
            nodes
                .get_mut(&parent_id)
                .unwrap()
                .children
                .insert(index, subtree_root.clone());
            changes.push(CommittedChange::Inserted {
                parent_id,
                root_id: subtree_root,
            });
        }
        PatchOp::RemoveNode { node_id } => {
            if node_id == *root_id {
                return Err(PatchError::RootInvalid);
            }
            let (parent, index) = parent_position(&node_id, nodes)?;
            nodes.get_mut(&parent).unwrap().children.remove(index);
            let removed_ids = remove_subtree(&node_id, nodes);
            changes.push(CommittedChange::Removed {
                root_id: node_id,
                removed_ids,
            });
        }
        PatchOp::ReplaceNode { node_id, node } => {
            let replacement_id = node.id.clone();
            let parent_position = if node_id == *root_id {
                None
            } else {
                Some(parent_position(&node_id, nodes)?)
            };
            let removed_ids = remove_subtree(&node_id, nodes);
            let parent = parent_position.as_ref().map(|(parent, _)| parent.clone());
            insert_subtree(node, parent.clone(), nodes, limits)?;
            if let Some((parent, index)) = parent_position {
                nodes.get_mut(&parent).unwrap().children[index].clone_from(&replacement_id);
            } else {
                root_id.clone_from(&replacement_id);
            }
            changes.push(CommittedChange::Replaced {
                root_id: replacement_id,
                removed_ids,
            });
        }
    }
    Ok(())
}

fn insert_subtree(
    root: UiNode,
    parent: Option<String>,
    nodes: &mut BTreeMap<String, RetainedNode>,
    limits: ProtocolLimits,
) -> Result<(), PatchError> {
    let mut staged = Vec::new();
    let mut seen = HashSet::new();
    let mut stack = vec![(root, parent)];
    while let Some((node, parent)) = stack.pop() {
        if nodes.contains_key(&node.id) || !seen.insert(node.id.clone()) {
            return Err(PatchError::TreeInvalid("duplicate node identity"));
        }
        validate_node_contract(&node, limits)
            .map_err(|_| PatchError::TreeInvalid("node contract"))?;
        let child_ids = node.children.iter().map(|child| child.id.clone()).collect();
        for child in node.children.into_iter().rev() {
            stack.push((child, Some(node.id.clone())));
        }
        staged.push(RetainedNode {
            id: node.id,
            kind: node.kind,
            props: node.props,
            children: child_ids,
            parent,
        });
    }
    nodes.extend(staged.into_iter().map(|node| (node.id.clone(), node)));
    Ok(())
}

fn remove_subtree(root_id: &str, nodes: &mut BTreeMap<String, RetainedNode>) -> Vec<String> {
    let mut removed = Vec::new();
    let mut stack = vec![root_id.to_owned()];
    while let Some(node_id) = stack.pop() {
        if let Some(node) = nodes.remove(&node_id) {
            stack.extend(node.children);
            removed.push(node_id);
        }
    }
    removed
}

fn parent_position(
    node_id: &str,
    nodes: &BTreeMap<String, RetainedNode>,
) -> Result<(String, usize), PatchError> {
    let node = nodes
        .get(node_id)
        .ok_or_else(|| PatchError::TargetInvalid(node_id.to_owned()))?;
    let parent = node.parent.clone().ok_or(PatchError::RootInvalid)?;
    let index = nodes
        .get(&parent)
        .and_then(|parent| parent.children.iter().position(|child| child == node_id))
        .ok_or(PatchError::TreeInvalid("parent link"))?;
    Ok((parent, index))
}

fn validate_combined_tree(
    root_id: &str,
    nodes: &BTreeMap<String, RetainedNode>,
    limits: ProtocolLimits,
) -> Result<(), PatchError> {
    if nodes.len() > limits.max_nodes {
        return Err(PatchError::TreeInvalid("node budget"));
    }
    let mut visited = HashSet::new();
    let mut stack = vec![(root_id, 1_usize)];
    while let Some((node_id, depth)) = stack.pop() {
        if depth > limits.max_tree_depth || !visited.insert(node_id) {
            return Err(PatchError::TreeInvalid("depth or cycle"));
        }
        let node = nodes
            .get(node_id)
            .ok_or(PatchError::TreeInvalid("missing child"))?;
        validate_retained_node(node, limits)?;
        for child_id in &node.children {
            let child = nodes
                .get(child_id)
                .ok_or(PatchError::TreeInvalid("missing child"))?;
            if child.parent.as_deref() != Some(node_id) {
                return Err(PatchError::TreeInvalid("parent link"));
            }
            stack.push((child_id, depth + 1));
        }
    }
    if visited.len() != nodes.len() {
        return Err(PatchError::TreeInvalid("unreachable node"));
    }
    Ok(())
}

fn validate_retained_node(node: &RetainedNode, limits: ProtocolLimits) -> Result<(), PatchError> {
    let children = node
        .children
        .iter()
        .map(|id| UiNode {
            id: id.clone(),
            kind: studio_protocol::NodeKind::Text,
            props: BTreeMap::new(),
            children: Vec::new(),
        })
        .collect();
    validate_node_contract(
        &UiNode {
            id: node.id.clone(),
            kind: node.kind,
            props: node.props.clone(),
            children,
        },
        limits,
    )
    .map_err(|_| PatchError::TreeInvalid("node contract"))
}
