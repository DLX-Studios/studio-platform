use serde_json::json;
use studio_host::{
    AuditEvent, AuditEventType, AuditLog, AuditLogErrorCode, AuditQuery, Durability,
    EmbeddedLocalStore, LocalStore, StoreBatch,
};
use studio_security::{PluginPrincipal, SensitiveValueFilter, TrustMode};
use tempfile::tempdir;

fn principal() -> PluginPrincipal {
    PluginPrincipal::new_verified(
        "publisher.example",
        "key-1",
        "restaurant-pos",
        [7; 32],
        [9; 16],
        TrustMode::Production,
    )
    .expect("fixture principal is valid")
}

#[tokio::test]
async fn all_security_event_classes_are_append_only_queryable_and_redacted() {
    let directory = tempdir().expect("temporary directory is created");
    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let mut filter = SensitiveValueFilter::new();
    filter
        .register_secret(b"super-secret-value")
        .expect("fixture secret is valid");
    let log = AuditLog::with_filter(store, &principal(), filter);

    let events = [
        AuditEvent::authentication_attempt("user:alice", 1_000, true).expect("event is valid"),
        AuditEvent::role_change(
            "user:alice",
            2_000,
            json!({ "role": "manager", "password": "super-secret-value" }),
        )
        .expect("event is valid"),
        AuditEvent::membership_change("user:alice", 3_000, json!({ "member": "bob" }))
            .expect("event is valid"),
        AuditEvent::destructive_action("user:alice", 4_000, json!({ "resource": "order-1" }))
            .expect("event is valid"),
        AuditEvent::data_export("user:alice", 5_000, json!({ "rows": 12 }))
            .expect("event is valid"),
        AuditEvent::webhook_admission("webhook:orders", 6_000, json!({ "accepted": true }))
            .expect("event is valid"),
        AuditEvent::workflow_run("workflow:close-day", 7_000, json!({ "success": true }))
            .expect("event is valid"),
    ];
    for event in events {
        log.append(event).await.expect("event appends");
    }

    let records = log
        .query(
            AuditQuery::new()
                .time_range(Some(2_000), Some(6_000))
                .with_actor("user:alice")
                .with_limit(3),
        )
        .await
        .expect("query succeeds");
    assert_eq!(records.len(), 3);
    assert_eq!(records[0].sequence(), 1);
    assert_eq!(records[2].event_type(), AuditEventType::DestructiveAction);
    assert_eq!(records[1].details()["password"], "[REDACTED]");
    assert_ne!(records[0].hash(), records[1].hash());
    assert_eq!(records[1].previous_hash(), records[0].hash());

    let export = log
        .export(AuditQuery::default())
        .await
        .expect("export succeeds");
    assert!(export.contains("authentication_attempt"));
    assert!(export.contains("workflow_run"));
    assert!(export.contains("[REDACTED]"));
    assert!(!export.contains("super-secret-value"));
    log.verify_integrity().await.expect("chain verifies");

    log.into_inner().close().await.expect("store closes");
}

#[tokio::test]
async fn tampered_record_is_rejected_after_reopen() {
    let directory = tempdir().expect("temporary directory is created");
    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let log = AuditLog::new(store, &principal());
    log.append(
        AuditEvent::new(
            AuditEventType::DataExport,
            "user:owner",
            42,
            json!({ "rows": 3 }),
        )
        .expect("event is valid"),
    )
    .await
    .expect("event appends");
    let batch_id = log.storage_id().to_owned();
    let store = log.into_inner();
    let mut entries = store
        .batch_entries(&batch_id)
        .await
        .expect("audit batch reads");
    entries[1].payload["details"]["rows"] = json!(999);
    let tampered = StoreBatch::new(batch_id.clone(), entries).expect("batch remains well-formed");
    store
        .write_batch(&tampered)
        .await
        .expect("fixture tamper writes through low-level host seam");
    store.close().await.expect("store closes");

    let reopened = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store reopens");
    let log = AuditLog::new(reopened, &principal());
    let error = log
        .query(AuditQuery::default())
        .await
        .expect_err("tamper is rejected");
    assert_eq!(error.diagnostic().code(), AuditLogErrorCode::Tampered);
    log.into_inner().close().await.expect("store closes");
}

#[tokio::test]
async fn invalid_events_and_queries_fail_with_safe_codes() {
    let directory = tempdir().expect("temporary directory is created");
    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let log = AuditLog::new(store, &principal());
    let invalid = AuditEvent::new(AuditEventType::WorkflowRun, "", 1, json!({}))
        .expect_err("empty actor rejected");
    assert_eq!(
        invalid.diagnostic().code(),
        AuditLogErrorCode::RequestInvalid
    );
    let invalid_query = log
        .query(AuditQuery::new().time_range(Some(5), Some(4)))
        .await
        .expect_err("reversed range rejected");
    assert_eq!(
        invalid_query.diagnostic().code(),
        AuditLogErrorCode::RequestInvalid
    );
    log.into_inner().close().await.expect("store closes");
}
