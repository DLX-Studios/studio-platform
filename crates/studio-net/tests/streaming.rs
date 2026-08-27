//! SSE streaming: validated typed chunks, guest cancellation, per-stream bounds, and host-owned
//! reconnect policy.

#![allow(missing_docs, clippy::all, clippy::pedantic, dead_code)]

mod common;

use std::time::Duration;

use common::{
    ReadStep, broker, broker_with_limits, chunk_values, code_of, drain_stream, sse_group,
};
use studio_net::declaration::HttpMethod;
use studio_net::error::BrokerErrorCode;
use studio_net::guest::{BrokerRequest, StreamEvent};
use studio_net::limits::BrokerLimits;

const ORIGIN: &str = common::ORIGIN;
const PATH: &str = "/v1/stream";

fn frame(text: &str) -> Vec<u8> {
    format!("data: {{\"text\":\"{text}\"}}\n\n").into_bytes()
}

fn stream_request() -> BrokerRequest {
    BrokerRequest::new(ORIGIN, HttpMethod::Get, PATH)
}

#[test]
fn chunks_are_framed_across_reads_and_validated() {
    let fixture = broker(&[sse_group()]);
    let mut whole = frame("alpha");
    whole.extend_from_slice(&frame("beta")[..4]);
    let mut rest = frame("beta")[4..].to_vec();
    rest.extend_from_slice(&frame("gamma"));
    rest.extend_from_slice(b": keepalive comment\n\n");
    rest.extend_from_slice(b"\n");
    fixture.transport.plan_stream(vec![
        ReadStep::Bytes(whole),
        ReadStep::Bytes(rest),
        ReadStep::End,
    ]);
    let events = drain_stream(
        &fixture
            .broker
            .open_stream(stream_request())
            .expect("stream"),
    );
    assert!(
        events
            .first()
            .is_some_and(|event| matches!(event, StreamEvent::Opened))
    );
    assert_eq!(
        chunk_values(&events),
        vec![
            serde_json::json!({"text":"alpha"}),
            serde_json::json!({"text":"beta"}),
            serde_json::json!({"text":"gamma"}),
        ]
    );
    assert!(matches!(events.last(), Some(StreamEvent::Completed)));
}

#[test]
fn invalid_chunk_json_fails_the_stream_before_guest_visibility() {
    let fixture = broker(&[sse_group()]);
    fixture.transport.plan_stream(vec![ReadStep::Bytes(
        b"data: {not json}\n\ndata: {\"text\":\"ok\"}\n\n".to_vec(),
    )]);
    let events = drain_stream(
        &fixture
            .broker
            .open_stream(stream_request())
            .expect("stream"),
    );
    assert_eq!(chunk_values(&events).len(), 0);
    assert!(matches!(events.last(), Some(StreamEvent::Failed(error))
 if code_of(error) == BrokerErrorCode::ResponseMalformed));
}

#[test]
fn schema_violating_chunk_is_never_delivered() {
    let fixture = broker(&[sse_group()]);
    fixture.transport.plan_stream(vec![ReadStep::Bytes(
        b"data: {\"wrong\":true}\n\n".to_vec(),
    )]);
    let events = drain_stream(
        &fixture
            .broker
            .open_stream(stream_request())
            .expect("stream"),
    );
    assert_eq!(chunk_values(&events).len(), 0);
    assert!(matches!(events.last(), Some(StreamEvent::Failed(error))
 if code_of(error) == BrokerErrorCode::ResponseSchemaMismatch));
}

#[test]
fn guest_cancellation_stops_delivery() {
    let fixture = broker(&[sse_group()]);
    let mut steps = vec![ReadStep::Bytes(frame("first"))];
    for index in 0..1000 {
        steps.push(ReadStep::Wait(Duration::from_millis(if index == 0 {
            200
        } else {
            5
        })));
        steps.push(ReadStep::Bytes(
            format!("data: {{\"text\":\"{index}\"}}\n\n").into_bytes(),
        ));
    }
    fixture.transport.plan_stream(steps);
    let handle = fixture
        .broker
        .open_stream(stream_request())
        .expect("stream");
    // Wait until the first validated chunk arrives, then cancel.
    loop {
        match handle.next_event() {
            Some(StreamEvent::Chunk(_)) => break,
            Some(_) => continue,
            None => panic!("stream ended before first chunk"),
        }
    }
    handle.cancel();
    let events = drain_stream(&handle);
    assert!(matches!(events.last(), Some(StreamEvent::Cancelled)));
}

#[test]
fn event_count_bound_terminates_the_stream() {
    let mut group = sse_group();
    group.limits.max_stream_events = Some(2);
    let fixture = broker_with_limits(&[group], BrokerLimits::default());
    fixture.transport.plan_stream(vec![
        ReadStep::Bytes(frame("one")),
        ReadStep::Bytes(frame("two")),
        ReadStep::Bytes(frame("three")),
        ReadStep::End,
    ]);
    let events = drain_stream(
        &fixture
            .broker
            .open_stream(stream_request())
            .expect("stream"),
    );
    assert_eq!(chunk_values(&events).len(), 2);
    assert!(matches!(events.last(), Some(StreamEvent::Failed(error))
 if code_of(error) == BrokerErrorCode::StreamExceeded));
}

#[test]
fn byte_budget_bound_terminates_the_stream() {
    let mut group = sse_group();
    group.limits.max_stream_bytes = Some(8);
    let fixture = broker_with_limits(&[group], BrokerLimits::default());
    fixture
        .transport
        .plan_stream(vec![ReadStep::Bytes(frame("too-large-for-budget"))]);
    let events = drain_stream(
        &fixture
            .broker
            .open_stream(stream_request())
            .expect("stream"),
    );
    assert!(chunk_values(&events).is_empty());
    assert!(matches!(events.last(), Some(StreamEvent::Failed(error))
 if code_of(error) == BrokerErrorCode::StreamExceeded));
}

#[test]
fn transport_failure_triggers_host_owned_reconnect_with_last_event_id() {
    let fixture = broker(&[sse_group()]);
    let mut first = frame("before-drop");
    first.extend_from_slice(b"id: evt-7\n\n");
    fixture.transport.plan_stream(vec![
        ReadStep::Bytes(first),
        ReadStep::Fail(studio_net::transport::TransportError::ConnectionFailure),
    ]);
    fixture.transport.plan_stream(vec![
        ReadStep::Bytes(frame("after-reconnect")),
        ReadStep::End,
    ]);
    let events = drain_stream(
        &fixture
            .broker
            .open_stream(stream_request())
            .expect("stream"),
    );
    assert_eq!(
        chunk_values(&events),
        vec![
            serde_json::json!({"text":"before-drop"}),
            serde_json::json!({"text":"after-reconnect"}),
        ]
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, StreamEvent::RetryScheduled { attempt: 1, .. }))
    );
    let requests = fixture.transport.recorded_requests();
    assert_eq!(requests.len(), 2, "one reconnect");
    assert!(
        requests[1]
            .headers
            .iter()
            .any(|(name, value)| name == "last-event-id" && value == "evt-7")
    );
}

#[test]
fn exhausted_reconnect_policy_fails_the_stream() {
    let fixture = broker(&[sse_group()]);
    fixture.transport.plan_stream(vec![ReadStep::Fail(
        studio_net::transport::TransportError::ConnectionFailure,
    )]);
    fixture.transport.plan_stream(vec![ReadStep::Fail(
        studio_net::transport::TransportError::ConnectionFailure,
    )]);
    let events = drain_stream(
        &fixture
            .broker
            .open_stream(stream_request())
            .expect("stream"),
    );
    assert!(matches!(events.last(), Some(StreamEvent::Failed(error))
 if code_of(error) == BrokerErrorCode::TransportFailure));
}

#[test]
fn non_streaming_route_rejects_open_stream_and_streaming_route_rejects_execute() {
    let fixture = broker(&[sse_group(), common::json_api_group()]);
    let error = fixture
        .broker
        .open_stream(common::get_items_request("/v1/items"))
        .expect_err("non-streaming route");
    assert_eq!(code_of(&error), BrokerErrorCode::RouteNotStreaming);

    fixture
        .transport
        .respond(200, "application/json", r#"{"id":"a","name":"b"}"#);
    let _ok = fixture
        .broker
        .execute(common::get_items_request("/v1/items"))
        .expect("plain route executes");

    let error = fixture
        .broker
        .execute(stream_request())
        .expect_err("streaming route");
    assert_eq!(code_of(&error), BrokerErrorCode::RouteIsStreaming);
}

#[test]
fn post_stream_preserves_bounded_canonical_body_for_reconnects() {
    let mut group = sse_group();
    group.methods = vec![HttpMethod::Post];
    group.allowed_headers = vec!["content-type".to_owned()];
    group.request_schema = Some(serde_json::json!({
        "type": "object",
        "required": ["model", "messages", "stream"],
        "properties": {
            "model": { "type": "string", "minLength": 1, "maxLength": 64 },
            "messages": { "type": "array", "minItems": 1, "maxItems": 4, "items": {
                "type": "object",
                "required": ["role", "content"],
                "properties": {
                    "role": { "type": "string", "maxLength": 32 },
                    "content": { "type": "string", "maxLength": 1024 }
                },
                "additionalProperties": false
            }},
            "stream": { "type": "boolean" },
            "temperature": { "type": "number", "minimum": 0, "maximum": 2 },
            "stream_options": { "type": "object", "properties": { "include_usage": { "type": "boolean" } }, "additionalProperties": false }
        },
        "additionalProperties": false
    }));
    let fixture = broker(&[group]);
    fixture
        .transport
        .plan_stream(vec![
            ReadStep::Bytes(b"data: {\"text\":\"ok\"}\n\n".to_vec()),
            ReadStep::Fail(studio_net::transport::TransportError::ConnectionFailure),
        ]);
    fixture.transport.plan_stream(vec![ReadStep::End]);
    let body = serde_json::json!({
        "model": "proof-model",
        "messages": [
            { "role": "system", "content": "be concise" },
            { "role": "user", "content": "hello" }
        ],
        "stream": true,
        "temperature": 0.25,
        "stream_options": { "include_usage": true }
    });
    let request = BrokerRequest::new(ORIGIN, HttpMethod::Post, PATH)
        .with_header("content-type", "application/json")
        .with_body(body.clone());
    let _events = drain_stream(&fixture.broker.open_stream(request).expect("post stream"));
    let requests = fixture.transport.recorded_requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, HttpMethod::Post);
    assert_eq!(requests[0].body, Some(serde_json::to_vec(&body).expect("json body")));
    assert_eq!(requests[1].body, requests[0].body);
}

#[test]
fn invalid_post_stream_body_is_rejected_before_dispatch() {
    let mut group = sse_group();
    group.methods = vec![HttpMethod::Post];
    group.request_schema = Some(serde_json::json!({
        "type": "object",
        "required": ["model"],
        "properties": { "model": { "type": "string", "minLength": 1, "maxLength": 64 } },
        "additionalProperties": false
    }));
    let fixture = broker(&[group]);
    let error = fixture
        .broker
        .open_stream(
            BrokerRequest::new(ORIGIN, HttpMethod::Post, PATH)
                .with_body(serde_json::json!({ "model": "" })),
        )
        .expect_err("invalid body");
    assert_eq!(code_of(&error), BrokerErrorCode::RequestSchemaInvalid);
    assert!(fixture.transport.recorded_requests().is_empty());
}

#[test]
fn terminal_usage_and_error_frames_keep_order_before_done() {
    let mut group = sse_group();
    group.chunk_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "choices": { "type": "array" },
            "usage": { "type": "object", "additionalProperties": true },
            "error": { "type": "object", "additionalProperties": true }
        },
        "additionalProperties": true
    });
    let fixture = broker(&[group]);
    fixture.transport.plan_stream(vec![ReadStep::Bytes(
        b"data: {\"choices\":[{\"delta\":{\"content\":\"one\"}}]}\n\ndata: {\"choices\":[],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":1,\"total_tokens\":3}}\n\ndata: {\"error\":{\"message\":\"provider unavailable\"}}\n\ndata: [DONE]\n\n".to_vec(),
    )]);
    let events = drain_stream(
        &fixture
            .broker
            .open_stream(stream_request())
            .expect("stream"),
    );
    let chunks: Vec<&serde_json::Value> = events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::Chunk(value) => Some(value),
            _ => None,
        })
        .collect();
    assert_eq!(chunks.len(), 3);
    assert!(chunks[0].get("choices").is_some());
    assert!(chunks[1].get("usage").is_some());
    assert!(chunks[2].get("error").is_some());
    assert!(matches!(events.last(), Some(StreamEvent::Completed)));
}
