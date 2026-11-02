//! Local MCP/JSON-RPC server for agent access to one Studio source project.
//!
//! The server uses newline-delimited JSON on stdio, which lets local agent
//! hosts start it as a child process. It never writes protocol output to
//! stderr/stdout mixed with logs. Every file operation is delegated to the
//! bounded, project-confined [`ProjectSourceWorkspace`].

use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use crate::source_workspace::{PatchEdit, ProjectSourceError, ProjectSourceWorkspace};

const MAX_RPC_FRAME_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_PROTOCOL_VERSION: &str = "2025-11-25";
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] =
    &["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];

/// Run the Designer MCP server on the process's stdin and stdout.
///
/// The selected project root is canonicalized before any tool is exposed.
/// Exit code 0 means the client closed stdin normally; setup or transport
/// failures return 2.
pub fn run_stdio(project_root: &Path) -> i32 {
    let workspace = match ProjectSourceWorkspace::open(project_root) {
        Ok(workspace) => workspace,
        Err(error) => {
            eprintln!("Studio Designer agent server: {error}");
            return 2;
        }
    };

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = BufReader::new(stdin.lock());
    let mut output = stdout.lock();
    let mut frame = Vec::new();

    loop {
        let has_frame = match read_bounded_frame(&mut input, &mut frame, MAX_RPC_FRAME_BYTES) {
            Ok(has_frame) => has_frame,
            Err(error) => {
                eprintln!("Studio Designer agent server: invalid input frame: {error}");
                return 2;
            }
        };
        if !has_frame {
            return 0;
        }

        let response = match serde_json::from_slice::<Value>(&frame) {
            Ok(request) => dispatch(&request, &workspace),
            Err(error) => Some(json_rpc_error(Value::Null, -32700, error.to_string())),
        };
        if let Some(response) = response {
            if let Err(error) = write_frame(&mut output, &response) {
                if error.kind() == io::ErrorKind::InvalidData {
                    let id = response.get("id").cloned().unwrap_or(Value::Null);
                    let fallback = json_rpc_error(id, -32603, "response exceeds the message limit");
                    if let Err(error) = write_frame(&mut output, &fallback) {
                        eprintln!("Studio Designer agent server: write failed: {error}");
                        return 2;
                    }
                } else {
                    eprintln!("Studio Designer agent server: write failed: {error}");
                    return 2;
                }
            }
        }
    }
}

fn read_bounded_frame(
    input: &mut impl BufRead,
    frame: &mut Vec<u8>,
    max_bytes: usize,
) -> io::Result<bool> {
    frame.clear();
    let bytes = input
        .take((max_bytes.saturating_add(1)) as u64)
        .read_until(b'\n', frame)?;
    if bytes == 0 {
        return Ok(false);
    }
    if frame.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "request frame exceeds the size limit",
        ));
    }
    if frame.last() == Some(&b'\n') {
        frame.pop();
    }
    if frame.last() == Some(&b'\r') {
        frame.pop();
    }
    Ok(true)
}

fn dispatch(request: &Value, workspace: &ProjectSourceWorkspace) -> Option<Value> {
    let id = request.get("id").cloned();
    let Some(object) = request.as_object() else {
        return id.map(|id| json_rpc_error(id, -32600, "request must be a JSON object"));
    };
    if id
        .as_ref()
        .is_some_and(|id| !matches!(id, Value::Null | Value::String(_) | Value::Number(_)))
    {
        return Some(json_rpc_error(
            Value::Null,
            -32600,
            "request id must be a string, number, or null",
        ));
    }
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return id.map(|id| json_rpc_error(id, -32600, "jsonrpc must be \"2.0\""));
    }
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return id.map(|id| json_rpc_error(id, -32600, "request method must be a string"));
    };

    match method {
        "initialize" => id.map(|id| initialize(id, request)),
        "notifications/initialized" | "notifications/cancelled" => None,
        "ping" => id.map(|id| json_rpc_result(id, json!({}))),
        "tools/list" => id.map(|id| json_rpc_result(id, json!({ "tools": tools() }))),
        "tools/call" => {
            let Some(params) = request.get("params").and_then(Value::as_object) else {
                return id.map(|id| json_rpc_error(id, -32602, "tools/call requires params"));
            };
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return id.map(|id| json_rpc_error(id, -32602, "tools/call requires a tool name"));
            };
            if !is_known_tool(name) {
                return id.map(|id| json_rpc_error(id, -32601, "tool not found"));
            }
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            id.map(|id| json_rpc_result(id, call_tool(name, &arguments, workspace)))
        }
        _ => id.map(|id| json_rpc_error(id, -32601, "method not found")),
    }
}

fn initialize(id: Value, request: &Value) -> Value {
    let requested = request
        .pointer("/params/protocolVersion")
        .and_then(Value::as_str);
    let protocol_version = requested
        .filter(|version| SUPPORTED_PROTOCOL_VERSIONS.contains(version))
        .unwrap_or(DEFAULT_PROTOCOL_VERSION);
    json_rpc_result(
        id,
        json!({
            "protocolVersion": protocol_version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": {
                "name": "studio-designer",
                "version": env!("CARGO_PKG_VERSION")
            },
            "instructions": "Edit Studio project source files with revision-guarded tools. Saved changes are observed by studio dev."
        }),
    )
}

fn call_tool(name: &str, arguments: &Value, workspace: &ProjectSourceWorkspace) -> Value {
    let result = match name {
        "project.info" => Ok(json!({
            "project_root": workspace.root().to_string_lossy(),
            "watcher": "studio dev"
        })),
        "project.list_files" => workspace
            .list_files()
            .map(|files| {
                json!({
                    "files": files.into_iter().map(|file| json!({
                        "path": file.path,
                        "revision": file.revision,
                        "bytes": file.bytes
                    })).collect::<Vec<_>>()
                })
            })
            .map_err(project_error),
        "project.read_file" => {
            let Some(path) = arguments.get("path").and_then(Value::as_str) else {
                return tool_error("invalid_arguments", "path must be a string");
            };
            workspace
                .read_file(path)
                .map(|file| {
                    json!({
                        "path": file.path,
                        "revision": file.revision,
                        "content": file.content
                    })
                })
                .map_err(project_error)
        }
        "project.write_file" => {
            let Some(path) = arguments.get("path").and_then(Value::as_str) else {
                return tool_error("invalid_arguments", "path must be a string");
            };
            let Some(content) = arguments.get("content").and_then(Value::as_str) else {
                return tool_error("invalid_arguments", "content must be a string");
            };
            let expected_revision = match arguments.get("expected_revision") {
                None | Some(Value::Null) => None,
                Some(Value::String(revision)) => Some(revision.as_str()),
                Some(_) => {
                    return tool_error(
                        "invalid_arguments",
                        "expected_revision must be a SHA-256 revision or null for create-only",
                    );
                }
            };
            workspace
                .write_file(path, content, expected_revision)
                .map(|file| {
                    json!({
                        "path": file.path,
                        "revision": file.revision,
                        "bytes": file.content.len()
                    })
                })
                .map_err(project_error)
        }
        "project.apply_patch" => {
            let Some(path) = arguments.get("path").and_then(Value::as_str) else {
                return tool_error("invalid_arguments", "path must be a string");
            };
            let Some(expected_revision) =
                arguments.get("expected_revision").and_then(Value::as_str)
            else {
                return tool_error("invalid_arguments", "expected_revision must be a string");
            };
            let edits: Vec<PatchEdit> = match arguments.get("edits") {
                Some(edits) => match serde_json::from_value(edits.clone()) {
                    Ok(edits) => edits,
                    Err(error) => {
                        return tool_error("invalid_arguments", error.to_string());
                    }
                },
                None => return tool_error("invalid_arguments", "edits must be an array"),
            };
            workspace
                .apply_patch(path, &edits, expected_revision)
                .map(|file| {
                    json!({
                        "path": file.path,
                        "revision": file.revision,
                        "bytes": file.content.len()
                    })
                })
                .map_err(project_error)
        }
        "preview.status" => {
            match studio_protocol::dev_rpc::request_status(
                &workspace.root().join("build"),
                Duration::from_secs(2),
            ) {
                Ok(status) => {
                    let mut status = serde_json::to_value(status).unwrap_or_else(|_| json!({}));
                    if let Some(fields) = status.as_object_mut() {
                        fields.insert("running".to_owned(), Value::Bool(true));
                    }
                    Ok(status)
                }
                Err(error) if error.is_not_running() => Ok(json!({
                    "running": false,
                    "phase": "not_running"
                })),
                Err(error) => Err(("PREVIEW_UNAVAILABLE".to_owned(), error.to_string())),
            }
        }
        _ => unreachable!("dispatch validates Designer tool names"),
    };

    match result {
        Ok(data) => tool_success(data),
        Err((code, message)) => tool_error(&code, message),
    }
}

fn is_known_tool(name: &str) -> bool {
    matches!(
        name,
        "project.info"
            | "project.list_files"
            | "project.read_file"
            | "project.write_file"
            | "project.apply_patch"
            | "preview.status"
    )
}

fn project_error(error: ProjectSourceError) -> (String, String) {
    (error.code().to_owned(), error.to_string())
}

fn tool_success(data: Value) -> Value {
    let text = serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".to_owned());
    json!({
        "content": [{ "type": "text", "text": text }],
        "structuredContent": data,
        "isError": false
    })
}

fn tool_error(code: &str, message: impl Into<String>) -> Value {
    let message = message.into();
    json!({
        "content": [{ "type": "text", "text": message }],
        "structuredContent": { "error": { "code": code, "message": message } },
        "isError": true
    })
}

fn tools() -> Value {
    json!([
        {
            "name": "project.info",
            "description": "Return the configured Studio project root and preview watcher.",
            "annotations": { "readOnlyHint": true, "destructiveHint": false, "openWorldHint": false },
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        },
        {
            "name": "project.list_files",
            "description": "List editable UTF-8 project files and their current SHA-256 revisions.",
            "annotations": { "readOnlyHint": true, "destructiveHint": false, "openWorldHint": false },
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        },
        {
            "name": "project.read_file",
            "description": "Read one project source file and its SHA-256 revision before editing it.",
            "annotations": { "readOnlyHint": true, "destructiveHint": false, "openWorldHint": false },
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string", "description": "Project-relative path." } },
                "required": ["path"],
                "additionalProperties": false
            }
        },
        {
            "name": "project.write_file",
            "description": "Write or create a complete UTF-8 file under an existing project directory. Supply the revision from read_file; use null only to create a new file. The saved file is picked up by studio dev.",
            "annotations": { "readOnlyHint": false, "destructiveHint": true, "openWorldHint": false },
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                    "expected_revision": { "type": ["string", "null"] }
                },
                "required": ["path", "content", "expected_revision"],
                "additionalProperties": false
            }
        },
        {
            "name": "project.apply_patch",
            "description": "Apply UTF-8 byte-range replacements to a file only if its current revision matches.",
            "annotations": { "readOnlyHint": false, "destructiveHint": true, "openWorldHint": false },
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "expected_revision": { "type": "string" },
                    "edits": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "start": { "type": "integer", "minimum": 0 },
                                "end": { "type": "integer", "minimum": 0 },
                                "replacement": { "type": "string" }
                            },
                            "required": ["start", "end", "replacement"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["path", "expected_revision", "edits"],
                "additionalProperties": false
            }
        },
        {
            "name": "preview.status",
            "description": "Return the currently applied Studio dev revision and the last reload diagnostic.",
            "annotations": { "readOnlyHint": true, "destructiveHint": false, "openWorldHint": false },
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        }
    ])
}

fn json_rpc_result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn json_rpc_error(id: Value, code: i64, message: impl Into<String>) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message.into() } })
}

fn write_frame(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let bytes = serde_json::to_vec(message).map_err(io::Error::other)?;
    if bytes.len() > MAX_RPC_FRAME_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "response frame exceeds the size limit",
        ));
    }
    writer.write_all(&bytes)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn workspace() -> (tempfile::TempDir, ProjectSourceWorkspace) {
        let directory = tempfile::tempdir().unwrap();
        fs::write(directory.path().join("manifest.json"), "{}").unwrap();
        fs::write(directory.path().join("app.studio"), "<Text>Hello</Text>").unwrap();
        let workspace = ProjectSourceWorkspace::open(directory.path()).unwrap();
        (directory, workspace)
    }

    #[test]
    fn mcp_initialize_and_tool_discovery_use_json_rpc() {
        let (_directory, workspace) = workspace();
        let response = dispatch(
            &json!({
                "jsonrpc": "2.0",
                "id": 7,
                "method": "initialize",
                "params": { "protocolVersion": DEFAULT_PROTOCOL_VERSION }
            }),
            &workspace,
        )
        .unwrap();
        assert_eq!(
            response["result"]["protocolVersion"],
            DEFAULT_PROTOCOL_VERSION
        );

        let tools = dispatch(
            &json!({ "jsonrpc": "2.0", "id": 8, "method": "tools/list" }),
            &workspace,
        )
        .unwrap();
        assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 6);
        assert!(
            tools["result"]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["name"] == "project.apply_patch")
        );
    }

    #[test]
    fn tool_write_uses_revision_and_read_returns_the_saved_content() {
        let (_directory, workspace) = workspace();
        let initial = workspace.read_file("app.studio").unwrap();
        let response = dispatch(
            &json!({
                "jsonrpc": "2.0",
                "id": 9,
                "method": "tools/call",
                "params": {
                    "name": "project.write_file",
                    "arguments": {
                        "path": "app.studio",
                        "content": "<Text>Updated</Text>",
                        "expected_revision": initial.revision
                    }
                }
            }),
            &workspace,
        )
        .unwrap();
        assert_eq!(response["result"]["isError"], false);
        assert_eq!(
            workspace.read_file("app.studio").unwrap().content,
            "<Text>Updated</Text>"
        );
    }

    #[test]
    fn notifications_do_not_write_a_response_and_unknown_methods_use_standard_errors() {
        let (_directory, workspace) = workspace();
        assert!(
            dispatch(
                &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
                &workspace
            )
            .is_none()
        );
        let response = dispatch(
            &json!({ "jsonrpc": "2.0", "id": "x", "method": "not-a-method" }),
            &workspace,
        )
        .unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }

    #[test]
    fn unknown_tools_and_invalid_request_ids_use_json_rpc_errors() {
        let (_directory, workspace) = workspace();
        let unknown = dispatch(
            &json!({
                "jsonrpc": "2.0",
                "id": 10,
                "method": "tools/call",
                "params": { "name": "project.delete_everything", "arguments": {} }
            }),
            &workspace,
        )
        .unwrap();
        assert_eq!(unknown["error"]["code"], -32601);

        let invalid_id = dispatch(
            &json!({ "jsonrpc": "2.0", "id": [], "method": "ping" }),
            &workspace,
        )
        .unwrap();
        assert_eq!(invalid_id["error"]["code"], -32600);
        assert!(invalid_id["id"].is_null());
    }

    #[test]
    fn stdio_frames_are_newline_delimited_and_bounded() {
        let mut frame = Vec::new();
        let mut valid = io::Cursor::new(b"{\"jsonrpc\":\"2.0\"}\n".to_vec());
        assert!(read_bounded_frame(&mut valid, &mut frame, 32).unwrap());
        assert_eq!(frame, br#"{"jsonrpc":"2.0"}"#);
        assert!(!read_bounded_frame(&mut valid, &mut frame, 32).unwrap());

        let mut oversized = io::Cursor::new(b"12345\n".to_vec());
        assert_eq!(
            read_bounded_frame(&mut oversized, &mut frame, 4)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
}
