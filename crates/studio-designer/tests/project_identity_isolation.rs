#![allow(missing_docs)]

use studio_designer::{NativeProductBootstrap, ProductRoute};

#[test]
fn authenticated_session_cannot_route_to_an_unowned_project() {
    let directory = tempfile::tempdir().expect("temporary durable store");
    let mut bootstrap = NativeProductBootstrap::open(directory.path()).expect("bootstrap");
    bootstrap
        .create_identity_and_sign_in_blocking(
            "Alice",
            b"password".to_vec(),
            b"password".to_vec(),
            false,
        )
        .expect("Alice session");
    bootstrap.sign_out().expect("sign out Alice");
    bootstrap
        .create_identity_and_sign_in_blocking(
            "Bob",
            b"password".to_vec(),
            b"password".to_vec(),
            false,
        )
        .expect("Bob session");
    assert!(!bootstrap.state_mut().open_project("local-project-1"));
    assert!(!bootstrap.state_mut().navigate(ProductRoute::Project {
        project_id: "local-project-1".to_owned(),
    }));
}

use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};
use studio_design::{
    Actor, ActorId, ActorKind, CommandOutcome, ConflictPersistence, DesignerPersistence, NodeId,
    OperationId, ProjectId, PropertyValue, RecoveryPersistence, UndoGroupId,
    WORKSPACE_STATE_SCHEMA_VERSION, WorkspacePersistence, WorkspaceRecord, WorkspaceState,
};
use studio_designer::project_dashboard::{ProjectAuthority, ProjectRecord};

struct TestWake;
impl Wake for TestWake {
    fn wake(self: Arc<Self>) {}
}
fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(TestWake));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(result) => return result,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}
fn actor() -> Actor {
    Actor {
        id: ActorId::new("alice-editor").unwrap(),
        kind: ActorKind::Human,
        display_name: "Alice".to_owned(),
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one child-process journey proves isolation and cold reopen together"
)]
fn owned_source_survives_restart_and_foreign_or_expired_sessions_are_denied() {
    if let Some(path) = std::env::var_os("STUDIO_ISOLATION_WORKER_PATH") {
        let path = std::path::PathBuf::from(path);
        let mut bootstrap = NativeProductBootstrap::open(&path).unwrap();
        let alice_id = bootstrap
            .create_identity_and_sign_in_blocking(
                "Alice",
                b"password".to_vec(),
                b"password".to_vec(),
                false,
            )
            .unwrap()
            .identity_id;
        let alice_project = bootstrap
            .create_project_blocking("Alice's project")
            .unwrap();
        bootstrap.open_project(&alice_project).unwrap();
        let persistence = bootstrap.project_persistence(&alice_project).unwrap();
        let project = ProjectId::new(&alice_project).unwrap();
        let mut model = block_on(bootstrap.open_project_model(&alice_project)).unwrap();
        model.select_str("headline").unwrap();
        assert!(matches!(
            block_on(model.edit_property(
                OperationId::new("alice-secret").unwrap(),
                actor(),
                UndoGroupId::new("alice-edit").unwrap(),
                "Edit title",
                "text",
                Some(PropertyValue::String("Alice private draft".to_owned())),
            )),
            CommandOutcome::Accepted(_)
        ));
        let workspace = WorkspaceRecord {
            schema_version: WORKSPACE_STATE_SCHEMA_VERSION,
            project_id: project.clone(),
            state: WorkspaceState::default(),
        };
        block_on(WorkspacePersistence::save(&persistence, &workspace)).unwrap();
        let alice_settings = bootstrap.settings().unwrap();
        assert!(alice_settings.project(&alice_project).is_ok());
        bootstrap.sign_out().unwrap();
        assert!(alice_settings.project(&alice_project).is_err());
        assert!(block_on(DesignerPersistence::load(&persistence, &project)).is_err());
        assert!(block_on(WorkspacePersistence::save(&persistence, &workspace)).is_err());
        assert!(
            block_on(ConflictPersistence::save_conflicts(
                &persistence,
                &project,
                &[]
            ))
            .is_err()
        );
        assert!(
            block_on(RecoveryPersistence::save_recovery(
                &persistence,
                &project,
                &[]
            ))
            .is_err()
        );
        assert!(!matches!(
            block_on(model.edit_property(
                OperationId::new("expired-edit").unwrap(),
                actor(),
                UndoGroupId::new("expired").unwrap(),
                "Edit title",
                "text",
                Some(PropertyValue::String("Should never persist".to_owned())),
            )),
            CommandOutcome::Accepted(_)
        ));

        bootstrap
            .create_identity_and_sign_in_blocking(
                "Bob",
                b"password".to_vec(),
                b"password".to_vec(),
                false,
            )
            .unwrap();
        let bob_project = bootstrap.create_project_blocking("Bob's project").unwrap();
        assert_ne!(alice_project, bob_project);
        let bob = bootstrap.project_persistence(&bob_project).unwrap();
        assert!(
            bootstrap
                .settings()
                .unwrap()
                .project(&alice_project)
                .is_err()
        );
        assert!(block_on(DesignerPersistence::load(&bob, &project)).is_err());
        assert!(block_on(WorkspacePersistence::save(&bob, &workspace)).is_err());
        // Dashboard metadata is not authority, including the colliding legacy ID.
        for id in [&alice_project, "local-project-1"] {
            bootstrap
                .dashboard()
                .unwrap()
                .add_project(ProjectRecord::new(
                    id,
                    "Forged catalog entry",
                    ProjectAuthority::Local,
                    1,
                ))
                .unwrap();
            assert!(bootstrap.open_project(id).is_err());
            assert!(block_on(bootstrap.open_project_model(id)).is_err());
        }
        let bob_model = block_on(bootstrap.open_project_model(&bob_project)).unwrap();
        assert!(!bob_model.script_source().contains("Alice private draft"));
        bootstrap.sign_out().unwrap();
        std::fs::write(
            path.join("test-project-ids.json"),
            serde_json::to_vec(&(alice_id, alice_project)).unwrap(),
        )
        .unwrap();
        return;
    }
    // A new OS process proves cold reopen and avoids reusing the engine's background router.
    let directory = tempfile::tempdir().unwrap();
    let worker = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "owned_source_survives_restart_and_foreign_or_expired_sessions_are_denied",
            "--nocapture",
        ])
        .env("STUDIO_ISOLATION_WORKER_PATH", directory.path())
        .output()
        .unwrap();
    assert!(
        worker.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&worker.stdout),
        String::from_utf8_lossy(&worker.stderr)
    );
    let (alice_id, alice_project): (String, String) = serde_json::from_slice(
        &std::fs::read(directory.path().join("test-project-ids.json")).unwrap(),
    )
    .unwrap();
    let mut reopened = NativeProductBootstrap::open(directory.path()).unwrap();
    reopened
        .sign_in_blocking(&alice_id, b"password", false)
        .unwrap();
    let model = block_on(reopened.open_project_model(&alice_project)).unwrap();
    let text =
        &model.source_snapshot().design.nodes[&NodeId::new("headline").unwrap()].properties["text"];
    assert_eq!(
        text,
        &PropertyValue::String("Alice private draft".to_owned())
    );
    let adapter = reopened.project_persistence(&alice_project).unwrap();
    assert!(
        block_on(WorkspacePersistence::load(
            &adapter,
            &ProjectId::new(&alice_project).unwrap()
        ))
        .unwrap()
        .is_some()
    );
    let second = reopened.create_project_blocking("After restart").unwrap();
    assert_ne!(alice_project, second);
    assert_eq!(reopened.dashboard().unwrap().snapshot().projects.len(), 2);
}
