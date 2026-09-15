#![allow(missing_docs)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use studio_package::{TrustStore, TrustedPublisherKey, verify_document_signature};
use studio_plugin_registry::{
    ActionContribution, ApprovedKindCatalog, CommandContribution, CompatibilityRange,
    CompositionContribution, CompositionNode, ConsentDecision, DeclaredCapability,
    DescriptorErrorCode, DescriptorPolicy, HookBudget, HookCallback, HookDeclaration,
    LifecycleHook, OwnedArtifact, PluginDescriptorV1, PluginState, RegistryErrorCode,
    SignedDescriptorEnvelope, ViolationReason, pos_pack_descriptor, pos_pack_envelope,
    pos_pack_seed, pos_pack_trust_keys,
};

const PROJECT: &str = "project-1";
const POS_PACK: &str = "com.studio.pack.pos";

fn registry() -> studio_plugin_registry::ExtensionRegistry {
    let trust = TrustStore::from_keys(pos_pack_trust_keys()).expect("fixture trust keys are valid");
    studio_plugin_registry::ExtensionRegistry::new(
        DescriptorPolicy::default(),
        trust,
        ApprovedKindCatalog::with_defaults(),
    )
}

fn signed_variant(mutate: impl FnOnce(&mut PluginDescriptorV1)) -> SignedDescriptorEnvelope {
    let mut descriptor = pos_pack_descriptor();
    mutate(&mut descriptor);
    SignedDescriptorEnvelope::sign(&descriptor, &pos_pack_seed()).expect("variant signs")
}

fn log_handler(log: &Arc<Mutex<Vec<&'static str>>>, tag: &'static str) -> HookCallback {
    let log = Arc::clone(log);
    Box::new(move |_| {
        log.lock().expect("log mutex").push(tag);
        Ok(Vec::new())
    })
}

#[test]
fn signed_descriptor_verifies_with_package_trust_machinery() {
    let envelope = pos_pack_envelope();
    let signature = envelope.signature_bytes().expect("hex signature");
    let verified = verify_document_signature(
        &envelope.descriptor,
        &signature,
        envelope.signature.publisher_id.as_str(),
        envelope.signature.key_id.as_str(),
        &studio_plugin_registry::pos_pack_trust_store(),
    )
    .expect("fixture signature verifies");
    assert!(!verified.signed_document.is_empty());
}

#[test]
#[allow(clippy::too_many_lines)]
fn full_walkthrough_admission_consent_install_lifecycle_removal_report() {
    let mut registry = registry();
    let log = Arc::new(Mutex::new(Vec::new()));
    registry.hooks_mut().register(
        POS_PACK,
        LifecycleHook::Admission,
        log_handler(&log, "admission"),
    );
    registry.hooks_mut().register(
        POS_PACK,
        LifecycleHook::Install,
        log_handler(&log, "install"),
    );
    registry.hooks_mut().register(
        POS_PACK,
        LifecycleHook::Activate,
        log_handler(&log, "activate"),
    );
    registry.hooks_mut().register(
        POS_PACK,
        LifecycleHook::ProjectOpen,
        log_handler(&log, "project-open"),
    );
    registry.hooks_mut().register(
        POS_PACK,
        LifecycleHook::Deactivate,
        log_handler(&log, "deactivate"),
    );
    registry
        .hooks_mut()
        .register(POS_PACK, LifecycleHook::Remove, log_handler(&log, "remove"));

    registry.admit(&pos_pack_envelope()).expect("admission");
    let admitted = registry.plugin(POS_PACK).expect("admitted extension");
    assert_eq!(admitted.state, PluginState::Admitted);
    assert_eq!(log.lock().unwrap().as_slice(), ["admission"]);

    registry.install(PROJECT, POS_PACK).expect("install");
    assert_eq!(log.lock().unwrap().as_slice(), ["admission", "install"]);

    let denied = registry
        .activate(PROJECT, POS_PACK)
        .expect_err("deny-by-default consent");
    assert_eq!(denied.code(), RegistryErrorCode::ConsentDenied);

    registry
        .grant_consent(PROJECT, POS_PACK, DeclaredCapability::PrinterSimulate)
        .expect("consent grant for declared capability");
    assert_eq!(
        registry.consent_decision(PROJECT, POS_PACK, DeclaredCapability::PrinterSimulate),
        Some(ConsentDecision::Granted)
    );

    registry.activate(PROJECT, POS_PACK).expect("activate");
    registry
        .project_open(PROJECT, POS_PACK)
        .expect("project open");
    assert_eq!(
        log.lock().unwrap().as_slice(),
        ["admission", "install", "activate", "project-open"]
    );

    registry
        .record_usage(
            PROJECT,
            POS_PACK,
            OwnedArtifact::Composition("pos.receipt-totals".to_owned()),
        )
        .expect("usage recorded");
    registry
        .record_usage(
            PROJECT,
            POS_PACK,
            OwnedArtifact::SettingsGroup("pos.receipt".to_owned()),
        )
        .expect("usage recorded");

    let report = registry.plan_removal(PROJECT, POS_PACK).expect("plan");
    assert_eq!(report.remaining_artifacts.len(), 2);
    let blocked = registry
        .complete_removal(PROJECT, POS_PACK, false)
        .expect_err("removal blocked before artifacts resolve");
    assert_eq!(blocked.code(), RegistryErrorCode::RemovalBlocked);
    assert_eq!(
        registry.plugin(POS_PACK).expect("still installed").state,
        PluginState::Active
    );

    registry.deactivate(PROJECT, POS_PACK).expect("deactivate");
    assert_eq!(registry.clear_usage(PROJECT, POS_PACK), 2);

    let report = registry.plan_removal(PROJECT, POS_PACK).expect("re-plan");
    assert!(report.remaining_artifacts.is_empty());
    let report = registry
        .complete_removal(PROJECT, POS_PACK, false)
        .expect("removal completes");
    assert!(report.remaining_artifacts.is_empty());
    assert_eq!(
        log.lock().unwrap().as_slice(),
        [
            "admission",
            "install",
            "activate",
            "project-open",
            "deactivate",
            "remove",
        ]
    );
    assert_eq!(
        registry.consent_decision(PROJECT, POS_PACK, DeclaredCapability::PrinterSimulate),
        None
    );
    assert_eq!(
        registry.plugin(POS_PACK).expect("still known").state,
        PluginState::Admitted
    );
}

#[test]
fn consent_is_revocable_and_deactivates_active_extensions() {
    let mut registry = registry();
    registry.admit(&pos_pack_envelope()).expect("admission");
    registry.install(PROJECT, POS_PACK).expect("install");
    registry
        .grant_consent(PROJECT, POS_PACK, DeclaredCapability::PrinterSimulate)
        .expect("grant");
    registry.activate(PROJECT, POS_PACK).expect("activate");

    let revoked = registry
        .revoke_consent(PROJECT, POS_PACK, DeclaredCapability::PrinterSimulate)
        .expect("revocation succeeds");
    assert!(revoked);
    assert_eq!(
        registry.plugin(POS_PACK).expect("known").state,
        PluginState::Installed
    );
    let denied = registry
        .activate(PROJECT, POS_PACK)
        .expect_err("revoked consent returns to deny-by-default");
    assert_eq!(denied.code(), RegistryErrorCode::ConsentDenied);
}

#[test]
fn explicit_denial_deactivates_an_active_extension() {
    let mut registry = registry();
    registry.admit(&pos_pack_envelope()).expect("admission");
    registry.install(PROJECT, POS_PACK).expect("install");
    registry
        .grant_consent(PROJECT, POS_PACK, DeclaredCapability::PrinterSimulate)
        .expect("grant");
    registry.activate(PROJECT, POS_PACK).expect("activate");

    registry
        .deny_consent(PROJECT, POS_PACK, DeclaredCapability::PrinterSimulate)
        .expect("denial deactivates");
    assert_eq!(
        registry.consent_decision(PROJECT, POS_PACK, DeclaredCapability::PrinterSimulate),
        Some(ConsentDecision::Denied)
    );
    assert_eq!(
        registry.plugin(POS_PACK).expect("known").state,
        PluginState::Installed
    );
    let denied = registry
        .project_open(PROJECT, POS_PACK)
        .expect_err("denied extension is no longer active");
    assert_eq!(denied.code(), RegistryErrorCode::StateInvalid);
}

#[test]
fn failed_admission_hook_is_contained_without_registering_the_extension() {
    let mut registry = registry();
    registry.hooks_mut().register(
        POS_PACK,
        LifecycleHook::Admission,
        Box::new(|_| Ok(vec![0_u8; 32 * 1024])),
    );

    let error = registry
        .admit(&pos_pack_envelope())
        .expect_err("admission hook exceeds fixture budget");
    assert_eq!(error.code(), RegistryErrorCode::HookViolation);
    assert!(registry.plugin(POS_PACK).is_none());
    assert!(matches!(
        registry.violations()[0].reason,
        ViolationReason::OutputBudgetExceeded {
            allowed_bytes: 16_384,
            actual_bytes: 32_768,
        }
    ));
}

#[test]
fn hook_time_budget_violation_is_contained_and_quarantines() {
    let mut registry = registry();
    registry
        .admit(&signed_variant(|descriptor| {
            descriptor.lifecycle = vec![HookDeclaration {
                hook: LifecycleHook::Activate,
                budget: HookBudget {
                    time_ms: 1,
                    memory_bytes: 1024,
                },
            }];
        }))
        .expect("admission");
    registry.install(PROJECT, POS_PACK).expect("install");
    registry
        .grant_consent(PROJECT, POS_PACK, DeclaredCapability::PrinterSimulate)
        .expect("grant");
    registry.hooks_mut().register(
        POS_PACK,
        LifecycleHook::Activate,
        Box::new(|_| {
            std::thread::sleep(Duration::from_millis(30));
            Ok(Vec::new())
        }),
    );

    let error = registry
        .activate(PROJECT, POS_PACK)
        .expect_err("overrun activate must be contained");
    assert_eq!(error.code(), RegistryErrorCode::HookViolation);
    let violations = registry.violations();
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].hook, LifecycleHook::Activate);
    assert!(matches!(
        violations[0].reason,
        ViolationReason::TimeBudgetExceeded { allowed_ms: 1, .. }
    ));
    assert_eq!(
        registry.plugin(POS_PACK).expect("known").state,
        PluginState::Quarantined
    );
    let refused = registry
        .project_open(PROJECT, POS_PACK)
        .expect_err("quarantined extensions refuse hooks");
    assert_eq!(refused.code(), RegistryErrorCode::StateInvalid);
}

#[test]
fn hook_output_budget_violation_is_contained() {
    let mut registry = registry();
    registry
        .admit(&signed_variant(|descriptor| {
            descriptor.lifecycle = vec![HookDeclaration {
                hook: LifecycleHook::Install,
                budget: HookBudget {
                    time_ms: 1_000,
                    memory_bytes: 64,
                },
            }];
        }))
        .expect("admission");
    registry.hooks_mut().register(
        POS_PACK,
        LifecycleHook::Install,
        Box::new(|_| Ok(vec![0_u8; 200])),
    );

    let error = registry
        .install(PROJECT, POS_PACK)
        .expect_err("oversized output must be contained");
    assert_eq!(error.code(), RegistryErrorCode::HookViolation);
    assert!(matches!(
        registry.violations()[0].reason,
        ViolationReason::OutputBudgetExceeded {
            allowed_bytes: 64,
            actual_bytes: 200
        }
    ));
    assert_eq!(
        registry.plugin(POS_PACK).expect("known").state,
        PluginState::Quarantined
    );
}

#[test]
fn panicking_hook_adapter_is_contained_as_a_rejection() {
    let mut registry = registry();
    registry.admit(&pos_pack_envelope()).expect("admission");
    registry.hooks_mut().register(
        POS_PACK,
        LifecycleHook::Install,
        Box::new(|_| panic!("simulated guest trap")),
    );

    let error = registry
        .install(PROJECT, POS_PACK)
        .expect_err("guest trap must be contained");
    assert_eq!(error.code(), RegistryErrorCode::HookViolation);
    assert_eq!(
        registry.plugin(POS_PACK).expect("known").state,
        PluginState::Quarantined
    );
    assert!(matches!(
        registry.violations()[0].reason,
        ViolationReason::HandlerRejected
    ));
}

#[test]
fn changed_project_usage_invalidates_a_removal_report() {
    let mut registry = registry();
    registry.admit(&pos_pack_envelope()).expect("admission");
    registry.install(PROJECT, POS_PACK).expect("install");
    registry
        .plan_removal(PROJECT, POS_PACK)
        .expect("empty plan");
    registry
        .record_usage(
            PROJECT,
            POS_PACK,
            OwnedArtifact::Composition("pos.receipt-totals".to_owned()),
        )
        .expect("usage recorded after plan");

    let error = registry
        .complete_removal(PROJECT, POS_PACK, true)
        .expect_err("stale report cannot authorize mutation");
    assert_eq!(error.code(), RegistryErrorCode::RemovalPlanInvalid);
    assert_eq!(
        registry.plugin(POS_PACK).expect("still installed").state,
        PluginState::Installed
    );
}

#[test]
fn tampered_descriptor_is_rejected_before_activation() {
    let mut envelope = pos_pack_envelope();
    mutate_name(&mut envelope.descriptor, "Tampered Pack");
    let mut registry = registry();
    let error = registry
        .admit(&envelope)
        .expect_err("tampering must break the signature");
    assert_eq!(error.code(), RegistryErrorCode::AdmissionSignatureInvalid);
    assert!(registry.plugin(POS_PACK).is_none());
    registry
        .install(PROJECT, POS_PACK)
        .expect_err("nothing to install");
}

fn mutate_name(value: &mut Value, name: &str) {
    value["name"] = Value::String(name.to_owned());
}

#[test]
fn expired_compatibility_is_rejected_at_admission() {
    let envelope = signed_variant(|descriptor| {
        descriptor.compatibility = CompatibilityRange {
            studio_version: "^0.1.0".to_owned(),
            schema_versions: vec![2],
        };
    });
    let mut registry = registry();
    let error = registry
        .admit(&envelope)
        .expect_err("expired schema compatibility must fail");
    assert_eq!(error.code(), RegistryErrorCode::CompatibilityUnsupported);
}

#[test]
fn disabled_trust_key_rejects_admission() {
    let keys: Vec<TrustedPublisherKey> = pos_pack_trust_keys()
        .into_iter()
        .map(|mut key| {
            key.enabled = false;
            key
        })
        .collect();
    let trust = TrustStore::from_keys(keys).expect("disabled key store valid");
    let mut registry = studio_plugin_registry::ExtensionRegistry::new(
        DescriptorPolicy::default(),
        trust,
        ApprovedKindCatalog::with_defaults(),
    );
    let error = registry
        .admit(&pos_pack_envelope())
        .expect_err("disabled publisher keys reject admission");
    assert_eq!(error.code(), RegistryErrorCode::AdmissionSignatureInvalid);
}

#[test]
fn third_party_descriptors_cannot_register_renderer_kinds_or_unknown_fields() {
    let hostile_kind = signed_variant(|descriptor| {
        descriptor
            .contributions
            .compositions
            .push(CompositionContribution {
                id: "hostile.renderer".to_owned(),
                title: "Hostile Renderer".to_owned(),
                tree: CompositionNode {
                    kind: "native.fancyRenderer".to_owned(),
                    inputs: std::collections::BTreeMap::default(),
                    children: Vec::new(),
                },
            });
    });
    let mut registry = registry();
    let error = registry
        .admit(&hostile_kind)
        .expect_err("unapproved renderer kinds must fail structurally");
    assert_eq!(error.code(), RegistryErrorCode::ContributionUnapproved);

    let unknown_field = serde_json::json!({
        "schemaVersion": 1,
        "id": "evil.pack.native",
        "name": "Evil Pack",
        "version": "1.0.0",
        "publisher": { "id": "com.studio", "keyId": "pos-pack-2026" },
        "compatibility": { "studioVersion": "^0.1.0", "schemaVersions": [1] },
        "rendererKinds": ["fancy"],
        "signature": {
            "publisherId": "com.studio",
            "keyId": "pos-pack-2026",
            "signature": "00".repeat(64),
        },
    });
    let bytes = serde_json::to_vec(&unknown_field).expect("encode");
    let policy = DescriptorPolicy::default();
    let error = studio_plugin_registry::parse_descriptor_envelope(&bytes, &policy)
        .expect_err("unknown fields must fail parsing");
    assert_eq!(error.code(), DescriptorErrorCode::SchemaUnknownField);

    let command_without_action = signed_variant(|descriptor| {
        descriptor.contributions.commands = vec![CommandContribution {
            id: "orphan.command".to_owned(),
            title: "Orphan command".to_owned(),
            action: "missing.action".to_owned(),
        }];
    });
    let error = registry
        .admit(&command_without_action)
        .expect_err("commands must reference declared actions");
    assert_eq!(error.code(), RegistryErrorCode::DescriptorInvalid);
}

#[test]
fn secret_references_follow_protected_secret_store_conventions() {
    let bad_name = signed_variant(|descriptor| {
        if let studio_plugin_registry::SettingsFieldType::SecretReference { name, .. } =
            &mut descriptor.contributions.settings_groups[0].fields[5].kind
        {
            *name = "Manager PIN".to_owned();
        }
    });
    let mut registry = registry();
    let error = registry
        .admit(&bad_name)
        .expect_err("secret names must follow ticket-18 protected-store rules");
    assert_eq!(error.code(), RegistryErrorCode::DescriptorInvalid);

    let duplicate = signed_variant(|descriptor| {
        descriptor.contributions.settings_groups[0].fields.push(
            studio_plugin_registry::SettingsField {
                id: "servicePinMirror".to_owned(),
                label: "Duplicate pin reference".to_owned(),
                kind: studio_plugin_registry::SettingsFieldType::SecretReference {
                    name: "manager.service-pin".to_owned(),
                    purpose: "Duplicates an existing declaration".to_owned(),
                },
            },
        );
    });
    let error = registry
        .admit(&duplicate)
        .expect_err("duplicate secret names must be rejected");
    assert_eq!(error.code(), RegistryErrorCode::DescriptorInvalid);
}

#[test]
fn contributions_deliver_compositions_settings_and_commands_as_data() {
    let descriptor = pos_pack_descriptor();
    let compositions = &descriptor.contributions.compositions;
    assert_eq!(compositions.len(), 2);
    let totals = compositions
        .iter()
        .find(|composition| composition.id == "pos.receipt-totals")
        .expect("receipt totals contribution");
    assert_eq!(totals.tree.kind, "column");
    assert_eq!(totals.tree.children.len(), 4);
    assert!(compositions.iter().all(|composition| {
        fn approved(node: &CompositionNode) -> bool {
            studio_plugin_registry::DEFAULT_PRIMITIVE_CATALOG.contains(&node.kind.as_str())
                && node.children.iter().all(approved)
        }
        approved(&composition.tree)
    }));

    let groups = &descriptor.contributions.settings_groups;
    assert_eq!(groups.len(), 1);
    let field_kinds: Vec<&str> = groups[0]
        .fields
        .iter()
        .filter_map(|field| match &field.kind {
            studio_plugin_registry::SettingsFieldType::SecretReference { .. } => {
                Some("secretReference")
            }
            studio_plugin_registry::SettingsFieldType::DevicePicker { .. } => Some("devicePicker"),
            _ => None,
        })
        .collect();
    assert_eq!(field_kinds, vec!["secretReference", "devicePicker"]);

    let commands: &[CommandContribution] = &descriptor.contributions.commands;
    assert_eq!(commands.len(), 1);
    let actions: &[ActionContribution] = &descriptor.contributions.actions;
    assert!(actions.iter().any(|action| action.id == commands[0].action));

    assert_eq!(
        descriptor.capabilities,
        vec![DeclaredCapability::PrinterSimulate]
    );
}

#[test]
fn descriptor_error_families_are_stable_for_diagnostics() {
    let oversized = vec![b' '; studio_plugin_registry::MAX_DESCRIPTOR_BYTES + 1];
    let policy = DescriptorPolicy::default();
    let error = studio_plugin_registry::parse_descriptor_envelope(&oversized, &policy)
        .expect_err("byte ceiling enforced");
    assert_eq!(error.code(), DescriptorErrorCode::ByteLimitExceeded);

    let malformed: &[u8] = b"{not json";
    let error = studio_plugin_registry::parse_descriptor_envelope(malformed, &policy)
        .expect_err("malformed json rejected");
    assert_eq!(error.code(), DescriptorErrorCode::SchemaFieldInvalid);
}
