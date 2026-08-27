#![allow(missing_docs)]

use std::{
    collections::BTreeMap,
    future::Future,
    sync::{Arc, Mutex},
    task::{Context, Poll, Wake, Waker},
};

use studio_design::{
    Actor, ActorId, ActorKind, AgentBatchOutcome, AgentCheckFeedback, AgentCommandBatch,
    AgentEvent, AgentEventSink, AgentProgress, AgentReadResult, AgentReadScope, AgentRunId,
    AgentRunRequest, Command, CommandBatch, DefaultDesignerSession, DesignNode, DesignerSession,
    InMemoryDesignerPersistence, LiveAgentChannel, NodeId, NodeParent, OperationId, ProjectId,
    PropertyValue, RevisionId, STUDIO_DESIGN_SCHEMA_VERSION, Screen, ScreenId, StudioDesign,
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
    ProjectId::new("agent-project").unwrap()
}

fn node_id(value: &str) -> NodeId {
    NodeId::new(value).unwrap()
}

fn operation_id(value: &str) -> OperationId {
    OperationId::new(value).unwrap()
}

fn actor(kind: ActorKind, value: &str) -> Actor {
    Actor {
        id: ActorId::new(value).unwrap(),
        kind,
        display_name: value.to_owned(),
    }
}

fn design() -> StudioDesign {
    let screen_id = ScreenId::new("main").unwrap();
    let root_id = node_id("root");
    let left_id = node_id("left");
    let right_id = node_id("right");
    let mut root = DesignNode::primitive(root_id.clone(), "Root", NodeKind::Box);
    root.children = vec![left_id.clone(), right_id.clone()];
    let left = DesignNode::primitive(left_id.clone(), "Left", NodeKind::Text);
    let right = DesignNode::primitive(right_id.clone(), "Right", NodeKind::Text);
    StudioDesign {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        project_id: project_id(),
        name: "Agent project".to_owned(),
        nodes: [
            (root_id.clone(), root),
            (left_id.clone(), left),
            (right_id.clone(), right),
        ]
        .into_iter()
        .collect(),
        parents: [
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
            (right_id, NodeParent::Node { node_id: root_id }),
        ]
        .into_iter()
        .collect(),
        screens: [(
            screen_id.clone(),
            Screen {
                schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
                id: screen_id.clone(),
                name: "Main".to_owned(),
                route: "/".to_owned(),
                root_node_id: node_id("root"),
            },
        )]
        .into_iter()
        .collect(),
        screen_order: vec![screen_id],
        compositions: BTreeMap::new(),
        tokens: BTreeMap::new(),
        responsive_variants: BTreeMap::new(),
        interactions: BTreeMap::new(),
        collections: BTreeMap::new(),
        bindings: BTreeMap::new(),
        forms: BTreeMap::new(),
    }
}

fn batch(
    actor: Actor,
    base_revision: RevisionId,
    operation: &str,
    group: &str,
    command: Command,
) -> CommandBatch {
    CommandBatch {
        schema_version: STUDIO_DESIGN_SCHEMA_VERSION,
        operation_id: operation_id(operation),
        actor,
        project_id: project_id(),
        base_revision,
        undo_group_id: UndoGroupId::new(group).unwrap(),
        undo_group_name: group.to_owned(),
        preconditions: Vec::new(),
        commands: vec![command],
    }
}

fn agent_batch(batch: CommandBatch, run_id: &str, completed: u32) -> AgentCommandBatch {
    AgentCommandBatch {
        run_id: AgentRunId::new(run_id).unwrap(),
        batch,
        progress: AgentProgress {
            completed,
            total: Some(3),
            message: format!("batch {completed}"),
        },
    }
}

fn created() -> DefaultDesignerSession<InMemoryDesignerPersistence> {
    block_on(DefaultDesignerSession::create(
        InMemoryDesignerPersistence::default(),
        design(),
        operation_id("create"),
        actor(ActorKind::Human, "designer"),
        UndoGroupId::new("create").unwrap(),
    ))
    .unwrap()
}

#[derive(Clone, Default)]
struct RecordingSink(Arc<Mutex<Vec<AgentEvent>>>);

impl AgentEventSink for RecordingSink {
    fn emit(&mut self, event: AgentEvent) {
        self.0.lock().unwrap().push(event);
    }
}

#[test]
fn scoped_reads_are_explicit_and_events_are_transport_free() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink(events.clone());
    let mut channel = LiveAgentChannel::with_event_sink(created(), sink);
    let run_id = AgentRunId::new("run-read").unwrap();
    channel
        .start_run(AgentRunRequest {
            run_id: run_id.clone(),
            actor: actor(ActorKind::Agent, "agent"),
            total_batches: Some(3),
        })
        .unwrap();

    assert!(matches!(
        channel.read(AgentReadScope::Selection).unwrap(),
        AgentReadResult::Selection(_)
    ));
    let subtree = channel
        .read(AgentReadScope::Subtree {
            root_node_id: node_id("root"),
        })
        .unwrap();
    match subtree {
        AgentReadResult::Subtree(subtree) => assert_eq!(subtree.nodes.len(), 3),
        other => panic!("unexpected scoped result: {other:?}"),
    }
    assert!(matches!(
        channel.read(AgentReadScope::Schemas).unwrap(),
        AgentReadResult::Schemas(_)
    ));
    assert!(events.lock().unwrap().iter().any(|event| matches!(
        event,
        AgentEvent::RunStarted { run_id: event_run, .. } if event_run == &run_id
    )));
}

#[test]
fn independent_human_and_agent_edits_rebase_but_overlap_preserves_both_intents() {
    let agent = actor(ActorKind::Agent, "agent");
    let human = actor(ActorKind::Human, "designer");
    let mut channel = LiveAgentChannel::new(created());
    let run_id = AgentRunId::new("run-edit").unwrap();
    channel
        .start_run(AgentRunRequest {
            run_id: run_id.clone(),
            actor: agent.clone(),
            total_batches: Some(3),
        })
        .unwrap();

    let human_edit = batch(
        human,
        RevisionId::INITIAL,
        "human-edit",
        "human",
        Command::SetProperty {
            node_id: node_id("left"),
            property: "text".to_owned(),
            value: Some(PropertyValue::String("human".to_owned())),
        },
    );
    assert!(matches!(
        block_on(channel.session_mut().submit(human_edit)),
        studio_design::CommandOutcome::Accepted(_)
    ));

    let independent = agent_batch(
        batch(
            agent.clone(),
            RevisionId::INITIAL,
            "agent-right",
            "agent-edit",
            Command::SetProperty {
                node_id: node_id("right"),
                property: "text".to_owned(),
                value: Some(PropertyValue::String("agent".to_owned())),
            },
        ),
        "run-edit",
        1,
    );
    let accepted = block_on(channel.submit_batch(independent.clone()));
    assert!(matches!(accepted.outcome, AgentBatchOutcome::Accepted(_)));
    let retried = block_on(channel.submit_batch(independent));
    assert!(matches!(retried.outcome, AgentBatchOutcome::Accepted(_)));

    let overlap = agent_batch(
        batch(
            agent,
            RevisionId::INITIAL,
            "agent-left",
            "agent-edit",
            Command::SetProperty {
                node_id: node_id("left"),
                property: "text".to_owned(),
                value: Some(PropertyValue::String("agent".to_owned())),
            },
        ),
        "run-edit",
        2,
    );
    let conflict = block_on(channel.submit_batch(overlap));
    match conflict.outcome {
        AgentBatchOutcome::Conflict(conflict) => {
            assert_eq!(conflict.attempted.operation_id, operation_id("agent-left"));
            assert_eq!(conflict.intervening.len(), 2);
            assert_eq!(
                conflict.current.design.nodes[&node_id("left")].properties["text"],
                PropertyValue::String("human".to_owned())
            );
        }
        other => panic!("expected overlap conflict, got {other:?}"),
    }
}

#[test]
fn cancellation_stops_future_batches_and_grouped_stream_undoes_as_one() {
    let agent = actor(ActorKind::Agent, "agent");
    let mut channel = LiveAgentChannel::new(created());
    let run_id = AgentRunId::new("run-cancel").unwrap();
    channel
        .start_run(AgentRunRequest {
            run_id: run_id.clone(),
            actor: agent.clone(),
            total_batches: Some(3),
        })
        .unwrap();

    for (operation, command) in [
        (
            "one",
            Command::SetProperty {
                node_id: node_id("left"),
                property: "text".to_owned(),
                value: Some(PropertyValue::String("one".to_owned())),
            },
        ),
        (
            "two",
            Command::RenameNode {
                node_id: node_id("left"),
                name: "Renamed".to_owned(),
            },
        ),
    ] {
        let base = if operation == "one" {
            RevisionId::INITIAL
        } else {
            RevisionId::new(1)
        };
        let result = block_on(channel.submit_batch(agent_batch(
            batch(agent.clone(), base, operation, "agent-task", command),
            "run-cancel",
            if operation == "one" { 1 } else { 2 },
        )));
        assert!(matches!(result.outcome, AgentBatchOutcome::Accepted(_)));
    }
    assert!(channel.cancel_run(&run_id));
    let cancelled = block_on(channel.submit_batch(agent_batch(
        batch(
            agent.clone(),
            RevisionId::new(2),
            "three",
            "agent-task",
            Command::RenameNode {
                node_id: node_id("right"),
                name: "Never accepted".to_owned(),
            },
        ),
        "run-cancel",
        3,
    )));
    assert!(matches!(cancelled.outcome, AgentBatchOutcome::Cancelled(_)));

    let history = channel
        .session()
        .query(studio_design::DesignerQuery::History);
    match history {
        studio_design::DesignerQueryResult::History(history) => {
            assert_eq!(history.entries.len(), 1);
            assert_eq!(history.entries[0].batches.len(), 2);
        }
        other => panic!("unexpected history result: {other:?}"),
    }
    assert!(matches!(
        AgentCheckFeedback::default(),
        AgentCheckFeedback { .. }
    ));
}

#[test]
fn injected_studio_check_feedback_is_machine_readable_and_emitted_to_the_dock() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = RecordingSink(events.clone());
    let checker = |_snapshot: &studio_design::StudioDesignSnapshot| AgentCheckFeedback {
        diagnostics: vec![studio_design::DesignerDiagnostic {
            code: "STUDIO_CHECK_A11Y".to_owned(),
            severity: studio_design::DiagnosticSeverity::Warning,
            message: "add an accessible label".to_owned(),
            node_id: Some(node_id("left")),
            interaction_id: None,
            collection_id: None,
            binding_id: None,
            form_id: None,
            record_id: None,
        }],
    };
    let mut channel = LiveAgentChannel::with_checker_and_event_sink(created(), checker, sink);
    let run_id = AgentRunId::new("run-check").unwrap();
    let agent = actor(ActorKind::Agent, "agent");
    channel
        .start_run(AgentRunRequest::new(run_id.clone(), agent.clone(), Some(1)))
        .unwrap();
    let result = block_on(channel.submit_batch(AgentCommandBatch::new(
        run_id.clone(),
        batch(
            agent,
            RevisionId::INITIAL,
            "checked",
            "checked-edit",
            Command::RenameNode {
                node_id: node_id("left"),
                name: "Checked".to_owned(),
            },
        ),
        AgentProgress {
            completed: 1,
            total: Some(1),
            message: "checked".to_owned(),
        },
    )));
    assert!(matches!(result.outcome, AgentBatchOutcome::Accepted(_)));
    assert!(result.check.has_warnings());
    assert!(events.lock().unwrap().iter().any(|event| matches!(
        event,
        AgentEvent::Warning { diagnostics, .. }
            if diagnostics[0].code == "STUDIO_CHECK_A11Y"
    )));
}
