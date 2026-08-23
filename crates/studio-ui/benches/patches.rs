#![allow(missing_docs)]

use std::{collections::BTreeMap, hint::black_box, time::Instant};

use serde_json::json;
use studio_protocol::{MountTree, NodeKind, PatchBatch, PatchOp, ProtocolLimits, UiNode};
use studio_ui::{InstanceId, UiRegistry};

fn text(id: String) -> UiNode {
    UiNode {
        id,
        kind: NodeKind::Text,
        props: BTreeMap::from([("text".to_owned(), json!("0"))]),
        children: Vec::new(),
    }
}

fn registry(count: usize) -> (InstanceId, UiRegistry) {
    let owner = InstanceId::new("patch-benchmark").unwrap();
    let mut registry = UiRegistry::new(owner.clone(), ProtocolLimits::default());
    registry
        .mount(MountTree {
            protocol_version: 1,
            route: "/benchmark".to_owned(),
            root: UiNode {
                id: "root".to_owned(),
                kind: NodeKind::Column,
                props: BTreeMap::new(),
                children: (0..count)
                    .map(|index| text(format!("value-{index}")))
                    .collect(),
            },
        })
        .unwrap();
    (owner, registry)
}

fn main() {
    const ITERATIONS: u64 = 2_000;
    let (owner, mut single) = registry(1);
    let started = Instant::now();
    for sequence in 1..=ITERATIONS {
        black_box(
            single
                .apply_patch(
                    &owner,
                    PatchBatch {
                        sequence,
                        operations: vec![PatchOp::UpdateProp {
                            node_id: "value-0".to_owned(),
                            property: "text".to_owned(),
                            value: json!(sequence.to_string()),
                        }],
                    },
                )
                .unwrap(),
        );
    }
    let single_average = started.elapsed().as_nanos() / u128::from(ITERATIONS);

    let (owner, mut batched) = registry(100);
    let started = Instant::now();
    for sequence in 1..=ITERATIONS {
        let operations = (0..100)
            .map(|index| PatchOp::UpdateProp {
                node_id: format!("value-{index}"),
                property: "text".to_owned(),
                value: json!(sequence.to_string()),
            })
            .collect();
        black_box(
            batched
                .apply_patch(
                    &owner,
                    PatchBatch {
                        sequence,
                        operations,
                    },
                )
                .unwrap(),
        );
    }
    let batch_average = started.elapsed().as_nanos() / u128::from(ITERATIONS);
    println!("single_property_average_ns={single_average}");
    println!("hundred_property_batch_average_ns={batch_average}");
}
