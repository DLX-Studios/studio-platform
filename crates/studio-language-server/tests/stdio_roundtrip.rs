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
    let source = "<Button id=\"save\" />\n<Text value={token.brand.primary} />\n<Text value={$item.user.name} />\n<Text value={plugin.analytics} />\n<CustomCard";
    let token_character = source.lines().nth(1).unwrap().find("token.").unwrap() as u32 + 8;
    let item_character = source.lines().nth(2).unwrap().find("$item.").unwrap() as u32 + 8;
    let plugin_character = source.lines().nth(3).unwrap().find("plugin.").unwrap() as u32 + 8;
    let brand_character = source
        .lines()
        .nth(1)
        .unwrap()
        .find("brand.primary")
        .unwrap() as u32
        + 4;
    let mut workspace = Workspace::new();
    workspace.add_component("CustomCard", Some("Project card".to_owned()));
    workspace.add_token(
        "brand.primary",
        "Color",
        Some("Primary brand color".to_owned()),
    );
    workspace.add_plugin_surface(
        "analytics",
        "Analytics SDK",
        Some("Track product events".to_owned()),
    );
    workspace.add_response_schema("user", [("name", "string")]);
    workspace.add_file("file:///main.studio", source);
    let mut server = LanguageServer::new(workspace);
    let mut request = Vec::new();
    request.extend(frame(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    ));
    request.extend(frame(json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///main.studio","languageId":"studio","version":1,"text":source}}})));
    request.extend(frame(json!({"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///main.studio"},"position":{"line":4,"character":3}}})));
    request.extend(frame(json!({"jsonrpc":"2.0","id":3,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///main.studio"},"position":{"line":1,"character":token_character}}})));
    request.extend(frame(json!({"jsonrpc":"2.0","id":4,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///main.studio"},"position":{"line":2,"character":item_character}}})));
    request.extend(frame(json!({"jsonrpc":"2.0","id":5,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///main.studio"},"position":{"line":3,"character":plugin_character}}})));
    request.extend(frame(json!({"jsonrpc":"2.0","id":6,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///main.studio"},"position":{"line":1,"character":brand_character}}})));
    request.extend(frame(json!({"jsonrpc":"2.0","id":7,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///main.studio"},"position":{"line":1,"character":brand_character}}})));
    let invalid_source = "<CustomCard>";
    request.extend(frame(json!({"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///main.studio","version":2},"contentChanges":[{"text":invalid_source}]}})));
    request.extend(frame(json!({"jsonrpc":"2.0","id":8,"method":"textDocument/diagnostic","params":{"textDocument":{"uri":"file:///main.studio"}}})));
    request.extend(frame(json!({"jsonrpc":"2.0","id":9,"method":"textDocument/completion","params":{"textDocument":{"uri":"not-a-uri"},"position":{"line":0,"character":0}}})));
    request.extend(frame(json!({"jsonrpc":"2.0","id":10,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///main.studio"},"position":{"line":"bad","character":0}}})));
    request.extend(frame(
        json!({"jsonrpc":"2.0","id":11,"method":"shutdown","params":null}),
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
    for id in 2..=8 {
        assert!(
            responses
                .iter()
                .any(|value| value.get("id") == Some(&json!(id)))
        );
    }
    let completion = |id: u64| {
        responses
            .iter()
            .find(|value| value.get("id") == Some(&json!(id)))
            .and_then(|value| value.pointer("/result/items"))
            .expect("completion result")
    };
    assert!(
        completion(2).to_string().contains("CustomCard"),
        "{}",
        completion(2)
    );
    assert!(completion(3).to_string().contains("brand.primary"));
    assert!(completion(4).to_string().contains("name"));
    assert!(completion(5).to_string().contains("analytics"));
    let hover = responses
        .iter()
        .find(|value| value.get("id") == Some(&json!(6)))
        .and_then(|value| value.pointer("/result/contents/value"))
        .and_then(Value::as_str)
        .expect("token hover result");
    assert!(hover.contains("brand.primary"));
    assert_eq!(
        responses
            .iter()
            .find(|value| value.get("id") == Some(&json!(7)))
            .and_then(|value| value.pointer("/result/0/uri"))
            .and_then(Value::as_str),
        Some("studio://tokens")
    );
    assert!(
        responses.iter().any(|value| {
            value.get("id") == Some(&json!(8))
                && value
                    .pointer("/result/items")
                    .is_some_and(|items| !items.as_array().unwrap().is_empty())
        }),
        "diagnostic response: {:?}",
        responses
            .iter()
            .find(|value| value.get("id") == Some(&json!(8)))
    );
    for id in [9, 10] {
        assert_eq!(
            responses
                .iter()
                .find(|value| value.get("id") == Some(&json!(id)))
                .and_then(|value| value.pointer("/error/code")),
            Some(&json!(-32602))
        );
    }
    assert!(
        responses
            .iter()
            .any(|value| value.get("method") == Some(&json!("textDocument/publishDiagnostics")))
    );
}
