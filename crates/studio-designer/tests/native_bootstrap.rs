#![allow(missing_docs)]

use studio_designer::{NativeProductBootstrap, NativeProductState, ProductRoute};
use studio_host::{
    IdentityErrorCode, IdentityKind, IdentitySnapshot, IdentityState, IdentitySummary,
};

fn first_launch() -> NativeProductState {
    NativeProductState::new(
        IdentitySnapshot {
            welcome_dismissed: false,
            identities: vec![IdentitySummary {
                identity_id: "local-1".to_owned(),
                kind: IdentityKind::Local,
                display_name: "Local Designer".to_owned(),
                email: None,
                avatar: None,
                state: IdentityState::Available,
            }],
            sessions: Vec::new(),
        },
        None,
    )
}

#[test]
fn clean_start_is_welcome_and_does_not_authorize_project_routes() {
    let mut state = first_launch();
    assert_eq!(state.route(), &ProductRoute::Welcome);
    assert!(!state.is_authenticated());
    assert!(!state.navigate(ProductRoute::Dashboard));
    assert!(!state.open_project("local-project"));
}

#[test]
fn welcome_dismissal_reaches_identity_gate_and_support_routes_remain_reachable() {
    let mut state = first_launch();
    state.dismiss_welcome();
    assert_eq!(state.route(), &ProductRoute::IdentityChooser);
    assert!(state.navigate(ProductRoute::Help));
    assert_eq!(state.route().path(), "/help");
    assert!(state.navigate(ProductRoute::About));
    assert_eq!(state.route().path(), "/about");
}

#[test]
fn product_routes_have_stable_paths_for_recovery_and_sync_entry_points() {
    assert_eq!(ProductRoute::Settings.path(), "/settings");
    assert_eq!(ProductRoute::SyncStatus.path(), "/dashboard/sync");
    assert_eq!(ProductRoute::Conflicts.path(), "/dashboard/conflicts");
    assert_eq!(ProductRoute::Recovery.path(), "/dashboard/recovery");
    assert_eq!(
        ProductRoute::Project {
            project_id: "local-project".to_owned()
        }
        .path(),
        "/projects/local-project"
    );
    assert_eq!(
        ProductRoute::SignIn {
            identity_id: "local-1".to_owned()
        }
        .path(),
        "/identity/local-1/sign-in"
    );
    assert_eq!(
        ProductRoute::Unlock {
            identity_id: "local-1".to_owned()
        }
        .path(),
        "/identity/local-1/unlock"
    );
}

#[test]
fn designer_binary_owns_the_product_shell_without_runtime_bundle_admission() {
    let main = include_str!("../src/main.rs");
    let bootstrap = include_str!("../src/bootstrap.rs");
    assert!(main.contains("NativeProductBootstrap"));
    assert!(main.contains("NativeProductShell"));
    assert!(bootstrap.contains("FocusView::new"));
    assert!(bootstrap.contains("LocalStoreDesignerPersistence"));
    assert!(!main.contains("LaunchRequest"));
    assert!(!bootstrap.contains("FoundationGallery"));
}

#[test]
fn native_identity_forms_and_dashboard_route_use_host_services() {
    let directory = tempfile::tempdir().expect("temporary Studio data directory");
    let mut bootstrap = NativeProductBootstrap::open(directory.path()).expect("bootstrap opens");
    assert_eq!(bootstrap.state().route(), &ProductRoute::Welcome);

    bootstrap
        .dismiss_welcome()
        .expect("welcome dismissal persists");
    assert_eq!(bootstrap.state().route(), &ProductRoute::IdentityChooser);
    assert_eq!(
        bootstrap
            .create_identity_blocking("", b"password".to_vec(), b"password".to_vec())
            .expect_err("empty display name is rejected")
            .code(),
        IdentityErrorCode::InvalidInput
    );
    bootstrap
        .create_identity_blocking("Local Designer", b"password".to_vec(), b"password".to_vec())
        .expect("identity is created");
    assert_eq!(bootstrap.state().route(), &ProductRoute::IdentityChooser);

    let identity_id = bootstrap.state().identity().identities()[0]
        .identity_id
        .clone();
    let wrong = bootstrap
        .sign_in_blocking(&identity_id, b"wrong", false)
        .expect_err("wrong password is rejected and locks");
    assert_eq!(wrong.code(), IdentityErrorCode::WrongPassword);
    assert_eq!(
        bootstrap.state().identity().identities()[0].state,
        IdentityState::Locked
    );
    bootstrap
        .unlock_blocking(&identity_id, b"password", false)
        .expect("locked identity unlocks");
    assert_eq!(bootstrap.state().route(), &ProductRoute::Dashboard);

    let mut dashboard = bootstrap.dashboard().expect("dashboard opens");
    dashboard
        .add_project(studio_designer::project_dashboard::ProjectRecord::new(
            "native-project",
            "Native project",
            studio_designer::project_dashboard::ProjectAuthority::Local,
            1,
        ))
        .expect("project persists");
    assert_eq!(dashboard.snapshot().projects.len(), 1);
    assert!(bootstrap.state_mut().open_project("native-project"));
    assert_eq!(
        bootstrap.state().route(),
        &ProductRoute::Project {
            project_id: "native-project".to_owned()
        }
    );
}
