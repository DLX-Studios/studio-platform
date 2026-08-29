//! Extension registry: signed admission, consent-gated activation, bounded lifecycle, and
//! removal-safe teardown.

use std::collections::{BTreeSet, HashMap};

use serde_json::Value;
use studio_package::{TrustStore, VerifiedIntegrity, verify_document_signature};

use crate::consent::{ConsentDecision, ConsentLedger};
use crate::descriptor::{
    CompositionNode, DeclaredCapability, DescriptorPolicy, LifecycleHook, PluginDescriptorV1,
    SignedDescriptorEnvelope,
};
use crate::error::{DescriptorError, DescriptorErrorCode, RegistryError};
use crate::lifecycle::{HookRunner, PluginState, ViolationReason, ViolationRecord};
use crate::removal::{OwnedArtifact, ProjectUsage};

/// Primitive kinds third-party composition trees may reference.
///
/// Anything outside this set is rejected structurally; the closed descriptor schema offers no
/// other path to introduce renderer kinds.
pub const DEFAULT_PRIMITIVE_CATALOG: &[&str] = &[
    "box",
    "column",
    "row",
    "stack",
    "grid",
    "scroll_view",
    "list_view",
    "spacer",
    "divider",
    "text",
    "icon",
    "image",
    "card",
    "badge",
    "tag",
    "avatar",
    "empty",
    "skeleton",
    "button",
    "checkbox",
    "switch",
    "select",
    "text_input",
    "number_input",
    "field",
    "dialog",
];

/// Host-approved primitive kind catalog for contributed composition trees.
#[derive(Clone, Debug, Default)]
pub struct ApprovedKindCatalog {
    kinds: BTreeSet<String>,
}

impl ApprovedKindCatalog {
    /// Build a catalog from explicit kind names.
    #[must_use]
    pub fn from_kinds(kinds: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        Self {
            kinds: kinds
                .into_iter()
                .map(|kind| kind.as_ref().to_owned())
                .collect(),
        }
    }

    /// The shipped first-party default catalog.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::from_kinds(DEFAULT_PRIMITIVE_CATALOG.iter().copied())
    }

    /// Whether one kind may be referenced by contributed trees.
    #[must_use]
    pub fn contains(&self, kind: &str) -> bool {
        self.kinds.contains(kind)
    }
}

/// One admitted extension with its verified integrity evidence.
#[derive(Clone, Debug)]
pub struct AdmittedExtension {
    /// Validated typed descriptor.
    pub descriptor: PluginDescriptorV1,
    /// Verification evidence retained for audit.
    pub integrity: VerifiedIntegrity,
    /// Current lifecycle state.
    pub state: PluginState,
}

/// Removal report produced before any project mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemovalReport {
    /// Project whose artifacts were audited.
    pub project_id: String,
    /// Extension that owns the artifacts.
    pub plugin_id: String,
    /// Artifacts still referencing this extension's contributions.
    pub remaining_artifacts: Vec<OwnedArtifact>,
}

/// Registry coordinating admission, consent, lifecycle, and removal.
#[derive(Debug)]
pub struct ExtensionRegistry {
    policy: DescriptorPolicy,
    trust: TrustStore,
    approved_kinds: ApprovedKindCatalog,
    admitted: HashMap<String, AdmittedExtension>,
    installed: BTreeSet<(String, String)>,
    active: BTreeSet<(String, String)>,
    consents: ConsentLedger,
    usage: ProjectUsage,
    hooks: HookRunner,
    violations: Vec<ViolationRecord>,
    pending_removals: HashMap<(String, String), RemovalReport>,
}

impl ExtensionRegistry {
    /// Create a registry over one trust snapshot and host policy.
    #[must_use]
    pub fn new(
        policy: DescriptorPolicy,
        trust: TrustStore,
        approved_kinds: ApprovedKindCatalog,
    ) -> Self {
        Self {
            policy,
            trust,
            approved_kinds,
            admitted: HashMap::new(),
            installed: BTreeSet::new(),
            active: BTreeSet::new(),
            consents: ConsentLedger::new(),
            usage: ProjectUsage::new(),
            hooks: HookRunner::default(),
            violations: Vec::new(),
            pending_removals: HashMap::new(),
        }
    }

    /// Admit one signed descriptor envelope after full verification.
    ///
    /// Pipeline: envelope/signature attribution match -> Ed25519 verification against the
    /// provisioned trust store (reusing bundle trust machinery with a document-domain
    /// signature) -> closed-schema parsing and validation -> compatibility check ->
    /// structural contribution-kind approval -> bounded admission hook. Re-admitting an id
    /// replaces the prior descriptor while preserving host-wired lifecycle callbacks.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError`] for every rejection family. Failed admission never changes
    /// admitted/project state; a contained admission-hook violation is appended to the audit log.
    pub fn admit(&mut self, envelope: &SignedDescriptorEnvelope) -> Result<(), RegistryError> {
        let publisher_id = envelope
            .descriptor
            .get("publisher")
            .and_then(|publisher| publisher.get("id"))
            .and_then(Value::as_str);
        let key_id = envelope
            .descriptor
            .get("publisher")
            .and_then(|publisher| publisher.get("keyId"))
            .and_then(Value::as_str);
        if publisher_id != Some(envelope.signature.publisher_id.as_str())
            || key_id != Some(envelope.signature.key_id.as_str())
        {
            return Err(RegistryError::admission_signature_invalid(
                "signature attribution does not match descriptor publisher",
            ));
        }
        let raw_signature = envelope
            .signature_bytes()
            .map_err(|error| RegistryError::admission_signature_invalid(error.detail()))?;
        let verified = verify_document_signature(
            &envelope.descriptor,
            &raw_signature,
            &envelope.signature.publisher_id,
            &envelope.signature.key_id,
            &self.trust,
        )
        .map_err(|error| RegistryError::admission_signature_invalid(error.to_string()))?;
        let descriptor = validate_for_admission(&envelope.descriptor, &self.policy)?;
        validate_kind_references(&descriptor, &self.approved_kinds)?;
        if let Some(budget) = descriptor.hook_budget(LifecycleHook::Admission)
            && let Err(reason) =
                self.hooks
                    .dispatch(&descriptor.id, "", LifecycleHook::Admission, budget)
        {
            self.violations.push(ViolationRecord {
                plugin_id: descriptor.id.clone(),
                hook: LifecycleHook::Admission,
                reason: reason.clone(),
            });
            return Err(RegistryError::hook_violation(
                LifecycleHook::Admission,
                &reason,
            ));
        }
        let state = self
            .admitted
            .get(&descriptor.id)
            .map_or(PluginState::Admitted, |extension| extension.state);
        self.admitted.insert(
            descriptor.id.clone(),
            AdmittedExtension {
                descriptor,
                integrity: verified,
                state,
            },
        );
        Ok(())
    }

    /// Borrowed view of one admitted extension.
    #[must_use]
    pub fn plugin(&self, plugin_id: &str) -> Option<&AdmittedExtension> {
        self.admitted.get(plugin_id)
    }

    /// Return admitted extensions in stable plugin-id order for Designer browsing.
    #[must_use]
    pub fn admitted_extensions(&self) -> Vec<&AdmittedExtension> {
        let mut extensions = self.admitted.values().collect::<Vec<_>>();
        extensions.sort_by(|left, right| left.descriptor.id.cmp(&right.descriptor.id));
        extensions
    }
    /// Record one explicit consent grant for a declared capability.
    ///
    /// # Errors
    ///
    /// Fails when the plugin is not admitted or never declared the capability.
    pub fn grant_consent(
        &mut self,
        project_id: &str,
        plugin_id: &str,
        capability: DeclaredCapability,
    ) -> Result<(), RegistryError> {
        let extension = self
            .admitted
            .get(plugin_id)
            .ok_or_else(RegistryError::unknown_plugin)?;
        if !extension.descriptor.capabilities.contains(&capability) {
            return Err(RegistryError::consent_denied(format!(
                "{plugin_id} never declared {}",
                capability.name()
            )));
        }
        self.consents.grant(project_id, plugin_id, capability);
        Ok(())
    }

    /// Record one explicit consent denial.
    ///
    /// # Errors
    ///
    /// Fails when the plugin is not admitted or never declared the capability.
    pub fn deny_consent(
        &mut self,
        project_id: &str,
        plugin_id: &str,
        capability: DeclaredCapability,
    ) -> Result<(), RegistryError> {
        let extension = self
            .admitted
            .get(plugin_id)
            .ok_or_else(RegistryError::unknown_plugin)?;
        if !extension.descriptor.capabilities.contains(&capability) {
            return Err(RegistryError::consent_denied(format!(
                "{plugin_id} never declared {}",
                capability.name()
            )));
        }
        self.consents.deny(project_id, plugin_id, capability);
        if self
            .active
            .remove(&(project_id.to_owned(), plugin_id.to_owned()))
        {
            self.run_hook(project_id, plugin_id, LifecycleHook::Deactivate)?;
            self.refresh_state(plugin_id);
        }
        Ok(())
    }

    /// Inspect the explicit consent decision for one project capability.
    #[must_use]
    pub fn consent_decision(
        &self,
        project_id: &str,
        plugin_id: &str,
        capability: DeclaredCapability,
    ) -> Option<ConsentDecision> {
        self.consents.decision(project_id, plugin_id, capability)
    }

    /// Revoke a recorded consent decision; deactivates the extension for the project.
    ///
    /// # Errors
    ///
    /// Fails when the plugin is unknown or its deactivate hook must be contained.
    pub fn revoke_consent(
        &mut self,
        project_id: &str,
        plugin_id: &str,
        capability: DeclaredCapability,
    ) -> Result<bool, RegistryError> {
        if !self.admitted.contains_key(plugin_id) {
            return Err(RegistryError::unknown_plugin());
        }
        let revoked = self.consents.revoke(project_id, plugin_id, capability);
        if revoked
            && self
                .active
                .remove(&(project_id.to_owned(), plugin_id.to_owned()))
        {
            self.run_hook(project_id, plugin_id, LifecycleHook::Deactivate)?;
            self.refresh_state(plugin_id);
        }
        Ok(revoked)
    }

    /// Install an admitted extension into a project.
    ///
    /// # Errors
    ///
    /// Fails for unknown plugins, quarantined plugins, re-installation, or contained hooks.
    pub fn install(&mut self, project_id: &str, plugin_id: &str) -> Result<(), RegistryError> {
        self.check_runnable(plugin_id)?;
        let pair = (project_id.to_owned(), plugin_id.to_owned());
        if self.installed.contains(&pair) {
            return Err(RegistryError::state_invalid(format!(
                "{plugin_id} already installed into {project_id}"
            )));
        }
        self.run_hook(project_id, plugin_id, LifecycleHook::Install)?;
        self.installed.insert(pair);
        if let Some(extension) = self.admitted.get_mut(plugin_id) {
            extension.state = PluginState::Installed;
        }
        Ok(())
    }

    /// Activate an installed extension for a project after checking recorded consent.
    ///
    /// Every requested capability must have a granted decision; anything missing keeps the
    /// extension inactive (deny-by-default).
    ///
    /// # Errors
    ///
    /// Fails for unknown/uninstalled/quarantined plugins, missing consent, or contained hooks.
    pub fn activate(&mut self, project_id: &str, plugin_id: &str) -> Result<(), RegistryError> {
        self.check_runnable(plugin_id)?;
        let pair = (project_id.to_owned(), plugin_id.to_owned());
        if !self.installed.contains(&pair) {
            return Err(RegistryError::state_invalid(format!(
                "{plugin_id} is not installed into {project_id}"
            )));
        }
        let granted = self.consents.granted_for(project_id, plugin_id);
        let unconsented = self
            .admitted
            .get(plugin_id)
            .ok_or_else(RegistryError::unknown_plugin)?
            .descriptor
            .unconsented_capabilities(&granted);
        if !unconsented.is_empty() {
            let names = unconsented
                .iter()
                .map(|capability| capability.name())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(RegistryError::consent_denied(format!(
                "missing consent for {names}"
            )));
        }
        self.run_hook(project_id, plugin_id, LifecycleHook::Activate)?;
        self.active.insert(pair);
        if let Some(extension) = self.admitted.get_mut(plugin_id) {
            extension.state = PluginState::Active;
        }
        Ok(())
    }

    /// Run the project-open hook for an active extension.
    ///
    /// # Errors
    ///
    /// Fails unless the extension is active for the project, or the hook must be contained.
    pub fn project_open(&mut self, project_id: &str, plugin_id: &str) -> Result<(), RegistryError> {
        if !self
            .active
            .contains(&(project_id.to_owned(), plugin_id.to_owned()))
        {
            return Err(RegistryError::state_invalid(format!(
                "{plugin_id} is not active for {project_id}"
            )));
        }
        self.run_hook(project_id, plugin_id, LifecycleHook::ProjectOpen)
    }

    /// Deactivate an active extension for a project.
    ///
    /// # Errors
    ///
    /// Fails unless the extension is active, or the hook must be contained.
    pub fn deactivate(&mut self, project_id: &str, plugin_id: &str) -> Result<(), RegistryError> {
        if !self
            .active
            .remove(&(project_id.to_owned(), plugin_id.to_owned()))
        {
            return Err(RegistryError::state_invalid(format!(
                "{plugin_id} is not active for {project_id}"
            )));
        }
        self.run_hook(project_id, plugin_id, LifecycleHook::Deactivate)?;
        self.refresh_state(plugin_id);
        Ok(())
    }

    /// Record that a project artifact references one declared contribution.
    ///
    /// # Errors
    ///
    /// Fails when the plugin is unknown or the artifact was never declared by it.
    pub fn record_usage(
        &mut self,
        project_id: &str,
        plugin_id: &str,
        artifact: OwnedArtifact,
    ) -> Result<(), RegistryError> {
        let extension = self
            .admitted
            .get(plugin_id)
            .ok_or_else(RegistryError::unknown_plugin)?;
        if !self
            .installed
            .contains(&(project_id.to_owned(), plugin_id.to_owned()))
        {
            return Err(RegistryError::state_invalid(format!(
                "{plugin_id} is not installed into {project_id}"
            )));
        }
        let declared = match &artifact {
            OwnedArtifact::Composition(id) => extension.descriptor.declares_composition(id),
            OwnedArtifact::SettingsGroup(id) => extension.descriptor.declares_settings_group(id),
            OwnedArtifact::Command(id) => extension.descriptor.declares_command(id),
            OwnedArtifact::Action(id) => extension.descriptor.declares_action(id),
        };
        if !declared {
            return Err(RegistryError::usage_unknown(format!(
                "{plugin_id} never declared artifact `{}`",
                artifact.id()
            )));
        }
        self.usage.record(project_id, plugin_id, artifact);
        Ok(())
    }

    /// Audit remaining owned artifacts without mutating anything.
    ///
    /// The report is stored as the pending removal plan required by
    /// [`ExtensionRegistry::complete_removal`].
    ///
    /// # Errors
    ///
    /// Fails when the plugin is not admitted.
    pub fn plan_removal(
        &mut self,
        project_id: &str,
        plugin_id: &str,
    ) -> Result<RemovalReport, RegistryError> {
        if !self.admitted.contains_key(plugin_id) {
            return Err(RegistryError::unknown_plugin());
        }
        if !self
            .installed
            .contains(&(project_id.to_owned(), plugin_id.to_owned()))
        {
            return Err(RegistryError::state_invalid(format!(
                "{plugin_id} is not installed into {project_id}"
            )));
        }
        let report = RemovalReport {
            project_id: project_id.to_owned(),
            plugin_id: plugin_id.to_owned(),
            remaining_artifacts: self.usage.remaining(project_id, plugin_id),
        };
        self.pending_removals.insert(
            (project_id.to_owned(), plugin_id.to_owned()),
            report.clone(),
        );
        Ok(report)
    }

    /// Complete a planned removal after reporting remaining artifacts.
    ///
    /// With `force = false` the removal refuses to mutate while owned artifacts remain; the
    /// caller resolves them and re-plans. With `force = true` the report stands as the
    /// pre-mutation disclosure and teardown proceeds anyway.
    ///
    /// # Errors
    ///
    /// Fails without a matching fresh plan, when blocked and not forced, or on hook violation.
    pub fn complete_removal(
        &mut self,
        project_id: &str,
        plugin_id: &str,
        force: bool,
    ) -> Result<RemovalReport, RegistryError> {
        let pair = (project_id.to_owned(), plugin_id.to_owned());
        if !self.pending_removals.contains_key(&pair) {
            return Err(RegistryError::removal_plan_invalid(
                "call plan_removal before completing removal",
            ));
        }
        let report = self
            .pending_removals
            .get(&pair)
            .cloned()
            .ok_or_else(|| RegistryError::removal_plan_invalid("plan vanished"))?;
        if self.usage.remaining(project_id, plugin_id) != report.remaining_artifacts {
            return Err(RegistryError::removal_plan_invalid(
                "project usage changed after the removal report; re-plan removal",
            ));
        }
        if !force && !report.remaining_artifacts.is_empty() {
            return Err(RegistryError::removal_blocked(
                report.remaining_artifacts.len(),
            ));
        }
        if self.active.remove(&pair) {
            self.run_hook(project_id, plugin_id, LifecycleHook::Deactivate)?;
        }
        self.run_hook(project_id, plugin_id, LifecycleHook::Remove)?;
        self.usage.release_all(project_id, plugin_id);
        self.consents.revoke_project_plugin(project_id, plugin_id);
        self.installed.remove(&pair);
        self.pending_removals.remove(&pair);
        self.refresh_state(plugin_id);
        Ok(report)
    }

    /// Recorded containment events in order.
    #[must_use]
    pub fn violations(&self) -> &[ViolationRecord] {
        &self.violations
    }

    /// Mutable hook callback table for hosts wiring lifecycle handlers.
    pub fn hooks_mut(&mut self) -> &mut HookRunner {
        &mut self.hooks
    }

    /// Drop every recorded usage entry for one `(project, plugin)` pair.
    ///
    /// Returns the number of released artifacts. Hosts call this after the Designer removes
    /// or migrates the referencing nodes, bindings, and settings values.
    pub fn clear_usage(&mut self, project_id: &str, plugin_id: &str) -> usize {
        self.usage.release_all(project_id, plugin_id)
    }

    fn check_runnable(&self, plugin_id: &str) -> Result<(), RegistryError> {
        let extension = self
            .admitted
            .get(plugin_id)
            .ok_or_else(RegistryError::unknown_plugin)?;
        if extension.state == PluginState::Quarantined {
            return Err(RegistryError::state_invalid(format!(
                "{plugin_id} is quarantined after a hook-budget violation"
            )));
        }
        Ok(())
    }

    fn run_hook(
        &mut self,
        project_id: &str,
        plugin_id: &str,
        hook: LifecycleHook,
    ) -> Result<(), RegistryError> {
        self.check_runnable(plugin_id)?;
        let budget = self
            .admitted
            .get(plugin_id)
            .ok_or_else(RegistryError::unknown_plugin)?
            .descriptor
            .hook_budget(hook);
        let Some(budget) = budget else {
            return Ok(());
        };
        match self.hooks.dispatch(plugin_id, project_id, hook, budget) {
            Ok(_) => Ok(()),
            Err(reason) => {
                self.contain(plugin_id, hook, reason.clone());
                Err(RegistryError::hook_violation(hook, &reason))
            }
        }
    }

    fn contain(&mut self, plugin_id: &str, hook: LifecycleHook, reason: ViolationReason) {
        if let Some(extension) = self.admitted.get_mut(plugin_id) {
            extension.state = PluginState::Quarantined;
        }
        self.active.retain(|(_, owner)| owner != plugin_id);
        self.violations.push(ViolationRecord {
            plugin_id: plugin_id.to_owned(),
            hook,
            reason,
        });
    }

    fn refresh_state(&mut self, plugin_id: &str) {
        let still_installed = self.installed.iter().any(|(_, owner)| owner == plugin_id);
        if let Some(extension) = self.admitted.get_mut(plugin_id)
            && extension.state != PluginState::Quarantined
        {
            extension.state = if still_installed {
                PluginState::Installed
            } else {
                PluginState::Admitted
            };
        }
    }
}

fn validate_for_admission(
    value: &Value,
    policy: &DescriptorPolicy,
) -> Result<PluginDescriptorV1, RegistryError> {
    crate::descriptor::validate_descriptor_value(value, policy)
        .map_err(|error| map_descriptor_error(&error))
}

fn map_descriptor_error(error: &DescriptorError) -> RegistryError {
    match error.code() {
        DescriptorErrorCode::VersionUnsupported => {
            RegistryError::compatibility_unsupported(error.detail())
        }
        DescriptorErrorCode::SignatureInvalid => {
            RegistryError::admission_signature_invalid(error.detail())
        }
        _ => RegistryError::descriptor_invalid(error.detail()),
    }
}

fn validate_kind_references(
    descriptor: &PluginDescriptorV1,
    approved: &ApprovedKindCatalog,
) -> Result<(), RegistryError> {
    fn walk(node: &CompositionNode, approved: &ApprovedKindCatalog) -> Result<(), RegistryError> {
        if !approved.contains(&node.kind) {
            return Err(RegistryError::contribution_unapproved(&node.kind));
        }
        for child in &node.children {
            walk(child, approved)?;
        }
        Ok(())
    }
    for composition in &descriptor.contributions.compositions {
        walk(&composition.tree, approved)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::CompatibilityRange;
    use crate::fixture::{
        POS_PACK_KEY_ID, POS_PACK_PUBLISHER, pos_pack_descriptor, pos_pack_envelope, pos_pack_seed,
        pos_pack_trust_keys,
    };

    fn registry() -> ExtensionRegistry {
        let trust = TrustStore::from_keys(pos_pack_trust_keys()).expect("trust keys valid");
        ExtensionRegistry::new(
            DescriptorPolicy::default(),
            trust,
            ApprovedKindCatalog::with_defaults(),
        )
    }

    #[test]
    fn admits_the_signed_pos_pack_fixture() {
        let mut registry = registry();
        registry.admit(&pos_pack_envelope()).expect("admission");
        let extension = registry.plugin("com.studio.pack.pos").expect("admitted");
        assert_eq!(extension.state, PluginState::Admitted);
        assert_eq!(extension.descriptor.publisher.id, POS_PACK_PUBLISHER);
        assert_eq!(extension.descriptor.publisher.key_id, POS_PACK_KEY_ID);
    }

    #[test]
    fn rejects_a_tampered_descriptor_before_activation() {
        let mut envelope = pos_pack_envelope();
        envelope.descriptor["name"] = Value::String("Tampered Pack".to_owned());
        let mut registry = registry();
        let error = registry
            .admit(&envelope)
            .expect_err("tampered descriptor must fail");
        assert_eq!(
            error.code(),
            crate::error::RegistryErrorCode::AdmissionSignatureInvalid
        );
        assert!(registry.plugin("com.studio.pack.pos").is_none());
    }

    #[test]
    fn rejects_descriptors_incompatible_with_the_host() {
        let mut descriptor = pos_pack_descriptor();
        descriptor.compatibility = CompatibilityRange {
            studio_version: ">=99.0.0".to_owned(),
            schema_versions: vec![1],
        };
        let envelope = SignedDescriptorEnvelope::sign(&descriptor, &pos_pack_seed())
            .expect("signing fixture descriptor");
        let mut registry = registry();
        let error = registry
            .admit(&envelope)
            .expect_err("incompatible descriptor must fail");
        assert_eq!(
            error.code(),
            crate::error::RegistryErrorCode::CompatibilityUnsupported
        );
    }

    #[test]
    fn rejects_third_party_renderer_kind_registration() {
        let mut descriptor = pos_pack_descriptor();
        let mut hostile = descriptor.contributions.compositions[0].tree.clone();
        hostile.kind = "native.fancyRenderer".to_owned();
        descriptor.contributions.compositions[0].tree = hostile;
        let envelope = SignedDescriptorEnvelope::sign(&descriptor, &pos_pack_seed())
            .expect("signing fixture descriptor");
        let mut registry = registry();
        let error = registry
            .admit(&envelope)
            .expect_err("unapproved kind must fail");
        assert_eq!(
            error.code(),
            crate::error::RegistryErrorCode::ContributionUnapproved
        );
    }

    #[test]
    fn rejects_unknown_schema_fields_structurally() {
        let mut envelope = pos_pack_envelope();
        envelope.descriptor["rendererKinds"] = serde_json::json!(["fancy"]);
        let bytes = serde_json::to_vec(&envelope).expect("encode");
        let policy = DescriptorPolicy::default();
        let error = crate::descriptor::parse_descriptor_envelope(&bytes, &policy)
            .expect_err("unknown field must fail");
        assert_eq!(
            error.code(),
            crate::error::DescriptorErrorCode::SchemaUnknownField
        );
    }

    #[test]
    fn installs_uses_revokes_and_removes_first_party_pack_with_artifact_report() {
        let mut registry = registry();
        registry
            .admit(&pos_pack_envelope())
            .expect("signed pack admits");
        let descriptor = &registry
            .plugin("com.studio.pack.pos")
            .expect("pack exists")
            .descriptor;
        assert!(!descriptor.contributions.compositions.is_empty());
        assert!(!descriptor.contributions.settings_groups.is_empty());

        registry
            .install("project-a", "com.studio.pack.pos")
            .expect("pack installs");
        assert_eq!(
            registry
                .activate("project-a", "com.studio.pack.pos")
                .expect_err("capability consent is required")
                .code(),
            crate::error::RegistryErrorCode::ConsentDenied
        );
        registry
            .grant_consent(
                "project-a",
                "com.studio.pack.pos",
                DeclaredCapability::PrinterSimulate,
            )
            .expect("explicit project consent records");
        assert_eq!(
            registry.consent_decision(
                "project-a",
                "com.studio.pack.pos",
                DeclaredCapability::PrinterSimulate,
            ),
            Some(ConsentDecision::Granted)
        );
        registry
            .activate("project-a", "com.studio.pack.pos")
            .expect("consented pack activates");
        registry
            .project_open("project-a", "com.studio.pack.pos")
            .expect("project-open hook runs");
        registry
            .record_usage(
                "project-a",
                "com.studio.pack.pos",
                OwnedArtifact::Composition("pos.product-row".to_owned()),
            )
            .expect("declared composition is usable");
        registry
            .record_usage(
                "project-a",
                "com.studio.pack.pos",
                OwnedArtifact::SettingsGroup("pos.receipt".to_owned()),
            )
            .expect("declared settings group is usable");
        assert!(
            registry
                .revoke_consent(
                    "project-a",
                    "com.studio.pack.pos",
                    DeclaredCapability::PrinterSimulate,
                )
                .expect("consent revokes")
        );
        assert_eq!(
            registry.consent_decision(
                "project-a",
                "com.studio.pack.pos",
                DeclaredCapability::PrinterSimulate,
            ),
            None
        );

        // Re-grant for the removal journey; revocation already deactivated the project.
        registry
            .grant_consent(
                "project-a",
                "com.studio.pack.pos",
                DeclaredCapability::PrinterSimulate,
            )
            .expect("consent re-grants");
        registry
            .activate("project-a", "com.studio.pack.pos")
            .expect("pack reactivates");
        let report = registry
            .plan_removal("project-a", "com.studio.pack.pos")
            .expect("removal reports owned artifacts");
        assert_eq!(
            report.remaining_artifacts,
            vec![
                OwnedArtifact::Composition("pos.product-row".to_owned()),
                OwnedArtifact::SettingsGroup("pos.receipt".to_owned()),
            ]
        );
        assert_eq!(
            registry
                .complete_removal("project-a", "com.studio.pack.pos", false)
                .expect_err("removal is blocked until project artifacts are resolved")
                .code(),
            crate::error::RegistryErrorCode::RemovalBlocked
        );
        assert_eq!(registry.clear_usage("project-a", "com.studio.pack.pos"), 2);
        registry
            .plan_removal("project-a", "com.studio.pack.pos")
            .expect("fresh empty removal plan");
        registry
            .complete_removal("project-a", "com.studio.pack.pos", false)
            .expect("pack removes after report");
        assert!(registry.plugin("com.studio.pack.pos").is_some());
        assert_eq!(
            registry
                .plugin("com.studio.pack.pos")
                .expect("pack remains admitted")
                .state,
            PluginState::Admitted
        );
    }

    #[test]
    fn lifecycle_output_over_budget_quarantines_extension() {
        let mut registry = registry();
        registry
            .admit(&pos_pack_envelope())
            .expect("signed pack admits");
        registry
            .install("project-a", "com.studio.pack.pos")
            .expect("pack installs");
        registry
            .grant_consent(
                "project-a",
                "com.studio.pack.pos",
                DeclaredCapability::PrinterSimulate,
            )
            .expect("consent records");
        registry.hooks_mut().register(
            "com.studio.pack.pos",
            LifecycleHook::Activate,
            Box::new(|_| Ok(vec![0_u8; 64 * 1024 + 1])),
        );
        assert_eq!(
            registry
                .activate("project-a", "com.studio.pack.pos")
                .expect_err("over-budget hook is contained")
                .code(),
            crate::error::RegistryErrorCode::HookViolation
        );
        assert_eq!(
            registry
                .plugin("com.studio.pack.pos")
                .expect("pack remains visible")
                .state,
            PluginState::Quarantined
        );
        assert_eq!(registry.violations().len(), 1);
    }
}
