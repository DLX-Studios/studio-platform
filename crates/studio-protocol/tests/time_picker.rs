#![allow(missing_docs)]
use serde_json::json;
use std::collections::BTreeMap;
use studio_protocol::{
    ChildCardinality, NodeKind, ProtocolLimits, UiNode, child_cardinality, is_known_property,
    validate_node_contract,
};

fn picker(value: &str) -> UiNode {
    UiNode {
        id: "time".into(),
        kind: NodeKind::TimePicker,
        props: BTreeMap::from([("value".into(), json!(value))]),
        children: vec![],
    }
}
#[test]
fn time_values_are_canonical_and_bounded() {
    for value in ["00:00", "09:30", "23:59", "23:59:59", "00:00:00"] {
        assert!(
            validate_node_contract(&picker(value), ProtocolLimits::default()).is_ok(),
            "{value}"
        );
    }
    for value in [
        "",
        "9:30",
        "24:00",
        "12:60",
        "23:59:60",
        "09:30Z",
        "12:30 PM",
        "１２:３０",
        "09:30:00.0",
        "09:30:aa",
    ] {
        assert!(
            validate_node_contract(&picker(value), ProtocolLimits::default()).is_err(),
            "{value}"
        );
    }
    assert!(is_known_property(
        NodeKind::TimePicker,
        "value",
        ProtocolLimits::default()
    ));
    assert!(is_known_property(
        NodeKind::TimePicker,
        "on_changed",
        ProtocolLimits::default()
    ));
    assert!(!is_known_property(
        NodeKind::TimePicker,
        "items",
        ProtocolLimits::default()
    ));
    assert_eq!(
        child_cardinality(NodeKind::TimePicker),
        ChildCardinality::None
    );
    let mut with_child = picker("09:30");
    with_child.children.push(picker("10:00"));
    assert!(validate_node_contract(&with_child, ProtocolLimits::default()).is_err());
}
