#![allow(missing_docs)]

use serde_json::json;
use studio_components::NativeStateStore;
use studio_protocol::{MountTree, NodeKind, ProtocolLimits, UiNode, decode_guest_message};
use studio_ui::{InstanceId, UiRegistry};

#[test]
fn host_state_values_are_scoped_to_node_and_survive_unrelated_reads() {
    let owner = InstanceId::new("stateful-test").unwrap();
    let root = UiNode {
        id: "root".into(),
        kind: NodeKind::Column,
        props: std::collections::BTreeMap::default(),
        children: vec![UiNode {
            id: "picker".into(),
            kind: NodeKind::DatePicker,
            props: std::collections::BTreeMap::default(),
            children: vec![],
        }],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/state".into(),
        root,
    };
    let encoded = serde_json::to_vec(&studio_protocol::GuestMessage::Mount(mount)).unwrap();
    let message = decode_guest_message(&encoded, ProtocolLimits::default()).unwrap();
    let studio_protocol::GuestMessage::Mount(mount) = message else {
        unreachable!()
    };
    let mut registry = UiRegistry::new(owner.clone(), ProtocolLimits::default());
    registry.mount(mount).unwrap();
    let mut state = NativeStateStore::from_registry(&owner, &registry).unwrap();
    state
        .set_component_value(&owner, "picker", "selected", json!("2026-08-06"))
        .unwrap();
    assert_eq!(
        state.component_value(&owner, "picker", "selected").unwrap(),
        Some(&json!("2026-08-06"))
    );
}
