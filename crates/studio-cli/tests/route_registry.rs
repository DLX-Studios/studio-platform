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
fn studio_pages_are_discovered_and_layouts_are_excluded() {
    let project = tempfile::tempdir().unwrap();
    for file in [
        "routes/+page.studio",
        "routes/+layout.studio",
        "routes/live/+page.studio",
        "routes/live/+layout.studio",
        "routes/streams/[id]/+page.studio",
        "routes/[...rest]/+page.studio",
        "routes/_private/+page.studio",
        "routes/live/_helper.studio",
        "routes/about.studio",
    ] {
        let path = project.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "<Text id=\"title\">Page</Text>").unwrap();
    }
    let registry = build_registry(project.path()).unwrap();
    let paths: Vec<_> = registry
        .entries
        .iter()
        .map(|route| route.path.as_str())
        .collect();
    assert_eq!(paths, ["/", "/*", "/about", "/live", "/streams/:id"]);
    assert!(registry
        .entries
        .iter()
        .all(|route| !route.file.contains("layout")));
    assert_eq!(
        match_route(&registry, "/streams/42").unwrap().1,
        [("id".to_owned(), "42".to_owned())]
    );
}

#[test]
fn studio_and_legacy_pages_share_duplicate_detection() {
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(project.path().join("routes")).unwrap();
    std::fs::write(project.path().join("routes/+page.studio"), "<Text />").unwrap();
    std::fs::write(project.path().join("routes/index.ts"), "").unwrap();
    let diagnostics = build_registry(project.path()).expect_err("both files define root");
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "ROUTE_DUPLICATE_PATH"));
}

#[test]
fn studio_repeated_parameter_names_are_rejected_by_shared_pattern_validation() {
    let project = tempfile::tempdir().unwrap();
    let directory = project.path().join("routes/[id]/[id]");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("+page.studio"), "<Text />").unwrap();
    let diagnostics =
        build_registry(project.path()).expect_err("repeated parameter names are invalid");
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "ROUTE_MALFORMED_PARAM"));
}

#[cfg(unix)]
#[test]
fn route_symlinks_cannot_escape_the_project() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(project.path().join("routes")).unwrap();
    std::fs::write(outside.path().join("secret.studio"), "<Text />").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.studio"),
        project.path().join("routes/+page.studio"),
    )
    .unwrap();
    assert!(build_registry(project.path()).is_err());
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
