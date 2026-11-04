#![allow(missing_docs)]

use std::{collections::BTreeSet, time::Duration};

use studio_navigation::timers::{TimerManifest, TimerPage, TimerSchedule, TimerSpec};

fn spec(id: &str, interval_ms: u64, completion_delay_ms: u64) -> TimerSpec {
    TimerSpec {
        id: id.into(),
        tick_node: format!("{id}--tick"),
        complete_node: format!("{id}--complete"),
        status_node: format!("{id}--status"),
        interval_ms,
        completion_delay_ms,
    }
}

fn manifest() -> TimerManifest {
    TimerManifest {
        schema_version: 1,
        pages: vec![TimerPage {
            id: "home".into(),
            timers: vec![spec("clock", 1000, 700)],
        }],
    }
}

#[test]
fn deadlines_use_elapsed_time_and_catch_up_in_bounded_batches_without_drift() {
    let mut schedule =
        TimerSchedule::new(&[spec("clock", 1000, 700)], Duration::from_millis(500)).unwrap();
    assert!(
        schedule
            .poll(Duration::from_millis(1499), &BTreeSet::new())
            .is_empty()
    );
    let first = schedule.poll(Duration::from_millis(1500), &BTreeSet::new());
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].node_id, "clock--tick");
    assert_eq!(first[0].event, "pressed");
    assert_eq!(
        schedule
            .poll(Duration::from_secs(101), &BTreeSet::new())
            .len(),
        32
    );
    assert_eq!(
        schedule
            .poll(Duration::from_secs(101), &BTreeSet::new())
            .len(),
        32
    );
    assert_eq!(
        schedule
            .poll(Duration::from_secs(101), &BTreeSet::new())
            .len(),
        32
    );
    assert_eq!(
        schedule
            .poll(Duration::from_secs(101), &BTreeSet::new())
            .len(),
        3
    );
    assert!(
        schedule
            .poll(Duration::from_secs(101), &BTreeSet::new())
            .is_empty()
    );
    assert!(
        schedule
            .poll(Duration::from_millis(101_499), &BTreeSet::new())
            .is_empty()
    );
    assert_eq!(
        schedule
            .poll(Duration::from_millis(101_500), &BTreeSet::new())
            .len(),
        1
    );
}

#[test]
fn completion_cancels_overdue_ticks_and_emits_once_after_its_delay() {
    let mut schedule = TimerSchedule::new(&[spec("clock", 1000, 700)], Duration::ZERO).unwrap();
    let completed = BTreeSet::from(["clock".to_owned()]);
    assert!(
        schedule
            .poll(Duration::from_secs(10), &completed)
            .is_empty()
    );
    assert!(
        schedule
            .poll(Duration::from_millis(10_699), &completed)
            .is_empty()
    );
    let events = schedule.poll(Duration::from_millis(10_700), &completed);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].node_id, "clock--complete");
    assert_eq!(events[0].event, "pressed");
    assert!(
        schedule
            .poll(Duration::from_secs(100), &completed)
            .is_empty()
    );
    assert!(
        schedule
            .poll(Duration::from_secs(200), &BTreeSet::new())
            .is_empty()
    );
}

#[test]
fn due_events_are_ordered_by_deadline_then_declaration_order() {
    let mut schedule = TimerSchedule::new(
        &[spec("slow", 2000, 0), spec("fast", 1000, 0)],
        Duration::ZERO,
    )
    .unwrap();
    let events = schedule.poll(Duration::from_secs(4), &BTreeSet::new());
    let nodes: Vec<_> = events.into_iter().map(|event| event.node_id).collect();
    assert_eq!(
        nodes,
        [
            "fast--tick",
            "slow--tick",
            "fast--tick",
            "fast--tick",
            "slow--tick",
            "fast--tick"
        ]
    );
    let completed = BTreeSet::from(["fast".to_owned()]);
    let events = schedule.poll(Duration::from_secs(4), &completed);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].node_id, "fast--complete");
}

#[test]
fn page_return_uses_a_fresh_interval_and_independent_cloned_deadlines() {
    let mut original = TimerSchedule::new(&[spec("clock", 1000, 0)], Duration::ZERO).unwrap();
    let mut staged = original.clone();
    assert_eq!(
        staged.poll(Duration::from_secs(1), &BTreeSet::new()).len(),
        1
    );
    assert_eq!(
        original
            .poll(Duration::from_secs(1), &BTreeSet::new())
            .len(),
        1
    );
    let mut returning =
        TimerSchedule::new(&[spec("clock", 1000, 0)], Duration::from_secs(50)).unwrap();
    assert!(
        returning
            .poll(Duration::from_millis(50_999), &BTreeSet::new())
            .is_empty()
    );
    assert_eq!(
        returning
            .poll(Duration::from_secs(51), &BTreeSet::new())
            .len(),
        1
    );
}

#[test]
fn timer_metadata_is_closed_versioned_bounded_and_uniquely_owned() {
    assert!(manifest().validate().is_ok());
    assert!(TimerManifest::from_bytes(br#"{"schemaVersion":1,"pages":[],"extra":true}"#).is_err());
    assert!(TimerManifest::from_bytes(br#"{"schemaVersion":1,"pages":[{"id":"home","timers":[{"id":"clock","tickNode":"tick","completeNode":"complete","statusNode":"status","intervalMs":1000.5,"completionDelayMs":0}]}]}"#).is_err());
    assert!(TimerManifest::from_bytes(&vec![b' '; 1024 * 1024 + 1]).is_err());
    for interval in [0, 999, 60_001] {
        let mut data = manifest();
        data.pages[0].timers[0].interval_ms = interval;
        assert!(data.validate().is_err());
        assert!(TimerSchedule::new(&data.pages[0].timers, Duration::ZERO).is_err());
    }
    let mut data = manifest();
    data.pages[0].timers[0].completion_delay_ms = 60_001;
    assert!(data.validate().is_err());
    data = manifest();
    data.schema_version = 2;
    assert!(data.validate().is_err());
    data = manifest();
    data.pages.push(data.pages[0].clone());
    assert!(data.validate().is_err());
    data = manifest();
    let duplicate = data.pages[0].timers[0].clone();
    data.pages[0].timers.push(duplicate);
    assert!(data.validate().is_err());
    data = manifest();
    data.pages[0].timers[0].status_node = data.pages[0].timers[0].tick_node.clone();
    assert!(data.validate().is_err());
    data = manifest();
    data.pages[0].timers[0].tick_node = "x".repeat(129);
    assert!(data.validate().is_err());
    data = manifest();
    data.pages[0].timers = (0..17)
        .map(|index| spec(&format!("timer{index}"), 1000, 0))
        .collect();
    assert!(data.validate().is_err());
    data = manifest();
    data.pages = (0..65)
        .map(|index| TimerPage {
            id: format!("page{index}"),
            timers: vec![],
        })
        .collect();
    assert!(data.validate().is_err());
}

#[test]
fn backwards_time_and_deadline_overflow_never_wrap_or_emit_early_completion() {
    let specs = [spec("clock", 1000, 700)];
    assert!(TimerSchedule::new(&specs, Duration::MAX).is_err());
    let mut schedule = TimerSchedule::new(&specs, Duration::from_secs(5)).unwrap();
    assert_eq!(
        schedule
            .poll(Duration::from_secs(7), &BTreeSet::new())
            .len(),
        2
    );
    assert!(
        schedule
            .poll(Duration::from_secs(6), &BTreeSet::new())
            .is_empty()
    );
    let completed = BTreeSet::from(["clock".to_owned()]);
    assert!(schedule.poll(Duration::from_secs(6), &completed).is_empty());
    assert!(
        schedule
            .poll(Duration::from_millis(7699), &completed)
            .is_empty()
    );
    assert_eq!(
        schedule.poll(Duration::from_millis(7700), &completed).len(),
        1
    );
    let mut exhausted = TimerSchedule::new(&specs, Duration::ZERO).unwrap();
    assert!(exhausted.poll(Duration::MAX, &completed).is_empty());
    assert!(exhausted.poll(Duration::MAX, &completed).is_empty());
}

#[test]
fn resetting_before_delayed_completion_cancels_exit_and_starts_a_fresh_interval() {
    let mut schedule = TimerSchedule::new(&[spec("clock", 1000, 700)], Duration::ZERO).unwrap();
    let completed = BTreeSet::from(["clock".to_owned()]);
    assert!(schedule.poll(Duration::from_secs(1), &completed).is_empty());
    assert!(
        schedule
            .poll(Duration::from_millis(1500), &BTreeSet::new())
            .is_empty()
    );
    assert!(
        schedule
            .poll(Duration::from_millis(1700), &BTreeSet::new())
            .is_empty()
    );
    assert!(
        schedule
            .poll(Duration::from_millis(2499), &BTreeSet::new())
            .is_empty()
    );
    let events = schedule.poll(Duration::from_millis(2500), &BTreeSet::new());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].node_id, "clock--tick");
    assert!(schedule.poll(Duration::from_secs(3), &completed).is_empty());
    assert_eq!(
        schedule.poll(Duration::from_millis(3700), &completed)[0].node_id,
        "clock--complete"
    );
    assert!(
        schedule
            .poll(Duration::from_secs(4), &BTreeSet::new())
            .is_empty()
    );
    assert!(
        schedule
            .poll(Duration::from_secs(10), &BTreeSet::new())
            .is_empty()
    );
}

#[test]
fn observing_completion_after_a_tick_starts_its_delay_without_consuming_events() {
    let mut schedule = TimerSchedule::new(&[spec("clock", 1000, 700)], Duration::ZERO).unwrap();
    assert_eq!(
        schedule
            .poll(Duration::from_secs(1), &BTreeSet::new())
            .len(),
        1
    );
    let completed = BTreeSet::from(["clock".to_owned()]);
    schedule.observe(Duration::from_secs(1), &completed);
    schedule.observe(Duration::from_millis(1500), &completed);
    assert!(
        schedule
            .poll(Duration::from_millis(1699), &completed)
            .is_empty()
    );
    assert_eq!(
        schedule.poll(Duration::from_millis(1700), &completed)[0].node_id,
        "clock--complete"
    );

    let mut ticking = TimerSchedule::new(&[spec("clock", 1000, 0)], Duration::ZERO).unwrap();
    ticking.observe(Duration::from_secs(2), &BTreeSet::new());
    assert_eq!(
        ticking.poll(Duration::from_secs(2), &BTreeSet::new()).len(),
        2
    );
}

#[test]
fn timer_wrapper_ids_cannot_alias_any_native_dispatch_or_status_node() {
    let mut data = manifest();
    data.pages[0].timers[0].id = data.pages[0].timers[0].tick_node.clone();
    assert!(data.validate().is_err());
}

#[test]
fn polling_one_event_allows_an_earlier_tick_to_cancel_another_timers_pending_exit() {
    let mut schedule = TimerSchedule::new(
        &[spec("resetter", 1000, 0), spec("clock", 1000, 1000)],
        Duration::ZERO,
    )
    .unwrap();
    let completed = BTreeSet::from(["clock".to_owned()]);
    schedule.observe(Duration::ZERO, &completed);
    let event = schedule
        .poll_one(Duration::from_secs(1), &completed)
        .unwrap();
    assert_eq!(event.node_id, "resetter--tick");
    // The resetter's real guest handler changes clock's checked status to running.
    // The clock completion must remain pending until after this host observation.
    schedule.observe(Duration::from_secs(1), &BTreeSet::new());
    assert!(
        schedule
            .poll_one(Duration::from_secs(1), &BTreeSet::new())
            .is_none()
    );
    assert_eq!(
        schedule
            .poll_one(Duration::from_secs(2), &BTreeSet::new())
            .unwrap()
            .node_id,
        "resetter--tick"
    );
    assert_eq!(
        schedule
            .poll_one(Duration::from_secs(2), &BTreeSet::new())
            .unwrap()
            .node_id,
        "clock--tick"
    );
    assert!(
        schedule
            .poll_one(Duration::from_secs(2), &BTreeSet::new())
            .is_none()
    );
}
