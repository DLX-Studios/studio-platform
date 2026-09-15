//! Child process that is killed after a Designer revision reaches its durable point.

use std::{env, path::PathBuf, thread};

use studio_design::{
    Actor, ActorId, ActorKind, Command, CommandBatch, CommandOutcome, DefaultDesignerSession,
    DesignNode, DesignerSession, NodeId, NodeKind, NodeParent, OperationId, ProjectId,
    PropertyValue, RevisionId, STUDIO_DESIGN_SCHEMA_VERSION, Screen, ScreenId, StudioDesign,
    UndoGroupId,
};
use studio_host::{Durability, EmbeddedLocalStore, LocalStoreDesignerPersistence};

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

fn main() {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let Some(directory) = arguments.next() else {
        std::process::exit(2);
    };
    if arguments.next().is_some() {
        std::process::exit(2);
    }
    let directory = PathBuf::from(directory);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("worker runtime starts");
    runtime.block_on(async move {
        let store = EmbeddedLocalStore::open(&directory, Durability::Every)
            .await
            .expect("worker opens LocalStore");
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
        .expect("worker creates Designer session");
        let outcome = session
            .submit(CommandBatch {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                operation_id: operation_id("durable-edit"),
                actor: actor(),
                project_id: project_id(),
                base_revision: RevisionId::INITIAL,
                undo_group_id: undo_group_id("edit-text"),
                undo_group_name: "Edit text".to_owned(),
                preconditions: Vec::new(),
                commands: vec![Command::SetProperty {
                    node_id: node_id("item"),
                    property: "text".to_owned(),
                    value: Some(PropertyValue::String("accepted-before-kill".to_owned())),
                }],
            })
            .await;
        assert!(matches!(outcome, CommandOutcome::Accepted(_)));
        std::fs::write(directory.join(CRASH_MARKER_FILE), b"durable")
            .expect("worker signals durable point");
        loop {
            thread::park();
        }
    });
}
