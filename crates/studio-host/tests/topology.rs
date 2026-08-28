#![allow(missing_docs)]

//! Deterministic in-process center/station topology tests.

use std::sync::Arc;

use studio_host::{
    ApplyResult, CenterId, CenterServer, CenterTopology, ConflictResolution, Durability,
    EmbeddedLocalStore, LocalStore, OperationId, PersistentCenter, Station, StationSettings,
    StationWriteResult, WriteOperation,
};

fn center() -> CenterServer {
    CenterServer::new(
        CenterId::new("restaurant").expect("center id"),
        CenterTopology::SelfHosted {
            endpoint: String::from("hub.local:4317"),
        },
    )
    .expect("center")
}

fn settings(name: &str) -> StationSettings {
    StationSettings::new(
        name,
        CenterTopology::SelfHosted {
            endpoint: String::from("hub.local:4317"),
        },
    )
    .expect("settings")
}

#[test]
fn pairing_tokens_are_scoped_single_use_and_logically_expiring() {
    let center = center();
    let token = center.issue_pairing_token().expect("token");
    let enrollment = center.pair(&token, "counter-a").expect("pair");
    assert_eq!(enrollment.station_id().as_str(), "station-1");
    assert_eq!(
        center
            .pair(&token, "counter-b")
            .expect_err("token is single-use")
            .code(),
        studio_host::TopologyErrorCode::PairingTokenUnknown
    );

    let expiring = center.issue_pairing_token().expect("expiring token");
    center.advance_pairing_clock(101).expect("clock");
    assert_eq!(
        center
            .pair(&expiring, "counter-c")
            .expect_err("expired token")
            .code(),
        studio_host::TopologyErrorCode::PairingTokenUnknown
    );
}

#[test]
fn two_stations_converge_on_authoritative_table_and_check_state() {
    let center = center();
    let token_a = center.issue_pairing_token().expect("token a");
    let token_b = center.issue_pairing_token().expect("token b");
    let mut station_a = Station::enroll(&center, token_a, settings("counter-a")).expect("a");
    let mut station_b = Station::enroll(&center, token_b, settings("counter-b")).expect("b");

    station_a
        .set("tables", "table-7", serde_json::json!({"open": true}))
        .expect("table write");
    station_b.sync().expect("b pulls table");
    station_b
        .set("checks", "check-12", serde_json::json!({"total": 2400}))
        .expect("check write");
    station_a.sync().expect("a pulls check");

    assert_eq!(
        station_a.local_state().cache().snapshot(),
        station_b.local_state().cache().snapshot()
    );
    assert_eq!(
        center.snapshot().expect("snapshot"),
        *station_a.local_state().cache().snapshot().expect("cache")
    );
}

#[test]
fn disconnected_writes_replay_once_logically_after_reconnect() {
    let center = center();
    let token = center.issue_pairing_token().expect("token");
    let mut station = Station::enroll(&center, token, settings("counter-a")).expect("station");
    station.disconnect();
    let queued = station
        .set("checks", "check-13", serde_json::json!({"total": 900}))
        .expect("queued write");
    let operation = match queued {
        StationWriteResult::Queued(operation) => operation,
        other => panic!("expected queued operation, got {other:?}"),
    };
    assert_eq!(station.pending_count(), 1);
    let replay = station.reconnect().expect("reconnect");
    assert!(matches!(
        replay.as_slice(),
        [StationWriteResult::Applied(_)]
    ));
    assert_eq!(station.pending_count(), 0);
    let revision = center.snapshot().expect("snapshot").revision();

    let duplicate = center
        .apply(station.enrollment(), &operation)
        .expect("duplicate submission");
    assert!(matches!(duplicate, ApplyResult::Replayed(_)));
    assert_eq!(center.snapshot().expect("snapshot").revision(), revision);
}

#[test]
fn station_local_state_serializes_settings_and_cache_but_not_authority_or_outbox() {
    let center = center();
    let token = center.issue_pairing_token().expect("token");
    let mut station = Station::enroll(&center, token, settings("counter-a")).expect("station");
    station.disconnect();
    station
        .set("checks", "check-private", serde_json::json!({"total": 44}))
        .expect("queued");

    let local = serde_json::to_value(station.local_state()).expect("local state is serializable");
    let rendered = local.to_string();
    assert!(rendered.contains("counter-a"));
    assert!(rendered.contains("snapshot"));
    assert!(!rendered.contains("check-private"));
    assert!(!rendered.contains("pending"));
    assert_eq!(station.pending_count(), 1);
}

#[test]
fn concurrent_stale_intents_remain_as_explicit_resolvable_conflicts() {
    let center = center();
    let token_a = center.issue_pairing_token().expect("token a");
    let token_b = center.issue_pairing_token().expect("token b");
    let mut station_a = Station::enroll(&center, token_a, settings("counter-a")).expect("a");
    let mut station_b = Station::enroll(&center, token_b, settings("counter-b")).expect("b");

    station_b.disconnect();
    station_b
        .set("checks", "check-14", serde_json::json!({"total": 1000}))
        .expect("queued");
    station_a
        .set("checks", "check-14", serde_json::json!({"total": 1200}))
        .expect("authoritative write");

    let replay = station_b.reconnect().expect("reconnect");
    let conflict = match replay.as_slice() {
        [StationWriteResult::Conflict { conflict, .. }] => conflict.clone(),
        other => panic!("expected conflict, got {other:?}"),
    };
    assert!(matches!(conflict.state(), studio_host::ConflictState::Open));
    assert_eq!(
        conflict.authoritative().expect("authority").value(),
        Some(&serde_json::json!({"total": 1200}))
    );
    assert_eq!(
        conflict.incoming_intent(),
        &studio_host::WriteIntent::Set(serde_json::json!({"total": 1000}))
    );

    center
        .resolve_conflict(
            station_a.enrollment(),
            conflict.id(),
            OperationId::new("resolve-check-14").expect("resolution id"),
            ConflictResolution::ApplyIncoming,
        )
        .expect("resolve");
    station_a.sync().expect("a sync");
    station_b.sync().expect("b sync");
    assert_eq!(
        station_a.local_state().cache().snapshot(),
        station_b.local_state().cache().snapshot()
    );
    assert!(matches!(
        station_a
            .local_state()
            .cache()
            .snapshot()
            .expect("cache")
            .conflicts()[0]
            .state(),
        studio_host::ConflictState::Resolved { .. }
    ));
}

#[test]
fn acknowledged_state_survives_center_restart_through_local_store() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let store = Arc::new(
            EmbeddedLocalStore::open(directory.path(), Durability::Every)
                .await
                .expect("store"),
        );
        let persistent = PersistentCenter::open(
            CenterId::new("restaurant").expect("center id"),
            CenterTopology::StudioCloud {
                namespace: String::from("acct-1/restaurant"),
            },
            store.clone(),
        )
        .await
        .expect("persistent center");
        let token = persistent.issue_pairing_token().await.expect("token");
        let enrollment = persistent.pair(&token, "counter-a").await.expect("pair");
        let operation = WriteOperation::set(
            OperationId::new("station-1:1").expect("operation id"),
            "checks",
            "check-15",
            0,
            serde_json::json!({"total": 3300}),
        )
        .expect("operation");
        persistent
            .apply(&enrollment, &operation)
            .await
            .expect("apply");
        let before = persistent.snapshot().expect("snapshot");
        drop(persistent);
        let store = Arc::try_unwrap(store).unwrap_or_else(|_| panic!("store still referenced"));
        store.close().await.expect("close");

        let reopened_store = Arc::new(
            EmbeddedLocalStore::recover(directory.path(), Durability::Every)
                .await
                .expect("recover"),
        );
        let reopened = PersistentCenter::open(
            CenterId::new("restaurant").expect("center id"),
            CenterTopology::StudioCloud {
                namespace: String::from("acct-1/restaurant"),
            },
            reopened_store.clone(),
        )
        .await
        .expect("reopen");
        assert_eq!(reopened.snapshot().expect("snapshot"), before);
        assert!(matches!(
            reopened
                .apply(&enrollment, &operation)
                .await
                .expect("replay"),
            ApplyResult::Replayed(_)
        ));
        drop(reopened);
        let reopened_store =
            Arc::try_unwrap(reopened_store).unwrap_or_else(|_| panic!("store still referenced"));
        reopened_store.close().await.expect("close");
    });
}
