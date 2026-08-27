#![allow(missing_docs)]

use studio_flagship::{
    run_demo_day, BillingAllocation, BillingEdit, BillingEngine, BillingOutcome, BillingVariant,
    CenterTopology, KitchenPrinterAdapter, KitchenTicket, PublishDisposition, RestRoute,
    StripeSandboxAdapter, FakeRestBroker,
};

#[test]
fn demo_day_report_passes_deterministic_gates_but_keeps_external_blockers_visible() {
    let report = run_demo_day();

    assert!(report.all_gates_passed());
    assert!(!report.release_ready);
    assert!(report.determinism.repeated_run_equal);
    assert!(!report.determinism.digest.is_empty());
    assert_eq!(report.center.converged_check_line_count, 3);
    assert_eq!(report.center.duplicate_replay_count, 1);
    assert!(report.payroll.matches_tracking);
    assert!(report.billing.all_variants);
    assert!(report.authoring.grouped_undo);
    assert!(report.audit.complete);
    assert!(report.audit.secrets_absent);
    assert!(report.security.secret_free_report);
    assert!(report.verification_gaps.iter().any(|gap| gap.gate == "stripe_sandbox" && gap.blocking));
    assert!(report.prerequisites.iter().any(|item| item.ticket == 25 && item.status == "not_integrated"));
}

#[test]
fn report_round_trips_as_machine_readable_json() {
    let report = run_demo_day();
    let json = report.to_json().expect("report serializes");
    let decoded: studio_flagship::ReleaseEvidenceReport = serde_json::from_str(&json).expect("report round-trips");
    assert_eq!(decoded, report);
}

#[test]
fn disconnected_station_replays_duplicate_event_once() {
    let mut center = CenterTopology::default();
    center.disconnect("terminal-table");
    let event = studio_flagship::RestaurantEvent {
        event_id: "offline-1".to_owned(),
        station_id: "terminal-table".to_owned(),
        check_id: "check-1".to_owned(),
        table_id: "table-1".to_owned(),
        line: Some(studio_flagship::OrderLine { item: "tea".to_owned(), seat: "seat-1".to_owned(), quantity: 1 }),
    };
    assert_eq!(center.publish(event.clone()), PublishDisposition::Queued);
    assert_eq!(center.publish(event), PublishDisposition::Queued);
    assert_eq!(center.reconnect_and_reconcile("terminal-table"), (1, 1));
    assert_eq!(center.state().checks["check-1"].lines.len(), 1);
}

#[test]
fn stale_billing_edit_is_reported_without_last_writer_wins() {
    let mut billing = BillingEngine::new(1_000);
    assert!(matches!(billing.apply(BillingEdit { base_revision: 0, variant: BillingVariant::Single, allocations: vec![BillingAllocation { label: "whole".to_owned(), amount_minor: 1_000 }] }), BillingOutcome::Applied { revision: 1 }));
    assert_eq!(billing.apply(BillingEdit { base_revision: 0, variant: BillingVariant::Split, allocations: vec![BillingAllocation { label: "stale".to_owned(), amount_minor: 1_000 }] }), BillingOutcome::Conflict { expected: 0, actual: 1 });
}

#[test]
fn stripe_adapter_rejects_undeclared_routes_and_fake_peripheral_is_structured() {
    let mut broker = FakeRestBroker::default();
    assert!(StripeSandboxAdapter::new(&mut broker).charge(1_000).is_err());
    broker.declare(RestRoute::post("/v1/payment_intents"));
    assert!(StripeSandboxAdapter::new(&mut broker).charge(1_000).is_ok());
    assert_eq!(broker.credential_reads(), 0);

    let mut peripheral = studio_flagship::FakePeripheralAdapters::default();
    let job = peripheral.print_kitchen_ticket(&KitchenTicket { ticket_id: "ticket-1".to_owned(), check_id: "check-1".to_owned(), item_count: 1 });
    assert!(job.structured);
}
