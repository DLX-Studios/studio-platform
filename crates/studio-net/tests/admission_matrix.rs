//! Admission denial matrix: undeclared origins, paths, methods, and headers are denied with
//! stable codes before any transport activity.

#![allow(missing_docs)]

mod common;

use common::{broker, code_of, get_items_request, json_api_group, ORIGIN};
use studio_net::declaration::HttpMethod;
use studio_net::error::BrokerErrorCode;
use studio_net::guest::BrokerRequest;

#[test]
fn undeclared_origin_is_denied_with_stable_code() {
    let fixture = broker(&[json_api_group()]);
    let error = fixture
        .broker
        .execute(BrokerRequest::new(
            "https://evil.example.test",
            HttpMethod::Get,
            "/v1/items",
        ))
        .expect_err("undeclared origin");
    assert_eq!(code_of(&error), BrokerErrorCode::OriginNotDeclared);
    assert!(fixture.transport.recorded_requests().is_empty());
}

#[test]
fn malformed_origin_is_denied_as_undeclared() {
    let fixture = broker(&[json_api_group()]);
    let error = fixture
        .broker
        .execute(BrokerRequest::new(
            "gopher://api.example.test",
            HttpMethod::Get,
            "/v1/items",
        ))
        .expect_err("malformed origin");
    assert_eq!(code_of(&error), BrokerErrorCode::OriginNotDeclared);
}

#[test]
fn undeclared_path_is_denied_with_stable_code() {
    let fixture = broker(&[json_api_group()]);
    let error = fixture
        .broker
        .execute(get_items_request("/v1/admin/secrets"))
        .expect_err("undeclared path");
    assert_eq!(code_of(&error), BrokerErrorCode::PathNotDeclared);
}

#[test]
fn undeclared_method_is_denied_with_stable_code() {
    let fixture = broker(&[json_api_group()]);
    let error = fixture
        .broker
        .execute(BrokerRequest::new(
            ORIGIN,
            HttpMethod::Delete,
            "/v1/items/item-1",
        ))
        .expect_err("undeclared method");
    assert_eq!(code_of(&error), BrokerErrorCode::MethodNotAllowed);
}

#[test]
fn undeclared_header_is_denied_with_stable_code() {
    let fixture = broker(&[json_api_group()]);
    let request = get_items_request("/v1/items/item-1").with_header("x-custom", "value");
    let error = fixture.broker.execute(request).expect_err("bad header");
    assert_eq!(code_of(&error), BrokerErrorCode::HeaderNotAllowed);
    assert!(fixture.transport.recorded_requests().is_empty());
}

#[test]
fn declared_slot_path_and_query_are_admitted() {
    let fixture = broker(&[json_api_group()]);
    fixture
        .transport
        .respond(200, "application/json", r#"{"id":"item-1","name":"ok"}"#);
    let response = fixture
        .broker
        .execute(
            get_items_request("/v1/items/item-1")
                .with_query("expand=name")
                .with_header("X-Request-Id", "req-1"),
        )
        .expect("admitted");
    assert_eq!(response.status(), 200);
    assert_eq!(
        fixture.transport.recorded_requests()[0].url,
        format!("{ORIGIN}/v1/items/item-1?expand=name")
    );
}

#[test]
fn remainder_pattern_covers_nested_paths() {
    let fixture = broker(&[json_api_group()]);
    for path in ["/v1/search/a", "/v1/search/a/b", "/v1/search"] {
        fixture
            .transport
            .respond(200, "application/json", r#"{"id":"s","name":"ok"}"#);
        fixture
            .broker
            .execute(get_items_request(path))
            .unwrap_or_else(|error| panic!("{path} must be admitted: {error}"));
    }
}

#[test]
fn origin_comparison_is_normalized() {
    // Declared uppercase host must match the canonical lowercase request form.
    let mut group = json_api_group();
    group.origins = vec![String::from("HTTPS://API.EXAMPLE.TEST")];
    let fixture = broker(&[group]);
    fixture
        .transport
        .respond(200, "application/json", r#"{"id":"a","name":"b"}"#);
    fixture
        .broker
        .execute(get_items_request("/v1/items"))
        .expect("normalized origin matches");
}

#[test]
fn explicit_default_port_normalizes_away() {
    let mut group = json_api_group();
    group.origins = vec![String::from("https://api.example.test:443")];
    let fixture = broker(&[group]);
    fixture
        .transport
        .respond(200, "application/json", r#"{"id":"a","name":"b"}"#);
    let error = fixture
        .broker
        .execute(get_items_request("/v1/items"))
        .expect_err("explicit default port differs from omitted port");
    assert_eq!(code_of(&error), BrokerErrorCode::OriginNotDeclared);

    fixture.transport.respond(200, "application/json", r#"{"id":"a","name":"b"}"#);
    let request = BrokerRequest::new(
        format!("{ORIGIN}:443"),
        HttpMethod::Get,
        "/v1/items",
    );
    fixture
        .broker
        .execute(request)
        .expect("identical explicit ports match");
}
