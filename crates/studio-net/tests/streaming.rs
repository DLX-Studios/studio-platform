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
