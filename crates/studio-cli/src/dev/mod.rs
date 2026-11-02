//! Native development host owned by the `studio` executable.
//! One worker hashes and compiles inputs; the UI thread owns live state and commits updates.
mod compiler;
mod project;
mod rpc;
mod surface;

use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::{Duration, Instant};

use gpui_kit::assets::Assets;
use gpui_kit::component::{Root, Theme, ThemeMode};
use gpui_kit::{
    px, size, App, AppContext, Bounds, Context as GpuiContext, Entity, IntoElement, Render,
    Subscription, Window, WindowBounds, WindowOptions,
};
use studio_components::native_view::{NativeRuntimeView, NativeViewEvent};
use studio_package::TrustStore;
use studio_protocol::dev_rpc::{DevPhase, DevViewport};
use studio_runtime::gpui_https_client::GpuiHttpsClient;
use studio_runtime::{
    cli::{LaunchMode, LaunchRequest},
    host::NativeDisplayAvailability,
    StudioHost,
};

use crate::build::{BuildDiagnostic, BuildPhase, BuildRequest};
use compiler::{Artifact, Code, DevCompiler};
use surface::{DevSurface, Replacement};

type Acknowledgement = Result<u64, String>;
struct Update {
    replacement: Replacement,
    reset: bool,
    hash: String,
    acknowledge: mpsc::SyncSender<Acknowledgement>,
}
struct Restart(mpsc::SyncSender<Acknowledgement>);
struct SessionGuard {
    cancelled: Arc<AtomicBool>,
    socket: Option<PathBuf>,
}
impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Some(socket) = &self.socket {
            let _ = std::fs::remove_file(socket);
        }
    }
}

fn report(message: impl Into<String>) {
    BuildDiagnostic::error(BuildPhase::Watch, "DEV_RELOAD", message).report();
}

/// Open a native preview and incrementally reload loose IR, Wasm and asset artifacts.
/// The CLI owns the window; `studio-app` is never launched by this command.
pub fn run(mut request: BuildRequest, _port: u16) -> i32 {
    request.project_root = match request.project_root.canonicalize() {
        Ok(root) => root,
        Err(error) => {
            report(error.to_string());
            return BuildPhase::Io.exit_code();
        }
    };
    if let Err(error) = std::fs::create_dir_all(request.project_root.join("build")) {
        report(error.to_string());
        return BuildPhase::Io.exit_code();
    }
    let socket = studio_protocol::reload::socket_path(&request.project_root.join("build"));
    if studio_protocol::reload::connect_endpoint(&socket).is_ok() {
        report("a development session already owns this project's restart socket");
        return BuildPhase::Watch.exit_code();
    }
    let listener = match studio_protocol::reload::bind_endpoint(&socket) {
        Ok(listener) => listener,
        Err(error) => {
            report(format!("{}: {error}", socket.display()));
            return BuildPhase::Watch.exit_code();
        }
    };
    if let Err(error) = listener.set_nonblocking(true) {
        report(error.to_string());
        return BuildPhase::Watch.exit_code();
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let guard = SessionGuard {
        cancelled: Arc::clone(&cancelled),
        socket: Some(socket),
    };
    let status = rpc::StatusSnapshot::default();
    let (preview_command_sender, preview_command_receiver) = mpsc::sync_channel(16);
    let status_server = match rpc::Server::start(
        &request.project_root.join("build"),
        status.clone(),
        preview_command_sender,
    ) {
        Ok(server) => server,
        Err(error) => {
            report(format!(
                "could not start the development status endpoint: {error}"
            ));
            return BuildPhase::Watch.exit_code();
        }
    };
    let (restart_sender, restart_receiver) = mpsc::sync_channel(1);
    let socket_cancel = Arc::clone(&cancelled);
    let control_thread =
        std::thread::spawn(move || control_server(listener, restart_sender, &socket_cancel));
    println!(
        "studio dev {} — native host; content-hashed IR/Wasm updates",
        request.example
    );
    let code = window(
        Some(request),
        None,
        cancelled,
        Some(restart_receiver),
        Some(status.clone()),
        Some(preview_command_receiver),
    );
    status.set_phase(DevPhase::Stopping);
    drop(guard);
    drop(status_server);
    let _ = control_thread.join();
    code
}

/// Preview an already built local bundle in the CLI's development host.
pub fn preview(bundle: &Path) -> i32 {
    window(
        None,
        Some(bundle.to_path_buf()),
        Arc::new(AtomicBool::new(false)),
        None,
        None,
        None,
    )
}

struct DevPreview {
    view: Entity<NativeRuntimeView<DevSurface>>,
    _selection_subscription: Subscription,
    status: Option<rpc::StatusSnapshot>,
}

impl DevPreview {
    fn new(
        reduced_motion: bool,
        initial: DevSurface,
        window: &mut Window,
        cx: &mut GpuiContext<Self>,
        status: Option<rpc::StatusSnapshot>,
    ) -> Self {
        let view =
            cx.new(|cx| NativeRuntimeView::with_surface(reduced_motion, initial, window, cx));
        let selection_subscription =
            cx.subscribe(&view, |this, _, event: &NativeViewEvent, _cx| {
                if let NativeViewEvent::Selected(node_id) = event {
                    if let Some(status) = &this.status {
                        status.set_selection(node_id.clone());
                    }
                }
            });
        Self {
            view,
            _selection_subscription: selection_subscription,
            status,
        }
    }

    fn apply_surface_updates(
        &mut self,
        updates: &mpsc::Receiver<Update>,
        revision: &mut u64,
        cx: &mut GpuiContext<Self>,
    ) {
        while let Ok(update) = updates.try_recv() {
            let Update {
                replacement,
                reset,
                hash,
                acknowledge,
            } = update;
            let status = self.status.clone();
            let mut metadata = None;
            let mut outcome = Err(String::from("no live surface"));
            self.view.update(cx, |view, view_cx| {
                view.update_surface(
                    |surface| match surface.replace(replacement, reset) {
                        Ok(()) => {
                            metadata = surface.metadata();
                            *revision += 1;
                            println!(
                                "[dev] committed revision {} ({})",
                                *revision,
                                &hash[..hash.len().min(12)]
                            );
                            outcome = Ok(*revision);
                            true
                        }
                        Err(error) => {
                            outcome = Err(error);
                            false
                        }
                    },
                    view_cx,
                );
                if outcome.is_ok() {
                    if let (Some(status), Some(metadata)) = (&status, metadata) {
                        debug_assert!(metadata.source_locations.values().all(|location| {
                            metadata.screen_source_path.as_deref() == Some(location.path.as_str())
                        }));
                        status.set_surface_identity(
                            metadata.component_id,
                            metadata.screen_id,
                            metadata.available_screen_ids,
                            metadata.screen_source_path,
                            metadata.source_locations,
                        );
                    }
                    if let Some(status) = &status {
                        let current = status.read();
                        view.set_authoring(
                            current.authoring_selection,
                            current.selection.map(|selection| selection.node_id),
                            view_cx,
                        );
                    }
                }
            });
            let _ = acknowledge.send(outcome);
        }
    }

    fn apply_commands(
        &mut self,
        commands: &mpsc::Receiver<rpc::PendingCommand>,
        window: &mut Window,
        cx: &mut GpuiContext<Self>,
    ) {
        while let Ok(pending) = commands.try_recv() {
            let status = self.status.clone();
            let result = match (pending.command, status) {
                (rpc::DevCommand::SetAuthoringSelection(enabled), Some(status)) => {
                    let current = status.read();
                    self.view.update(cx, |view, view_cx| {
                        view.set_authoring(
                            enabled,
                            current.selection.map(|selection| selection.node_id),
                            view_cx,
                        );
                    });
                    status.set_authoring_selection(enabled);
                    Ok(status.read())
                }
                (rpc::DevCommand::SetPresentation(update), Some(status)) => {
                    let previous = status.read().presentation.clone();
                    let requested = update.apply_to(&previous);
                    match requested {
                        Ok(requested) => {
                            let screen_changed = requested.screen_id != previous.screen_id;
                            let zoom_changed = requested.zoom_milli != previous.zoom_milli;
                            let mut metadata = None;
                            let mut switch_error = None;
                            self.view.update(cx, |view, view_cx| {
                                if screen_changed {
                                    if let Some(screen_id) = requested.screen_id.as_deref() {
                                        view.update_surface(
                                            |surface| match surface.switch_screen(screen_id) {
                                                Ok(_) => {
                                                    metadata = surface.metadata();
                                                    true
                                                }
                                                Err(error) => {
                                                    switch_error = Some(error);
                                                    false
                                                }
                                            },
                                            view_cx,
                                        );
                                    }
                                    if switch_error.is_none() {
                                        view.set_authoring(
                                            status.read().authoring_selection,
                                            None,
                                            view_cx,
                                        );
                                    }
                                }
                                if switch_error.is_none() && zoom_changed {
                                    view.set_zoom_milli(requested.zoom_milli, window, view_cx);
                                }
                            });
                            if let Some(error) = switch_error {
                                Err(rpc::RpcCommandError {
                                    code: -32602,
                                    message: error,
                                })
                            } else {
                                if let Some(metadata) = metadata {
                                    status.set_surface_identity(
                                        metadata.component_id,
                                        metadata.screen_id,
                                        metadata.available_screen_ids,
                                        metadata.screen_source_path,
                                        metadata.source_locations,
                                    );
                                }
                                let status = status.set_presentation(&update);
                                if status.as_ref().is_ok_and(|current| {
                                    current.presentation.viewport != previous.viewport
                                }) {
                                    window.resize(size(
                                        px(requested.viewport.width as f32),
                                        px(requested.viewport.height as f32),
                                    ));
                                }
                                status
                            }
                        }
                        Err(error) => Err(rpc::RpcCommandError {
                            code: -32602,
                            message: error.to_string(),
                        }),
                    }
                }
                (_, None) => Err(rpc::RpcCommandError {
                    code: -32603,
                    message: "preview control is unavailable".to_owned(),
                }),
            };
            let _ = pending.response.send(result);
        }
    }

    fn pump_host_actions(&mut self, cx: &mut GpuiContext<Self>) {
        let result = self.view.update(cx, |view, _cx| view.pump_host_actions());
        match result {
            Ok(true) => cx.notify(),
            Err(error) => {
                if let Some(status) = &self.status {
                    status.set_diagnostic("DEV_HOST_ACTION", error.clone());
                }
                report(error);
            }
            Ok(false) => {}
        }
    }

    fn set_observed_viewport(&self, viewport: DevViewport) {
        if let Some(status) = &self.status {
            status.set_observed_viewport(viewport);
        }
    }
}

impl Render for DevPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut GpuiContext<Self>) -> impl IntoElement {
        self.view.clone()
    }
}

fn window(
    request: Option<BuildRequest>,
    bundle: Option<PathBuf>,
    cancelled: Arc<AtomicBool>,
    restarts: Option<mpsc::Receiver<Restart>>,
    dev_status: Option<rpc::StatusSnapshot>,
    preview_commands: Option<mpsc::Receiver<rpc::PendingCommand>>,
) -> i32 {
    if NativeDisplayAvailability::from_environment() == NativeDisplayAvailability::Unavailable {
        if let Some(status) = &dev_status {
            status.set_failure(
                "DEV_DISPLAY_UNAVAILABLE",
                "Studio requires a graphical session (Wayland or X11 on Linux)",
            );
        }
        report("Studio requires a graphical session (Wayland or X11 on Linux)");
        return 2;
    }
    crate::watch::raise_dev_stack();
    let https = Arc::new(GpuiHttpsClient::default());
    let host = match studio_runtime::bootstrap::host_from_environment(
        TrustStore::default(),
        https.clone(),
    ) {
        Ok(host) => host,
        Err(error) => {
            if let Some(status) = &dev_status {
                status.set_failure("DEV_HOST_INIT", error.to_string());
            }
            report(error);
            return 2;
        }
    };
    let (sender, receiver) = mpsc::sync_channel::<Update>(1);
    let initial = match bundle {
        Some(bundle) => match host.prepare(LaunchRequest::new(LaunchMode::Development, bundle)) {
            Ok(surface) => DevSurface::Wasm(Box::new(surface)),
            Err(error) => {
                report(error.to_string());
                return 2;
            }
        },
        None => DevSurface::Empty,
    };
    let worker = Arc::new(std::sync::Mutex::new(None));
    let app_worker = Arc::clone(&worker);
    let cancel_app = Arc::clone(&cancelled);
    let status_for_ui = dev_status.clone();
    gpui_kit::platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            let _ = https.install(cx.http_client());
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Light, None, cx);
            if let Some(request) = request {
                let cancelled = Arc::clone(&cancel_app);
                let status = dev_status
                    .clone()
                    .expect("dev sessions expose a status snapshot");
                *app_worker.lock().expect("worker handle") = Some(std::thread::spawn(move || {
                    watch(request, host, sender, restarts, cancelled, status)
                }));
            }
            let close_cancel = Arc::clone(&cancel_app);
            cx.on_window_closed(move |cx, _| {
                if cx.windows().is_empty() {
                    close_cancel.store(true, Ordering::SeqCst);
                    cx.quit();
                }
            })
            .detach();
            let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..WindowOptions::default()
                },
                move |window, cx| {
                    let reduced_motion = cx.reduce_motion();
                    let window_handle = window.window_handle();
                    let preview = cx.new(|cx| {
                        DevPreview::new(reduced_motion, initial, window, cx, status_for_ui)
                    });
                    let weak = preview.downgrade();
                    cx.spawn(async move |cx| {
                        let mut revision = 0;
                        loop {
                            cx.background_executor()
                                .timer(Duration::from_millis(32))
                                .await;
                            let mut async_cx = cx.clone();
                            let result = async_cx.update_window(window_handle, |_, window, app| {
                                weak.update(app, |preview, cx| {
                                    preview.apply_surface_updates(&receiver, &mut revision, cx);
                                    if let Some(commands) = preview_commands.as_ref() {
                                        preview.apply_commands(commands, window, cx);
                                    }
                                    let size = window.viewport_size();
                                    preview.set_observed_viewport(DevViewport {
                                        width: size.width.as_f32().round().max(0.0) as u32,
                                        height: size.height.as_f32().round().max(0.0) as u32,
                                    });
                                    preview.pump_host_actions(cx);
                                })
                            });
                            if result.is_err() {
                                break;
                            }
                        }
                    })
                    .detach();
                    cx.new(|cx| Root::new(preview, window, cx))
                },
            )
            .expect("Studio could not create its development window");
            cx.activate(true);
        });
    cancelled.store(true, Ordering::SeqCst);
    // App shutdown drops the receiver, releasing a worker waiting for its commit acknowledgement.
    if let Some(worker) = worker.lock().expect("worker handle").take() {
        let _ = worker.join();
    }
    0
}

fn watch(
    request: BuildRequest,
    host: StudioHost,
    updates: mpsc::SyncSender<Update>,
    restarts: Option<mpsc::Receiver<Restart>>,
    cancelled: Arc<AtomicBool>,
    status: rpc::StatusSnapshot,
) {
    let mut compiler = DevCompiler::new(request);
    compiler.set_cancellation(Arc::clone(&cancelled));
    let mut observed = None;
    let mut pending = Some(Instant::now() - Duration::from_secs(1));
    let mut committed: Option<(String, Vec<u8>)> = None;
    while !cancelled.load(Ordering::SeqCst) {
        let restart = restarts
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok());
        let inputs = match compiler.inputs() {
            Ok(inputs) => inputs,
            Err(error) => {
                if let Some(diagnostic) = error.diagnostics.last() {
                    status.set_failure(diagnostic.code.clone(), diagnostic.message.clone());
                } else {
                    status.set_failure("DEV_INPUTS", "development inputs are unavailable");
                }
                error.report();
                if let Some(Restart(sender)) = restart {
                    let _ = sender.send(Err("development inputs are unavailable".into()));
                }
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
        };
        if observed.as_ref() != Some(&inputs) {
            if observed.is_some() {
                pending = Some(Instant::now());
            }
            observed = Some(inputs.clone());
        }
        let reset = restart.is_some();
        if reset || pending.is_some_and(|since| since.elapsed() >= Duration::from_millis(150)) {
            pending = None;
            status.set_phase(DevPhase::Compiling);
            let mut compile_diagnostic = None;
            let outcome = compiler
                .compile(&inputs)
                .map_err(|failure| {
                    failure.report();
                    compile_diagnostic = failure
                        .diagnostics
                        .last()
                        .map(|diagnostic| (diagnostic.code.clone(), diagnostic.message.clone()));
                    String::from("development compilation failed; keeping the previous revision")
                })
                .and_then(|artifact| {
                    let hash = artifact.code_hash.clone();
                    let manifest = artifact.manifest.clone();
                    let assets_only = !reset
                        && committed.as_ref().is_some_and(|(old_hash, old_manifest)| {
                            old_hash == &hash && old_manifest == &manifest
                        });
                    let replacement = prepare(artifact, &host, assets_only)?;
                    status.set_phase(DevPhase::Applying);
                    let (acknowledge, receiver) = mpsc::sync_channel(1);
                    updates
                        .send(Update {
                            replacement,
                            reset,
                            hash: hash.clone(),
                            acknowledge,
                        })
                        .map_err(|_| "development window closed".to_owned())?;
                    let result = loop {
                        match receiver.recv_timeout(Duration::from_millis(100)) {
                            Ok(result) => break result,
                            Err(mpsc::RecvTimeoutError::Timeout)
                                if !cancelled.load(Ordering::SeqCst) =>
                            {
                                continue;
                            }
                            _ => return Err("development window closed".to_owned()),
                        }
                    };
                    match result {
                        Ok(revision) => {
                            committed = Some((hash.clone(), manifest));
                            status.set_applied(revision, hash);
                            Ok(revision)
                        }
                        Err(error) => Err(error),
                    }
                });
            if let Err(error) = &outcome {
                let (code, message) = compile_diagnostic
                    .take()
                    .unwrap_or_else(|| ("DEV_RELOAD".to_owned(), error.clone()));
                status.set_failure(code, message);
                report(error.clone());
            }
            if let Some(Restart(sender)) = restart {
                let _ = sender.send(outcome);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn prepare(
    artifact: Artifact,
    host: &StudioHost,
    assets_only: bool,
) -> Result<Replacement, String> {
    let policy = studio_package::ArchivePolicy::default();
    let asset_bytes: usize = artifact.assets.values().map(Vec::len).sum();
    if asset_bytes > policy.max_asset_bytes || artifact.assets.len() + 3 > policy.max_entries {
        return Err("development assets exceed package limits".into());
    }
    // Validate manifest changes even when execution is interpreted rather than Wasm.
    studio_package::parse_manifest(
        &artifact.manifest,
        studio_package::ManifestPolicy::default(),
    )
    .map_err(|error| error.to_string())?;
    if assets_only {
        return Ok(Replacement::Assets(artifact.assets));
    }
    match artifact.code {
        Code::Ir(module) => Ok(Replacement::Ir {
            module,
            manifest: artifact.manifest,
            assets: artifact.assets,
            screen_source_paths: artifact.screen_source_paths,
        }),
        Code::Wasm(module) => host
            .prepare_development(&artifact.manifest, (*module).clone(), artifact.assets)
            .map(Box::new)
            .map(Replacement::Wasm)
            .map_err(|error| error.to_string()),
    }
}

fn control_server(
    listener: studio_protocol::reload::ReloadListener,
    restarts: mpsc::SyncSender<Restart>,
    cancelled: &AtomicBool,
) {
    use std::io::{BufRead, BufReader, Read};
    use studio_protocol::reload::{send_line, RELOAD_PROTOCOL};
    while !cancelled.load(Ordering::SeqCst) {
        let (mut stream, _) = match listener.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(_) => break,
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let mut line = String::new();
        if BufReader::new((&stream).take(16_385))
            .read_line(&mut line)
            .is_err()
            || line.len() > 16_384
        {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let id = value["request_id"].as_str().unwrap_or_default();
        let valid =
            value["protocol"] == RELOAD_PROTOCOL && value["type"] == "restart" && !id.is_empty();
        let (sender, receiver) = mpsc::sync_channel(1);
        let outcome = if valid && restarts.try_send(Restart(sender)).is_ok() {
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                match receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(outcome) => break outcome,
                    Err(mpsc::RecvTimeoutError::Timeout)
                        if Instant::now() < deadline && !cancelled.load(Ordering::SeqCst) =>
                    {
                        continue;
                    }
                    _ => break Err("development restart ended before commit".into()),
                }
            }
        } else {
            Err("expected an available studio-reload/1 restart request".into())
        };
        let response = match outcome {
            Ok(revision) => {
                serde_json::json!({"protocol": RELOAD_PROTOCOL, "type": "restarted", "request_id": id, "revision": revision})
            }
            Err(message) => {
                serde_json::json!({"protocol": RELOAD_PROTOCOL, "type": "rejected", "request_id": id, "stage": "commit", "code": "DEV_RESTART", "message": message})
            }
        };
        let _ = send_line(&mut stream, &response.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_components::{native_view::NativeSurface, InputAction};

    #[test]
    fn real_worker_reloads_ir_and_assets_and_acknowledges_restart_after_commit() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let source = "<script lang=\"ts\">let count = $state(1); function add() { count += 1; }</script><Column id=\"content\"><Text id=\"count\">{count}</Text><Button id=\"add\" onclick={add}>Add</Button></Column>";
        std::fs::create_dir(root.join("assets")).unwrap();
        let mut manifest: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../../../examples/starter/manifest.json"))
                .unwrap();
        manifest["assets"] = serde_json::json!(["assets/test.txt"]);
        std::fs::write(root.join("manifest.json"), manifest.to_string()).unwrap();
        std::fs::write(root.join("assets/test.txt"), "before").unwrap();
        std::fs::write(root.join("app.studio"), source).unwrap();
        let resolved = BuildRequest::resolve(root.join(".").to_str().unwrap()).unwrap();
        assert_eq!(resolved.project_root, root.canonicalize().unwrap());
        let request = BuildRequest {
            example: "session".into(),
            project_root: root.to_path_buf(),
            output_bundle: root.join("build/session.studio"),
        };
        let host = StudioHost::new(
            studio_runtime::host::HostConfig::new(TrustStore::default()),
            NativeDisplayAvailability::Available,
        );
        let (sender, receiver) = mpsc::sync_channel(1);
        let (restart_sender, restarts) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancelled);
        let worker = std::thread::spawn(move || {
            watch(
                request,
                host,
                sender,
                Some(restarts),
                worker_cancel,
                rpc::StatusSnapshot::default(),
            )
        });
        // The guard cancels even when an assertion unwinds, so a failed test cannot leave a worker running.
        let guard = SessionGuard {
            cancelled,
            socket: None,
        };
        let mut surface = DevSurface::Empty;
        let first = receiver.recv_timeout(Duration::from_secs(10)).unwrap();
        surface.replace(first.replacement, first.reset).unwrap();
        first.acknowledge.send(Ok(1)).unwrap();
        surface
            .process_input("add", InputAction::PointerClick)
            .unwrap();
        std::fs::write(
            root.join("app.studio"),
            source.replace("$state(1)", "$state(99)"),
        )
        .unwrap();
        let second = receiver.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(!second.reset);
        surface.replace(second.replacement, second.reset).unwrap();
        second.acknowledge.send(Ok(2)).unwrap();
        std::fs::write(root.join("assets/test.txt"), "after").unwrap();
        let asset_update = receiver.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(matches!(asset_update.replacement, Replacement::Assets(_)));
        surface
            .replace(asset_update.replacement, asset_update.reset)
            .unwrap();
        asset_update.acknowledge.send(Ok(3)).unwrap();
        assert_eq!(surface.asset("assets/test.txt"), Some(b"after".as_slice()));
        let (acknowledge, restarted) = mpsc::sync_channel(1);
        restart_sender.send(Restart(acknowledge)).unwrap();
        let restart = receiver.recv_timeout(Duration::from_secs(10)).unwrap();
        assert!(restart.reset);
        assert!(
            restarted.try_recv().is_err(),
            "restart must await the owning UI thread"
        );
        surface.replace(restart.replacement, restart.reset).unwrap();
        restart.acknowledge.send(Ok(4)).unwrap();
        assert_eq!(
            restarted.recv_timeout(Duration::from_secs(2)).unwrap(),
            Ok(4)
        );
        assert!(!root.join("build/session.studio").exists());
        drop(guard);
        worker.join().unwrap();
    }
}
