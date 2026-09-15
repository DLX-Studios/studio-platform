//! Content-hash dev watcher: debounced, single-flight, generator-aware.
//!
//! Change detection hashes watched files instead of trusting mtimes and
//! excludes generator outputs (`assembly/routes.generated.ts`, lucide icon
//! writes under `assets/icons/`), so builds never self-trigger. Rapid saves
//! coalesce inside the settle window; a change observed after a build started
//! produces exactly one trailing rebuild with the final state.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::build::{build_example, BuildDiagnostic, BuildPhase, BuildRequest, EXIT_CANCELLED};
use studio_protocol::reload::{
    connect_endpoint, decode_outcome, encode_request, encode_restart, read_line, send_line,
    socket_path, DecodedOutcome, ReloadRequest,
};

/// Watch tuning; the defaults match the documented contract.
#[derive(Clone, Copy, Debug)]
pub struct WatchConfig {
    /// Directory scan interval.
    pub poll: Duration,
    /// Debounce window coalescing rapid saves before rebuilding.
    pub settle: Duration,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            poll: Duration::from_millis(500),
            settle: Duration::from_millis(300),
        }
    }
}

/// True when a watched path is generator output that must never trigger a
/// rebuild.
#[must_use]
pub fn is_generator_output(project: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(project) else {
        return false;
    };
    let relative = relative.to_string_lossy().replace('\\', "/");
    relative == "assembly/routes.generated.ts" || relative.starts_with("assets/icons/")
}

/// Every watched source file, sorted and deduplicated, generator outputs
/// excluded.
///
/// Besides the classic `assembly/`, `routes/`, and `assets/` roots, Studio
/// Script sources live in `components/` and as top-level `*.studio` files;
/// generated `build/` output is never watched.
#[must_use]
pub fn watched_files(project: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for root in ["assembly", "routes", "assets", "components"] {
        let root = project.join(root);
        if !root.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&root) {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path().to_path_buf();
            if is_generator_output(project, &path) {
                continue;
            }
            files.push(path);
        }
    }
    if let Ok(read) = std::fs::read_dir(project) {
        for entry in read.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension == "studio")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    files.dedup();
    files
}

fn content_hash(path: &Path) -> Option<[u8; 32]> {
    let bytes = std::fs::read(path).ok()?;
    Some(Sha256::digest(&bytes).into())
}

/// Change-detection state for one watch session.
#[derive(Debug, Default)]
pub struct WatchSession {
    hashes: BTreeMap<PathBuf, [u8; 32]>,
    primed: bool,
    pending_since: Option<Instant>,
    reported_roots: BTreeMap<String, bool>,
}

impl WatchSession {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Re-scan watched files. Returns true when real source content changed
    /// since the previous scan; the first scan only primes the baseline. A
    /// watched root that disappears is reported once as a structured warning.
    pub fn detect_changes(&mut self, project: &Path) -> bool {
        for root in ["assembly", "routes", "assets"] {
            let exists = project.join(root).exists();
            let previous = self.reported_roots.insert(root.to_owned(), exists);
            if previous == Some(true) && !exists {
                BuildDiagnostic::warning(
                    BuildPhase::Watch,
                    "BUILD_WATCH_ROOT_GONE",
                    format!("watched directory {root}/ no longer exists"),
                )
                .report();
            }
        }
        let mut changed = false;
        let mut seen = std::collections::BTreeSet::new();
        for path in watched_files(project) {
            seen.insert(path.clone());
            let hash = content_hash(&path);
            match (self.hashes.get(&path), hash) {
                (None, Some(current)) => {
                    if self.primed {
                        changed = true;
                    }
                    self.hashes.insert(path, current);
                }
                (Some(previous), Some(current)) => {
                    if previous != &current {
                        changed = true;
                        self.hashes.insert(path, current);
                    }
                }
                (_, None) => {
                    if self.hashes.remove(&path).is_some() {
                        changed = true;
                    }
                }
            }
        }
        self.hashes.retain(|path, _| seen.contains(path));
        self.primed = true;
        if changed && self.pending_since.is_none() {
            self.pending_since = Some(Instant::now());
        }
        changed
    }

    /// True when the settle window has elapsed since the first pending change.
    #[must_use]
    pub fn settle_elapsed(&self, settle: Duration) -> bool {
        self.pending_since
            .is_some_and(|since| since.elapsed() >= settle)
    }

    /// Clear pending state after a build consumed it.
    pub fn consume_pending(&mut self) {
        self.pending_since = None;
    }

    /// Whether a rebuild is currently pending.
    #[must_use]
    pub fn pending(&self) -> bool {
        self.pending_since.is_some()
    }
}

/// Raise the soft stack ceiling before launching the runtime host.
///
/// Unoptimized GPUI frames are large: a 10-deep plugin tree overflows the
/// default 8 MiB main-thread stack in dev builds (release inlines this
/// away). Best-effort — keeps the inherited limit when raising fails — and
/// inherited by the spawned runtime.
#[allow(
    clippy::useless_conversion,
    reason = "libc::rlim_t width is platform-dependent; u64::from is a no-op only on 64-bit targets"
)]
pub fn raise_dev_stack() {
    const DEV_STACK_BYTES: u64 = 64 * 1024 * 1024;
    let mut current = std::mem::MaybeUninit::<libc::rlimit>::uninit();
    // SAFETY: `current` is a valid out-pointer; `getrlimit` writes exactly
    // one `rlimit` on success, and the pointer is not read otherwise.
    let ok = unsafe { libc::getrlimit(libc::RLIMIT_STACK, current.as_mut_ptr()) == 0 };
    if !ok {
        return;
    }
    // SAFETY: initialized by the successful `getrlimit` above.
    let current = unsafe { current.assume_init() };
    let soft = u64::from(current.rlim_cur);
    let hard = u64::from(current.rlim_max);
    if soft == u64::from(libc::RLIM_INFINITY) || soft >= DEV_STACK_BYTES {
        return;
    }
    let target = DEV_STACK_BYTES.min(hard);
    if target <= soft {
        return;
    }
    let raised = libc::rlimit {
        rlim_cur: target.try_into().unwrap_or(libc::rlim_t::MAX),
        rlim_max: current.rlim_max,
    };
    // SAFETY: `raised` is a valid `rlimit`; failure keeps the inherited
    // limit and is ignored by design.
    unsafe {
        libc::setrlimit(libc::RLIMIT_STACK, &raised);
    }
}

pub fn runtime_binary() -> PathBuf {
    if let Ok(binary) = std::env::var("STUDIO_APP_BINARY") {
        return PathBuf::from(binary);
    }
    if let Ok(target) = std::env::var("CARGO_TARGET_DIR") {
        let candidate = PathBuf::from(target).join("debug/studio-app");
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/debug/studio-app")
}

/// Run the dev watch loop until cancelled or the runtime exits.
///
/// Builds once up front, launches the runtime host in development mode, then
/// rebuilds on settled source changes. Returns the process exit code.
/// `runtime_override` replaces the resolved runtime binary (used by tests).
pub fn run(
    project: &Path,
    example: &str,
    port: u16,
    cancel: &AtomicBool,
    config: WatchConfig,
    runtime_override: Option<&Path>,
) -> i32 {
    println!(
        "studio dev {example} on :{port} watching assembly/ routes/ assets/ (generator outputs excluded)"
    );
    let bundle = match build_example(&BuildRequest {
        example: example.to_owned(),
        project_root: project.to_path_buf(),
        output_bundle: project.join("build").join(format!("{example}.studio")),
    }) {
        Ok(bundle) => bundle,
        Err(failure) => {
            failure.report();
            return failure.exit_code();
        }
    };
    println!("built {}", bundle.display());

    let socket = socket_path(&project.join("build"));
    let binary = runtime_override
        .map(Path::to_path_buf)
        .unwrap_or_else(runtime_binary);
    raise_dev_stack();
    let child = Command::new(&binary)
        .arg("--dev")
        .arg(&bundle)
        .arg("--reload-socket")
        .arg(&socket)
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .env("GALLIUM_DRIVER", "llvmpipe")
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(error) => {
            BuildDiagnostic::error(
                BuildPhase::Watch,
                "BUILD_WATCH_RUNTIME",
                format!("launch runtime host {}: {error}", binary.display()),
            )
            .report();
            return BuildPhase::Watch.exit_code();
        }
    };

    let mut session = WatchSession::new();
    loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return EXIT_CANCELLED;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                BuildDiagnostic::error(
                    BuildPhase::Watch,
                    "BUILD_WATCH_RUNTIME_EXITED",
                    format!("runtime host exited: {status}"),
                )
                .report();
                return BuildPhase::Watch.exit_code();
            }
            Ok(None) => {}
            Err(error) => {
                BuildDiagnostic::error(
                    BuildPhase::Watch,
                    "BUILD_WATCH_RUNTIME",
                    format!("poll runtime host: {error}"),
                )
                .report();
                return BuildPhase::Watch.exit_code();
            }
        }
        std::thread::sleep(config.poll);
        session.detect_changes(project);
        if session.pending() && session.settle_elapsed(config.settle) {
            session.consume_pending();
            println!("change detected — rebuilding");
            let request = BuildRequest {
                example: example.to_owned(),
                project_root: project.to_path_buf(),
                output_bundle: project.join("build").join(format!("{example}.studio")),
            };
            let rebuilt = match build_example(&request) {
                Ok(bundle) => bundle,
                Err(failure) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    failure.report();
                    return failure.exit_code();
                }
            };
            // A change that arrived while building was already captured by the
            // next detect_changes; the loop naturally performs exactly one
            // trailing rebuild with the final state.
            println!("rebuild ok — reloading {}", rebuilt.display());
            let routes = crate::build::scan_routes(project);
            if let Err(code) = request_reload(&rebuilt, &socket, &routes) {
                return code;
            }
        }
    }
}

/// Send one reload request for a fresh bundle and await the outcome.
///
/// Returns the process exit code when the reload fails; the session ends
/// because a lost or rejecting runtime cannot meaningfully continue.
pub fn request_reload(bundle: &Path, socket: &Path, routes: &[String]) -> Result<(), i32> {
    let mut stream = connect_endpoint(socket).map_err(|error| {
        BuildDiagnostic::error(
            BuildPhase::Watch,
            "BUILD_WATCH_RELOAD_CHANNEL",
            format!("connect reload channel {}: {error}", socket.display()),
        )
        .report();
        BuildPhase::Watch.exit_code()
    })?;
    let request = ReloadRequest {
        request_id: format!("reload-{}", std::process::id()),
        bundle: bundle.display().to_string(),
        base_revision: 0,
        declared_routes: routes.to_vec(),
    };
    if send_line(&mut stream, &encode_request(&request)).is_err() {
        BuildDiagnostic::error(
            BuildPhase::Watch,
            "BUILD_WATCH_RELOAD_CHANNEL",
            "reload channel broke while sending",
        )
        .report();
        return Err(BuildPhase::Watch.exit_code());
    }
    match read_line(&stream, Duration::from_secs(30)) {
        Ok(line) => match decode_outcome(&line) {
            Ok(DecodedOutcome::Accepted { revision, .. }) => {
                println!("reloaded revision {revision}");
                Ok(())
            }
            Ok(DecodedOutcome::Rejected {
                stage,
                code,
                message,
                ..
            }) => {
                BuildDiagnostic::error(
                    BuildPhase::Watch,
                    code,
                    format!(
                        "reload rejected at {}: {message}",
                        stage.unwrap_or_else(|| "unknown".to_owned())
                    ),
                )
                .report();
                Err(BuildPhase::Watch.exit_code())
            }
            Err(error) => {
                BuildDiagnostic::error(
                    BuildPhase::Watch,
                    "BUILD_WATCH_RELOAD_CHANNEL",
                    format!("undecodable reload outcome: {error}"),
                )
                .report();
                Err(BuildPhase::Watch.exit_code())
            }
        },
        Err(_) => {
            BuildDiagnostic::error(
                BuildPhase::Watch,
                "BUILD_WATCH_RELOAD_CHANNEL",
                "reload outcome timed out",
            )
            .report();
            Err(BuildPhase::Watch.exit_code())
        }
    }
}

/// Send a restart request to a running session's socket.
///
/// Returns the process exit code.
pub fn restart_session(socket: &Path) -> i32 {
    let mut stream = match connect_endpoint(socket) {
        Ok(stream) => stream,
        Err(error) => {
            BuildDiagnostic::error(
                BuildPhase::Watch,
                "BUILD_WATCH_RELOAD_CHANNEL",
                format!("connect reload channel {}: {error}", socket.display()),
            )
            .report();
            return BuildPhase::Watch.exit_code();
        }
    };
    let id = format!("restart-{}", std::process::id());
    if send_line(&mut stream, &encode_restart(&id)).is_err() {
        BuildDiagnostic::error(
            BuildPhase::Watch,
            "BUILD_WATCH_RELOAD_CHANNEL",
            "reload channel broke while sending",
        )
        .report();
        return BuildPhase::Watch.exit_code();
    }
    match read_line(&stream, Duration::from_secs(30)) {
        Ok(line) => match decode_outcome(&line) {
            Ok(DecodedOutcome::Accepted { revision, .. }) => {
                println!("restarted at revision {revision}");
                crate::build::EXIT_SUCCESS
            }
            Ok(DecodedOutcome::Rejected { code, message, .. }) => {
                BuildDiagnostic::error(BuildPhase::Watch, code, message).report();
                BuildPhase::Watch.exit_code()
            }
            Err(error) => {
                BuildDiagnostic::error(
                    BuildPhase::Watch,
                    "BUILD_WATCH_RELOAD_CHANNEL",
                    format!("undecodable restart outcome: {error}"),
                )
                .report();
                BuildPhase::Watch.exit_code()
            }
        },
        Err(_) => {
            BuildDiagnostic::error(
                BuildPhase::Watch,
                "BUILD_WATCH_RELOAD_CHANNEL",
                "restart outcome timed out",
            )
            .report();
            BuildPhase::Watch.exit_code()
        }
    }
}
