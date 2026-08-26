//! Closed plugin-descriptor schema v1 with strict host-ceiling validation.
//!
//! The schema is closed: unknown fields are rejected at every level via
//! `deny_unknown_fields`, so a descriptor can never smuggle an undeclared contribution
//! surface (for example a hypothetical `rendererKinds` field fails parsing outright).
//! Third-party extensions therefore cannot introduce native renderer kinds through this
//! format; composition trees may only reference host-approved primitive kinds, enforced
//! structurally in [`crate::registry`].

use std::collections::BTreeMap;

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::DescriptorError;

/// Exact supported descriptor schema major.
pub const DESCRIPTOR_SCHEMA_VERSION: u16 = 1;

/// Maximum encoded descriptor bytes accepted for parsing.
pub const MAX_DESCRIPTOR_BYTES: usize = 128 * 1024;

/// Host ceiling for one lifecycle hook's declared time budget in milliseconds.
pub const MAX_HOOK_TIME_MS: u64 = 5_000;

/// Host ceiling for one lifecycle hook's declared memory budget in bytes.
pub const MAX_HOOK_MEMORY_BYTES: usize = 4 * 1024 * 1024;

const MAX_TREE_DEPTH: usize = 16;
const MAX_TREE_NODES: usize = 256;
const MAX_TEXT_BYTES: usize = 128;
const MAX_ID_BYTES: usize = 128;
/// Matches the bundle-manifest `secrets.purpose` ceiling.
const MAX_SECRET_PURPOSE_BYTES: usize = 256;

/// Complete closed plugin-descriptor-v1 wire shape.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginDescriptorV1 {
    /// Exact descriptor schema major.
    pub schema_version: u16,
    /// Stable reverse-domain plugin identity.
    pub id: String,
    /// Safe host-visible display name.
    pub name: String,
    /// Semantic plugin release version.
    pub version: String,
    /// Publisher and provisioned signing-key identity.
    pub publisher: DescriptorPublisher,
    /// Compatible Studio and schema versions.
    pub compatibility: CompatibilityRange,
    /// Declared Designer contributions, all delivered as data.
    #[serde(default)]
    pub contributions: Contributions,
    /// Closed requested host capabilities; denied until consent is recorded.
    #[serde(default)]
    pub capabilities: Vec<DeclaredCapability>,
    /// Lifecycle hooks with per-hook bounded time/memory budgets.
    #[serde(default)]
    pub lifecycle: Vec<HookDeclaration>,
}

/// Signed publisher identity inside a descriptor.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DescriptorPublisher {
    /// Publisher identifier provisioned with the host trust store.
    pub id: String,
    /// Provisioned public-key identifier.
    pub key_id: String,
}

/// Compatibility range checked against the running host at admission.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompatibilityRange {
    /// Semantic-version requirement on the Studio release, e.g. `^0.1.0`.
    pub studio_version: String,
    /// Schema majors this plugin understands; must include the host schema major.
    pub schema_versions: Vec<u16>,
}

/// Closed capability catalog shared with bundle manifests for milestone one.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
pub enum DeclaredCapability {
    /// Deterministic payment simulator.
    #[serde(rename = "payment.simulate")]
    PaymentSimulate,
    /// Structured printer-preview simulator.
    #[serde(rename = "printer.simulate")]
    PrinterSimulate,
}

impl DeclaredCapability {
    /// Stable wire name for diagnostics and consent records.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PaymentSimulate => "payment.simulate",
            Self::PrinterSimulate => "printer.simulate",
        }
    }
}

/// Every declarative contribution surface a plugin may provide.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Contributions {
    /// Reusable Compositions built from approved primitive kinds.
    #[serde(default)]
    pub compositions: Vec<CompositionContribution>,
    /// Named settings groups rendered as typed tab groups by the Designer.
    #[serde(default)]
    pub settings_groups: Vec<SettingsGroup>,
    /// Commands surfaced through `DesignerSession`.
    #[serde(default)]
    pub commands: Vec<CommandContribution>,
    /// Declarative actions referenced by commands and interactions.
    #[serde(default)]
    pub actions: Vec<ActionContribution>,
}

/// One Reusable Composition contributed as a tree of approved primitive kinds.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompositionContribution {
    /// Stable composition identifier scoped to the plugin.
    pub id: String,
    /// Safe display title.
    pub title: String,
    /// Root of the primitive-node tree.
    pub tree: CompositionNode,
}

/// One node inside a contributed composition tree.
///
/// `kind` must resolve against the host-approved primitive catalog at admission. There is
/// no field through which new renderer kinds could be introduced.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompositionNode {
    /// Approved primitive kind reference, e.g. `column`.
    pub kind: String,
    /// Typed input values keyed by input name.
    #[serde(default)]
    pub inputs: BTreeMap<String, PrimitiveInputValue>,
    /// Child nodes in document order.
    #[serde(default)]
    pub children: Vec<CompositionNode>,
}

/// Closed scalar input values allowed inside contributed trees.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum PrimitiveInputValue {
    /// Plain text value.
    Text(String),
    /// Numeric value.
    Number(f64),
    /// Boolean value.
    Boolean(bool),
}

/// One named settings group rendered as a top-level tab of typed fields.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsGroup {
    /// Stable group identifier scoped to the plugin.
    pub id: String,
    /// Safe tab title.
    pub title: String,
    /// Typed fields rendered by the generic settings renderer.
    #[serde(default)]
    pub fields: Vec<SettingsField>,
}

/// One typed settings field.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsField {
    /// Stable field identifier scoped to the group.
    pub id: String,
    /// Safe label shown beside the control.
    pub label: String,
    /// Closed field type with type-specific options.
    #[serde(rename = "type")]
    pub kind: SettingsFieldType,
}

/// Closed settings-field catalog including secret references and device pickers.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum SettingsFieldType {
    /// Free text input.
    Text {
        /// Optional default value.
        #[serde(default)]
        default: Option<String>,
        /// Optional maximum length.
        #[serde(default)]
        max_length: Option<u32>,
    },
    /// Numeric input with optional bounds.
    Number {
        /// Optional inclusive minimum.
        #[serde(default)]
        min: Option<f64>,
        /// Optional inclusive maximum.
        #[serde(default)]
        max: Option<f64>,
        /// Optional default value.
        #[serde(default)]
        default: Option<f64>,
    },
    /// Boolean toggle.
    Boolean {
        /// Default position.
        #[serde(default)]
        default: bool,
    },
    /// Color picker with optional hex default.
    Color {
        /// Optional default color.
        #[serde(default)]
        default: Option<String>,
    },
    /// Image picker with no extra options.
    Image,
    /// Single choice from declared options.
    Select {
        /// Allowed choices.
        options: Vec<SelectOption>,
        /// Optional default choice value.
        #[serde(default)]
        default: Option<String>,
    },
    /// Reference to a protected secret declared by name and purpose; values live only in
    /// the host's protected store and never enter guest memory.
    ///
    /// `name` follows the same rules as bundle-manifest [`studio_package::manifest`]
    /// `SecretDeclaration.name` and `ProtectedSecretKey`, so Designer-configured values
    /// resolve against the same app-scoped protected partitions landed by ticket 18.
    SecretReference {
        /// Stable lowercase identifier matching the plugin's declared protected secrets.
        name: String,
        /// Safe host-visible explanation shown during configuration and consent.
        purpose: String,
    },
    /// Device or station picker constrained to one device kind.
    DevicePicker {
        /// Device kind offered, e.g. `printer`.
        device_kind: String,
    },
}

/// One labeled choice inside a select field.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectOption {
    /// Stored value.
    pub value: String,
    /// Safe display label.
    pub label: String,
}

/// One command contributed to `DesignerSession`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandContribution {
    /// Stable command identifier scoped to the plugin.
    pub id: String,
    /// Safe menu title.
    pub title: String,
    /// Declarative action executed when the command runs.
    pub action: String,
}

/// One declarative action definition.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionContribution {
    /// Stable action identifier scoped to the plugin.
    pub id: String,
    /// Safe description.
    pub title: String,
    /// Closed declarative operation.
    pub operation: ActionOperation,
}

/// Closed declarative operations executable without plugin code.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum ActionOperation {
    /// Navigate to a declared screen.
    Navigate {
        /// Target screen identifier.
        screen: String,
    },
    /// Set one project state key.
    SetState {
        /// State key.
        key: String,
        /// Value to assign; omitting clears the key.
        #[serde(default)]
        value: Option<PrimitiveInputValue>,
    },
}

/// Closed lifecycle hook positions.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash, Deserialize, Serialize)]
pub enum LifecycleHook {
    /// Ran once when the registry admits the signed descriptor.
    #[serde(rename = "admission")]
    Admission,
    /// Ran when the extension is installed into a project.
    #[serde(rename = "install")]
    Install,
    /// Ran when the extension activates for a project.
    #[serde(rename = "activate")]
    Activate,
    /// Ran whenever a project using the extension opens.
    #[serde(rename = "projectOpen")]
    ProjectOpen,
    /// Ran when the extension deactivates for a project.
    #[serde(rename = "deactivate")]
    Deactivate,
    /// Ran after removal is confirmed and before state teardown completes.
    #[serde(rename = "remove")]
    Remove,
}

/// Per-hook resource budgets declared by the plugin and capped by host ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookBudget {
    /// Wall-clock ceiling in milliseconds.
    #[serde(rename = "timeMs")]
    pub time_ms: u64,
    /// Memory/output ceiling in bytes.
    pub memory_bytes: usize,
}

/// One declared hook with its budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookDeclaration {
    /// Which hook position this declaration configures.
    pub hook: LifecycleHook,
    /// Bounded resources granted to this hook.
    pub budget: HookBudget,
}

/// Signature envelope wrapping one exact descriptor JSON document.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignedDescriptorEnvelope {
    /// Exact descriptor JSON covered by the signature.
    pub descriptor: Value,
    /// Publisher/key attribution and raw signature.
    pub signature: DescriptorSignature,
}

/// Attribution and raw Ed25519 signature bytes for one descriptor.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DescriptorSignature {
    /// Publisher identifier looked up in the trust store.
    pub publisher_id: String,
    /// Provisioned key identifier looked up in the trust store.
    pub key_id: String,
    /// Hex-encoded 64-byte Ed25519 signature.
    pub signature: String,
}

/// Host policy applied while parsing and admitting descriptors.
#[derive(Clone, Debug)]
pub struct DescriptorPolicy {
    /// Supported descriptor schema major.
    pub schema_version: u16,
    /// Running Studio release version.
    pub studio_version: Version,
}

impl Default for DescriptorPolicy {
    fn default() -> Self {
        Self {
            schema_version: DESCRIPTOR_SCHEMA_VERSION,
            studio_version: Version::new(0, 1, 0),
        }
    }
}

impl PluginDescriptorV1 {
    /// Declared budget for one hook position, if any.
    #[must_use]
    pub fn hook_budget(&self, hook: LifecycleHook) -> Option<HookBudget> {
        self.lifecycle
            .iter()
            .find(|declaration| declaration.hook == hook)
            .map(|declaration| declaration.budget)
    }

    /// Whether the plugin declared one composition id.
    #[must_use]
    pub fn declares_composition(&self, id: &str) -> bool {
        self.contributions
            .compositions
            .iter()
            .any(|composition| composition.id == id)
    }

    /// Whether the plugin declared one settings-group id.
    #[must_use]
    pub fn declares_settings_group(&self, id: &str) -> bool {
        self.contributions
            .settings_groups
            .iter()
            .any(|group| group.id == id)
    }

    /// Whether the plugin declared one command id.
    #[must_use]
    pub fn declares_command(&self, id: &str) -> bool {
        self.contributions
            .commands
            .iter()
            .any(|command| command.id == id)
    }

    /// Whether the plugin declared one action id.
    #[must_use]
    pub fn declares_action(&self, id: &str) -> bool {
        self.contributions
            .actions
            .iter()
            .any(|action| action.id == id)
    }

    /// Whether every requested capability lacks consent according to `consented`.
    #[must_use]
    pub fn unconsented_capabilities(
        &self,
        consented: &std::collections::BTreeSet<DeclaredCapability>,
    ) -> Vec<DeclaredCapability> {
        self.capabilities
            .iter()
            .copied()
            .filter(|capability| !consented.contains(capability))
            .collect()
    }
}

/// Decode and validate one untrusted signed descriptor envelope.
///
/// # Errors
///
/// Returns [`DescriptorError`] for byte-limit, JSON, unknown-field, field-validation,
/// version, or contribution errors.
pub fn parse_descriptor_envelope(
    bytes: &[u8],
    policy: &DescriptorPolicy,
) -> Result<SignedDescriptorEnvelope, DescriptorError> {
    if bytes.len() > MAX_DESCRIPTOR_BYTES {
        return Err(DescriptorError::byte_limit());
    }
    let envelope: SignedDescriptorEnvelope = deserialize_closed(bytes)?;
    validate_descriptor_value(&envelope.descriptor, policy)?;
    Ok(envelope)
}

/// Decode and validate one untrusted descriptor JSON value against host policy.
///
/// # Errors
///
/// Returns [`DescriptorError`] for JSON, unknown-field, field-validation, version, or
/// contribution errors.
pub fn validate_descriptor_value(
    value: &Value,
    policy: &DescriptorPolicy,
) -> Result<PluginDescriptorV1, DescriptorError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| DescriptorError::json_invalid(error.to_string()))?;
    let descriptor: PluginDescriptorV1 = deserialize_closed(&bytes)?;
    validate_descriptor(&descriptor, policy)?;
    Ok(descriptor)
}

fn deserialize_closed<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, DescriptorError> {
    serde_json::from_slice(bytes).map_err(|error| {
        if error.to_string().contains("unknown field") {
            DescriptorError::schema_unknown_field(error.to_string())
        } else {
            DescriptorError::schema_field_invalid(error.to_string())
        }
    })
}

fn validate_descriptor(
    descriptor: &PluginDescriptorV1,
    policy: &DescriptorPolicy,
) -> Result<(), DescriptorError> {
    if descriptor.schema_version != policy.schema_version {
        return Err(DescriptorError::version_unsupported(format!(
            "descriptor schema {} unsupported; host supports {}",
            descriptor.schema_version, policy.schema_version
        )));
    }
    validate_plugin_id(&descriptor.id)?;
    validate_safe_text(&descriptor.name, "name")?;
    validate_safe_text(&descriptor.publisher.id, "publisher.id")?;
    validate_safe_text(&descriptor.publisher.key_id, "publisher.keyId")?;
    Version::parse(&descriptor.version)
        .map_err(|_| DescriptorError::schema_field_invalid("version".to_owned()))?;
    validate_compatibility(&descriptor.compatibility, policy)?;
    validate_contributions(descriptor)?;
    validate_capabilities(descriptor)?;
    validate_lifecycle(descriptor)?;
    Ok(())
}

fn validate_compatibility(
    compatibility: &CompatibilityRange,
    policy: &DescriptorPolicy,
) -> Result<(), DescriptorError> {
    let requirement = VersionReq::parse(&compatibility.studio_version).map_err(|_| {
        DescriptorError::schema_field_invalid("compatibility.studioVersion".to_owned())
    })?;
    if !requirement.matches(&policy.studio_version) {
        return Err(DescriptorError::version_unsupported(format!(
            "plugin requires Studio {} but host is {}",
            compatibility.studio_version, policy.studio_version
        )));
    }
    if compatibility.schema_versions.is_empty()
        || !compatibility
            .schema_versions
            .contains(&policy.schema_version)
    {
        return Err(DescriptorError::version_unsupported(format!(
            "plugin schemas {:?} exclude host schema {}",
            compatibility.schema_versions, policy.schema_version
        )));
    }
    Ok(())
}

fn validate_contributions(descriptor: &PluginDescriptorV1) -> Result<(), DescriptorError> {
    let mut composition_ids = std::collections::BTreeSet::new();
    for composition in &descriptor.contributions.compositions {
        validate_id_text(&composition.id, "contribution.id")?;
        validate_safe_text(&composition.title, "composition.title")?;
        if !composition_ids.insert(composition.id.as_str()) {
            return Err(DescriptorError::duplicate_contribution(&composition.id));
        }
        validate_tree(&composition.tree)?;
    }
    let mut group_ids = std::collections::BTreeSet::new();
    let mut secret_names = std::collections::BTreeSet::new();
    for group in &descriptor.contributions.settings_groups {
        validate_id_text(&group.id, "contribution.id")?;
        validate_safe_text(&group.title, "settings.title")?;
        if !group_ids.insert(group.id.as_str()) {
            return Err(DescriptorError::duplicate_contribution(&group.id));
        }
        let mut field_ids = std::collections::BTreeSet::new();
        for field in &group.fields {
            validate_id_text(&field.id, "settings.field.id")?;
            validate_safe_text(&field.label, "settings.field.label")?;
            if !field_ids.insert(field.id.as_str()) {
                return Err(DescriptorError::duplicate_contribution(&field.id));
            }
            if let SettingsFieldType::SecretReference { name, .. } = &field.kind
                && !secret_names.insert(name.as_str())
            {
                return Err(DescriptorError::duplicate_contribution(name));
            }
            validate_field_type(&field.kind)?;
        }
    }
    let mut action_ids = std::collections::BTreeSet::new();
    for action in &descriptor.contributions.actions {
        validate_id_text(&action.id, "contribution.id")?;
        validate_safe_text(&action.title, "action.title")?;
        if !action_ids.insert(action.id.as_str()) {
            return Err(DescriptorError::duplicate_contribution(&action.id));
        }
        match &action.operation {
            ActionOperation::Navigate { screen } => {
                validate_id_text(screen, "action.screen")?;
            }
            ActionOperation::SetState { key, .. } => {
                validate_id_text(key, "action.key")?;
            }
        }
    }
    let mut command_ids = std::collections::BTreeSet::new();
    for command in &descriptor.contributions.commands {
        validate_id_text(&command.id, "contribution.id")?;
        validate_safe_text(&command.title, "command.title")?;
        if !command_ids.insert(command.id.as_str()) {
            return Err(DescriptorError::duplicate_contribution(&command.id));
        }
        if !descriptor.declares_action(&command.action) {
            return Err(DescriptorError::contribution_invalid(format!(
                "command {} references undeclared action {}",
                command.id, command.action
            )));
        }
    }
    Ok(())
}

fn validate_tree(node: &CompositionNode) -> Result<(), DescriptorError> {
    fn walk(
        node: &CompositionNode,
        depth: usize,
        count: &mut usize,
    ) -> Result<(), DescriptorError> {
        *count += 1;
        if depth > MAX_TREE_DEPTH || *count > MAX_TREE_NODES {
            return Err(DescriptorError::contribution_invalid(
                "composition tree exceeds depth or node budget".to_owned(),
            ));
        }
        validate_kind_reference(&node.kind)?;
        for child in &node.children {
            walk(child, depth + 1, count)?;
        }
        Ok(())
    }

    let mut count = 0;
    walk(node, 1, &mut count)
}

fn validate_kind_reference(kind: &str) -> Result<(), DescriptorError> {
    if kind.is_empty() || kind.len() > 64 || kind.chars().any(char::is_control) {
        return Err(DescriptorError::contribution_invalid(format!(
            "invalid kind reference {kind}"
        )));
    }
    Ok(())
}

fn validate_field_type(kind: &SettingsFieldType) -> Result<(), DescriptorError> {
    match kind {
        SettingsFieldType::Text {
            default,
            max_length,
        } => {
            if max_length.is_some_and(|limit| limit == 0) {
                return Err(DescriptorError::contribution_invalid(
                    "text field maximum length must be positive".to_owned(),
                ));
            }
            if let Some(value) = default {
                validate_safe_text(value, "settings.text.default")?;
                if max_length.is_some_and(|limit| {
                    usize::try_from(limit).is_ok_and(|limit| value.chars().count() > limit)
                }) {
                    return Err(DescriptorError::contribution_invalid(
                        "text field default exceeds maximum length".to_owned(),
                    ));
                }
            }
        }
        SettingsFieldType::Number { min, max, default } => {
            if let (Some(min), Some(max)) = (min, max) {
                if min > max {
                    return Err(DescriptorError::contribution_invalid(
                        "number field bounds inverted".to_owned(),
                    ));
                }
            }
            if let Some(default) = default
                && (min.is_some_and(|min| *default < min) || max.is_some_and(|max| *default > max))
            {
                return Err(DescriptorError::contribution_invalid(
                    "number field default outside declared bounds".to_owned(),
                ));
            }
        }
        SettingsFieldType::Color { default } => {
            if let Some(value) = default {
                let valid_hex = value.len() == 7
                    && value.starts_with('#')
                    && value[1..].chars().all(|c| c.is_ascii_hexdigit());
                if !valid_hex {
                    return Err(DescriptorError::contribution_invalid(format!(
                        "invalid color default {value}"
                    )));
                }
            }
        }
        SettingsFieldType::Select { options, default } => {
            if options.is_empty() {
                return Err(DescriptorError::contribution_invalid(
                    "select field requires options".to_owned(),
                ));
            }
            let mut values = std::collections::BTreeSet::new();
            for option in options {
                validate_id_text(&option.value, "settings.select.option.value")?;
                validate_safe_text(&option.label, "settings.select.option.label")?;
                if !values.insert(option.value.as_str()) {
                    return Err(DescriptorError::contribution_invalid(format!(
                        "duplicate select option {}",
                        option.value
                    )));
                }
            }
            if let Some(value) = default
                && !options.iter().any(|option| option.value == *value)
            {
                return Err(DescriptorError::contribution_invalid(format!(
                    "select default {value} missing from options"
                )));
            }
        }
        SettingsFieldType::SecretReference { name, purpose } => {
            validate_secret_name(name)?;
            if purpose.is_empty()
                || purpose.len() > MAX_SECRET_PURPOSE_BYTES
                || purpose.chars().any(char::is_control)
            {
                return Err(DescriptorError::schema_field_invalid(
                    "secretReference.purpose".to_owned(),
                ));
            }
        }
        SettingsFieldType::DevicePicker { device_kind } => {
            validate_id_text(device_kind, "devicePicker.deviceKind")?;
        }
        SettingsFieldType::Image => {}
    }
    Ok(())
}

fn validate_capabilities(descriptor: &PluginDescriptorV1) -> Result<(), DescriptorError> {
    let mut seen = std::collections::BTreeSet::new();
    if descriptor
        .capabilities
        .iter()
        .any(|capability| !seen.insert(*capability))
    {
        return Err(DescriptorError::schema_field_invalid(
            "capabilities contains duplicates".to_owned(),
        ));
    }
    Ok(())
}

fn validate_lifecycle(descriptor: &PluginDescriptorV1) -> Result<(), DescriptorError> {
    let mut seen = std::collections::BTreeSet::new();
    for declaration in &descriptor.lifecycle {
        if !seen.insert(declaration.hook) {
            return Err(DescriptorError::schema_field_invalid(format!(
                "hook {} declared more than once",
                declaration.hook.name()
            )));
        }
        if declaration.budget.time_ms == 0 || declaration.budget.time_ms > MAX_HOOK_TIME_MS {
            return Err(DescriptorError::schema_field_invalid(format!(
                "hook {} time budget outside host ceiling",
                declaration.hook.name()
            )));
        }
        if declaration.budget.memory_bytes == 0
            || declaration.budget.memory_bytes > MAX_HOOK_MEMORY_BYTES
        {
            return Err(DescriptorError::schema_field_invalid(format!(
                "hook {} memory budget outside host ceiling",
                declaration.hook.name()
            )));
        }
    }
    Ok(())
}

impl LifecycleHook {
    /// Stable wire name used in diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Admission => "admission",
            Self::Install => "install",
            Self::Activate => "activate",
            Self::ProjectOpen => "projectOpen",
            Self::Deactivate => "deactivate",
            Self::Remove => "remove",
        }
    }
}

impl SignedDescriptorEnvelope {
    /// Sign one validated descriptor into an envelope using a raw publisher seed.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError`] when serialization or canonical signing fails.
    pub fn sign(
        descriptor: &PluginDescriptorV1,
        signing_seed: &[u8; 32],
    ) -> Result<Self, DescriptorError> {
        let value = serde_json::to_value(descriptor)
            .map_err(|error| DescriptorError::json_invalid(error.to_string()))?;
        let raw = studio_package::sign_document(&value, signing_seed)
            .map_err(|_| DescriptorError::signature_invalid())?;
        Ok(Self {
            descriptor: value,
            signature: DescriptorSignature {
                publisher_id: descriptor.publisher.id.clone(),
                key_id: descriptor.publisher.key_id.clone(),
                signature: hex_encode(&raw),
            },
        })
    }

    /// Raw signature bytes, strictly decoded from the hex encoding.
    ///
    /// # Errors
    ///
    /// Returns [`DescriptorError`] when the hex encoding is malformed.
    pub fn signature_bytes(&self) -> Result<Vec<u8>, DescriptorError> {
        hex_decode(&self.signature.signature)
            .ok_or_else(|| DescriptorError::schema_field_invalid("signature encoding".to_owned()))
    }
}

fn validate_plugin_id(id: &str) -> Result<(), DescriptorError> {
    let segments: Vec<_> = id.split('.').collect();
    if segments.len() < 2
        || id.len() > MAX_ID_BYTES
        || segments.iter().any(|segment| {
            segment.is_empty()
                || !segment.starts_with(char::is_alphanumeric)
                || !segment.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
                })
        })
    {
        return Err(DescriptorError::schema_field_invalid(
            "plugin id".to_owned(),
        ));
    }
    Ok(())
}

fn validate_id_text(value: &str, field: &'static str) -> Result<(), DescriptorError> {
    if value.is_empty()
        || value.len() > MAX_ID_BYTES
        || value.chars().any(char::is_control)
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
    {
        return Err(DescriptorError::schema_field_invalid(field.to_owned()));
    }
    Ok(())
}

fn validate_safe_text(value: &str, field: &'static str) -> Result<(), DescriptorError> {
    if value.is_empty() || value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(DescriptorError::schema_field_invalid(field.to_owned()));
    }
    Ok(())
}

/// Mirrors `SecretDeclaration.name` / `ProtectedSecretKey` validation from ticket 18 so
/// descriptor-declared secret references resolve against the same protected partitions.
fn validate_secret_name(value: &str) -> Result<(), DescriptorError> {
    let valid = !value.is_empty()
        && value.len() <= MAX_ID_BYTES
        && value.starts_with(|character: char| character.is_ascii_lowercase())
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        });
    if valid {
        Ok(())
    } else {
        Err(DescriptorError::schema_field_invalid(
            "secretReference.name".to_owned(),
        ))
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if value.len() != 128 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}
