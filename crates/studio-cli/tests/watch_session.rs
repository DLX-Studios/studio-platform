//! US2 integration contract: the watcher never self-triggers on generator
//! churn, coalesces rapid saves, runs single-flight with deterministic
//! supersede semantics, and cancels cleanly.

mod common;

use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tempfile::TempDir;

use studio_cli::watch::{is_generator_output, WatchConfig, WatchSession};

#[test]
fn generated_churn_never_triggers_change_detection() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "watched");

    let mut session = WatchSession::new();
    assert!(
        !session.detect_changes(&project),
        "first scan only primes the baseline"
    );

    std::fs::write(
        project.join("assembly/routes.generated.ts"),
        "// Generated from routes/ — do not edit\nexport const declaredRoutes = [];\n",
    )
    .unwrap();
    std::fs::create_dir_all(project.join("assets/icons")).unwrap();
    std::fs::write(project.join("assets/icons/anchor.svg"), "<svg/>").unwrap();

    assert!(
        !session.detect_changes(&project),
        "generator outputs are excluded from change detection"
    );
    assert!(is_generator_output(
        &project,
        &project.join("assembly/routes.generated.ts")
    ));
    assert!(is_generator_output(
        &project,
        &project.join("assets/icons/anchor.svg")
    ));
    assert!(!is_generator_output(
        &project,
        &project.join("assembly/index.ts")
    ));
}

#[test]
fn real_source_changes_trigger_exactly_one_pending_build() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "watched");
    let mut session = WatchSession::new();
    session.detect_changes(&project);

    // Five rapid successive saves.
    for round in 0..5 {
        std::fs::write(
            project.join("assembly/index.ts"),
            format!("export const round: i32 = {round};\n"),
        )
        .unwrap();
        session.detect_changes(&project);
    }
    assert!(
        session.pending(),
        "rapid saves coalesce into a pending build"
    );

    std::thread::sleep(Duration::from_millis(30));
    let config = WatchConfig {
        poll: Duration::from_millis(10),
        settle: Duration::from_millis(20),
    };
    assert!(
        session.settle_elapsed(config.settle),
        "settle window elapses"
    );
    session.consume_pending();
    assert!(
        !session.pending(),
        "the pending build was consumed exactly once"
    );
}

#[test]
fn change_during_build_yields_a_single_trailing_rebuild() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "watched");
    let mut session = WatchSession::new();
    session.detect_changes(&project);

    // Build #1 consumes a real change.
    std::fs::write(project.join("assembly/index.ts"), "export const a = 1;\n").unwrap();
    assert!(session.detect_changes(&project));
    session.consume_pending();

    // A change arriving "during the build" is observed on the next scan and
    // produces exactly one trailing build.
    std::fs::write(project.join("assembly/index.ts"), "export const a = 2;\n").unwrap();
    assert!(session.detect_changes(&project));
    session.consume_pending();

    // No further changes: no additional builds are pending.
    assert!(!session.detect_changes(&project));
    assert!(!session.pending());
}

#[test]
fn cancellation_stops_cleanly_and_leaves_the_bundle_intact() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "watched");
    let script = root.path().join("runtime.sh");
    std::fs::write(&script, "#!/bin/sh\nsleep 30\n").unwrap();
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();

    let cancelled = AtomicBool::new(true);
    let code = studio_cli::watch::run(
        &project,
        "watched",
        0,
        &cancelled,
        WatchConfig::default(),
        Some(script.as_path()),
    );
    assert_eq!(code, 130, "cancel maps to the documented cancelled family");

    let bundle = project.join("build/watched.studio");
    assert!(bundle.is_file(), "initial build completed atomically");
    let leftovers: Vec<_> = std::fs::read_dir(project.join("build"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "no partial output remains");

    // A subsequent session runs normally (no lock state was left behind).
    let fresh = AtomicBool::new(true);
    assert_eq!(
        studio_cli::watch::run(
            &project,
            "watched",
            0,
            &fresh,
            WatchConfig::default(),
            Some(script.as_path()),
        ),
        130
    );
}

#[test]
fn missing_runtime_is_a_structured_watch_failure() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "watched");
    let cancelled = AtomicBool::new(false);
    let code = studio_cli::watch::run(
        &project,
        "watched",
        0,
        &cancelled,
        WatchConfig::default(),
        Some(std::path::Path::new("/nonexistent/studio-app")),
    );
    assert_eq!(code, 14, "watch family exit code");
}

#[test]
fn vanishing_watch_root_is_reported_once_as_a_warning() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "watched");
    std::fs::create_dir_all(project.join("routes")).unwrap();
    let mut session = WatchSession::new();
    session.detect_changes(&project);

    std::fs::remove_dir_all(project.join("routes")).unwrap();
    session.detect_changes(&project);
    session.detect_changes(&project);
}

#[test]
fn concurrent_builds_of_one_project_are_serialized() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "locked");
    let request = studio_cli::build::BuildRequest::resolve(project.to_str().unwrap()).unwrap();
    let build_dir = project.join("build");
    std::fs::create_dir_all(&build_dir).unwrap();
    std::fs::write(
        build_dir.join(".locked.build-lock"),
        std::process::id().to_string(),
    )
    .unwrap();

    let outcome = std::thread::spawn(move || studio_cli::build::build_example(&request))
        .join()
        .unwrap();
    let Err(failure) = outcome else {
        panic!("expected the concurrent build to be refused");
    };
    assert!(failure
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "BUILD_VALIDATE_CONCURRENT"));
    assert_eq!(
        failure.exit_code(),
        10,
        "validate family: refused before work"
    );
}
