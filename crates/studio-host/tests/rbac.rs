#![allow(missing_docs)]

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use studio_host::{
    ApplicationAuditEvent, ApplicationAuditEventKind, ApplicationAuditSink,
    ApplicationDataErrorCode, ApplicationDataGuestApi, ApplicationDataHost,
    ApplicationRbacSettings, AuthorizationTarget, CollectionDeclaration, CollectionGrant,
    CollectionRequest, CredentialInput, DataOperation, Durability, EmbeddedLocalStore,
    FieldDeclaration, FieldType, GuestDataRequest, LocalStore, PatchOperation, RbacErrorCode,
    RecordId, RecordSchema, RoleDefinition, RowScope, ThrottlePolicy,
};
use studio_security::{PluginPrincipal, TrustMode};
use tempfile::tempdir;

fn principal() -> PluginPrincipal {
    PluginPrincipal::new_verified(
        "publisher.example",
        "key-1",
        "employee-app",
        [7; 32],
        [9; 16],
        TrustMode::Production,
    )
    .expect("fixture principal is valid")
}

fn tickets() -> CollectionDeclaration {
    CollectionDeclaration::new(
        "tickets",
        RecordSchema::new(
            1,
            BTreeMap::from([
                (
                    "owner".to_owned(),
                    FieldDeclaration::required(FieldType::String),
                ),
                (
                    "status".to_owned(),
                    FieldDeclaration::required(FieldType::String),
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

#[derive(Clone, Default)]
struct AuditCapture(Arc<Mutex<Vec<ApplicationAuditEvent>>>);

impl ApplicationAuditSink for AuditCapture {
    fn record(&self, event: ApplicationAuditEvent) {
        self.0.lock().expect("audit capture lock").push(event);
    }
}

#[tokio::test]
async fn host_rbac_enforces_routes_rows_and_direct_collection_calls() {
    let directory = tempdir().expect("temporary directory is created");
    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let host = ApplicationDataHost::new(store);
    let capture = AuditCapture::default();
    let rbac = host
        .bind_rbac(
            &principal(),
            [tickets()],
            ApplicationRbacSettings::default().with_audit_sink(Arc::new(capture.clone())),
        )
        .expect("rbac binds");

    let mut role = RoleDefinition::new("technician")
        .expect("role is valid")
        .with_route("/tickets")
        .expect("route is valid")
        .with_screen("ticket-list")
        .expect("screen is valid")
        .with_action("ticket.update")
        .expect("action is valid");
    role.grant_collection(
        CollectionGrant::new(
            "tickets",
            [
                DataOperation::Select,
                DataOperation::List,
                DataOperation::Create,
                DataOperation::Update,
                DataOperation::Delete,
            ],
            RowScope::own("owner").expect("row scope is valid"),
        )
        .expect("grant is valid"),
    );
    rbac.define_role(role).await.expect("role persists");
    rbac.create_pin_user("alice", "alice", "Alice", b"1234")
        .await
        .expect("alice persists");
    rbac.create_pin_user("bob", "bob", "Bob", b"5678")
        .await
        .expect("bob persists");
    rbac.assign_role("alice", "technician")
        .await
        .expect("alice role persists");
    rbac.assign_role("bob", "technician")
        .await
        .expect("bob role persists");

    let alice = rbac
        .authenticate_pin("alice", b"1234")
        .await
        .expect("alice authenticates offline");
    let bob = rbac
        .authenticate_pin("bob", b"5678")
        .await
        .expect("bob authenticates offline");
    rbac.authorize(&alice, AuthorizationTarget::Route("/tickets".to_owned()))
        .await
        .expect("route grant is host-enforced");
    rbac.authorize_screen(&alice, "ticket-list")
        .await
        .expect("screen grant is host-enforced");
    rbac.authorize_action(&alice, "ticket.update")
        .await
        .expect("action grant is host-enforced");

    let alice_data = rbac.data_for(&alice).await.expect("alice data binds");
    let bob_data = rbac.data_for(&bob).await.expect("bob data binds");
    alice_data
        .create(
            "tickets",
            id("alice-ticket"),
            json!({"owner": "alice", "status": "open"}),
        )
        .await
        .expect("alice creates own ticket");
    bob_data
        .create(
            "tickets",
            id("bob-ticket"),
            json!({"owner": "bob", "status": "open"}),
        )
        .await
        .expect("bob creates own ticket");

    let visible = alice_data.list("tickets").await.expect("list is allowed");
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id.as_str(), "alice-ticket");
    assert_eq!(
        alice_data
            .select("tickets", id("bob-ticket"))
            .await
            .expect_err("foreign select is denied")
            .code(),
        RbacErrorCode::AuthorizationDenied
    );
    assert_eq!(
        alice_data
            .execute(GuestDataRequest::Collection(CollectionRequest::Select {
                collection: "tickets".to_owned(),
                id: id("bob-ticket"),
            }))
            .await
            .expect_err("direct collection helper is also denied")
            .code(),
        ApplicationDataErrorCode::AuthorizationDenied
    );
    assert_eq!(
        alice_data
            .update_patch(
                "tickets",
                id("bob-ticket"),
                vec![PatchOperation::Set {
                    field: "status".to_owned(),
                    value: json!("closed"),
                }],
            )
            .await
            .expect_err("foreign update is denied")
            .code(),
        RbacErrorCode::AuthorizationDenied
    );
    assert_eq!(
        alice_data
            .update_patch(
                "tickets",
                id("alice-ticket"),
                vec![PatchOperation::Set {
                    field: "owner".to_owned(),
                    value: json!("bob"),
                }],
            )
            .await
            .expect_err("moving a row out of scope is denied")
            .code(),
        RbacErrorCode::AuthorizationDenied
    );
    assert_eq!(
        bob_data
            .select("tickets", id("bob-ticket"))
            .await
            .expect("bob still reads own row")
            .expect("bob row exists")
            .value["status"],
        "open"
    );

    rbac.revoke_role("alice", "technician")
        .await
        .expect("role revokes");
    assert_eq!(
        rbac.data_for(&alice)
            .await
            .err()
            .expect("membership change invalidates session")
            .code(),
        RbacErrorCode::SessionInvalid
    );
    rbac.disable_user("bob").await.expect("user disables");

    let kinds = capture
        .0
        .lock()
        .expect("audit capture lock")
        .iter()
        .map(ApplicationAuditEvent::kind)
        .collect::<Vec<_>>();
    assert!(kinds.contains(&ApplicationAuditEventKind::RoleCreated));
    assert!(kinds.contains(&ApplicationAuditEventKind::UserCreated));
    assert!(kinds.contains(&ApplicationAuditEventKind::RoleAssigned));
    assert!(kinds.contains(&ApplicationAuditEventKind::Authentication));
    assert!(kinds.contains(&ApplicationAuditEventKind::RoleRevoked));
    assert!(kinds.contains(&ApplicationAuditEventKind::UserDisabled));

    drop(alice_data);
    drop(bob_data);
    host.into_inner().close().await.expect("store closes");
}

#[tokio::test]
async fn offline_pin_failures_throttle_then_unlock_at_declared_time() {
    let directory = tempdir().expect("temporary directory is created");
    let store = EmbeddedLocalStore::open(directory.path(), Durability::Every)
        .await
        .expect("store opens");
    let host = ApplicationDataHost::new(store);
    let policy = ThrottlePolicy::new(2, Duration::from_secs(60)).expect("policy is valid");
    let rbac = host
        .bind_rbac(
            &principal(),
            [tickets()],
            ApplicationRbacSettings::default().with_throttle(policy),
        )
        .expect("rbac binds");
    rbac.create_pin_user("alice", "alice", "Alice", b"1234")
        .await
        .expect("user persists");
    let epoch = UNIX_EPOCH + Duration::from_secs(100);

    assert_eq!(
        rbac.authenticate_at("alice", CredentialInput::Pin(b"9999"), epoch)
            .await
            .expect_err("first failure is rejected")
            .code(),
        RbacErrorCode::AuthenticationInvalid
    );
    assert_eq!(
        rbac.authenticate_at("alice", CredentialInput::Pin(b"9999"), epoch)
            .await
            .expect_err("second failure locks out")
            .code(),
        RbacErrorCode::AuthenticationThrottled
    );
    assert_eq!(
        rbac.authenticate_at("alice", CredentialInput::Pin(b"1234"), epoch)
            .await
            .expect_err("correct PIN remains locked out")
            .code(),
        RbacErrorCode::AuthenticationThrottled
    );
    let session = rbac
        .authenticate_at(
            "alice",
            CredentialInput::Pin(b"1234"),
            SystemTime::from(UNIX_EPOCH + Duration::from_secs(161)),
        )
        .await
        .expect("PIN unlocks after policy duration");
    assert_eq!(session.user_id(), "alice");

    host.into_inner().close().await.expect("store closes");
}
