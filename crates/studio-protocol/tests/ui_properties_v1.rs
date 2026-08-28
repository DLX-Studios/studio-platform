#![allow(missing_docs)]

use std::collections::BTreeMap;

use serde_json::{Value, json};
use studio_protocol::{
    ErrorCode, GuestMessage, MountTree, NodeKind, ProtocolLimits, UiNode, validate_guest_message,
};

fn node(id: &str, kind: NodeKind, props: Value, children: Vec<UiNode>) -> UiNode {
    let Value::Object(props) = props else {
        panic!("test properties must be an object")
    };
    UiNode {
        id: id.to_owned(),
        kind,
        props: props.into_iter().collect(),
        children,
    }
}

fn leaf(id: &str) -> UiNode {
    node(id, NodeKind::Text, json!({"text": id}), vec![])
}

fn validate(root: UiNode) -> Result<(), studio_protocol::ProtocolError> {
    validate_guest_message(
        &GuestMessage::Mount(MountTree {
            protocol_version: 1,
            route: "/catalog".to_owned(),
            root,
        }),
        ProtocolLimits::default(),
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive fixture makes coverage of all 27 closed kinds auditable"
)]
fn valid_catalog() -> [(NodeKind, Value, Vec<UiNode>); 27] {
    [
        (
            NodeKind::Box,
            json!({"padding": 8, "background": "surface", "width": 390, "height": 720, "shrink": true}),
            vec![leaf("box-child")],
        ),
        (
            NodeKind::Column,
            json!({"gap": 12, "alignment": "start", "flex": 1}),
            vec![leaf("column-child")],
        ),
        (
            NodeKind::Row,
            json!({"gap": 8, "alignment": "center"}),
            vec![leaf("row-child")],
        ),
        (
            NodeKind::Stack,
            json!({"alignment": "end"}),
            vec![leaf("stack-child")],
        ),
        (
            NodeKind::Grid,
            json!({"columns": 3, "gap": 8}),
            vec![leaf("grid-child")],
        ),
        (
            NodeKind::ScrollView,
            json!({"axis": "vertical"}),
            vec![leaf("scroll-child")],
        ),
        (
            NodeKind::ListView,
            json!({"axis": "vertical", "gap": 4}),
            vec![leaf("list-child")],
        ),
        (NodeKind::Spacer, json!({"size": 16}), vec![]),
        (NodeKind::Divider, json!({"thickness": 1}), vec![]),
        (
            NodeKind::Text,
            json!({"text": "Catalog", "typography_role": "title"}),
            vec![],
        ),
        (NodeKind::Icon, json!({"name": "cart"}), vec![]),
        (
            NodeKind::Image,
            json!({"asset": "assets/item.png", "alt": "Item", "width": 72, "height": 72}),
            vec![],
        ),
        (
            NodeKind::Card,
            json!({"padding": 12}),
            vec![leaf("card-child")],
        ),
        (NodeKind::Badge, json!({"label": "New"}), vec![]),
        (NodeKind::ProgressIndicator, json!({"value": 0.5}), vec![]),
        (
            NodeKind::Button,
            json!({"label": "Checkout", "enabled": true, "on_pressed": "checkout", "variant": "primary", "width": "full"}),
            vec![],
        ),
        (
            NodeKind::IconButton,
            json!({"icon": "remove", "accessibility_label": "Remove", "enabled": true, "on_pressed": "remove"}),
            vec![],
        ),
        (
            NodeKind::Checkbox,
            json!({"label": "Tax exempt", "value": false, "enabled": true, "on_changed": "tax"}),
            vec![],
        ),
        (
            NodeKind::Switch,
            json!({"label": "Receipt", "value": true, "enabled": true, "on_changed": "receipt"}),
            vec![],
        ),
        (
            NodeKind::Slider,
            json!({"label": "Discount", "min": 0, "max": 0.5, "value": 0.15, "enabled": true, "on_changed": "discount"}),
            vec![],
        ),
        (
            NodeKind::Select,
            json!({"label": "Category", "options": ["all", "cuts"], "value": "all", "enabled": true, "on_changed": "category"}),
            vec![],
        ),
        (
            NodeKind::TextInput,
            json!({"label": "Search", "value": "", "placeholder": "Products", "enabled": true, "on_changed": "search"}),
            vec![],
        ),
        (
            NodeKind::SecretInput,
            json!({"label": "Authorization", "ready": false, "enabled": true, "on_ready": "authorization"}),
            vec![],
        ),
        (
            NodeKind::Dialog,
            json!({"title": "Confirm", "open": true}),
            vec![leaf("dialog-child")],
        ),
        (
            NodeKind::BottomSheet,
            json!({"open": true}),
            vec![leaf("sheet-child")],
        ),
        (NodeKind::Toast, json!({"message": "Added"}), vec![]),
        (
            NodeKind::Tooltip,
            json!({"message": "More information"}),
            vec![leaf("tooltip-child")],
        ),
    ]
}

#[test]
fn accepts_the_closed_property_schema_for_every_protocol_kind() {
    for (index, (kind, mut props, children)) in valid_catalog().into_iter().enumerate() {
        props
            .as_object_mut()
            .unwrap()
            .insert("visible".to_owned(), json!(true));
        props.as_object_mut().unwrap().insert(
            "transition".to_owned(),
            json!({"duration_ms": 150, "curve": "ease_out"}),
        );
        validate(node(&format!("node-{index}"), kind, props, children)).unwrap();
    }
}

#[test]
fn rejects_unknown_properties_and_wrong_property_types_for_every_kind() {
    for (index, (kind, _, _)) in valid_catalog().into_iter().enumerate() {
        let error = validate(node(
            &format!("unknown-{index}"),
            kind,
            json!({"html": "<b>not native</b>"}),
            vec![],
        ))
        .unwrap_err();
        assert_eq!(error.code(), ErrorCode::TreeInvalid);
    }

    for (kind, props) in [
        (NodeKind::Column, json!({"gap": "large"})),
        (NodeKind::Grid, json!({"columns": 0})),
        (NodeKind::Text, json!({"text": 12})),
        (NodeKind::ProgressIndicator, json!({"value": 1.1})),
        (NodeKind::Button, json!({"enabled": "yes"})),
        (NodeKind::Slider, json!({"min": 1, "max": 0, "value": 2})),
        (NodeKind::Select, json!({"options": ["a"], "value": "b"})),
        (NodeKind::SecretInput, json!({"value": "4111111111111111"})),
        (NodeKind::Toast, json!({"message": false})),
    ] {
        assert_eq!(
            validate(node("invalid", kind, props, vec![]))
                .unwrap_err()
                .code(),
            ErrorCode::TreeInvalid
        );
    }
}

#[test]
fn enforces_leaf_single_child_and_required_child_cardinality() {
    let leaf_kinds = [
        NodeKind::Spacer,
        NodeKind::Divider,
        NodeKind::Text,
        NodeKind::Icon,
        NodeKind::Image,
        NodeKind::Badge,
        NodeKind::ProgressIndicator,
        NodeKind::Button,
        NodeKind::IconButton,
        NodeKind::Checkbox,
        NodeKind::Switch,
        NodeKind::Slider,
        NodeKind::Select,
        NodeKind::TextInput,
        NodeKind::SecretInput,
        NodeKind::Toast,
    ];
    for (index, kind) in leaf_kinds.into_iter().enumerate() {
        assert!(
            validate(node(
                &format!("leaf-{index}"),
                kind,
                json!({}),
                vec![leaf("illegal-child")],
            ))
            .is_err()
        );
    }

    for kind in [
        NodeKind::ScrollView,
        NodeKind::Card,
        NodeKind::Dialog,
        NodeKind::BottomSheet,
        NodeKind::Tooltip,
    ] {
        assert!(
            validate(node(
                "single",
                kind,
                json!({}),
                vec![leaf("first"), leaf("second")],
            ))
            .is_err()
        );
    }
    assert!(validate(node("tooltip", NodeKind::Tooltip, json!({}), vec![])).is_err());
}

#[test]
fn rejects_malformed_common_semantics_and_event_bindings() {
    for props in [
        json!({"visible": "yes"}),
        json!({"opacity": -0.1}),
        json!({"opacity": 1.1}),
        json!({"transition": {"duration_ms": 150, "curve": "bounce"}}),
        json!({"transition": {"duration_ms": 60_001, "curve": "linear"}}),
        json!({"accessibility_label": ""}),
        json!({"on_pressed": "contains spaces"}),
    ] {
        assert!(validate(node("invalid", NodeKind::Button, props, vec![])).is_err());
    }

    let mut oversized = BTreeMap::new();
    oversized.insert("text".to_owned(), json!("x".repeat(65 * 1024)));
    assert!(
        validate(UiNode {
            id: "text".to_owned(),
            kind: NodeKind::Text,
            props: oversized,
            children: vec![],
        })
        .is_err()
    );
}
