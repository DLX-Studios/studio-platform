#![allow(missing_docs)]

use std::time::Duration;

use studio_navigation::{
    HostClock, MotionPreference, RouteTransition, TransitionController, TransitionKind,
    TransitionState,
};

#[derive(Default)]
struct ManualClock(Duration);

impl HostClock for ManualClock {
    fn elapsed(&self) -> Duration {
        self.0
    }
}

impl ManualClock {
    fn advance(&mut self, duration: Duration) {
        self.0 += duration;
    }
}

#[test]
fn route_transition_timing_uses_only_the_host_clock() {
    let mut clock = ManualClock::default();
    let mut controller = TransitionController::new(MotionPreference::Standard);
    for (kind, expected) in [
        (TransitionKind::Push, 180),
        (TransitionKind::Pop, 160),
        (TransitionKind::Replace, 120),
    ] {
        controller.begin(kind, "/from", "/to", &clock);
        assert_eq!(
            controller.active().unwrap().duration(),
            Duration::from_millis(expected)
        );
        assert_eq!(controller.sample(&clock), TransitionState::Running);
        clock.advance(Duration::from_millis(expected));
        assert_eq!(controller.sample(&clock), TransitionState::Completed);
        assert_eq!(controller.current_route(), "/to");
    }
}

#[test]
fn interruption_commits_the_newest_deterministic_final_state() {
    let mut clock = ManualClock::default();
    let mut controller = TransitionController::new(MotionPreference::Standard);
    controller.begin(TransitionKind::Push, "/catalog", "/cart", &clock);
    clock.advance(Duration::from_millis(60));
    assert_eq!(controller.sample(&clock), TransitionState::Running);
    controller.begin(TransitionKind::Replace, "/cart", "/payment", &clock);
    clock.advance(Duration::from_millis(120));
    assert_eq!(controller.sample(&clock), TransitionState::Completed);
    assert_eq!(controller.current_route(), "/payment");
    assert!(controller.active().is_none());
}

#[test]
fn reduced_motion_has_zero_duration_but_the_same_final_state() {
    let clock = ManualClock::default();
    let mut controller = TransitionController::new(MotionPreference::Reduced);
    controller.begin(TransitionKind::Push, "/catalog", "/cart", &clock);
    assert_eq!(controller.sample(&clock), TransitionState::Completed);
    assert_eq!(controller.current_route(), "/cart");

    let resolved = RouteTransition::resolve(TransitionKind::Pop, MotionPreference::Reduced);
    assert_eq!(resolved.duration(), Duration::ZERO);
}
