#![allow(missing_docs)]

use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use serde_json::json;
use studio_protocol::{MountTree, NodeKind, PatchBatch, PatchOp, ProtocolLimits, UiNode};
use studio_ui::{InstanceId, UiRegistry};

#[test]
fn ordinary_catalog_property_batches_remain_below_interaction_budget() {
    let owner = InstanceId::new("latency-test").unwrap();
    let mut registry = UiRegistry::new(owner.clone(), ProtocolLimits::default());
    registry
        .mount(MountTree {
            protocol_version: 1,
            route: "/catalog".to_owned(),
            root: UiNode {
                id: "root".to_owned(),
                kind: NodeKind::Column,
                props: BTreeMap::new(),
                children: (0..100)
                    .map(|index| UiNode {
                        id: format!("price-{index}"),
                        kind: NodeKind::Text,
                        props: BTreeMap::from([("text".to_owned(), json!("$0.00"))]),
                        children: Vec::new(),
                    })
                    .collect(),
            },
        })
        .unwrap();

    let mut samples = Vec::new();
    for sequence in 1..=200 {
        let operations = (0..100)
            .map(|index| PatchOp::UpdateProp {
                node_id: format!("price-{index}"),
                property: "text".to_owned(),
                value: json!(format!("${sequence}.00")),
            })
            .collect();
        let started = Instant::now();
        registry
            .apply_patch(
                &owner,
                PatchBatch {
                    sequence,
                    operations,
                },
            )
            .unwrap();
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    let p95 = samples[samples.len() * 95 / 100];
    assert!(
        p95 < Duration::from_millis(100),
        "100-property p95 was {p95:?}"
    );
    assert_eq!(registry.patch_metrics().committed_batches, 200);
    assert_eq!(registry.patch_metrics().property_operations, 20_000);
    assert_eq!(registry.patch_metrics().structural_operations, 0);
}
