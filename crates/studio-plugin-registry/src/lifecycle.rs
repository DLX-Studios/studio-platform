//! Bounded lifecycle hook execution with violation containment.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

use crate::descriptor::{HookBudget, LifecycleHook};

/// Lifecycle state machine maintained per admitted extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PluginState {
    /// Signature and compatibility verified; not installed anywhere.
    Admitted,
    /// Installed into at least one project.
    Installed,
    /// Active for at least one project.
    Active,
    /// Quarantined after a hook-budget violation; every further hook is refused.
    Quarantined,
}

/// Host-supplied context handed to one hook invocation.
#[derive(Clone, Copy, Debug)]
pub struct HookContext<'a> {
    /// Project the hook runs for; empty string for project-independent hooks.
    pub project_id: &'a str,
    /// Plugin identity running the hook.
    pub plugin_id: &'a str,
    /// Hook position being executed.
    pub hook: LifecycleHook,
    /// Declared wall-clock budget in milliseconds.
    pub budget_ms: u64,
    /// Declared memory/output budget in bytes.
    pub budget_memory_bytes: usize,
}

/// Handler-declared failure; treated identically to a budget overrun.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HookFailure;

/// Registered host callback for one `(plugin, hook)` position.
///
/// Handlers are pure data transforms in this registry surface; the runtime host wraps real
/// guest invocations behind this signature.
pub type HookCallback = Box<dyn FnMut(&HookContext<'_>) -> Result<Vec<u8>, HookFailure> + Send>;

/// Why one hook invocation was contained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViolationReason {
    /// The hook ran longer than its declared time budget.
    TimeBudgetExceeded {
        /// Declared ceiling in milliseconds.
        allowed_ms: u64,
        /// Observed duration in milliseconds.
        actual_ms: u64,
    },
    /// Hook output exceeded the declared memory/output budget.
    OutputBudgetExceeded {
        /// Declared ceiling in bytes.
        allowed_bytes: usize,
        /// Observed output size in bytes.
        actual_bytes: usize,
    },
    /// The handler rejected its invocation.
    HandlerRejected,
}

/// One recorded containment event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViolationRecord {
    /// Plugin that produced the violation.
    pub plugin_id: String,
    /// Hook position contained.
    pub hook: LifecycleHook,
    /// Containment cause.
    pub reason: ViolationReason,
}

/// Report of one completed or skipped hook dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HookRunReport {
    /// Hook position dispatched.
    pub hook: LifecycleHook,
    /// Whether a handler was registered for this position.
    pub ran: bool,
    /// Output byte count; zero when skipped.
    pub output_bytes: usize,
    /// Observed wall-clock duration; zero when skipped.
    pub duration: Duration,
}

/// Registry-owned callback table keyed by `(plugin id, hook)`.
#[derive(Default)]
pub struct HookRunner {
    handlers: BTreeMap<(String, LifecycleHook), HookCallback>,
}

impl HookRunner {
    /// Register a handler for one plugin/hook position, replacing any prior handler.
    pub fn register(&mut self, plugin_id: &str, hook: LifecycleHook, handler: HookCallback) {
        self.handlers.insert((plugin_id.to_owned(), hook), handler);
    }

    /// Drop every handler registered for one plugin.
    pub fn unregister_plugin(&mut self, plugin_id: &str) {
        self.handlers.retain(|(owner, _), _| owner != plugin_id);
    }

    /// Dispatch one hook under its declared budget.
    ///
    /// Time is measured around the synchronous call and enforced after return; output size is
    /// checked against the declared memory budget. Real preemption of a runaway guest requires
    /// wasm fuel/epoch interruption in the runtime host — UNVERIFIED against that layer.
    ///
    /// # Errors
    ///
    /// Returns [`ViolationReason`] when the invocation must be contained.
    pub fn dispatch(
        &mut self,
        plugin_id: &str,
        project_id: &str,
        hook: LifecycleHook,
        budget: HookBudget,
    ) -> Result<HookRunReport, ViolationReason> {
        let Some(mut handler) = self.handlers.remove(&(plugin_id.to_owned(), hook)) else {
            return Ok(HookRunReport {
                hook,
                ran: false,
                output_bytes: 0,
                duration: Duration::ZERO,
            });
        };
        let context = HookContext {
            project_id,
            plugin_id,
            hook,
            budget_ms: budget.time_ms,
            budget_memory_bytes: budget.memory_bytes,
        };
        let started = Instant::now();
        let outcome =
            catch_unwind(AssertUnwindSafe(|| handler(&context))).unwrap_or(Err(HookFailure));
        let elapsed = started.elapsed();
        self.handlers.insert((plugin_id.to_owned(), hook), handler);
        let report = HookRunReport {
            hook,
            ran: true,
            output_bytes: 0,
            duration: elapsed,
        };
        let output = match outcome {
            Ok(output) => output,
            Err(HookFailure) => return Err(ViolationReason::HandlerRejected),
        };
        if elapsed > Duration::from_millis(budget.time_ms) {
            return Err(ViolationReason::TimeBudgetExceeded {
                allowed_ms: budget.time_ms,
                actual_ms: u64::try_from(elapsed.as_nanos().div_ceil(1_000_000))
                    .unwrap_or(u64::MAX),
            });
        }
        if output.len() > budget.memory_bytes {
            return Err(ViolationReason::OutputBudgetExceeded {
                allowed_bytes: budget.memory_bytes,
                actual_bytes: output.len(),
            });
        }
        Ok(HookRunReport {
            output_bytes: output.len(),
            ..report
        })
    }
}

impl std::fmt::Debug for HookRunner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HookRunner")
            .field("handlers", &self.handlers.len())
            .finish()
    }
}
