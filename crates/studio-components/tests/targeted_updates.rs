#![allow(missing_docs)]

use std::collections::BTreeMap;

use serde_json::{Value, json};
use studio_components::{NativeStateStore, UpdateErrorCode};
use studio_protocol::{MountTree, NodeKind, PatchBatch, PatchOp, ProtocolLimits, UiNode};
use studio_ui::{InstanceId, UiRegistry};

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

fn mounted() -> (InstanceId, UiRegistry) {
    let owner = InstanceId::new("instance-a").unwrap();
    let mut registry = UiRegistry::new(owner.clone(), ProtocolLimits::default());
    registry
        .mount(MountTree {
            protocol_version: 1,
            route: "/catalog".to_owned(),
            root: node(
                "root",
                NodeKind::Column,
                &[],
                vec![
                    node(
                        "title",
                        NodeKind::Text,
                        &[("text", json!("Catalog"))],
                        vec![],
                    ),
                    node(
                        "list",
                        NodeKind::ListView,
                        &[],
                        vec![node(
                            "product",
                            NodeKind::Card,
                            &[],
                            vec![node(
                                "price",
                                NodeKind::Text,
                                &[("text", json!("$35.00"))],
                                vec![],
                            )],
                        )],
                    ),
                    node(
                        "search",
                        NodeKind::TextInput,
                        &[("label", json!("Search")), ("value", json!(""))],
                        vec![],
                    ),
                    node(
                        "discount",
                        NodeKind::Slider,
                        &[("min", json!(0)), ("max", json!(0.5)), ("value", json!(0))],
                        vec![],
                    ),
                    node(
                        "checkout",
                        NodeKind::Button,
                        &[("label", json!("Checkout")), ("enabled", json!(true))],
                        vec![],
                    ),
                ],
            ),
        })
        .unwrap();
    (owner, registry)
}

#[test]
fn property_updates_invalidate_only_targets_and_preserve_interaction_state() {
    let (owner, mut registry) = mounted();
    let mut native = NativeStateStore::from_registry(&owner, &registry).unwrap();
    native.focus(&owner, "search").unwrap();
    native.set_scroll_offset(&owner, "list", 84.0).unwrap();
    native
        .set_input_buffer(&owner, "search", "classic")
        .unwrap();
    let root_identity = native.native_identity(&owner, "root").unwrap();
    let list_identity = native.native_identity(&owner, "list").unwrap();

    let commit = registry
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
                    PatchOp::UpdateProp {
                        node_id: "discount".to_owned(),
                        property: "value".to_owned(),
                        value: json!(0.25),
                    },
                    PatchOp::UpdateProp {
                        node_id: "checkout".to_owned(),
                        property: "enabled".to_owned(),
                        value: json!(false),
                    },
                ],
            },
        )
        .unwrap();
    let report = native.apply_commit(&owner, &registry, &commit).unwrap();

    assert_eq!(report.invalidated_nodes, ["title", "discount", "checkout"]);
    assert_eq!(
        native.property(&owner, "title", "text").unwrap(),
        &json!("Products")
    );
    assert_eq!(
        native.property(&owner, "discount", "value").unwrap(),
        &json!(0.25)
    );
    assert_eq!(
        native.property(&owner, "checkout", "enabled").unwrap(),
        &json!(false)
    );
    assert_eq!(native.focused_id(&owner).unwrap(), Some("search"));
    assert!((native.scroll_offset(&owner, "list").unwrap() - 84.0).abs() < f32::EPSILON);
    assert_eq!(native.input_buffer(&owner, "search").unwrap(), "classic");
    assert_eq!(
        native.native_identity(&owner, "root").unwrap(),
        root_identity
    );
    assert_eq!(
        native.native_identity(&owner, "list").unwrap(),
        list_identity
    );
}

#[test]
fn structural_reconciliation_preserves_unaffected_ancestors_and_siblings() {
    let (owner, mut registry) = mounted();
    let mut native = NativeStateStore::from_registry(&owner, &registry).unwrap();
    native.focus(&owner, "search").unwrap();
    native.set_scroll_offset(&owner, "list", 42.0).unwrap();
    let root_identity = native.native_identity(&owner, "root").unwrap();
    let search_identity = native.native_identity(&owner, "search").unwrap();
    let old_product_identity = native.native_identity(&owner, "product").unwrap();

    let commit = registry
        .apply_patch(
            &owner,
            PatchBatch {
                sequence: 1,
                operations: vec![PatchOp::ReplaceNode {
                    node_id: "product".to_owned(),
                    node: node(
                        "product",
                        NodeKind::Card,
                        &[],
                        vec![node(
                            "price",
                            NodeKind::Text,
                            &[("text", json!("$31.50"))],
                            vec![],
                        )],
                    ),
                }],
            },
        )
        .unwrap();
    native.apply_commit(&owner, &registry, &commit).unwrap();

    assert_eq!(
        native.native_identity(&owner, "root").unwrap(),
        root_identity
    );
    assert_eq!(
        native.native_identity(&owner, "search").unwrap(),
        search_identity
    );
    assert_ne!(
        native.native_identity(&owner, "product").unwrap(),
        old_product_identity
    );
    assert_eq!(native.focused_id(&owner).unwrap(), Some("search"));
    assert!((native.scroll_offset(&owner, "list").unwrap() - 42.0).abs() < f32::EPSILON);
}

#[test]
fn rejects_cross_owner_native_updates_without_mutation() {
    let (owner, mut registry) = mounted();
    let other = InstanceId::new("instance-b").unwrap();
    let mut native = NativeStateStore::from_registry(&owner, &registry).unwrap();
    let before = native.snapshot(&owner).unwrap();
    let commit = registry
        .apply_patch(
            &owner,
            PatchBatch {
                sequence: 1,
                operations: vec![PatchOp::UpdateProp {
                    node_id: "title".to_owned(),
                    property: "text".to_owned(),
                    value: json!("Products"),
                }],
            },
        )
        .unwrap();
    assert_eq!(
        native
            .apply_commit(&other, &registry, &commit)
            .unwrap_err()
            .code(),
        UpdateErrorCode::OwnerMismatch
    );
    assert_eq!(native.snapshot(&owner).unwrap(), before);
}
