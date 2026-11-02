//! Explicit boundary checks for the Runtime application.

#[test]
fn runtime_shell_has_no_designer_product_entry_points() {
    let lib = include_str!("../src/lib.rs");
    let main = include_str!("../src/main.rs");
    for source in [lib, main] {
        for forbidden in [
            "NativeProductShell",
            "NativeProductBootstrap",
            "ProductRoute",
            "FocusView",
            "IdentityChooser",
            "open_project",
        ] {
            assert!(
                !source.contains(forbidden),
                "Runtime source must not contain Designer entry point {forbidden}"
            );
        }
    }
}

#[test]
fn runtime_main_accepts_only_production_bundles() {
    let main = include_str!("../src/main.rs");
    assert!(main.contains("usage: studio-app --bundle <absolute-path>"));
    assert!(main.contains("LaunchRequest::parse_runtime_from"));
    assert!(main.contains("FoundationGallery::with_plugin_surface"));
}

#[test]
fn runtime_rejects_development_and_reload_arguments() {
    use studio_app::cli::LaunchRequest;
    assert!(LaunchRequest::parse_runtime_from(["studio-app", "--dev", "/tmp/app.studio"]).is_err());
    assert!(
        LaunchRequest::parse_runtime_from([
            "studio-app",
            "--bundle",
            "/tmp/app.studio",
            "--reload-socket",
            "/tmp/socket"
        ])
        .is_err()
    );
    assert!(
        LaunchRequest::parse_runtime_from(["studio-app", "--bundle", "/tmp/app.studio"]).is_ok()
    );
}
