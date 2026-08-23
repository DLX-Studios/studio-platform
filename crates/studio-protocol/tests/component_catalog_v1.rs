#![allow(missing_docs)]
#![allow(clippy::default_trait_access)]
#![allow(clippy::too_many_lines)]

use serde_json::json;
use studio_protocol::{
    ErrorCode, GuestMessage, MountTree, NodeKind, ProtocolError, ProtocolLimits, UiNode,
    validate_guest_message,
};

fn all_supported_kinds() -> Vec<NodeKind> {
    vec![
        NodeKind::Box,
        NodeKind::Column,
        NodeKind::Row,
        NodeKind::Stack,
        NodeKind::Grid,
        NodeKind::ScrollView,
        NodeKind::ListView,
        NodeKind::Spacer,
        NodeKind::Divider,
        NodeKind::Text,
        NodeKind::Icon,
        NodeKind::Image,
        NodeKind::Card,
        NodeKind::Badge,
        NodeKind::Tag,
        NodeKind::Avatar,
        NodeKind::Empty,
        NodeKind::Skeleton,
        NodeKind::ProgressIndicator,
        NodeKind::ProgressCircle,
        NodeKind::Spinner,
        NodeKind::Button,
        NodeKind::IconButton,
        NodeKind::Checkbox,
        NodeKind::Radio,
        NodeKind::Switch,
        NodeKind::Toggle,
        NodeKind::ButtonGroup,
        NodeKind::Slider,
        NodeKind::RangeSlider,
        NodeKind::Select,
        NodeKind::Combobox,
        NodeKind::NumberInput,
        NodeKind::TextInput,
        NodeKind::TextArea,
        NodeKind::Field,
        NodeKind::InputGroup,
        NodeKind::OtpInput,
        NodeKind::SecretInput,
        NodeKind::Dialog,
        NodeKind::AlertDialog,
        NodeKind::Popover,
        NodeKind::Sheet,
        NodeKind::BottomSheet,
        NodeKind::Toast,
        NodeKind::Notification,
        NodeKind::Banner,
        NodeKind::ContextMenu,
        NodeKind::CommandPalette,
        NodeKind::Tooltip,
        NodeKind::Scaffold,
        NodeKind::AppBar,
        NodeKind::Sidebar,
        NodeKind::NavigationBar,
        NodeKind::NavigationRail,
        NodeKind::Drawer,
        NodeKind::Tabs,
        NodeKind::Breadcrumb,
        NodeKind::Stepper,
        NodeKind::Pagination,
        NodeKind::ListTile,
        NodeKind::SearchableList,
        NodeKind::VirtualList,
        NodeKind::DataTable,
        NodeKind::Tree,
        NodeKind::DescriptionList,
        NodeKind::Calendar,
        NodeKind::DatePicker,
        NodeKind::TimePicker,
        NodeKind::Separator,
        NodeKind::Accordion,
        NodeKind::Collapsible,
        NodeKind::HoverCard,
        NodeKind::MenuBar,
        NodeKind::StatusBar,
        NodeKind::KeyboardShortcuts,
        NodeKind::Kbd,
        NodeKind::ColorPicker,
        NodeKind::Rating,
        NodeKind::Resizable,
        NodeKind::Dock,
        NodeKind::Chart,
        NodeKind::Editor,
        NodeKind::RichText,
        NodeKind::Carousel,
        NodeKind::DragDrop,
        NodeKind::Theme,
        NodeKind::AspectRatio,
        NodeKind::Alert,
        NodeKind::Attachment,
        NodeKind::Bubble,
        NodeKind::Command,
        NodeKind::NativeSelect,
        NodeKind::NavigationMenu,
        NodeKind::ScrollArea,
        NodeKind::Item,
        NodeKind::Message,
        NodeKind::MessageScroller,
        NodeKind::ToggleGroup,
        NodeKind::Sonner,
    ]
}

#[test]
fn current_catalog_is_closed_and_serializes_all_supported_kinds() {
    let kinds = all_supported_kinds();
    assert_eq!(kinds.len(), 100);
    for (index, kind) in kinds.into_iter().enumerate() {
        let node = UiNode {
            id: format!("catalog-{index}"),
            kind,
            props: std::collections::BTreeMap::default(),
            children: vec![],
        };
        let encoded = serde_json::to_value(&node).expect("catalog node serializes");
        assert_eq!(
            encoded["kind"],
            json!(serde_json::to_string(&kind).unwrap().trim_matches('"'))
        );
        // round-trip through serde
        let decoded: UiNode = serde_json::from_value(encoded).expect("round-trip");
        assert_eq!(decoded.kind, kind);
    }
    // unknown kind must be rejected
    let unknown = json!({"id":"x","kind":"unknown_kind","props":{},"children":[]});
    assert!(serde_json::from_value::<UiNode>(unknown).is_err());
}

#[test]
fn malformed_catalog_root_is_rejected_before_mutation() {
    let node = UiNode {
        id: "root".into(),
        kind: NodeKind::Text,
        props: [("text".into(), json!("unexpected child"))]
            .into_iter()
            .collect(),
        children: vec![UiNode {
            id: "child".into(),
            kind: NodeKind::Text,
            props: std::collections::BTreeMap::default(),
            children: vec![],
        }],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/pos".into(),
        root: node,
    };
    assert!(
        validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_err()
    );
}

#[test]
fn display_batch_accepts_typed_properties() {
    let cases = [
        (NodeKind::Tag, json!({"label": "New", "variant": "success"})),
        (
            NodeKind::Avatar,
            json!({"fallback": "JD", "alt": "Jane Doe"}),
        ),
        (
            NodeKind::Empty,
            json!({"title": "No services", "description": "Try another filter"}),
        ),
        (NodeKind::Skeleton, json!({"width": 120.0, "height": 16.0})),
        (NodeKind::ProgressCircle, json!({"value": 0.5})),
        (NodeKind::Spinner, json!({"label": "Loading catalog"})),
    ];
    for (index, (kind, properties)) in cases.into_iter().enumerate() {
        let props = properties
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        let node = UiNode {
            id: format!("display-{index}"),
            kind,
            props,
            children: vec![],
        };
        let mount = MountTree {
            protocol_version: studio_protocol::PROTOCOL_VERSION,
            route: "/display".into(),
            root: node,
        };
        assert!(
            validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_ok()
        );
    }
}

#[test]
fn property_closure_rejects_unknown_properties() {
    // known good
    let good = UiNode {
        id: "good".into(),
        kind: NodeKind::Button,
        props: [
            ("label".into(), json!("Pay")),
            ("enabled".into(), json!(true)),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: good,
    };
    assert!(validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_ok());

    // unknown property should be rejected
    let bad = UiNode {
        id: "bad".into(),
        kind: NodeKind::Button,
        props: [("unknown_prop".into(), json!("oops"))]
            .into_iter()
            .collect(),
        children: vec![],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: bad,
    };
    let err =
        validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).unwrap_err();
    assert_eq!(err.code(), ErrorCode::TreeInvalid);
}

#[test]
fn property_closure_validates_typed_values_and_cross_properties() {
    // Slider cross-property: min < max and value in range
    let valid_slider = UiNode {
        id: "slider".into(),
        kind: NodeKind::Slider,
        props: [
            ("label".into(), json!("Volume")),
            ("min".into(), json!(0.0)),
            ("max".into(), json!(10.0)),
            ("value".into(), json!(5.0)),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: valid_slider,
    };
    assert!(validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_ok());

    // invalid: min >= max
    let invalid_slider = UiNode {
        id: "slider".into(),
        kind: NodeKind::Slider,
        props: [
            ("label".into(), json!("Volume")),
            ("min".into(), json!(10.0)),
            ("max".into(), json!(5.0)),
            ("value".into(), json!(7.0)),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: invalid_slider,
    };
    assert!(
        validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_err()
    );

    // Select cross-property: value must be in options
    let valid_select = UiNode {
        id: "select".into(),
        kind: NodeKind::Select,
        props: [
            ("label".into(), json!("Category")),
            ("value".into(), json!("Hair")),
            ("options".into(), json!(["Hair", "Nails"])),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: valid_select,
    };
    assert!(validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_ok());

    let invalid_select = UiNode {
        id: "select".into(),
        kind: NodeKind::Select,
        props: [
            ("label".into(), json!("Category")),
            ("value".into(), json!("Unknown")),
            ("options".into(), json!(["Hair", "Nails"])),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: invalid_select,
    };
    assert!(
        validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_err()
    );

    // RangeSlider, NumberInput, etc. typed checks
    let range = UiNode {
        id: "range".into(),
        kind: NodeKind::RangeSlider,
        props: [
            ("label".into(), json!("Price")),
            ("min".into(), json!(0.0)),
            ("max".into(), json!(100.0)),
            ("start".into(), json!(20.0)),
            ("end".into(), json!(80.0)),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: range,
    };
    assert!(validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_ok());

    // OtpInput length bounds 1..=12
    let otp_bad = UiNode {
        id: "otp".into(),
        kind: NodeKind::OtpInput,
        props: [
            ("label".into(), json!("Code")),
            ("length".into(), json!(20)),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: otp_bad,
    };
    assert!(
        validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_err()
    );

    // common transition property
    let with_transition = UiNode {
        id: "box".into(),
        kind: NodeKind::Box,
        props: [
            ("padding".into(), json!(8.0)),
            (
                "transition".into(),
                json!({"curve":"ease_in","duration_ms":200}),
            ),
        ]
        .into_iter()
        .collect(),
        children: vec![],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: with_transition,
    };
    assert!(validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_ok());

    let bad_transition = UiNode {
        id: "box".into(),
        kind: NodeKind::Box,
        props: [(
            "transition".into(),
            json!({"curve":"invalid","duration_ms":200}),
        )]
        .into_iter()
        .collect(),
        children: vec![],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: bad_transition,
    };
    assert!(
        validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_err()
    );
}

#[test]
fn child_cardinality_rules_are_enforced() {
    // leaf nodes must have 0 children (Text)
    let leaf_with_child = UiNode {
        id: "leaf".into(),
        kind: NodeKind::Text,
        props: [("text".into(), json!("hi"))].into_iter().collect(),
        children: vec![UiNode {
            id: "child".into(),
            kind: NodeKind::Text,
            props: [("text".into(), json!("child"))].into_iter().collect(),
            children: vec![],
        }],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: leaf_with_child,
    };
    assert!(
        validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_err()
    );

    // container can have children
    let container = UiNode {
        id: "root".into(),
        kind: NodeKind::Box,
        props: Default::default(),
        children: vec![UiNode {
            id: "child".into(),
            kind: NodeKind::Text,
            props: [("text".into(), json!("hi"))].into_iter().collect(),
            children: vec![],
        }],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: container,
    };
    assert!(validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_ok());

    // Tooltip requires exactly 1 child
    let tooltip_no_child = UiNode {
        id: "tip".into(),
        kind: NodeKind::Tooltip,
        props: [("message".into(), json!("help"))].into_iter().collect(),
        children: vec![],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: tooltip_no_child,
    };
    assert!(
        validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_err()
    );

    let tooltip_ok = UiNode {
        id: "tip".into(),
        kind: NodeKind::Tooltip,
        props: [("message".into(), json!("help"))].into_iter().collect(),
        children: vec![UiNode {
            id: "content".into(),
            kind: NodeKind::Text,
            props: [("text".into(), json!("hello"))].into_iter().collect(),
            children: vec![],
        }],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: tooltip_ok,
    };
    assert!(validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_ok());

    // Card allows <=1 child, 2 should fail
    let card_two = UiNode {
        id: "card".into(),
        kind: NodeKind::Card,
        props: Default::default(),
        children: vec![
            UiNode {
                id: "a".into(),
                kind: NodeKind::Text,
                props: [("text".into(), json!("a"))].into_iter().collect(),
                children: vec![],
            },
            UiNode {
                id: "b".into(),
                kind: NodeKind::Text,
                props: [("text".into(), json!("b"))].into_iter().collect(),
                children: vec![],
            },
        ],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: card_two,
    };
    assert!(
        validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).is_err()
    );
}

#[test]
fn stable_error_families_are_exhaustive_and_non_oracular() {
    // TreeInvalid for unknown property
    let node = UiNode {
        id: "x".into(),
        kind: NodeKind::Button,
        props: [("bad".into(), json!("x"))].into_iter().collect(),
        children: vec![],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: node,
    };
    let err =
        validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).unwrap_err();
    assert_eq!(err.code(), ErrorCode::TreeInvalid);
    assert_eq!(err.diagnostic().code(), "tree_invalid");
    assert!(!err.to_string().contains("unknown_kind"));

    // MessageTooLarge for oversized string
    let big = "a".repeat(70_000);
    let node = UiNode {
        id: "x".into(),
        kind: NodeKind::Text,
        props: [("text".into(), json!(big))].into_iter().collect(),
        children: vec![],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: node,
    };
    let err =
        validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).unwrap_err();
    assert_eq!(err.code(), ErrorCode::MessageTooLarge);

    // SequenceInvalid
    let batch = studio_protocol::PatchBatch {
        sequence: 0,
        operations: vec![studio_protocol::PatchOp::RemoveNode {
            node_id: "x".into(),
        }],
    };
    let err = studio_protocol::validate_patch_sequence(&batch, Some(0)).unwrap_err();
    assert_eq!(err.code(), ErrorCode::SequenceInvalid);
    assert_eq!(err.diagnostic().code(), "sequence_invalid");

    // Verify all ErrorCode diagnostic codes are stable snake_case
    let codes = [
        ErrorCode::MessageInvalid,
        ErrorCode::MessageTooLarge,
        ErrorCode::ProtocolUnsupported,
        ErrorCode::LifecycleInvalid,
        ErrorCode::TreeInvalid,
        ErrorCode::PatchInvalid,
        ErrorCode::SequenceInvalid,
        ErrorCode::RouteNotFound,
        ErrorCode::NavigationDenied,
        ErrorCode::NavigationTimeout,
        ErrorCode::CapabilityDenied,
        ErrorCode::ActionInvalid,
        ErrorCode::ResourceExhausted,
        ErrorCode::GuestTerminated,
    ];
    for code in codes {
        let diagnostic = match code {
            ErrorCode::MessageInvalid => "message_invalid",
            ErrorCode::MessageTooLarge => "message_too_large",
            ErrorCode::ProtocolUnsupported => "protocol_unsupported",
            ErrorCode::LifecycleInvalid => "lifecycle_invalid",
            ErrorCode::TreeInvalid => "tree_invalid",
            ErrorCode::PatchInvalid => "patch_invalid",
            ErrorCode::SequenceInvalid => "sequence_invalid",
            ErrorCode::RouteNotFound => "route_not_found",
            ErrorCode::NavigationDenied => "navigation_denied",
            ErrorCode::NavigationTimeout => "navigation_timeout",
            ErrorCode::CapabilityDenied => "capability_denied",
            ErrorCode::ActionInvalid => "action_invalid",
            ErrorCode::ResourceExhausted => "resource_exhausted",
            ErrorCode::GuestTerminated => "guest_terminated",
        };
        // ensure diagnostic mapping exists and is stable
        let proto_err = match code {
            ErrorCode::MessageInvalid => ProtocolError::InvalidJson("x".into()),
            ErrorCode::MessageTooLarge => ProtocolError::MessageTooLarge {
                kind: "guest",
                actual: 100,
                limit: 10,
            },
            ErrorCode::ProtocolUnsupported => ProtocolError::UnsupportedVersion(99),
            ErrorCode::TreeInvalid => ProtocolError::InvalidNodeProperty {
                node_id: "x".into(),
                property: "p".into(),
            },
            ErrorCode::PatchInvalid => ProtocolError::InvalidPatchOperationCount(1),
            ErrorCode::SequenceInvalid => ProtocolError::InvalidPatchSequence {
                previous: 1,
                received: 1,
            },
            ErrorCode::RouteNotFound => ProtocolError::InvalidRoute("x".into()),
            ErrorCode::ActionInvalid => ProtocolError::InvalidAction("x"),
            ErrorCode::LifecycleInvalid => ProtocolError::InvalidLifecycle("x"),
            _ => continue,
        };
        assert_eq!(proto_err.diagnostic().code(), diagnostic);
    }
}

#[test]
fn protocol_limits_and_ids_are_bounded() {
    // duplicate node id
    let _root = UiNode {
        id: "dup".into(),
        kind: NodeKind::Box,
        props: Default::default(),
        children: vec![
            UiNode {
                id: "dup".into(),
                kind: NodeKind::Text,
                props: [("text".into(), json!("a"))].into_iter().collect(),
                children: vec![],
            },
            UiNode {
                id: "dup".into(),
                kind: NodeKind::Text,
                props: [("text".into(), json!("b"))].into_iter().collect(),
                children: vec![],
            },
        ],
    };
    // duplicate should be caught even though Box allows children, duplicate id is separate check
    let dup_root = UiNode {
        id: "root".into(),
        kind: NodeKind::Column,
        props: Default::default(),
        children: vec![
            UiNode {
                id: "same".into(),
                kind: NodeKind::Text,
                props: [("text".into(), json!("a"))].into_iter().collect(),
                children: vec![],
            },
            UiNode {
                id: "same".into(),
                kind: NodeKind::Text,
                props: [("text".into(), json!("b"))].into_iter().collect(),
                children: vec![],
            },
        ],
    };
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: dup_root,
    };
    let err =
        validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).unwrap_err();
    assert_eq!(err.code(), ErrorCode::TreeInvalid);

    // deeply nested tree
    let mut deep = UiNode {
        id: "leaf".into(),
        kind: NodeKind::Box,
        props: Default::default(),
        children: vec![],
    };
    for i in 0..70 {
        deep = UiNode {
            id: format!("node-{i}"),
            kind: NodeKind::Box,
            props: Default::default(),
            children: vec![deep],
        };
    }
    let mount = MountTree {
        protocol_version: studio_protocol::PROTOCOL_VERSION,
        route: "/test".into(),
        root: deep,
    };
    let err =
        validate_guest_message(&GuestMessage::Mount(mount), ProtocolLimits::default()).unwrap_err();
    assert_eq!(err.code(), ErrorCode::TreeInvalid);
}
