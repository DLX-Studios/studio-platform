//! T53 Plugin/template UX — tracked plugin installation and generated
//! tab-group settings surfaces applied through the DesignerSession authority.
//!
//! Installation and every generated settings edit are ordinary tracked project
//! commands, so they share undo, history, actor attribution, and stale-base
//! conflict detection with all other Designer authoring.
use std::collections::BTreeMap;

use studio_design::{
    Actor, BatchConflict, CommandBatch, CommandOutcome, CommandReceipt, DesignerDiagnostic,
    DesignerQuery, DesignerQueryResult, DesignerSession, GeneratedSettingsSurface, OperationId,
    PluginId, ProjectId, RevisionId, SettingKey, SettingValue, SettingsError, SourceProvenance,
    UndoGroupId, plugin_install_batch, setting_change_batch,
};
use studio_plugin_registry::PluginDescriptorV1;
use thiserror::Error;
/// Safe failure modes for plugin installation and generated settings edits.
#[derive(Debug, Error)]
pub enum PluginSurfaceError {
    /// The generated settings edit violated the declared field schema.
    #[error(transparent)]
    Settings(#[from] SettingsError),
    /// The session authority rejected the command batch.
    #[error("plugin command batch was rejected")]
    Rejected(Vec<DesignerDiagnostic>),
    /// The command batch was built on a stale base revision.
    #[error("plugin command batch conflicts with the session authority")]
    Conflict(BatchConflict),
    /// The durable commit behind the command batch failed.
    #[error("plugin command persistence failed: {0}")]
    Persistence(String),
}

/// Session-backed plugin installation and generated settings surface.
#[derive(Debug, Default, Clone, Copy)]
pub struct PluginTemplateUx;

impl PluginTemplateUx {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Build the tracked command batch that installs an admitted plugin.
    #[must_use]
    pub fn install_batch(
        &self,
        descriptor: &PluginDescriptorV1,
        provenance: SourceProvenance,
        project_id: ProjectId,
        base_revision: RevisionId,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandBatch {
        plugin_install_batch(
            project_id,
            descriptor,
            provenance,
            base_revision,
            operation_id,
            actor,
            undo_group_id,
        )
    }

    /// Install an admitted plugin through the session command authority.
    ///
    /// # Errors
    ///
    /// Returns the structured session rejection when the batch fails.
    pub async fn install(
        &self,
        session: &mut dyn DesignerSession,
        descriptor: &PluginDescriptorV1,
        provenance: SourceProvenance,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> Result<CommandReceipt, PluginSurfaceError> {
        let state = session_state(session)?;
        let batch = self.install_batch(
            descriptor,
            provenance,
            state.project_id,
            state.revision_id,
            operation_id,
            actor,
            undo_group_id,
        );
        outcome(session.submit(batch).await)
    }

    /// Render the declared tab-group settings surface for an admitted plugin.
    #[must_use]
    pub fn settings_surface(
        &self,
        descriptor: &PluginDescriptorV1,
        settings: &BTreeMap<SettingKey, SettingValue>,
    ) -> GeneratedSettingsSurface {
        GeneratedSettingsSurface::from_descriptor(
            PluginId::new(descriptor.id.clone()).expect("admitted descriptor has a valid id"),
            descriptor,
            settings,
        )
    }

    /// Build the tracked command batch for one generated settings edit.
    ///
    /// # Errors
    ///
    /// Returns the settings error when the field or value is invalid.
    #[allow(clippy::too_many_arguments)]
    pub fn settings_change_batch(
        &self,
        descriptor: &PluginDescriptorV1,
        key: SettingKey,
        value: Option<SettingValue>,
        project_id: ProjectId,
        base_revision: RevisionId,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> Result<CommandBatch, PluginSurfaceError> {
        Ok(setting_change_batch(
            project_id,
            descriptor,
            key,
            value,
            base_revision,
            operation_id,
            actor,
            undo_group_id,
        )?)
    }

    /// Apply one generated settings edit through the session authority.
    ///
    /// # Errors
    ///
    /// Returns the settings error or the structured session rejection.
    #[allow(clippy::too_many_arguments)]
    pub async fn change_setting(
        &self,
        session: &mut dyn DesignerSession,
        descriptor: &PluginDescriptorV1,
        key: SettingKey,
        value: Option<SettingValue>,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> Result<CommandReceipt, PluginSurfaceError> {
        let state = session_state(session)?;
        let batch = self.settings_change_batch(
            descriptor,
            key,
            value,
            state.project_id,
            state.revision_id,
            operation_id,
            actor,
            undo_group_id,
        )?;
        outcome(session.submit(batch).await)
    }
}

fn session_state(
    session: &dyn DesignerSession,
) -> Result<studio_design::SessionStateSnapshot, PluginSurfaceError> {
    match session.query(DesignerQuery::SessionState) {
        DesignerQueryResult::SessionState(state) => Ok(state),
        _ => unreachable!("DesignerSession returned the wrong query result"),
    }
}

fn outcome(outcome: CommandOutcome) -> Result<CommandReceipt, PluginSurfaceError> {
    match outcome {
        CommandOutcome::Accepted(receipt) => Ok(receipt),
        CommandOutcome::Rejected(diagnostics) => Err(PluginSurfaceError::Rejected(diagnostics)),
        CommandOutcome::Conflict(conflict) => Err(PluginSurfaceError::Conflict(conflict)),
        CommandOutcome::PersistenceFailed(error) => {
            Err(PluginSurfaceError::Persistence(error.to_string()))
        }
    }
}
