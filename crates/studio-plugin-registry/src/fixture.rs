//! First-party `pos-pack` fixture: a signed vertical pack providing compositions, typed
//! settings groups (including secret-reference and device-picker fields), commands, and a
//! capability request. Used by tests and as the authored consumer proof for ticket 35.

use ed25519_dalek::SigningKey;

use crate::descriptor::{
    ActionContribution, ActionOperation, CommandContribution, CompatibilityRange,
    CompositionContribution, CompositionNode, Contributions, DeclaredCapability,
    HookBudget, HookDeclaration, LifecycleHook, PluginDescriptorV1, PrimitiveInputValue,
    SelectOption, SettingsField, SettingsFieldType, SettingsGroup, SignedDescriptorEnvelope,
};
use studio_package::{TrustedPublisherKey, TrustStore};

/// Fixture publisher identity.
pub const POS_PACK_PUBLISHER: &str = "com.studio";

/// Fixture provisioned key identifier.
pub const POS_PACK_KEY_ID: &str = "pos-pack-2026";

/// Deterministic test-only signing seed for the fixture publisher key.
#[must_use]
pub const fn pos_pack_seed() -> [u8; 32] {
    [
        0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35,
        0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35, 0x35,
        0x35, 0x35,
    ]
}

fn text(value: &str) -> PrimitiveInputValue {
    PrimitiveInputValue::Text(value.to_owned())
}

fn node(kind: &str) -> CompositionNode {
    CompositionNode {
        kind: kind.to_owned(),
        inputs: Default::default(),
        children: Vec::new(),
    }
}

fn leaf(kind: &str, name: &str) -> CompositionNode {
    CompositionNode {
        kind: kind.to_owned(),
        inputs: [("text".to_owned(), text(name))].into_iter().collect(),
        children: Vec::new(),
    }
}

fn branch(kind: &str, children: Vec<CompositionNode>) -> CompositionNode {
    CompositionNode {
        kind: kind.to_owned(),
        inputs: Default::default(),
        children,
    }
}

/// The authored pos-pack descriptor before signing.
#[must_use]
pub fn pos_pack_descriptor() -> PluginDescriptorV1 {
    let receipt_totals = CompositionContribution {
        id: "pos.receipt-totals".to_owned(),
        title: "Receipt Totals".to_owned(),
        tree: branch(
            "column",
            vec![
                leaf("text", "Subtotal"),
                node("divider"),
                leaf("text", "Tax"),
                leaf("text", "Total"),
            ],
        ),
    };
    let product_row = CompositionContribution {
        id: "pos.product-row".to_owned(),
        title: "Product Row".to_owned(),
        tree: branch(
            "row",
            vec![
                CompositionNode {
                    kind: "image".to_owned(),
                    inputs: [("alt".to_owned(), text("Product photo"))]
                        .into_iter()
                        .collect(),
                    children: Vec::new(),
                },
                leaf("text", "Name"),
                leaf("text", "Price"),
            ],
        ),
    };
    let receipt_settings = SettingsGroup {
        id: "pos.receipt".to_owned(),
        title: "Receipt".to_owned(),
        fields: vec![
            SettingsField {
                id: "storeHeader".to_owned(),
                label: "Store header".to_owned(),
                kind: SettingsFieldType::Text {
                    default: Some("Studio Coffee".to_owned()),
                    max_length: Some(64),
                },
            },
            SettingsField {
                id: "taxRate".to_owned(),
                label: "Tax rate".to_owned(),
                kind: SettingsFieldType::Number {
                    min: Some(0.0),
                    max: Some(1.0),
                    default: Some(0.082_5),
                },
            },
            SettingsField {
                id: "printLogo".to_owned(),
                label: "Print logo".to_owned(),
                kind: SettingsFieldType::Boolean { default: true },
            },
            SettingsField {
                id: "paperSize".to_owned(),
                label: "Paper size".to_owned(),
                kind: SettingsFieldType::Select {
                    options: vec![
                        SelectOption {
                            value: "80mm".to_owned(),
                            label: "80 mm".to_owned(),
                        },
                        SelectOption {
                            value: "58mm".to_owned(),
                            label: "58 mm".to_owned(),
                        },
                    ],
                    default: Some("80mm".to_owned()),
                },
            },
            SettingsField {
                id: "accentColor".to_owned(),
                label: "Accent color".to_owned(),
                kind: SettingsFieldType::Color {
                    default: Some("#0F766E".to_owned()),
                },
            },
            SettingsField {
                id: "servicePin".to_owned(),
                label: "Manager service pin".to_owned(),
                kind: SettingsFieldType::SecretReference {
                    name: "manager.service-pin".to_owned(),
                    purpose: "Approves tip adjustments on receipts".to_owned(),
                },
            },
            SettingsField {
                id: "receiptPrinter".to_owned(),
                label: "Receipt printer".to_owned(),
                kind: SettingsFieldType::DevicePicker {
                    device_kind: "printer".to_owned(),
                },
            },
        ],
    };
    PluginDescriptorV1 {
        schema_version: crate::descriptor::DESCRIPTOR_SCHEMA_VERSION,
        id: "com.studio.pack.pos".to_owned(),
        name: "Studio POS Pack".to_owned(),
        version: "1.4.0".to_owned(),
        publisher: crate::descriptor::DescriptorPublisher {
            id: POS_PACK_PUBLISHER.to_owned(),
            key_id: POS_PACK_KEY_ID.to_owned(),
        },
        compatibility: CompatibilityRange {
            studio_version: "^0.1.0".to_owned(),
            schema_versions: vec![1],
        },
        contributions: Contributions {
            compositions: vec![receipt_totals, product_row],
            settings_groups: vec![receipt_settings],
            commands: vec![CommandContribution {
                id: "pos.openRegister".to_owned(),
                title: "Open register".to_owned(),
                action: "pos.navigate-register".to_owned(),
            }],
            actions: vec![ActionContribution {
                id: "pos.navigate-register".to_owned(),
                title: "Navigate to register screen".to_owned(),
                operation: ActionOperation::Navigate {
                    screen: "register".to_owned(),
                },
            }],
        },
        capabilities: vec![DeclaredCapability::PrinterSimulate],
        lifecycle: vec![
            HookDeclaration {
                hook: LifecycleHook::Admission,
                budget: HookBudget {
                    time_ms: 500,
                    memory_bytes: 16 * 1024,
                },
            },
            HookDeclaration {
                hook: LifecycleHook::Install,
                budget: HookBudget {
                    time_ms: 1_000,
                    memory_bytes: 32 * 1024,
                },
            },
            HookDeclaration {
                hook: LifecycleHook::Activate,
                budget: HookBudget {
                    time_ms: 2_000,
                    memory_bytes: 64 * 1024,
                },
            },
            HookDeclaration {
                hook: LifecycleHook::ProjectOpen,
                budget: HookBudget {
                    time_ms: 500,
                    memory_bytes: 16 * 1024,
                },
            },
            HookDeclaration {
                hook: LifecycleHook::Deactivate,
                budget: HookBudget {
                    time_ms: 1_000,
                    memory_bytes: 16 * 1024,
                },
            },
            HookDeclaration {
                hook: LifecycleHook::Remove,
                budget: HookBudget {
                    time_ms: 1_000,
                    memory_bytes: 16 * 1024,
                },
            },
        ],
    }
}

/// Sign the fixture descriptor with the deterministic fixture seed.
///
/// # Panics
///
/// Panics if canonical signing fails, which cannot happen for this closed fixture.
#[must_use]
pub fn pos_pack_envelope() -> SignedDescriptorEnvelope {
    SignedDescriptorEnvelope::sign(&pos_pack_descriptor(), &pos_pack_seed())
        .expect("fixture descriptor signs")
}

/// Trust-store entries accepting the fixture publisher key.
#[must_use]
pub fn pos_pack_trust_keys() -> Vec<TrustedPublisherKey> {
    let verifying_key = SigningKey::from_bytes(&pos_pack_seed()).verifying_key();
    vec![TrustedPublisherKey {
        publisher_id: POS_PACK_PUBLISHER.to_owned(),
        key_id: POS_PACK_KEY_ID.to_owned(),
        verifying_key: verifying_key.to_bytes(),
        enabled: true,
    }]
}

/// Convenience trust store holding only the fixture publisher key.
///
/// # Panics
///
/// Panics if the fixture key material is invalid, which cannot happen.
#[must_use]
pub fn pos_pack_trust_store() -> TrustStore {
    TrustStore::from_keys(pos_pack_trust_keys()).expect("fixture trust keys valid")
}
