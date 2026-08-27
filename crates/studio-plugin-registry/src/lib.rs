//! Signed plugin-descriptor admission, consent, bounded lifecycle, and removal safety.
//!
//! Refines open grilling issue 07 (`define extension authority`): this crate fixes the
//! descriptor schema and registry mechanics that issue will generalize. Where the
//! specification is still underspecified the implementation prefers the closed,
//! deny-by-default reading and records the open questions in `docs/plugin-registry.md`.

mod consent;
mod descriptor;
mod error;
mod fixture;
mod lifecycle;
mod registry;
mod removal;

pub use consent::{ConsentDecision, ConsentLedger};
pub use descriptor::{
    ActionContribution, ActionOperation, CommandContribution, CompatibilityRange,
    CompositionContribution, CompositionNode, Contributions, DESCRIPTOR_SCHEMA_VERSION,
    DeclaredCapability, DescriptorPolicy, DescriptorPublisher, DescriptorSignature, HookBudget,
    HookDeclaration, LifecycleHook, MAX_DESCRIPTOR_BYTES, MAX_HOOK_MEMORY_BYTES, MAX_HOOK_TIME_MS,
    PluginDescriptorV1, PrimitiveInputValue, SelectOption, SettingsField, SettingsFieldType,
    SettingsGroup, SignedDescriptorEnvelope, TemplateContribution, TemplateScreen, TemplateToken,
    BrandSlotContribution, parse_descriptor_envelope, validate_descriptor_value,
};
pub use error::{DescriptorError, DescriptorErrorCode, RegistryError, RegistryErrorCode};
pub use fixture::{
    POS_PACK_KEY_ID, POS_PACK_PUBLISHER, pos_pack_descriptor, pos_pack_envelope, pos_pack_seed,
    pos_pack_template_descriptor, pos_pack_template_envelope, pos_pack_trust_keys,
    pos_pack_trust_store,
};
pub use lifecycle::{
    HookCallback, HookContext, HookFailure, HookRunReport, HookRunner, PluginState,
    ViolationReason, ViolationRecord,
};
pub use registry::{
    AdmittedExtension, ApprovedKindCatalog, DEFAULT_PRIMITIVE_CATALOG, ExtensionRegistry,
    RemovalReport,
};
pub use removal::{OwnedArtifact, ProjectUsage};
