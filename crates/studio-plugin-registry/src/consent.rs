//! Per-project, revocable consent for requested plugin capabilities.
//!
//! Consent is deny-by-default: nothing is granted until an explicit user decision is
//! recorded, every grant is scoped to `(project, plugin, capability)`, and revocation takes
//! effect immediately by deactivating active extensions.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::descriptor::DeclaredCapability;

/// One recorded grant decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConsentDecision {
    /// The user explicitly allowed this capability for this project.
    Granted,
    /// The user explicitly denied this capability for this project.
    Denied,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ConsentKey {
    project_id: String,
    plugin_id: String,
    capability: DeclaredCapability,
}

/// Deny-by-default consent ledger scoped per project.
#[derive(Clone, Debug, Default)]
pub struct ConsentLedger {
    decisions: BTreeMap<ConsentKey, ConsentDecision>,
}

impl ConsentLedger {
    /// Create an empty ledger where everything is denied.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one explicit grant.
    pub fn grant(&mut self, project_id: &str, plugin_id: &str, capability: DeclaredCapability) {
        self.decisions.insert(
            ConsentKey {
                project_id: project_id.to_owned(),
                plugin_id: plugin_id.to_owned(),
                capability,
            },
            ConsentDecision::Granted,
        );
    }

    /// Record one explicit denial.
    pub fn deny(&mut self, project_id: &str, plugin_id: &str, capability: DeclaredCapability) {
        self.decisions.insert(
            ConsentKey {
                project_id: project_id.to_owned(),
                plugin_id: plugin_id.to_owned(),
                capability,
            },
            ConsentDecision::Denied,
        );
    }

    /// Revoke any recorded decision; the capability returns to its denied default.
    ///
    /// Returns whether a decision existed.
    pub fn revoke(
        &mut self,
        project_id: &str,
        plugin_id: &str,
        capability: DeclaredCapability,
    ) -> bool {
        self.decisions
            .remove(&ConsentKey {
                project_id: project_id.to_owned(),
                plugin_id: plugin_id.to_owned(),
                capability,
            })
            .is_some()
    }

    /// Drop every decision recorded for one plugin across all projects.
    pub fn revoke_plugin(&mut self, plugin_id: &str) {
        self.decisions.retain(|key, _| key.plugin_id != plugin_id);
    }

    /// Drop every decision recorded for one plugin in one project.
    pub fn revoke_project_plugin(&mut self, project_id: &str, plugin_id: &str) {
        self.decisions
            .retain(|key, _| key.project_id != project_id || key.plugin_id != plugin_id);
    }

    /// Recorded decision for one capability, if the user made one.
    #[must_use]
    pub fn decision(
        &self,
        project_id: &str,
        plugin_id: &str,
        capability: DeclaredCapability,
    ) -> Option<ConsentDecision> {
        self.decisions
            .get(&ConsentKey {
                project_id: project_id.to_owned(),
                plugin_id: plugin_id.to_owned(),
                capability,
            })
            .copied()
    }

    /// Whether one capability is currently granted; absent decisions are denied.
    #[must_use]
    pub fn is_granted(
        &self,
        project_id: &str,
        plugin_id: &str,
        capability: DeclaredCapability,
    ) -> bool {
        self.decisions.get(&ConsentKey {
            project_id: project_id.to_owned(),
            plugin_id: plugin_id.to_owned(),
            capability,
        }) == Some(&ConsentDecision::Granted)
    }

    /// All currently granted capabilities for one `(project, plugin)` pair.
    #[must_use]
    pub fn granted_for(&self, project_id: &str, plugin_id: &str) -> BTreeSet<DeclaredCapability> {
        self.decisions
            .iter()
            .filter(|(key, decision)| {
                key.project_id == project_id
                    && key.plugin_id == plugin_id
                    && *decision == ConsentDecision::Granted
            })
            .map(|(key, _)| key.capability)
            .collect()
    }
}
