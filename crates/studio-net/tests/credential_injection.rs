//! Credential injection path with a deterministic fake backend: values move only into the
//! outgoing send, never into guest-visible responses or diagnostic surfaces.

#![allow(missing_docs, clippy::all, clippy::pedantic, dead_code)]

mod common;

use std::sync::Arc;

use common::{ORIGIN, broker, code_of, named_secret_group, secret_fixture};
use studio_net::declaration::HttpMethod;
use studio_net::error::BrokerErrorCode;
use studio_net::guest::BrokerRequest;
use studio_security::ProtectedSecretKey;

const SECRET_VALUE: &str = "sk_live_51abcdefghijabcdefghij";

#[test]
fn named_secret_is_injected_at_send_time_with_prefix() {
    let secret = secret_fixture("payments.key", SECRET_VALUE);
    let fixture = broker(&[named_secret_group("authorization", Some("Bearer "))]);
    let handle = secret.injection_handle(&[secret.key.clone()]);
    fixture.broker.set_named_secret_injector(Arc::new(handle));
    fixture
        .transport
        .respond(200, "application/json", r#"{"ok":true}"#);
    let response = fixture
        .broker
        .execute(BrokerRequest::new(ORIGIN, HttpMethod::Get, "/v1/private"))
        .expect("injected request succeeds");
    assert_eq!(response.body()["ok"], true);
    let request = &fixture.transport.recorded_requests()[0];
    assert!(
        request
            .headers
            .iter()
            .any(|(name, value)| name == "authorization"
                && value == &format!("Bearer {SECRET_VALUE}")),
        "credential header must be attached at send time"
    );
}

#[test]
fn credential_echoing_response_is_rejected_before_guest_visibility() {
    let secret = secret_fixture("payments.key", SECRET_VALUE);
    let fixture = broker(&[named_secret_group("x-api-key", None)]);
    let handle = secret.injection_handle(&[secret.key.clone()]);
    fixture.broker.set_named_secret_injector(Arc::new(handle));
    // Valid JSON that echoes the injected credential back: the whole response is discarded so
    // no guest ever observes registered credential material.
    fixture.transport.respond(
        200,
        "text/plain",
        &format!(r#"{{"echo":"{SECRET_VALUE}"}}"#),
    );
    let error = fixture
        .broker
        .execute(BrokerRequest::new(ORIGIN, HttpMethod::Get, "/v1/private"))
        .expect_err("echoed credential");
    assert_eq!(code_of(&error), BrokerErrorCode::SensitiveContentRejected);
    if let Some(detail) = error.detail() {
        assert!(!detail.contains(SECRET_VALUE));
    }
}

#[test]
fn diagnostics_after_injection_are_scrubbed_of_registered_values() {
    let secret = secret_fixture("payments.key", SECRET_VALUE);
    let fixture = broker(&[named_secret_group("x-api-key", None)]);
    let handle = secret.injection_handle(&[secret.key.clone()]);
    fixture.broker.set_named_secret_injector(Arc::new(handle));
    fixture
        .transport
        .respond(200, "application/json", "not json at all");
    let error = fixture
        .broker
        .execute(BrokerRequest::new(ORIGIN, HttpMethod::Get, "/v1/private"))
        .expect_err("malformed response");
    if let Some(detail) = error.detail() {
        assert!(!detail.contains(SECRET_VALUE), "detail leaked credential");
    }
}

#[test]
fn missing_configured_value_fails_closed_with_safe_code() {
    let group = named_secret_group("authorization", None);
    // Declare the key but configure nothing in the partition bound to the handle.
    let declared_key = ProtectedSecretKey::new(
        "payments.key",
        "REST broker credential for route group secret-api",
    )
    .expect("key");
    let empty_partition = secret_fixture("unused.name", "unused-value");
    let handle =
        empty_partition.injection_handle(&[declared_key.clone(), empty_partition.key.clone()]);
    let fixture = broker(&[group]);
    fixture.broker.set_named_secret_injector(Arc::new(handle));
    let error = fixture
        .broker
        .execute(BrokerRequest::new(ORIGIN, HttpMethod::Get, "/v1/private"))
        .expect_err("missing value");
    assert_eq!(code_of(&error), BrokerErrorCode::CredentialUnavailable);
}

#[test]
fn undeclared_name_is_rejected_by_the_injection_handle() {
    // The broker's group references `payments.key`, but the bound handle only declares another
    // name; injection must reject without leaking whether the value exists.
    let other = secret_fixture("other.name", SECRET_VALUE);
    let fixture = broker(&[named_secret_group("authorization", None)]);
    let handle = other.injection_handle(&[other.key.clone()]);
    fixture.broker.set_named_secret_injector(Arc::new(handle));
    let error = fixture
        .broker
        .execute(BrokerRequest::new(ORIGIN, HttpMethod::Get, "/v1/private"))
        .expect_err("undeclared reference");
    assert_eq!(code_of(&error), BrokerErrorCode::DeclarationInvalid);
}

#[test]
fn revoked_named_secret_fails_closed() {
    let secret = secret_fixture("payments.key", SECRET_VALUE);
    let fixture = broker(&[named_secret_group("authorization", None)]);
    // Revoke through the host configuration surface.
    use studio_security::{ApplicationEnvironment, PluginPrincipal, TrustMode};
    let principal = PluginPrincipal::new_verified(
        "publisher.example",
        "signing-key-1",
        "com.example.app",
        [1_u8; 32],
        [2_u8; 16],
        TrustMode::Production,
    )
    .expect("principal");
    let application = secret
        .store
        .for_application(&principal, ApplicationEnvironment::Development)
        .expect("partition");
    application.revoke(&secret.key).expect("revoke");
    let handle = secret.injection_handle(&[secret.key.clone()]);
    fixture.broker.set_named_secret_injector(Arc::new(handle));
    let error = fixture
        .broker
        .execute(BrokerRequest::new(ORIGIN, HttpMethod::Get, "/v1/private"))
        .expect_err("revoked value");
    assert_eq!(code_of(&error), BrokerErrorCode::CredentialUnavailable);
}
