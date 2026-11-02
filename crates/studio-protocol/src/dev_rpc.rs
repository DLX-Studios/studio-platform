//! Local JSON-RPC endpoint for querying a running `studio dev` session.
//!
//! This endpoint is separate from `studio-reload/1`, whose wire contract is
//! retained for existing reload clients. Messages use one UTF-8 JSON-RPC 2.0
//! object per line over a local stream socket.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Socket filename for the development status and control endpoint.
pub const DEV_RPC_SOCKET_NAME: &str = ".studio-dev.sock";

/// Maximum request or response line size, including the trailing newline.
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024;

/// Status method exposed by the `studio dev` host.
pub const STATUS_METHOD: &str = "studio.dev.status";

/// Native local listener for a development RPC session.
pub use super::reload::ReloadListener as DevRpcListener;
/// Native local stream for a development RPC session.
pub use super::reload::ReloadStream as DevRpcStream;

/// Current phase of the `studio dev` host.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DevPhase {
    /// Host setup is in progress.
    Starting,
    /// Sources are being compiled and admitted.
    Compiling,
    /// A prepared revision is being committed to the live window.
    Applying,
    /// The live window is running the most recently applied revision.
    Ready,
    /// The latest build or live update failed; the previous revision remains active.
    Failed,
    /// The host is shutting down.
    Stopping,
}

/// Last diagnostic reported by the development host.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevDiagnostic {
    /// Stable diagnostic code.
    pub code: String,
    /// Actionable diagnostic text.
    pub message: String,
}

/// Thread-safe snapshot payload returned by `studio.dev.status`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevStatus {
    /// Current development-host phase.
    pub phase: DevPhase,
    /// Revision most recently applied to the live window.
    pub applied_revision: u64,
    /// Source-code fingerprint of the applied revision, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_hash: Option<String>,
    /// Most recent failure, retained after a later successful update for context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_diagnostic: Option<DevDiagnostic>,
}

impl Default for DevStatus {
    fn default() -> Self {
        Self {
            phase: DevPhase::Starting,
            applied_revision: 0,
            code_hash: None,
            last_diagnostic: None,
        }
    }
}

/// Errors returned by the bounded local status client.
#[derive(Debug, Error)]
pub enum DevRpcError {
    /// The local endpoint could not be reached or used.
    #[error("development status socket error: {0}")]
    Io(#[source] io::Error),
    /// The peer did not reply within the requested deadline.
    #[error("development status request timed out")]
    Timeout,
    /// The peer returned an invalid JSON-RPC message.
    #[error("invalid development status response: {0}")]
    InvalidResponse(String),
    /// The server returned a JSON-RPC error.
    #[error("development status RPC error {code}: {message}")]
    Remote { code: i64, message: String },
}

impl DevRpcError {
    /// Whether no development host currently owns the requested project endpoint.
    #[must_use]
    pub fn is_not_running(&self) -> bool {
        matches!(
            self,
            Self::Io(error)
                if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused)
        )
    }
}

/// Resolve the endpoint for a project build directory.
///
/// Short Unix paths stay beside the project. Deep paths and Windows use a
/// deterministic hashed filename in the system temporary directory.
#[must_use]
pub fn socket_path(build_dir: &Path) -> PathBuf {
    const MAX_SOCKET_PATH_BYTES: usize = 100;
    let direct = build_dir.join(DEV_RPC_SOCKET_NAME);
    if cfg!(unix) && direct.as_os_str().as_encoded_bytes().len() <= MAX_SOCKET_PATH_BYTES {
        return direct;
    }

    // FNV-1a keeps endpoint naming deterministic without another dependency.
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in build_dir.as_os_str().as_encoded_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    std::env::temp_dir().join(format!("studio-dev-{hash:016x}.sock"))
}

/// Bind the local endpoint, removing a stale socket file first.
///
/// # Errors
///
/// Returns an I/O error when the listener cannot be created.
pub fn bind_endpoint(path: &Path) -> io::Result<DevRpcListener> {
    let _ = std::fs::remove_file(path);
    let listener = DevRpcListener::bind(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
            drop(listener);
            let _ = std::fs::remove_file(path);
            return Err(error);
        }
    }
    Ok(listener)
}

/// Connect to the local development endpoint.
///
/// # Errors
///
/// Returns an I/O error when no listener owns the endpoint.
pub fn connect_endpoint(path: &Path) -> io::Result<DevRpcStream> {
    DevRpcStream::connect(path)
}

/// Send a JSON-RPC status request and return the current host snapshot.
///
/// The total response wait uses the supplied timeout, and the response line
/// is limited to [`MAX_MESSAGE_BYTES`].
///
/// # Errors
///
/// Returns a timeout, transport, JSON-RPC, or decoding error.
pub fn request_status(build_dir: &Path, timeout: Duration) -> Result<DevStatus, DevRpcError> {
    if timeout.is_zero() {
        return Err(DevRpcError::Timeout);
    }
    let started = Instant::now();
    let endpoint = socket_path(build_dir);
    let mut stream = connect_endpoint(&endpoint).map_err(map_io_error)?;
    let remaining = timeout
        .checked_sub(started.elapsed())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(DevRpcError::Timeout)?;
    stream
        .set_write_timeout(Some(remaining))
        .map_err(map_io_error)?;
    stream
        .set_read_timeout(Some(remaining))
        .map_err(map_io_error)?;

    let request_id = "studio-status";
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "method": STATUS_METHOD,
        "params": {},
    });
    let line = serde_json::to_string(&request)
        .map_err(|error| DevRpcError::InvalidResponse(error.to_string()))?;
    if line.len() + 1 > MAX_MESSAGE_BYTES {
        return Err(DevRpcError::InvalidResponse(
            "status request exceeds the message limit".to_owned(),
        ));
    }
    stream.write_all(line.as_bytes()).map_err(map_io_error)?;
    stream.write_all(b"\n").map_err(map_io_error)?;
    stream.flush().map_err(map_io_error)?;

    let remaining = timeout
        .checked_sub(started.elapsed())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(DevRpcError::Timeout)?;
    stream
        .set_read_timeout(Some(remaining))
        .map_err(map_io_error)?;
    let mut response_line = String::new();
    let read = BufReader::new(&mut stream)
        .take((MAX_MESSAGE_BYTES + 1) as u64)
        .read_line(&mut response_line)
        .map_err(map_io_error)?;
    if read == 0 || !response_line.ends_with('\n') {
        return Err(DevRpcError::InvalidResponse(
            "response is missing its newline terminator".to_owned(),
        ));
    }
    if read > MAX_MESSAGE_BYTES {
        return Err(DevRpcError::InvalidResponse(
            "response exceeds the message limit".to_owned(),
        ));
    }

    let response: Value = serde_json::from_str(&response_line)
        .map_err(|error| DevRpcError::InvalidResponse(error.to_string()))?;
    if response.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || response.get("id") != Some(&Value::String(request_id.to_owned()))
    {
        return Err(DevRpcError::InvalidResponse(
            "response version or id does not match the request".to_owned(),
        ));
    }
    if let Some(error) = response.get("error") {
        let code = error
            .get("code")
            .and_then(Value::as_i64)
            .ok_or_else(|| DevRpcError::InvalidResponse("error has no numeric code".to_owned()))?;
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .ok_or_else(|| DevRpcError::InvalidResponse("error has no message".to_owned()))?;
        return Err(DevRpcError::Remote {
            code,
            message: message.to_owned(),
        });
    }
    let result = response
        .get("result")
        .cloned()
        .ok_or_else(|| DevRpcError::InvalidResponse("response has no result".to_owned()))?;
    serde_json::from_value(result).map_err(|error| DevRpcError::InvalidResponse(error.to_string()))
}

fn map_io_error(error: io::Error) -> DevRpcError {
    if matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ) {
        DevRpcError::Timeout
    } else {
        DevRpcError::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_endpoint_paths_use_a_stable_short_temp_name() {
        let deep = PathBuf::from("/")
            .join("very-long-project-name".repeat(8))
            .join("build");
        let first = socket_path(&deep);
        let second = socket_path(&deep);
        assert_eq!(first, second);
        assert!(first.starts_with(std::env::temp_dir()));
        assert!(first.as_os_str().as_encoded_bytes().len() <= 100);
        assert_ne!(first, socket_path(&deep.join("another-project")));
    }

    #[cfg(unix)]
    #[test]
    fn short_unix_endpoint_stays_in_the_build_directory() {
        let build = Path::new("/tmp/studio/project/build");
        assert_eq!(socket_path(build), build.join(DEV_RPC_SOCKET_NAME));
    }

    #[test]
    fn status_client_sends_json_rpc_and_decodes_the_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let endpoint = socket_path(directory.path());
        let listener = bind_endpoint(&endpoint).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = String::new();
            BufReader::new(&mut stream).read_line(&mut request).unwrap();
            let request: Value = serde_json::from_str(&request).unwrap();
            assert_eq!(request["jsonrpc"], "2.0");
            assert_eq!(request["method"], STATUS_METHOD);
            let status = DevStatus {
                phase: DevPhase::Ready,
                applied_revision: 7,
                code_hash: Some("012345abcdef".to_owned()),
                last_diagnostic: Some(DevDiagnostic {
                    code: "BUILD_COMPILE_ASC".to_owned(),
                    message: "kept last good revision".to_owned(),
                }),
            };
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": request["id"],
                "result": status,
            });
            writeln!(stream, "{response}").unwrap();
        });

        let status = request_status(directory.path(), Duration::from_secs(2)).unwrap();
        assert_eq!(status.phase, DevPhase::Ready);
        assert_eq!(status.applied_revision, 7);
        assert_eq!(status.code_hash.as_deref(), Some("012345abcdef"));
        assert_eq!(status.last_diagnostic.unwrap().code, "BUILD_COMPILE_ASC");
        server.join().unwrap();
    }

    #[test]
    fn status_client_bounds_wait_for_server_response() {
        let directory = tempfile::tempdir().unwrap();
        let listener = bind_endpoint(&socket_path(directory.path())).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = String::new();
            BufReader::new(&mut stream).read_line(&mut request).unwrap();
            std::thread::sleep(Duration::from_millis(200));
        });

        let result = request_status(directory.path(), Duration::from_millis(50));
        assert!(matches!(result, Err(DevRpcError::Timeout)));
        server.join().unwrap();
    }
}
