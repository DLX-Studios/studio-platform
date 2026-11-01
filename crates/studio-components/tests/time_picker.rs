#![allow(missing_docs)]
use serde_json::json;
use studio_components::{
    ComponentCatalog, DispatchError, HostEventDispatcher, InputAction, RuntimeControl,
};
use studio_protocol::{HostEvent, NodeKind, UiNode};
use studio_ui::InstanceId;

#[test]
fn time_picker_is_focusable_and_dispatches_canonical_changed_events() {
    let node = UiNode {
        id: "time".into(),
        kind: NodeKind::TimePicker,
        props: std::collections::BTreeMap::from([("label".into(), json!("Start time"))]),
        children: vec![],
    };
    let mapped = ComponentCatalog::default().map(&node).unwrap();
    assert_eq!(mapped.control, Some(RuntimeControl::Time));
    assert!(mapped.focusable && mapped.keyboard_enabled && mapped.pointer_enabled);
    assert_eq!(mapped.accessibility_label.as_deref(), Some("Start time"));
    let owner = InstanceId::new("time-owner").unwrap();
    let mut dispatcher = HostEventDispatcher::new(owner.clone());
    dispatcher.register("time", NodeKind::TimePicker).unwrap();
    let action = |value: &str| InputAction::SelectionChanged {
        value: value.to_owned(),
    };
    let HostEvent::Ui(event) = dispatcher
        .dispatch(&owner, "time", action("09:30:15"))
        .unwrap()
    else {
        panic!("UI event expected")
    };
    assert_eq!(event.event, "changed");
    assert_eq!(event.payload, json!({"value": "09:30:15"}));
    assert_eq!(
        dispatcher.dispatch(&owner, "time", action("25:00")),
        Err(DispatchError::ActionInvalid)
    );
    assert_eq!(
        dispatcher.dispatch(
            &InstanceId::new("foreign").unwrap(),
            "time",
            action("09:30")
        ),
        Err(DispatchError::OwnerMismatch)
    );
}
