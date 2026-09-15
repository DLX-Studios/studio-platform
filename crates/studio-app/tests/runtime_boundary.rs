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
fn runtime_main_requires_an_explicit_bundle_or_development_input() {
    let main = include_str!("../src/main.rs");
    assert!(main.contains("usage: studio-app (--bundle <absolute-path> | --dev <local-path>)"));
    assert!(main.contains("LaunchRequest::parse_from"));
    assert!(main.contains("FoundationGallery::with_plugin_surface"));
}
