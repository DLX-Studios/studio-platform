//! Host-clock route transition resolution.

use std::time::Duration;

/// Read-only monotonic time source owned by the host.
pub trait HostClock {
    /// Duration since an arbitrary, stable host epoch.
    fn elapsed(&self) -> Duration;
}

/// Authoritative user motion preference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MotionPreference {
    /// Use approved host-scheduled movement.
    Standard,
    /// Complete transitions without animated movement.
    Reduced,
}

/// Closed route transition kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionKind {
    /// A route was pushed.
    Push,
    /// A route was popped.
    Pop,
    /// A route was replaced.
    Replace,
}

/// Resolved transition policy; plugins cannot request frame callbacks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteTransition {
    kind: TransitionKind,
    duration: Duration,
}

impl RouteTransition {
    /// Resolve an approved duration under the host motion preference.
    #[must_use]
    pub const fn resolve(kind: TransitionKind, preference: MotionPreference) -> Self {
        let duration = match preference {
            MotionPreference::Reduced => Duration::ZERO,
            MotionPreference::Standard => match kind {
                TransitionKind::Push => Duration::from_millis(180),
                TransitionKind::Pop => Duration::from_millis(160),
                TransitionKind::Replace => Duration::from_millis(120),
            },
        };
        Self { kind, duration }
    }

    /// Return the resolved duration.
    #[must_use]
    pub const fn duration(self) -> Duration {
        self.duration
    }

    /// Return the route operation represented by this policy.
    #[must_use]
    pub const fn kind(self) -> TransitionKind {
        self.kind
    }
}

/// Result of sampling an active transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionState {
    /// More host frames are needed.
    Running,
    /// The destination state has been committed.
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveTransition {
    policy: RouteTransition,
    started_at: Duration,
    destination: String,
}

/// Interruptible route transition scheduler driven exclusively by a host clock.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionController {
    preference: MotionPreference,
    current_route: String,
    active: Option<ActiveTransition>,
}

impl TransitionController {
    /// Create an idle controller with the authoritative motion preference.
    #[must_use]
    pub const fn new(preference: MotionPreference) -> Self {
        Self {
            preference,
            current_route: String::new(),
            active: None,
        }
    }

    /// Interrupt any prior transition and start the newest route change.
    pub fn begin(
        &mut self,
        kind: TransitionKind,
        from: &str,
        destination: &str,
        clock: &impl HostClock,
    ) {
        if self.current_route.is_empty() {
            from.clone_into(&mut self.current_route);
        }
        self.active = Some(ActiveTransition {
            policy: RouteTransition::resolve(kind, self.preference),
            started_at: clock.elapsed(),
            destination: destination.to_owned(),
        });
    }

    /// Sample against host time and commit the deterministic destination when complete.
    pub fn sample(&mut self, clock: &impl HostClock) -> TransitionState {
        let Some(active) = self.active.as_ref() else {
            return TransitionState::Completed;
        };
        if clock.elapsed().saturating_sub(active.started_at) < active.policy.duration {
            return TransitionState::Running;
        }
        self.current_route.clone_from(&active.destination);
        self.active = None;
        TransitionState::Completed
    }

    /// Return the active resolved transition, if any.
    #[must_use]
    pub fn active(&self) -> Option<RouteTransition> {
        self.active.as_ref().map(|active| active.policy)
    }

    /// Return the last committed route.
    #[must_use]
    pub fn current_route(&self) -> &str {
        &self.current_route
    }
}
