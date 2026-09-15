//! Limits enforcement: sliding-window rates, request bounds/schema, timeout mapping, and the
//! OAuth session seam's fail-closed default.

#![allow(missing_docs, clippy::all, clippy::pedantic, dead_code)]

mod common;

use std::sync::Arc;

use common::{ORIGIN, broker, broker_with_limits, code_of, get_items_request, json_api_group};
use serde_json::json;
use studio_net::declaration::{CredentialSource, HttpMethod, RouteGroupDeclaration};
use studio_net::error::BrokerErrorCode;
use studio_net::guest::BrokerRequest;
use studio_net::limits::{BrokerLimits, DeclaredLimits};
use studio_net::transport::TransportError;

#[test]
fn rate_window_exhaustion_is_rate_limited() {
    let mut group = json_api_group();
    group.limits.max_requests_per_window = Some(2);
    let fixture = broker_with_limits(&[group], BrokerLimits::default());
    for _ in 0..2 {
        fixture
            .transport
            .respond(200, "application/json", r#"{"id":"a","name":"b"}"#);
        fixture
            .broker
            .execute(get_items_request("/v1/items"))
            .expect("within allowance");
    }
    let error = fixture
        .broker
        .execute(get_items_request("/v1/items"))
        .expect_err("allowance exhausted");
    assert_eq!(code_of(&error), BrokerErrorCode::RateLimited);
}

#[test]
fn oversized_request_body_is_rejected_before_send() {
    let mut group = common::post_items_group();
    group.limits.max_request_bytes = Some(16);
    let fixture = broker_with_limits(&[group], BrokerLimits::default());
    let error = fixture
        .broker
        .execute(
            BrokerRequest::new(ORIGIN, HttpMethod::Post, "/v1/items")
                .with_body(json!({"name": "a name far longer than sixteen bytes"})),
        )
        .expect_err("oversized body");
    assert_eq!(code_of(&error), BrokerErrorCode::RequestTooLarge);
    assert!(fixture.transport.recorded_requests().is_empty());
}

#[test]
fn request_body_violating_declared_schema_is_rejected() {
    let fixture = broker(&[common::post_items_group()]);
    let error = fixture
        .broker
        .execute(
            BrokerRequest::new(ORIGIN, HttpMethod::Post, "/v1/items")
                .with_body(json!({"wrong": true})),
        )
        .expect_err("missing required property");
    assert_eq!(code_of(&error), BrokerErrorCode::RequestSchemaInvalid);

    let error = fixture
        .broker
        .execute(BrokerRequest::new(ORIGIN, HttpMethod::Post, "/v1/items"))
        .expect_err("schema declared but no body");
    assert_eq!(code_of(&error), BrokerErrorCode::RequestSchemaInvalid);
}

#[test]
fn valid_request_body_passes_and_rides_the_send() {
    let fixture = broker(&[common::post_items_group()]);
    fixture
        .transport
        .respond(201, "application/json", r#"{"id":"a","name":"b"}"#);
    let response = fixture
        .broker
        .execute(
            BrokerRequest::new(ORIGIN, HttpMethod::Post, "/v1/items")
                .with_body(json!({"name": "widget"})),
        )
        .expect("valid body admitted");
    assert_eq!(response.status(), 201);
    let request = &fixture.transport.recorded_requests()[0];
    assert!(request.body.is_some());
    assert!(
        request
            .headers
            .iter()
            .any(|(name, value)| name == "content-type" && value == "application/json")
    );
}

#[test]
fn transport_timeout_maps_to_stable_code() {
    let fixture = broker(&[json_api_group()]);
    fixture.transport.fail_exchange(TransportError::TimedOut);
    let error = fixture
        .broker
        .execute(get_items_request("/v1/items"))
        .expect_err("timeout");
    assert_eq!(code_of(&error), BrokerErrorCode::Timeout);
}

#[test]
fn transport_failure_maps_to_stable_code() {
    let fixture = broker(&[json_api_group()]);
    fixture
        .transport
        .fail_exchange(TransportError::ConnectionFailure);
    let error = fixture
        .broker
        .execute(get_items_request("/v1/items"))
        .expect_err("connection failure");
    assert_eq!(code_of(&error), BrokerErrorCode::TransportFailure);
}

fn provider_session_group() -> RouteGroupDeclaration {
    RouteGroupDeclaration {
        id: String::from("provider-api"),
        origins: vec![ORIGIN.to_owned()],
        methods: vec![HttpMethod::Get],
        paths: vec![String::from("/v1/me")],
        allowed_headers: Vec::new(),
        credential: CredentialSource::OauthProviderSession {
            provider: String::from("github"),
        },
        request_schema: None,
        response_schema: None,
        streaming: None,
        limits: DeclaredLimits::default(),
    }
}

#[test]
fn oauth_session_without_resolver_fails_closed() {
    let fixture = broker(&[provider_session_group()]);
    let error = fixture
        .broker
        .execute(BrokerRequest::new(ORIGIN, HttpMethod::Get, "/v1/me"))
        .expect_err("no resolver wired");
    assert_eq!(code_of(&error), BrokerErrorCode::OauthSessionUnavailable);
    assert!(fixture.transport.recorded_requests().is_empty());
}

struct StubResolver;

impl studio_net::credential::OAuthSessionResolver for StubResolver {
    fn inject_session(
        &self,
        _provider: &str,
        _route_group_id: &str,
        sink: &mut dyn studio_security::BrokerCredentialSink,
    ) -> Result<(), studio_net::error::BrokerError> {
        sink.inject(b"session-token-value")
            .map_err(|_| studio_net::error::BrokerError::new(BrokerErrorCode::InjectionRejected))
    }
}

#[test]
fn wired_oauth_resolver_injects_at_send_time() {
    let fixture = broker(&[provider_session_group()]);
    fixture.broker.set_oauth_resolver(Arc::new(StubResolver));
    fixture
        .transport
        .respond(200, "application/json", r#"{"login":"octocat"}"#);
    let response = fixture
        .broker
        .execute(BrokerRequest::new(ORIGIN, HttpMethod::Get, "/v1/me"))
        .expect("resolved session");
    assert_eq!(response.body()["login"], "octocat");
    let request = &fixture.transport.recorded_requests()[0];
    assert!(
        request
            .headers
            .iter()
            .any(|(name, value)| name == "authorization" && value.contains("session-token-value")),
        "session credential must ride the send"
    );
}
