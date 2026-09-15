//! US1/US2 scripted reload sessions: 10 scripted reloads over a real
//! loopback channel against a stub host end on the last valid surface with
//! every outcome acknowledged.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use studio_host::reload::{
    DecodedOutcome, ReloadOutcome, ReloadRequest, SwapPlan, SwapSession, bind_endpoint,
    connect_endpoint, decode_outcome, encode_request, read_line, send_line, socket_path,
};

/// Stub host state: live bundle plus revision, driven by the swap planner.
struct StubHost {
    session: SwapSession,
    live: Option<String>,
}

impl StubHost {
    fn new() -> Self {
        Self {
            session: SwapSession::new(),
            live: None,
        }
    }

    /// Simulate prepare: paths containing markers fail at their stage.
    fn prepare(bundle: &str) -> Result<String, (studio_host::reload::ReloadStage, String, String)> {
        use studio_host::reload::ReloadStage;
        if bundle.contains("invalid") {
            return Err((
                ReloadStage::Validate,
                "MANIFEST_INVALID_JSON".to_owned(),
                "stub validation failed".to_owned(),
            ));
        }
        if bundle.contains("crash") {
            return Err((
                ReloadStage::Instantiate,
                "RELOAD_INIT_CRASH".to_owned(),
                "stub init crashed".to_owned(),
            ));
        }
        Ok(bundle.to_owned())
    }

    fn handle(&mut self, request: &ReloadRequest) -> ReloadOutcome {
        let prepared = Self::prepare(&request.bundle);
        let (plan, outcome) = self.session.plan(request, prepared);
        if matches!(plan, SwapPlan::Commit { .. }) {
            if let SwapPlan::Commit { bundle } = plan {
                self.live = Some(bundle);
            }
            self.session.committed();
        }
        outcome
    }
}

/// Read one newline-terminated line, preserving over-read bytes across calls.
fn read_line_buffered(
    reader: &mut std::os::unix::net::UnixStream,
    buffered: &mut Vec<u8>,
) -> std::io::Result<String> {
    use std::io::Read;
    reader.set_read_timeout(Some(Duration::from_secs(10)))?;
    loop {
        if let Some(position) = buffered.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = buffered.drain(..=position).collect();
            return Ok(String::from_utf8_lossy(&line).trim_end().to_owned());
        }
        let mut chunk = [0u8; 1024];
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            if buffered.is_empty() {
                return Ok(String::new());
            }
            return Ok(String::from_utf8_lossy(buffered).trim_end().to_owned());
        }
        buffered.extend_from_slice(&chunk[..read]);
    }
}

fn outcome_line(outcome: &ReloadOutcome) -> String {
    match outcome {
        ReloadOutcome::Reloaded {
            request_id,
            revision,
        } => serde_json::json!({
            "protocol": "studio-reload/1",
            "type": "reloaded",
            "request_id": request_id,
            "revision": revision,
        })
        .to_string(),
        ReloadOutcome::Rejected {
            request_id,
            stage,
            code,
            message,
        } => serde_json::json!({
            "protocol": "studio-reload/1",
            "type": "rejected",
            "request_id": request_id,
            "stage": stage.as_str(),
            "code": code,
            "message": message,
        })
        .to_string(),
    }
}

#[allow(clippy::needless_pass_by_value)]
fn run_stub(
    path: std::path::PathBuf,
    live: Arc<Mutex<Option<String>>>,
    revisions: Arc<Mutex<Vec<u64>>>,
) {
    let listener = bind_endpoint(&path).unwrap();
    let (stream, _) = listener.accept().unwrap();
    let mut host = StubHost::new();
    // One persistent buffered reader: recreating it per line would drop
    // already-buffered requests.
    let mut buffered: Vec<u8> = Vec::new();
    let mut reader = stream.try_clone().unwrap();
    loop {
        let raw = match read_line_buffered(&mut reader, &mut buffered) {
            Ok(raw) if raw.is_empty() => break,
            Ok(raw) => raw,
            Err(_) => break,
        };
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        if value.get("type").and_then(|kind| kind.as_str()) == Some("stop") {
            break;
        }
        let mut declared_routes = Vec::new();
        if let Some(routes) = value.get("routes").and_then(|routes| routes.as_array()) {
            for route in routes {
                if let Some(route) = route.as_str() {
                    declared_routes.push(route.to_owned());
                }
            }
        }
        let request = ReloadRequest {
            request_id: value["request_id"].as_str().unwrap().to_owned(),
            bundle: value["bundle"].as_str().unwrap().to_owned(),
            base_revision: value["base_revision"].as_u64().unwrap(),
            declared_routes,
        };
        let outcome = host.handle(&request);
        if let ReloadOutcome::Reloaded { revision, .. } = &outcome {
            revisions.lock().unwrap().push(*revision);
        }
        let mut writer = stream.try_clone().unwrap();
        send_line(&mut writer, &outcome_line(&outcome)).unwrap();
        (*live.lock().unwrap()).clone_from(&host.live);
    }
}

#[test]
fn ten_reload_script_ends_on_the_last_valid_surface() {
    let root = tempfile::tempdir().unwrap();
    let path = socket_path(root.path());
    let live: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let revisions: Arc<Mutex<Vec<u64>>> = Arc::new(Mutex::new(Vec::new()));
    let server = {
        let path = path.clone();
        let live = Arc::clone(&live);
        let revisions = Arc::clone(&revisions);
        std::thread::spawn(move || run_stub(path, live, revisions))
    };

    let bundles = [
        "good-1.studio",
        "good-2.studio",
        "invalid-a.studio",
        "good-3.studio",
        "crash-a.studio",
        "good-4.studio",
        "invalid-b.studio",
        "good-5.studio",
        "good-6.studio",
        "good-7.studio",
    ];
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
    let mut outcomes = BTreeMap::new();
    for (index, bundle) in bundles.iter().enumerate() {
        let id = format!("r-{}", index + 1);
        let request = ReloadRequest {
            request_id: id.clone(),
            bundle: (*bundle).to_owned(),
            base_revision: 1,
            declared_routes: Vec::new(),
        };
        send_line(&mut stream, &encode_request(&request)).unwrap();
        let raw = read_line(&stream, Duration::from_secs(10)).unwrap();
        outcomes.insert(id, decode_outcome(&raw).unwrap());
    }
    send_line(&mut stream, r#"{"type":"stop"}"#).unwrap();
    server.join().unwrap();

    // 7 valid reloads accepted with advancing revisions; 3 rejected.
    // Collect in request order (BTreeMap would sort "r-10" before "r-2").
    let accepted: Vec<_> = (1..=10)
        .map(|index| format!("r-{index}"))
        .filter_map(|id| match outcomes.get(&id)? {
            DecodedOutcome::Accepted { revision, .. } => Some(*revision),
            DecodedOutcome::Rejected { .. } => None,
        })
        .collect();
    assert_eq!(accepted, vec![2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(
        outcomes
            .values()
            .filter(|o| matches!(o, DecodedOutcome::Rejected { .. }))
            .count(),
        3
    );
    assert_eq!(live.lock().unwrap().as_deref(), Some("good-7.studio"));
    assert_eq!(*revisions.lock().unwrap(), vec![2, 3, 4, 5, 6, 7, 8]);
}
