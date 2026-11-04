//! Web target exports compiled Studio sources rather than a separately authored app.
use std::{fs, path::Path, process::Command};

fn fixture(root: &Path, kind: &str) {
    fs::create_dir_all(root.join("routes")).unwrap();
    fs::write(
        root.join("manifest.json"),
        include_str!("../../../examples/stream-countdown/manifest.json")
            .replace("\"assets/vice-city-sunset.svg\",", "")
            .replace("\"assets/stream-starting.svg\",", "")
            .replace("\"assets/stream-live.svg\"", ""),
    )
    .unwrap();
    fs::write(
        root.join("routes/+page.studio"),
        format!("<{kind} id=\"home\" />"),
    )
    .unwrap();
}

#[test]
fn web_target_exports_compiled_module_and_shared_route_metadata() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), "Text");
    // This filesystem export test supplies bridge fixtures. Real Rust bridge/guest
    // execution and the actual countdown are exercised by browser acceptance.
    let core = dir.path().join("core");
    fs::create_dir(&core).unwrap();
    fs::write(
        core.join("studio_web_core.js"),
        "export default function init() {};",
    )
    .unwrap();
    fs::write(core.join("studio_web_core_bg.wasm"), b"\0asm\x01\0\0\0").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_studio"))
        .args(["build", dir.path().to_str().unwrap(), "--target", "web"])
        .env("STUDIO_WEB_CORE_DIR", &core)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let exported = dir.path().join("build/web");
    for name in [
        "index.html",
        "app.js",
        "worker.js",
        "style.css",
        "module.wasm",
        "web-config.json",
        "core/studio_web_core.js",
        "core/studio_web_core_bg.wasm",
    ] {
        assert!(exported.join(name).is_file(), "missing {name}");
    }
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(exported.join("web-config.json")).unwrap()).unwrap();
    assert_eq!(config["routing"]["routes"][0]["path"], "/");
    assert!(fs::read(exported.join("module.wasm"))
        .unwrap()
        .starts_with(b"\0asm"));
    assert!(!dir.path().join("app.studio").exists());
}

#[test]
fn web_target_rejects_unsupported_components_without_touching_last_good_output() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), "Card");
    let exported = dir.path().join("build/web");
    fs::create_dir_all(&exported).unwrap();
    fs::write(exported.join("index.html"), "last good").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_studio"))
        .args(["build", dir.path().to_str().unwrap(), "--target", "web"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("BUILD_WEB_UNSUPPORTED_KIND"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(exported.join("index.html")).unwrap(),
        "last good"
    );
}

#[test]
fn web_target_reports_properties_without_a_browser_mapping_before_export() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), "Scaffold");
    fs::write(
        dir.path().join("routes/+page.studio"),
        "<Scaffold id=\"shell\" title=\"Unsupported title slot\" />",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_studio"))
        .args(["build", dir.path().to_str().unwrap(), "--target", "web"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("BUILD_WEB_UNSUPPORTED_PROPERTY"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn standalone_studio_sources_export_at_the_browser_root() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path(), "Text");
    fs::remove_dir_all(dir.path().join("routes")).unwrap();
    fs::write(
        dir.path().join("app.studio"),
        "<Text id=\"home\">Standalone</Text>",
    )
    .unwrap();
    let core = dir.path().join("core");
    fs::create_dir(&core).unwrap();
    fs::write(
        core.join("studio_web_core.js"),
        "export default function init() {};",
    )
    .unwrap();
    fs::write(core.join("studio_web_core_bg.wasm"), b"\0asm\x01\0\0\0").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_studio"))
        .args(["build", dir.path().to_str().unwrap(), "--target", "web"])
        .env("STUDIO_WEB_CORE_DIR", &core)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.path().join("build/web/web-config.json")).unwrap())
            .unwrap();
    assert_eq!(
        config["routing"]["routes"][0]["path"], "/",
        "standalone apps must open from the hosted root"
    );
}
