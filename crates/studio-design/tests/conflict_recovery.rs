#![allow(missing_docs)]

use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

use studio_design::{
    Actor, ActorId, ActorKind, Command, CommandBatch, ConflictCenter, ConflictIntent,
    ConflictRecord, DesignerQuery, DesignerQueryResult, DesignerSession, InMemoryConflictPersistence,
    InMemoryDesignerPersistence, InMemoryRecoveryPersistence, JournalEntry, LogicalSnapshot,
    NodeId, NodeParent, OperationId, ProjectId, PropertyValue, RecoveryBundle, RecoveryCenter,
    RecoveryRecord, ResolutionChoice, RevisionId, RevisionMetadata, RevisionReason, Screen,
    ScreenId, StudioDesign, StudioDesignSnapshot, STUDIO_DESIGN_SCHEMA_VERSION, UndoGroupId,
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
    ProjectId::new("conflict-recovery-project").unwrap()
}

fn operation_id(value: &str) -> OperationId {
    OperationId::new(value).unwrap()
}

fn actor(name: &str) -> Actor {
    Actor {
        id: ActorId::new(format!("actor-{name}")).unwrap(),
        kind: ActorKind::Human,
        display_name: name.to_owned(),
    }
}

fn batch(operation: &str, text: &str) -> CommandBatch {
    CommandBatch {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        operation_id: operation_id(operation),
        actor: actor(operation),
        project_id: project_id(),
        base_revision: RevisionId::INITIAL,
        undo_group_id: UndoGroupId::new("recovery-edit").unwrap(),
        undo_group_name: "Recovery edit".to_owned(),
        preconditions: Vec::new(),
        commands: vec![Command::SetProperty {
            node_id: NodeId::new("item").unwrap(),
            property: "text".to_owned(),
            value: Some(PropertyValue::String(text.to_owned())),
        }],
    }
}

fn design() -> StudioDesign {
    let screen_id = ScreenId::new("main").unwrap();
    let root_id = NodeId::new("root").unwrap();
    let item_id = NodeId::new("item").unwrap();
    let mut root = studio_design::DesignNode::primitive(root_id.clone(), "Root", NodeKind::Box);
    root.children.push(item_id.clone());
    let item = studio_design::DesignNode::primitive(item_id.clone(), "Item", NodeKind::Text);
    let mut design = StudioDesign::empty(project_id(), "Recovery fixture");
    design.nodes.extend([(root_id.clone(), root), (item_id.clone(), item)]);
    design.parents.insert(
        root_id.clone(),
        NodeParent::Screen {
            screen_id: screen_id.clone(),
        },
    );
    design.parents.insert(
        item_id,
        NodeParent::Node { node_id: root_id.clone() },
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

fn initial_snapshot() -> StudioDesignSnapshot {
    StudioDesignSnapshot {
        revision: RevisionMetadata {
            id: RevisionId::INITIAL,
            parent_id: None,
            operation_id: operation_id("create"),
            actor: actor("creator"),
            undo_group_id: UndoGroupId::new("create").unwrap(),
            undo_group_name: "Create project".to_owned(),
            reason: RevisionReason::Initial,
        },
        design: design(),
        tombstones: Default::default(),
    }
}

#[test]
fn seeded_conflict_is_visible_before_open_and_keeps_both_intents() {
    block_on(async {
        let persistence = InMemoryConflictPersistence::default();
        let mut center = ConflictCenter::open(persistence.clone(), project_id()).await.unwrap();
        let local = ConflictIntent::new(batch("local", "local intent"), Some(RevisionId::new(2))).unwrap();
        let remote = ConflictIntent::new(batch("remote", "remote intent"), Some(RevisionId::new(2))).unwrap();
        let conflict = ConflictRecord::new(project_id(), 42, local.clone(), remote.clone()).unwrap();
        let conflict_id = conflict.conflict_id.clone();
        center.record(conflict).await.unwrap();

        let reopened = ConflictCenter::open(persistence, project_id()).await.unwrap();
        assert_eq!(reopened.pending().len(), 1);
        let visible = &reopened.records()[0];
        assert_eq!(visible.local.batch.commands, local.batch.commands);
        assert_eq!(visible.remote.batch.commands, remote.batch.commands);
        assert_eq!(visible.local.batch.commands[0], local.batch.commands[0]);
        assert_eq!(visible.conflict_id, conflict_id);
    });
}

#[test]
fn every_resolution_choice_retains_the_two_promised_intents() {
    for choice in [
        ResolutionChoice::KeepLocal,
        ResolutionChoice::KeepRemote,
        ResolutionChoice::KeepBoth,
    ] {
        let local = ConflictIntent::new(batch("local", "local intent"), None).unwrap();
        let remote = ConflictIntent::new(batch("remote", "remote intent"), None).unwrap();
        let mut conflict = ConflictRecord::new(project_id(), 42, local.clone(), remote.clone()).unwrap();
        let plan = conflict.resolve(choice).unwrap();
        assert_eq!(plan.retained.len(), 2);
        assert!(plan.retained.contains(&local));
        assert!(plan.retained.contains(&remote));
        assert_eq!(conflict.resolution, Some(choice));
    }
}

#[test]
fn logical_snapshot_plus_journal_rebuilds_a_working_session() {
    block_on(async {
        let snapshot = LogicalSnapshot::new(project_id(), initial_snapshot(), 0, 100).unwrap();
        let entry = JournalEntry::new(1, project_id(), batch("journal-edit", "replayed"), RevisionId::new(1)).unwrap();
        let bundle = RecoveryBundle {
            snapshot,
            journal: vec![entry],
        };
        let session = bundle
            .restore(InMemoryDesignerPersistence::default())
            .await
            .unwrap();
        let DesignerQueryResult::Snapshot(snapshot) = session.query(DesignerQuery::Snapshot) else {
            panic!("expected rebuilt design snapshot");
        };
        assert_eq!(snapshot.revision.id, RevisionId::new(1));
        assert_eq!(
            snapshot.design.nodes[&NodeId::new("item").unwrap()].properties["text"],
            PropertyValue::String("replayed".to_owned())
        );
    });
}

#[test]
fn recovery_center_quarantines_without_deleting_source_bundle() {
    block_on(async {
        let persistence = InMemoryRecoveryPersistence::default();
        let bundle = RecoveryBundle {
            snapshot: LogicalSnapshot::new(project_id(), initial_snapshot(), 0, 100).unwrap(),
            journal: Vec::new(),
        };
        let record = RecoveryRecord::new("recovery-1", bundle.clone()).unwrap();
        let mut center = RecoveryCenter::open(persistence.clone(), project_id()).await.unwrap();
        center.record(record).await.unwrap();
        center.quarantine("recovery-1", "migration interrupted").await.unwrap();
        let saved = persistence.records(&project_id());
        assert!(matches!(saved[0].state, studio_design::RecoveryState::Quarantined));
        assert_eq!(saved[0].bundle, bundle);
    });
}
