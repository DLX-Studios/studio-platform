#![allow(missing_docs)]

use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

use studio_design::{
    Actor, ActorId, ActorKind, Command, CommandBatch, CommandOutcome, DefaultDesignerSession,
    DesignNode, DesignerQuery, DesignerQueryResult, DesignerSession, InMemoryDesignerPersistence,
    InputEnvironment, LayoutPosition, LayoutProperties, Length, LengthUnit, NodeId, NodeParent,
    OperationId, ProjectId, ResponsiveVariant, ResponsiveVariantId, RevisionId,
    STUDIO_DESIGN_SCHEMA_VERSION, Screen, ScreenId, StudioDesign, UndoGroupId,
};
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

fn node_id(value: &str) -> NodeId {
    NodeId::new(value).unwrap()
}

fn project_id() -> ProjectId {
    ProjectId::new("layout-project").unwrap()
}

fn operation_id(value: &str) -> OperationId {
    OperationId::new(value).unwrap()
}

fn undo_group_id(value: &str) -> UndoGroupId {
    UndoGroupId::new(value).unwrap()
}

fn variant_id(value: &str) -> ResponsiveVariantId {
    ResponsiveVariantId::new(value).unwrap()
}

fn actor() -> Actor {
    Actor {
        id: ActorId::new("designer").unwrap(),
        kind: ActorKind::Human,
        display_name: "Designer".to_owned(),
    }
}

fn px(value: &str) -> Length {
    Length {
        value: value.to_owned(),
        unit: LengthUnit::Pixels,
    }
}

fn batch(base_revision: RevisionId, operation: &str, commands: Vec<Command>) -> CommandBatch {
    CommandBatch {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        operation_id: operation_id(operation),
        actor: actor(),
        project_id: project_id(),
        base_revision,
        undo_group_id: undo_group_id(operation),
        undo_group_name: operation.to_owned(),
        preconditions: Vec::new(),
        commands,
    }
}

fn stack_design() -> StudioDesign {
    let screen_id = ScreenId::new("screen").unwrap();
    let root_id = node_id("stack");
    let image_id = node_id("media");
    let badge_id = node_id("badge");
    let mut root = DesignNode::primitive(root_id.clone(), "Media stack", NodeKind::Stack);
    root.children = vec![image_id.clone(), badge_id.clone()];
    let image = DesignNode::primitive(image_id.clone(), "Image", NodeKind::Image);
    let badge = DesignNode::primitive(badge_id.clone(), "Badge", NodeKind::Badge);
    let mut design = StudioDesign::empty(project_id(), "Layout fixture");
    design.nodes = [
        (root_id.clone(), root),
        (image_id.clone(), image),
        (badge_id.clone(), badge),
    ]
    .into_iter()
    .collect();
    design.parents = [
        (
            root_id.clone(),
            NodeParent::Screen {
                screen_id: screen_id.clone(),
            },
        ),
        (
            image_id,
            NodeParent::Node {
                node_id: root_id.clone(),
            },
        ),
        (badge_id, NodeParent::Node { node_id: root_id }),
    ]
    .into_iter()
    .collect();
    design.screens.insert(
        screen_id.clone(),
        Screen {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: screen_id.clone(),
            name: "Screen".to_owned(),
            route: "/".to_owned(),
            root_node_id: node_id("stack"),
        },
    );
    design.screen_order.push(screen_id);
    design
}

fn created(design: StudioDesign) -> DefaultDesignerSession<InMemoryDesignerPersistence> {
    block_on(DefaultDesignerSession::create(
        InMemoryDesignerPersistence::default(),
        design,
        operation_id("create"),
        actor(),
        undo_group_id("create"),
    ))
    .unwrap()
}

#[test]
fn badge_over_image_uses_ordinary_primitives_and_typed_positioning() {
    let mut session = created(stack_design());
    let mut layout = LayoutProperties::overlay();
    layout.position = Some(LayoutPosition {
        bottom: Some(px("8")),
        left: Some(px("8")),
        ..LayoutPosition::default()
    });
    let outcome = block_on(session.submit(batch(
        RevisionId::INITIAL,
        "place-badge",
        vec![Command::set_layout(node_id("badge"), layout.clone())],
    )));
    assert!(matches!(outcome, CommandOutcome::Accepted(_)));

    let snapshot = match session.query(DesignerQuery::Snapshot) {
        DesignerQueryResult::Snapshot(snapshot) => snapshot,
        other => panic!("expected snapshot, got {other:?}"),
    };
    assert_eq!(
        snapshot.design.nodes[&node_id("badge")].source,
        studio_design::DesignNodeSource::Primitive {
            kind: NodeKind::Badge
        }
    );
    assert_eq!(snapshot.design.nodes[&node_id("badge")].layout, layout);
    assert!(
        snapshot.design.nodes[&node_id("stack")]
            .children
            .contains(&node_id("media"))
    );

    let undo = block_on(session.undo(studio_design::HistoryOperation {
        operation_id: operation_id("undo-badge"),
        actor: actor(),
        base_revision: RevisionId::new(1),
    }));
    assert!(matches!(undo, CommandOutcome::Accepted(_)));
    assert_eq!(snapshot_layout(&session, "badge").placement, None);
}

#[test]
fn invalid_overlay_parent_and_constraints_are_explained_at_the_node() {
    let mut design = stack_design();
    design.nodes.get_mut(&node_id("stack")).unwrap().source =
        studio_design::DesignNodeSource::Primitive {
            kind: NodeKind::Row,
        };
    let mut session = created(design);
    let mut invalid_overlay = LayoutProperties::absolute();
    invalid_overlay.position = Some(LayoutPosition {
        left: Some(px("4")),
        ..Default::default()
    });
    let outcome = block_on(session.submit(batch(
        RevisionId::INITIAL,
        "invalid-overlay",
        vec![Command::set_layout(node_id("badge"), invalid_overlay)],
    )));
    assert_diagnostic(outcome, "DESIGN_LAYOUT_OVERLAY_PARENT_INVALID", "badge");
    assert!(match session.query(DesignerQuery::DiagnosticsForNode {
        node_id: node_id("badge"),
    }) {
        DesignerQueryResult::Diagnostics(diagnostics) => diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "DESIGN_LAYOUT_OVERLAY_PARENT_INVALID" }),
        other => panic!("expected diagnostics, got {other:?}"),
    });

    let invalid_constraints = LayoutProperties {
        min_width: Some(px("300")),
        max_width: Some(px("100")),
        ..Default::default()
    };
    let outcome = block_on(session.submit(batch(
        RevisionId::INITIAL,
        "invalid-constraints",
        vec![Command::set_layout(node_id("media"), invalid_constraints)],
    )));
    assert_diagnostic(outcome, "DESIGN_LAYOUT_CONSTRAINT_INVALID", "media");
}

#[test]
#[allow(clippy::too_many_lines)]
fn responsive_constraint_overrides_merge_deterministically_across_profile_switches() {
    let mut design = stack_design();
    let phone = variant_id("phone");
    let desktop = variant_id("desktop");
    for (id, name, minimum_width) in [
        (&phone, "Phone", Some(0)),
        (&desktop, "Desktop", Some(1024)),
    ] {
        design.responsive_variants.insert(
            id.clone(),
            ResponsiveVariant {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                id: id.clone(),
                name: name.to_owned(),
                minimum_width,
                maximum_width: None,
                input: InputEnvironment::Any,
            },
        );
    }
    let persistence = InMemoryDesignerPersistence::default();
    let mut session = block_on(DefaultDesignerSession::create(
        persistence,
        design,
        operation_id("create-responsive"),
        actor(),
        undo_group_id("create"),
    ))
    .unwrap();

    let base = LayoutProperties {
        min_width: Some(px("160")),
        max_width: Some(px("1200")),
        ..Default::default()
    };
    assert!(matches!(
        block_on(session.submit(batch(
            RevisionId::INITIAL,
            "base-constraints",
            vec![Command::set_layout(node_id("media"), base)],
        ))),
        CommandOutcome::Accepted(_)
    ));

    let phone_layout = LayoutProperties {
        max_width: Some(px("480")),
        ..Default::default()
    };
    assert!(matches!(
        block_on(session.submit(batch(
            RevisionId::new(1),
            "phone-constraints",
            vec![Command::set_responsive_layout(
                node_id("media"),
                phone.clone(),
                phone_layout
            )],
        ))),
        CommandOutcome::Accepted(_)
    ));

    assert!(matches!(
        block_on(session.undo(studio_design::HistoryOperation {
            operation_id: operation_id("undo-phone"),
            actor: actor(),
            base_revision: RevisionId::new(2),
        })),
        CommandOutcome::Accepted(_)
    ));
    assert!(match session.query(DesignerQuery::Snapshot) {
        DesignerQueryResult::Snapshot(snapshot) => {
            !snapshot.design.nodes[&node_id("media")]
                .responsive_overrides
                .contains_key(&phone)
        }
        other => panic!("expected snapshot, got {other:?}"),
    });
    assert!(matches!(
        block_on(session.redo(studio_design::HistoryOperation {
            operation_id: operation_id("redo-phone"),
            actor: actor(),
            base_revision: RevisionId::new(3),
        })),
        CommandOutcome::Accepted(_)
    ));

    let revision_before_switch = snapshot_revision(&session);
    let phone_node = node_for_profile(&session, "media", Some("phone"));
    assert_eq!(phone_node.layout.min_width, Some(px("160")));
    assert_eq!(phone_node.layout.max_width, Some(px("480")));
    session.update_context(studio_design::SessionContextUpdate {
        device_profile: Some(Some("Desktop".to_owned())),
        ..Default::default()
    });
    let desktop_node = node_for_profile(&session, "media", Some("Desktop"));
    assert_eq!(desktop_node.layout.min_width, Some(px("160")));
    assert_eq!(desktop_node.layout.max_width, Some(px("1200")));
    session.update_context(studio_design::SessionContextUpdate {
        device_profile: Some(Some("phone".to_owned())),
        ..Default::default()
    });
    assert_eq!(
        node_for_profile(&session, "media", Some("phone")).layout,
        phone_node.layout
    );
    assert_eq!(snapshot_revision(&session), revision_before_switch);
}

fn snapshot_layout(session: &impl DesignerSession, id: &str) -> LayoutProperties {
    match session.query(DesignerQuery::Node {
        node_id: node_id(id),
    }) {
        DesignerQueryResult::Node(Some(node)) => node.layout,
        other => panic!("expected node, got {other:?}"),
    }
}

fn node_for_profile(
    session: &impl DesignerSession,
    node: &str,
    profile: Option<&str>,
) -> DesignNode {
    match session.query(DesignerQuery::NodeForProfile {
        node_id: node_id(node),
        profile: profile.map(ToOwned::to_owned),
    }) {
        DesignerQueryResult::Node(Some(node)) => node,
        other => panic!("expected profiled node, got {other:?}"),
    }
}

fn snapshot_revision(session: &impl DesignerSession) -> RevisionId {
    match session.query(DesignerQuery::Snapshot) {
        DesignerQueryResult::Snapshot(snapshot) => snapshot.revision.id,
        other => panic!("expected snapshot, got {other:?}"),
    }
}

fn assert_diagnostic(outcome: CommandOutcome, code: &str, node: &str) {
    match outcome {
        CommandOutcome::Rejected(diagnostics) => assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == code
                && diagnostic
                    .node_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == node)
        })),
        other => panic!("expected rejected command, got {other:?}"),
    }
}
