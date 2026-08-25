//! Response pipeline: malformed or schema-violating responses are rejected before guest
//! visibility; declared and transport bounds are enforced.

#![allow(missing_docs)]

mod common;

use common::{broker, broker_with_limits, code_of, get_items_request, json_api_group};
use studio_net::error::BrokerErrorCode;
use studio_net::limits::{BrokerLimits, DeclaredLimits};

#[test]
fn valid_response_is_delivered_typed() {
    let fixture = broker(&[json_api_group()]);
    fixture
        .transport
        .respond(200, "application/json", r#"{"id":"a","name":"b"}"#);
    let response = fixture
        .broker
        .execute(get_items_request("/v1/items"))
        .expect("valid response");
    assert_eq!(response.status(), 200);
    assert_eq!(response.body()["id"], "a");
}

#[test]
fn malformed_json_never_reaches_the_guest() {
    let fixture = broker(&[json_api_group()]);
    fixture
        .transport
        .respond(200, "application/json", "<html>not json</html>");
    let error = fixture
        .broker
        .execute(get_items_request("/v1/items"))
        .expect_err("malformed body");
    assert_eq!(code_of(&error), BrokerErrorCode::ResponseMalformed);
}

#[test]
fn schema_violating_response_is_rejected_before_guest_visibility() {
    let fixture = broker(&[json_api_group()]);
    // Missing required `name`; extra undeclared property is also rejected.
    fixture.transport.respond(
        200,
        "application/json",
        r#"{"id":"a","name":7,"surprise":true}"#,
    );
    let error = fixture
        .broker
        .execute(get_items_request("/v1/items"))
        .expect_err("schema violation");
    assert_eq!(code_of(&error), BrokerErrorCode::ResponseSchemaMismatch);
}

#[test]
fn upstream_status_outside_success_window_is_rejected() {
    let fixture = broker(&[json_api_group()]);
    fixture.transport.respond(503, "application/json", "{}");
    let error = fixture
        .broker
        .execute(get_items_request("/v1/items"))
        .expect_err("upstream failure");
    assert_eq!(code_of(&error), BrokerErrorCode::UpstreamRejected);
}

#[test]
fn declared_response_bound_is_enforced() {
    let mut group = json_api_group();
    group.limits = DeclaredLimits {
        max_response_bytes: Some(16),
        ..DeclaredLimits::default()
    };
    let fixture = broker_with_limits(&[group], BrokerLimits::default());
    fixture.transport.respond(
        200,
        "application/json",
        r#"{"id":"a-long-identifier","name":"b"}"#,
    );
    let error = fixture
        .broker
        .execute(get_items_request("/v1/items"))
        .expect_err("oversized response");
    assert_eq!(code_of(&error), BrokerErrorCode::ResponseTooLarge);
}

#[test]
fn route_without_response_schema_accepts_any_valid_json() {
    let mut group = json_api_group();
    group.response_schema = None;
    let fixture = broker(&[group]);
    fixture.transport.respond(200, "application/json", "[1,2,3]");
    let response = fixture
        .broker
        .execute(get_items_request("/v1/items"))
        .expect("any valid JSON passes when no schema is declared");
    assert_eq!(response.body()[0], 1);
}
