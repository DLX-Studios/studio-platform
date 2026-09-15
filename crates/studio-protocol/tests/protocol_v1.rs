#![allow(missing_docs)]

use std::collections::BTreeMap;

use serde_json::{Value, json};
use studio_protocol::{
    ErrorCode, GuestMessage, HostEvent, ProtocolLimits,
    actions::{ActionRequest, ActionResult},
    decode_guest_message, decode_host_event,
    lifecycle::{LifecycleEvent, LifecycleState},
    navigation::{NavigationCommand, NavigationEvent, validate_navigation_command},
    ui::{NodeKind, PatchBatch, PatchOp, UiEvent, UiNode},
    validate_guest_message, validate_host_event, validate_patch_sequence,
};

fn node(id: &str, kind: NodeKind, children: Vec<UiNode>) -> UiNode {
    UiNode {
        id: id.to_owned(),
        kind,
        props: BTreeMap::new(),
        children,
    }
}

fn decode_guest(value: &Value) -> Result<GuestMessage, studio_protocol::ProtocolError> {
    decode_guest_message(
        &serde_json::to_vec(&value).unwrap(),
        ProtocolLimits::default(),
    )
}

#[test]
fn accepts_the_golden_mount_and_every_closed_node_kind() {
    let kinds = [
        ("box", NodeKind::Box),
        ("column", NodeKind::Column),
        ("row", NodeKind::Row),
        ("stack", NodeKind::Stack),
        ("grid", NodeKind::Grid),
        ("scroll_view", NodeKind::ScrollView),
        ("list_view", NodeKind::ListView),
        ("spacer", NodeKind::Spacer),
        ("divider", NodeKind::Divider),
        ("text", NodeKind::Text),
        ("icon", NodeKind::Icon),
        ("image", NodeKind::Image),
        ("card", NodeKind::Card),
        ("badge", NodeKind::Badge),
        ("progress_indicator", NodeKind::ProgressIndicator),
        ("button", NodeKind::Button),
        ("icon_button", NodeKind::IconButton),
        ("checkbox", NodeKind::Checkbox),
        ("switch", NodeKind::Switch),
        ("slider", NodeKind::Slider),
        ("select", NodeKind::Select),
        ("text_input", NodeKind::TextInput),
        ("secret_input", NodeKind::SecretInput),
        ("dialog", NodeKind::Dialog),
        ("bottom_sheet", NodeKind::BottomSheet),
        ("toast", NodeKind::Toast),
        ("tooltip", NodeKind::Tooltip),
    ];

    for (wire_name, expected) in kinds {
        assert_eq!(
            serde_json::from_value::<NodeKind>(json!(wire_name)).unwrap(),
            expected
        );
    }

    let message = decode_guest(&json!({
        "type": "mount",
        "payload": {
            "protocol_version": 1,
            "route": "/catalog",
            "root": {
                "id": "root",
                "kind": "column",
                "props": {},
                "children": [{"id": "title", "kind": "text"}]
            }
        }
    }))
    .unwrap();
    assert!(matches!(message, GuestMessage::Mount(_)));
}

#[test]
fn rejects_open_envelopes_unknown_variants_and_unsupported_versions() {
    for invalid in [
        json!({"type":"log", "payload":{"level":"info", "message":"ok"}, "owner":"guest"}),
        json!({"type":"log", "payload":{"level":"info", "message":"ok", "extra":true}}),
        json!({"type":"unknown", "payload":{}}),
        json!({"type":"mount", "payload":{"protocol_version":2, "route":"/", "root":{"id":"root", "kind":"column"}}}),
        json!({"type":"mount", "payload":{"protocol_version":1, "route":"/", "root":{"id":"root", "kind":"div"}}}),
    ] {
        assert!(decode_guest(&invalid).is_err());
    }

    let error = decode_guest(&json!({
        "type":"mount",
        "payload":{"protocol_version":9, "route":"/", "root":{"id":"root", "kind":"column"}}
    }))
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::ProtocolUnsupported);
}

#[test]
fn enforces_node_id_count_and_depth_budgets() {
    for invalid_id in [String::new(), "é".repeat(65)] {
        let message = GuestMessage::Mount(studio_protocol::MountTree {
            protocol_version: 1,
            route: "/".to_owned(),
            root: node(&invalid_id, NodeKind::Text, vec![]),
        });
        let error = validate_guest_message(&message, ProtocolLimits::default()).unwrap_err();
        assert_eq!(error.code(), ErrorCode::TreeInvalid);
    }

    let duplicate = GuestMessage::Mount(studio_protocol::MountTree {
        protocol_version: 1,
        route: "/".to_owned(),
        root: node(
            "same",
            NodeKind::Column,
            vec![node("same", NodeKind::Text, vec![])],
        ),
    });
    assert_eq!(
        validate_guest_message(&duplicate, ProtocolLimits::default())
            .unwrap_err()
            .code(),
        ErrorCode::TreeInvalid
    );

    let two_nodes = GuestMessage::Mount(studio_protocol::MountTree {
        protocol_version: 1,
        route: "/".to_owned(),
        root: node(
            "root",
            NodeKind::Column,
            vec![node("child", NodeKind::Text, vec![])],
        ),
    });
    let count_limits = ProtocolLimits {
        max_nodes: 1,
        ..ProtocolLimits::default()
    };
    assert!(validate_guest_message(&two_nodes, count_limits).is_err());
    let depth_limits = ProtocolLimits {
        max_tree_depth: 1,
        ..ProtocolLimits::default()
    };
    assert!(validate_guest_message(&two_nodes, depth_limits).is_err());
}

#[test]
fn validates_patch_variants_budgets_and_strict_sequence() {
    let operations = vec![
        PatchOp::UpdateProp {
            node_id: "total".to_owned(),
            property: "text".to_owned(),
            value: json!("$80.00"),
        },
        PatchOp::InsertChild {
            parent_id: "root".to_owned(),
            index: 0,
            node: node("new", NodeKind::Text, vec![]),
        },
        PatchOp::RemoveNode {
            node_id: "old".to_owned(),
        },
        PatchOp::ReplaceNode {
            node_id: "replace".to_owned(),
            node: node("replacement", NodeKind::Card, vec![]),
        },
    ];
    let batch = PatchBatch {
        sequence: 8,
        operations,
    };
    validate_patch_sequence(&batch, Some(7)).unwrap();

    for (sequence, previous) in [(0, None), (7, Some(7)), (6, Some(7))] {
        let invalid = PatchBatch {
            sequence,
            operations: vec![],
        };
        assert_eq!(
            validate_patch_sequence(&invalid, previous)
                .unwrap_err()
                .code(),
            ErrorCode::SequenceInvalid
        );
    }

    let too_many = GuestMessage::Patch(PatchBatch {
        sequence: 1,
        operations: vec![
            PatchOp::RemoveNode {
                node_id: "a".to_owned()
            };
            2
        ],
    });
    let limits = ProtocolLimits {
        max_patch_operations: 1,
        ..ProtocolLimits::default()
    };
    assert_eq!(
        validate_guest_message(&too_many, limits)
            .unwrap_err()
            .code(),
        ErrorCode::PatchInvalid
    );

    assert!(decode_guest(&json!({
        "type":"patch",
        "payload":{"sequence":1,"operations":[{"op":"remove_node","node_id":"a","owner":"other"}]}
    })).is_err());
}

#[test]
fn navigation_action_and_lifecycle_variants_are_closed() {
    let navigation = [
        json!({"operation":"push", "route":"/checkout"}),
        json!({"operation":"replace", "route":"/cart"}),
        json!({"operation":"pop"}),
        json!({"operation":"pop_to", "route":"/catalog"}),
        json!({"operation":"reset", "route":"/"}),
    ];
    for value in navigation {
        let command: NavigationCommand = serde_json::from_value(value).unwrap();
        validate_navigation_command(&command).unwrap();
    }
    assert!(
        validate_navigation_command(&NavigationCommand::Push {
            route: "relative".into()
        })
        .is_err()
    );
    assert!(
        serde_json::from_value::<NavigationCommand>(json!({"operation":"pop", "route":"/guest"}))
            .is_err()
    );

    let action = ActionRequest {
        request_id: "req-1".into(),
        capability: "payment.simulate".into(),
        operation: "charge".into(),
        payload: json!({}),
    };
    assert!(matches!(
        GuestMessage::Action(action),
        GuestMessage::Action(_)
    ));
    assert!(decode_guest(&json!({
        "type":"action",
        "payload":{"request_id":"req-1","capability":"payment.simulate","operation":"charge","payload":{},"owner":"guest"}
    })).is_err());

    for state in [
        LifecycleState::Loading,
        LifecycleState::Running,
        LifecycleState::Trapped,
        LifecycleState::Stopped,
    ] {
        let event = HostEvent::Lifecycle(LifecycleEvent {
            state,
            message: None,
        });
        validate_host_event(&event, ProtocolLimits::default()).unwrap();
    }
    assert!(serde_json::from_value::<LifecycleState>(json!("terminated_by_guest")).is_err());
}

#[test]
fn decodes_all_host_events_without_guest_selectable_ownership() {
    let events = [
        HostEvent::Ui(UiEvent {
            node_id: "button".into(),
            event: "pressed".into(),
            payload: json!({}),
        }),
        HostEvent::Navigation(NavigationEvent {
            route: "/cart".into(),
            accepted: true,
            error_code: None,
        }),
        HostEvent::ActionResult(ActionResult::Success {
            request_id: "req-1".into(),
            payload: json!({"result_ref":"result-1"}),
        }),
        HostEvent::Lifecycle(LifecycleEvent {
            state: LifecycleState::Running,
            message: None,
        }),
    ];
    for event in events {
        let encoded = serde_json::to_vec(&event).unwrap();
        assert_eq!(
            decode_host_event(&encoded, ProtocolLimits::default()).unwrap(),
            event
        );
    }

    let owner_injection = serde_json::to_vec(&json!({
        "type":"ui",
        "payload":{"node_id":"button","event":"pressed","payload":{},"owner":"other-instance"}
    }))
    .unwrap();
    assert!(decode_host_event(&owner_injection, ProtocolLimits::default()).is_err());
}
