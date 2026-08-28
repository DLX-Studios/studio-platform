#![allow(missing_docs)]

use std::collections::BTreeMap;

use studio_design::{
    Actor, ActorId, ActorKind, BrandSlot, Command, DesignToken, GeneratedSettingsSurface,
    ImportDestination, ImportProposal, ImportReview, ImportReviewError, ImportReviewStatus,
    ImportSource, InferredEntity, InstalledPlugin, NodeId, NodeParent, OperationId,
    ParentPlacement, PluginId, ProjectId, RevisionId, STUDIO_DESIGN_SCHEMA_VERSION, SettingKey,
    SettingValue, SourceProvenance, TemplateDefinition, TemplateNode, TemplateScreen, TokenId,
    TokenKind, TokenValue, UndoGroupId,
};
use studio_plugin_registry::{
    ApprovedKindCatalog, DescriptorPolicy, ExtensionRegistry, pos_pack_template_envelope,
    pos_pack_trust_store,
};
use studio_plugin_registry::{
    SettingsField, SettingsFieldType, SettingsGroup, pos_pack_descriptor,
};
use studio_protocol::NodeKind;

fn id<T>(
    value: &str,
    constructor: impl FnOnce(String) -> Result<T, studio_design::model::InvalidIdentity>,
) -> T {
    constructor(value.to_owned()).expect("valid test identity")
}

fn actor() -> Actor {
    Actor {
        id: id("designer", ActorId::new),
        kind: ActorKind::Human,
        display_name: "Designer".to_owned(),
    }
}

fn provenance() -> SourceProvenance {
    SourceProvenance {
        source_id: "source:fixture".to_owned(),
        source_label: "Fixture".to_owned(),
        source_locator: Some("fixture://source".to_owned()),
    }
}

fn node(id_value: &str, kind: NodeKind) -> TemplateNode {
    TemplateNode {
        id: id(id_value, NodeId::new),
        name: id_value.to_owned(),
        kind,
        properties: BTreeMap::new(),
        children: Vec::new(),
        provenance: provenance(),
    }
}

#[test]
fn settings_surface_renders_every_declared_control_kind() {
    let mut descriptor = pos_pack_descriptor();
    descriptor
        .contributions
        .settings_groups
        .push(SettingsGroup {
            id: "all-types".to_owned(),
            title: "All types".to_owned(),
            fields: vec![SettingsField {
                id: "image".to_owned(),
                label: "Image".to_owned(),
                kind: SettingsFieldType::Image,
            }],
        });
    let surface = GeneratedSettingsSurface::from_descriptor(
        id("com.studio.pack.pos", PluginId::new),
        &descriptor,
        &BTreeMap::new(),
    );
    let controls = surface
        .tabs
        .iter()
        .flat_map(|tab| &tab.fields)
        .map(|field| &field.control)
        .collect::<Vec<_>>();
    assert!(
        controls
            .iter()
            .any(|control| matches!(control, studio_design::ux::SettingsControl::Text { .. }))
    );
    assert!(
        controls
            .iter()
            .any(|control| matches!(control, studio_design::ux::SettingsControl::Number { .. }))
    );
    assert!(
        controls
            .iter()
            .any(|control| matches!(control, studio_design::ux::SettingsControl::Boolean { .. }))
    );
    assert!(
        controls
            .iter()
            .any(|control| matches!(control, studio_design::ux::SettingsControl::Color { .. }))
    );
    assert!(
        controls
            .iter()
            .any(|control| matches!(control, studio_design::ux::SettingsControl::Image { .. }))
    );
    assert!(
        controls
            .iter()
            .any(|control| matches!(control, studio_design::ux::SettingsControl::Select { .. }))
    );
    assert!(controls.iter().any(|control| matches!(
        control,
        studio_design::ux::SettingsControl::SecretReference { .. }
    )));
    assert!(controls.iter().any(|control| matches!(
        control,
        studio_design::ux::SettingsControl::DevicePicker { .. }
    )));
}

#[test]
fn setting_edit_is_a_typed_tracked_command() {
    let descriptor = pos_pack_descriptor();
    let key = SettingKey {
        plugin_id: id("com.studio.pack.pos", PluginId::new),
        group_id: "pos.receipt".to_owned(),
        field_id: "taxRate".to_owned(),
    };
    let batch = studio_design::ux::setting_change_batch(
        id("project", ProjectId::new),
        &descriptor,
        key.clone(),
        Some(SettingValue::Number("0.1".to_owned())),
        RevisionId::INITIAL,
        id("operation", OperationId::new),
        actor(),
        id("settings", UndoGroupId::new),
    )
    .expect("declared number accepts bounded value");
    assert_eq!(batch.undo_group_name, "Update Tax rate");
    assert!(matches!(
        batch.commands.as_slice(),
        [Command::SetSetting { key: actual, value: Some(SettingValue::Number(value)) }]
            if actual == &key && value == "0.1"
    ));
}

#[test]
fn template_install_and_rebrand_batches_have_narrow_command_scopes() {
    let plugin_id = id("com.example.template", PluginId::new);
    let plugin = InstalledPlugin {
        id: plugin_id.clone(),
        version: "1.0.0".to_owned(),
        publisher: "example".to_owned(),
        provenance: provenance(),
    };
    let token_id = id("brand.primary", TokenId::new);
    let template = TemplateDefinition {
        id: "restaurant".to_owned(),
        title: "Restaurant".to_owned(),
        version: "1.0.0".to_owned(),
        plugin,
        provenance: provenance(),
        screens: vec![TemplateScreen {
            id: id("screen.main", studio_design::ScreenId::new),
            name: "Main".to_owned(),
            route: "/".to_owned(),
            root: node("root", NodeKind::Column),
        }],
        tokens: vec![DesignToken {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: token_id.clone(),
            name: "Primary".to_owned(),
            kind: TokenKind::Color,
            value: TokenValue::Color(studio_design::ColorValue::SrgbHex("#123456".to_owned())),
        }],
        brand_slots: vec![BrandSlot {
            id: "primary".to_owned(),
            label: "Primary".to_owned(),
            token_id,
            kind: TokenKind::Color,
        }],
        settings: BTreeMap::new(),
    };
    let install = template.instantiate_batch(
        id("project", ProjectId::new),
        RevisionId::INITIAL,
        id("install", OperationId::new),
        actor(),
        id("install-group", UndoGroupId::new),
    );
    assert!(
        install
            .commands
            .iter()
            .any(|command| matches!(command, Command::SetPlugin { .. }))
    );
    assert!(
        install
            .commands
            .iter()
            .any(|command| matches!(command, Command::InsertScreen { .. }))
    );
    assert!(
        install
            .commands
            .iter()
            .any(|command| matches!(command, Command::InsertNode { .. }))
    );
    assert!(
        install
            .commands
            .iter()
            .all(|command| !matches!(command, Command::SetProperty { .. }))
    );
}

#[test]
fn import_review_requires_approval_and_keeps_source_provenance() {
    let source = provenance();
    let mut review = ImportReview {
        id: "review-1".to_owned(),
        sources: vec![ImportSource {
            provenance: source.clone(),
            media_type: "text/html".to_owned(),
        }],
        entities: vec![InferredEntity {
            id: "entity-1".to_owned(),
            label: "Imported heading".to_owned(),
            entity_kind: "node".to_owned(),
            confidence: 0.99,
        }],
        warnings: Vec::new(),
        destination: ImportDestination {
            project_id: id("project", ProjectId::new),
            parent: ParentPlacement {
                parent: NodeParent::Node {
                    node_id: id("root", NodeId::new),
                },
                index: 0,
            },
        },
        proposals: vec![ImportProposal {
            entity_id: "entity-1".to_owned(),
            source_id: source.source_id.clone(),
            node: node("imported", NodeKind::Text),
        }],
        status: ImportReviewStatus::Pending,
    };
    let blocked = review.command_batch(
        RevisionId::INITIAL,
        id("import", OperationId::new),
        actor(),
        id("import-group", UndoGroupId::new),
    );
    assert_eq!(blocked, Err(ImportReviewError::NotApproved));
    review.approve().expect("pending review approves");
    let batch = review
        .command_batch(
            RevisionId::INITIAL,
            id("import", OperationId::new),
            Actor {
                kind: ActorKind::Ingestion,
                ..actor()
            },
            id("import-group", UndoGroupId::new),
        )
        .expect("approved review emits commands");
    assert!(
        matches!(batch.commands.first(), Some(Command::InsertNode { node, .. }) if node.provenance.as_ref() == Some(&source))
    );
}

#[test]
fn admitted_registry_template_projects_to_browser_ready_definition() {
    let mut registry = ExtensionRegistry::new(
        DescriptorPolicy::default(),
        pos_pack_trust_store(),
        ApprovedKindCatalog::with_defaults(),
    );
    registry
        .admit(&pos_pack_template_envelope())
        .expect("fixture admits");
    let template =
        TemplateDefinition::from_registry(&registry, "com.studio.pack.pos", "pos.register")
            .expect("template projects");
    assert_eq!(template.screens.len(), 1);
    assert_eq!(template.brand_slots.len(), 1);
    assert_eq!(
        template.provenance.source_id,
        "plugin:com.studio.pack.pos/template:pos.register"
    );
}
