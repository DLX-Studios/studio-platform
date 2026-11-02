//! Native renderer interaction and retained-state tests against production views.
#![allow(missing_docs)]
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, WindowOptions, point};
use serde_json::json;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use studio_components::{
    InputAction,
    native_view::{NativeRenderNode, NativeRuntimeView, NativeSurface, ProjectionSurface},
};
use studio_protocol::{NodeKind, UiNode};

type Recorded = Rc<RefCell<Vec<(String, InputAction)>>>;
struct Surface {
    projection: ProjectionSurface,
    events: Recorded,
}
impl NativeSurface for Surface {
    type Error = std::convert::Infallible;
    fn render_tree(&self) -> Option<NativeRenderNode> {
        self.projection.render_tree()
    }
    fn process_input(&mut self, id: &str, action: InputAction) -> Result<(), Self::Error> {
        self.events.borrow_mut().push((id.to_owned(), action));
        Ok(())
    }
    fn poll_action_results(&mut self) -> Result<bool, Self::Error> {
        Ok(false)
    }
    fn asset(&self, path: &str) -> Option<&[u8]> {
        self.projection.asset(path)
    }
    fn warning(&self) -> Option<&str> {
        None
    }
}
fn node(id: &str, kind: NodeKind, props: serde_json::Value) -> UiNode {
    UiNode {
        id: id.into(),
        kind,
        props: serde_json::from_value(props).unwrap(),
        children: vec![],
    }
}
#[gpui_kit::test]
fn time_controls_keep_independent_values_and_honor_host_updates(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut root = node("root", NodeKind::Column, json!({"gap": 8}));
    root.children = vec![
        node("start", NodeKind::TimePicker, json!({"value":"09:30:15"})),
        node(
            "end",
            NodeKind::TimePicker,
            json!({"value":"17:00", "enabled":false}),
        ),
    ];
    let events = Recorded::default();
    let surface = Surface {
        projection: ProjectionSurface {
            root: Some(NativeRenderNode::from(&root)),
            assets: BTreeMap::new(),
        },
        events: events.clone(),
    };
    let (handle, view) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| NativeRuntimeView::with_surface(true, surface, window, cx))
        })
        .unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("start").value(), Some("09:30:15"));
        window.within("start").click("second", cx);
        window.press("up", cx);
        assert_eq!(window.find("start").value(), Some("09:30:16"));
        assert_eq!(window.find("end").disabled(), Some(true));
        window.click("end", cx);
        window.press("up", cx);
        assert_eq!(window.find("end").value(), Some("17:00"));
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        *events.borrow(),
        vec![(
            "start".into(),
            InputAction::SelectionChanged {
                value: "09:30:16".into()
            }
        )]
    );
    cx.update(|cx| {
        view.update(cx, |view, cx| {
            view.update_surface(
                |surface| {
                    let root = surface.projection.root.as_mut().unwrap();
                    root.children[0]
                        .props
                        .insert("value".into(), json!("10:45"));
                    true
                },
                cx,
            );
        });
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("start").value(), Some("10:45"));
        assert_eq!(window.find("end").value(), Some("17:00"));
        assert!(window.within("start").try_find("second").is_none());
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        events.borrow().len(),
        1,
        "host updates must not emit a user event"
    );
}

#[gpui_kit::test]
fn deeply_nested_tree_fits_the_default_native_test_stack(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut root = node("leaf", NodeKind::Button, json!({"label":"Nested"}));
    for depth in 0..32 {
        let mut parent = node(&format!("container-{depth}"), NodeKind::Column, json!({}));
        parent.children.push(root);
        root = parent;
    }
    let surface = ProjectionSurface {
        root: Some(NativeRenderNode::from(&root)),
        assets: BTreeMap::new(),
    };
    let (handle, _) = cx.update(|cx| {
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| NativeRuntimeView::with_surface(true, surface, window, cx))
        })
        .unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("leaf").label(), Some("Nested"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn preview_zoom_scales_layout_text_and_pointer_hit_targets(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut root = node("root", NodeKind::Column, json!({"gap": 8}));
    root.children.push(node(
        "action",
        NodeKind::Button,
        json!({"label":"Zoomed action", "variant":"secondary"}),
    ));
    let events = Recorded::default();
    let surface = Surface {
        projection: ProjectionSurface {
            root: Some(NativeRenderNode::from(&root)),
            assets: BTreeMap::new(),
        },
        events: events.clone(),
    };
    let mut view_entity = None;
    let (handle, view_entity) = cx.update(|cx| {
        let handle = cx
            .open_window(WindowOptions::default(), |window, cx| {
                let view = cx
                    .new(|view_cx| NativeRuntimeView::with_surface(true, surface, window, view_cx));
                view_entity = Some(view.clone());
                view
            })
            .unwrap();
        (handle.into(), view_entity.take().unwrap())
    });
    let initial_bounds = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            (window.find("action").bounds(), window.rem_size().as_f32())
        })
        .unwrap();

    cx.update_window(handle, |_, window, cx| {
        view_entity.update(cx, |view, view_cx| {
            assert!(view.set_zoom_milli(2_000, window, view_cx));
        });
        window.render_frame(cx);
        let scaled = window.find("action").bounds();
        let scaled_rem = window.rem_size().as_f32();
        assert!(
            scaled_rem > initial_bounds.1 * 1.8,
            "text metrics should scale with the layout: {} -> {}",
            initial_bounds.1,
            scaled_rem
        );
        assert!(
            scaled.size.height.as_f32() > initial_bounds.0.size.height.as_f32() * 1.8,
            "button height should scale: {} -> {}",
            initial_bounds.0.size.height.as_f32(),
            scaled.size.height.as_f32()
        );
        window.click_at(
            "action",
            point(scaled.size.width / 2.0, scaled.size.height * 0.75),
            cx,
        );
        assert_eq!(
            events.borrow().as_slice(),
            &[("action".to_owned(), InputAction::PointerClick)],
            "a pointer in the scaled-only area must activate the component"
        );

        view_entity.update(cx, |view, view_cx| {
            assert!(view.set_zoom_milli(1_000, window, view_cx));
        });
        window.render_frame(cx);
        assert_eq!(window.find("action").bounds().size, initial_bounds.0.size);
        assert!(
            (window.rem_size().as_f32() - initial_bounds.1).abs() < f32::EPSILON,
            "resetting zoom should restore the original rem size"
        );
    })
    .unwrap();
}
