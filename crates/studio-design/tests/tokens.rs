#![allow(missing_docs)]

use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

use studio_design::{
    Actor, ActorId, ActorKind, Command, CommandBatch, CommandOutcome, DefaultDesignerSession,
    DesignNode, DesignToken, DesignerQuery, DesignerQueryResult, DesignerSession,
    InMemoryDesignerPersistence, InspectedTokenValue, Length, LengthUnit, NodeParent, OperationId,
    ProjectId, PropertyValue, RevisionId, STUDIO_DESIGN_SCHEMA_VERSION, Screen, ScreenId,
    StudioDesign, TokenId, TokenKind, TokenValue, UndoGroupId,
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

fn project() -> ProjectId {
    ProjectId::new("token-project").unwrap()
}
fn operation(value: &str) -> OperationId {
    OperationId::new(value).unwrap()
}
fn undo(value: &str) -> UndoGroupId {
    UndoGroupId::new(value).unwrap()
}
fn actor() -> Actor {
    Actor {
        id: ActorId::new("token-test").unwrap(),
        kind: ActorKind::Human,
        display_name: "Token test".to_owned(),
    }
}
fn token_id(value: &str) -> TokenId {
    TokenId::new(value).unwrap()
}
fn node_id(value: &str) -> studio_design::NodeId {
    studio_design::NodeId::new(value).unwrap()
}
fn batch(base_revision: RevisionId, operation_id: &str, commands: Vec<Command>) -> CommandBatch {
    CommandBatch {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        operation_id: operation(operation_id),
        actor: actor(),
        project_id: project(),
        base_revision,
        undo_group_id: undo(operation_id),
        undo_group_name: operation_id.to_owned(),
        preconditions: Vec::new(),
        commands,
    }
}

fn design() -> StudioDesign {
    let screen_id = ScreenId::new("main").unwrap();
    let root_id = node_id("root");
    let child_id = node_id("child");
    let mut root = DesignNode::primitive(root_id.clone(), "Root", NodeKind::Box);
    root.children.push(child_id.clone());
    let child = DesignNode::primitive(child_id.clone(), "Child", NodeKind::Text);
    let mut design = StudioDesign::empty(project(), "Tokens");
    design.nodes = [(root_id.clone(), root), (child_id.clone(), child)]
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
            child_id,
            NodeParent::Node {
                node_id: root_id.clone(),
            },
        ),
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
            root_node_id: root_id,
        },
    );
    design.screen_order.push(screen_id);
    design
}

fn token() -> DesignToken {
    DesignToken {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        id: token_id("space-md"),
        name: "Space / Medium".to_owned(),
        kind: TokenKind::Spacing,
        value: TokenValue::Length(Length {
            value: "16".to_owned(),
            unit: LengthUnit::Pixels,
        }),
    }
}

fn snapshot(session: &impl DesignerSession) -> studio_design::StudioDesignSnapshot {
    match session.query(DesignerQuery::Snapshot) {
        DesignerQueryResult::Snapshot(snapshot) => snapshot,
        result => panic!("expected snapshot, got {result:?}"),
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn token_apply_override_clear_edit_and_inspector_preserve_shared_intent() {
    block_on(async {
        let mut session = DefaultDesignerSession::create(
            InMemoryDesignerPersistence::default(),
            design(),
            operation("create"),
            actor(),
            undo("create"),
        )
        .await
        .unwrap();

        assert!(matches!(
            session
                .submit(batch(
                    RevisionId::INITIAL,
                    "create-token",
                    vec![Command::CreateToken {
                        token: Box::new(token())
                    }]
                ))
                .await,
            CommandOutcome::Accepted(_)
        ));
        assert!(matches!(
            session
                .submit(batch(
                    RevisionId::new(1),
                    "apply-token",
                    vec![Command::ApplyToken {
                        node_id: node_id("child"),
                        property: "gap".to_owned(),
                        token_id: token_id("space-md")
                    }]
                ))
                .await,
            CommandOutcome::Accepted(_)
        ));
        assert!(matches!(
            session
                .submit(batch(
                    RevisionId::new(2),
                    "override-token",
                    vec![Command::OverrideToken {
                        node_id: node_id("child"),
                        property: "gap".to_owned(),
                        value: TokenValue::Length(Length {
                            value: "24".to_owned(),
                            unit: LengthUnit::Pixels
                        })
                    }]
                ))
                .await,
            CommandOutcome::Accepted(_)
        ));

        let inspected = match session.query(DesignerQuery::NodeTokenValues {
            node_id: node_id("child"),
        }) {
            DesignerQueryResult::NodeTokenValues(values) => values,
            result => panic!("expected token inspector, got {result:?}"),
        };
        assert_eq!(
            inspected,
            vec![InspectedTokenValue {
                property: "gap".to_owned(),
                token_id: token_id("space-md"),
                shared_value: Some(token().value),
                local_value: Some(TokenValue::Length(Length {
                    value: "24".to_owned(),
                    unit: LengthUnit::Pixels
                })),
            }]
        );

        assert!(matches!(
            session
                .submit(batch(
                    RevisionId::new(3),
                    "edit-token",
                    vec![Command::EditToken {
                        token_id: token_id("space-md"),
                        value: TokenValue::Length(Length {
                            value: "20".to_owned(),
                            unit: LengthUnit::Pixels
                        })
                    }]
                ))
                .await,
            CommandOutcome::Accepted(_)
        ));
        let DesignerQueryResult::NodeTokenValues(inspected) =
            session.query(DesignerQuery::NodeTokenValues {
                node_id: node_id("child"),
            })
        else {
            unreachable!()
        };
        assert_eq!(
            inspected[0].shared_value,
            Some(TokenValue::Length(Length {
                value: "20".to_owned(),
                unit: LengthUnit::Pixels
            }))
        );
        assert_eq!(
            inspected[0].local_value,
            Some(TokenValue::Length(Length {
                value: "24".to_owned(),
                unit: LengthUnit::Pixels
            }))
        );

        assert!(matches!(
            session
                .submit(batch(
                    RevisionId::new(4),
                    "clear-token",
                    vec![Command::ClearTokenOverride {
                        node_id: node_id("child"),
                        property: "gap".to_owned()
                    }]
                ))
                .await,
            CommandOutcome::Accepted(_)
        ));
        let DesignerQueryResult::NodeTokenValues(inspected) =
            session.query(DesignerQuery::NodeTokenValues {
                node_id: node_id("child"),
            })
        else {
            unreachable!()
        };
        assert_eq!(inspected[0].local_value, None);
        assert_eq!(
            snapshot(&session).design.nodes[&node_id("child")].properties["gap"],
            PropertyValue::Token(token_id("space-md"))
        );
    });
}

#[test]
fn rename_keeps_identity_and_delete_lists_usages_before_confirmation() {
    block_on(async {
        let mut session = DefaultDesignerSession::create(
            InMemoryDesignerPersistence::default(),
            design(),
            operation("create"),
            actor(),
            undo("create"),
        )
        .await
        .unwrap();
        for (revision, operation_id, commands) in [
            (
                RevisionId::INITIAL,
                "create-token",
                vec![Command::CreateToken {
                    token: Box::new(token()),
                }],
            ),
            (
                RevisionId::new(1),
                "apply-token",
                vec![Command::ApplyToken {
                    node_id: node_id("child"),
                    property: "gap".to_owned(),
                    token_id: token_id("space-md"),
                }],
            ),
            (
                RevisionId::new(2),
                "rename-token",
                vec![Command::RenameToken {
                    token_id: token_id("space-md"),
                    name: "Spacing / Medium".to_owned(),
                }],
            ),
        ] {
            assert!(matches!(
                session
                    .submit(batch(revision, operation_id, commands))
                    .await,
                CommandOutcome::Accepted(_)
            ));
        }
        let DesignerQueryResult::TokenUsages(usages) = session.query(DesignerQuery::TokenUsages {
            token_id: token_id("space-md"),
        }) else {
            unreachable!()
        };
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].property, "gap");
        assert_eq!(
            snapshot(&session).design.tokens[&token_id("space-md")].name,
            "Spacing / Medium"
        );

        let rejected = session
            .submit(batch(
                RevisionId::new(3),
                "delete-token",
                vec![Command::DeleteToken {
                    token_id: token_id("space-md"),
                    confirm: false,
                }],
            ))
            .await;
        match rejected {
            CommandOutcome::Rejected(diagnostics) => {
                assert_eq!(
                    diagnostics[0].code,
                    "DESIGN_TOKEN_DELETE_CONFIRMATION_REQUIRED"
                );
                assert!(diagnostics[0].message.contains("node:child:gap"));
            }
            other => panic!("expected confirmation rejection, got {other:?}"),
        }
        assert!(matches!(
            session
                .submit(batch(
                    RevisionId::new(3),
                    "delete-token-confirmed",
                    vec![Command::DeleteToken {
                        token_id: token_id("space-md"),
                        confirm: true
                    }]
                ))
                .await,
            CommandOutcome::Accepted(_)
        ));
        assert!(
            !snapshot(&session)
                .design
                .tokens
                .contains_key(&token_id("space-md"))
        );
    });
}
