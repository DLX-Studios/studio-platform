#![allow(missing_docs)]

use serde_json::json;
use studio_protocol::{MountTree, NodeKind, ProtocolLimits, UiNode};
use studio_ui::{InstanceId, MountErrorCode, UiRegistry};

fn node(
    id: &str,
    kind: NodeKind,
    props: &[(&str, serde_json::Value)],
    children: Vec<UiNode>,
) -> UiNode {
    UiNode {
        id: id.to_owned(),
        kind,
        props: props
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
        children,
    }
}

fn valid_mount() -> MountTree {
    MountTree {
        protocol_version: 1,
        route: "/catalog".to_owned(),
        root: node(
            "root",
            NodeKind::Column,
            &[("gap", json!(12))],
            vec![
                node(
                    "title",
                    NodeKind::Text,
                    &[("text", json!("Catalog"))],
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
    }
}

#[test]
fn mounts_a_complete_instance_owned_tree_atomically() {
    let owner = InstanceId::new("instance-a").unwrap();
    let mut registry = UiRegistry::new(owner.clone(), ProtocolLimits::default());
    registry.mount(valid_mount()).unwrap();

    assert_eq!(registry.len(), 3);
    assert_eq!(registry.root_id(), Some("root"));
    assert_eq!(registry.get(&owner, "title").unwrap().kind, NodeKind::Text);
    assert_eq!(registry.parent_id(&owner, "title").unwrap(), Some("root"));
}

#[test]
fn duplicate_ids_count_depth_and_invalid_properties_leave_registry_empty() {
    let owner = InstanceId::new("instance-a").unwrap();
    let cases = [
        (
            MountTree {
                protocol_version: 1,
                route: "/".into(),
                root: node(
                    "same",
                    NodeKind::Column,
                    &[],
                    vec![node("same", NodeKind::Text, &[], vec![])],
                ),
            },
            ProtocolLimits::default(),
            MountErrorCode::TreeInvalid,
        ),
        (
            valid_mount(),
            ProtocolLimits {
                max_nodes: 2,
                ..ProtocolLimits::default()
            },
            MountErrorCode::TreeInvalid,
        ),
        (
            valid_mount(),
            ProtocolLimits {
                max_tree_depth: 1,
                ..ProtocolLimits::default()
            },
            MountErrorCode::TreeInvalid,
        ),
        (
            MountTree {
                protocol_version: 1,
                route: "/".into(),
                root: node(
                    "root",
                    NodeKind::Text,
                    &[("html", json!("<b>no</b>"))],
                    vec![],
                ),
            },
            ProtocolLimits::default(),
            MountErrorCode::PropertyInvalid,
        ),
    ];

    for (mount, limits, expected) in cases {
        let mut registry = UiRegistry::new(owner.clone(), limits);
        assert_eq!(registry.mount(mount).unwrap_err().code(), expected);
        assert!(registry.is_empty());
        assert_eq!(registry.root_id(), None);
    }
}

#[test]
fn rejects_second_mount_without_mutating_the_first_tree() {
    let owner = InstanceId::new("instance-a").unwrap();
    let mut registry = UiRegistry::new(owner.clone(), ProtocolLimits::default());
    registry.mount(valid_mount()).unwrap();
    let before = registry.snapshot(&owner).unwrap();

    assert_eq!(
        registry.mount(valid_mount()).unwrap_err().code(),
        MountErrorCode::AlreadyMounted
    );
    assert_eq!(registry.snapshot(&owner).unwrap(), before);
}

#[test]
fn identical_local_ids_are_isolated_by_instance_owner() {
    let owner_a = InstanceId::new("instance-a").unwrap();
    let owner_b = InstanceId::new("instance-b").unwrap();
    let mut registry_a = UiRegistry::new(owner_a.clone(), ProtocolLimits::default());
    let mut registry_b = UiRegistry::new(owner_b.clone(), ProtocolLimits::default());
    registry_a.mount(valid_mount()).unwrap();
    registry_b.mount(valid_mount()).unwrap();

    assert!(registry_a.get(&owner_a, "title").is_ok());
    assert!(registry_b.get(&owner_b, "title").is_ok());
    assert_eq!(
        registry_a.get(&owner_b, "title").unwrap_err().code(),
        MountErrorCode::OwnerMismatch
    );
    assert_eq!(
        registry_b.get(&owner_a, "title").unwrap_err().code(),
        MountErrorCode::OwnerMismatch
    );
}

#[test]
fn rejects_empty_or_oversized_instance_ids() {
    assert_eq!(
        InstanceId::new("").unwrap_err().code(),
        MountErrorCode::OwnerInvalid
    );
    assert_eq!(
        InstanceId::new("x".repeat(129)).unwrap_err().code(),
        MountErrorCode::OwnerInvalid
    );
}
