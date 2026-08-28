//! Declaration admission validation: closed wire shape, bounded schema keywords, ceiling
//! enforcement, and streaming rules are all rejected before any broker exists.

#![allow(missing_docs, clippy::all, clippy::pedantic, dead_code)]

mod common;

use common::{ORIGIN, json_api_group};
use studio_net::declaration::{CredentialSource, HttpMethod, RouteGroupDeclaration};
use studio_net::error::BrokerErrorCode;
use studio_net::limits::{BrokerLimits, DeclaredLimits};

fn compile(group: &RouteGroupDeclaration) -> Result<(), BrokerErrorCode> {
    group
        .compile(&BrokerLimits::default())
        .map(|_| ())
        .map_err(|error| error.code())
}

#[test]
fn valid_declaration_compiles() {
    assert_eq!(compile(&json_api_group()), Ok(()));
}

#[test]
fn declarations_reject_unknown_wire_fields() {
    let raw = serde_json::json!({
    "id": "group",
    "origins": ["https://api.example.test"],
    "methods": ["GET"],
    "paths": ["/v1/items"],
    "credential": {"source": "public"},
    "surpriseField": true,
    });
    assert!(serde_json::from_value::<RouteGroupDeclaration>(raw).is_err());
}

#[test]
fn declarations_parse_closed_credential_sources() {
    let raw = serde_json::json!({
    "id": "group",
    "origins": ["https://api.example.test"],
    "methods": ["GET"],
    "paths": ["/v1/private"],
    "credential": {
    "source": "namedSecret",
    "name": "payments.key",
    "header": "authorization",
    "prefix": "Bearer ",
    },
    });
    let parsed: RouteGroupDeclaration = serde_json::from_value(raw).expect("named secret parses");
    assert_eq!(
        parsed.credential,
        CredentialSource::NamedSecret {
            name: String::from("payments.key"),
            header: String::from("authorization"),
            prefix: Some(String::from("Bearer ")),
        }
    );
}

#[test]
fn unknown_schema_keywords_are_rejected_at_compile() {
    let mut group = json_api_group();
    group.response_schema = Some(serde_json::json!({
    "type": "object",
    "properties": {"id": {"type": "string"}},
    "required": ["id"],
    "patternProperties": {".*": {}},
    }));
    assert_eq!(compile(&group), Err(BrokerErrorCode::DeclarationInvalid));
}

#[test]
fn required_entries_must_reference_declared_properties() {
    let mut group = json_api_group();
    group.response_schema = Some(serde_json::json!({
    "type": "object",
    "properties": {"id": {"type": "string"}},
    "required": ["ghost"],
    }));
    assert_eq!(compile(&group), Err(BrokerErrorCode::DeclarationInvalid));
}

#[test]
fn declared_limits_above_host_ceilings_are_rejected() {
    let mut group = json_api_group();
    group.limits = DeclaredLimits {
        max_response_bytes: Some(BrokerLimits::default().max_response_bytes + 1),
        ..DeclaredLimits::default()
    };
    assert_eq!(compile(&group), Err(BrokerErrorCode::DeclarationInvalid));
}

#[test]
fn streaming_routes_allow_bounded_get_or_post() {
    let mut group = json_api_group();
    group.methods = vec![HttpMethod::Post];
    group.response_schema = None;
    group.streaming = Some(common_sse());
    assert_eq!(compile(&group), Ok(()));

    group.methods = vec![HttpMethod::Put];
    assert_eq!(compile(&group), Err(BrokerErrorCode::DeclarationInvalid));
}

#[test]
fn streaming_conflicts_with_response_schema() {
    let mut group = json_api_group();
    group.methods = vec![HttpMethod::Get];
    group.streaming = Some(common_sse());
    // response_schema still present from the fixture.
    assert_eq!(compile(&group), Err(BrokerErrorCode::DeclarationInvalid));
}

#[test]
fn duplicate_origins_and_invalid_ids_are_rejected() {
    let mut group = json_api_group();
    group.origins = vec![ORIGIN.to_owned(), ORIGIN.to_owned()];
    assert_eq!(compile(&group), Err(BrokerErrorCode::DeclarationInvalid));

    group.origins = vec![ORIGIN.to_owned()];
    group.id = String::from("Bad-Id");
    assert_eq!(compile(&group), Err(BrokerErrorCode::DeclarationInvalid));
}

fn common_sse() -> studio_net::declaration::StreamingDeclaration {
    studio_net::declaration::StreamingDeclaration {
        chunk_schema: serde_json::json!({
        "type": "object",
        "properties": {"text": {"type": "string"}},
        "required": ["text"],
        }),
        reconnects: Some(1),
        retry_base_delay_ms: Some(50),
    }
}
