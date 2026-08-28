#![allow(missing_docs)]

use std::collections::BTreeMap;

use serde_json::json;
use studio_host::{
    ApplicationDataErrorCode, ApplicationDataGuestApi, ApplicationDataHost, CollectionDeclaration,
    Durability, EmbeddedLocalStore, FieldDeclaration, FieldType, ForbiddenDataOperation,
    GuestDataRequest, LocalStore, PatchOperation, RecordId, RecordSchema,
};
use studio_security::{PluginPrincipal, TrustMode};
use tempfile::tempdir;

fn principal(publisher: &str, key: &str, application: &str) -> PluginPrincipal {
    PluginPrincipal::new_verified(
        publisher,
        key,
        application,
        [7; 32],
        [9; 16],
        TrustMode::Production,
    )
    .expect("fixture principal is valid")
}

fn catalog_declaration() -> CollectionDeclaration {
    CollectionDeclaration::new(
        "catalog",
        RecordSchema::new(
            1,
            BTreeMap::from([
                (
                    "name".to_owned(),
                    FieldDeclaration::required(FieldType::String),
                ),
                (
                    "price_cents".to_owned(),
                    FieldDeclaration::required(FieldType::Integer),
                ),
                (
                    "category".to_owned(),
                    FieldDeclaration::optional(FieldType::String),
                ),
                (
                    "available".to_owned(),
                    FieldDeclaration::required(FieldType::Boolean),
                ),
            ]),
        )
        .expect("fixture schema is valid"),
    )
    .expect("fixture collection is valid")
}

fn cart_declaration() -> CollectionDeclaration {
    CollectionDeclaration::new(
        "cart",
        RecordSchema::new(
            1,
            BTreeMap::from([
                (
                    "catalog_item_id".to_owned(),
                    FieldDeclaration::required(FieldType::String),
                ),
                (
                    "quantity".to_owned(),
                    FieldDeclaration::required(FieldType::Integer),
                ),
            ]),
        )
        .expect("fixture schema is valid"),
    )
    .expect("fixture collection is valid")
}

fn id(value: &str) -> RecordId {
    RecordId::new(value).expect("fixture id is valid")
}

fn p95_ms(samples: &mut [f64]) -> f64 {
    assert!(!samples.is_empty(), "timing samples are required");
    samples.sort_by(|a, b| a.partial_cmp(b).expect("finite duration"));
    let rank = samples.len().saturating_mul(95).div_ceil(100);
    samples[rank.saturating_sub(1).min(samples.len() - 1)]
}

#[tokio::test]
async fn namespaces_isolate_apps_and_forbidden_guest_paths_fail_closed() {
    let directory = tempdir().expect("temporary directory is created");
    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let host = ApplicationDataHost::new(store);

    {
        let alpha_principal = principal("publisher.example", "key-old", "pos-alpha");
        let rotated_alpha = principal("publisher.example", "key-new", "pos-alpha");
        let beta_principal = principal("publisher.example", "key-old", "pos-beta");
        let alpha = host
            .bind(&alpha_principal, [catalog_declaration()])
            .expect("alpha binds");
        let alpha_after_rotation = host
            .bind(&rotated_alpha, [catalog_declaration()])
            .expect("rotated alpha binds");
        let beta = host
            .bind(&beta_principal, [catalog_declaration()])
            .expect("beta binds");

        assert_eq!(alpha.namespace(), alpha_after_rotation.namespace());
        assert_ne!(alpha.namespace(), beta.namespace());
        alpha
            .create(
                "catalog",
                id("coffee"),
                json!({
                    "name": "Coffee",
                    "price_cents": 350,
                    "available": true
                }),
            )
            .await
            .expect("alpha creates its record");

        assert!(
            beta.select("catalog", id("coffee"))
                .await
                .expect("beta reads only its partition")
                .is_none()
        );
        assert_eq!(
            beta.authorize_namespace(alpha.namespace())
                .expect_err("cross-namespace attribution is denied")
                .code(),
            ApplicationDataErrorCode::CrossNamespaceDenied
        );

        for (operation, expected) in [
            (
                ForbiddenDataOperation::RawQuery,
                ApplicationDataErrorCode::RawQueryDenied,
            ),
            (
                ForbiddenDataOperation::NamespaceSwitch,
                ApplicationDataErrorCode::NamespaceSwitchDenied,
            ),
            (
                ForbiddenDataOperation::DatabaseSwitch,
                ApplicationDataErrorCode::DatabaseSwitchDenied,
            ),
        ] {
            assert_eq!(
                beta.execute(GuestDataRequest::Forbidden(operation))
                    .await
                    .expect_err("forbidden guest path is denied")
                    .code(),
                expected
            );
        }
    }

    host.into_inner().close().await.expect("store closes");
}

#[tokio::test]
async fn declared_schema_supports_select_create_merge_patch_list_and_delete() {
    let directory = tempdir().expect("temporary directory is created");
    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let host = ApplicationDataHost::new(store);

    {
        let app = principal("publisher.example", "key", "pos");
        let data = host
            .bind(&app, [catalog_declaration()])
            .expect("application binds");

        assert_eq!(
            data.create(
                "catalog",
                id("invalid"),
                json!({ "name": "Missing fields" }),
            )
            .await
            .expect_err("incomplete record is rejected")
            .code(),
            ApplicationDataErrorCode::SchemaViolation
        );

        data.create(
            "catalog",
            id("latte"),
            json!({
                "name": "Latte",
                "price_cents": 450,
                "category": "drinks",
                "available": true
            }),
        )
        .await
        .expect("record is created");

        let merged = data
            .update_merge("catalog", id("latte"), json!({ "price_cents": 475 }))
            .await
            .expect("record is merged");
        assert_eq!(merged.value["price_cents"], 475);

        let patched = data
            .update_patch(
                "catalog",
                id("latte"),
                vec![
                    PatchOperation::Set {
                        field: "available".to_owned(),
                        value: json!(false),
                    },
                    PatchOperation::Remove {
                        field: "category".to_owned(),
                    },
                ],
            )
            .await
            .expect("record is patched");
        assert_eq!(patched.value["available"], false);
        assert!(patched.value.get("category").is_none());

        let selected = data
            .select("catalog", id("latte"))
            .await
            .expect("record is selected")
            .expect("record exists");
        assert_eq!(selected, patched);
        assert_eq!(
            data.list("catalog").await.expect("collection lists"),
            vec![patched]
        );
        assert!(
            data.delete("catalog", id("latte"))
                .await
                .expect("record deletes")
        );
        assert!(
            data.list("catalog")
                .await
                .expect("collection lists")
                .is_empty()
        );
    }

    host.into_inner().close().await.expect("store closes");
}

#[tokio::test]
async fn pos_catalog_and_cart_persist_across_store_reopen() {
    let directory = tempdir().expect("temporary directory is created");
    let app = principal("publisher.example", "key", "restaurant-pos");

    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let host = ApplicationDataHost::new(store);
    {
        let data = host
            .bind(&app, [catalog_declaration(), cart_declaration()])
            .expect("application binds");
        for (item_id, name, price) in [
            ("burger", "Burger", 1299),
            ("fries", "Fries", 499),
            ("shake", "Shake", 599),
        ] {
            data.create(
                "catalog",
                id(item_id),
                json!({
                    "name": name,
                    "price_cents": price,
                    "category": "menu",
                    "available": true
                }),
            )
            .await
            .expect("catalog item persists");
        }
        data.create(
            "cart",
            id("line-1"),
            json!({ "catalog_item_id": "burger", "quantity": 2 }),
        )
        .await
        .expect("cart line persists");
    }
    host.into_inner().close().await.expect("store closes");

    let reopened = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store reopens");
    let host = ApplicationDataHost::new(reopened);
    {
        let data = host
            .bind(&app, [catalog_declaration(), cart_declaration()])
            .expect("application rebinds");
        let records = data.list("catalog").await.expect("catalog reloads");
        assert_eq!(
            records
                .iter()
                .map(|record| record.id.as_str())
                .collect::<Vec<_>>(),
            ["burger", "fries", "shake"]
        );
        assert_eq!(records[0].value["price_cents"], 1299);
        let cart = data.list("cart").await.expect("cart reloads");
        assert_eq!(cart.len(), 1);
        assert_eq!(cart[0].value["catalog_item_id"], "burger");
        assert_eq!(cart[0].value["quantity"], 2);
    }
    host.into_inner().close().await.expect("store closes");
}

#[tokio::test]
async fn cross_principal_denial_is_deterministic_and_diagnostics_are_safe() {
    let directory = tempdir().expect("temporary directory is created");
    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let host = ApplicationDataHost::new(store);

    {
        // Four principals: same publisher/app with rotated key must retain partition;
        // different publishers or applications must be isolated.
        let acme_pos = principal("acme.example", "key-v1", "restaurant-pos");
        let acme_pos_rotated = principal("acme.example", "key-v2", "restaurant-pos");
        let acme_other = principal("acme.example", "key-v1", "other-app");
        let other_pos = principal("other.example", "key-v1", "restaurant-pos");

        let h_acme = host
            .bind(&acme_pos, [catalog_declaration(), cart_declaration()])
            .expect("acme/pos binds");
        let h_acme_rotated = host
            .bind(
                &acme_pos_rotated,
                [catalog_declaration(), cart_declaration()],
            )
            .expect("acme/pos rotated binds");
        let h_acme_other = host
            .bind(&acme_other, [catalog_declaration(), cart_declaration()])
            .expect("acme/other binds");
        let h_other = host
            .bind(&other_pos, [catalog_declaration(), cart_declaration()])
            .expect("other/pos binds");

        // Key rotation stability: digest unchanged.
        assert_eq!(
            h_acme.namespace(),
            h_acme_rotated.namespace(),
            "signing-key rotation retains namespace"
        );
        // Distinct principals are distinct.
        assert_ne!(
            h_acme.namespace(),
            h_acme_other.namespace(),
            "different app is isolated"
        );
        assert_ne!(
            h_acme.namespace(),
            h_other.namespace(),
            "different publisher is isolated"
        );
        assert_ne!(
            h_acme_other.namespace(),
            h_other.namespace(),
            "both publisher and app differences are isolated"
        );

        // Opaque namespace: Debug must not leak digest hex.
        let debug = format!("{:?}", h_acme.namespace());
        assert!(
            !debug.contains("appdata"),
            "namespace debug is non-exhaustive and does not leak storage prefix"
        );

        // Each principal creates a distinct record; others cannot observe it.
        h_acme
            .create(
                "catalog",
                id("shared-id"),
                json!({"name": "ACME Pos", "price_cents": 100, "available": true}),
            )
            .await
            .expect("acme pos creates");
        h_acme_other
            .create(
                "catalog",
                id("shared-id"),
                json!({"name": "ACME Other", "price_cents": 200, "available": false}),
            )
            .await
            .expect("acme other creates");
        h_other
            .create(
                "catalog",
                id("shared-id"),
                json!({"name": "Other Pos", "price_cents": 300, "available": true}),
            )
            .await
            .expect("other pos creates");

        let acme_record = h_acme
            .select("catalog", id("shared-id"))
            .await
            .expect("select succeeds")
            .expect("record exists");
        assert_eq!(acme_record.value["name"], "ACME Pos");
        assert_eq!(
            h_acme_other
                .select("catalog", id("shared-id"))
                .await
                .expect("select succeeds")
                .expect("record exists")
                .value["name"],
            "ACME Other"
        );

        // Cross listing remains empty until that namespace writes; no cross leak.
        let fresh = principal("fresh.example", "key", "fresh-app");
        let h_fresh = host
            .bind(&fresh, [catalog_declaration()])
            .expect("fresh binds");
        assert!(
            h_fresh
                .list("catalog")
                .await
                .expect("fresh list succeeds")
                .is_empty(),
            "fresh namespace sees no foreign records"
        );

        // Deterministic cross-namespace denial via host-attributed check.
        let pairs = [
            (&h_acme, h_acme_other.namespace()),
            (&h_acme, h_other.namespace()),
            (&h_acme_other, h_acme.namespace()),
            (&h_other, h_acme.namespace()),
            (&h_fresh, h_acme.namespace()),
        ];
        for (handle, foreign) in pairs {
            for _ in 0..5 {
                let err = handle
                    .authorize_namespace(foreign)
                    .expect_err("cross namespace is denied");
                assert_eq!(err.code(), ApplicationDataErrorCode::CrossNamespaceDenied);
                let msg = err.to_string();
                assert_eq!(msg, "application namespace denied");
                assert!(!msg.contains("appdata"));
                assert!(!msg.contains("catalog"));
            }
        }
        assert!(h_acme.authorize_namespace(h_acme.namespace()).is_ok());

        // Forbidden guest operations fail closed with stable codes and safe messages.
        for (op, expected_code, expected_msg) in [
            (
                ForbiddenDataOperation::RawQuery,
                ApplicationDataErrorCode::RawQueryDenied,
                "raw application data query denied",
            ),
            (
                ForbiddenDataOperation::NamespaceSwitch,
                ApplicationDataErrorCode::NamespaceSwitchDenied,
                "namespace switch denied",
            ),
            (
                ForbiddenDataOperation::DatabaseSwitch,
                ApplicationDataErrorCode::DatabaseSwitchDenied,
                "database switch denied",
            ),
        ] {
            for _ in 0..3 {
                let err = h_acme
                    .execute(GuestDataRequest::Forbidden(op))
                    .await
                    .expect_err("forbidden path is denied");
                assert_eq!(err.code(), expected_code);
                assert_eq!(err.to_string(), expected_msg);
                assert!(!err.to_string().contains("SELECT"));
                assert!(!err.to_string().contains("appdata"));
            }
        }

        // Schema rejection is typed and safe: no offending value in diagnostic.
        let sensitive_name = "SENSITIVE_PAYLOAD_123";
        let err = h_acme
            .create(
                "catalog",
                id("bad-type"),
                json!({"name": sensitive_name, "price_cents": "not-an-int", "available": true}),
            )
            .await
            .expect_err("type mismatch is rejected");
        assert_eq!(err.code(), ApplicationDataErrorCode::SchemaViolation);
        let msg = err.to_string();
        assert_eq!(msg, "record schema validation failed");
        assert!(
            !msg.contains(sensitive_name),
            "safe diagnostic must not contain record payload"
        );
        assert!(
            !format!("{err:?}").contains(sensitive_name),
            "debug must not contain payload"
        );

        // Extra field is also schema violation and safe.
        let err = h_acme
            .create(
                "catalog",
                id("extra-field"),
                json!({"name": "X", "price_cents": 100, "available": true, "secret": "leak"}),
            )
            .await
            .expect_err("extra field is rejected");
        assert_eq!(err.code(), ApplicationDataErrorCode::SchemaViolation);
        assert_eq!(err.to_string(), "record schema validation failed");

        // Missing required field is schema violation.
        let err = h_acme
            .create(
                "catalog",
                id("missing-field"),
                json!({"name": "X", "available": true}),
            )
            .await
            .expect_err("missing field is rejected");
        assert_eq!(err.code(), ApplicationDataErrorCode::SchemaViolation);

        // Undeclared collection is stable and safe.
        let err = h_acme
            .select("undeclared", id("any"))
            .await
            .expect_err("undeclared collection is rejected");
        assert_eq!(err.code(), ApplicationDataErrorCode::CollectionUndeclared);
        assert_eq!(err.to_string(), "collection not declared");

        // Patch removing required field is schema violation and safe.
        h_acme
            .create(
                "catalog",
                id("patch-target"),
                json!({"name": "Patch", "price_cents": 100, "available": true}),
            )
            .await
            .expect("create for patch");
        let err = h_acme
            .update_patch(
                "catalog",
                id("patch-target"),
                vec![PatchOperation::Remove {
                    field: "name".to_owned(),
                }],
            )
            .await
            .expect_err("removing required field is rejected");
        assert_eq!(err.code(), ApplicationDataErrorCode::SchemaViolation);
        assert_eq!(err.to_string(), "record schema validation failed");

        // Record not found is stable and safe.
        let err = h_acme
            .update_merge("catalog", id("absent"), json!({"price_cents": 200}))
            .await
            .expect_err("missing record is not found");
        assert_eq!(err.code(), ApplicationDataErrorCode::RecordNotFound);
        assert_eq!(err.to_string(), "record not found");
    }

    host.into_inner().close().await.expect("store closes");
}

#[tokio::test]
async fn throughput_smoke_evidence_is_conservative_and_reported() {
    use std::time::Instant;

    let directory = tempdir().expect("temporary directory is created");
    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let host = ApplicationDataHost::new(store);

    {
        let app = principal("bench.example", "bench-key", "throughput-app");
        let bench_schema = RecordSchema::new(
            1,
            BTreeMap::from([
                (
                    "name".to_owned(),
                    FieldDeclaration::required(FieldType::String),
                ),
                (
                    "price_cents".to_owned(),
                    FieldDeclaration::required(FieldType::Integer),
                ),
                (
                    "available".to_owned(),
                    FieldDeclaration::required(FieldType::Boolean),
                ),
            ]),
        )
        .expect("bench schema is valid");
        let bench_decl =
            CollectionDeclaration::new("bench", bench_schema).expect("bench collection is valid");
        let data = host.bind(&app, [bench_decl]).expect("bench binds");

        // Warmup: 5 operations so RocksDB/SurrealDB page caches are hot before measurement,
        // matching the "after warmup" qualifier in the preliminary budget.
        for i in 0..5 {
            data.create(
                "bench",
                id(&format!("warm-{i}")),
                json!({"name": "Warm", "price_cents": 100, "available": true}),
            )
            .await
            .expect("warmup create");
        }
        for i in 0..5 {
            data.select("bench", id(&format!("warm-{i}")))
                .await
                .expect("warmup select");
        }
        for i in 0..5 {
            data.delete("bench", id(&format!("warm-{i}")))
                .await
                .expect("warmup cleanup");
        }

        // Build a 250-record corpus (conservative smoke for the 1,000-record
        // preliminary budget; enough to exercise ordered list and per-record
        // read/modify/write without making CI flaky).
        let corpus_size = 250usize;
        let mut create_ms = Vec::with_capacity(corpus_size);
        let total_wall = Instant::now();
        for i in 0..corpus_size {
            let label = format!("item-{i:04}");
            let start = Instant::now();
            data.create(
                "bench",
                id(&label),
                json!({"name": "Bench Item", "price_cents": 999, "available": true}),
            )
            .await
            .expect("corpus create");
            create_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        // Sampled reads: 100 selects
        let mut select_ms = Vec::with_capacity(100);
        for i in (0..100).map(|k| k * 2) {
            let label = format!("item-{i:04}");
            let start = Instant::now();
            let rec = data
                .select("bench", id(&label))
                .await
                .expect("select succeeds")
                .expect("record exists");
            assert_eq!(rec.value["name"], "Bench Item");
            select_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        // Sampled updates: 50 merges and 50 patches
        let mut merge_ms = Vec::with_capacity(50);
        for i in 0..50 {
            let label = format!("item-{i:04}");
            let start = Instant::now();
            data.update_merge("bench", id(&label), json!({"price_cents": 1234}))
                .await
                .expect("merge succeeds");
            merge_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        let mut patch_ms = Vec::with_capacity(50);
        for i in 50..100 {
            let label = format!("item-{i:04}");
            let start = Instant::now();
            data.update_patch(
                "bench",
                id(&label),
                vec![PatchOperation::Set {
                    field: "available".to_owned(),
                    value: json!(false),
                }],
            )
            .await
            .expect("patch succeeds");
            patch_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        // List evidence: 20 full ordered lists of 250 records after warmup
        let mut list_ms = Vec::with_capacity(20);
        for _ in 0..20 {
            let start = Instant::now();
            let listed = data.list("bench").await.expect("list succeeds");
            assert_eq!(
                listed.len(),
                corpus_size,
                "ordered list returns full corpus"
            );
            assert_eq!(listed.first().unwrap().id.as_str(), "item-0000");
            assert_eq!(
                listed.last().unwrap().id.as_str(),
                format!("item-{:04}", corpus_size - 1)
            );
            list_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        // Deletes: 50 deletes
        let mut delete_ms = Vec::with_capacity(50);
        for i in 100..150 {
            let label = format!("item-{i:04}");
            let start = Instant::now();
            assert!(
                data.delete("bench", id(&label))
                    .await
                    .expect("delete succeeds"),
                "record existed"
            );
            delete_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        let total_elapsed = total_wall.elapsed();
        let total_ops = create_ms.len()
            + select_ms.len()
            + merge_ms.len()
            + patch_ms.len()
            + list_ms.len()
            + delete_ms.len();

        let p95_select = p95_ms(&mut select_ms);
        let p95_create = p95_ms(&mut create_ms);
        let p95_merge = p95_ms(&mut merge_ms);
        let p95_patch = p95_ms(&mut patch_ms);
        let p95_list = p95_ms(&mut list_ms);
        let p95_delete = p95_ms(&mut delete_ms);

        eprintln!(
            "application-data throughput evidence (Durability::Every, corpus={}, total_ops={}, total_wall_ms={:.1}): \
             select p95={:.2}ms (n={}), create p95={:.2}ms (n={}), update_merge p95={:.2}ms (n={}), \
             update_patch p95={:.2}ms (n={}), delete p95={:.2}ms (n={}), list p95={:.2}ms (n={}) | \
             original qualification targets (STUDIO-BENCH-1, 1k records): select<=10ms create/update/delete<=25ms list<=75ms | \
             timing is informational smoke evidence only",
            corpus_size,
            total_ops,
            total_elapsed.as_secs_f64() * 1000.0,
            p95_select,
            select_ms.len(),
            p95_create,
            create_ms.len(),
            p95_merge,
            merge_ms.len(),
            p95_patch,
            patch_ms.len(),
            p95_delete,
            delete_ms.len(),
            p95_list,
            list_ms.len(),
        );

        let remaining = data.list("bench").await.expect("final list");
        assert_eq!(
            remaining.len(),
            corpus_size - 50,
            "delete count reflected in final list"
        );
    }

    host.into_inner().close().await.expect("store closes");
}

/// The only in-repo path that can claim STUDIO-BENCH-1 qualification.
///
/// This test is intentionally ignored and must be run as a serialized test process with explicit
/// baseline designation and hardware metadata, for example:
///
/// ```text
/// STUDIO_BENCH_PROFILE=STUDIO-BENCH-1 \
/// STUDIO_BENCH_MACHINE='Intel N100 / 8 GiB / Intel UHD / Weston' \
/// STUDIO_BENCH_STORAGE='NVMe' \
/// STUDIO_BENCH_DURABILITY=Every \
/// CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR=/home/sir/.cache/studio-platform-target \
/// cargo test --locked -p studio-host --test application_data studio_bench_1_qualification \
/// -- --ignored --exact --test-threads=1 --nocapture
/// ```
///
/// A generic host cannot silently become the baseline: the test refuses to start without the
/// explicit profile gate and all required metadata.
#[tokio::test]
#[ignore = "explicit STUDIO-BENCH-1 qualification; requires serialized invocation and metadata"]
#[allow(clippy::too_many_lines)]
async fn studio_bench_1_qualification() {
    use std::{env, time::Instant};

    const PROFILE_ENV: &str = "STUDIO_BENCH_PROFILE";
    const MACHINE_ENV: &str = "STUDIO_BENCH_MACHINE";
    const STORAGE_ENV: &str = "STUDIO_BENCH_STORAGE";
    const DURABILITY_ENV: &str = "STUDIO_BENCH_DURABILITY";
    const PROFILE: &str = "STUDIO-BENCH-1";
    const CORPUS_SIZE: usize = 1_000;
    const SAMPLES_PER_VERB: usize = 1_000;
    const SELECT_P95_TARGET_MS: f64 = 10.0;
    const WRITE_P95_TARGET_MS: f64 = 25.0;
    const LIST_P95_TARGET_MS: f64 = 75.0;

    let profile = env::var(PROFILE_ENV)
        .unwrap_or_else(|_| panic!("{PROFILE_ENV}={PROFILE:?} is required for qualification"));
    assert_eq!(
        profile, PROFILE,
        "qualification refuses non-baseline profile {profile:?}"
    );
    let machine = env::var(MACHINE_ENV)
        .unwrap_or_else(|_| panic!("{MACHINE_ENV} metadata is required for qualification"));
    let storage = env::var(STORAGE_ENV)
        .unwrap_or_else(|_| panic!("{STORAGE_ENV} metadata is required for qualification"));
    let durability = env::var(DURABILITY_ENV)
        .unwrap_or_else(|_| panic!("{DURABILITY_ENV} metadata is required for qualification"));
    assert_eq!(
        durability, "Every",
        "qualification target requires Durability::Every, got {durability:?}"
    );
    assert!(
        !machine.trim().is_empty(),
        "{MACHINE_ENV} metadata cannot be empty"
    );
    assert!(
        !storage.trim().is_empty(),
        "{STORAGE_ENV} metadata cannot be empty"
    );

    let directory = tempdir().expect("temporary directory is created");
    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let host = ApplicationDataHost::new(store);

    {
        let app = principal("bench.example", "bench-key", "throughput-app");
        let bench_schema = RecordSchema::new(
            1,
            BTreeMap::from([
                (
                    "name".to_owned(),
                    FieldDeclaration::required(FieldType::String),
                ),
                (
                    "price_cents".to_owned(),
                    FieldDeclaration::required(FieldType::Integer),
                ),
                (
                    "available".to_owned(),
                    FieldDeclaration::required(FieldType::Boolean),
                ),
            ]),
        )
        .expect("bench schema is valid");
        let bench_decl =
            CollectionDeclaration::new("bench", bench_schema).expect("bench collection is valid");
        let data = host.bind(&app, [bench_decl]).expect("bench binds");

        // Warm up the engine before recording the required 1,000 samples per verb.
        for i in 0..5 {
            data.create(
                "bench",
                id(&format!("warm-{i}")),
                json!({"name": "Warm", "price_cents": 100, "available": true}),
            )
            .await
            .expect("warmup create");
        }
        for i in 0..5 {
            data.select("bench", id(&format!("warm-{i}")))
                .await
                .expect("warmup select");
        }
        for _ in 0..5 {
            data.list("bench").await.expect("warmup list");
        }
        for i in 0..5 {
            data.update_merge(
                "bench",
                id(&format!("warm-{i}")),
                json!({"price_cents": 101}),
            )
            .await
            .expect("warmup merge");
            data.update_patch(
                "bench",
                id(&format!("warm-{i}")),
                vec![PatchOperation::Set {
                    field: "available".to_owned(),
                    value: json!(false),
                }],
            )
            .await
            .expect("warmup patch");
        }
        for i in 0..5 {
            data.delete("bench", id(&format!("warm-{i}")))
                .await
                .expect("warmup cleanup");
        }

        let mut create_ms = Vec::with_capacity(SAMPLES_PER_VERB);
        for i in 0..CORPUS_SIZE {
            let start = Instant::now();
            data.create(
                "bench",
                id(&format!("item-{i:04}")),
                json!({"name": "Bench Item", "price_cents": 999, "available": true}),
            )
            .await
            .expect("qualification create");
            create_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        let mut select_ms = Vec::with_capacity(SAMPLES_PER_VERB);
        for i in 0..SAMPLES_PER_VERB {
            let start = Instant::now();
            let record = data
                .select("bench", id(&format!("item-{i:04}")))
                .await
                .expect("qualification select")
                .expect("qualification record exists");
            assert_eq!(record.value["name"], "Bench Item");
            select_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        let mut merge_ms = Vec::with_capacity(SAMPLES_PER_VERB);
        for i in 0..SAMPLES_PER_VERB {
            let start = Instant::now();
            data.update_merge(
                "bench",
                id(&format!("item-{i:04}")),
                json!({"price_cents": 1234}),
            )
            .await
            .expect("qualification update_merge");
            merge_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        let mut patch_ms = Vec::with_capacity(SAMPLES_PER_VERB);
        for i in 0..SAMPLES_PER_VERB {
            let start = Instant::now();
            data.update_patch(
                "bench",
                id(&format!("item-{i:04}")),
                vec![PatchOperation::Set {
                    field: "available".to_owned(),
                    value: json!(false),
                }],
            )
            .await
            .expect("qualification update_patch");
            patch_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        let mut list_ms = Vec::with_capacity(SAMPLES_PER_VERB);
        for _ in 0..SAMPLES_PER_VERB {
            let start = Instant::now();
            let records = data.list("bench").await.expect("qualification list");
            assert_eq!(records.len(), CORPUS_SIZE);
            assert_eq!(
                records.first().expect("first record").id.as_str(),
                "item-0000"
            );
            assert_eq!(
                records.last().expect("last record").id.as_str(),
                "item-0999"
            );
            list_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        let mut delete_ms = Vec::with_capacity(SAMPLES_PER_VERB);
        for i in 0..SAMPLES_PER_VERB {
            let start = Instant::now();
            assert!(
                data.delete("bench", id(&format!("item-{i:04}")))
                    .await
                    .expect("qualification delete")
            );
            delete_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        }

        for (verb, samples) in [
            ("create", create_ms.len()),
            ("select", select_ms.len()),
            ("update_merge", merge_ms.len()),
            ("update_patch", patch_ms.len()),
            ("list", list_ms.len()),
            ("delete", delete_ms.len()),
        ] {
            assert!(
                samples >= SAMPLES_PER_VERB,
                "{verb} has {samples} samples; qualification requires at least {SAMPLES_PER_VERB}"
            );
        }

        let p95_create = p95_ms(&mut create_ms);
        let p95_select = p95_ms(&mut select_ms);
        let p95_merge = p95_ms(&mut merge_ms);
        let p95_patch = p95_ms(&mut patch_ms);
        let p95_list = p95_ms(&mut list_ms);
        let p95_delete = p95_ms(&mut delete_ms);
        let report = json!({
            "qualification": PROFILE,
            "machine": machine,
            "storage": storage,
            "durability": durability,
            "runtime": {
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
            },
            "serialization": "--test-threads=1 (required)",
            "corpus_records": CORPUS_SIZE,
            "samples_per_verb": {
                "create": create_ms.len(),
                "select": select_ms.len(),
                "update_merge": merge_ms.len(),
                "update_patch": patch_ms.len(),
                "list": list_ms.len(),
                "delete": delete_ms.len(),
            },
            "p95_ms": {
                "create": p95_create,
                "select": p95_select,
                "update_merge": p95_merge,
                "update_patch": p95_patch,
                "list": p95_list,
                "delete": p95_delete,
            },
            "targets_p95_ms": {
                "select": SELECT_P95_TARGET_MS,
                "create": WRITE_P95_TARGET_MS,
                "update_merge": WRITE_P95_TARGET_MS,
                "update_patch": WRITE_P95_TARGET_MS,
                "delete": WRITE_P95_TARGET_MS,
                "list": LIST_P95_TARGET_MS,
            },
        });
        eprintln!("STUDIO-BENCH-1 qualification report: {report}");

        assert!(
            p95_select <= SELECT_P95_TARGET_MS,
            "select p95 {p95_select:.2}ms exceeds original target {SELECT_P95_TARGET_MS}ms"
        );
        for (verb, p95) in [
            ("create", p95_create),
            ("update_merge", p95_merge),
            ("update_patch", p95_patch),
            ("delete", p95_delete),
        ] {
            assert!(
                p95 <= WRITE_P95_TARGET_MS,
                "{verb} p95 {p95:.2}ms exceeds original target {WRITE_P95_TARGET_MS}ms"
            );
        }
        assert!(
            p95_list <= LIST_P95_TARGET_MS,
            "list p95 {p95_list:.2}ms exceeds original target {LIST_P95_TARGET_MS}ms"
        );
        assert!(
            data.list("bench").await.expect("final list").is_empty(),
            "qualification deletes the complete corpus"
        );
    }

    host.into_inner().close().await.expect("store closes");
}
