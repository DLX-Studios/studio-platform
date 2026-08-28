#![allow(missing_docs)]
#![allow(clippy::needless_pass_by_value)]

use std::io::Cursor;

use serde_json::{Value, json};
use studio_language_server::{LanguageServer, Workspace};

fn frame(message: Value) -> Vec<u8> {
    let payload = serde_json::to_vec(&message).expect("fixture serializes");
    format!("Content-Length: {}\r\n\r\n", payload.len())
        .into_bytes()
        .into_iter()
        .chain(payload)
        .collect()
}

fn read_frames(mut bytes: &[u8]) -> Vec<Value> {
    let mut output = Vec::new();
    while !bytes.is_empty() {
        let separator = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("header separator");
        let header = std::str::from_utf8(&bytes[..separator]).expect("header utf8");
        let length: usize = header
            .strip_prefix("Content-Length: ")
            .expect("content length")
            .parse()
            .expect("length");
        let start = separator + 4;
        output.push(serde_json::from_slice(&bytes[start..start + length]).expect("response json"));
        bytes = &bytes[start + length..];
    }
    output
}

#[test]
fn drives_stdio_server_end_to_end_without_designer() {
    let source = "<script>\n</script>\n<Button id=\"save\" />";
    let mut workspace = Workspace::new();
    workspace.add_file("file:///main.studio", source);
    let mut server = LanguageServer::new(workspace);
    let mut request = Vec::new();
    request.extend(frame(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    ));
    request.extend(frame(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///main.studio","languageId":"studio","version":1,"text":source}}})));
    request.extend(frame(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///main.studio"},"position":{"line":2,"character":2}}})));
    request.extend(frame(
        json!({"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}),
    ));
    let mut output = Vec::new();
    server
        .serve(Cursor::new(request), &mut output)
        .expect("stdio session succeeds");
    let responses = read_frames(&output);
    assert!(
        responses
            .iter()
            .any(|value| value.get("id") == Some(&json!(1)))
    );
    assert!(
        responses
            .iter()
            .any(|value| value.get("id") == Some(&json!(2)))
    );
    assert!(
        responses
            .iter()
            .any(|value| value.get("method") == Some(&json!("textDocument/publishDiagnostics")))
    );
}
