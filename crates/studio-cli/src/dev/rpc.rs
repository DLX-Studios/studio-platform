//! Read-only JSON-RPC status endpoint for a running development session.
//!
//! The listener thread reads an immutable status snapshot and never enters GPUI
//! or mutates the live surface.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};
use std::thread::JoinHandle;
use std::time::Duration;

use studio_protocol::dev_rpc::{
    self as protocol, DevDiagnostic, DevPhase, DevRpcListener, DevRpcStream, DevStatus,
    MAX_MESSAGE_BYTES,
};

/// Shared status snapshot updated by the compiler worker and read by the RPC thread.
#[derive(Clone, Debug, Default)]
pub(super) struct StatusSnapshot(Arc<RwLock<DevStatus>>);

impl StatusSnapshot {
    /// Return a consistent copy for a status request.
    pub(super) fn read(&self) -> DevStatus {
        self.0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Set the current host phase.
    pub(super) fn set_phase(&self, phase: DevPhase) {
        self.0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .phase = phase;
    }

    /// Record a failure while retaining the last successfully applied revision.
    pub(super) fn set_failure(&self, code: impl Into<String>, message: impl Into<String>) {
        let mut status = self
            .0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        status.phase = DevPhase::Failed;
        status.last_diagnostic = Some(diagnostic(code, message));
    }

    /// Record a diagnostic without changing the host's current phase.
    pub(super) fn set_diagnostic(&self, code: impl Into<String>, message: impl Into<String>) {
        self.0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .last_diagnostic = Some(diagnostic(code, message));
    }

    /// Record a successful live update.
    pub(super) fn set_applied(&self, revision: u64, code_hash: impl Into<String>) {
        let mut status = self
            .0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        status.phase = DevPhase::Ready;
        status.applied_revision = revision;
        status.code_hash = Some(code_hash.into());
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
    pub(super) fn start(build_dir: &Path, snapshot: StatusSnapshot) -> io::Result<Self> {
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
            .spawn(move || serve(listener, snapshot, worker_stopped))
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

fn serve(listener: DevRpcListener, snapshot: StatusSnapshot, stopped: Arc<AtomicBool>) {
    while !stopped.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                serve_one(&mut stream, &snapshot);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => break,
        }
    }
}

fn serve_one(stream: &mut DevRpcStream, snapshot: &StatusSnapshot) {
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
        Ok(_) => response_for(&line, snapshot),
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

fn response_for(line: &str, snapshot: &StatusSnapshot) -> Option<serde_json::Value> {
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
    if method != protocol::STATUS_METHOD {
        return Some(error_response(id, -32601, "method not found"));
    }
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
    use studio_protocol::dev_rpc::{request_status, DevRpcError};

    #[test]
    fn status_endpoint_serves_snapshot_and_unlinks_on_shutdown() {
        let directory = tempfile::tempdir().unwrap();
        let snapshot = StatusSnapshot::default();
        snapshot.set_applied(3, "aabbccdd");
        let endpoint = protocol::socket_path(directory.path());
        let server = Server::start(directory.path(), snapshot).unwrap();

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
        let response = response_for(
            r#"{"jsonrpc":"2.0","id":9,"method":"studio.no.such.method"}"#,
            &StatusSnapshot::default(),
        )
        .unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 9);
        assert_eq!(response["error"]["code"], -32601);
    }
}
