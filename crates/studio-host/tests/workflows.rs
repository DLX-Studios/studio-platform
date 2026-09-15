#![allow(missing_docs)]

use serde_json::json;
use studio_host::{
    ManualWorkflowClock, MemoryWorkflowAuditLog, MemoryWorkflowRuntime, MissedFirePolicy,
    QueueWorkflowEventSource, RetryPolicy, WorkflowAction, WorkflowDefinition, WorkflowEngine,
    WorkflowEvent, WorkflowEventSourceKind, WorkflowPayload, WorkflowRunStatus, WorkflowTrigger,
};

#[test]
fn interval_drift_uses_declared_missed_fire_bound_and_absolute_boundaries() {
    let definition = WorkflowDefinition::new(
        "accrue",
        WorkflowTrigger::interval(100, 100, MissedFirePolicy::CatchUp { max_runs: 2 }),
        vec![WorkflowAction::increment_state("count", 1)],
        RetryPolicy::no_retry(),
    )
    .expect("definition is valid");
    let runtime = MemoryWorkflowRuntime::new(json!({ "count": 0 })).expect("state is valid");
    let audit = MemoryWorkflowAuditLog::new();
    let mut engine = WorkflowEngine::new([definition], runtime, audit).expect("engine is valid");
    let mut clock = ManualWorkflowClock::new(550);
    let mut source = QueueWorkflowEventSource::new();

    let first = engine.tick(&clock, &mut source).expect("tick succeeds");
    assert_eq!(first.len(), 2);
    assert_eq!(engine.runtime().state()["count"], 2);

    clock.set(500);
    assert!(
        engine
            .tick(&clock, &mut source)
            .expect("backward tick succeeds")
            .is_empty()
    );
    clock.set(650);
    assert_eq!(
        engine
            .tick(&clock, &mut source)
            .expect("tick succeeds")
            .len(),
        1
    );
    assert_eq!(engine.runtime().state()["count"], 3);
}

#[test]
fn event_trigger_delivers_only_validated_trigger_payload_to_plugin() {
    let definition = WorkflowDefinition::new(
        "webhook-report",
        WorkflowTrigger::event(WorkflowEventSourceKind::Webhook, "report.ready"),
        vec![WorkflowAction::EmitPluginEvent {
            plugin: "reports".to_owned(),
            event: "report_received".to_owned(),
            payload: WorkflowPayload::TriggerEvent,
        }],
        RetryPolicy::no_retry(),
    )
    .expect("definition is valid");
    let runtime = MemoryWorkflowRuntime::new(json!({})).expect("state is valid");
    let mut engine = WorkflowEngine::new([definition], runtime, MemoryWorkflowAuditLog::new())
        .expect("engine is valid");
    let event = WorkflowEvent::new(
        WorkflowEventSourceKind::Webhook,
        "report.ready",
        json!({ "report_id": "r-7" }),
    )
    .expect("event is valid");
    assert_eq!(engine.dispatch_event(event).expect("event admits"), 1);
    let reports = engine.run_due(42).expect("run succeeds");
    assert_eq!(reports[0].status(), WorkflowRunStatus::Succeeded);
    assert_eq!(engine.runtime().plugin_events()[0].plugin(), "reports");
    assert_eq!(
        engine.runtime().plugin_events()[0].payload(),
        &json!({ "report_id": "r-7" })
    );
}

#[test]
fn rejected_commit_isolated_and_retried_without_partial_state() {
    let definition = WorkflowDefinition::new(
        "retrying",
        WorkflowTrigger::at(100, MissedFirePolicy::FireOnce),
        vec![WorkflowAction::increment_state("count", 1)],
        RetryPolicy::new(2, 10).expect("retry policy is valid"),
    )
    .expect("definition is valid");
    let mut runtime = MemoryWorkflowRuntime::new(json!({ "count": 0 })).expect("state is valid");
    runtime.reject_next_commits(1);
    let mut engine = WorkflowEngine::new([definition], runtime, MemoryWorkflowAuditLog::new())
        .expect("engine is valid");
    let mut clock = ManualWorkflowClock::new(100);
    let mut source = QueueWorkflowEventSource::new();

    let failed = engine
        .tick(&clock, &mut source)
        .expect("failed tick is reported");
    assert_eq!(failed[0].status(), WorkflowRunStatus::RetryScheduled);
    assert_eq!(engine.runtime().state()["count"], 0);
    clock.set(110);
    let retried = engine
        .tick(&clock, &mut source)
        .expect("retry tick succeeds");
    assert_eq!(retried[0].status(), WorkflowRunStatus::Succeeded);
    assert_eq!(engine.runtime().state()["count"], 1);
    assert_eq!(engine.audit().entries().len(), 2);
    assert_eq!(engine.audit().entries()[0].actor(), "workflow:retrying");
}

#[test]
fn skipped_missed_fixed_time_is_audited_without_running_actions() {
    let definition = WorkflowDefinition::new(
        "stale-report",
        WorkflowTrigger::at(100, MissedFirePolicy::Skip),
        vec![WorkflowAction::set_state("ran", json!(true))],
        RetryPolicy::no_retry(),
    )
    .expect("definition is valid");
    let runtime = MemoryWorkflowRuntime::new(json!({ "ran": false })).expect("state is valid");
    let mut engine = WorkflowEngine::new([definition], runtime, MemoryWorkflowAuditLog::new())
        .expect("engine is valid");
    let mut clock = ManualWorkflowClock::new(101);
    let mut source = QueueWorkflowEventSource::new();
    let reports = engine.tick(&clock, &mut source).expect("tick succeeds");
    assert_eq!(reports[0].status(), WorkflowRunStatus::Skipped);
    assert_eq!(engine.runtime().state()["ran"], false);
    assert_eq!(engine.audit().entries()[0].actor(), "workflow:stale-report");
    clock.set(1000);
    assert!(
        engine
            .tick(&clock, &mut source)
            .expect("one-shot stays complete")
            .is_empty()
    );
}
