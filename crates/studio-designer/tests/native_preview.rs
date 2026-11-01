//! Production Focus view tests using native GPUI controls and input dispatch.
#![allow(missing_docs)]

use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Bounds, TestAppContext, WindowBounds, WindowOptions, point, px, size};
use std::{
    collections::BTreeMap,
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};
use studio_design::{
    Actor, ActorId, ActorKind, DefaultDesignerSession, DesignNode, InMemoryDesignerPersistence,
    NodeId, NodeKind, NodeParent, OperationId, ProjectId, PropertyValue,
    STUDIO_DESIGN_SCHEMA_VERSION, Screen, ScreenId, StudioDesign, UndoGroupId,
};
use studio_designer::{FocusView, FocusViewModel};

struct TestWake;
impl Wake for TestWake {
    fn wake(self: Arc<Self>) {}
}
fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(TestWake));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(result) => return result,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}
fn model() -> FocusViewModel<InMemoryDesignerPersistence> {
    let mut design =
        StudioDesign::empty(ProjectId::new("native-preview").unwrap(), "Native preview");
    let screen = ScreenId::new("home").unwrap();
    let root = NodeId::new("root").unwrap();
    let mut container = DesignNode::primitive(root.clone(), "Canvas", NodeKind::Column);
    for (id, name, kind, property, value) in [
        ("authored-text", "Title", NodeKind::Text, "text", "Before"),
        (
            "authored-time",
            "Time",
            NodeKind::TimePicker,
            "value",
            "09:30",
        ),
        (
            "authored-input",
            "Name",
            NodeKind::TextInput,
            "value",
            "Seed",
        ),
        (
            "authored-button",
            "Continue",
            NodeKind::Button,
            "label",
            "Continue",
        ),
    ] {
        let id = NodeId::new(id).unwrap();
        let mut node = DesignNode::primitive(id.clone(), name, kind);
        node.properties
            .insert(property.to_owned(), PropertyValue::String(value.to_owned()));
        container.children.push(id.clone());
        design.parents.insert(
            id.clone(),
            NodeParent::Node {
                node_id: root.clone(),
            },
        );
        design.nodes.insert(id, node);
    }
    design.nodes.insert(root.clone(), container);
    design.parents.insert(
        root.clone(),
        NodeParent::Screen {
            screen_id: screen.clone(),
        },
    );
    design.screens.insert(
        screen.clone(),
        Screen {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: screen.clone(),
            name: "Home".to_owned(),
            route: "/".to_owned(),
            root_node_id: root,
        },
    );
    design.screen_order.push(screen);
    let interaction_id = studio_design::InteractionId::new("continue").unwrap();
    design.interactions.insert(
        interaction_id.clone(),
        studio_design::Interaction {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: interaction_id,
            source: studio_design::InteractionSource {
                node_id: NodeId::new("authored-button").unwrap(),
                event: studio_design::InteractionEvent::Pressed,
            },
            action: studio_design::InteractionAction::SetProperty {
                node_id: NodeId::new("authored-text").unwrap(),
                property: "text".to_owned(),
                value: PropertyValue::String("Activated".to_owned()),
            },
        },
    );
    let session = block_on(DefaultDesignerSession::create(
        InMemoryDesignerPersistence::default(),
        design,
        OperationId::new("create").unwrap(),
        Actor {
            id: ActorId::new("author").unwrap(),
            kind: ActorKind::Human,
            display_name: "Author".to_owned(),
        },
        UndoGroupId::new("create").unwrap(),
    ))
    .unwrap();
    FocusViewModel::from_session(session, None)
}

fn open_focus(
    cx: &mut TestAppContext,
) -> (
    gpui_kit::AnyWindowHandle,
    gpui_kit::Entity<FocusView<InMemoryDesignerPersistence>>,
) {
    cx.update(gpui_kit::init);
    let model = model();
    cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(1400.), px(1200.)),
                ))),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| FocusView::new(model, window, cx)),
        )
        .unwrap()
    })
}

#[gpui_kit::test]
fn focus_mounts_authored_native_controls(cx: &mut TestAppContext) {
    let (handle, _) = open_focus(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("authored-text").label(), Some("Before"));
        let button = window
            .try_find("authored-button")
            .expect("Focus must mount the authored native Button");
        assert_eq!(button.label(), Some("Continue"));
        assert!(button.visible());
        assert_eq!(window.find("authored-input").value(), Some("Seed"));
        window.click("focus-prototype-mode", cx);
        window.click("authored-input", cx);
        assert_eq!(window.find("authored-input").focused(), Some(true));
        window.input("!", cx);
        assert!(window.find("authored-input").value().unwrap().contains('!'));
    })
    .unwrap();
}

#[gpui_kit::test]
fn native_selection_edit_and_prototype_use_the_same_source(cx: &mut TestAppContext) {
    let (handle, view) = open_focus(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("authored-text", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx)
                .model
                .as_ref()
                .unwrap()
                .snapshot()
                .selected_node_id
                .as_ref()
                .map(NodeId::as_str),
            Some("authored-text")
        );
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("focus-edit-text", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let revision = cx.update(|cx| view.read(cx).model.as_ref().unwrap().snapshot().revision_id);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("authored-text").label(), Some("Edited"));
        window.click("focus-prototype-mode", cx);
        window.click("authored-button", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("authored-text").label(), Some("Activated"));
        window.click("focus-prototype-mode", cx);
        assert_eq!(window.find("authored-text").label(), Some("Edited"));
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).model.as_ref().unwrap().snapshot().revision_id,
            revision
        );
    });
}

#[gpui_kit::test]
fn time_picker_uses_native_keyboard_and_accessibility(cx: &mut TestAppContext) {
    let (handle, _) = open_focus(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("focus-prototype-mode", cx);
        assert_eq!(window.find("authored-time").value(), Some("09:30"));
        window.within("authored-time").click("hour", cx);
        window.press("up", cx);
        assert_eq!(window.find("authored-time").value(), Some("10:30"));
        assert_eq!(window.find("authored-time").focused(), Some(true));
        window.press("right", cx);
        window.press("down", cx);
        assert_eq!(window.find("authored-time").value(), Some("10:29"));
        window.click("focus-prototype-mode", cx);
        assert_eq!(window.find("authored-time").value(), Some("09:30"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn time_inspector_commits_valid_values_and_preserves_source_on_invalid_input(
    cx: &mut TestAppContext,
) {
    let (handle, view) = open_focus(cx);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("authored-time", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let before = cx.update(|cx| view.read(cx).model.as_ref().unwrap().snapshot().revision_id);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("focus-time-value", cx);
        window.press("secondary-a", cx);
        window.input("25:00", cx);
        window.click("focus-time-apply", cx);
        assert_eq!(window.find("authored-time").value(), Some("09:30"));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).model.as_ref().unwrap().snapshot().revision_id,
            before
        );
    });
    cx.update_window(handle, |_, window, cx| {
        window.click("focus-time-value", cx);
        window.press("secondary-a", cx);
        window.input("11:45:12", cx);
        window.click("focus-time-apply", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("authored-time").value(), Some("11:45:12"));
    })
    .unwrap();
    cx.update(|cx| assert!(view.read(cx).model.as_ref().unwrap().snapshot().revision_id > before));
}

#[gpui_kit::test]
fn focus_and_runtime_renderer_share_control_values_and_geometry(cx: &mut TestAppContext) {
    use studio_components::native_view::{NativeRenderNode, NativeRuntimeView, ProjectionSurface};
    let projected = model().snapshot().canvas.unwrap();
    let (focus, _) = open_focus(cx);
    let (runtime, _) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                NativeRuntimeView::with_surface(
                    true,
                    ProjectionSurface {
                        root: Some(NativeRenderNode::from(&projected)),
                        assets: BTreeMap::default(),
                    },
                    window,
                    cx,
                )
            })
        })
        .unwrap()
    });
    let measure = |handle, cx: &mut TestAppContext| {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let text = window.find("authored-text");
            let time = window.find("authored-time");
            let input = window.find("authored-input");
            let button = window.find("authored-button");
            (
                text.label().map(str::to_owned),
                input.value().map(str::to_owned),
                time.value().map(str::to_owned),
                button.label().map(str::to_owned),
                input.bounds().size.height,
                button.bounds().size.height,
                time.bounds().top() - text.bounds().top(),
            )
        })
        .unwrap()
    };
    assert_eq!(measure(focus, cx), measure(runtime, cx));
}
