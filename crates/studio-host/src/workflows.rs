//! Host-owned declarative scheduling for application workflows.
//!
//! A workflow is a closed, validated description of a trigger and a small list of typed
//! actions.  The scheduler owns time and event admission; guests never receive a timer, socket,
//! thread, filesystem handle, or storage-engine handle.  [`WorkflowRuntime`] is the narrow seam
//! used by the application-data topology to provide an atomic snapshot/commit operation.

use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use thiserror::Error;

const MAX_WORKFLOWS: usize = 128;
const MAX_ACTIONS: usize = 32;
const MAX_PENDING_RUNS: usize = 512;
const MAX_EVENTS_PER_POLL: usize = 256;
const MAX_AUDIT_ENTRIES: usize = 4096;
const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_STATE_KEY_BYTES: usize = 128;
const MAX_VALUE_BYTES: usize = 64 * 1024;
const MAX_VALUE_DEPTH: usize = 16;
const MAX_COLLECTION_ITEMS: usize = 4096;
const MAX_INTERVAL_MILLIS: u64 = 365 * 24 * 60 * 60 * 1_000;
const MAX_RETRY_ATTEMPTS: u8 = 8;
const MAX_RETRY_BACKOFF_MILLIS: u64 = 24 * 60 * 60 * 1_000;
const MAX_RUNS_PER_TICK: usize = 256;

/// Stable codes for declarative workflow validation and runtime diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDiagnosticCode {
    /// A definition or one of its fields is malformed.
    DefinitionInvalid,
    /// A closed-set trigger is malformed or cannot be scheduled.
    TriggerInvalid,
    /// An action is malformed or uses an unavailable trigger context.
    ActionInvalid,
    /// A payload, action list, queue, or other workflow bound was exceeded.
    BoundsExceeded,
    /// An incoming application or webhook event is malformed.
    EventInvalid,
    /// The injected event source returned too many events.
    EventSourceOverrun,
    /// The bounded pending-run queue is full.
    QueueFull,
    /// The runtime could not provide a state snapshot.
    StateUnavailable,
    /// A staged action could not be applied to the state snapshot.
    StateInvalid,
    /// The runtime rejected an atomic commit.
    CommitRejected,
    /// The retry policy has no attempts remaining.
    RetryExhausted,
    /// The optional audit sink could not append a record.
    AuditUnavailable,
}

impl WorkflowDiagnosticCode {
    const fn message(self) -> &'static str {
        match self {
            Self::DefinitionInvalid => "workflow definition is invalid",
            Self::TriggerInvalid => "workflow trigger is invalid",
            Self::ActionInvalid => "workflow action is invalid",
            Self::BoundsExceeded => "workflow bound exceeded",
            Self::EventInvalid => "workflow event is invalid",
            Self::EventSourceOverrun => "workflow event source exceeded its bound",
            Self::QueueFull => "workflow run queue is full",
            Self::StateUnavailable => "workflow state is unavailable",
            Self::StateInvalid => "workflow state is invalid",
            Self::CommitRejected => "workflow state commit was rejected",
            Self::RetryExhausted => "workflow retry policy exhausted",
            Self::AuditUnavailable => "workflow audit sink is unavailable",
        }
    }
}

/// A stable operator-facing workflow diagnostic with no untrusted context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkflowDiagnostic {
    code: WorkflowDiagnosticCode,
}

impl WorkflowDiagnostic {
    /// Construct a diagnostic from a stable code.
    #[must_use]
    pub const fn new(code: WorkflowDiagnosticCode) -> Self {
        Self { code }
    }

    /// Stable machine-readable diagnostic code.
    #[must_use]
    pub const fn code(self) -> WorkflowDiagnosticCode {
        self.code
    }

    /// Safe operator-facing message.
    #[must_use]
    pub const fn message(self) -> &'static str {
        self.code.message()
    }
}

impl fmt::Display for WorkflowDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message())
    }
}

/// Workflow operation failure carrying a stable diagnostic only.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[error("{diagnostic}")]
pub struct WorkflowError {
    diagnostic: WorkflowDiagnostic,
}

impl WorkflowError {
    const fn new(code: WorkflowDiagnosticCode) -> Self {
        Self {
            diagnostic: WorkflowDiagnostic::new(code),
        }
    }

    /// Stable diagnostic for this failure.
    #[must_use]
    pub const fn diagnostic(self) -> WorkflowDiagnostic {
        self.diagnostic
    }

    /// Stable machine-readable failure code.
    #[must_use]
    pub const fn code(self) -> WorkflowDiagnosticCode {
        self.diagnostic.code()
    }
}

/// Policy for timer occurrences that became due while the host was stopped or delayed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MissedFirePolicy {
    /// Discard missed occurrences and continue at the next future occurrence.
    Skip,
    /// Run at most one occurrence, regardless of the number missed.
    FireOnce,
    /// Run at most `max_runs` missed occurrences, then continue in the future.
    CatchUp { max_runs: u8 },
}

impl MissedFirePolicy {
    fn validate(self) -> Result<(), WorkflowError> {
        if let Self::CatchUp { max_runs } = self {
            if max_runs == 0 || usize::from(max_runs) > MAX_RUNS_PER_TICK {
                return Err(WorkflowError::new(WorkflowDiagnosticCode::BoundsExceeded));
            }
        }
        Ok(())
    }
}

/// Closed set of host-admitted event sources.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowEventSourceKind {
    /// An event produced by the host application.
    Application,
    /// A payload admitted by a host-owned signed webhook listener.
    Webhook,
}

/// A declarative workflow trigger.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowTrigger {
    /// Fire once at an absolute Unix timestamp in milliseconds.
    At {
        /// Absolute Unix timestamp in milliseconds.
        at_millis: i64,
        /// Policy used if the host starts after the timestamp.
        missed_fire: MissedFirePolicy,
    },
    /// Fire on absolute interval boundaries.
    Interval {
        /// Absolute Unix timestamp of the first interval boundary.
        start_at_millis: i64,
        /// Positive interval length in milliseconds.
        every_millis: u64,
        /// Policy used for interval boundaries missed by the host.
        missed_fire: MissedFirePolicy,
    },
    /// Fire for one exact event source/name pair.
    Event {
        /// Host-approved event source.
        source: WorkflowEventSourceKind,
        /// Stable application or webhook event name.
        name: String,
    },
}

impl WorkflowTrigger {
    /// Construct a fixed-time trigger.
    #[must_use]
    pub const fn at(at_millis: i64, missed_fire: MissedFirePolicy) -> Self {
        Self::At {
            at_millis,
            missed_fire,
        }
    }

    /// Construct an interval trigger.
    #[must_use]
    pub const fn interval(
        start_at_millis: i64,
        every_millis: u64,
        missed_fire: MissedFirePolicy,
    ) -> Self {
        Self::Interval {
            start_at_millis,
            every_millis,
            missed_fire,
        }
    }

    /// Construct an application or webhook event trigger.
    #[must_use]
    pub fn event(source: WorkflowEventSourceKind, name: impl Into<String>) -> Self {
        Self::Event {
            source,
            name: name.into(),
        }
    }

    fn validate(&self) -> Result<(), WorkflowError> {
        match self {
            Self::At { missed_fire, .. } => missed_fire.validate(),
            Self::Interval {
                every_millis,
                missed_fire,
                ..
            } => {
                if *every_millis == 0 || *every_millis > MAX_INTERVAL_MILLIS {
                    return Err(WorkflowError::new(WorkflowDiagnosticCode::TriggerInvalid));
                }
                missed_fire.validate()
            }
            Self::Event { name, .. } => validate_token(name, MAX_IDENTIFIER_BYTES)
                .map_err(|_| WorkflowError::new(WorkflowDiagnosticCode::TriggerInvalid)),
        }
    }
}

/// Payload source for a plugin action.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowPayload {
    /// A definition-owned bounded JSON payload.
    Static {
        /// Payload delivered to the plugin.
        value: Value,
    },
    /// The validated payload of the event that caused this run.
    TriggerEvent,
}

/// A bounded typed workflow action.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkflowAction {
    /// Set one top-level application-state value.
    SetState {
        /// Top-level state key.
        key: String,
        /// New JSON value.
        value: Value,
    },
    /// Add a signed integer delta to one top-level state value.
    IncrementState {
        /// Top-level state key.
        key: String,
        /// Signed delta.
        by: i64,
    },
    /// Emit one host-mediated typed event to a verified plugin instance.
    EmitPluginEvent {
        /// Verified plugin/application target identity.
        plugin: String,
        /// Typed event name.
        event: String,
        /// Static or triggering-event payload.
        payload: WorkflowPayload,
    },
}

impl WorkflowAction {
    /// Construct a state assignment action.
    #[must_use]
    pub fn set_state(key: impl Into<String>, value: Value) -> Self {
        Self::SetState {
            key: key.into(),
            value,
        }
    }

    /// Construct an integer state increment action.
    #[must_use]
    pub fn increment_state(key: impl Into<String>, by: i64) -> Self {
        Self::IncrementState {
            key: key.into(),
            by,
        }
    }

    /// Construct a plugin event action with a static payload.
    #[must_use]
    pub fn emit_plugin_event(
        plugin: impl Into<String>,
        event: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self::EmitPluginEvent {
            plugin: plugin.into(),
            event: event.into(),
            payload: WorkflowPayload::Static { value: payload },
        }
    }

    fn validate(&self) -> Result<(), WorkflowError> {
        match self {
            Self::SetState { key, value } => {
                validate_token(key, MAX_STATE_KEY_BYTES)
                    .map_err(|_| WorkflowError::new(WorkflowDiagnosticCode::ActionInvalid))?;
                validate_value(value)
            }
            Self::IncrementState { key, .. } => validate_token(key, MAX_STATE_KEY_BYTES)
                .map_err(|_| WorkflowError::new(WorkflowDiagnosticCode::ActionInvalid)),
            Self::EmitPluginEvent {
                plugin,
                event,
                payload,
            } => {
                validate_token(plugin, MAX_IDENTIFIER_BYTES)
                    .map_err(|_| WorkflowError::new(WorkflowDiagnosticCode::ActionInvalid))?;
                validate_token(event, MAX_IDENTIFIER_BYTES)
                    .map_err(|_| WorkflowError::new(WorkflowDiagnosticCode::ActionInvalid))?;
                if let WorkflowPayload::Static { value } = payload {
                    validate_value(value)?;
                }
                Ok(())
            }
        }
    }
}

/// Bounded retry behavior for one workflow run. `max_attempts` includes the first attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetryPolicy {
    /// Total attempts, including the initial execution; at most eight.
    pub max_attempts: u8,
    /// Base delay before a retry, doubled for each subsequent retry.
    pub backoff_millis: u64,
}

impl RetryPolicy {
    /// Construct and validate a retry policy.
    ///
    /// # Errors
    ///
    /// Returns a stable diagnostic if the policy exceeds scheduler bounds.
    pub fn new(max_attempts: u8, backoff_millis: u64) -> Result<Self, WorkflowError> {
        let policy = Self {
            max_attempts,
            backoff_millis,
        };
        policy.validate()?;
        Ok(policy)
    }

    /// Policy for one attempt and no retry.
    #[must_use]
    pub const fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            backoff_millis: 0,
        }
    }

    fn validate(self) -> Result<(), WorkflowError> {
        if self.max_attempts == 0
            || self.max_attempts > MAX_RETRY_ATTEMPTS
            || self.backoff_millis > MAX_RETRY_BACKOFF_MILLIS
        {
            return Err(WorkflowError::new(WorkflowDiagnosticCode::BoundsExceeded));
        }
        Ok(())
    }
}

/// A validated declarative workflow definition.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDefinition {
    id: String,
    trigger: WorkflowTrigger,
    actions: Vec<WorkflowAction>,
    retry: RetryPolicy,
    enabled: bool,
}

impl WorkflowDefinition {
    /// Construct an enabled workflow and validate its closed contract.
    ///
    /// # Errors
    ///
    /// Returns a stable diagnostic for malformed identifiers, triggers, actions, or bounds.
    pub fn new(
        id: impl Into<String>,
        trigger: WorkflowTrigger,
        actions: Vec<WorkflowAction>,
        retry: RetryPolicy,
    ) -> Result<Self, WorkflowError> {
        let definition = Self {
            id: id.into(),
            trigger,
            actions,
            retry,
            enabled: true,
        };
        definition.validate()?;
        Ok(definition)
    }

    /// Construct an enabled workflow with no retries.
    ///
    /// # Errors
    ///
    /// Returns a stable diagnostic when the definition is invalid.
    pub fn without_retry(
        id: impl Into<String>,
        trigger: WorkflowTrigger,
        actions: Vec<WorkflowAction>,
    ) -> Result<Self, WorkflowError> {
        Self::new(id, trigger, actions, RetryPolicy::no_retry())
    }

    /// Stable workflow identity.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Declared trigger.
    #[must_use]
    pub const fn trigger(&self) -> &WorkflowTrigger {
        &self.trigger
    }

    /// Ordered typed actions.
    #[must_use]
    pub fn actions(&self) -> &[WorkflowAction] {
        &self.actions
    }

    /// Declared retry behavior.
    #[must_use]
    pub const fn retry(&self) -> RetryPolicy {
        self.retry
    }

    /// Whether this definition participates in scheduling and event matching.
    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    /// Stable actor identity used for audit records (`workflow:<id>`).
    #[must_use]
    pub fn actor(&self) -> String {
        actor_for(&self.id)
    }

    /// Return a copy with scheduling disabled.
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Validate a definition received from a declarative artifact.
    ///
    /// # Errors
    ///
    /// Returns a stable diagnostic before the definition reaches the scheduler.
    pub fn validate(&self) -> Result<(), WorkflowError> {
        validate_token(&self.id, MAX_IDENTIFIER_BYTES)
            .map_err(|_| WorkflowError::new(WorkflowDiagnosticCode::DefinitionInvalid))?;
        if self.actions.is_empty() || self.actions.len() > MAX_ACTIONS {
            return Err(WorkflowError::new(WorkflowDiagnosticCode::BoundsExceeded));
        }
        self.trigger.validate()?;
        self.retry.validate()?;
        for action in &self.actions {
            action.validate()?;
            if matches!(&self.trigger, WorkflowTrigger::Event { .. }) {
                continue;
            }
            if matches!(
                action,
                WorkflowAction::EmitPluginEvent {
                    payload: WorkflowPayload::TriggerEvent,
                    ..
                }
            ) {
                return Err(WorkflowError::new(WorkflowDiagnosticCode::ActionInvalid));
            }
        }
        Ok(())
    }
}

/// One validated host event supplied by an application or webhook adapter.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowEvent {
    /// Host-approved source.
    pub source: WorkflowEventSourceKind,
    /// Exact event name.
    pub name: String,
    /// Bounded schema-validated payload.
    pub payload: Value,
}

impl WorkflowEvent {
    /// Construct and validate a host event.
    ///
    /// # Errors
    ///
    /// Returns a stable diagnostic for malformed names or oversized payloads.
    pub fn new(
        source: WorkflowEventSourceKind,
        name: impl Into<String>,
        payload: Value,
    ) -> Result<Self, WorkflowError> {
        let event = Self {
            source,
            name: name.into(),
            payload,
        };
        event.validate()?;
        Ok(event)
    }

    fn validate(&self) -> Result<(), WorkflowError> {
        validate_token(&self.name, MAX_IDENTIFIER_BYTES)
            .map_err(|_| WorkflowError::new(WorkflowDiagnosticCode::EventInvalid))?;
        validate_value(&self.payload)
            .map_err(|_| WorkflowError::new(WorkflowDiagnosticCode::EventInvalid))
    }
}

/// Clock seam used by [`WorkflowEngine::tick`] and deterministic tests.
pub trait WorkflowClock {
    /// Current absolute Unix timestamp in milliseconds.
    fn now_millis(&self) -> i64;
}

/// Production clock backed by the host system clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemWorkflowClock;

impl WorkflowClock for SystemWorkflowClock {
    fn now_millis(&self) -> i64 {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());
        i64::try_from(millis).unwrap_or(i64::MAX)
    }
}

/// Mutable deterministic clock for scheduler tests and host simulations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManualWorkflowClock {
    now_millis: i64,
}

impl ManualWorkflowClock {
    /// Create a clock at an absolute Unix timestamp in milliseconds.
    #[must_use]
    pub const fn new(now_millis: i64) -> Self {
        Self { now_millis }
    }

    /// Set the clock, including moving it backwards to model clock correction.
    pub const fn set(&mut self, now_millis: i64) {
        self.now_millis = now_millis;
    }

    /// Advance the clock by a signed amount, saturating at the timestamp bounds.
    pub fn advance(&mut self, delta_millis: i64) {
        self.now_millis = self.now_millis.saturating_add(delta_millis);
    }
}

impl WorkflowClock for ManualWorkflowClock {
    fn now_millis(&self) -> i64 {
        self.now_millis
    }
}

/// Event-source seam used by the scheduler.
pub trait WorkflowEventSource {
    /// Poll a bounded batch of already host-admitted events.
    ///
    /// # Errors
    ///
    /// Returns a stable diagnostic when the source cannot provide a bounded batch.
    fn poll(&mut self) -> Result<Vec<WorkflowEvent>, WorkflowError>;
}

/// Deterministic bounded event source for tests and host adapters.
#[derive(Clone, Debug, Default)]
pub struct QueueWorkflowEventSource {
    events: VecDeque<WorkflowEvent>,
}

impl QueueWorkflowEventSource {
    /// Create an empty event source.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            events: VecDeque::new(),
        }
    }

    /// Admit one validated event.
    ///
    /// # Errors
    ///
    /// Returns a stable diagnostic if the payload/name is invalid or the queue is full.
    pub fn push(&mut self, event: WorkflowEvent) -> Result<(), WorkflowError> {
        event.validate()?;
        if self.events.len() >= MAX_EVENTS_PER_POLL {
            return Err(WorkflowError::new(WorkflowDiagnosticCode::BoundsExceeded));
        }
        self.events.push_back(event);
        Ok(())
    }

    /// Number of events waiting to be polled.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether no events are waiting.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl WorkflowEventSource for QueueWorkflowEventSource {
    fn poll(&mut self) -> Result<Vec<WorkflowEvent>, WorkflowError> {
        Ok(self.events.drain(..).collect())
    }
}

/// A staged plugin event in an atomic workflow commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowPluginEvent {
    plugin: String,
    event: String,
    payload: Value,
}

impl WorkflowPluginEvent {
    /// Target verified plugin identity.
    #[must_use]
    pub fn plugin(&self) -> &str {
        &self.plugin
    }

    /// Typed event name.
    #[must_use]
    pub fn event(&self) -> &str {
        &self.event
    }

    /// Event payload.
    #[must_use]
    pub const fn payload(&self) -> &Value {
        &self.payload
    }
}

/// All effects of one workflow run, committed as one host-owned unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowCommit {
    workflow_id: String,
    run_id: String,
    state: Value,
    plugin_events: Vec<WorkflowPluginEvent>,
}

impl WorkflowCommit {
    fn new(
        workflow_id: &str,
        run_id: &str,
        state: Value,
        plugin_events: Vec<WorkflowPluginEvent>,
    ) -> Self {
        Self {
            workflow_id: workflow_id.to_owned(),
            run_id: run_id.to_owned(),
            state,
            plugin_events,
        }
    }

    /// Workflow identity responsible for this commit.
    #[must_use]
    pub fn workflow_id(&self) -> &str {
        &self.workflow_id
    }

    /// Stable run identity for idempotent host adapters.
    #[must_use]
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// Candidate application state.
    #[must_use]
    pub const fn state(&self) -> &Value {
        &self.state
    }

    /// Staged plugin effects.
    #[must_use]
    pub fn plugin_events(&self) -> &[WorkflowPluginEvent] {
        &self.plugin_events
    }
}

/// Stable host runtime failure codes exposed to workflow diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkflowRuntimeErrorCode {
    /// The authoritative state could not be snapshotted.
    SnapshotUnavailable,
    /// The atomic commit was rejected without applying its candidate state.
    CommitRejected,
    /// A runtime-provided state value was malformed.
    StateInvalid,
}

/// Runtime failure with no storage or engine details.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[error("workflow runtime failure")]
pub struct WorkflowRuntimeError {
    code: WorkflowRuntimeErrorCode,
}

impl WorkflowRuntimeError {
    /// Construct a stable runtime failure.
    #[must_use]
    pub const fn new(code: WorkflowRuntimeErrorCode) -> Self {
        Self { code }
    }

    /// Stable runtime failure code.
    #[must_use]
    pub const fn code(self) -> WorkflowRuntimeErrorCode {
        self.code
    }
}

/// Atomic state/effect boundary for the authoritative application topology.
pub trait WorkflowRuntime {
    /// Snapshot state. The returned value must be an object within workflow bounds.
    ///
    /// # Errors
    ///
    /// Returns a stable runtime failure when authoritative state is unavailable.
    fn snapshot(&self) -> Result<Value, WorkflowRuntimeError>;

    /// Commit a candidate state and staged plugin events atomically.
    ///
    /// # Errors
    ///
    /// Returns a stable runtime failure when the atomic commit is rejected.
    fn commit(&mut self, commit: WorkflowCommit) -> Result<(), WorkflowRuntimeError>;
}

/// In-memory runtime useful for deterministic tests and small host simulations.
#[derive(Clone, Debug)]
pub struct MemoryWorkflowRuntime {
    state: Value,
    plugin_events: Vec<WorkflowPluginEvent>,
    rejected_commits: usize,
}

impl MemoryWorkflowRuntime {
    /// Construct a runtime with an object-shaped initial state.
    ///
    /// # Errors
    ///
    /// Returns a stable diagnostic if the initial state is outside workflow bounds.
    pub fn new(state: Value) -> Result<Self, WorkflowError> {
        validate_state(&state)?;
        Ok(Self {
            state,
            plugin_events: Vec::new(),
            rejected_commits: 0,
        })
    }

    /// Current committed state.
    #[must_use]
    pub const fn state(&self) -> &Value {
        &self.state
    }

    /// Plugin events committed by successful workflow runs.
    #[must_use]
    pub fn plugin_events(&self) -> &[WorkflowPluginEvent] {
        &self.plugin_events
    }

    /// Reject the next `count` commits without mutating state or staged events.
    pub fn reject_next_commits(&mut self, count: usize) {
        self.rejected_commits = count;
    }
}

impl WorkflowRuntime for MemoryWorkflowRuntime {
    fn snapshot(&self) -> Result<Value, WorkflowRuntimeError> {
        Ok(self.state.clone())
    }

    fn commit(&mut self, commit: WorkflowCommit) -> Result<(), WorkflowRuntimeError> {
        if self.rejected_commits > 0 {
            self.rejected_commits -= 1;
            return Err(WorkflowRuntimeError::new(
                WorkflowRuntimeErrorCode::CommitRejected,
            ));
        }
        validate_state(&commit.state).map_err(|_| {
            WorkflowRuntimeError::new(WorkflowRuntimeErrorCode::StateInvalid)
        })?;
        self.state = commit.state;
        self.plugin_events.extend(commit.plugin_events);
        Ok(())
    }
}

/// Audit status for one attempt or skipped occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunStatus {
    /// All staged effects committed.
    Succeeded,
    /// The attempt failed and will be retried.
    RetryScheduled,
    /// The attempt failed and has no retry remaining.
    Failed,
    /// A missed timer occurrence was intentionally discarded.
    Skipped,
}

/// One append-only audit record. It intentionally excludes trigger payloads and state values.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct WorkflowAuditEntry {
    workflow_id: String,
    actor: String,
    run_id: String,
    attempt: u8,
    status: WorkflowRunStatus,
    scheduled_at_millis: i64,
    occurred_at_millis: i64,
    diagnostic: Option<WorkflowDiagnosticCode>,
}

impl WorkflowAuditEntry {
    /// Stable workflow identity.
    #[must_use]
    pub fn workflow_id(&self) -> &str {
        &self.workflow_id
    }

    /// Actor identity exposed for audit integration (`workflow:<id>`).
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }

    /// Stable run identity.
    #[must_use]
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// Attempt number, starting at one.
    #[must_use]
    pub const fn attempt(&self) -> u8 {
        self.attempt
    }

    /// Run status.
    #[must_use]
    pub const fn status(&self) -> WorkflowRunStatus {
        self.status
    }

    /// Scheduled trigger timestamp.
    #[must_use]
    pub const fn scheduled_at_millis(&self) -> i64 {
        self.scheduled_at_millis
    }

    /// Host timestamp at which this audit record was emitted.
    #[must_use]
    pub const fn occurred_at_millis(&self) -> i64 {
        self.occurred_at_millis
    }

    /// Failure diagnostic, if any.
    #[must_use]
    pub const fn diagnostic(&self) -> Option<WorkflowDiagnosticCode> {
        self.diagnostic
    }

}

/// Audit sink used by the scheduler. Ticket 29 can adapt this to its append-only log.
pub trait WorkflowAuditSink {
    /// Append one workflow record without exposing payload or state data.
    ///
    /// # Errors
    ///
    /// Returns a stable diagnostic when the sink cannot accept another record.
    fn append(&mut self, entry: WorkflowAuditEntry) -> Result<(), WorkflowError>;
}

/// Optional audit implementation that deliberately drops records.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopWorkflowAudit;

impl WorkflowAuditSink for NoopWorkflowAudit {
    fn append(&mut self, _entry: WorkflowAuditEntry) -> Result<(), WorkflowError> {
        Ok(())
    }
}

/// Bounded in-memory audit sink for deterministic tests and preview hosts.
#[derive(Clone, Debug)]
pub struct MemoryWorkflowAuditLog {
    entries: Vec<WorkflowAuditEntry>,
}

impl MemoryWorkflowAuditLog {
    /// Create an empty audit log.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Read records in append order.
    #[must_use]
    pub fn entries(&self) -> &[WorkflowAuditEntry] {
        &self.entries
    }
}

impl Default for MemoryWorkflowAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowAuditSink for MemoryWorkflowAuditLog {
    fn append(&mut self, entry: WorkflowAuditEntry) -> Result<(), WorkflowError> {
        if self.entries.len() >= MAX_AUDIT_ENTRIES {
            return Err(WorkflowError::new(WorkflowDiagnosticCode::AuditUnavailable));
        }
        self.entries.push(entry);
        Ok(())
    }
}

/// Observable result of one attempted or skipped workflow occurrence.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct WorkflowRunReport {
    workflow_id: String,
    actor: String,
    run_id: String,
    attempt: u8,
    status: WorkflowRunStatus,
    scheduled_at_millis: i64,
    occurred_at_millis: i64,
    diagnostic: Option<WorkflowDiagnosticCode>,
}

impl WorkflowRunReport {
    /// Stable workflow identity.
    #[must_use]
    pub fn workflow_id(&self) -> &str {
        &self.workflow_id
    }

    /// Workflow actor identity (`workflow:<id>`).
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }

    /// Stable run identity.
    #[must_use]
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// Attempt number.
    #[must_use]
    pub const fn attempt(&self) -> u8 {
        self.attempt
    }

    /// Run status.
    #[must_use]
    pub const fn status(&self) -> WorkflowRunStatus {
        self.status
    }

    /// Failure diagnostic, if any.
    #[must_use]
    pub const fn diagnostic(&self) -> Option<WorkflowDiagnosticCode> {
        self.diagnostic
    }

    /// Scheduled trigger timestamp.
    #[must_use]
    pub const fn scheduled_at_millis(&self) -> i64 {
        self.scheduled_at_millis
    }

    /// Host timestamp at which this report was emitted.
    #[must_use]
    pub const fn occurred_at_millis(&self) -> i64 {
        self.occurred_at_millis
    }
}

#[derive(Clone, Debug)]
enum ScheduleState {
    At { fired: bool },
    Interval { next_at_millis: i64, exhausted: bool },
    Event,
}

#[derive(Clone, Debug)]
struct PendingRun {
    workflow_id: String,
    run_id: String,
    scheduled_at_millis: i64,
    attempt: u8,
    next_attempt_at_millis: i64,
    event: Option<WorkflowEvent>,
}

/// Host-owned bounded workflow scheduler.
pub struct WorkflowEngine<R, A = NoopWorkflowAudit> {
    workflows: BTreeMap<String, WorkflowDefinition>,
    schedules: BTreeMap<String, ScheduleState>,
    pending: VecDeque<PendingRun>,
    runtime: R,
    audit: A,
    next_run_number: u64,
}

impl<R: WorkflowRuntime, A: WorkflowAuditSink> WorkflowEngine<R, A> {
    /// Build a scheduler from validated declarative definitions and host-owned seams.
    ///
    /// # Errors
    ///
    /// Returns a stable diagnostic if definitions are invalid, duplicated, or excessive.
    pub fn new(
        definitions: impl IntoIterator<Item = WorkflowDefinition>,
        runtime: R,
        audit: A,
    ) -> Result<Self, WorkflowError> {
        let mut workflows = BTreeMap::new();
        for definition in definitions {
            definition.validate()?;
            if workflows.len() >= MAX_WORKFLOWS
                || workflows
                    .insert(definition.id.clone(), definition)
                    .is_some()
            {
                return Err(WorkflowError::new(WorkflowDiagnosticCode::BoundsExceeded));
            }
        }
        let schedules = workflows
            .values()
            .map(|workflow| (workflow.id.clone(), schedule_for(workflow)))
            .collect();
        Ok(Self {
            workflows,
            schedules,
            pending: VecDeque::new(),
            runtime,
            audit,
            next_run_number: 1,
        })
    }

    /// Stable sorted workflow identities.
    #[must_use]
    pub fn workflow_ids(&self) -> impl Iterator<Item = &str> {
        self.workflows.keys().map(String::as_str)
    }

    /// Immutable access to the authoritative runtime seam.
    #[must_use]
    pub const fn runtime(&self) -> &R {
        &self.runtime
    }

    /// Mutable access to the authoritative runtime seam.
    #[must_use]
    pub const fn runtime_mut(&mut self) -> &mut R {
        &mut self.runtime
    }

    /// Immutable access to the configured audit sink.
    #[must_use]
    pub const fn audit(&self) -> &A {
        &self.audit
    }

    /// Mutable access to the configured audit sink.
    #[must_use]
    pub const fn audit_mut(&mut self) -> &mut A {
        &mut self.audit
    }

    /// Number of queued timer/event/retry runs.
    #[must_use]
    pub fn pending_runs(&self) -> usize {
        self.pending.len()
    }

    /// Poll an injected clock and event source, then execute due runs.
    ///
    /// Events are validated before any event is admitted, and each run snapshots state and
    /// commits all actions/effects atomically through [`WorkflowRuntime`].
    pub fn tick<C: WorkflowClock, E: WorkflowEventSource>(
        &mut self,
        clock: &C,
        source: &mut E,
    ) -> Result<Vec<WorkflowRunReport>, WorkflowError> {
        let now = clock.now_millis();
        let events = source.poll()?;
        if events.len() > MAX_EVENTS_PER_POLL {
            return Err(WorkflowError::new(
                WorkflowDiagnosticCode::EventSourceOverrun,
            ));
        }
        for event in &events {
            event.validate()?;
        }
        let mut reports = self.enqueue_timers(now)?;
        for event in events {
            self.dispatch_event_at(event, now)?;
        }
        reports.extend(self.run_due(now)?);
        Ok(reports)
    }

    /// Admit one validated application/webhook event without polling an event source.
    ///
    /// # Errors
    ///
    /// Returns a stable diagnostic when the event or pending-run bound is exceeded.
    pub fn dispatch_event(&mut self, event: WorkflowEvent) -> Result<usize, WorkflowError> {
        self.dispatch_event_at(event, 0)
    }

    fn dispatch_event_at(
        &mut self,
        event: WorkflowEvent,
        scheduled_at_millis: i64,
    ) -> Result<usize, WorkflowError> {
        event.validate()?;
        let matching_ids = self
            .workflows
            .values()
            .filter(|workflow| workflow.enabled)
            .filter(|workflow| {
                matches!(
                    &workflow.trigger,
                    WorkflowTrigger::Event { source, name }
                        if *source == event.source && name == &event.name
                )
            })
            .map(|workflow| workflow.id.clone())
            .collect::<Vec<_>>();
        if self.pending.len() + matching_ids.len() > MAX_PENDING_RUNS {
            return Err(WorkflowError::new(WorkflowDiagnosticCode::QueueFull));
        }
        for workflow_id in &matching_ids {
            let run_id = self.next_run_id(workflow_id)?;
            self.pending.push_back(PendingRun {
                workflow_id: workflow_id.clone(),
                run_id,
                scheduled_at_millis,
                attempt: 1,
                next_attempt_at_millis: 0,
                event: Some(event.clone()),
            });
        }
        Ok(matching_ids.len())
    }

    /// Execute all due timer/event/retry runs at a supplied timestamp.
    pub fn run_due(&mut self, now_millis: i64) -> Result<Vec<WorkflowRunReport>, WorkflowError> {
        let mut reports = Vec::new();
        for _ in 0..MAX_RUNS_PER_TICK {
            let Some(index) = self
                .pending
                .iter()
                .position(|run| run.next_attempt_at_millis <= now_millis)
            else {
                break;
            };
            let Some(run) = self.pending.remove(index) else {
                return Err(WorkflowError::new(WorkflowDiagnosticCode::QueueFull));
            };
            let retry_event = run.event.clone();
            let report = self.execute_run(run, now_millis)?;
            let retry = report.status == WorkflowRunStatus::RetryScheduled;
            if retry {
                let attempt = report.attempt.saturating_add(1);
                let workflow = self
                    .workflows
                    .get(report.workflow_id())
                    .ok_or_else(|| WorkflowError::new(WorkflowDiagnosticCode::DefinitionInvalid))?;
                let delay = retry_delay(workflow.retry, report.attempt);
                self.pending.push_back(PendingRun {
                    workflow_id: report.workflow_id.clone(),
                    run_id: report.run_id.clone(),
                    scheduled_at_millis: report.scheduled_at_millis,
                    attempt,
                    next_attempt_at_millis: now_millis.saturating_add(delay),
                    event: retry_event,
                });
            }
            reports.push(report);
        }
        Ok(reports)
    }

    #[allow(clippy::too_many_lines)]
    fn enqueue_timers(&mut self, now_millis: i64) -> Result<Vec<WorkflowRunReport>, WorkflowError> {
        let ids = self.workflows.keys().cloned().collect::<Vec<_>>();
        let mut reports = Vec::new();
        for id in ids {
            let Some(workflow) = self.workflows.get(&id).cloned() else {
                continue;
            };
            if !workflow.enabled || matches!(&workflow.trigger, WorkflowTrigger::Event { .. }) {
                continue;
            }
            let Some(schedule) = self.schedules.get(&id).cloned() else {
                continue;
            };
            let mut next_schedule = schedule;
            let mut timer_count = 0_usize;
            let mut timer_start = 0_i64;
            let mut timer_every = 0_u64;
            let mut skip_at = None;
            match (&workflow.trigger, &mut next_schedule) {
                (
                    WorkflowTrigger::At {
                        at_millis,
                        missed_fire,
                    },
                    ScheduleState::At { fired },
                ) if !*fired && now_millis >= *at_millis => {
                    *fired = true;
                    let missed = now_millis > *at_millis;
                    timer_count = if missed {
                        match missed_fire {
                            MissedFirePolicy::Skip => 0,
                            MissedFirePolicy::FireOnce | MissedFirePolicy::CatchUp { .. } => 1,
                        }
                    } else {
                        1
                    };
                    if timer_count == 0 {
                        skip_at = Some(*at_millis);
                    } else {
                        timer_start = *at_millis;
                    }
                }
                (
                    WorkflowTrigger::Interval {
                        every_millis,
                        missed_fire,
                        ..
                    },
                    ScheduleState::Interval {
                        next_at_millis,
                        exhausted,
                    },
                ) if !*exhausted && now_millis >= *next_at_millis => {
                    let every = i64::try_from(*every_millis).map_err(|_| {
                        WorkflowError::new(WorkflowDiagnosticCode::TriggerInvalid)
                    })?;
                    let due_count = u64::try_from(
                        now_millis.saturating_sub(*next_at_millis) / every,
                    )
                    .unwrap_or(u64::MAX)
                    .saturating_add(1);
                    timer_count = match missed_fire {
                        MissedFirePolicy::Skip if due_count > 1 => 0,
                        MissedFirePolicy::FireOnce => 1,
                        MissedFirePolicy::CatchUp { max_runs } => usize::try_from(
                            due_count.min(u64::from(*max_runs)),
                        )
                        .map_err(|_| WorkflowError::new(WorkflowDiagnosticCode::BoundsExceeded))?,
                        MissedFirePolicy::Skip => 1,
                    };
                    timer_start = *next_at_millis;
                    timer_every = *every_millis;
                    let new_next = advance_interval(*next_at_millis, every, due_count);
                    if let Some(next) = new_next {
                        *next_at_millis = next;
                    } else {
                        *exhausted = true;
                    }
                    if timer_count == 0 {
                        skip_at = Some(timer_start);
                    }
                }
                _ => {}
            }
            if timer_count > 0 {
                self.ensure_capacity(timer_count)?;
            }
            self.schedules.insert(id, next_schedule);
            if let Some(scheduled) = skip_at {
                reports.push(self.skipped_report(&workflow.id, scheduled, now_millis)?);
            } else if timer_count > 0 {
                for offset in 0..timer_count {
                    let offset = u64::try_from(offset).map_err(|_| {
                        WorkflowError::new(WorkflowDiagnosticCode::BoundsExceeded)
                    })?;
                    let occurrence = timer_start.saturating_add(
                        i64::try_from(offset.saturating_mul(timer_every)).unwrap_or(i64::MAX),
                    );
                    let run = self.timer_run(
                        &workflow.id,
                        occurrence,
                        now_millis,
                    )?;
                    self.pending.push_back(run);
                }
            }
        }
        Ok(reports)
    }

    #[allow(clippy::too_many_lines)]
    fn execute_run(
        &mut self,
        run: PendingRun,
        now_millis: i64,
    ) -> Result<WorkflowRunReport, WorkflowError> {
        let workflow = self
            .workflows
            .get(&run.workflow_id)
            .ok_or_else(|| WorkflowError::new(WorkflowDiagnosticCode::DefinitionInvalid))?
            .clone();
        let outcome = self.runtime.snapshot().map_err(|error| {
            WorkflowError::new(match error.code() {
                WorkflowRuntimeErrorCode::SnapshotUnavailable => {
                    WorkflowDiagnosticCode::StateUnavailable
                }
                WorkflowRuntimeErrorCode::StateInvalid => WorkflowDiagnosticCode::StateInvalid,
                WorkflowRuntimeErrorCode::CommitRejected => WorkflowDiagnosticCode::StateUnavailable,
            })
        });
        let report = match outcome {
            Ok(mut state) => match apply_actions(&workflow, &run, &mut state) {
                Ok(plugin_events) => {
                    let commit = WorkflowCommit::new(
                        &workflow.id,
                        &run.run_id,
                        state,
                        plugin_events,
                    );
                    match self.runtime.commit(commit) {
                        Ok(()) => self.report(
                            &workflow,
                            &run,
                            now_millis,
                            WorkflowRunStatus::Succeeded,
                            None,
                        ),
                        Err(error) => self.failure_report(
                            &workflow,
                            &run,
                            now_millis,
                            match error.code() {
                                WorkflowRuntimeErrorCode::CommitRejected => {
                                    WorkflowDiagnosticCode::CommitRejected
                                }
                                WorkflowRuntimeErrorCode::StateInvalid => {
                                    WorkflowDiagnosticCode::StateInvalid
                                }
                                WorkflowRuntimeErrorCode::SnapshotUnavailable => {
                                    WorkflowDiagnosticCode::StateUnavailable
                                }
                            },
                        ),
                    }
                }
                Err(code) => self.failure_report(&workflow, &run, now_millis, code),
            },
            Err(error) => self.failure_report(&workflow, &run, now_millis, error.code()),
        };
        self.append_audit(&report)?;
        Ok(report)
    }

    fn failure_report(
        &self,
        workflow: &WorkflowDefinition,
        run: &PendingRun,
        now_millis: i64,
        diagnostic: WorkflowDiagnosticCode,
    ) -> WorkflowRunReport {
        let status = if run.attempt < workflow.retry.max_attempts {
            WorkflowRunStatus::RetryScheduled
        } else {
            WorkflowRunStatus::Failed
        };
        self.report(workflow, run, now_millis, status, Some(diagnostic))
    }

    fn report(
        &self,
        workflow: &WorkflowDefinition,
        run: &PendingRun,
        now_millis: i64,
        status: WorkflowRunStatus,
        diagnostic: Option<WorkflowDiagnosticCode>,
    ) -> WorkflowRunReport {
        WorkflowRunReport {
            workflow_id: workflow.id.clone(),
            actor: actor_for(&workflow.id),
            run_id: run.run_id.clone(),
            attempt: run.attempt,
            status,
            scheduled_at_millis: run.scheduled_at_millis,
            occurred_at_millis: now_millis,
            diagnostic,
        }
    }

    fn skipped_report(
        &mut self,
        workflow_id: &str,
        scheduled_at_millis: i64,
        now_millis: i64,
    ) -> Result<WorkflowRunReport, WorkflowError> {
        if !self.workflows.contains_key(workflow_id) {
            return Err(WorkflowError::new(WorkflowDiagnosticCode::DefinitionInvalid));
        }
        let run_id = self.next_run_id(workflow_id)?;
        let workflow = self
            .workflows
            .get(workflow_id)
            .ok_or_else(|| WorkflowError::new(WorkflowDiagnosticCode::DefinitionInvalid))?
            .clone();
        let run = PendingRun {
            workflow_id: workflow_id.to_owned(),
            run_id,
            scheduled_at_millis,
            attempt: 0,
            next_attempt_at_millis: now_millis,
            event: None,
        };
        let report = self.report(
            &workflow,
            &run,
            now_millis,
            WorkflowRunStatus::Skipped,
            None,
        );
        self.append_audit(&report)?;
        Ok(report)
    }

    fn append_audit(&mut self, report: &WorkflowRunReport) -> Result<(), WorkflowError> {
        self.audit.append(WorkflowAuditEntry {
            workflow_id: report.workflow_id.clone(),
            actor: report.actor.clone(),
            run_id: report.run_id.clone(),
            attempt: report.attempt,
            status: report.status,
            scheduled_at_millis: report.scheduled_at_millis,
            occurred_at_millis: report.occurred_at_millis,
            diagnostic: report.diagnostic,
        })
    }

    fn timer_run(
        &mut self,
        workflow_id: &str,
        scheduled_at_millis: i64,
        now_millis: i64,
    ) -> Result<PendingRun, WorkflowError> {
        Ok(PendingRun {
            workflow_id: workflow_id.to_owned(),
            run_id: self.next_run_id(workflow_id)?,
            scheduled_at_millis,
            attempt: 1,
            next_attempt_at_millis: now_millis,
            event: None,
        })
    }

    fn next_run_id(&mut self, workflow_id: &str) -> Result<String, WorkflowError> {
        let number = self.next_run_number;
        self.next_run_number = self
            .next_run_number
            .checked_add(1)
            .ok_or_else(|| WorkflowError::new(WorkflowDiagnosticCode::BoundsExceeded))?;
        Ok(format!("{workflow_id}-{number:016x}"))
    }

    fn ensure_capacity(&self, additional: usize) -> Result<(), WorkflowError> {
        if self.pending.len().saturating_add(additional) > MAX_PENDING_RUNS {
            return Err(WorkflowError::new(WorkflowDiagnosticCode::QueueFull));
        }
        Ok(())
    }
}

impl<R: WorkflowRuntime> WorkflowEngine<R, NoopWorkflowAudit> {
    /// Build a scheduler with the optional audit sink disabled.
    ///
    /// # Errors
    ///
    /// Returns a stable diagnostic if any definition is invalid, duplicated, or excessive.
    pub fn without_audit(
        definitions: impl IntoIterator<Item = WorkflowDefinition>,
        runtime: R,
    ) -> Result<Self, WorkflowError> {
        Self::new(definitions, runtime, NoopWorkflowAudit)
    }
}

fn schedule_for(workflow: &WorkflowDefinition) -> ScheduleState {
    match &workflow.trigger {
        WorkflowTrigger::At { .. } => ScheduleState::At { fired: false },
        WorkflowTrigger::Interval {
            start_at_millis, ..
        } => ScheduleState::Interval {
            next_at_millis: *start_at_millis,
            exhausted: false,
        },
        WorkflowTrigger::Event { .. } => ScheduleState::Event,
    }
}

fn apply_actions(
    workflow: &WorkflowDefinition,
    run: &PendingRun,
    state: &mut Value,
) -> Result<Vec<WorkflowPluginEvent>, WorkflowDiagnosticCode> {
    validate_state(state)
        .map_err(|_| WorkflowDiagnosticCode::StateInvalid)?;
    let object = state
        .as_object_mut()
        .ok_or(WorkflowDiagnosticCode::StateInvalid)?;
    let mut plugin_events = Vec::new();
    for action in &workflow.actions {
        match action {
            WorkflowAction::SetState { key, value } => {
                object.insert(key.clone(), value.clone());
            }
            WorkflowAction::IncrementState { key, by } => {
                let current = object.get(key).map_or(Ok(0_i64), |value| {
                    value
                        .as_i64()
                        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
                        .ok_or(WorkflowDiagnosticCode::StateInvalid)
                })?;
                let next = current
                    .checked_add(*by)
                    .ok_or(WorkflowDiagnosticCode::StateInvalid)?;
                object.insert(key.clone(), Value::Number(Number::from(next)));
            }
            WorkflowAction::EmitPluginEvent {
                plugin,
                event,
                payload,
            } => {
                let payload = match payload {
                    WorkflowPayload::Static { value } => value.clone(),
                    WorkflowPayload::TriggerEvent => run
                        .event
                        .as_ref()
                        .ok_or(WorkflowDiagnosticCode::ActionInvalid)?
                        .payload
                        .clone(),
                };
                plugin_events.push(WorkflowPluginEvent {
                    plugin: plugin.clone(),
                    event: event.clone(),
                    payload,
                });
            }
        }
    }
    validate_state(&Value::Object(object.clone()))
        .map_err(|_| WorkflowDiagnosticCode::BoundsExceeded)?;
    Ok(plugin_events)
}

fn retry_delay(policy: RetryPolicy, attempt: u8) -> i64 {
    let exponent = u32::from(attempt.saturating_sub(1));
    let multiplier = 1_u64.checked_shl(exponent).unwrap_or(u64::MAX);
    let delay = policy.backoff_millis.saturating_mul(multiplier);
    i64::try_from(delay).unwrap_or(i64::MAX)
}

fn advance_interval(next_at: i64, every: i64, count: u64) -> Option<i64> {
    let offset = i128::from(every).checked_mul(i128::from(count))?;
    let next = i128::from(next_at).checked_add(offset)?;
    i64::try_from(next).ok()
}

fn actor_for(workflow_id: &str) -> String {
    format!("workflow:{workflow_id}")
}

fn validate_token(value: &str, limit: usize) -> Result<(), ()> {
    if value.is_empty()
        || value.len() > limit
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'/'))
    {
        return Err(());
    }
    Ok(())
}

fn validate_state(value: &Value) -> Result<(), WorkflowError> {
    if !value.is_object() {
        return Err(WorkflowError::new(WorkflowDiagnosticCode::StateInvalid));
    }
    validate_value(value).map_err(|_| WorkflowError::new(WorkflowDiagnosticCode::StateInvalid))
}

fn validate_value(value: &Value) -> Result<(), WorkflowError> {
    if serde_json::to_vec(value)
        .map_err(|_| WorkflowError::new(WorkflowDiagnosticCode::BoundsExceeded))?
        .len()
        > MAX_VALUE_BYTES
    {
        return Err(WorkflowError::new(WorkflowDiagnosticCode::BoundsExceeded));
    }
    validate_value_depth(value, 0)
}

fn validate_value_depth(value: &Value, depth: usize) -> Result<(), WorkflowError> {
    if depth > MAX_VALUE_DEPTH {
        return Err(WorkflowError::new(WorkflowDiagnosticCode::BoundsExceeded));
    }
    match value {
        Value::Array(values) => {
            if values.len() > MAX_COLLECTION_ITEMS {
                return Err(WorkflowError::new(WorkflowDiagnosticCode::BoundsExceeded));
            }
            for value in values {
                validate_value_depth(value, depth + 1)?;
            }
        }
        Value::Object(values) => {
            if values.len() > MAX_COLLECTION_ITEMS {
                return Err(WorkflowError::new(WorkflowDiagnosticCode::BoundsExceeded));
            }
            for (key, value) in values {
                validate_token(key, MAX_STATE_KEY_BYTES)
                    .map_err(|_| WorkflowError::new(WorkflowDiagnosticCode::BoundsExceeded))?;
                validate_value_depth(value, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
