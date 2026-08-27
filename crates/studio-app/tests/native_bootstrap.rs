#![allow(missing_docs)]

use studio_app::{NativeProductState, ProductRoute};
use studio_host::{IdentityKind, IdentitySnapshot, IdentityState, IdentitySummary};

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
