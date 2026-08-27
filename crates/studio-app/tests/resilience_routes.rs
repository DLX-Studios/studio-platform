#![allow(missing_docs)]

use studio_app::resilience::{
    ResilienceCenter, ResilienceEntryPoint, ResilienceRoute,
};

#[test]
fn dashboard_centers_are_reachable_without_opening_a_project() {
    let conflicts = ResilienceRoute::from_dashboard(ResilienceCenter::Conflicts);
    assert_eq!(conflicts.entry_point(), ResilienceEntryPoint::Dashboard);
    assert_eq!(conflicts.path(), "/dashboard/conflicts");
    assert_eq!(conflicts.project_id(), None);

    let recovery = ResilienceRoute::from_dashboard(ResilienceCenter::Recovery);
    assert_eq!(recovery.path(), "/dashboard/recovery");
}

#[test]
fn project_settings_center_preserves_project_identity_in_route() {
    let route = ResilienceRoute::from_project_settings(
        "restaurant-local",
        ResilienceCenter::Recovery,
    )
    .unwrap();
    assert_eq!(route.entry_point(), ResilienceEntryPoint::ProjectSettings);
    assert_eq!(route.project_id(), Some("restaurant-local"));
    assert_eq!(route.path(), "/projects/restaurant-local/settings/recovery");
}

#[test]
fn project_settings_route_rejects_delimiters_and_controls() {
    for project_id in ["", "../secret", "bad/project", "bad?project", "bad\nproject"] {
        assert!(ResilienceRoute::from_project_settings(project_id, ResilienceCenter::Conflicts)
            .is_err());
    }
}
