//! Designer-facing projections for admitted plugins, templates, settings, and ingestion.
//!
//! This module produces data and ordinary commands. Native GPUI views, agents, and MCP callers
//! can all consume the same projections without gaining a second mutation path.

#![allow(missing_docs)]
#![allow(clippy::all)]
#![allow(
    clippy::doc_markdown,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::needless_pass_by_value,
    clippy::too_many_arguments
)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use studio_plugin_registry::{
    ExtensionRegistry, PluginDescriptorV1, PrimitiveInputValue, SelectOption, SettingsField,
    SettingsFieldType, SettingsGroup, TemplateContribution,
};
use studio_protocol::NodeKind;
use thiserror::Error;

use crate::{
    command::{Command, CommandBatch, ParentPlacement},
    model::{
        AccessibilityProperties, Actor, DesignNode, DesignNodeSource, DesignToken, InstalledPlugin,
        LayoutProperties, NodeId, NodeParent, OperationId, PluginId, ProjectId, PropertyValue,
        RevisionId, STUDIO_DESIGN_SCHEMA_VERSION, Screen, ScreenId, SettingKey, SettingValue,
        SourceProvenance, StudioDesignSnapshot, StyleProperties, TokenId, TokenKind, TokenValue,
        UndoGroupId,
    },
    session::{CommandOutcome, DesignerQuery, DesignerQueryResult, DesignerSession},
};

/// One generated settings control. Every descriptor field maps to exactly one variant.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SettingsControl {
    Text {
        value: Option<String>,
        max_length: Option<u32>,
    },
    Number {
        value: Option<String>,
        min: Option<f64>,
        max: Option<f64>,
    },
    Boolean {
        value: bool,
    },
    Color {
        value: Option<String>,
    },
    Image {
        asset: Option<crate::LibraryAssetId>,
    },
    Select {
        options: Vec<SelectOption>,
        value: Option<String>,
    },
    SecretReference {
        name: String,
        purpose: String,
        configured: bool,
    },
    DevicePicker {
        device_kind: String,
        device: Option<String>,
    },
}

/// One field in a generated settings tab.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsFieldView {
    pub key: SettingKey,
    pub label: String,
    pub control: SettingsControl,
}

/// One top-level settings tab generated from a plugin declaration.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsTab {
    pub id: String,
    pub title: String,
    pub fields: Vec<SettingsFieldView>,
}

/// Complete generated settings surface for one admitted plugin.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedSettingsSurface {
    pub plugin_id: PluginId,
    pub plugin_name: String,
    pub tabs: Vec<SettingsTab>,
}

impl GeneratedSettingsSurface {
    /// Render every declared group and field in declaration order.
    #[must_use]
    pub fn from_descriptor(
        plugin_id: PluginId,
        descriptor: &PluginDescriptorV1,
        settings: &BTreeMap<SettingKey, SettingValue>,
    ) -> Self {
        let tabs = descriptor
            .contributions
            .settings_groups
            .iter()
            .map(|group| settings_tab(&plugin_id, group, settings))
            .collect();
        Self {
            plugin_id,
            plugin_name: descriptor.name.clone(),
            tabs,
        }
    }

    /// Find a generated field by its stable key.
    #[must_use]
    pub fn field(&self, key: &SettingKey) -> Option<&SettingsFieldView> {
        self.tabs
            .iter()
            .flat_map(|tab| &tab.fields)
            .find(|field| &field.key == key)
    }
}

fn settings_tab(
    plugin_id: &PluginId,
    group: &SettingsGroup,
    settings: &BTreeMap<SettingKey, SettingValue>,
) -> SettingsTab {
    let fields = group
        .fields
        .iter()
        .map(|field| {
            let key = SettingKey {
                plugin_id: plugin_id.clone(),
                group_id: group.id.clone(),
                field_id: field.id.clone(),
            };
            SettingsFieldView {
                label: field.label.clone(),
                control: settings_control(field, settings.get(&key)),
                key,
            }
        })
        .collect();
    SettingsTab {
        id: group.id.clone(),
        title: group.title.clone(),
        fields,
    }
}

fn settings_control(field: &SettingsField, value: Option<&SettingValue>) -> SettingsControl {
    match &field.kind {
        SettingsFieldType::Text {
            max_length,
            default,
        } => SettingsControl::Text {
            value: setting_text(value).or_else(|| default.clone()),
            max_length: *max_length,
        },
        SettingsFieldType::Number { min, max, default } => SettingsControl::Number {
            value: setting_number(value).or_else(|| default.map(|value| value.to_string())),
            min: *min,
            max: *max,
        },
        SettingsFieldType::Boolean { default } => SettingsControl::Boolean {
            value: setting_bool(value).unwrap_or(*default),
        },
        SettingsFieldType::Color { default } => SettingsControl::Color {
            value: setting_color(value).or_else(|| default.clone()),
        },
        SettingsFieldType::Image => SettingsControl::Image {
            asset: setting_image(value),
        },
        SettingsFieldType::Select { options, default } => SettingsControl::Select {
            options: options.clone(),
            value: setting_select(value).or_else(|| default.clone()),
        },
        SettingsFieldType::SecretReference { name, purpose } => SettingsControl::SecretReference {
            name: name.clone(),
            purpose: purpose.clone(),
            configured: matches!(value, Some(SettingValue::SecretReference(_))),
        },
        SettingsFieldType::DevicePicker { device_kind } => SettingsControl::DevicePicker {
            device_kind: device_kind.clone(),
            device: setting_device(value),
        },
    }
}

fn setting_text(value: Option<&SettingValue>) -> Option<String> {
    match value {
        Some(SettingValue::Text(value)) => Some(value.clone()),
        _ => None,
    }
}
fn setting_number(value: Option<&SettingValue>) -> Option<String> {
    match value {
        Some(SettingValue::Number(value)) => Some(value.clone()),
        _ => None,
    }
}
fn setting_bool(value: Option<&SettingValue>) -> Option<bool> {
    match value {
        Some(SettingValue::Boolean(value)) => Some(*value),
        _ => None,
    }
}
fn setting_color(value: Option<&SettingValue>) -> Option<String> {
    match value {
        Some(SettingValue::Color(value)) => Some(value.clone()),
        _ => None,
    }
}
fn setting_image(value: Option<&SettingValue>) -> Option<crate::LibraryAssetId> {
    match value {
        Some(SettingValue::Image(value)) => Some(value.clone()),
        _ => None,
    }
}
fn setting_select(value: Option<&SettingValue>) -> Option<String> {
    match value {
        Some(SettingValue::Select(value)) => Some(value.clone()),
        _ => None,
    }
}
fn setting_device(value: Option<&SettingValue>) -> Option<String> {
    match value {
        Some(SettingValue::Device(value)) => Some(value.clone()),
        _ => None,
    }
}

/// Rejection while turning a generated control edit into a tracked command.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SettingsError {
    #[error("settings field {0} does not exist in the plugin descriptor")]
    FieldMissing(String),
    #[error("value for settings field {field} has the wrong type")]
    TypeMismatch { field: String },
    #[error("value for settings field {field} violates its declared bounds")]
    ValueInvalid { field: String },
}

/// Build the ordinary tracked command used by every generated settings edit.
pub fn setting_change_batch(
    project_id: ProjectId,
    descriptor: &PluginDescriptorV1,
    key: SettingKey,
    value: Option<SettingValue>,
    base_revision: RevisionId,
    operation_id: OperationId,
    actor: Actor,
    undo_group_id: UndoGroupId,
) -> Result<CommandBatch, SettingsError> {
    let field = descriptor
        .contributions
        .settings_groups
        .iter()
        .find(|group| group.id == key.group_id)
        .and_then(|group| group.fields.iter().find(|field| field.id == key.field_id))
        .ok_or_else(|| SettingsError::FieldMissing(key.field_id.clone()))?;
    validate_setting_value(field, value.as_ref())?;
    Ok(CommandBatch {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        operation_id,
        actor,
        project_id,
        base_revision,
        undo_group_id,
        undo_group_name: format!("Update {}", field.label),
        preconditions: Vec::new(),
        commands: vec![Command::SetSetting { key, value }],
    })
}

/// Build the ordinary tracked project command for installing an admitted integration plugin.
#[must_use]
pub fn plugin_install_batch(
    project_id: ProjectId,
    descriptor: &PluginDescriptorV1,
    provenance: SourceProvenance,
    base_revision: RevisionId,
    operation_id: OperationId,
    actor: Actor,
    undo_group_id: UndoGroupId,
) -> CommandBatch {
    let plugin_id =
        PluginId::new(descriptor.id.clone()).expect("admitted descriptor has a valid id");
    CommandBatch {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        operation_id,
        actor,
        project_id,
        base_revision,
        undo_group_id,
        undo_group_name: format!("Install plugin: {}", descriptor.name),
        preconditions: Vec::new(),
        commands: vec![Command::SetPlugin {
            plugin_id: plugin_id.clone(),
            plugin: Some(InstalledPlugin {
                id: plugin_id,
                version: descriptor.version.clone(),
                publisher: descriptor.publisher.id.clone(),
                provenance,
            }),
        }],
    }
}

fn validate_setting_value(
    field: &SettingsField,
    value: Option<&SettingValue>,
) -> Result<(), SettingsError> {
    let Some(value) = value else { return Ok(()) };
    let valid = match (&field.kind, value) {
        (SettingsFieldType::Text { max_length, .. }, SettingValue::Text(value)) => {
            max_length.is_none_or(|limit| value.chars().count() <= limit as usize)
                && !value.chars().any(char::is_control)
        }
        (SettingsFieldType::Number { min, max, .. }, SettingValue::Number(value)) => {
            value.parse::<f64>().is_ok_and(|number| {
                number.is_finite()
                    && min.is_none_or(|bound| number >= bound)
                    && max.is_none_or(|bound| number <= bound)
            })
        }
        (SettingsFieldType::Boolean { .. }, SettingValue::Boolean(_)) => true,
        (SettingsFieldType::Color { .. }, SettingValue::Color(value)) => is_hex_color(value),
        (SettingsFieldType::Image, SettingValue::Image(_)) => true,
        (SettingsFieldType::Select { options, .. }, SettingValue::Select(value)) => {
            options.iter().any(|option| option.value == *value)
        }
        (SettingsFieldType::SecretReference { name, .. }, SettingValue::SecretReference(value)) => {
            value == name
        }
        (SettingsFieldType::DevicePicker { .. }, SettingValue::Device(value)) => {
            !value.is_empty() && !value.chars().any(char::is_control)
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else if matches_type(&field.kind, value) {
        Err(SettingsError::ValueInvalid {
            field: field.id.clone(),
        })
    } else {
        Err(SettingsError::TypeMismatch {
            field: field.id.clone(),
        })
    }
}

fn matches_type(field: &SettingsFieldType, value: &SettingValue) -> bool {
    matches!(
        (field, value),
        (SettingsFieldType::Text { .. }, SettingValue::Text(_))
            | (SettingsFieldType::Number { .. }, SettingValue::Number(_))
            | (SettingsFieldType::Boolean { .. }, SettingValue::Boolean(_))
            | (SettingsFieldType::Color { .. }, SettingValue::Color(_))
            | (SettingsFieldType::Image, SettingValue::Image(_))
            | (SettingsFieldType::Select { .. }, SettingValue::Select(_))
            | (
                SettingsFieldType::SecretReference { .. },
                SettingValue::SecretReference(_)
            )
            | (
                SettingsFieldType::DevicePicker { .. },
                SettingValue::Device(_)
            )
    )
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// An authored template node before it is flattened into ordinary InsertNode commands.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateNode {
    pub id: NodeId,
    pub name: String,
    pub kind: NodeKind,
    pub properties: BTreeMap<String, PropertyValue>,
    pub children: Vec<Self>,
    pub provenance: SourceProvenance,
}

/// One screen seeded by a template.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateScreen {
    pub id: ScreenId,
    pub name: String,
    pub route: String,
    pub root: TemplateNode,
}

/// A browser-ready vertical template with brand slots and initial settings.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateDefinition {
    pub id: String,
    pub title: String,
    pub version: String,
    pub plugin: InstalledPlugin,
    pub provenance: SourceProvenance,
    pub screens: Vec<TemplateScreen>,
    pub tokens: Vec<DesignToken>,
    pub brand_slots: Vec<BrandSlot>,
    pub settings: BTreeMap<SettingKey, SettingValue>,
}

/// One customer-editable template token slot.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrandSlot {
    pub id: String,
    pub label: String,
    pub token_id: TokenId,
    pub kind: TokenKind,
}

/// Rejection while projecting or instantiating a template.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TemplateError {
    #[error("template {0} does not exist")]
    Missing(String),
    #[error("template {0} contains an invalid node or screen identity")]
    Invalid(String),
    #[error("template token {0} could not be decoded")]
    TokenInvalid(String),
    #[error("template screen tree kind {0} is not in the Runtime catalog")]
    NodeKindInvalid(String),
    #[error("template {template} has no token for brand slot {slot}")]
    BrandSlotMissing { template: String, slot: String },
    #[error("brand slot {slot} expects token kind {expected:?}")]
    BrandSlotKindMismatch { slot: String, expected: TokenKind },
}

impl TemplateDefinition {
    /// Resolve one admitted plugin/template pair from the registry and project it for Designer.
    pub fn from_registry(
        registry: &ExtensionRegistry,
        plugin_id: &str,
        template_id: &str,
    ) -> Result<Self, TemplateError> {
        let extension = registry
            .plugin(plugin_id)
            .ok_or_else(|| TemplateError::Missing(plugin_id.to_owned()))?;
        let contribution = extension
            .descriptor
            .contributions
            .templates
            .iter()
            .find(|template| template.id == template_id)
            .ok_or_else(|| TemplateError::Missing(template_id.to_owned()))?;
        let plugin_id = PluginId::new(plugin_id.to_owned())
            .map_err(|_| TemplateError::Invalid(plugin_id.to_owned()))?;
        Self::from_contribution(plugin_id, &extension.descriptor, contribution)
    }

    /// Project an admitted registry contribution into a host-independent template definition.
    pub fn from_contribution(
        plugin_id: PluginId,
        descriptor: &PluginDescriptorV1,
        contribution: &TemplateContribution,
    ) -> Result<Self, TemplateError> {
        let provenance = SourceProvenance {
            source_id: format!("plugin:{plugin_id}/template:{}", contribution.id),
            source_label: descriptor.name.clone(),
            source_locator: Some(format!("plugin://{plugin_id}/{}", contribution.id)),
        };
        let plugin = InstalledPlugin {
            id: plugin_id,
            version: descriptor.version.clone(),
            publisher: descriptor.publisher.id.clone(),
            provenance: provenance.clone(),
        };
        let screens = contribution
            .screens
            .iter()
            .map(|screen| {
                let id = ScreenId::new(format!("{}.{}", contribution.id, screen.id))
                    .map_err(|_| TemplateError::Invalid(screen.id.clone()))?;
                let root = template_node(
                    &contribution.id,
                    &screen.id,
                    &screen.tree,
                    &provenance,
                    &mut Vec::new(),
                )?;
                Ok(TemplateScreen {
                    id,
                    name: screen.title.clone(),
                    route: screen.route.clone(),
                    root,
                })
            })
            .collect::<Result<Vec<_>, TemplateError>>()?;
        let tokens = contribution
            .tokens
            .iter()
            .map(|token| {
                let id = TokenId::new(format!("{}.{}", contribution.id, token.id))
                    .map_err(|_| TemplateError::TokenInvalid(token.id.clone()))?;
                let kind = token_kind(&token.kind)
                    .ok_or_else(|| TemplateError::TokenInvalid(token.id.clone()))?;
                let value: TokenValue = serde_json::from_value(token.value.clone())
                    .map_err(|_| TemplateError::TokenInvalid(token.id.clone()))?;
                if !token_value_matches(kind, &value) {
                    return Err(TemplateError::TokenInvalid(token.id.clone()));
                }
                Ok(DesignToken {
                    schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                    id,
                    name: token.name.clone(),
                    kind,
                    value,
                })
            })
            .collect::<Result<Vec<_>, TemplateError>>()?;
        let token_ids = tokens
            .iter()
            .map(|token| token.id.clone())
            .collect::<BTreeSet<_>>();
        let brand_slots = contribution
            .brand_slots
            .iter()
            .map(|slot| {
                let token_id = TokenId::new(format!("{}.{}", contribution.id, slot.token_id))
                    .map_err(|_| TemplateError::Invalid(slot.id.clone()))?;
                let kind = token_kind(&slot.kind)
                    .ok_or_else(|| TemplateError::Invalid(slot.id.clone()))?;
                if !token_ids.contains(&token_id) {
                    return Err(TemplateError::Invalid(slot.id.clone()));
                }
                let token = tokens
                    .iter()
                    .find(|token| token.id == token_id)
                    .expect("token id checked");
                if token.kind != kind {
                    return Err(TemplateError::Invalid(slot.id.clone()));
                }
                Ok(BrandSlot {
                    id: slot.id.clone(),
                    label: slot.label.clone(),
                    token_id,
                    kind,
                })
            })
            .collect::<Result<Vec<_>, TemplateError>>()?;
        Ok(Self {
            id: contribution.id.clone(),
            title: contribution.title.clone(),
            version: descriptor.version.clone(),
            plugin,
            provenance,
            screens,
            tokens,
            brand_slots,
            settings: BTreeMap::new(),
        })
    }

    /// Build one atomic command batch that installs the plugin, screens, tokens, and settings.
    #[must_use]
    pub fn instantiate_batch(
        &self,
        project_id: ProjectId,
        base_revision: RevisionId,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> CommandBatch {
        let mut commands = vec![Command::SetPlugin {
            plugin_id: self.plugin.id.clone(),
            plugin: Some(self.plugin.clone()),
        }];
        commands.extend(self.tokens.iter().map(|token| Command::SetToken {
            token_id: token.id.clone(),
            token: Some(token.clone()),
        }));
        commands.extend(
            self.settings
                .iter()
                .map(|(key, value)| Command::SetSetting {
                    key: key.clone(),
                    value: Some(value.clone()),
                }),
        );
        for (index, screen) in self.screens.iter().enumerate() {
            commands.push(Command::InsertScreen {
                screen: Box::new(Screen {
                    schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                    id: screen.id.clone(),
                    name: screen.name.clone(),
                    route: screen.route.clone(),
                    root_node_id: screen.root.id.clone(),
                }),
                index,
            });
            flatten_node(
                &screen.root,
                NodeParent::Screen {
                    screen_id: screen.id.clone(),
                },
                0,
                &mut commands,
            );
        }
        CommandBatch {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            operation_id,
            actor,
            project_id,
            base_revision,
            undo_group_id,
            undo_group_name: format!("Install template: {}", self.title),
            preconditions: Vec::new(),
            commands,
        }
    }

    /// Build a token-only rebrand batch. No node, layout, or screen command is emitted.
    pub fn rebrand_batch(
        &self,
        project_id: ProjectId,
        snapshot: &StudioDesignSnapshot,
        replacements: &BTreeMap<String, TokenValue>,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> Result<CommandBatch, TemplateError> {
        let mut commands = Vec::with_capacity(replacements.len());
        for (slot_id, value) in replacements {
            let slot = self
                .brand_slots
                .iter()
                .find(|slot| slot.id == *slot_id)
                .ok_or_else(|| TemplateError::BrandSlotMissing {
                    template: self.id.clone(),
                    slot: slot_id.clone(),
                })?;
            if !token_value_matches(slot.kind, value) {
                return Err(TemplateError::BrandSlotKindMismatch {
                    slot: slot_id.clone(),
                    expected: slot.kind,
                });
            }
            let mut token = snapshot
                .design
                .tokens
                .get(&slot.token_id)
                .cloned()
                .or_else(|| {
                    self.tokens
                        .iter()
                        .find(|token| token.id == slot.token_id)
                        .cloned()
                })
                .ok_or_else(|| TemplateError::BrandSlotMissing {
                    template: self.id.clone(),
                    slot: slot_id.clone(),
                })?;
            token.value = value.clone();
            commands.push(Command::SetToken {
                token_id: slot.token_id.clone(),
                token: Some(token),
            });
        }
        Ok(CommandBatch {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            operation_id,
            actor,
            project_id,
            base_revision: snapshot.revision.id,
            undo_group_id,
            undo_group_name: format!("Rebrand template: {}", self.title),
            preconditions: Vec::new(),
            commands,
        })
    }
}

fn flatten_node(
    node: &TemplateNode,
    parent: NodeParent,
    index: usize,
    commands: &mut Vec<Command>,
) {
    let authored = DesignNode {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        id: node.id.clone(),
        name: node.name.clone(),
        source: DesignNodeSource::Primitive { kind: node.kind },
        children: Vec::new(),
        properties: node.properties.clone(),
        token_overrides: BTreeMap::new(),
        layout: LayoutProperties::default(),
        style: StyleProperties::default(),
        accessibility: AccessibilityProperties::default(),
        responsive_overrides: BTreeMap::new(),
        interaction_ids: Vec::new(),
        provenance: Some(node.provenance.clone()),
    };
    commands.push(Command::InsertNode {
        parent: ParentPlacement {
            parent: parent.clone(),
            index,
        },
        node: Box::new(authored),
    });
    for (index, child) in node.children.iter().enumerate() {
        flatten_node(
            child,
            NodeParent::Node {
                node_id: node.id.clone(),
            },
            index,
            commands,
        );
    }
}

fn template_node(
    template_id: &str,
    screen_id: &str,
    node: &studio_plugin_registry::CompositionNode,
    provenance: &SourceProvenance,
    path: &mut Vec<usize>,
) -> Result<TemplateNode, TemplateError> {
    let suffix = path
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(".");
    let id = NodeId::new(format!("{template_id}.{screen_id}.{suffix}"))
        .map_err(|_| TemplateError::Invalid(node.kind.clone()))?;
    let kind: NodeKind = serde_json::from_value(Value::String(node.kind.clone()))
        .map_err(|_| TemplateError::NodeKindInvalid(node.kind.clone()))?;
    let properties = node
        .inputs
        .iter()
        .map(|(key, value)| (key.clone(), primitive_value(value)))
        .collect();
    let mut children = Vec::with_capacity(node.children.len());
    for (index, child) in node.children.iter().enumerate() {
        path.push(index);
        children.push(template_node(
            template_id,
            screen_id,
            child,
            provenance,
            path,
        )?);
        path.pop();
    }
    Ok(TemplateNode {
        id,
        name: node.kind.clone(),
        kind,
        properties,
        children,
        provenance: provenance.clone(),
    })
}

fn primitive_value(value: &PrimitiveInputValue) -> PropertyValue {
    match value {
        PrimitiveInputValue::Text(value) => PropertyValue::String(value.clone()),
        PrimitiveInputValue::Number(value) => PropertyValue::Decimal(value.to_string()),
        PrimitiveInputValue::Boolean(value) => PropertyValue::Boolean(*value),
    }
}

fn token_kind(value: &str) -> Option<TokenKind> {
    match value {
        "color" => Some(TokenKind::Color),
        "length" => Some(TokenKind::Length),
        "number" => Some(TokenKind::Number),
        "string" => Some(TokenKind::String),
        "typography" => Some(TokenKind::Typography),
        _ => None,
    }
}

fn token_value_matches(kind: TokenKind, value: &TokenValue) -> bool {
    matches!(
        (kind, value),
        (TokenKind::Color, TokenValue::Color(_))
            | (TokenKind::Length, TokenValue::Length(_))
            | (TokenKind::Number, TokenValue::Number(_))
            | (TokenKind::String, TokenValue::String(_))
            | (TokenKind::Typography, TokenValue::Typography(_))
    )
}

/// A compact browse card for one admitted plugin and its templates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginBrowseCard {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub settings_group_count: usize,
    pub template_ids: Vec<String>,
}

/// Registry-backed Designer browser projection.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PluginCatalog {
    pub plugins: Vec<PluginBrowseCard>,
}

impl PluginCatalog {
    /// Build a deterministic browser list from admitted extensions.
    #[must_use]
    pub fn from_registry(registry: &ExtensionRegistry) -> Self {
        let mut plugins = registry
            .admitted_extensions()
            .into_iter()
            .map(|extension| PluginBrowseCard {
                plugin_id: extension.descriptor.id.clone(),
                name: extension.descriptor.name.clone(),
                version: extension.descriptor.version.clone(),
                publisher: extension.descriptor.publisher.id.clone(),
                settings_group_count: extension.descriptor.contributions.settings_groups.len(),
                template_ids: extension
                    .descriptor
                    .contributions
                    .templates
                    .iter()
                    .map(|template| template.id.clone())
                    .collect(),
            })
            .collect::<Vec<_>>();
        plugins.sort_by(|left, right| left.plugin_id.cmp(&right.plugin_id));
        Self { plugins }
    }
}

/// One bounded source shown in an import review.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportSource {
    pub provenance: SourceProvenance,
    pub media_type: String,
}

/// Inferred entity metadata shown before it becomes a design command.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InferredEntity {
    pub id: String,
    pub label: String,
    pub entity_kind: String,
    pub confidence: f32,
}

/// One warning surfaced in an import review.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportWarning {
    pub code: String,
    pub message: String,
    pub severity: crate::DiagnosticSeverity,
}

/// Destination selected by the designer before import commands are built.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportDestination {
    pub project_id: ProjectId,
    pub parent: ParentPlacement,
}

/// A proposed node insertion associated with an inferred entity and source.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportProposal {
    pub entity_id: String,
    pub source_id: String,
    pub node: TemplateNode,
}

/// Review lifecycle; only approved reviews can produce executable commands.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportReviewStatus {
    Pending,
    Approved,
    Applied,
    Rejected,
}

/// Controlled import plan retaining sources, inference, warnings, and destination.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImportReview {
    pub id: String,
    pub sources: Vec<ImportSource>,
    pub entities: Vec<InferredEntity>,
    pub warnings: Vec<ImportWarning>,
    pub destination: ImportDestination,
    pub proposals: Vec<ImportProposal>,
    pub status: ImportReviewStatus,
}

/// Import review rejection family.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ImportReviewError {
    #[error("import review must be approved before commands can execute")]
    NotApproved,
    #[error("import review is already {0:?}")]
    InvalidStatus(ImportReviewStatus),
    #[error("import proposal references unknown source {0}")]
    SourceMissing(String),
    #[error("import proposal references unknown entity {0}")]
    EntityMissing(String),
}

impl ImportReview {
    /// Approve a pending review after its evidence has been inspected.
    pub fn approve(&mut self) -> Result<(), ImportReviewError> {
        if self.status != ImportReviewStatus::Pending {
            return Err(ImportReviewError::InvalidStatus(self.status));
        }
        self.status = ImportReviewStatus::Approved;
        Ok(())
    }

    /// Reject a pending review without producing commands.
    pub fn reject(&mut self) -> Result<(), ImportReviewError> {
        if self.status != ImportReviewStatus::Pending {
            return Err(ImportReviewError::InvalidStatus(self.status));
        }
        self.status = ImportReviewStatus::Rejected;
        Ok(())
    }

    /// Build ordinary InsertNode commands, refusing execution before approval.
    pub fn command_batch(
        &self,
        base_revision: RevisionId,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> Result<CommandBatch, ImportReviewError> {
        if self.status != ImportReviewStatus::Approved {
            return Err(ImportReviewError::NotApproved);
        }
        let source_map = self
            .sources
            .iter()
            .map(|source| (source.provenance.source_id.clone(), &source.provenance))
            .collect::<BTreeMap<_, _>>();
        let entity_ids = self
            .entities
            .iter()
            .map(|entity| entity.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut commands = Vec::new();
        for (offset, proposal) in self.proposals.iter().enumerate() {
            let provenance = source_map
                .get(&proposal.source_id)
                .ok_or_else(|| ImportReviewError::SourceMissing(proposal.source_id.clone()))?;
            if !entity_ids.contains(proposal.entity_id.as_str()) {
                return Err(ImportReviewError::EntityMissing(proposal.entity_id.clone()));
            }
            let mut node = proposal.node.clone();
            set_provenance(&mut node, provenance);
            flatten_node(
                &node,
                self.destination.parent.parent.clone(),
                self.destination.parent.index.saturating_add(offset),
                &mut commands,
            );
        }
        Ok(CommandBatch {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            operation_id,
            actor,
            project_id: self.destination.project_id.clone(),
            base_revision,
            undo_group_id,
            undo_group_name: format!("Import {}", self.id),
            preconditions: Vec::new(),
            commands,
        })
    }

    /// Apply the reviewed batch through the existing session seam and retain applied state.
    pub async fn apply<S: DesignerSession>(
        &mut self,
        session: &mut S,
        operation_id: OperationId,
        actor: Actor,
        undo_group_id: UndoGroupId,
    ) -> Result<CommandOutcome, ImportReviewError> {
        let snapshot = match session.query(DesignerQuery::Snapshot) {
            DesignerQueryResult::Snapshot(snapshot) => snapshot,
            _ => return Err(ImportReviewError::NotApproved),
        };
        let batch = self.command_batch(snapshot.revision.id, operation_id, actor, undo_group_id)?;
        let outcome = session.submit(batch).await;
        if matches!(outcome, CommandOutcome::Accepted(_)) {
            self.status = ImportReviewStatus::Applied;
        }
        Ok(outcome)
    }
}

fn set_provenance(node: &mut TemplateNode, provenance: &SourceProvenance) {
    node.provenance = provenance.clone();
    for child in &mut node.children {
        set_provenance(child, provenance);
    }
}
