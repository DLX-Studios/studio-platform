#![allow(missing_docs)]

use serde_json::json;
use studio_components::{
    COMPONENT_RENDERER_READINESS, ComponentCatalog, DispatchErrorCode, HostEventDispatcher,
    InputAction, NativeLayer, RuntimeControl, certify_renderer_readiness, component_readiness,
    uncertified_renderer_kinds,
};
use studio_protocol::{HostEvent, NodeKind, UiNode};
use studio_ui::InstanceId;

const ALL_KINDS: [NodeKind; 43] = [
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
    NodeKind::BottomSheet,
    NodeKind::Toast,
    NodeKind::Tooltip,
];

fn node(id: &str, kind: NodeKind, props: &[(&str, serde_json::Value)]) -> UiNode {
    UiNode {
        id: id.into(),
        kind,
        props: props
            .iter()
            .map(|(key, value)| ((*key).into(), value.clone()))
            .collect(),
        children: vec![],
    }
}

#[test]
fn maps_every_closed_protocol_kind_to_a_studio_owned_native_layer() {
    let catalog = ComponentCatalog::default();
    for kind in ALL_KINDS {
        let mapped = catalog.map(&node("node", kind, &[])).unwrap();
        assert_eq!(mapped.kind, kind);
        assert_ne!(mapped.layer, NativeLayer::WebOrDom);
    }

    assert_eq!(
        catalog.map(&node("box", NodeKind::Box, &[])).unwrap().layer,
        NativeLayer::GpuiDiv
    );
    assert_eq!(
        catalog
            .map(&node("text", NodeKind::Text, &[]))
            .unwrap()
            .layer,
        NativeLayer::GpuiText
    );
    assert_eq!(
        catalog
            .map(&node("dialog", NodeKind::Dialog, &[]))
            .unwrap()
            .layer,
        NativeLayer::HostOverlay
    );
}

#[test]
fn maps_extended_component_catalog_to_native_layers() {
    let catalog = ComponentCatalog::default();
    let kinds = [
        NodeKind::AlertDialog,
        NodeKind::Popover,
        NodeKind::Sheet,
        NodeKind::Drawer,
        NodeKind::Notification,
        NodeKind::Banner,
        NodeKind::ContextMenu,
        NodeKind::CommandPalette,
        NodeKind::Scaffold,
        NodeKind::AppBar,
        NodeKind::Sidebar,
        NodeKind::NavigationBar,
        NodeKind::NavigationRail,
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
    ];
    for kind in kinds {
        assert_ne!(
            catalog.map(&node("extended", kind, &[])).unwrap().layer,
            NativeLayer::WebOrDom
        );
    }
}

#[test]
fn renderer_readiness_matrix_distinguishes_mapped_from_rendered() {
    assert_eq!(COMPONENT_RENDERER_READINESS.len(), 100);
    for readiness in COMPONENT_RENDERER_READINESS {
        assert!(readiness.protocol_declared);
        assert!(readiness.native_mapped);
    }

    for kind in [
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
        NodeKind::Separator,
        NodeKind::AspectRatio,
    ] {
        let readiness = component_readiness(kind);
        assert!(readiness.semantically_rendered, "{kind:?}");
        assert!(readiness.verified, "{kind:?}");
    }

    for kind in [
        NodeKind::Accordion,
        NodeKind::TimePicker,
        NodeKind::ToggleGroup,
        NodeKind::Chart,
    ] {
        let readiness = component_readiness(kind);
        assert!(readiness.native_mapped, "{kind:?}");
        assert!(!readiness.semantically_rendered, "{kind:?}");
        assert!(!readiness.verified, "{kind:?}");
    }
}

#[test]
fn release_certification_rejects_unrendered_catalog_kinds() {
    let missing = certify_renderer_readiness().expect_err("deferred kinds must block release");
    assert_eq!(missing, uncertified_renderer_kinds());
    assert!(missing.contains(&NodeKind::TimePicker));
    assert!(missing.contains(&NodeKind::Chart));
}

#[test]
fn form_input_kinds_are_semantically_rendered_after_batch_b() {
    for kind in [
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
    ] {
        let readiness = component_readiness(kind);
        assert!(readiness.semantically_rendered, "{kind:?}");
        assert!(readiness.verified, "{kind:?}");
    }
    // SecretInput stays host-owned even after semantic rendering: its value never enters the
    // protocol event path (see HostEventDispatcher: SecretInput accepts no TextChanged action).
    let secret = ComponentCatalog::default()
        .map(&node(
            "secret",
            NodeKind::SecretInput,
            &[("label", json!("PIN"))],
        ))
        .unwrap();
    assert!(secret.host_owned_value);
}

#[test]
fn overlay_navigation_and_data_display_kinds_are_rendered_after_batch_c() {
    for kind in [
        NodeKind::Dialog,
        NodeKind::AlertDialog,
        NodeKind::Popover,
        NodeKind::Sheet,
        NodeKind::BottomSheet,
        NodeKind::Drawer,
        NodeKind::Toast,
        NodeKind::Notification,
        NodeKind::Banner,
        NodeKind::ContextMenu,
        NodeKind::CommandPalette,
        NodeKind::Tooltip,
        NodeKind::ProgressIndicator,
        NodeKind::ProgressCircle,
        NodeKind::Spinner,
        NodeKind::Scaffold,
        NodeKind::AppBar,
        NodeKind::Sidebar,
        NodeKind::NavigationBar,
        NodeKind::NavigationRail,
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
    ] {
        let readiness = component_readiness(kind);
        assert!(readiness.semantically_rendered, "{kind:?}");
        assert!(readiness.verified, "{kind:?}");
    }
}

#[test]
fn interactive_controls_expose_keyboard_pointer_touch_and_accessibility_contracts() {
    let catalog = ComponentCatalog::default();
    for (kind, props) in [
        (NodeKind::Button, vec![("label", json!("Checkout"))]),
        (
            NodeKind::IconButton,
            vec![("accessibility_label", json!("Remove item"))],
        ),
        (NodeKind::Checkbox, vec![("label", json!("Tax exempt"))]),
        (NodeKind::Switch, vec![("label", json!("Receipt"))]),
        (NodeKind::Slider, vec![("label", json!("Discount"))]),
        (NodeKind::Select, vec![("label", json!("Category"))]),
        (NodeKind::TextInput, vec![("label", json!("Search"))]),
        (
            NodeKind::SecretInput,
            vec![("label", json!("Card authorization"))],
        ),
    ] {
        let mapped = catalog.map(&node("control", kind, &props)).unwrap();
        assert!(mapped.focusable);
        assert!(mapped.accessibility_label.is_some());
        assert!(mapped.minimum_pointer_target.width >= 44.0);
        assert!(mapped.minimum_pointer_target.height >= 44.0);
        assert!(mapped.keyboard_enabled);
        assert!(mapped.pointer_enabled);
        assert!(mapped.touch_enabled);
    }
}

#[test]
fn four_first_class_controls_are_available_to_the_runtime_catalog() {
    let catalog = ComponentCatalog::default();
    assert_eq!(
        catalog
            .map(&node("button", NodeKind::Button, &[]))
            .unwrap()
            .control,
        Some(RuntimeControl::Button)
    );
    assert_eq!(
        catalog
            .map(&node("input", NodeKind::TextInput, &[]))
            .unwrap()
            .control,
        Some(RuntimeControl::Input)
    );
    assert_eq!(
        catalog
            .map(&node("select", NodeKind::Select, &[]))
            .unwrap()
            .control,
        Some(RuntimeControl::Select)
    );
    assert_eq!(
        catalog
            .map(&node("slider", NodeKind::Slider, &[]))
            .unwrap()
            .control,
        Some(RuntimeControl::Slider)
    );
}

#[test]
fn secret_input_is_host_owned_and_never_accepts_or_emits_raw_text() {
    let catalog = ComponentCatalog::default();
    let secret = catalog
        .map(&node(
            "authorization",
            NodeKind::SecretInput,
            &[("label", json!("Authorization"))],
        ))
        .unwrap();
    assert!(secret.host_owned_value);

    let invalid = node(
        "authorization",
        NodeKind::SecretInput,
        &[
            ("label", json!("Authorization")),
            ("value", json!("4111111111111111")),
        ],
    );
    assert!(catalog.map(&invalid).is_err());
}

#[test]
fn dispatches_typed_non_secret_events_from_host_owner_context() {
    let owner = InstanceId::new("instance-a").unwrap();
    let other = InstanceId::new("instance-b").unwrap();
    let mut dispatcher = HostEventDispatcher::new(owner.clone());
    dispatcher.register("button", NodeKind::Button).unwrap();
    dispatcher.register("scroll", NodeKind::ScrollView).unwrap();
    dispatcher.register("slider", NodeKind::Slider).unwrap();
    dispatcher.register("select", NodeKind::Select).unwrap();

    let cases = [
        ("button", InputAction::PointerClick, "pressed"),
        ("button", InputAction::KeyboardActivate, "pressed"),
        (
            "scroll",
            InputAction::Scroll {
                delta_x: 0.0,
                delta_y: 24.0,
            },
            "scrolled",
        ),
        ("slider", InputAction::SliderDrag { value: 0.25 }, "changed"),
        (
            "select",
            InputAction::SelectionChanged {
                value: "Hair".to_owned(),
            },
            "changed",
        ),
    ];
    for (node_id, action, expected_event) in cases {
        let event = dispatcher.dispatch(&owner, node_id, action).unwrap();
        let HostEvent::Ui(event) = event else {
            panic!("expected UI event")
        };
        assert_eq!(event.node_id, node_id);
        assert_eq!(event.event, expected_event);
        assert!(!serde_json::to_value(event)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("owner"));
    }

    assert_eq!(
        dispatcher
            .dispatch(&other, "button", InputAction::PointerClick)
            .unwrap_err()
            .code(),
        DispatchErrorCode::OwnerMismatch
    );
}
