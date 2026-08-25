//! Project-scoped usage ledger backing removal-safety reporting.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// One project artifact owned by an installed extension.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "id")]
pub enum OwnedArtifact {
    /// A Reusable Composition instance placed in the project.
    Composition(String),
    /// A settings group whose values are present in the project.
    SettingsGroup(String),
    /// A command bound into the project.
    Command(String),
    /// A declarative action referenced by the project.
    Action(String),
}

impl OwnedArtifact {
    /// Stable identifier of the owned artifact.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Composition(id)
            | Self::SettingsGroup(id)
            | Self::Command(id)
            | Self::Action(id) => id,
        }
    }
}

/// Per-`(project, plugin)` record of artifacts that reference extension contributions.
#[derive(Clone, Debug, Default)]
pub struct ProjectUsage {
    entries: BTreeMap<(String, String), BTreeSet<OwnedArtifact>>,
}

impl ProjectUsage {
    /// Create an empty usage ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one artifact instance referencing a declared contribution.
    pub fn record(&mut self, project_id: &str, plugin_id: &str, artifact: OwnedArtifact) {
        self.entries
            .entry((project_id.to_owned(), plugin_id.to_owned()))
            .or_default()
            .insert(artifact);
    }

    /// Whether one exact artifact is recorded.
    #[must_use]
    pub fn contains(
        &self,
        project_id: &str,
        plugin_id: &str,
        artifact: &OwnedArtifact,
    ) -> bool {
        self.entries
            .get(&(project_id.to_owned(), plugin_id.to_owned()))
            .is_some_and(|artifacts| artifacts.contains(artifact))
    }

    /// All artifacts recorded for one `(project, plugin)` pair in stable order.
    #[must_use]
    pub fn remaining(&self, project_id: &str, plugin_id: &str) -> Vec<OwnedArtifact> {
        self.entries
            .get(&(project_id.to_owned(), plugin_id.to_owned()))
            .map(|artifacts| artifacts.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Drop every recorded artifact for one `(project, plugin)` pair; returns the count.
    pub fn release_all(&mut self, project_id: &str, plugin_id: &str) -> usize {
        self.entries
            .remove(&(project_id.to_owned(), plugin_id.to_owned()))
            .map(|artifacts| artifacts.len())
            .unwrap_or(0)
    }
}
