//! Process-level tests for the Designer agent JSON-RPC server.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct ServerProcess(Child);

impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct RpcClient {
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next_id: u64,
}

impl RpcClient {
    fn notify(&mut self, method: &str, params: &serde_json::Value) {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        serde_json::to_writer(&mut self.input, &request).unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
    }

    fn call(&mut self, method: &str, params: &serde_json::Value) -> serde_json::Value {
        let id = self.next_id;
        self.next_id += 1;
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });
        serde_json::to_writer(&mut self.input, &request).unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
        let mut response = String::new();
        self.output.read_line(&mut response).unwrap();
        serde_json::from_str(&response).unwrap()
    }

    fn tool(&mut self, name: &str, arguments: &serde_json::Value) -> serde_json::Value {
        self.call(
            "tools/call",
            &serde_json::json!({ "name": name, "arguments": arguments }),
        )["result"]
            .clone()
    }
}

fn start_server(project_root: &Path) -> (ServerProcess, RpcClient) {
    start_server_with_binaries(project_root, None)
}

fn start_server_with_binaries(
    project_root: &Path,
    binary: Option<&Path>,
) -> (ServerProcess, RpcClient) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_studio-designer"));
    if let Some(binary) = binary {
        command
            .env("STUDIO_CLI_BINARY", binary)
            .env("STUDIO_RUNTIME_BINARY", binary);
    }
    let mut child = ServerProcess(
        command
            .arg("agent-server")
            .current_dir(project_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let input = child.0.stdin.take().unwrap();
    let output = BufReader::new(child.0.stdout.take().unwrap());
    let client = RpcClient {
        input,
        output,
        next_id: 1,
    };
    (child, client)
}

fn initialize(client: &mut RpcClient) {
    let initialized = client.call(
        "initialize",
        &serde_json::json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "test-agent", "version": "1" }
        }),
    );
    assert_eq!(
        initialized["result"]["serverInfo"]["name"],
        "studio-designer"
    );
    client.notify("notifications/initialized", &serde_json::json!({}));
}

fn assert_file_is_listed(client: &mut RpcClient) {
    let listed = client.tool("project.list_files", &serde_json::json!({}));
    assert_eq!(listed["isError"], false);
    assert!(
        listed["structuredContent"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|file| file["path"] == "app.studio")
    );
}

fn read_file_revision(client: &mut RpcClient) -> String {
    let read = client.tool(
        "project.read_file",
        &serde_json::json!({ "path": "app.studio" }),
    );
    assert_eq!(read["structuredContent"]["content"], "<Text>Hello</Text>");
    read["structuredContent"]["revision"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn assert_write_and_stale_conflict(client: &mut RpcClient, project_root: &Path, revision: &str) {
    let written = client.tool(
        "project.write_file",
        &serde_json::json!({
            "path": "app.studio",
            "content": "<Text>Agent edit</Text>",
            "expected_revision": revision
        }),
    );
    assert_eq!(written["isError"], false);
    assert_eq!(
        std::fs::read_to_string(project_root.join("app.studio")).unwrap(),
        "<Text>Agent edit</Text>"
    );

    let stale = client.tool(
        "project.write_file",
        &serde_json::json!({
            "path": "app.studio",
            "content": "<Text>Stale edit</Text>",
            "expected_revision": revision
        }),
    );
    assert_eq!(stale["isError"], true);
    assert_eq!(
        std::fs::read_to_string(project_root.join("app.studio")).unwrap(),
        "<Text>Agent edit</Text>"
    );
}

fn assert_patch_applies(client: &mut RpcClient, project_root: &Path) {
    let current = client.tool(
        "project.read_file",
        &serde_json::json!({ "path": "app.studio" }),
    );
    let current_revision = current["structuredContent"]["revision"].as_str().unwrap();
    let patch = client.tool(
        "project.apply_patch",
        &serde_json::json!({
            "path": "app.studio",
            "expected_revision": current_revision,
            "edits": [{ "start": 6, "end": 16, "replacement": "Patched" }]
        }),
    );
    assert_eq!(patch["isError"], false);
    assert_eq!(
        std::fs::read_to_string(project_root.join("app.studio")).unwrap(),
        "<Text>Patched</Text>"
    );
}

#[test]
fn agent_can_read_edit_and_reconcile_a_project_file_via_the_designer_server() {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(
        project.path().join("manifest.json"),
        include_bytes!("../../../examples/starter/manifest.json"),
    )
    .unwrap();
    std::fs::write(project.path().join("app.studio"), "<Text>Hello</Text>").unwrap();

    let (_server, mut client) = start_server(project.path());
    initialize(&mut client);
    let info = client.tool("project.info", &serde_json::json!({}));
    assert_eq!(
        info["structuredContent"]["project_root"],
        project.path().canonicalize().unwrap().to_str().unwrap()
    );
    assert_file_is_listed(&mut client);
    let revision = read_file_revision(&mut client);
    assert_write_and_stale_conflict(&mut client, project.path(), &revision);
    assert_patch_applies(&mut client, project.path());

    let status = client.tool("preview.status", &serde_json::json!({}));
    assert_eq!(status["isError"], false);
    assert_eq!(status["structuredContent"]["running"], false);

    let presentation = client.tool(
        "preview.presentation.set",
        &serde_json::json!({
            "viewport": { "width": 390, "height": 844 },
            "orientation": "portrait"
        }),
    );
    assert_eq!(presentation["isError"], true);
    assert_eq!(
        presentation["structuredContent"]["error"]["code"],
        "PREVIEW_CONTROL_FAILED"
    );

    let selection = client.tool(
        "preview.selection.set_mode",
        &serde_json::json!({ "enabled": true }),
    );
    assert_eq!(selection["isError"], true);
    assert_eq!(
        selection["structuredContent"]["error"]["code"],
        "PREVIEW_CONTROL_FAILED"
    );
}

#[test]
fn server_discovers_extended_tools_and_rejects_invalid_lifecycle_arguments() {
    let project = tempfile::tempdir().unwrap();
    let (_server, mut client) = start_server(project.path());
    initialize(&mut client);
    let tools = client.call("tools/list", &serde_json::json!({}));
    let tools = tools["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 22);
    for name in [
        "studio.check",
        "studio.build",
        "runtime.start",
        "preview.start",
        "project.search",
    ] {
        assert!(tools.iter().any(|tool| tool["name"] == name));
    }
    for (name, arguments) in [
        ("preview.start", serde_json::json!({"project_root": "/"})),
        ("runtime.start", serde_json::json!({})),
        ("studio.build", serde_json::json!([])),
    ] {
        let result = client.tool(name, &arguments);
        assert_eq!(result["isError"], true);
        assert_eq!(
            result["structuredContent"]["error"]["code"],
            "invalid_arguments"
        );
    }
    let idle = client.tool("runtime.status", &serde_json::json!({}));
    assert_eq!(idle["structuredContent"]["phase"], "not_started");
    let stopped = client.tool("preview.stop", &serde_json::json!({}));
    assert_eq!(stopped["structuredContent"]["managed"], false);
    let no_sources = client.tool("studio.check", &serde_json::json!({}));
    assert_eq!(
        no_sources["structuredContent"]["error"]["code"],
        "NO_SOURCE_FILES"
    );
}

#[cfg(unix)]
#[test]
fn agent_can_manage_cli_tasks_and_session_owned_preview_without_protocol_log_leaks() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("app.studio"), "<Text>Hello</Text>").unwrap();
    let binary = project.path().join("fake-studio");
    fs::write(&binary, "#!/bin/sh\ncase \"$1\" in\ncheck) echo '{\"code\":\"TEST_DIAGNOSTIC\"}'; echo check-failed >&2; exit 10;;\ndev) exec sleep 30;;\n*) echo \"$@\";;\nesac\n").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    let (mut server, mut client) = start_server_with_binaries(project.path(), Some(&binary));
    initialize(&mut client);

    let started = client.tool("studio.check", &serde_json::json!({}));
    assert_eq!(started["isError"], false);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let logs = client.tool("studio.task.logs", &serde_json::json!({}));
        let data = &logs["structuredContent"];
        if data["running"] == false && data["output_complete"] == true {
            assert_eq!(data["phase"], "failed");
            assert_eq!(data["exit_code"], 10);
            assert!(
                data["stdout"]["text"]
                    .as_str()
                    .unwrap()
                    .contains("TEST_DIAGNOSTIC")
            );
            assert!(
                data["stderr"]["text"]
                    .as_str()
                    .unwrap()
                    .contains("check-failed")
            );
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    let preview = client.tool("preview.start", &serde_json::json!({}));
    assert_eq!(preview["isError"], false);
    assert_eq!(preview["structuredContent"]["running"], true);
    let pid = nix::unistd::Pid::from_raw(
        i32::try_from(preview["structuredContent"]["pid"].as_u64().unwrap()).unwrap(),
    );
    // Normal stdin EOF must drop/reap managed children before the MCP process exits.
    drop(client);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(exit) = server.0.try_wait().unwrap() {
            assert!(exit.success());
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        nix::sys::signal::kill(pid, None),
        Err(nix::errno::Errno::ESRCH)
    );
}
