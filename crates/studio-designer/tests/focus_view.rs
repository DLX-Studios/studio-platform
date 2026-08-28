//! Session-driven Focus View MVP checks without requiring a native Wayland display.

use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

use studio_design::{
    Actor, ActorId, ActorKind, DefaultDesignerSession, DesignNode, InMemoryDesignerPersistence,
    OperationId, ProjectId, PropertyValue, Screen, ScreenId, StudioDesign, UndoGroupId,
};
use studio_designer::{FocusSelectionError, FocusViewModel, FocusViewState};
use studio_protocol::NodeKind;

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

fn actor() -> Actor {
    Actor {
        id: ActorId::new("focus-human").unwrap(),
        kind: ActorKind::Human,
        display_name: "Focus Human".to_owned(),
    }
}

fn seed() -> StudioDesign {
    let project_id = ProjectId::new("focus-project").unwrap();
    let screen_id = ScreenId::new("home").unwrap();
    let root_id = studio_design::NodeId::new("canvas").unwrap();
    let node_id = studio_design::NodeId::new("headline").unwrap();
    let mut root = DesignNode::primitive(root_id.clone(), "Canvas", NodeKind::Box);
    root.children.push(node_id.clone());
    root.properties.insert(
        studio_design::CANVAS_RECT_PROPERTY.to_owned(),
        studio_design::CanvasRect::new(0.0, 0.0, 200.0, 80.0)
            .to_property_value()
            .unwrap(),
    );
    let mut node = DesignNode::primitive(node_id.clone(), "Headline", NodeKind::Text);
    node.properties.insert(
        "text".to_owned(),
        PropertyValue::String("Before".to_owned()),
    );
    node.properties.insert(
        studio_design::CANVAS_RECT_PROPERTY.to_owned(),
        studio_design::CanvasRect::new(8.0, 8.0, 160.0, 32.0)
            .to_property_value()
            .unwrap(),
    );
    let mut design = StudioDesign::empty(project_id, "Focus project");
    design
        .nodes
        .extend([(root_id.clone(), root), (node_id.clone(), node)]);
    design.parents.insert(
        root_id.clone(),
        studio_design::NodeParent::Screen {
            screen_id: screen_id.clone(),
        },
    );
    design.parents.insert(
        node_id.clone(),
        studio_design::NodeParent::Node {
            node_id: root_id.clone(),
        },
    );
    design.screens.insert(
        screen_id.clone(),
        Screen {
            schema_version: studio_design::STUDIO_DESIGN_SCHEMA_VERSION,
            id: screen_id,
            name: "Home".to_owned(),
            route: "/home".to_owned(),
            root_node_id: root_id,
        },
    );
    design.screen_order.push(ScreenId::new("home").unwrap());
    design
}

fn model() -> FocusViewModel<InMemoryDesignerPersistence> {
    let persistence = InMemoryDesignerPersistence::default();
    let session = block_on(DefaultDesignerSession::create(
        persistence,
        seed(),
        OperationId::new("create").unwrap(),
        actor(),
        UndoGroupId::new("create-group").unwrap(),
    ))
    .unwrap();
    FocusViewModel::from_session(session, None)
}

#[test]
fn focus_selection_inspector_edit_and_undo_are_session_backed() {
    let mut model = model();
    let node_id = studio_design::NodeId::new("headline").unwrap();
    model.select(&node_id).unwrap();
    assert_eq!(model.snapshot().selected_node_id, Some(node_id));

    let outcome = block_on(model.edit_property(
        OperationId::new("edit-1").unwrap(),
        actor(),
        UndoGroupId::new("text-edit").unwrap(),
        "Edit text",
        "text",
        Some(PropertyValue::String("After".to_owned())),
    ));
    assert!(matches!(
        outcome,
        studio_design::CommandOutcome::Accepted(_)
    ));
    let edited = model.snapshot();
    assert_eq!(edited.revision_id.get(), 1);
    assert_eq!(edited.canvas.unwrap().children[0].props["text"], "After");

    let undone = block_on(model.undo(OperationId::new("undo-1").unwrap(), actor()));
    assert!(matches!(undone, studio_design::CommandOutcome::Accepted(_)));
    let restored = model.snapshot();
    assert_eq!(restored.revision_id.get(), 2);
    assert_eq!(restored.canvas.unwrap().children[0].props["text"], "Before");
}

#[test]
fn focus_reports_selection_and_projection_failures_explicitly() {
    let mut model = model();
    let missing = studio_design::NodeId::new("missing").unwrap();
    assert_eq!(
        model.select(&missing),
        Err(FocusSelectionError::NodeNotFound(missing))
    );

    let outcome = block_on(model.edit_property(
        OperationId::new("edit-without-selection").unwrap(),
        actor(),
        UndoGroupId::new("text-edit").unwrap(),
        "Edit text",
        "text",
        Some(PropertyValue::String("Rejected".to_owned())),
    ));
    assert!(matches!(
        outcome,
        studio_design::CommandOutcome::Rejected(_)
    ));
    assert!(matches!(
        model.snapshot().state,
        FocusViewState::CommandRejected(_)
    ));
}

#[test]
fn ticket_40_controls_submit_through_the_session_authority() {
    let mut model = model();
    let node_id = studio_design::NodeId::new("headline").unwrap();
    model.select(&node_id).unwrap();

    let moved = block_on(model.nudge_selected(
        OperationId::new("nudge-1").unwrap(),
        actor(),
        UndoGroupId::new("nudge").unwrap(),
        studio_design::CanvasPoint::new(1.0, 0.0),
    ));
    assert!(matches!(moved, studio_design::CommandOutcome::Accepted(_)));
    assert_eq!(model.snapshot().revision_id.get(), 1);

    let renamed = block_on(model.rename_selected(
        OperationId::new("rename-1").unwrap(),
        actor(),
        UndoGroupId::new("rename").unwrap(),
    ));
    assert!(matches!(
        renamed,
        studio_design::CommandOutcome::Accepted(_)
    ));
    assert_eq!(
        model.snapshot().selected_node.unwrap().name,
        "Renamed layer"
    );

    let source = include_str!("../src/focus_view.rs");
    for control in [
        "focus-drag",
        "focus-hit-test",
        "focus-nudge-left",
        "focus-resize",
        "focus-rename",
        "focus-reparent",
        "focus-duplicate",
        "focus-delete",
        "focus-restore",
        "focus-diagnostic-details",
        "focus-retry",
    ] {
        assert!(
            source.contains(control),
            "missing visible control {control}"
        );
    }
}

#[test]
fn delete_retains_a_tombstone_selection_for_restore_and_refreshes_persistence() {
    let persistence = InMemoryDesignerPersistence::default();
    let project_id = ProjectId::new("focus-project").unwrap();
    let session = block_on(DefaultDesignerSession::create(
        persistence.clone(),
        seed(),
        OperationId::new("create-delete-restore").unwrap(),
        actor(),
        UndoGroupId::new("create-delete-restore").unwrap(),
    ))
    .unwrap();
    let mut model = FocusViewModel::from_session(session, None);
    let node_id = studio_design::NodeId::new("headline").unwrap();
    model.select(&node_id).unwrap();

    let deleted = block_on(model.delete_selected(
        OperationId::new("delete-headline").unwrap(),
        actor(),
        UndoGroupId::new("delete-headline").unwrap(),
    ));
    assert!(matches!(
        deleted,
        studio_design::CommandOutcome::Accepted(_)
    ));
    assert_eq!(model.snapshot().selected_node_id, Some(node_id.clone()));
    assert!(model.snapshot().selected_node.is_none());
    assert_eq!(model.selected_tombstones(), vec![node_id.clone()]);
    let durable_after_delete = persistence.transaction(&project_id).unwrap();
    assert!(
        durable_after_delete
            .state
            .current
            .tombstones
            .contains_key(&node_id)
    );

    let restored = block_on(model.restore_selected(
        OperationId::new("restore-headline").unwrap(),
        actor(),
        UndoGroupId::new("restore-headline").unwrap(),
    ));
    assert!(matches!(
        restored,
        studio_design::CommandOutcome::Accepted(_)
    ));
    assert_eq!(model.snapshot().selected_node_id, Some(node_id.clone()));
    assert!(model.snapshot().selected_node.is_some());
    assert!(model.selected_tombstones().is_empty());
    assert_eq!(model.snapshot().revision_id.get(), 2);

    let reopened = block_on(FocusViewModel::open(persistence, &project_id, None)).unwrap();
    let reopened_snapshot = reopened.snapshot();
    assert!(reopened_snapshot.canvas.is_some());
    assert!(reopened_snapshot.selected_node_id.is_none());
    assert!(reopened_snapshot.selected_node.is_none());
    assert!(!reopened.source_snapshot().tombstones.contains_key(&node_id));
}
