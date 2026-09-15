//! US1/US2 storm and recovery contract: concurrent requests collapse to one
//! trailing reload, mid-swap failures roll back with disposal cancelled, and
//! first-bundle failure yields a safe empty state.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use studio_host::reload::{
    ReloadOutcome, ReloadRequest, ReloadStage, SwapPlan, SwapSession, bind_endpoint,
    connect_endpoint, decode_outcome, encode_request, send_line, socket_path,
};

#[test]
fn storm_collapses_to_one_trailing_reload() {
    let mut session = SwapSession::new();
    // r-1 executes; r-2..r-5 arrive during the swap (latest wins).
    session.offer(request("r-1", "/tmp/a.studio"), false);
    for (id, bundle) in [
        ("r-2", "/tmp/b.studio"),
        ("r-3", "/tmp/c.studio"),
        ("r-4", "/tmp/d.studio"),
        ("r-5", "/tmp/e.studio"),
    ] {
        session.offer(request(id, bundle), true);
    }
    assert_eq!(session.take_next().unwrap().request_id, "r-1");
    assert!(
        session.take_next().is_none(),
        "nothing else runs concurrently"
    );
    session.promote_trailing();
    let trailing = session.take_next().unwrap();
    assert_eq!(trailing.bundle, "/tmp/e.studio", "latest bundle wins");
    assert!(session.take_next().is_none(), "exactly one trailing reload");
}

#[test]
fn mid_swap_failure_rolls_back_with_disposal_cancelled() {
    let session = SwapSession::new();
    let request = request("r-1", "/tmp/crash.studio");
    let (plan, outcome) = session.plan(
        &request,
        Err((
            ReloadStage::Commit,
            "RELOAD_COMMIT_FAILED".to_owned(),
            "commit failed".to_owned(),
        )),
    );
    assert!(matches!(plan, SwapPlan::Reject { .. }));
    assert!(matches!(
        outcome,
        ReloadOutcome::Rejected {
            stage: ReloadStage::Commit,
            ..
        }
    ));
    // No revision advance without a commit: the live surface stands.
    assert_eq!(session.revision(), 1);
}

#[test]
fn first_bundle_failure_yields_safe_empty_state() {
    // No live surface exists; the failure outcome is the whole state signal
    // and the session revision never advances.
    let mut session = SwapSession::new();
    assert!(session.take_next().is_none());
    let request = request("r-1", "/tmp/invalid.studio");
    let (plan, outcome) = session.plan(
        &request,
        Err((
            ReloadStage::Validate,
            "MANIFEST_INVALID_JSON".to_owned(),
            "bad manifest".to_owned(),
        )),
    );
    assert!(matches!(plan, SwapPlan::Reject { .. }));
    assert!(matches!(outcome, ReloadOutcome::Rejected { .. }));
    assert_eq!(session.revision(), 1);
}

#[test]
fn storm_over_a_real_channel_keeps_revision_accounting() {
    let root = tempfile::tempdir().unwrap();
    let path = socket_path(root.path());
    let revisions: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));
    let server = {
        let path = path.clone();
        let revisions = Arc::clone(&revisions);
        std::thread::spawn(move || {
            let listener = bind_endpoint(&path).unwrap();
            let (mut stream, _) = listener.accept().unwrap();
            let mut session = SwapSession::new();
            let mut buffered = Vec::new();
            loop {
                let line = read_assembled(&mut stream, &mut buffered);
                if line.is_empty() || line == "{\"type\":\"stop\"}" {
                    break;
                }
                let value: serde_json::Value = serde_json::from_str(&line).unwrap();
                let request = ReloadRequest {
                    request_id: value["request_id"].as_str().unwrap().to_owned(),
                    bundle: value["bundle"].as_str().unwrap().to_owned(),
                    base_revision: value["base_revision"].as_u64().unwrap(),
                    declared_routes: Vec::new(),
                };
                let outcome = ReloadOutcome::Reloaded {
                    request_id: request.request_id.clone(),
                    revision: session.committed(),
                };
                revisions.lock().unwrap().push(session.revision());
                let line = serde_json::json!({
                    "protocol": "studio-reload/1",
                    "type": "reloaded",
                    "request_id": request.request_id,
                    "revision": session.revision(),
                })
                .to_string();
                let _ = outcome;
                let mut writer = stream.try_clone().unwrap();
                send_line(&mut writer, &line).unwrap();
            }
        })
    };

    let mut stream = {
        let mut connected = None;
        for _ in 0..100 {
            match connect_endpoint(&path) {
                Ok(stream) => {
                    connected = Some(stream);
                    break;
                }
                Err(_) => std::thread::sleep(std::time::Duration::from_millis(20)),
            }
        }
        connected.expect("stub host binds")
    };
    let mut buffered = Vec::new();
    for index in 1..=5u32 {
        let request = request(&format!("r-{index}"), &format!("/tmp/{index}.studio"));
        send_line(&mut stream, &encode_request(&request)).unwrap();
        let line = read_assembled(&mut stream, &mut buffered);
        let outcome = decode_outcome(&line).unwrap();
        assert!(matches!(
            outcome,
            studio_host::reload::DecodedOutcome::Accepted { .. }
        ));
    }
    send_line(&mut stream, r#"{"type":"stop"}"#).unwrap();
    server.join().unwrap();
    assert_eq!(*revisions.lock().unwrap(), vec![2, 3, 4, 5, 6]);
}

fn request(id: &str, bundle: &str) -> ReloadRequest {
    ReloadRequest {
        request_id: id.to_owned(),
        bundle: bundle.to_owned(),
        base_revision: 1,
        declared_routes: Vec::new(),
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
            Ok(0) | Err(_) => return String::new(),
            Ok(read) => buffered.extend_from_slice(&chunk[..read]),
        }
    }
}
