//! Instance-owned retained registry with atomic initial mount commit.

use std::collections::BTreeMap;

use studio_protocol::{MountTree, PatchBatch, ProtocolLimits};

use crate::{
    InstanceId, MountError, PatchCommit, PatchError, RetainedNode, mount::stage_mount,
    transaction::apply_transaction,
};

/// Comparable host snapshot used for audit and transaction invariants.
#[derive(Clone, Debug, PartialEq)]
pub struct RegistrySnapshot {
    owner: InstanceId,
    route: String,
    root_id: String,
    nodes: BTreeMap<String, RetainedNode>,
    last_sequence: Option<u64>,
    metrics: PatchMetrics,
}

/// Monotonic instrumentation for successfully committed retained-tree work.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PatchMetrics {
    /// Successfully committed atomic batches.
    pub committed_batches: u64,
    /// Committed property operations.
    pub property_operations: u64,
    /// Committed insert, remove, and replace operations.
    pub structural_operations: u64,
}

/// Retained protocol tree owned by exactly one active plugin instance.
#[derive(Debug)]
pub struct UiRegistry {
    owner: InstanceId,
    limits: ProtocolLimits,
    route: Option<String>,
    root_id: Option<String>,
    nodes: BTreeMap<String, RetainedNode>,
    last_sequence: Option<u64>,
    metrics: PatchMetrics,
}

impl UiRegistry {
    /// Create an empty registry for one validated owner.
    #[must_use]
    pub fn new(owner: InstanceId, limits: ProtocolLimits) -> Self {
        Self {
            owner,
            limits,
            route: None,
            root_id: None,
            nodes: BTreeMap::new(),
            last_sequence: None,
            metrics: PatchMetrics::default(),
        }
    }

    /// Validate a complete initial tree off-registry and commit it all at once.
    ///
    /// # Errors
    ///
    /// Returns [`MountError::AlreadyMounted`] after the first successful mount, or a validation
    /// error without changing any retained state.
    pub fn mount(&mut self, mount: MountTree) -> Result<(), MountError> {
        if self.root_id.is_some() {
            return Err(MountError::AlreadyMounted);
        }
        let staged = stage_mount(mount, self.limits)?;
        self.route = Some(staged.route);
        self.root_id = Some(staged.root_id);
        self.nodes = staged.nodes;
        Ok(())
    }

    /// Number of retained protocol nodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether no tree has been committed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Root identity of the committed tree, if mounted.
    #[must_use]
    pub fn root_id(&self) -> Option<&str> {
        self.root_id.as_deref()
    }

    /// Sequence of the last atomically committed patch batch.
    #[must_use]
    pub const fn last_sequence(&self) -> Option<u64> {
        self.last_sequence
    }

    /// Return monotonic counters for committed patch work.
    #[must_use]
    pub const fn patch_metrics(&self) -> PatchMetrics {
        self.metrics
    }

    /// Stage, validate, and atomically commit a complete ordered patch batch.
    ///
    /// # Errors
    ///
    /// Returns [`PatchError`] without changing retained state or sequence when any operation or
    /// the combined resulting tree is invalid.
    pub fn apply_patch(
        &mut self,
        owner: &InstanceId,
        batch: PatchBatch,
    ) -> Result<PatchCommit, PatchError> {
        apply_transaction(self, owner, batch)
    }

    /// Look up a retained node after checking the caller's instance identity.
    ///
    /// # Errors
    ///
    /// Returns owner mismatch or missing-node errors without exposing another namespace.
    pub fn get(&self, owner: &InstanceId, node_id: &str) -> Result<&RetainedNode, MountError> {
        self.check_owner(owner)?;
        self.nodes
            .get(node_id)
            .ok_or_else(|| MountError::NodeNotFound(node_id.to_owned()))
    }

    /// Iterate all retained nodes after checking the instance namespace.
    ///
    /// # Errors
    ///
    /// Returns [`MountError::OwnerMismatch`] for a caller outside this registry's owner.
    pub fn retained_nodes(
        &self,
        owner: &InstanceId,
    ) -> Result<impl Iterator<Item = &RetainedNode>, MountError> {
        self.check_owner(owner)?;
        Ok(self.nodes.values())
    }

    /// Return the retained parent identity for a node.
    ///
    /// # Errors
    ///
    /// Returns owner mismatch or missing-node errors.
    pub fn parent_id(&self, owner: &InstanceId, node_id: &str) -> Result<Option<&str>, MountError> {
        Ok(self.get(owner, node_id)?.parent.as_deref())
    }

    /// Clone a stable snapshot after checking ownership.
    ///
    /// # Errors
    ///
    /// Returns owner mismatch when called outside this registry's instance namespace, or a tree
    /// error if no mount has committed.
    pub fn snapshot(&self, owner: &InstanceId) -> Result<RegistrySnapshot, MountError> {
        self.check_owner(owner)?;
        Ok(RegistrySnapshot {
            owner: self.owner.clone(),
            route: self
                .route
                .clone()
                .ok_or(MountError::TreeInvalid("tree is not mounted"))?,
            root_id: self
                .root_id
                .clone()
                .ok_or(MountError::TreeInvalid("tree is not mounted"))?,
            nodes: self.nodes.clone(),
            last_sequence: self.last_sequence,
            metrics: self.metrics,
        })
    }

    fn check_owner(&self, owner: &InstanceId) -> Result<(), MountError> {
        if owner != &self.owner {
            return Err(MountError::OwnerMismatch);
        }
        Ok(())
    }

    pub(crate) fn owns(&self, owner: &InstanceId) -> bool {
        owner == &self.owner
    }

    pub(crate) fn transaction_parts(
        &self,
    ) -> (
        &ProtocolLimits,
        &Option<String>,
        &BTreeMap<String, RetainedNode>,
        Option<u64>,
    ) {
        (&self.limits, &self.root_id, &self.nodes, self.last_sequence)
    }

    pub(crate) fn commit_transaction(
        &mut self,
        root_id: String,
        nodes: BTreeMap<String, RetainedNode>,
        sequence: u64,
        property_operations: u64,
        structural_operations: u64,
    ) {
        self.root_id = Some(root_id);
        self.nodes = nodes;
        self.last_sequence = Some(sequence);
        self.metrics.committed_batches = self.metrics.committed_batches.saturating_add(1);
        self.metrics.property_operations = self
            .metrics
            .property_operations
            .saturating_add(property_operations);
        self.metrics.structural_operations = self
            .metrics
            .structural_operations
            .saturating_add(structural_operations);
    }
}
