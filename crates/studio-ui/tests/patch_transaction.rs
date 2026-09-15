#![allow(missing_docs)]

use std::collections::BTreeMap;

use serde_json::{Value, json};
use studio_protocol::{MountTree, NodeKind, PatchBatch, PatchOp, ProtocolLimits, UiNode};
use studio_ui::{InstanceId, PatchErrorCode, UiRegistry};

fn node(id: &str, kind: NodeKind, props: &[(&str, Value)], children: Vec<UiNode>) -> UiNode {
    UiNode {
        id: id.to_owned(),
        kind,
        props: props
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect::<BTreeMap<_, _>>(),
        children,
    }
}

fn mounted(limits: ProtocolLimits) -> (InstanceId, UiRegistry) {
    let owner = InstanceId::new("instance-a").unwrap();
    let mut registry = UiRegistry::new(owner.clone(), limits);
    registry
        .mount(MountTree {
            protocol_version: 1,
            route: "/catalog".to_owned(),
            root: node(
                "root",
                NodeKind::Column,
                &[("gap", json!(8))],
                vec![
                    node(
                        "title",
                        NodeKind::Text,
                        &[("text", json!("Catalog"))],
                        vec![],
                    ),
                    node("old", NodeKind::Card, &[], vec![]),
                    node(
                        "checkout",
                        NodeKind::Button,
                        &[("label", json!("Checkout"))],
                        vec![],
                    ),
                ],
            ),
        })
        .unwrap();
    (owner, registry)
}

#[test]
fn commits_update_insert_remove_and_replace_as_one_ordered_batch() {
    let (owner, mut registry) = mounted(ProtocolLimits::default());
    registry
        .apply_patch(
            &owner,
            PatchBatch {
                sequence: 1,
                operations: vec![
                    PatchOp::UpdateProp {
                        node_id: "title".to_owned(),
                        property: "text".to_owned(),
                        value: json!("Products"),
                    },
                    PatchOp::InsertChild {
                        parent_id: "root".to_owned(),
                        index: 1,
                        node: node(
                            "inserted",
                            NodeKind::Text,
                            &[("text", json!("Inserted"))],
                            vec![],
                        ),
                    },
                    PatchOp::RemoveNode {
                        node_id: "old".to_owned(),
                    },
                    PatchOp::ReplaceNode {
                        node_id: "checkout".to_owned(),
                        node: node("pay", NodeKind::Button, &[("label", json!("Pay"))], vec![]),
                    },
                ],
            },
        )
        .unwrap();

    assert_eq!(registry.last_sequence(), Some(1));
    assert_eq!(registry.len(), 4);
    assert_eq!(
        registry.get(&owner, "title").unwrap().props["text"],
        "Products"
    );
    assert!(registry.get(&owner, "old").is_err());
    assert!(registry.get(&owner, "checkout").is_err());
    assert_eq!(registry.get(&owner, "pay").unwrap().kind, NodeKind::Button);
    assert_eq!(
        registry.get(&owner, "root").unwrap().children,
        ["title", "inserted", "pay"]
    );
}

#[test]
fn rejects_invalid_targets_indices_root_operations_and_replayed_sequences() {
    let cases = [
        (
            PatchOp::UpdateProp {
                node_id: "missing".to_owned(),
                property: "text".to_owned(),
                value: json!("x"),
            },
            PatchErrorCode::TargetInvalid,
        ),
        (
            PatchOp::InsertChild {
                parent_id: "root".to_owned(),
                index: 99,
                node: node("new", NodeKind::Text, &[("text", json!("x"))], vec![]),
            },
            PatchErrorCode::IndexInvalid,
        ),
        (
            PatchOp::RemoveNode {
                node_id: "root".to_owned(),
            },
            PatchErrorCode::RootInvalid,
        ),
    ];
    for (operation, expected) in cases {
        let (owner, mut registry) = mounted(ProtocolLimits::default());
        assert_eq!(
            registry
                .apply_patch(
                    &owner,
                    PatchBatch {
                        sequence: 1,
                        operations: vec![operation],
                    },
                )
                .unwrap_err()
                .code(),
            expected
        );
    }

    let (owner, mut registry) = mounted(ProtocolLimits::default());
    registry
        .apply_patch(
            &owner,
            PatchBatch {
                sequence: 2,
                operations: vec![PatchOp::UpdateProp {
                    node_id: "title".to_owned(),
                    property: "text".to_owned(),
                    value: json!("First"),
                }],
            },
        )
        .unwrap();
    for sequence in [2, 1] {
        assert_eq!(
            registry
                .apply_patch(
                    &owner,
                    PatchBatch {
                        sequence,
                        operations: vec![PatchOp::UpdateProp {
                            node_id: "title".to_owned(),
                            property: "text".to_owned(),
                            value: json!("Replay"),
                        }],
                    },
                )
                .unwrap_err()
                .code(),
            PatchErrorCode::SequenceInvalid
        );
    }
}

#[test]
fn validates_combined_tree_budgets_and_owner_before_commit() {
    let limits = ProtocolLimits {
        max_nodes: 4,
        ..ProtocolLimits::default()
    };
    let (owner, mut registry) = mounted(limits);
    let before = registry.snapshot(&owner).unwrap();
    assert_eq!(
        registry
            .apply_patch(
                &owner,
                PatchBatch {
                    sequence: 1,
                    operations: vec![PatchOp::InsertChild {
                        parent_id: "root".to_owned(),
                        index: 0,
                        node: node("over-budget", NodeKind::Text, &[], vec![]),
                    }],
                },
            )
            .unwrap_err()
            .code(),
        PatchErrorCode::TreeInvalid
    );
    assert_eq!(registry.snapshot(&owner).unwrap(), before);

    let other = InstanceId::new("instance-b").unwrap();
    assert_eq!(
        registry
            .apply_patch(
                &other,
                PatchBatch {
                    sequence: 1,
                    operations: vec![PatchOp::RemoveNode {
                        node_id: "old".to_owned(),
                    }],
                },
            )
            .unwrap_err()
            .code(),
        PatchErrorCode::OwnerMismatch
    );
    assert_eq!(registry.snapshot(&owner).unwrap(), before);
}

#[test]
fn rolls_back_earlier_valid_operations_when_a_later_operation_fails() {
    let (owner, mut registry) = mounted(ProtocolLimits::default());
    let before = registry.snapshot(&owner).unwrap();
    assert_eq!(
        registry
            .apply_patch(
                &owner,
                PatchBatch {
                    sequence: 1,
                    operations: vec![
                        PatchOp::UpdateProp {
                            node_id: "title".to_owned(),
                            property: "text".to_owned(),
                            value: json!("Must roll back"),
                        },
                        PatchOp::RemoveNode {
                            node_id: "missing".to_owned(),
                        },
                    ],
                },
            )
            .unwrap_err()
            .code(),
        PatchErrorCode::TargetInvalid
    );
    assert_eq!(registry.snapshot(&owner).unwrap(), before);
    assert_eq!(registry.last_sequence(), None);
}
