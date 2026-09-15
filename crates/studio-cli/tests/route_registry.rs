//! US1/US2 route registry contract: shapes map, errors name files, the
//! render is golden-stable, and the 40-path battery resolves per the shared
//! matching rule.

use std::path::PathBuf;
use studio_cli::routes::{build_registry, match_route, render_module, RouteKind};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn shapes_map_and_helpers_are_excluded() {
    let registry = build_registry(&fixture("routes-ok")).unwrap();
    let paths: Vec<&str> = registry
        .entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    assert_eq!(
        paths,
        vec!["/", "/*", "/orders/:id", "/orders/active", "/pos"]
    );
    assert!(
        registry
            .entries
            .iter()
            .all(|entry| entry.file != "routes/_helpers.ts"),
        "helper modules are excluded"
    );

    let pos = registry
        .entries
        .iter()
        .find(|entry| entry.path == "/pos")
        .unwrap();
    assert_eq!(pos.title, "Point of Sale");
    assert_eq!(pos.file, "routes/pos.ts");

    let index = registry
        .entries
        .iter()
        .find(|entry| entry.path == "/")
        .unwrap();
    assert_eq!(index.title, "Home");

    let param = registry
        .entries
        .iter()
        .find(|entry| entry.path == "/orders/:id")
        .unwrap();
    assert_eq!(param.kind, RouteKind::Param);
    assert_eq!(param.params, vec!["id"]);

    assert_eq!(registry.not_found, Some("/*".to_owned()));
}

#[test]
fn duplicates_ambiguity_and_malformed_are_rejected() {
    let error = build_registry(&fixture("routes-dup")).expect_err("duplicate must fail");
    assert!(error
        .iter()
        .any(|diagnostic| diagnostic.code == "ROUTE_DUPLICATE_PATH"
            && diagnostic.message.contains("/pos")));
    assert!(error
        .iter()
        .flat_map(|diagnostic| diagnostic.files.clone())
        .any(|file| file.ends_with("pos/index.ts")));

    let error = build_registry(&fixture("routes-ambiguous")).expect_err("ambiguous must fail");
    assert!(error
        .iter()
        .any(|diagnostic| diagnostic.code == "ROUTE_AMBIGUOUS_SHAPE"));

    let error = build_registry(&fixture("routes-malformed")).expect_err("malformed must fail");
    let codes: Vec<&str> = error.iter().map(|diagnostic| diagnostic.code).collect();
    assert!(
        codes.contains(&"ROUTE_MALFORMED_CHARS"),
        "space in name: {codes:?}"
    );
    assert!(
        codes.contains(&"ROUTE_MALFORMED_PARAM"),
        "empty []: {codes:?}"
    );
    assert!(
        codes.contains(&"ROUTE_MALFORMED_WILDCARD"),
        "empty [...]: {codes:?}"
    );

    let error = build_registry(&fixture("routes-mismatch")).expect_err("mismatch must fail");
    assert!(error
        .iter()
        .any(|diagnostic| diagnostic.code == "ROUTE_MISMATCH_PATH"));
}

#[test]
fn render_is_golden_stable_and_keeps_legacy_exports() {
    let registry = build_registry(&fixture("routes-ok")).unwrap();
    let rendered = render_module(&registry);
    assert!(rendered.contains("export const route_pos = \"/pos\";"));
    assert!(rendered.contains(
        "export const declaredRoutes = [\"/\", \"/orders/:id\", \"/orders/active\", \"/pos\"];"
    ));
    assert!(rendered.contains("export const routeTable: RouteEntry[] = ["));
    assert!(rendered.contains("export const notFoundRoute: string = \"/*\";"));
    let golden =
        std::fs::read_to_string(fixture("routes-ok-routes.generated.ts")).unwrap_or_default();
    if std::env::var("BLESS").is_ok() {
        std::fs::write(fixture("routes-ok-routes.generated.ts"), &rendered).unwrap();
    } else if !golden.is_empty() {
        assert_eq!(rendered, golden, "render drift");
    }
}

/// One battery row: path, expected entry path, expected params.
type BatteryRow<'a> = (&'a str, Option<(&'a str, &'a [(&'a str, &'a str)])>);

#[test]
fn path_battery_resolves_per_the_shared_rule() {
    let registry = build_registry(&fixture("routes-ok")).unwrap();
    // (path, expected entry path, expected params)
    let cases: &[BatteryRow<'_>] = &[
        ("/", Some(("/", &[]))),
        ("/pos", Some(("/pos", &[]))),
        ("/pos/", Some(("/pos", &[]))),
        ("/pos?tab=1#top", Some(("/pos", &[]))),
        ("/POS", Some(("/*", &[]))),
        ("/orders/active", Some(("/orders/active", &[]))),
        ("/orders/123", Some(("/orders/:id", &[("id", "123")]))),
        (
            "/orders/abc-def_9",
            Some(("/orders/:id", &[("id", "abc-def_9")])),
        ),
        ("/orders", Some(("/*", &[]))),
        ("/orders/1/2", Some(("/*", &[]))),
        ("/anything/at/all", Some(("/*", &[]))),
        ("/missing", Some(("/*", &[]))),
    ];
    for (path, expected) in cases {
        let found = match_route(&registry, path);
        match (found, expected) {
            (Some((entry, params)), Some((want_path, want_params))) => {
                assert_eq!(entry.path, *want_path, "path {path}");
                let params: Vec<(&str, &str)> = params
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str()))
                    .collect();
                assert_eq!(params, *want_params, "params for {path}");
            }
            (None, None) => {}
            (found, expected) => panic!("mismatch for {path}: {found:?} vs {expected:?}"),
        }
    }
    // Without a catch-all, misses are explicit.
    let bare = studio_cli::routes::RouteRegistry {
        entries: registry
            .entries
            .into_iter()
            .filter(|entry| entry.kind == RouteKind::Static)
            .collect(),
        not_found: None,
    };
    assert!(match_route(&bare, "/missing").is_none());
}
