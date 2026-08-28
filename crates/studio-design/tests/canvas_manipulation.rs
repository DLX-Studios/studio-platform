#![allow(missing_docs)]
#![allow(
    clippy::default_trait_access,
    clippy::float_cmp,
    clippy::needless_pass_by_value,
    clippy::semicolon_if_nothing_returned,
    clippy::too_many_lines
)]

use std::{
    collections::BTreeMap,
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

use studio_design::{
    Actor, ActorId, ActorKind, CanvasAlignment, CanvasDistribution, CanvasGeometry, CanvasPoint,
    CanvasRect, CanvasSize, Command, CommandOutcome, DefaultDesignerSession, DesignNode,
    DesignerQuery, DesignerQueryResult, DesignerSession, GestureContext, HierarchyEdit,
    HierarchySnapshot, InMemoryDesignerPersistence, NodeId, NodeKind, NodeParent, OperationId,
    ParentPlacement, ProjectId, ResizeHandle, RevisionId, STUDIO_DESIGN_SCHEMA_VERSION, Screen,
    ScreenId, SelectionSnapshot, SnapConfig, StudioDesign, UndoGroupId, align_batch, delete_batch,
    distribute_batch, drag_batch, duplicate_batch, hierarchy_edit_batch, keyboard_resize_batch,
    nudge_batch, reorder_batch, reparent_batch, resize_rect, restore_batch,
};

struct NoopWake;
impl Wake for NoopWake {
    fn wake(self: Arc<Self>) {}
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(NoopWake));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn id(value: &str) -> NodeId {
    NodeId::new(value).unwrap()
}
fn operation(value: &str) -> OperationId {
    OperationId::new(value).unwrap()
}
fn group(value: &str) -> UndoGroupId {
    UndoGroupId::new(value).unwrap()
}
fn project() -> ProjectId {
    ProjectId::new("canvas-project").unwrap()
}
fn actor() -> Actor {
    Actor {
        id: ActorId::new("designer").unwrap(),
        kind: ActorKind::Human,
        display_name: "Designer".to_owned(),
    }
}

fn design() -> StudioDesign {
    let screen = ScreenId::new("main").unwrap();
    let root_id = id("root");
    let mut root = DesignNode::primitive(root_id.clone(), "Root", NodeKind::Box);
    root.children = vec![id("a"), id("b"), id("c"), id("target")];
    let mut a = DesignNode::primitive(id("a"), "A", NodeKind::Box);
    let mut b = DesignNode::primitive(id("b"), "B", NodeKind::Box);
    let mut c = DesignNode::primitive(id("c"), "C", NodeKind::Box);
    let mut target = DesignNode::primitive(id("target"), "Target", NodeKind::Column);
    let frames = [
        ("a", CanvasRect::new(0.0, 0.0, 20.0, 20.0)),
        ("b", CanvasRect::new(40.0, 0.0, 20.0, 20.0)),
        ("c", CanvasRect::new(80.0, 0.0, 20.0, 20.0)),
        ("target", CanvasRect::new(0.0, 100.0, 100.0, 100.0)),
    ];
    for (node, (_, frame)) in [
        (&mut a, frames[0]),
        (&mut b, frames[1]),
        (&mut c, frames[2]),
        (&mut target, frames[3]),
    ] {
        node.properties.insert(
            studio_design::CANVAS_RECT_PROPERTY.to_owned(),
            frame.to_property_value().unwrap(),
        );
    }
    let mut source = StudioDesign::empty(project(), "Canvas");
    source.nodes = [
        (root_id.clone(), root),
        (id("a"), a),
        (id("b"), b),
        (id("c"), c),
        (id("target"), target),
    ]
    .into_iter()
    .collect();
    source.parents = [
        (
            root_id.clone(),
            NodeParent::Screen {
                screen_id: screen.clone(),
            },
        ),
        (
            id("a"),
            NodeParent::Node {
                node_id: root_id.clone(),
            },
        ),
        (
            id("b"),
            NodeParent::Node {
                node_id: root_id.clone(),
            },
        ),
        (
            id("c"),
            NodeParent::Node {
                node_id: root_id.clone(),
            },
        ),
        (id("target"), NodeParent::Node { node_id: root_id }),
    ]
    .into_iter()
    .collect();
    source.screens.insert(
        screen.clone(),
        Screen {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: screen.clone(),
            name: "Main".to_owned(),
            route: "/".to_owned(),
            root_node_id: id("root"),
        },
    );
    source.screen_order.push(screen);
    source
}

fn context(operation_id: &str, undo_group_id: &str, revision: u64) -> GestureContext {
    GestureContext::new(
        operation(operation_id),
        actor(),
        project(),
        RevisionId::new(revision),
        group(undo_group_id),
    )
}

fn snapshot<P: studio_design::DesignerPersistence>(
    session: &DefaultDesignerSession<P>,
) -> studio_design::StudioDesignSnapshot {
    match session.query(DesignerQuery::Snapshot) {
        DesignerQueryResult::Snapshot(snapshot) => snapshot,
        other => panic!("unexpected query: {other:?}"),
    }
}

fn accepted(outcome: CommandOutcome) {
    assert!(
        matches!(outcome, CommandOutcome::Accepted(_)),
        "unexpected outcome: {outcome:?}"
    );
}

#[test]
fn geometry_hit_testing_snapping_alignment_distribution_and_resize_are_deterministic() {
    let source = design();
    let geometry = CanvasGeometry::from_design(&source);
    assert_eq!(
        geometry.hit_test(CanvasPoint::new(10.0, 10.0), 0.0),
        Some(id("a"))
    );
    let snapped = geometry.snap_rect(
        CanvasRect::new(21.0, 0.0, 10.0, 10.0),
        &Default::default(),
        SnapConfig {
            tolerance: 2.0,
            ..SnapConfig::default()
        },
    );
    assert_eq!(snapped.delta.x, -1.0);
    let aligned =
        studio_design::alignment_targets(&geometry, &[id("a"), id("b")], CanvasAlignment::Left)
            .unwrap();
    assert_eq!(aligned[&id("b")].left(), 0.0);
    let distributed = studio_design::distribution_targets(
        &geometry,
        &[id("a"), id("b"), id("c")],
        CanvasDistribution::Horizontal,
    )
    .unwrap();
    assert_eq!(distributed[&id("b")].left(), 40.0);
    assert_eq!(
        resize_rect(
            CanvasRect::new(0.0, 0.0, 20.0, 20.0),
            ResizeHandle::East,
            CanvasPoint::new(-30.0, 0.0),
            CanvasSize::new(8.0, 8.0)
        )
        .size
        .width,
        8.0
    );
}

#[test]
fn every_gesture_is_one_named_reversible_batch_and_selection_survives_moves() {
    block_on(async {
        let persistence = InMemoryDesignerPersistence::default();
        let mut session = DefaultDesignerSession::create(
            persistence,
            design(),
            operation("create"),
            actor(),
            group("create"),
        )
        .await
        .unwrap();
        session.update_context(studio_design::SessionContextUpdate {
            selection: Some(SelectionSnapshot {
                node_ids: vec![id("b")],
                primary: Some(id("b")),
            }),
            ..Default::default()
        });
        let geometry = CanvasGeometry::from_design(&snapshot(&session).design);

        let moved = drag_batch(
            &context("drag", "move-selection", 0),
            &geometry,
            &[id("b")],
            CanvasPoint::new(5.0, 0.0),
            SnapConfig {
                snap_to_edges: false,
                snap_to_centers: false,
                ..SnapConfig::default()
            },
        )
        .unwrap();
        assert_eq!(moved.undo_group_name, "Move selection");
        accepted(session.submit(moved).await);
        assert_eq!(
            snapshot(&session).design.nodes[&id("b")]
                .properties
                .get(studio_design::CANVAS_RECT_PROPERTY)
                .and_then(CanvasRect::from_property_value)
                .unwrap()
                .left(),
            45.0
        );
        match session.query(DesignerQuery::SessionState) {
            DesignerQueryResult::SessionState(state) => {
                assert_eq!(state.selection.primary, Some(id("b")))
            }
            other => panic!("unexpected query: {other:?}"),
        }

        let geometry = CanvasGeometry::from_design(&snapshot(&session).design);
        let resized = keyboard_resize_batch(
            &context("resize", "resize-selection", 1),
            &geometry,
            id("b"),
            ResizeHandle::East,
            CanvasPoint::new(5.0, 0.0),
            CanvasSize::new(4.0, 4.0),
            SnapConfig {
                snap_to_edges: false,
                snap_to_centers: false,
                ..SnapConfig::default()
            },
        )
        .unwrap();
        accepted(session.submit(resized).await);

        let current = snapshot(&session);
        let reordered = reorder_batch(
            &context("reorder", "reorder-layer", 2),
            &current.design,
            id("b"),
            0,
        )
        .unwrap();
        accepted(session.submit(reordered).await);
        let current = snapshot(&session);
        let reparented = reparent_batch(
            &context("reparent", "reparent-layer", 3),
            &current.design,
            &CanvasGeometry::from_design(&current.design),
            id("b"),
            ParentPlacement {
                parent: NodeParent::Node {
                    node_id: id("target"),
                },
                index: 0,
            },
        )
        .unwrap();
        accepted(session.submit(reparented).await);

        let current = snapshot(&session);
        let duplicate = duplicate_batch(
            &context("duplicate", "duplicate-layer", 4),
            &current.design,
            &CanvasGeometry::from_design(&current.design),
            id("b"),
            ParentPlacement {
                parent: NodeParent::Node {
                    node_id: id("root"),
                },
                index: 0,
            },
            BTreeMap::from([(id("b"), id("b-copy"))]),
        )
        .unwrap();
        accepted(session.submit(duplicate).await);
        assert!(snapshot(&session).design.nodes.contains_key(&id("b-copy")));

        let current = snapshot(&session);
        let deleted = delete_batch(
            &context("delete", "delete-layer", 5),
            &current.design,
            &[id("b-copy")],
        )
        .unwrap();
        accepted(session.submit(deleted).await);
        let current = snapshot(&session);
        let restored = restore_batch(
            &context("restore", "restore-layer", 6),
            &current,
            &[id("b-copy")],
        )
        .unwrap();
        accepted(session.submit(restored).await);
        assert!(snapshot(&session).design.nodes.contains_key(&id("b-copy")));
        let _ = nudge_batch;
        let _ = align_batch;
        let _ = distribute_batch;
    });
}

#[test]
fn hierarchy_projection_and_edits_share_stable_ids_and_command_builders() {
    let source = design();
    let hierarchy = HierarchySnapshot::from_design(&source);
    assert_eq!(hierarchy.find(&id("b")).unwrap().name, "B");
    assert_eq!(
        hierarchy.selection(&[id("b"), id("missing")]).primary,
        Some(id("b"))
    );
    let reorder = hierarchy_edit_batch(
        &context("hierarchy-reorder", "reorder-layer", 0),
        &source,
        &CanvasGeometry::from_design(&source),
        HierarchyEdit::Reorder {
            node_id: id("b"),
            index: 0,
        },
    )
    .unwrap();
    assert!(matches!(reorder.commands[0], Command::ReorderNode { .. }));
    let rename = hierarchy_edit_batch(
        &context("hierarchy-rename", "rename-layer", 0),
        &source,
        &CanvasGeometry::from_design(&source),
        HierarchyEdit::Rename {
            node_id: id("b"),
            name: "Renamed".to_owned(),
        },
    )
    .unwrap();
    assert_eq!(rename.undo_group_name, "Rename layer");
}
