#![allow(missing_docs)]

use studio_navigation::project::{ProjectRoute, ProjectRouter, RoutingManifest};
use studio_protocol::NavigationCommand;

fn manifest() -> RoutingManifest {
    RoutingManifest {
        schema_version: 1,
        routes: vec![
            ProjectRoute {
                id: "home".into(),
                path: "/".into(),
                source: "routes/+page.studio".into(),
            },
            ProjectRoute {
                id: "stream".into(),
                path: "/streams/:id".into(),
                source: "routes/streams/[id]/+page.studio".into(),
            },
        ],
    }
}

#[test]
fn host_navigation_resolves_root_params_and_history_atomically() {
    let mut router = ProjectRouter::new(&manifest(), "/").unwrap();
    let selection = router
        .navigate(&NavigationCommand::Push {
            route: "/streams/42".into(),
        })
        .unwrap();
    assert_eq!(selection.id, "stream");
    assert_eq!(selection.params.get("id").unwrap(), "42");
    assert!(
        router
            .navigate(&NavigationCommand::Push {
                route: "/missing".into()
            })
            .is_err()
    );
    assert_eq!(router.current().route, "/streams/42");
    assert_eq!(router.navigate(&NavigationCommand::Pop).unwrap().route, "/");
    assert!(router.navigate(&NavigationCommand::Pop).is_err());
    assert_eq!(router.current().route, "/");
}

#[test]
fn routing_metadata_is_closed_bounded_and_source_confined() {
    assert!(
        RoutingManifest::from_bytes(br#"{"schemaVersion":1,"routes":[],"extra":true}"#).is_err()
    );
    for source in [
        "../outside.studio",
        "/outside.studio",
        "routes\\page.studio",
        "routes/page.ts",
    ] {
        let mut data = manifest();
        data.routes[0].source = source.into();
        assert!(ProjectRouter::new(&data, "/").is_err(), "{source}");
    }
    let mut too_many = manifest();
    too_many.routes = vec![too_many.routes[0].clone(); 65];
    assert!(ProjectRouter::new(&too_many, "/").is_err());
    let mut wrong_version = manifest();
    wrong_version.schema_version = 2;
    assert!(ProjectRouter::new(&wrong_version, "/").is_err());
}

#[test]
fn staged_navigation_clones_and_stack_limits_cannot_mutate_committed_history() {
    let router = ProjectRouter::new(&manifest(), "/").unwrap();
    let mut staged = router.clone();
    staged
        .navigate(&NavigationCommand::Push {
            route: "/streams/42".into(),
        })
        .unwrap();
    assert_eq!(router.current().route, "/");
    assert_eq!(staged.current().route, "/streams/42");
    for _ in 0..30 {
        staged
            .navigate(&NavigationCommand::Push {
                route: "/streams/42".into(),
            })
            .unwrap();
    }
    assert!(
        staged
            .navigate(&NavigationCommand::Push { route: "/".into() })
            .is_err()
    );
    assert_eq!(staged.current().route, "/streams/42");
    staged
        .navigate(&NavigationCommand::Reset { route: "/".into() })
        .unwrap();
    assert!(staged.navigate(&NavigationCommand::Pop).is_err());
}

#[test]
fn replace_pop_to_and_declared_catch_all_use_the_host_resolver() {
    let mut data = manifest();
    data.routes.push(ProjectRoute {
        id: "not_found".into(),
        path: "/*".into(),
        source: "routes/[...rest]/+page.studio".into(),
    });
    let mut router = ProjectRouter::new(&data, "/").unwrap();
    router
        .navigate(&NavigationCommand::Push {
            route: "/streams/42".into(),
        })
        .unwrap();
    assert_eq!(
        router
            .navigate(&NavigationCommand::Replace {
                route: "/unknown/deep/path".into()
            })
            .unwrap()
            .id,
        "not_found"
    );
    assert_eq!(
        router
            .navigate(&NavigationCommand::PopTo { route: "/".into() })
            .unwrap()
            .id,
        "home"
    );
    assert!(
        router
            .navigate(&NavigationCommand::PopTo {
                route: "/streams/42".into()
            })
            .is_err()
    );
    assert_eq!(router.current().route, "/");
}

#[test]
fn initial_route_prefers_root_and_rejects_parameter_only_startup() {
    let mut data = manifest();
    data.routes.reverse();
    assert_eq!(data.initial_route().unwrap(), "/");
    data.routes.retain(|entry| entry.path != "/");
    assert!(data.initial_route().is_err());
    data.routes.push(ProjectRoute {
        id: "live".into(),
        path: "/live".into(),
        source: "routes/live/+page.studio".into(),
    });
    assert_eq!(data.initial_route().unwrap(), "/live");
}

#[test]
fn metadata_rejects_duplicate_ids_ambiguous_patterns_and_oversized_documents() {
    let mut data = manifest();
    data.routes[1].id = data.routes[0].id.clone();
    assert!(data.validate().is_err());
    data = manifest();
    data.routes.push(ProjectRoute {
        id: "other".into(),
        path: "/streams/:name".into(),
        source: "routes/streams/[name]/+page.studio".into(),
    });
    assert!(data.validate().is_err());
    assert!(RoutingManifest::from_bytes(&vec![b' '; 1024 * 1024 + 1]).is_err());
    assert!(RoutingManifest::from_bytes(br#"{"schemaVersion":1,"routes":[{"id":"home","path":"/","source":"routes/+page.studio","extra":true}]}"#).is_err());
}

#[test]
fn reconfiguration_preserves_history_when_the_active_concrete_route_survives() {
    let mut router = ProjectRouter::new(&manifest(), "/").unwrap();
    router
        .navigate(&NavigationCommand::Push {
            route: "/streams/42".into(),
        })
        .unwrap();
    let mut changed = manifest();
    changed.routes.push(ProjectRoute {
        id: "live".into(),
        path: "/live".into(),
        source: "routes/live/+page.studio".into(),
    });
    let mut reconfigured = router.reconfigure(&changed).unwrap();
    assert_eq!(reconfigured.current().route, "/streams/42");
    assert_eq!(
        reconfigured
            .navigate(&NavigationCommand::Pop)
            .unwrap()
            .route,
        "/"
    );
    assert_eq!(router.current().route, "/streams/42");
}

#[test]
fn reconfiguration_resets_history_when_the_active_page_is_removed() {
    let mut router = ProjectRouter::new(&manifest(), "/").unwrap();
    router
        .navigate(&NavigationCommand::Push {
            route: "/streams/42".into(),
        })
        .unwrap();
    let mut changed = manifest();
    changed.routes.retain(|route| route.path == "/");
    let mut reconfigured = router.reconfigure(&changed).unwrap();
    assert_eq!(reconfigured.current().route, "/");
    assert!(reconfigured.navigate(&NavigationCommand::Pop).is_err());
    changed.schema_version = 2;
    assert!(router.reconfigure(&changed).is_err());
    assert_eq!(router.current().route, "/streams/42");
}
