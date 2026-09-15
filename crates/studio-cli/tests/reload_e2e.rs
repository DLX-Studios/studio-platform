//! US1 e2e contract: the real CLI client path (build, reload, restart)
//! against a stub runtime host over a loopback channel.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

use studio_protocol::reload::{
    bind_endpoint, decode_outcome, encode_request, send_line, socket_path, DecodedOutcome,
    ReloadRequest,
};

/// Stub host: accepts bundles whose path exists, rejects the rest, and
/// answers restart from the last accepted bundle.
fn run_stub(socket: std::path::PathBuf, log: Arc<Mutex<Vec<String>>>) {
    let listener = bind_endpoint(&socket).unwrap();
    let mut revision = 1u64;
    let mut live: Option<String> = None;
    loop {
        let (mut stream, _) = match listener.accept() {
            Ok(pair) => pair,
            Err(_) => break,
        };
        let mut buffered = Vec::new();
        loop {
            let line = read_assembled(&mut stream, &mut buffered);
            if line.is_empty() || line == "{\"type\":\"stop\"}" {
                break;
            }
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            match value.get("type").and_then(|kind| kind.as_str()) {
                Some("reload") => {
                    let bundle = value["bundle"].as_str().unwrap_or_default().to_owned();
                    let id = value["request_id"].as_str().unwrap_or_default().to_owned();
                    let response = if std::path::Path::new(&bundle).is_file() {
                        revision += 1;
                        live = Some(bundle);
                        serde_json::json!({
                            "protocol": "studio-reload/1",
                            "type": "reloaded",
                            "request_id": id,
                            "revision": revision,
                        })
                        .to_string()
                    } else {
                        serde_json::json!({
                            "protocol": "studio-reload/1",
                            "type": "rejected",
                            "request_id": id,
                            "stage": "validate",
                            "code": "RELOAD_PATH_INVALID",
                            "message": "stub has no such bundle",
                        })
                        .to_string()
                    };
                    log.lock().unwrap().push(format!("reload:{id}"));
                    let mut writer = stream.try_clone().unwrap();
                    send_line(&mut writer, &response).unwrap();
                }
                Some("restart") => {
                    let id = value["request_id"].as_str().unwrap_or_default().to_owned();
                    let response = match &live {
                        Some(_) => {
                            revision += 1;
                            serde_json::json!({
                                "protocol": "studio-reload/1",
                                "type": "restarted",
                                "request_id": id,
                                "revision": revision,
                            })
                            .to_string()
                        }
                        None => serde_json::json!({
                            "protocol": "studio-reload/1",
                            "type": "rejected",
                            "request_id": id,
                            "stage": "validate",
                            "code": "RELOAD_NO_LIVE_BUNDLE",
                            "message": "stub never accepted",
                        })
                        .to_string(),
                    };
                    log.lock().unwrap().push(format!("restart:{id}"));
                    let mut writer = stream.try_clone().unwrap();
                    send_line(&mut writer, &response).unwrap();
                }
                _ => break,
            }
        }
    }
}

fn read_assembled(stream: &mut std::os::unix::net::UnixStream, buffered: &mut Vec<u8>) -> String {
    use std::io::Read;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    loop {
        if let Some(position) = buffered.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = buffered.drain(..=position).collect();
            return String::from_utf8_lossy(&line).trim_end().to_owned();
        }
        let mut chunk = [0u8; 1024];
        match stream.read(&mut chunk) {
            Ok(0) => return String::new(),
            Ok(read) => buffered.extend_from_slice(&chunk[..read]),
            Err(_) => return String::new(),
        }
    }
}

#[test]
fn edit_rebuild_reload_flows_end_to_end() {
    let root = TempDir::new().unwrap();
    let project = common::seed_project(root.path(), "starter");

    // Build through the real binary (covers edit→rebuild).
    let built = common::run_studio(&["build", project.to_str().unwrap()]);
    assert_eq!(built.status.code(), Some(0));
    let bundle = project.join("build/starter.studio");
    assert!(bundle.is_file());

    // Stub runtime on the project's socket (covers reload + restart).
    let socket = socket_path(&project.join("build"));
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let server = {
        let socket = socket.clone();
        let log = Arc::clone(&log);
        std::thread::spawn(move || run_stub(socket, log))
    };
    // Give the listener a moment to bind.
    for _ in 0..50 {
        if socket.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    // Reload the fresh bundle through the real client path.
    assert!(studio_cli::watch::request_reload(&bundle, &socket, &[]).is_ok());

    // A missing bundle is rejected with the surface intact.
    let missing = project.join("build/missing.studio");
    assert!(studio_cli::watch::request_reload(&missing, &socket, &[]).is_err());

    // Restart re-instantiates from the last accepted bundle.
    assert_eq!(
        studio_cli::watch::restart_session(&socket),
        0,
        "restart succeeds after an accepted reload"
    );

    drop(server);
    let log = log.lock().unwrap();
    assert!(log.iter().any(|entry| entry.starts_with("reload:reload-")));
    assert!(log.iter().any(|entry| entry.starts_with("restart:")));
}

#[test]
fn restart_without_a_live_bundle_is_structured() {
    let root = TempDir::new().unwrap();
    let socket = socket_path(root.path());
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let server = {
        let socket = socket.clone();
        let log = Arc::clone(&log);
        std::thread::spawn(move || run_stub(socket, log))
    };
    for _ in 0..50 {
        if socket.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_ne!(
        studio_cli::watch::restart_session(&socket),
        0,
        "restart with no live bundle fails"
    );
    drop(server);
}

#[test]
fn reload_request_encodes_the_contract_shape() {
    let request = ReloadRequest {
        request_id: "r-1".to_owned(),
        bundle: "/tmp/a.studio".to_owned(),
        base_revision: 7,
        declared_routes: vec!["/pos".to_owned()],
    };
    let line = encode_request(&request);
    let value: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(value["protocol"], "studio-reload/1");
    assert_eq!(value["base_revision"], 7);
    assert_eq!(value["routes"], serde_json::json!(["/pos"]));
    let outcome = decode_outcome(
        r#"{"protocol":"studio-reload/1","type":"reloaded","request_id":"r-1","revision":8}"#,
    )
    .unwrap();
    assert_eq!(
        outcome,
        DecodedOutcome::Accepted {
            request_id: "r-1".to_owned(),
            revision: 8
        }
    );
}
