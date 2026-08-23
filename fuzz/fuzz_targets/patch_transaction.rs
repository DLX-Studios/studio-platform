#![no_main]
use std::collections::BTreeMap;
use libfuzzer_sys::fuzz_target;
use studio_protocol::{MountTree, NodeKind, PatchBatch, ProtocolLimits, UiNode};
use studio_ui::{InstanceId, UiRegistry};

fuzz_target!(|data: &[u8]| {
    let owner = InstanceId::new("fuzz-owner").unwrap();
    let mut registry = UiRegistry::new(owner.clone(), ProtocolLimits::default());
    registry.mount(MountTree { protocol_version: 1, route: "/fuzz".into(), root: UiNode {
        id: "root".into(), kind: NodeKind::Column, props: BTreeMap::new(), children: Vec::new(),
    }}).unwrap();
    if let Ok(batch) = serde_json::from_slice::<PatchBatch>(data) {
        let _ = registry.apply_patch(&owner, batch);
    }
});
