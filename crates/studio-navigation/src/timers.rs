//! Bounded host-scheduled timer events for compiler-declared page timers.
//!
//! The host supplies a monotonic elapsed clock and observes each timer's retained
//! status node. Idle polls never invoke guest code. Page owners drop schedules on
//! exit, and construct fresh schedules when returning to retained page state.

use std::collections::BTreeSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Reserved signed asset containing compiler-declared page timers.
pub const TIMER_ASSET: &str = "assets/studio/timers.json";
/// Maximum encoded metadata document size.
pub const MAX_TIMER_BYTES: usize = 1024 * 1024;
/// Maximum number of pages in a timer declaration.
pub const MAX_TIMER_PAGES: usize = 64;
/// Maximum timers scheduled on one active page.
pub const MAX_PAGE_TIMERS: usize = 16;
/// Maximum due events delivered by one host poll.
pub const MAX_TIMER_EVENTS: usize = 32;

/// One compiler-declared timer with host-validated retained-node targets.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimerSpec {
    /// Stable timer identity, scoped to its page.
    pub id: String,
    /// Invisible retained node receiving each tick's `pressed` event.
    pub tick_node: String,
    /// Invisible retained node receiving the completion `pressed` event.
    pub complete_node: String,
    /// Retained text node projecting `running` or `complete`.
    pub status_node: String,
    /// Tick interval, an integer between 1,000 and 60,000 milliseconds.
    pub interval_ms: u64,
    /// Delay after first observed completion, at most 60,000 milliseconds.
    pub completion_delay_ms: u64,
}

/// One compiled page's timer declarations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimerPage {
    /// Compiled page identity from the admitted routing manifest.
    pub id: String,
    /// Timers in deterministic declaration order.
    pub timers: Vec<TimerSpec>,
}

/// Closed signed timer metadata; it grants no arbitrary guest callback authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimerManifest {
    /// Timer metadata schema version, currently one.
    pub schema_version: u32,
    /// At most 64 uniquely identified compiled pages.
    pub pages: Vec<TimerPage>,
}

impl TimerManifest {
    /// Decode and validate a bounded, closed timer declaration.
    ///
    /// # Errors
    /// Rejects oversized documents, unknown fields, invalid integer timing, IDs or limits.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_TIMER_BYTES {
            return Err("timer metadata exceeds 1 MiB".to_owned());
        }
        let manifest: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("invalid timer metadata: {error}"))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate an in-memory compiler declaration before serializing or scheduling it.
    ///
    /// Native node ownership and kinds must additionally be checked against the
    /// host's admitted page registry before dispatching any timer event.
    ///
    /// # Errors
    /// Rejects unsupported versions, excessive declarations or ambiguous identities.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("timer schemaVersion must be 1".to_owned());
        }
        if self.pages.len() > MAX_TIMER_PAGES {
            return Err("timer metadata permits at most 64 pages".to_owned());
        }
        let mut pages = BTreeSet::new();
        for page in &self.pages {
            validate_id(&page.id)?;
            if !pages.insert(page.id.as_str()) {
                return Err("timer metadata contains duplicate page IDs".to_owned());
            }
            validate_specs(&page.timers)?;
        }
        Ok(())
    }
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
        return Err(
            "timer IDs and native node IDs must contain 1 to 128 bytes without controls".to_owned(),
        );
    }
    Ok(())
}

fn validate_specs(specs: &[TimerSpec]) -> Result<(), String> {
    if specs.len() > MAX_PAGE_TIMERS {
        return Err("a page permits at most 16 timers".to_owned());
    }
    let mut timers = BTreeSet::new();
    let mut nodes = BTreeSet::new();
    for spec in specs {
        validate_id(&spec.id)?;
        if !timers.insert(spec.id.as_str()) {
            return Err("page timers must have unique IDs".to_owned());
        }
        for node in [
            &spec.id,
            &spec.tick_node,
            &spec.complete_node,
            &spec.status_node,
        ] {
            validate_id(node)?;
            if !nodes.insert(node.as_str()) {
                return Err("timer native node IDs must be unique within their page".to_owned());
            }
        }
        if !(1000..=60_000).contains(&spec.interval_ms) {
            return Err("timer intervalMs must be an integer between 1000 and 60000".to_owned());
        }
        if spec.completion_delay_ms > 60_000 {
            return Err(
                "timer completionDelayMs must be an integer between 0 and 60000".to_owned(),
            );
        }
    }
    Ok(())
}

/// One due host event targeting an admitted timer node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimerEvent {
    /// Tick or completion node named in the signed timer declaration.
    pub node_id: String,
    /// Existing native event name, always `pressed`.
    pub event: String,
}

#[derive(Clone, Debug)]
struct ScheduledTimer {
    spec: TimerSpec,
    next_tick: Option<Duration>,
    completion_due: Option<Duration>,
    completion_observed: bool,
}

impl ScheduledTimer {
    fn deadline(&self) -> Option<Duration> {
        if self.completion_observed {
            self.completion_due
        } else {
            self.next_tick
        }
    }
}

/// Private active-page timer deadlines, with no thread, sleep or guest-owned clock.
#[derive(Clone, Debug)]
pub struct TimerSchedule {
    timers: Vec<ScheduledTimer>,
    observed_time: Duration,
}

impl TimerSchedule {
    /// Start fresh timer intervals for one admitted active page.
    ///
    /// # Errors
    /// Rejects invalid metadata and a starting time whose first deadline overflows.
    pub fn new(specs: &[TimerSpec], now: Duration) -> Result<Self, String> {
        validate_specs(specs)?;
        let timers = specs
            .iter()
            .map(|spec| {
                let next_tick = now
                    .checked_add(Duration::from_millis(spec.interval_ms))
                    .ok_or_else(|| "timer deadline exceeds the host clock range".to_owned())?;
                Ok(ScheduledTimer {
                    spec: spec.clone(),
                    next_tick: Some(next_tick),
                    completion_due: None,
                    completion_observed: false,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            timers,
            observed_time: now,
        })
    }

    /// Observe checked status changes without consuming any scheduled events.
    ///
    /// Completion IDs come from the host's checked status-node projection. First
    /// observation cancels all remaining ticks and schedules one completion event.
    /// A reset to `running` before completion dispatch cancels that pending exit
    /// and starts a fresh interval. After completion dispatch, a schedule remains
    /// terminal until its page owner creates a new schedule. Hosts should observe
    /// immediately after a tick to anchor completion delay to that state change.
    /// Backward clock samples are clamped to the last host observation.
    /// An unrepresentable future completion deadline is cancelled rather than
    /// emitted early; exhausted clock arithmetic never wraps or panics.
    pub fn observe(&mut self, now: Duration, completed: &BTreeSet<String>) {
        self.observed_time = self.observed_time.max(now);
        let now = self.observed_time;
        for timer in &mut self.timers {
            let complete = completed.contains(&timer.spec.id);
            if timer.completion_observed && timer.completion_due.is_some() && !complete {
                timer.completion_observed = false;
                timer.completion_due = None;
                timer.next_tick = now.checked_add(Duration::from_millis(timer.spec.interval_ms));
            } else if !timer.completion_observed && complete {
                timer.completion_observed = true;
                timer.next_tick = None;
                timer.completion_due =
                    now.checked_add(Duration::from_millis(timer.spec.completion_delay_ms));
            }
        }
    }

    /// Produce at most 32 due events, ordered by deadline then declaration order.
    ///
    /// This first observes checked completion/reset status. Catch-up advances from
    /// prior deadlines, preserving pending work and avoiding drift. Callers must
    /// recheck node ownership and completion before delivering queued events,
    /// because an earlier tick can complete or navigate away from the page.
    /// Interactive hosts should instead use [`Self::poll_one`] in a loop bounded
    /// by [`MAX_TIMER_EVENTS`], observing each real handler's resulting status.
    #[must_use]
    pub fn poll(&mut self, now: Duration, completed: &BTreeSet<String>) -> Vec<TimerEvent> {
        self.observe(now, completed);
        let now = self.observed_time;
        let mut events = Vec::new();
        for _ in 0..MAX_TIMER_EVENTS {
            let Some(event) = self.next_due(now) else {
                break;
            };
            events.push(event);
        }
        events
    }

    /// Observe status and consume only the next due event.
    ///
    /// The host can dispatch this event and observe its real resulting page state
    /// before consuming another timer's callback. This prevents an earlier tick
    /// that resets another timer from racing a pre-consumed completion event.
    /// Each host poll must bound its loop by [`MAX_TIMER_EVENTS`].
    #[must_use]
    pub fn poll_one(&mut self, now: Duration, completed: &BTreeSet<String>) -> Option<TimerEvent> {
        self.observe(now, completed);
        self.next_due(self.observed_time)
    }

    fn next_due(&mut self, now: Duration) -> Option<TimerEvent> {
        let (_, index) = self
            .timers
            .iter()
            .enumerate()
            .filter_map(|(index, timer)| {
                timer
                    .deadline()
                    .filter(|deadline| *deadline <= now)
                    .map(|deadline| (deadline, index))
            })
            .min()?;
        let timer = &mut self.timers[index];
        let node_id = if timer.completion_observed {
            timer.completion_due = None;
            timer.spec.complete_node.clone()
        } else {
            timer.next_tick = timer.next_tick.and_then(|deadline| {
                deadline.checked_add(Duration::from_millis(timer.spec.interval_ms))
            });
            timer.spec.tick_node.clone()
        };
        Some(TimerEvent {
            node_id,
            event: "pressed".to_owned(),
        })
    }
}
