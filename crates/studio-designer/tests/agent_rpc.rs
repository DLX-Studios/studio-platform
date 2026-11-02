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
    let mut child = ServerProcess(
        Command::new(env!("CARGO_BIN_EXE_studio-designer"))
            .arg("agent-server")
            .arg("--project-root")
            .arg(project_root)
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
    assert_file_is_listed(&mut client);
    let revision = read_file_revision(&mut client);
    assert_write_and_stale_conflict(&mut client, project.path(), &revision);
    assert_patch_applies(&mut client, project.path());

    let status = client.tool("preview.status", &serde_json::json!({}));
    assert_eq!(status["isError"], false);
    assert_eq!(status["structuredContent"]["running"], false);
}
