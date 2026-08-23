#![allow(missing_docs)]

use serde_json::json;
use studio_app::SafeDiagnostic;
use studio_components::{DispatchErrorCode, HostEventDispatcher, InputAction};
use studio_protocol::{
    ErrorCode, MountTree, NodeKind, PatchBatch, PatchOp, ProtocolLimits, UiNode,
    decode_guest_message,
};
use studio_security::{CapabilityId, SecurityErrorCode, SensitiveValueFilter};
use studio_ui::{InstanceId, PatchErrorCode, UiRegistry};

fn node(id: &str, kind: NodeKind, props: &[(&str, serde_json::Value)]) -> UiNode {
    UiNode {
        id: id.to_owned(),
        kind,
        props: props
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
        children: Vec::new(),
    }
}

#[test]
fn unknown_protocol_surface_operations_have_stable_redacted_diagnostics() {
    let limits = ProtocolLimits::default();
    for raw in [
        br#"{"type":"mount","payload":{"protocol_version":1,"route":"/","root":{"id":"x","kind":"html","props":{},"children":[]}}}"#.as_slice(),
        br#"{"type":"navigate","payload":{"operation":"push","route":"not-absolute"}}"#.as_slice(),
        br#"{"type":"lifecycle","payload":{"state":"invented"}}"#.as_slice(),
    ] {
        let error = decode_guest_message(raw, limits).unwrap_err();
        let diagnostic = SafeDiagnostic::from_protocol(&SensitiveValueFilter::new(), &error);
        assert!(!diagnostic.code().is_empty());
        assert!(!diagnostic.message().contains("opaque_"));
    }
    assert_eq!(
        CapabilityId::parse("network.raw").unwrap_err().code(),
        SecurityErrorCode::CapabilityDenied
    );

    let owner = InstanceId::new("owner").unwrap();
    let dispatcher = HostEventDispatcher::new(owner.clone());
    assert_eq!(
        dispatcher
            .dispatch(&owner, "missing", InputAction::KeyboardActivate)
            .unwrap_err()
            .code(),
        DispatchErrorCode::NodeNotFound
    );
}

#[test]
fn malformed_batches_and_unknown_properties_never_partially_mutate_ui() {
    let owner = InstanceId::new("owner").unwrap();
    let mut registry = UiRegistry::new(owner.clone(), ProtocolLimits::default());
    registry
        .mount(MountTree {
            protocol_version: 1,
            route: "/catalog".to_owned(),
            root: node("title", NodeKind::Text, &[("text", json!("Before"))]),
        })
        .unwrap();
    let before = registry.snapshot(&owner).unwrap();
    let error = registry
        .apply_patch(
            &owner,
            PatchBatch {
                sequence: 1,
                operations: vec![
                    PatchOp::UpdateProp {
                        node_id: "title".to_owned(),
                        property: "text".to_owned(),
                        value: json!("After"),
                    },
                    PatchOp::UpdateProp {
                        node_id: "title".to_owned(),
                        property: "html".to_owned(),
                        value: json!("<secret>"),
                    },
                ],
            },
        )
        .unwrap_err();
    assert_eq!(error.code(), PatchErrorCode::PropertyInvalid);
    assert_eq!(registry.snapshot(&owner).unwrap(), before);
    assert_eq!(error.diagnostic().code(), "property_invalid");

    let malformed = decode_guest_message(
        br#"{"type":"patch","payload":{"sequence":2,"operations":[]}}"#,
        ProtocolLimits::default(),
    )
    .unwrap_err();
    assert_eq!(malformed.code(), ErrorCode::PatchInvalid);
}
