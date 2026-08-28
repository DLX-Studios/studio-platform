use std::collections::BTreeMap;
use std::fs;
use std::process::Command as ProcessCommand;

use studio_design::{
    Actor, ActorId, ActorKind, Command, CommandBatch, CompositionId, CompositionInput, DesignNode,
    DesignToken, InputEnvironment, Interaction, InteractionAction, InteractionEvent, InteractionId,
    InteractionSource, NavigationMode, NodeId, NodeKind, NodeParent, OperationId, ParentPlacement,
    ProjectId, PropertyValue, ResponsiveVariant, ReusableComposition, RevisionId, Screen, ScreenId,
    StudioDesign, TokenId, TokenKind, TokenValue, UndoGroupId, ValueKind,
    STUDIO_DESIGN_SCHEMA_VERSION,
};

fn actor() -> Actor {
    Actor {
        id: ActorId::new("cli-test").unwrap(),
        kind: ActorKind::Human,
        display_name: "CLI test".to_owned(),
    }
}

fn design() -> StudioDesign {
    let project_id = ProjectId::new("cli-replay-project").unwrap();
    let screen_id = ScreenId::new("main").unwrap();
    let root_id = NodeId::new("root").unwrap();
    let button_id = NodeId::new("button").unwrap();
    let composition_root_id = NodeId::new("card-root").unwrap();
    let mut root = DesignNode::primitive(root_id.clone(), "Root", NodeKind::Box);
    root.children = vec![button_id.clone()];
    let button = DesignNode::primitive(button_id.clone(), "Button", NodeKind::Button);
    let composition_root = DesignNode::primitive(
        composition_root_id.clone(),
        "Card definition",
        NodeKind::Box,
    );
    let mut design = StudioDesign::empty(project_id, "CLI replay");
    design.nodes = [
        (root_id.clone(), root),
        (button_id.clone(), button),
        (composition_root_id.clone(), composition_root),
    ]
    .into_iter()
    .collect();
    design.parents = [
        (
            root_id,
            NodeParent::Screen {
                screen_id: screen_id.clone(),
            },
        ),
        (
            button_id,
            NodeParent::Node {
                node_id: NodeId::new("root").unwrap(),
            },
        ),
        (
            composition_root_id,
            NodeParent::Composition {
                composition_id: CompositionId::new("card").unwrap(),
            },
        ),
    ]
    .into_iter()
    .collect();
    design.screens.insert(
        screen_id.clone(),
        Screen {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: screen_id.clone(),
            name: "Main".to_owned(),
            route: "/".to_owned(),
            root_node_id: NodeId::new("root").unwrap(),
        },
    );
    design.screen_order.push(screen_id);
    design
}

fn batch(base_revision: u64, operation: &str, commands: Vec<Command>) -> CommandBatch {
    CommandBatch {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        operation_id: OperationId::new(operation).unwrap(),
        actor: actor(),
        project_id: ProjectId::new("cli-replay-project").unwrap(),
        base_revision: RevisionId::new(base_revision),
        undo_group_id: UndoGroupId::new(operation).unwrap(),
        undo_group_name: operation.to_owned(),
        preconditions: Vec::new(),
        commands,
    }
}

#[test]
fn replay_runs_typed_families_and_reopens_deterministically() {
    let project_id = ProjectId::new("cli-replay-project").unwrap();
    let screen_id = ScreenId::new("main").unwrap();
    let root_id = NodeId::new("root").unwrap();
    let button_id = NodeId::new("button").unwrap();
    let composition_id = CompositionId::new("card").unwrap();
    let token_id = TokenId::new("surface").unwrap();
    let variant_id = studio_design::ResponsiveVariantId::new("phone").unwrap();
    let interaction_id = InteractionId::new("open-main").unwrap();
    let mut input_batches = Vec::new();
    input_batches.push(batch(
        0,
        "define-profile",
        vec![Command::DefineResponsiveVariant {
            variant: ResponsiveVariant {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                id: variant_id.clone(),
                name: "Phone".to_owned(),
                minimum_width: None,
                maximum_width: Some(600),
                input: InputEnvironment::Touch,
            },
        }],
    ));
    input_batches.push(batch(
        1,
        "define-token",
        vec![Command::DefineToken {
            token: DesignToken {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                id: token_id.clone(),
                name: "Surface".to_owned(),
                kind: TokenKind::Color,
                value: TokenValue::Color(studio_design::ColorValue::SrgbHex("#ffffff".to_owned())),
            },
        }],
    ));
    input_batches.push(batch(
        2,
        "apply-token",
        vec![Command::ApplyToken {
            node_id: root_id.clone(),
            property: "background".to_owned(),
            token_id: token_id.clone(),
        }],
    ));
    input_batches.push(batch(
        3,
        "set-binding",
        vec![Command::SetBinding {
            node_id: button_id.clone(),
            property: "label".to_owned(),
            binding: Some(studio_design::BindingPath {
                collection: "menu".to_owned(),
                segments: vec!["title".to_owned()],
            }),
        }],
    ));
    input_batches.push(batch(
        4,
        "define-composition",
        vec![Command::DefineComposition {
            composition: ReusableComposition {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                id: composition_id.clone(),
                name: "Card".to_owned(),
                definition_version: 1,
                root_node_id: NodeId::new("card-root").unwrap(),
                inputs: [(
                    "label".to_owned(),
                    CompositionInput {
                        value_kind: ValueKind::String,
                        required: true,
                        default: None,
                        overridable: true,
                    },
                )]
                .into_iter()
                .collect(),
                slots: BTreeMap::new(),
            },
        }],
    ));
    input_batches.push(batch(
        5,
        "instantiate-composition",
        vec![Command::InstantiateComposition {
            node_id: NodeId::new("card-instance").unwrap(),
            name: "Card instance".to_owned(),
            parent: ParentPlacement {
                parent: NodeParent::Node {
                    node_id: root_id.clone(),
                },
                index: 1,
            },
            composition_id: composition_id.clone(),
            inputs: [(
                "label".to_owned(),
                PropertyValue::String("Dinner".to_owned()),
            )]
            .into_iter()
            .collect(),
        }],
    ));
    input_batches.push(batch(
        6,
        "define-interaction",
        vec![Command::DefineInteraction {
            interaction: Interaction {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                id: interaction_id,
                source: InteractionSource {
                    node_id: button_id,
                    event: InteractionEvent::Pressed,
                },
                action: InteractionAction::Navigate {
                    screen_id,
                    mode: NavigationMode::Push,
                },
            },
        }],
    ));
    input_batches.push(batch(
        7,
        "allowed-override",
        vec![Command::SetCompositionOverride {
            node_id: NodeId::new("card-instance").unwrap(),
            input: "label".to_owned(),
            value: Some(PropertyValue::String("Dessert".to_owned())),
        }],
    ));
    input_batches.push(batch(
        8,
        "forbidden-override",
        vec![Command::SetCompositionOverride {
            node_id: NodeId::new("card-instance").unwrap(),
            input: "missing".to_owned(),
            value: Some(PropertyValue::String("rejected".to_owned())),
        }],
    ));

    let path = std::env::temp_dir().join(format!(
        "studio-cli-replay-{}-{}.json",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let input = serde_json::json!({
        "design": design(),
        "batches": input_batches,
    });
    fs::write(&path, serde_json::to_vec(&input).unwrap()).unwrap();
    let output = ProcessCommand::new(env!("CARGO_BIN_EXE_studio"))
        .args(["replay", path.to_str().unwrap()])
        .output()
        .unwrap();
    fs::remove_file(path).unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["deterministic"], true);
    assert_eq!(report["outcomes"].as_array().unwrap().len(), 9);
    assert_eq!(report["outcomes"][0]["status"], "accepted");
    assert_eq!(report["outcomes"][7]["status"], "accepted");
    assert_eq!(report["outcomes"][8]["status"], "rejected");
    assert_eq!(report["snapshot"]["revision"]["id"], 8);
    assert_eq!(report["reopened_snapshot"], report["snapshot"]);
    assert_eq!(
        report["snapshot"]["design"]["project_id"],
        project_id.as_str()
    );
}
