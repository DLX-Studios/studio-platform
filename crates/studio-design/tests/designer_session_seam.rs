#![allow(missing_docs)]

use std::{
    collections::BTreeMap,
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

use studio_design::{
    Actor, ActorId, ActorKind, Command, CommandBatch, CommandOutcome, CommandPrecondition,
    DefaultDesignerSession, DesignNode, DesignerQuery, DesignerQueryResult, DesignerSession,
    HistoryOperation, InMemoryDesignerPersistence, Interaction, InteractionAction,
    InteractionEvent, InteractionId, InteractionSource, NodeId, NodeParent, OperationId,
    ParentPlacement, ProjectId, PropertyValue, RevisionId, STUDIO_DESIGN_SCHEMA_VERSION, Screen,
    ScreenId, SelectionSnapshot, SessionContextUpdate, StudioDesign, StudioDesignSnapshot,
    UndoGroupId,
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

fn project_id() -> ProjectId {
    ProjectId::new("project-restaurant").unwrap()
}

fn node_id(value: &str) -> NodeId {
    NodeId::new(value).unwrap()
}

fn operation_id(value: &str) -> OperationId {
    OperationId::new(value).unwrap()
}

fn undo_group_id(value: &str) -> UndoGroupId {
    UndoGroupId::new(value).unwrap()
}

fn actor() -> Actor {
    Actor {
        id: ActorId::new("actor-user").unwrap(),
        kind: ActorKind::Human,
        display_name: "Designer".to_owned(),
    }
}

fn seed_design() -> StudioDesign {
    let screen_id = ScreenId::new("screen-main").unwrap();
    let root_id = node_id("root");
    let left_id = node_id("left");
    let right_id = node_id("right");
    let item_id = node_id("item");
    let sibling_id = node_id("sibling");

    let mut root = DesignNode::primitive(root_id.clone(), "Root", NodeKind::Box);
    root.children = vec![left_id.clone(), right_id.clone()];
    let mut left = DesignNode::primitive(left_id.clone(), "Left", NodeKind::Column);
    left.children = vec![item_id.clone()];
    let mut right = DesignNode::primitive(right_id.clone(), "Right", NodeKind::Column);
    right.children = vec![sibling_id.clone()];
    let item = DesignNode::primitive(item_id.clone(), "Item", NodeKind::Text);
    let sibling = DesignNode::primitive(sibling_id.clone(), "Sibling", NodeKind::Text);

    let mut design = StudioDesign::empty(project_id(), "Restaurant");
    design.nodes = [
        (root_id.clone(), root),
        (left_id.clone(), left),
        (right_id.clone(), right),
        (item_id.clone(), item),
        (sibling_id.clone(), sibling),
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
            left_id.clone(),
            NodeParent::Node {
                node_id: root_id.clone(),
            },
        ),
        (right_id.clone(), NodeParent::Node { node_id: root_id }),
        (item_id, NodeParent::Node { node_id: left_id }),
        (sibling_id, NodeParent::Node { node_id: right_id }),
    ]
    .into_iter()
    .collect();
    design.screens.insert(
        screen_id.clone(),
        Screen {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: screen_id.clone(),
            name: "Main".to_owned(),
            route: "/".to_owned(),
            root_node_id: node_id("root"),
        },
    );
    design.screen_order.push(screen_id);
    design
}

fn batch(
    base_revision: RevisionId,
    operation: &str,
    group: &str,
    commands: Vec<Command>,
) -> CommandBatch {
    CommandBatch {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        operation_id: operation_id(operation),
        actor: actor(),
        project_id: project_id(),
        base_revision,
        undo_group_id: undo_group_id(group),
        undo_group_name: group.replace('-', " "),
        preconditions: Vec::new(),
        commands,
    }
}

fn snapshot(session: &impl DesignerSession) -> StudioDesignSnapshot {
    match session.query(DesignerQuery::Snapshot) {
        DesignerQueryResult::Snapshot(snapshot) => snapshot,
        other => panic!("expected snapshot, got {other:?}"),
    }
}

fn assert_accepted(outcome: CommandOutcome) -> studio_design::CommandReceipt {
    match outcome {
        CommandOutcome::Accepted(receipt) => receipt,
        other => panic!("expected accepted command, got {other:?}"),
    }
}

async fn created(
    persistence: InMemoryDesignerPersistence,
) -> DefaultDesignerSession<InMemoryDesignerPersistence> {
    DefaultDesignerSession::create(
        persistence,
        seed_design(),
        operation_id("create"),
        actor(),
        undo_group_id("create-project"),
    )
    .await
    .expect("valid project creates")
}

#[test]
fn create_edit_undo_redo_and_reopen_use_only_the_public_seam() {
    block_on(async {
        let persistence = InMemoryDesignerPersistence::default();
        let mut session = created(persistence.clone()).await;
        assert_eq!(snapshot(&session).revision.id, RevisionId::INITIAL);

        let inserted_id = node_id("inserted");
        let receipt = assert_accepted(
            session
                .submit(batch(
                    RevisionId::INITIAL,
                    "insert",
                    "insert-card",
                    vec![Command::InsertNode {
                        parent: ParentPlacement {
                            parent: NodeParent::Node {
                                node_id: node_id("right"),
                            },
                            index: 1,
                        },
                        node: Box::new(DesignNode::primitive(
                            inserted_id.clone(),
                            "Inserted",
                            NodeKind::Card,
                        )),
                    }],
                ))
                .await,
        );
        assert_eq!(receipt.committed_revision, RevisionId::new(1));
        assert!(snapshot(&session).design.nodes.contains_key(&inserted_id));

        let undo = assert_accepted(
            session
                .undo(HistoryOperation {
                    operation_id: operation_id("undo-insert"),
                    actor: actor(),
                    base_revision: RevisionId::new(1),
                })
                .await,
        );
        assert_eq!(undo.committed_revision, RevisionId::new(2));
        assert!(!snapshot(&session).design.nodes.contains_key(&inserted_id));

        let redo = assert_accepted(
            session
                .redo(HistoryOperation {
                    operation_id: operation_id("redo-insert"),
                    actor: actor(),
                    base_revision: RevisionId::new(2),
                })
                .await,
        );
        assert_eq!(redo.committed_revision, RevisionId::new(3));
        assert!(snapshot(&session).design.nodes.contains_key(&inserted_id));

        drop(session);
        let reopened = DefaultDesignerSession::open(persistence, &project_id())
            .await
            .expect("last durable revision reopens");
        let reopened_snapshot = snapshot(&reopened);
        assert_eq!(reopened_snapshot.revision.id, RevisionId::new(3));
        assert!(reopened_snapshot.design.nodes.contains_key(&inserted_id));
    });
}

#[test]
fn invalid_batch_rolls_back_every_command_and_creates_no_revision() {
    block_on(async {
        let persistence = InMemoryDesignerPersistence::default();
        let mut session = created(persistence.clone()).await;
        let item_id = node_id("item");
        let outcome = session
            .submit(batch(
                RevisionId::INITIAL,
                "invalid-atomic",
                "atomic-edit",
                vec![
                    Command::SetProperty {
                        node_id: item_id.clone(),
                        property: "text".to_owned(),
                        value: Some(PropertyValue::String("must roll back".to_owned())),
                    },
                    Command::MoveNode {
                        node_id: node_id("missing"),
                        destination: ParentPlacement {
                            parent: NodeParent::Node {
                                node_id: node_id("right"),
                            },
                            index: 0,
                        },
                    },
                ],
            ))
            .await;
        assert!(matches!(outcome, CommandOutcome::Rejected(_)));
        let current = snapshot(&session);
        assert_eq!(current.revision.id, RevisionId::INITIAL);
        assert!(current.design.nodes[&item_id].properties.is_empty());

        let reopened = DefaultDesignerSession::open(persistence, &project_id())
            .await
            .expect("initial durable revision remains");
        assert_eq!(snapshot(&reopened).revision.id, RevisionId::INITIAL);
    });
}

#[test]
fn rename_move_reorder_and_property_edits_preserve_node_identity() {
    block_on(async {
        let mut session = created(InMemoryDesignerPersistence::default()).await;
        let item_id = node_id("item");
        let state = session.update_context(SessionContextUpdate {
            selection: Some(SelectionSnapshot {
                node_ids: vec![item_id.clone()],
                primary: Some(item_id.clone()),
            }),
            ..SessionContextUpdate::default()
        });
        assert_eq!(state.selection.primary, Some(item_id.clone()));

        assert_accepted(
            session
                .submit(batch(
                    RevisionId::INITIAL,
                    "identity-edits",
                    "move-and-style",
                    vec![
                        Command::RenameNode {
                            node_id: item_id.clone(),
                            name: "Renamed item".to_owned(),
                        },
                        Command::MoveNode {
                            node_id: item_id.clone(),
                            destination: ParentPlacement {
                                parent: NodeParent::Node {
                                    node_id: node_id("right"),
                                },
                                index: 0,
                            },
                        },
                        Command::ReorderNode {
                            node_id: item_id.clone(),
                            index: 1,
                        },
                        Command::SetProperty {
                            node_id: item_id.clone(),
                            property: "typography_role".to_owned(),
                            value: Some(PropertyValue::String("headline".to_owned())),
                        },
                    ],
                ))
                .await,
        );

        let current = snapshot(&session);
        assert_eq!(current.design.nodes[&item_id].id, item_id);
        assert_eq!(current.design.nodes[&node_id("right")].children[1], item_id);
        assert_eq!(
            current.design.parents[&item_id],
            NodeParent::Node {
                node_id: node_id("right")
            }
        );
        assert_eq!(
            current.design.nodes[&item_id]
                .properties
                .get("typography_role"),
            Some(&PropertyValue::String("headline".to_owned()))
        );
    });
}

#[test]
fn duplicate_uses_explicit_stable_identity_map_and_round_trips_history() {
    block_on(async {
        let mut session = created(InMemoryDesignerPersistence::default()).await;
        let id_map = [
            (node_id("left"), node_id("left-copy")),
            (node_id("item"), node_id("item-copy")),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        assert_accepted(
            session
                .submit(batch(
                    RevisionId::INITIAL,
                    "duplicate",
                    "duplicate-column",
                    vec![Command::DuplicateNode {
                        source_node_id: node_id("left"),
                        destination: ParentPlacement {
                            parent: NodeParent::Node {
                                node_id: node_id("root"),
                            },
                            index: 2,
                        },
                        id_map,
                    }],
                ))
                .await,
        );
        let duplicated = snapshot(&session);
        assert_eq!(
            duplicated.design.nodes[&node_id("left-copy")].children,
            vec![node_id("item-copy")]
        );
        assert_eq!(
            duplicated.design.parents[&node_id("item-copy")],
            NodeParent::Node {
                node_id: node_id("left-copy")
            }
        );

        assert_accepted(
            session
                .undo(HistoryOperation {
                    operation_id: operation_id("undo-duplicate"),
                    actor: actor(),
                    base_revision: RevisionId::new(1),
                })
                .await,
        );
        assert!(
            !snapshot(&session)
                .design
                .nodes
                .contains_key(&node_id("left-copy"))
        );
        assert_accepted(
            session
                .redo(HistoryOperation {
                    operation_id: operation_id("redo-duplicate"),
                    actor: actor(),
                    base_revision: RevisionId::new(2),
                })
                .await,
        );
        assert!(
            snapshot(&session)
                .design
                .nodes
                .contains_key(&node_id("item-copy"))
        );
    });
}

#[test]
fn deletion_tombstone_restores_subtree_and_reports_typed_references() {
    block_on(async {
        let interaction_id = InteractionId::new("interaction-item").unwrap();
        let mut design = seed_design();
        design
            .nodes
            .get_mut(&node_id("item"))
            .unwrap()
            .interaction_ids
            .push(interaction_id.clone());
        design.interactions.insert(
            interaction_id.clone(),
            Interaction {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                id: interaction_id,
                source: InteractionSource {
                    node_id: node_id("item"),
                    event: InteractionEvent::Pressed,
                },
                action: InteractionAction::ToggleVisibility {
                    node_id: node_id("item"),
                },
            },
        );
        let persistence = InMemoryDesignerPersistence::default();
        let mut session = DefaultDesignerSession::create(
            persistence,
            design,
            operation_id("create-delete-case"),
            actor(),
            undo_group_id("create-project"),
        )
        .await
        .expect("project creates");
        session.update_context(SessionContextUpdate {
            selection: Some(SelectionSnapshot {
                node_ids: vec![node_id("item")],
                primary: Some(node_id("item")),
            }),
            ..SessionContextUpdate::default()
        });

        assert_accepted(
            session
                .submit(batch(
                    RevisionId::INITIAL,
                    "delete-item",
                    "delete-item",
                    vec![Command::DeleteNode {
                        node_id: node_id("item"),
                    }],
                ))
                .await,
        );
        let deleted = snapshot(&session);
        let tombstone = &deleted.tombstones[&node_id("item")];
        assert_eq!(tombstone.nodes.len(), 1);
        assert_eq!(tombstone.detached_index, 0);
        assert_eq!(tombstone.references.len(), 2);
        assert_eq!(tombstone.deleted_in_revision, Some(RevisionId::new(1)));
        match session.query(DesignerQuery::Diagnostics) {
            DesignerQueryResult::Diagnostics(diagnostics) => {
                assert_eq!(diagnostics.len(), 2);
                assert!(
                    diagnostics
                        .iter()
                        .all(|diagnostic| diagnostic.code == "DESIGN_REFERENCE_DELETED")
                );
            }
            other => panic!("expected diagnostics, got {other:?}"),
        }
        match session.query(DesignerQuery::SessionState) {
            DesignerQueryResult::SessionState(state) => assert!(state.selection.primary.is_none()),
            other => panic!("expected session state, got {other:?}"),
        }

        assert_accepted(
            session
                .undo(HistoryOperation {
                    operation_id: operation_id("undo-delete"),
                    actor: actor(),
                    base_revision: RevisionId::new(1),
                })
                .await,
        );
        let restored = snapshot(&session);
        assert!(restored.design.nodes.contains_key(&node_id("item")));
        assert!(restored.tombstones.is_empty());
        match session.query(DesignerQuery::Diagnostics) {
            DesignerQueryResult::Diagnostics(diagnostics) => assert!(diagnostics.is_empty()),
            other => panic!("expected diagnostics, got {other:?}"),
        }
    });
}

#[test]
fn contiguous_batches_share_one_named_undo_group() {
    block_on(async {
        let mut session = created(InMemoryDesignerPersistence::default()).await;
        assert_accepted(
            session
                .submit(batch(
                    RevisionId::INITIAL,
                    "stream-1",
                    "agent-stream",
                    vec![Command::SetProperty {
                        node_id: node_id("item"),
                        property: "text".to_owned(),
                        value: Some(PropertyValue::String("first".to_owned())),
                    }],
                ))
                .await,
        );
        assert_accepted(
            session
                .submit(batch(
                    RevisionId::new(1),
                    "stream-2",
                    "agent-stream",
                    vec![Command::RenameNode {
                        node_id: node_id("item"),
                        name: "Streamed rename".to_owned(),
                    }],
                ))
                .await,
        );
        match session.query(DesignerQuery::History) {
            DesignerQueryResult::History(history) => {
                assert_eq!(history.entries.len(), 1);
                assert_eq!(history.entries[0].batches.len(), 2);
                assert_eq!(history.cursor, 1);
            }
            other => panic!("expected history, got {other:?}"),
        }

        assert_accepted(
            session
                .undo(HistoryOperation {
                    operation_id: operation_id("undo-stream"),
                    actor: actor(),
                    base_revision: RevisionId::new(2),
                })
                .await,
        );
        let undone = snapshot(&session);
        assert_eq!(undone.design.nodes[&node_id("item")].name, "Item");
        assert!(undone.design.nodes[&node_id("item")].properties.is_empty());
    });
}

#[test]
fn stale_bases_and_failed_preconditions_return_structured_conflicts() {
    block_on(async {
        let mut session = created(InMemoryDesignerPersistence::default()).await;
        assert_accepted(
            session
                .submit(batch(
                    RevisionId::INITIAL,
                    "first",
                    "first-edit",
                    vec![Command::RenameNode {
                        node_id: node_id("item"),
                        name: "Current".to_owned(),
                    }],
                ))
                .await,
        );
        let stale = session
            .submit(batch(
                RevisionId::INITIAL,
                "stale",
                "stale-edit",
                vec![Command::RenameNode {
                    node_id: node_id("item"),
                    name: "Stale".to_owned(),
                }],
            ))
            .await;
        assert!(matches!(
            stale,
            CommandOutcome::Conflict(ref conflict)
                if conflict.code == "DESIGN_STALE_REVISION"
                    && conflict.actual_revision == RevisionId::new(1)
        ));

        let mut guarded = batch(
            RevisionId::new(1),
            "guarded",
            "guarded-edit",
            vec![Command::RenameNode {
                node_id: node_id("item"),
                name: "Guarded".to_owned(),
            }],
        );
        guarded
            .preconditions
            .push(CommandPrecondition::PropertyEquals {
                node_id: node_id("item"),
                property: "text".to_owned(),
                value: Some(PropertyValue::String("expected".to_owned())),
            });
        assert!(matches!(
            session.submit(guarded).await,
            CommandOutcome::Conflict(ref conflict)
                if conflict.code == "DESIGN_PRECONDITION_FAILED"
                    && conflict.failed_precondition == Some(0)
        ));
        assert_eq!(snapshot(&session).revision.id, RevisionId::new(1));
    });
}

#[test]
fn closed_source_schema_rejects_unknown_fields() {
    let mut encoded = serde_json::to_value(seed_design()).expect("design serializes");
    encoded
        .as_object_mut()
        .expect("design is an object")
        .insert("future_field".to_owned(), serde_json::json!(true));
    assert!(serde_json::from_value::<StudioDesign>(encoded).is_err());
}
