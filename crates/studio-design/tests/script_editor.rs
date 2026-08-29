#![allow(missing_docs)]

use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

use studio_design::{
    Actor, ActorId, ActorKind, Command, CommandBatch, CommandOutcome, DefaultDesignerSession,
    DesignNode, DesignerQuery, DesignerQueryResult, DesignerSession, InMemoryDesignerPersistence,
    NodeId, NodeParent, OperationId, ProjectId, RevisionId, STUDIO_DESIGN_SCHEMA_VERSION, Screen,
    ScreenId, ScriptCommitMetadata, ScriptCommitOutcome, ScriptDocumentAdapter, ScriptEdit,
    StudioDesign, UndoGroupId,
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
    ProjectId::new("editor-project").unwrap()
}

fn actor() -> Actor {
    Actor {
        id: ActorId::new("editor-user").unwrap(),
        kind: ActorKind::Human,
        display_name: "Editor".to_owned(),
    }
}

fn metadata(name: &str) -> ScriptCommitMetadata {
    ScriptCommitMetadata::new(
        OperationId::new(format!("operation-{name}")).unwrap(),
        actor(),
        UndoGroupId::new(format!("group-{name}")).unwrap(),
        name,
    )
}

fn seed() -> StudioDesign {
    let home = NodeId::new("home").unwrap();
    let title = NodeId::new("title").unwrap();
    let screen = ScreenId::new("home").unwrap();
    let mut root = DesignNode::primitive(home.clone(), "Home", NodeKind::Box);
    root.children = vec![title.clone()];
    let mut title_node = DesignNode::primitive(title.clone(), "title", NodeKind::Text);
    title_node.properties.insert(
        "text".to_owned(),
        studio_design::PropertyValue::String("Hello".to_owned()),
    );
    let mut design = StudioDesign::empty(project_id(), "Editor project");
    design.nodes.insert(home.clone(), root);
    design.nodes.insert(title.clone(), title_node);
    design.parents.insert(
        home.clone(),
        NodeParent::Screen {
            screen_id: screen.clone(),
        },
    );
    design
        .parents
        .insert(title, NodeParent::Node { node_id: home });
    design.screens.insert(
        screen.clone(),
        Screen {
            schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
            id: screen,
            name: "Home".to_owned(),
            route: "/home".to_owned(),
            root_node_id: NodeId::new("home").unwrap(),
        },
    );
    design.screen_order.push(ScreenId::new("home").unwrap());
    design
}

async fn session() -> DefaultDesignerSession<InMemoryDesignerPersistence> {
    DefaultDesignerSession::create(
        InMemoryDesignerPersistence::default(),
        seed(),
        OperationId::new("create-editor-project").unwrap(),
        actor(),
        UndoGroupId::new("create-editor-project").unwrap(),
    )
    .await
    .unwrap()
}

fn snapshot<S: studio_design::DesignerSession>(session: &S) -> studio_design::StudioDesignSnapshot {
    match session.query(DesignerQuery::Snapshot) {
        DesignerQueryResult::Snapshot(snapshot) => snapshot,
        result => panic!("expected snapshot, got {result:?}"),
    }
}

#[test]
fn valid_hand_edit_becomes_an_insert_command() {
    block_on(async {
        let mut session = session().await;
        let initial = snapshot(&session);
        let mut editor = ScriptDocumentAdapter::from_snapshot(&initial).unwrap();
        let end = editor.source().rfind("</box>").unwrap();
        editor
            .apply_edit(ScriptEdit::replace(
                end,
                end,
                "  <Button id=\"save\" label=\"Save\" />\n",
            ))
            .unwrap();

        let outcome = editor.commit(&mut session, metadata("insert-button")).await;
        let ScriptCommitOutcome::Committed { receipt, .. } = outcome else {
            panic!("expected committed editor change")
        };
        assert_eq!(receipt.base_revision, RevisionId::INITIAL);
        assert!(
            snapshot(&session)
                .design
                .nodes
                .contains_key(&NodeId::new("save").unwrap())
        );
    });
}

#[test]
fn invalid_hand_edit_only_updates_line_linked_problems() {
    block_on(async {
        let mut session = session().await;
        let initial = snapshot(&session);
        let mut editor = ScriptDocumentAdapter::from_snapshot(&initial).unwrap();
        let view = editor.replace_source("studio 1\n<Box id=\"home\">\n");
        assert!(!view.diagnostics.is_empty());
        assert!(
            view.diagnostics
                .iter()
                .all(|diagnostic| diagnostic.line() >= 2)
        );

        let outcome = editor.commit(&mut session, metadata("invalid-edit")).await;
        let ScriptCommitOutcome::Invalid { diagnostics } = outcome else {
            panic!("expected invalid editor change")
        };
        assert!(!diagnostics.is_empty());
        assert_eq!(snapshot(&session).revision.id, RevisionId::INITIAL);
    });
}

#[test]
fn canvas_refresh_keeps_comments_and_rebuilds_outline() {
    block_on(async {
        let session = session().await;
        let initial = snapshot(&session);
        let source = "studio 1\n<!-- keep this -->\n<Box id=\"home\">\n  <Text id=\"title\">Hello</Text>\n</Box>\n";
        let mut editor = ScriptDocumentAdapter::open(&initial, source);
        editor.refresh_from_snapshot(&initial).unwrap();
        assert!(editor.source().contains("<!-- keep this -->"));
        assert_eq!(editor.outline()[0].id, "home");
        assert_eq!(editor.outline()[0].children[0].id, "title");
    });
}

#[test]
fn canvas_commit_refreshes_script_buffer_to_the_new_revision() {
    block_on(async {
        let mut session = session().await;
        let initial = snapshot(&session);
        let mut editor = ScriptDocumentAdapter::from_snapshot(&initial).unwrap();
        let outcome = session
            .submit(CommandBatch {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                operation_id: OperationId::new("canvas-text-edit").unwrap(),
                actor: actor(),
                project_id: project_id(),
                base_revision: RevisionId::INITIAL,
                undo_group_id: UndoGroupId::new("canvas-edit").unwrap(),
                undo_group_name: "Canvas edit".to_owned(),
                preconditions: Vec::new(),
                commands: vec![Command::SetProperty {
                    node_id: NodeId::new("title").unwrap(),
                    property: "text".to_owned(),
                    value: Some(studio_design::PropertyValue::String(
                        "Canvas edit".to_owned(),
                    )),
                }],
            })
            .await;
        assert!(matches!(outcome, CommandOutcome::Accepted(_)));

        let updated = snapshot(&session);
        let refreshed = editor.refresh_from_snapshot(&updated).unwrap();
        assert_eq!(refreshed.base_revision, updated.revision.id);
        assert!(!refreshed.dirty);
        assert!(refreshed.source.contains("Canvas edit"));
        assert_eq!(editor.base_revision(), updated.revision.id);
    });
}
