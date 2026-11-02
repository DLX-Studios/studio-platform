//! JSON-RPC status and preview-control endpoint for a development session.
//!
//! The listener thread reads status snapshots and forwards bounded commands to
//! the GPUI thread; it never enters GPUI or mutates the live surface.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, SyncSender},
    Arc, RwLock,
};
use std::thread::JoinHandle;
use std::time::Duration;

use studio_protocol::dev_rpc::{
    self as protocol, DevDiagnostic, DevPhase, DevPresentationUpdate, DevRpcListener, DevRpcStream,
    DevSelection, DevSourceLocation, DevStatus, DevViewport, MAX_MESSAGE_BYTES,
};

/// Shared status snapshot updated by the compiler worker and read by the RPC thread.
#[derive(Clone, Debug, Default)]
pub(super) struct StatusSnapshot {
    status: Arc<RwLock<DevStatus>>,
    source_locations: Arc<RwLock<BTreeMap<String, DevSourceLocation>>>,
}

/// A presentation or selection command that must execute on the GPUI thread.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DevCommand {
    /// Intercept preview clicks and report selected source nodes.
    SetAuthoringSelection(bool),
    /// Update the active preview presentation.
    SetPresentation(DevPresentationUpdate),
}

/// One queued JSON-RPC command with a bounded response channel.
pub(super) struct PendingCommand {
    /// Requested preview operation.
    pub command: DevCommand,
    /// Response to the waiting local JSON-RPC caller.
    pub response: SyncSender<Result<DevStatus, RpcCommandError>>,
}

/// Stable failure returned to a preview RPC caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RpcCommandError {
    /// JSON-RPC error code.
    pub code: i64,
    /// User-readable reason.
    pub message: String,
}

impl StatusSnapshot {
    /// Return a consistent copy for a status request.
    pub(super) fn read(&self) -> DevStatus {
        self.status
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Set the current host phase.
    pub(super) fn set_phase(&self, phase: DevPhase) {
        self.status
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .phase = phase;
    }

    /// Record a failure while retaining the last successfully applied revision.
    pub(super) fn set_failure(&self, code: impl Into<String>, message: impl Into<String>) {
        let mut status = self
            .status
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        status.phase = DevPhase::Failed;
        status.last_diagnostic = Some(diagnostic(code, message));
    }

    /// Record a diagnostic without changing the host's current phase.
    pub(super) fn set_diagnostic(&self, code: impl Into<String>, message: impl Into<String>) {
        self.status
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .last_diagnostic = Some(diagnostic(code, message));
    }

    /// Record a successful live update.
    pub(super) fn set_applied(&self, revision: u64, code_hash: impl Into<String>) {
        let mut status = self
            .status
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        status.phase = DevPhase::Ready;
        status.applied_revision = revision;
        status.code_hash = Some(code_hash.into());
    }

    /// Replace the node-to-source map and the current one-screen identity.
    pub(super) fn set_surface_identity(
        &self,
        component_id: Option<String>,
        screen_id: Option<String>,
        source_locations: BTreeMap<String, DevSourceLocation>,
    ) {
        *self
            .source_locations
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = source_locations;
        let mut status = self
            .status
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        status.selection = None;
        status.presentation.component_id = component_id;
        status.presentation.screen_id = screen_id.clone();
        status.presentation.available_screen_ids = screen_id.into_iter().collect();
    }

    /// Record a native authoring selection and its source link, if available.
    pub(super) fn set_selection(&self, node_id: String) {
        let source = self
            .source_locations
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&node_id)
            .cloned();
        let mut status = self
            .status
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !status.authoring_selection {
            return;
        }
        status.selection = Some(DevSelection {
            node_id,
            component_id: status.presentation.component_id.clone(),
            screen_id: status.presentation.screen_id.clone(),
            source,
        });
    }

    /// Record the viewport currently reported by the GPUI window.
    pub(super) fn set_observed_viewport(&self, viewport: DevViewport) {
        let mut status = self
            .status
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        status.presentation.viewport = viewport;
        status.presentation.orientation = if viewport.width > viewport.height {
            studio_protocol::dev_rpc::DevOrientation::Landscape
        } else {
            studio_protocol::dev_rpc::DevOrientation::Portrait
        };
    }

    /// Enable or disable authoring selection, clearing stale selection on disable.
    pub(super) fn set_authoring_selection(&self, enabled: bool) {
        let mut status = self
            .status
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        status.authoring_selection = enabled;
        if !enabled {
            status.selection = None;
        }
    }

    /// Apply a presentation update after validating it against this session.
    pub(super) fn set_presentation(
        &self,
        update: &DevPresentationUpdate,
    ) -> Result<DevStatus, RpcCommandError> {
        let mut status = self
            .status
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        status.presentation =
            update
                .apply_to(&status.presentation)
                .map_err(|error| RpcCommandError {
                    code: -32602,
                    message: error.to_string(),
                })?;
        Ok(status.clone())
    }
}

fn diagnostic(code: impl Into<String>, message: impl Into<String>) -> DevDiagnostic {
    DevDiagnostic {
        code: code.into(),
        message: bounded_message(message.into()),
    }
}

fn bounded_message(message: String) -> String {
    const MAX_DIAGNOSTIC_BYTES: usize = 4 * 1024;
    if message.len() <= MAX_DIAGNOSTIC_BYTES {
        return message;
    }
    let mut bounded = String::with_capacity(MAX_DIAGNOSTIC_BYTES);
    for character in message.chars() {
        if bounded.len() + character.len_utf8() > MAX_DIAGNOSTIC_BYTES {
            break;
        }
        bounded.push(character);
    }
    bounded.push('…');
    bounded
}

/// Running status listener. Dropping it unlinks its endpoint and joins its thread.
pub(super) struct Server {
    endpoint: PathBuf,
    stopped: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl Server {
    /// Start the status endpoint beside a project build directory.
    pub(super) fn start(
        build_dir: &Path,
        snapshot: StatusSnapshot,
        commands: SyncSender<PendingCommand>,
    ) -> io::Result<Self> {
        let endpoint = protocol::socket_path(build_dir);
        if protocol::connect_endpoint(&endpoint).is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "a development RPC endpoint already owns this project",
            ));
        }
        let listener = protocol::bind_endpoint(&endpoint)?;
        if let Err(error) = listener.set_nonblocking(true) {
            let _ = std::fs::remove_file(&endpoint);
            return Err(error);
        }

        let stopped = Arc::new(AtomicBool::new(false));
        let worker_stopped = Arc::clone(&stopped);
        let worker = match std::thread::Builder::new()
            .name("studio-dev-rpc".to_owned())
            .spawn(move || serve(listener, snapshot, commands, worker_stopped))
        {
            Ok(worker) => worker,
            Err(error) => {
                let _ = std::fs::remove_file(&endpoint);
                return Err(error);
            }
        };
        Ok(Self {
            endpoint,
            stopped,
            worker: Some(worker),
        })
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        let _ = std::fs::remove_file(&self.endpoint);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn serve(
    listener: DevRpcListener,
    snapshot: StatusSnapshot,
    commands: SyncSender<PendingCommand>,
    stopped: Arc<AtomicBool>,
) {
    while !stopped.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                serve_one(&mut stream, &snapshot, &commands);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => break,
        }
    }
}

fn serve_one(
    stream: &mut DevRpcStream,
    snapshot: &StatusSnapshot,
    commands: &SyncSender<PendingCommand>,
) {
    let mut line = String::new();
    let read = {
        let reader = BufReader::new(&mut *stream);
        reader
            .take((MAX_MESSAGE_BYTES + 1) as u64)
            .read_line(&mut line)
    };
    let response = match read {
        Ok(0) => return,
        Ok(read) if read > MAX_MESSAGE_BYTES => Some(error_response(
            ValueId::Null,
            -32600,
            "request exceeds the message limit",
        )),
        Ok(_) if !line.ends_with('\n') => Some(error_response(
            ValueId::Null,
            -32600,
            "request is missing its newline terminator",
        )),
        Ok(_) => response_for(&line, snapshot, commands),
        Err(_) => return,
    };
    if let Some(response) = response {
        let mut encoded = response.to_string();
        if encoded.len() < MAX_MESSAGE_BYTES {
            encoded.push('\n');
            let _ = stream.write_all(encoded.as_bytes());
            let _ = stream.flush();
        }
    }
}

type ValueId = serde_json::Value;

fn response_for(
    line: &str,
    snapshot: &StatusSnapshot,
    commands: &SyncSender<PendingCommand>,
) -> Option<serde_json::Value> {
    use serde_json::Value;

    let request: Value = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(_) => return Some(error_response(Value::Null, -32700, "parse error")),
    };
    let Some(object) = request.as_object() else {
        return Some(error_response(
            Value::Null,
            -32600,
            "request must be a JSON object",
        ));
    };
    let id = match object.get("id") {
        Some(id) if id.is_null() || id.is_string() || id.is_number() => id.clone(),
        Some(_) => return Some(error_response(Value::Null, -32600, "invalid request id")),
        None => return None,
    };
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Some(error_response(id, -32600, "jsonrpc must be `2.0`"));
    }
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return Some(error_response(id, -32600, "method must be a string"));
    };
    match method {
        protocol::STATUS_METHOD => {
            if let Some(params) = object.get("params") {
                if !matches!(params, Value::Object(values) if values.is_empty()) {
                    return Some(error_response(id, -32602, "status takes no parameters"));
                }
            }
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": snapshot.read(),
            }))
        }
        protocol::SELECTION_MODE_METHOD => {
            #[derive(serde::Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Params {
                enabled: bool,
            }
            let params = match object.get("params") {
                Some(params) => match serde_json::from_value::<Params>(params.clone()) {
                    Ok(params) => params,
                    Err(error) => return Some(error_response(id, -32602, &error.to_string())),
                },
                None => {
                    return Some(error_response(
                        id,
                        -32602,
                        "selection.setMode requires params",
                    ));
                }
            };
            enqueue_command(
                id,
                DevCommand::SetAuthoringSelection(params.enabled),
                commands,
            )
        }
        protocol::PRESENTATION_METHOD => {
            let update: DevPresentationUpdate = match object.get("params") {
                Some(params) => match serde_json::from_value(params.clone()) {
                    Ok(update) => update,
                    Err(error) => {
                        return Some(error_response(id, -32602, &error.to_string()));
                    }
                },
                None => {
                    return Some(error_response(
                        id,
                        -32602,
                        "presentation.set requires params",
                    ));
                }
            };
            enqueue_command(id, DevCommand::SetPresentation(update), commands)
        }
        _ => Some(error_response(id, -32601, "method not found")),
    }
}

fn enqueue_command(
    id: ValueId,
    command: DevCommand,
    commands: &SyncSender<PendingCommand>,
) -> Option<serde_json::Value> {
    const COMMAND_TIMEOUT: Duration = Duration::from_secs(2);
    let (response, receiver) = mpsc::sync_channel(1);
    let pending = PendingCommand { command, response };
    if let Err(error) = commands.try_send(pending) {
        let message = match error {
            mpsc::TrySendError::Full(_) => "development command queue is full",
            mpsc::TrySendError::Disconnected(_) => "development UI command channel is closed",
        };
        return Some(error_response(id, -32001, message));
    }
    match receiver.recv_timeout(COMMAND_TIMEOUT) {
        Ok(Ok(status)) => Some(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": status,
        })),
        Ok(Err(error)) => Some(error_response(id, error.code, &error.message)),
        Err(mpsc::RecvTimeoutError::Timeout) => Some(error_response(
            id,
            -32002,
            "development UI command timed out",
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => Some(error_response(
            id,
            -32001,
            "development UI command channel is closed",
        )),
    }
}

fn error_response(id: ValueId, code: i64, message: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_protocol::dev_rpc::{
        request_status, set_authoring_selection, set_presentation, DevOrientation, DevPresentation,
        DevPresentationUpdate, DevRpcError, DevViewport,
    };

    #[test]
    fn status_endpoint_serves_snapshot_and_unlinks_on_shutdown() {
        let directory = tempfile::tempdir().unwrap();
        let snapshot = StatusSnapshot::default();
        snapshot.set_applied(3, "aabbccdd");
        let endpoint = protocol::socket_path(directory.path());
        let (commands, _receiver) = mpsc::sync_channel(16);
        let server = Server::start(directory.path(), snapshot, commands).unwrap();

        let status = request_status(directory.path(), Duration::from_secs(2)).unwrap();
        assert_eq!(status.phase, DevPhase::Ready);
        assert_eq!(status.applied_revision, 3);
        assert_eq!(status.code_hash.as_deref(), Some("aabbccdd"));

        drop(server);
        assert!(!endpoint.exists());
        let error = request_status(directory.path(), Duration::from_millis(50)).unwrap_err();
        assert!(error.is_not_running());
        assert!(!matches!(error, DevRpcError::Timeout));
    }

    #[test]
    fn unknown_methods_receive_a_json_rpc_method_error() {
        let (commands, _receiver) = mpsc::sync_channel(1);
        let response = response_for(
            r#"{"jsonrpc":"2.0","id":9,"method":"studio.no.such.method"}"#,
            &StatusSnapshot::default(),
            &commands,
        )
        .unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 9);
        assert_eq!(response["error"]["code"], -32601);
    }

    #[test]
    fn full_ui_command_queue_returns_a_bounded_json_rpc_error() {
        let (commands, _receiver) = mpsc::sync_channel(1);
        let (response, _response_receiver) = mpsc::sync_channel(1);
        commands
            .try_send(PendingCommand {
                command: DevCommand::SetAuthoringSelection(false),
                response,
            })
            .unwrap();

        let response = response_for(
            r#"{"jsonrpc":"2.0","id":10,"method":"studio.dev.selection.setMode","params":{"enabled":true}}"#,
            &StatusSnapshot::default(),
            &commands,
        )
        .unwrap();

        assert_eq!(response["error"]["code"], -32001);
        assert_eq!(
            response["error"]["message"],
            "development command queue is full"
        );
    }

    #[test]
    fn authoring_selection_reports_identity_and_an_optional_source_link() {
        let snapshot = StatusSnapshot::default();
        let source = DevSourceLocation {
            path: "src/app.studio".to_owned(),
            line: 8,
            column: 3,
            offset: 101,
        };
        snapshot.set_surface_identity(
            Some("home".to_owned()),
            Some("home".to_owned()),
            BTreeMap::from([("cta".to_owned(), source.clone())]),
        );
        snapshot.set_selection("cta".to_owned());
        assert_eq!(snapshot.read().selection, None);

        snapshot.set_authoring_selection(true);
        snapshot.set_selection("cta".to_owned());
        assert_eq!(
            snapshot.read().selection,
            Some(DevSelection {
                node_id: "cta".to_owned(),
                component_id: Some("home".to_owned()),
                screen_id: Some("home".to_owned()),
                source: Some(source),
            })
        );

        snapshot.set_authoring_selection(false);
        assert_eq!(snapshot.read().selection, None);
    }

    #[test]
    fn selection_and_presentation_requests_wait_for_ui_thread_commands() {
        let directory = tempfile::tempdir().unwrap();
        let snapshot = StatusSnapshot::default();
        let (commands, receiver) = mpsc::sync_channel(16);
        let server = Server::start(directory.path(), snapshot.clone(), commands).unwrap();

        let client_dir = directory.path().to_path_buf();
        let selection_client = std::thread::spawn(move || {
            set_authoring_selection(&client_dir, true, Duration::from_secs(2))
        });
        let selection = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(selection.command, DevCommand::SetAuthoringSelection(true));
        snapshot.set_authoring_selection(true);
        selection.response.send(Ok(snapshot.read())).unwrap();
        assert!(
            selection_client
                .join()
                .unwrap()
                .unwrap()
                .authoring_selection
        );

        let client_dir = directory.path().to_path_buf();
        let update = DevPresentationUpdate {
            viewport: Some(DevViewport {
                width: 390,
                height: 844,
            }),
            orientation: Some(DevOrientation::Portrait),
            ..DevPresentationUpdate::default()
        };
        let presentation_client = std::thread::spawn(move || {
            set_presentation(&client_dir, &update, Duration::from_secs(2))
        });
        let presentation = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(
            presentation.command,
            DevCommand::SetPresentation(DevPresentationUpdate {
                viewport: Some(DevViewport {
                    width: 390,
                    height: 844,
                }),
                orientation: Some(DevOrientation::Portrait),
                ..DevPresentationUpdate::default()
            })
        );
        let DevCommand::SetPresentation(update) = presentation.command else {
            unreachable!()
        };
        let status = snapshot.set_presentation(&update).unwrap();
        presentation.response.send(Ok(status)).unwrap();
        let status = presentation_client.join().unwrap().unwrap();
        assert_eq!(status.presentation.viewport.width, 390);
        assert_eq!(status.presentation.viewport.height, 844);
        assert_eq!(status.presentation.orientation, DevOrientation::Portrait);

        drop(server);
    }

    #[test]
    fn unavailable_screen_request_is_rejected_by_the_ui_command_handler() {
        let directory = tempfile::tempdir().unwrap();
        let snapshot = StatusSnapshot::default();
        snapshot.set_surface_identity(
            Some("home".to_owned()),
            Some("home".to_owned()),
            BTreeMap::new(),
        );
        let (commands, receiver) = mpsc::sync_channel(16);
        let server = Server::start(directory.path(), snapshot.clone(), commands).unwrap();
        let client_dir = directory.path().to_path_buf();
        let client = std::thread::spawn(move || {
            set_presentation(
                &client_dir,
                &DevPresentationUpdate {
                    screen_id: Some("settings".to_owned()),
                    ..DevPresentationUpdate::default()
                },
                Duration::from_secs(2),
            )
        });
        let pending = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        let DevCommand::SetPresentation(update) = pending.command else {
            unreachable!()
        };
        let previous = snapshot.read().presentation;
        let error = update.apply_to(&previous).unwrap_err();
        pending
            .response
            .send(Err(RpcCommandError {
                code: -32602,
                message: error.to_string(),
            }))
            .unwrap();
        assert!(matches!(
            client.join().unwrap(),
            Err(DevRpcError::Remote { code: -32602, .. })
        ));
        assert_eq!(
            snapshot.read().presentation,
            DevPresentation {
                component_id: Some("home".to_owned()),
                screen_id: Some("home".to_owned()),
                available_screen_ids: vec!["home".to_owned()],
                ..DevPresentation::default()
            }
        );
        drop(server);
    }
}
