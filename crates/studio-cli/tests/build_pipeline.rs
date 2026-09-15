//! US1/US4 integration contract: clean, failed, and repeated builds through
//! the real `studio` binary, plus the documented exit-code classification.

mod common;

use std::process::Output;
use tempfile::TempDir;

fn bundle_path(project: &std::path::Path) -> std::path::PathBuf {
    project.join("build/starter.studio")
}

#[test]
fn clean_build_produces_bundle_and_exits_zero() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "starter");

    let output = common::run_studio(&["build", project.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(bundle_path(&project).is_file(), "bundle was produced");
}

#[test]
fn repeated_builds_are_byte_identical() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "starter");

    let first = common::run_studio(&["build", project.to_str().unwrap()]);
    assert_eq!(first.status.code(), Some(0));
    let first_bytes = std::fs::read(bundle_path(&project)).unwrap();

    let second = common::run_studio(&["build", project.to_str().unwrap()]);
    assert_eq!(second.status.code(), Some(0));
    let second_bytes = std::fs::read(bundle_path(&project)).unwrap();

    assert_eq!(
        first_bytes, second_bytes,
        "identical inputs produce identical bundles"
    );
}

#[test]
fn compile_failure_is_structured_and_never_damages_the_destination() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "starter");
    std::fs::write(
        project.join("assembly/index.ts"),
        "const broken: i32 = \"not a number\";\n",
    )
    .unwrap();

    // A previously good bundle must survive a failed rebuild untouched.
    std::fs::create_dir_all(project.join("build")).unwrap();
    std::fs::write(bundle_path(&project), b"PREVIOUS-GOOD-BUNDLE").unwrap();

    let output = common::run_studio(&["build", project.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(11), "compile family exit code");
    let compile = common::json_lines_with(&output.stderr, "phase", "compile");
    assert!(
        compile
            .iter()
            .any(|record| record["code"] == "BUILD_COMPILE_ASC"),
        "expected BUILD_COMPILE_ASC diagnostic, got {compile:?}"
    );
    assert_eq!(
        std::fs::read(bundle_path(&project)).unwrap(),
        b"PREVIOUS-GOOD-BUNDLE",
        "previous bundle untouched by the failed build"
    );
    let leftovers: Vec<_> = std::fs::read_dir(project.join("build"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "no temp artifacts left behind");
}

#[test]
fn validate_failures_report_the_validate_family() {
    let root = TempDir::new().unwrap();
    let project = root.path().join("hollow");
    std::fs::create_dir_all(&project).unwrap();

    let output = common::run_studio(&["build", project.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(10), "validate family exit code");
    let validate = common::json_lines_with(&output.stderr, "phase", "validate");
    assert!(
        validate
            .iter()
            .any(|record| record["code"] == "BUILD_VALIDATE_SOURCE_MISSING"),
        "expected BUILD_VALIDATE_SOURCE_MISSING, got {validate:?}"
    );
    assert!(
        validate
            .iter()
            .any(|record| record["code"] == "BUILD_VALIDATE_MANIFEST_MISSING"),
        "expected BUILD_VALIDATE_MANIFEST_MISSING, got {validate:?}"
    );
}

#[test]
fn missing_declared_asset_fails_in_the_package_phase() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "starter");
    let manifest = std::fs::read_to_string(project.join("manifest.json")).unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    value["assets"] = serde_json::json!(["assets/missing.svg"]);
    std::fs::write(
        project.join("manifest.json"),
        serde_json::to_string_pretty(&value).unwrap(),
    )
    .unwrap();

    let output = common::run_studio(&["build", project.to_str().unwrap()]);

    assert_eq!(output.status.code(), Some(12), "package family exit code");
    let package = common::json_lines_with(&output.stderr, "phase", "package");
    assert!(
        package
            .iter()
            .any(|record| record["code"] == "BUILD_PACKAGE_ASSET_MISSING"),
        "expected BUILD_PACKAGE_ASSET_MISSING, got {package:?}"
    );
}

#[test]
fn unknown_example_maps_to_the_usage_family() {
    let output: Output = common::run_studio(&["build", "definitely-not-an-example"]);

    assert_eq!(output.status.code(), Some(2), "usage family exit code");
    let usage = common::json_lines_with(&output.stderr, "phase", "usage");
    assert!(
        usage
            .iter()
            .any(|record| record["code"] == "BUILD_USAGE_EXAMPLE_UNKNOWN"),
        "expected BUILD_USAGE_EXAMPLE_UNKNOWN, got {usage:?}"
    );
}

#[test]
fn content_stable_generators_do_not_rewrite_unchanged_outputs() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "starter");
    let request = studio_cli::build::BuildRequest::resolve(project.to_str().unwrap()).unwrap();
    std::fs::create_dir_all(project.join("routes")).unwrap();
    std::fs::write(
        project.join("routes/pos.ts"),
        "export const screen = 'pos';\n",
    )
    .unwrap();

    studio_cli::build::generate_routes(&request).unwrap();
    let generated = project.join("assembly/routes.generated.ts");
    let first_mtime = std::fs::metadata(&generated).unwrap().modified().unwrap();

    studio_cli::build::generate_routes(&request).unwrap();
    let second_mtime = std::fs::metadata(&generated).unwrap().modified().unwrap();
    assert_eq!(
        first_mtime, second_mtime,
        "unchanged inputs must not rewrite routes.generated.ts"
    );
}

#[test]
fn io_environment_failures_use_the_io_family() {
    let output = common::run_studio(&["check", "/nonexistent-path-for-cli-io-test"]);
    assert_eq!(output.status.code(), Some(13), "io family exit code");
}

fn seed_studio_project(root: &std::path::Path) -> std::path::PathBuf {
    let project = root.join("cards");
    std::fs::create_dir_all(project.join("components")).unwrap();
    std::fs::write(
        project.join("components").join("app.studio"),
        "<script lang=\"ts\">\n  let { title = \"Cards\" } = $props();\n  let count = $state(2);\n</script>\n<Card id=\"cards-root\">\n  <Column id=\"cards-col\">\n    <Text id=\"cards-title\">{title}</Text>\n    <Text id=\"cards-count\">{count}</Text>\n  </Column>\n</Card>\n",
    )
    .unwrap();
    std::fs::write(
        project.join("manifest.json"),
        "{\"schemaVersion\": 1, \"id\": \"com.studio.cards\", \"name\": \"Cards\", \"version\": \"0.1.0\", \"publisher\": { \"id\": \"studio-examples\", \"keyId\": \"example-key-1\" }, \"entry\": \"module.wasm\", \"sdkVersion\": \"^0.1.0\", \"protocolVersion\": 1, \"capabilities\": [], \"limits\": { \"memoryMiB\": 16, \"eventFuel\": 1000000 }, \"assets\": []}",
    )
    .unwrap();
    project
}

#[test]
fn check_validates_studio_sources_with_stable_codes() {
    let root = TempDir::new().unwrap();
    let project = seed_studio_project(root.path());
    let component = project.join("components").join("app.studio");

    let output = common::run_studio(&["check", component.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));

    std::fs::write(
        &component,
        "<script>let x = $state(document.title);</script>\n<Card id=\"a\" />",
    )
    .unwrap();
    let output = common::run_studio(&["check", component.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(10), "validate family");
    let diagnostics: Vec<serde_json::Value> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    assert!(
        diagnostics
            .iter()
            .any(|record| record["code"] == "STUDIO300"),
        "subset violation reported, got {diagnostics:?}"
    );
}

#[test]
fn build_compiles_a_studio_entry_project_end_to_end() {
    let root = TempDir::new().unwrap();
    let project = seed_studio_project(root.path());

    let output = common::run_studio(&["build", project.to_str().unwrap()]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let bundle = project.join("build/cards.studio");
    assert!(bundle.is_file(), "bundle produced for the studio entry");
    let generated = project.join("build/app.generated.ts");
    assert!(generated.is_file(), "generated assembly is materialized");
}

#[test]
fn shape_violations_report_studio015_with_spans() {
    let root = TempDir::new().unwrap();
    let project = seed_studio_project(root.path());
    let component = project.join("components").join("app.studio");

    for (source, expected) in [
        (
            "<Card id=\"c\" title=\"x\"><Text id=\"t\">Hi</Text></Card>",
            "property `title` is not known for `Card`",
        ),
        (
            "<Card id=\"c\"><Text id=\"a\">A</Text><Text id=\"b\">B</Text></Card>",
            "takes at most one child",
        ),
        (
            "<Text id=\"t\"><Button id=\"b\" label=\"x\" /></Text>",
            "cannot contain nested content",
        ),
    ] {
        std::fs::write(&component, source).unwrap();
        let output = common::run_studio(&["check", component.to_str().unwrap()]);
        assert_eq!(
            output.status.code(),
            Some(10),
            "validate family for {source}"
        );
        let diagnostics: Vec<serde_json::Value> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        assert!(
            diagnostics
                .iter()
                .any(|record| record["code"] == "STUDIO015"
                    && record["message"]
                        .as_str()
                        .unwrap_or_default()
                        .contains(expected)),
            "STUDIO015 with '{expected}', got {diagnostics:?}"
        );
    }

    // The valid seed still passes: no false positives on good trees.
    std::fs::write(
        &component,
        "<script lang=\"ts\">\n  let { title = \"Cards\" } = $props();\n  let count = $state(2);\n</script>\n<Card id=\"cards-root\">\n  <Column id=\"cards-col\">\n    <Text id=\"cards-count\">{count}</Text>\n  </Column>\n</Card>\n",
    )
    .unwrap();
    let output = common::run_studio(&["check", component.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
}
