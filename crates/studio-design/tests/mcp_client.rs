#![allow(missing_docs)]

use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

use studio_design::{
    access::ScopedDesignerAccess, Actor, ActorId, ActorKind, Command, CommandBatch, CommandOutcome,
    DefaultDesignerSession, DesignNode, DesignerQuery, DesignerQueryResult, DesignerScope,
    HistoryOperation, InMemoryDesignerPersistence, McpClient, NodeId, NodeParent, OperationId,
    ProjectId, RevisionId, Screen, ScreenId, StudioDesign, UndoGroupId,
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
    ProjectId::new("mcp-project").unwrap()
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

fn human_actor() -> Actor {
    Actor {
        id: ActorId::new("wire-human").unwrap(),
        kind: ActorKind::Human,
        display_name: "Wire actor".to_owned(),
    }
}

fn agent_actor() -> Actor {
    Actor {
        id: ActorId::new("agent-1").unwrap(),
        kind: ActorKind::Agent,
        display_name: "Agent".to_owned(),
    }
}

fn mcp_actor() -> Actor {
    Actor {
        id: ActorId::new("mcp-script-1").unwrap(),
        kind: ActorKind::Mcp,
        display_name: "Scripted MCP".to_owned(),
    }
}

fn seed_design() -> StudioDesign {
    let screen_id = ScreenId::new("screen-main").unwrap();
    let root_id = node_id("root");
    let left_id = node_id("left");
    let right_id = node_id("right");
    let mut root = DesignNode::primitive(root_id.clone(), "Root", NodeKind::Box);
    root.children = vec![left_id.clone(), right_id.clone()];
    let left = DesignNode::primitive(left_id.clone(), "Left", NodeKind::Text);
    let right = DesignNode::primitive(right_id.clone(), "Right", NodeKind::Text);

    let mut design = StudioDesign::empty(project_id(), "MCP parity");
    design.nodes = [
        (root_id.clone(), root),
        (left_id.clone(), left),
        (right_id.clone(), right),
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
            left_id,
            NodeParent::Node {
                node_id: root_id.clone(),
            },
        ),
        (right_id, NodeParent::Node { node_id: root_id }),
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

fn batch(base_revision: RevisionId, operation: &str, node: &str, name: &str) -> CommandBatch {
    CommandBatch {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        operation_id: operation_id(operation),
        actor: human_actor(),
        project_id: project_id(),
        base_revision,
        undo_group_id: undo_group_id(operation),
        undo_group_name: operation.to_owned(),
        preconditions: Vec::new(),
        commands: vec![Command::RenameNode {
            node_id: node_id(node),
            name: name.to_owned(),
        }],
    }
}

async fn create_session(
    persistence: InMemoryDesignerPersistence,
) -> DefaultDesignerSession<InMemoryDesignerPersistence> {
    DefaultDesignerSession::create(
        persistence,
        seed_design(),
        operation_id("create"),
        human_actor(),
        undo_group_id("create"),
    )
    .await
    .expect("seed design is valid")
}

fn full_scope() -> DesignerScope {
    DesignerScope::for_project(project_id())
        .allow_project_read()
        .allow_selection_read()
        .allow_schemas_read()
        .allow_diagnostics_read()
        .allow_history_read()
        .allow_command_write()
        .allow_history_write()
}

fn subtree_scope() -> DesignerScope {
    DesignerScope::for_project(project_id())
        .allow_subtree_read(node_id("left"))
        .allow_subtree_write(node_id("left"))
}

#[test]
fn scripted_mcp_edit_has_idempotent_receipt_and_mcp_audit_actor() {
    block_on(async {
        let persistence = InMemoryDesignerPersistence::default();
        let session = create_session(persistence).await;
        let mut client = McpClient::connect(session, full_scope(), mcp_actor()).unwrap();

        let request = batch(RevisionId::INITIAL, "rename-left", "left", "Renamed by MCP");
        let first = client.submit(request.clone()).await.unwrap();
        let receipt = match first {
            CommandOutcome::Accepted(receipt) => receipt,
            other => panic!("expected accepted MCP command, got {other:?}"),
        };
        assert_eq!(receipt.committed_revision, RevisionId::new(1));
        assert_eq!(receipt.actor, mcp_actor());

        let replay = client.submit(request).await.unwrap();
        assert_eq!(replay, CommandOutcome::Accepted(receipt.clone()));

        let snapshot = client
            .query(DesignerQuery::Snapshot)
            .expect("project read is granted");
        let DesignerQueryResult::Snapshot(snapshot) = snapshot else {
            panic!("expected snapshot")
        };
        assert_eq!(
            snapshot.design.nodes[&node_id("left")].name,
            "Renamed by MCP"
        );
        assert_eq!(snapshot.revision.actor.kind, ActorKind::Mcp);
    });
}

#[test]
fn mcp_and_agent_scoped_adapters_return_identical_scope_denials() {
    block_on(async {
        let mcp_session = create_session(InMemoryDesignerPersistence::default()).await;
        let agent_session = create_session(InMemoryDesignerPersistence::default()).await;
        let mut mcp = McpClient::connect(mcp_session, subtree_scope(), mcp_actor()).unwrap();
        let mut agent = studio_design::ScopedDesignerSession::new(agent_session, subtree_scope());

        let mcp_query = mcp
            .query(DesignerQuery::Node {
                node_id: node_id("right"),
            })
            .unwrap_err();
        let agent_query = agent
            .query_scoped(DesignerQuery::Node {
                node_id: node_id("right"),
            })
            .unwrap_err();
        assert_eq!(mcp_query, agent_query);
        assert_eq!(mcp_query.code, "DESIGN_SCOPE_DENIED");

        let mcp_command = mcp
            .submit(batch(RevisionId::INITIAL, "mcp-denied", "right", "Nope"))
            .await
            .unwrap_err();
        let agent_command = agent
            .submit_scoped(
                agent_actor(),
                batch(RevisionId::INITIAL, "mcp-denied", "right", "Nope"),
            )
            .await
            .unwrap_err();
        assert_eq!(mcp_command.code, "DESIGN_SCOPE_DENIED");
        assert_eq!(mcp_command.operation, agent_command.operation);
        assert_eq!(mcp_command.required, agent_command.required);

        assert!(mcp.query(DesignerQuery::Diagnostics).is_err());
        let snapshot = mcp.query(DesignerQuery::Node {
            node_id: node_id("left"),
        });
        assert!(snapshot.is_ok(), "the granted subtree remains readable");
    });
}

#[test]
fn stale_mcp_batches_preserve_the_shared_conflict_result() {
    block_on(async {
        let session = create_session(InMemoryDesignerPersistence::default()).await;
        let mut client = McpClient::connect(session, full_scope(), mcp_actor()).unwrap();
        assert!(matches!(
            client
                .submit(batch(RevisionId::INITIAL, "first", "left", "Current"))
                .await
                .unwrap(),
            CommandOutcome::Accepted(_)
        ));

        let stale = client
            .submit(batch(RevisionId::INITIAL, "stale", "right", "Stale"))
            .await
            .unwrap();
        let CommandOutcome::Conflict(conflict) = stale else {
            panic!("expected stale conflict")
        };
        assert_eq!(conflict.code, "DESIGN_STALE_REVISION");
        assert_eq!(conflict.expected_revision, RevisionId::INITIAL);
        assert_eq!(conflict.actual_revision, RevisionId::new(1));
        assert_eq!(
            client
                .query(DesignerQuery::Snapshot)
                .expect("project read")
                .into_snapshot()
                .revision
                .actor
                .kind,
            ActorKind::Mcp
        );
    });
}

trait IntoSnapshot {
    fn into_snapshot(self) -> studio_design::StudioDesignSnapshot;
}

impl IntoSnapshot for DesignerQueryResult {
    fn into_snapshot(self) -> studio_design::StudioDesignSnapshot {
        match self {
            DesignerQueryResult::Snapshot(snapshot) => snapshot,
            other => panic!("expected snapshot, got {other:?}"),
        }
    }
}

#[test]
fn mcp_history_uses_the_same_engine_and_mcp_audit_kind() {
    block_on(async {
        let session = create_session(InMemoryDesignerPersistence::default()).await;
        let mut client = McpClient::connect(session, full_scope(), mcp_actor()).unwrap();
        assert!(matches!(
            client
                .submit(batch(RevisionId::INITIAL, "history-edit", "left", "Edited"))
                .await
                .unwrap(),
            CommandOutcome::Accepted(_)
        ));

        let undo = client
            .undo(HistoryOperation {
                operation_id: operation_id("mcp-undo"),
                actor: human_actor(),
                base_revision: RevisionId::new(1),
            })
            .await
            .unwrap();
        let CommandOutcome::Accepted(undo) = undo else {
            panic!("expected accepted undo")
        };
        assert_eq!(undo.actor.kind, ActorKind::Mcp);
        assert_eq!(undo.committed_revision, RevisionId::new(2));

        let redo = client
            .redo(HistoryOperation {
                operation_id: operation_id("mcp-redo"),
                actor: human_actor(),
                base_revision: RevisionId::new(2),
            })
            .await
            .unwrap();
        let CommandOutcome::Accepted(redo) = redo else {
            panic!("expected accepted redo")
        };
        assert_eq!(redo.actor.kind, ActorKind::Mcp);
        assert_eq!(redo.committed_revision, RevisionId::new(3));
    });
}
