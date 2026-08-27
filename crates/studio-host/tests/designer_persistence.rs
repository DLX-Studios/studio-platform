#![allow(missing_docs)]

use std::{
    process::Command as ProcessCommand,
    thread,
    time::{Duration, Instant},
};

use studio_design::{
    Actor, ActorId, ActorKind, Command, CommandBatch, CommandOutcome, DefaultDesignerSession,
    DesignNode, DesignerQuery, DesignerQueryResult, DesignerSession, HistoryOperation, NodeId,
    NodeKind, NodeParent, OperationId, ProjectId, PropertyValue, RevisionId,
    STUDIO_DESIGN_SCHEMA_VERSION, Screen, ScreenId, StudioDesign, UndoGroupId,
    WORKSPACE_STATE_SCHEMA_VERSION, WorkspacePersistence, WorkspaceRecord, WorkspaceState,
};
use studio_host::{Durability, EmbeddedLocalStore, LocalStore, LocalStoreDesignerPersistence};
use tokio::runtime::Builder;

const CRASH_MARKER_FILE: &str = ".studio-designer-session-paused";

fn project_id() -> ProjectId {
    ProjectId::new("designer-persistence-project").expect("valid project identity")
}

fn node_id(value: &str) -> NodeId {
    NodeId::new(value).expect("valid node identity")
}

fn operation_id(value: &str) -> OperationId {
    OperationId::new(value).expect("valid operation identity")
}

fn undo_group_id(value: &str) -> UndoGroupId {
    UndoGroupId::new(value).expect("valid undo identity")
}

fn actor() -> Actor {
    Actor {
        id: ActorId::new("persistence-test-user").expect("valid actor identity"),
        kind: ActorKind::Human,
        display_name: "Persistence test".to_owned(),
    }
}

fn seed_design() -> StudioDesign {
    let screen_id = ScreenId::new("main-screen").expect("valid screen identity");
    let root_id = node_id("root");
    let item_id = node_id("item");
    let mut root = DesignNode::primitive(root_id.clone(), "Root", NodeKind::Box);
    root.children.push(item_id.clone());
    let item = DesignNode::primitive(item_id.clone(), "Item", NodeKind::Text);
    let mut design = StudioDesign::empty(project_id(), "Durable project");
    design
        .nodes
        .extend([(root_id.clone(), root), (item_id.clone(), item)]);
    design.parents.insert(
        root_id.clone(),
        NodeParent::Screen {
            screen_id: screen_id.clone(),
        },
    );
    design.parents.insert(
        item_id,
        NodeParent::Node {
            node_id: root_id.clone(),
        },
    );
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

fn batch(base_revision: RevisionId, operation: &str, text: &str) -> CommandBatch {
    CommandBatch {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        operation_id: operation_id(operation),
        actor: actor(),
        project_id: project_id(),
        base_revision,
        undo_group_id: undo_group_id("edit-text"),
        undo_group_name: "Edit text".to_owned(),
        preconditions: Vec::new(),
        commands: vec![Command::SetProperty {
            node_id: node_id("item"),
            property: "text".to_owned(),
            value: Some(PropertyValue::String(text.to_owned())),
        }],
    }
}

fn assert_accepted(outcome: CommandOutcome) {
    assert!(
        matches!(outcome, CommandOutcome::Accepted(_)),
        "expected an accepted command, got {outcome:?}"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn real_local_store_persists_edit_history_and_reopens_the_last_revision() {
    let directory = tempfile::tempdir().expect("temporary RocksDB directory");
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime starts")
        .block_on(async {
            let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
                .await
                .expect("LocalStore opens");
            let persistence =
                LocalStoreDesignerPersistence::new(store).expect("durable store is admitted");
            let mut session = DefaultDesignerSession::create(
                persistence,
                seed_design(),
                operation_id("create"),
                actor(),
                undo_group_id("create-project"),
            )
            .await
            .expect("Designer session creates durably");
            assert_accepted(
                session
                    .submit(batch(RevisionId::INITIAL, "edit", "durable"))
                    .await,
            );
            assert_accepted(
                session
                    .undo(HistoryOperation {
                        operation_id: operation_id("undo"),
                        actor: actor(),
                        base_revision: RevisionId::new(1),
                    })
                    .await,
            );
            assert_accepted(
                session
                    .redo(HistoryOperation {
                        operation_id: operation_id("redo"),
                        actor: actor(),
                        base_revision: RevisionId::new(2),
                    })
                    .await,
            );
            let persistence = session.into_persistence();
            let store = match persistence.try_into_store() {
                Ok(store) => store,
                Err(_) => panic!("the session owns the only adapter"),
            };
            store.close().await.expect("first store closes");

            let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
                .await
                .expect("LocalStore reopens");
            let persistence =
                LocalStoreDesignerPersistence::new(store).expect("reopened store is admitted");
            let reopened = DefaultDesignerSession::open(persistence, &project_id())
                .await
                .expect("Designer session reopens");
            match reopened.query(DesignerQuery::Snapshot) {
                DesignerQueryResult::Snapshot(snapshot) => {
                    assert_eq!(snapshot.revision.id, RevisionId::new(3));
                    assert_eq!(
                        snapshot.design.nodes[&node_id("item")]
                            .properties
                            .get("text"),
                        Some(&PropertyValue::String("durable".to_owned()))
                    );
                }
                other => panic!("expected snapshot, got {other:?}"),
            }
            match reopened.query(DesignerQuery::History) {
                DesignerQueryResult::History(history) => {
                    assert_eq!(history.entries.len(), 1);
                    assert_eq!(history.cursor, 1);
                }
                other => panic!("expected history, got {other:?}"),
            }
            let persistence = reopened.into_persistence();
            let store = match persistence.try_into_store() {
                Ok(store) => store,
                Err(_) => panic!("the reopened session owns the only adapter"),
            };
            store.close().await.expect("reopened store closes");
        });
}

#[test]
fn forced_termination_recovers_the_last_accepted_designer_revision() {
    let directory = tempfile::tempdir().expect("temporary RocksDB directory");
    let marker = directory.path().join(CRASH_MARKER_FILE);
    let mut child = ProcessCommand::new(env!("CARGO_BIN_EXE_designer_session_crash_worker"))
        .arg(directory.path())
        .spawn()
        .expect("Designer crash worker starts");

    let deadline = Instant::now() + Duration::from_secs(20);
    while !marker.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(marker.exists(), "worker reaches the accepted durable point");
    child.kill().expect("forced termination succeeds");
    child.wait().expect("worker is reaped");

    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("recovery runtime starts")
        .block_on(async {
            let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
                .await
                .expect("LocalStore recovers after termination");
            let persistence =
                LocalStoreDesignerPersistence::new(store).expect("recovered store is admitted");
            let recovered = DefaultDesignerSession::open(persistence, &project_id())
                .await
                .expect("Designer session recovers");
            match recovered.query(DesignerQuery::Snapshot) {
                DesignerQueryResult::Snapshot(snapshot) => {
                    assert_eq!(snapshot.revision.id, RevisionId::new(1));
                    assert_eq!(
                        snapshot.design.nodes[&node_id("item")]
                            .properties
                            .get("text"),
                        Some(&PropertyValue::String("accepted-before-kill".to_owned()))
                    );
                }
                other => panic!("expected snapshot, got {other:?}"),
            }
            let persistence = recovered.into_persistence();
            let store = match persistence.try_into_store() {
                Ok(store) => store,
                Err(_) => panic!("the recovered session owns the only adapter"),
            };
            store.close().await.expect("recovered store closes");
        });
}

#[test]
fn real_local_store_persists_focus_and_workbench_panel_geometry() {
    let directory = tempfile::tempdir().expect("temporary RocksDB directory");
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime starts")
        .block_on(async {
            let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
                .await
                .expect("LocalStore opens");
            let persistence =
                LocalStoreDesignerPersistence::new(store).expect("durable store is admitted");
            let mut state = WorkspaceState::new();
            state.workbench.set_geometry(
                studio_design::PanelId::Inspector,
                studio_design::PanelGeometry::new(32, 48, 600, 720),
            );
            let record = WorkspaceRecord {
                schema_version: WORKSPACE_STATE_SCHEMA_VERSION,
                project_id: project_id(),
                state,
            };
            persistence.save(&record).await.expect("workspace saves");
            let store = persistence
                .try_into_store()
                .ok()
                .expect("adapter owns the only store handle");
            store.close().await.expect("first store closes");

            let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
                .await
                .expect("LocalStore reopens");
            let persistence =
                LocalStoreDesignerPersistence::new(store).expect("reopened store is admitted");
            let reopened = persistence
                .load(&project_id())
                .await
                .expect("workspace loads")
                .expect("workspace record exists");
            assert_eq!(
                reopened
                    .state
                    .workbench
                    .panel(studio_design::PanelId::Inspector)
                    .geometry,
                studio_design::PanelGeometry::new(32, 48, 600, 720)
            );
            let store = persistence
                .try_into_store()
                .ok()
                .expect("reopened adapter owns the only store handle");
            store.close().await.expect("reopened store closes");
        });
}
