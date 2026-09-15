//! CLI-to-runtime reload protocol: versioned messages over a local channel.
//!
//! The transport is a Unix domain socket carrying newline-delimited JSON (see
//! `specs/005-reload-protocol/contracts/reload-protocol.md`). This module owns
//! the wire types both sides share; the swap planner lives in
//! `studio-host::reload`.

/// Protocol identifier; mismatches are rejected, never negotiated.
pub const RELOAD_PROTOCOL: &str = "studio-reload/1";

/// Outcome stages for rejections.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReloadStage {
    /// Bundle failed validation (manifest, signature, trust, assets).
    Validate,
    /// Bundle failed instantiation.
    Instantiate,
    /// Swap commit failed after preparation.
    Commit,
}

impl ReloadStage {
    /// Stable stage name for diagnostics and protocol messages.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Validate => "validate",
            Self::Instantiate => "instantiate",
            Self::Commit => "commit",
        }
    }
}

/// A reload request from the toolchain to the runtime.
#[derive(Clone, Debug, PartialEq)]
pub struct ReloadRequest {
    /// Client-generated correlation id.
    pub request_id: String,
    /// Candidate bundle path.
    pub bundle: String,
    /// Client's last known session revision.
    pub base_revision: u64,
    /// Route set declared by the project, for the preserved-route policy.
    /// Empty means unknown: the server falls back to the mount route.
    pub declared_routes: Vec<String>,
}

/// Outcome of one reload request.
#[derive(Clone, Debug, PartialEq)]
pub enum ReloadOutcome {
    /// The swap committed; carries the new session revision.
    Reloaded {
        /// Echoed correlation id.
        request_id: String,
        /// New session revision.
        revision: u64,
    },
    /// The reload was rejected; the live surface is untouched.
    Rejected {
        /// Echoed correlation id.
        request_id: String,
        /// Failing stage.
        stage: ReloadStage,
        /// Stable diagnostic code.
        code: String,
        /// Safe message.
        message: String,
    },
}

/// Outcome of one restart request.
#[derive(Clone, Debug, PartialEq)]
pub enum RestartOutcome {
    /// Dispose-then-instantiate completed with exact-once revision advance.
    Restarted {
        /// Echoed correlation id.
        request_id: String,
        /// New session revision.
        revision: u64,
    },
    /// Restart failed; the previous surface state is reported.
    Rejected {
        /// Echoed correlation id.
        request_id: String,
        /// Stable diagnostic code.
        code: String,
        /// Safe message.
        message: String,
    },
}

/// Serialize one protocol message as a single JSON line.
#[must_use]
pub fn encode_request(request: &ReloadRequest) -> String {
    serde_json::json!({
        "protocol": RELOAD_PROTOCOL,
        "type": "reload",
        "request_id": request.request_id,
        "bundle": request.bundle,
        "base_revision": request.base_revision,
        "routes": request.declared_routes,
    })
    .to_string()
}

/// Serialize a restart request as a single JSON line.
#[must_use]
pub fn encode_restart(request_id: &str) -> String {
    serde_json::json!({
        "protocol": RELOAD_PROTOCOL,
        "type": "restart",
        "request_id": request_id,
    })
    .to_string()
}

/// Decode one outcome line from the runtime.
#[derive(Clone, Debug, PartialEq)]
pub enum DecodedOutcome {
    /// An accepted reload or restart.
    Accepted {
        /// Echoed correlation id.
        request_id: String,
        /// New session revision.
        revision: u64,
    },
    /// A rejection with stage information when present.
    Rejected {
        /// Echoed correlation id.
        request_id: String,
        /// Failing stage, when the runtime named one.
        stage: Option<String>,
        /// Stable diagnostic code.
        code: String,
        /// Safe message.
        message: String,
    },
}

/// Decode one outcome line.
///
/// # Errors
///
/// Returns a message when the line is not a well-formed outcome.
pub fn decode_outcome(line: &str) -> Result<DecodedOutcome, String> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|_| "reload outcome is not JSON".to_owned())?;
    if value.get("protocol").and_then(|protocol| protocol.as_str()) != Some(RELOAD_PROTOCOL) {
        return Err("reload protocol mismatch".to_owned());
    }
    let request_id = value
        .get("request_id")
        .and_then(|id| id.as_str())
        .ok_or_else(|| "reload outcome has no request id".to_owned())?
        .to_owned();
    match value.get("type").and_then(|kind| kind.as_str()) {
        Some("reloaded" | "restarted") => {
            let revision = value
                .get("revision")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| "reload outcome has no revision".to_owned())?;
            Ok(DecodedOutcome::Accepted {
                request_id,
                revision,
            })
        }
        Some("rejected") => Ok(DecodedOutcome::Rejected {
            request_id,
            stage: value
                .get("stage")
                .and_then(|stage| stage.as_str())
                .map(str::to_owned),
            code: value
                .get("code")
                .and_then(|code| code.as_str())
                .unwrap_or("RELOAD_UNKNOWN")
                .to_owned(),
            message: value
                .get("message")
                .and_then(|message| message.as_str())
                .unwrap_or_default()
                .to_owned(),
        }),
        _ => Err("unknown reload outcome type".to_owned()),
    }
}

/// Socket filename inside a project's build directory.
pub const RELOAD_SOCKET_NAME: &str = ".studio-reload.sock";

/// Socket path budget with margin under the 108-byte Unix `sun_path` limit
/// (including the NUL terminator).
const MAX_SOCKET_PATH_BYTES: usize = 100;

/// Resolve the reload socket path for a project build directory.
///
/// Deep checkouts overflow the Unix `sun_path` limit: those fall back to a
/// stable hashed name in the system temp dir, so both dev peers (CLI watcher
/// and runtime host) compute the same endpoint from the same build dir.
#[must_use]
pub fn socket_path(build_dir: &std::path::Path) -> std::path::PathBuf {
    let direct = build_dir.join(RELOAD_SOCKET_NAME);
    if direct.as_os_str().as_encoded_bytes().len() <= MAX_SOCKET_PATH_BYTES {
        return direct;
    }
    // Deterministic FNV-1a over the build dir: stable across the CLI and
    // runtime processes without new dependencies.
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in build_dir.as_os_str().as_encoded_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    std::env::temp_dir().join(format!("studio-reload-{hash:016x}.sock"))
}

/// Bind the endpoint, unlinking our own stale socket file first.
///
/// # Errors
///
/// Returns the bind I/O error when the socket cannot be created.
pub fn bind_endpoint(path: &std::path::Path) -> std::io::Result<std::os::unix::net::UnixListener> {
    let _ = std::fs::remove_file(path);
    std::os::unix::net::UnixListener::bind(path)
}

/// Connect with refused-as-stale semantics: connection-refused means no live
/// session owns the path.
///
/// # Errors
///
/// Returns the connect I/O error (notably connection-refused for stale paths).
pub fn connect_endpoint(path: &std::path::Path) -> std::io::Result<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(path)
}

/// Write one JSON line.
///
/// # Errors
///
/// Returns the write I/O error when the peer is gone.
pub fn send_line(stream: &mut std::os::unix::net::UnixStream, line: &str) -> std::io::Result<()> {
    use std::io::Write;
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

/// Read one JSON line with a deadline.
///
/// # Errors
///
/// Returns the read I/O error on timeout or a broken peer.
pub fn read_line(
    stream: &std::os::unix::net::UnixStream,
    timeout: std::time::Duration,
) -> std::io::Result<String> {
    use std::io::{BufRead, BufReader};
    stream.set_read_timeout(Some(timeout))?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    Ok(line.trim_end().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: &str, bundle: &str) -> ReloadRequest {
        ReloadRequest {
            request_id: id.to_owned(),
            bundle: bundle.to_owned(),
            base_revision: 1,
            declared_routes: Vec::new(),
        }
    }

    #[test]
    fn protocol_round_trips_with_correlation() {
        let line = encode_request(&request("r-1", "/tmp/a.studio"));
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["protocol"], RELOAD_PROTOCOL);
        assert_eq!(value["request_id"], "r-1");

        let accepted = decode_outcome(
            r#"{"protocol":"studio-reload/1","type":"reloaded","request_id":"r-1","revision":4}"#,
        )
        .unwrap();
        assert_eq!(
            accepted,
            DecodedOutcome::Accepted {
                request_id: "r-1".to_owned(),
                revision: 4
            }
        );

        let rejected = decode_outcome(
            r#"{"protocol":"studio-reload/1","type":"rejected","request_id":"r-1","stage":"validate","code":"MANIFEST_INVALID_JSON","message":"bad"}"#,
        )
        .unwrap();
        match rejected {
            DecodedOutcome::Rejected {
                request_id,
                stage,
                code,
                ..
            } => {
                assert_eq!(request_id, "r-1");
                assert_eq!(stage.as_deref(), Some("validate"));
                assert_eq!(code, "MANIFEST_INVALID_JSON");
            }
            DecodedOutcome::Accepted { .. } => panic!("expected a rejection"),
        }
    }

    #[test]
    fn socket_path_stays_in_build_dir_when_short() {
        let path = socket_path(std::path::Path::new("/tmp/proj/build"));
        assert_eq!(
            path,
            std::path::Path::new("/tmp/proj/build/.studio-reload.sock")
        );
    }

    #[test]
    fn socket_path_falls_back_to_temp_dir_when_deep() {
        let deep = std::path::Path::new("/tmp").join("d".repeat(200));
        let first = socket_path(&deep);
        assert!(first.starts_with(std::env::temp_dir()));
        assert!(first.as_os_str().as_encoded_bytes().len() <= MAX_SOCKET_PATH_BYTES);
        // Stable across peers: both dev processes compute the same endpoint.
        assert_eq!(first, socket_path(&deep));
    }

    #[test]
    fn mismatches_and_unknown_types_are_rejected() {
        assert!(
            decode_outcome(
                r#"{"protocol":"other/1","type":"reloaded","request_id":"r","revision":1}"#
            )
            .is_err()
        );
        assert!(
            decode_outcome(r#"{"protocol":"studio-reload/1","type":"explode","request_id":"r"}"#)
                .is_err()
        );
        assert!(decode_outcome("not json").is_err());
    }
}
