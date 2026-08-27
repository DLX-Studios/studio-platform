#![allow(missing_docs)]

use serde_json::json;
use studio_net::declaration::{
    CredentialSource, HttpMethod, RouteGroupDeclaration, StreamingDeclaration,
};
use studio_package::{
    BundleLimits, Capability, IntegrationReference, ManifestV1, ProviderAdmissionErrorCode,
    ProviderRegistry, Publisher, SecretDeclaration,
};

fn manifest(integrations: Vec<IntegrationReference>, routes: Vec<RouteGroupDeclaration>) -> ManifestV1 {
    ManifestV1 {
        schema_version: 1,
        id: "com.example.provider-admission".to_owned(),
        name: "Provider admission fixture".to_owned(),
        version: "0.1.0".to_owned(),
        publisher: Publisher { id: "example".to_owned(), key_id: "fixture".to_owned() },
        entry: "module.wasm".to_owned(),
        sdk_version: "^0.1.0".to_owned(),
        protocol_version: 1,
        capabilities: vec![Capability::DataSurrealQuery],
        limits: BundleLimits { memory_mib: 16, event_fuel: 1_000_000 },
        assets: Vec::new(),
        secrets: vec![SecretDeclaration {
            name: "github.oauth.client_secret".to_owned(),
            purpose: "GitHub OAuth client configuration".to_owned(),
        }],
        integrations,
        routes,
        migrations: Vec::new(),
    }
}

fn github_route(id: &str, path: &str) -> RouteGroupDeclaration {
    RouteGroupDeclaration {
        id: id.to_owned(),
        origins: vec!["https://api.github.com".to_owned()],
        methods: vec![HttpMethod::Get],
        paths: vec![path.to_owned()],
        allowed_headers: vec![
            "accept".to_owned(),
            "x-github-api-version".to_owned(),
            "user-agent".to_owned(),
        ],
        credential: CredentialSource::OauthProviderSession { provider: "github".to_owned() },
        request_schema: None,
        response_schema: None,
        streaming: None,
        limits: Default::default(),
    }
}

fn ai_route(streaming: bool) -> RouteGroupDeclaration {
    RouteGroupDeclaration {
        id: if streaming {
            "ai.chat.completions.stream"
        } else {
            "ai.chat.completions"
        }
        .to_owned(),
        origins: vec!["https://api.openai.com".to_owned()],
        methods: vec![if streaming { HttpMethod::Get } else { HttpMethod::Post }],
        paths: vec![if streaming {
            "/v1/chat/completions/stream"
        } else {
            "/v1/chat/completions"
        }
        .to_owned()],
        allowed_headers: if streaming {
            vec!["accept".to_owned()]
        } else {
            vec!["content-type".to_owned(), "accept".to_owned()]
        },
        credential: CredentialSource::NamedSecret {
            name: "openai.api_key".to_owned(),
            header: "authorization".to_owned(),
            prefix: Some("Bearer ".to_owned()),
        },
        request_schema: None,
        response_schema: None,
        streaming: streaming.then_some(StreamingDeclaration {
            chunk_schema: json!({"type": "object"}),
            reconnects: Some(2),
            retry_base_delay_ms: Some(250),
        }),
        limits: Default::default(),
    }
}

#[test]
fn github_fixture_resolves_exact_routes_and_capabilities() {
    let package = manifest(
        vec![IntegrationReference {
            id: "github".to_owned(),
            version: "1.0.0".to_owned(),
            config: Some(json!({
                "clientId": "fixture-client",
                "clientSecretName": "github.oauth.client_secret",
                "scopes": ["read:user", "user:email"]
            })),
        }],
        vec![github_route("github.user", "/user")],
    );
    let plan = ProviderRegistry::maintained()
        .admit(&package, &Default::default())
        .expect("maintained GitHub provider admits fixture");
    assert_eq!(plan.providers()[0].id, "github");
    assert_eq!(plan.providers()[0].version, "1.0.0");
    assert_eq!(plan.capabilities(), &[Capability::DataSurrealQuery]);
    assert_eq!(plan.route_groups().len(), 1);
}

#[test]
fn ai_fixture_resolves_dynamic_origin_and_secret_route_policy() {
    let mut package = manifest(
        vec![IntegrationReference {
            id: "ai".to_owned(),
            version: "1.0.0".to_owned(),
            config: Some(json!({
                "origin": "https://api.openai.com",
                "apiKeyName": "openai.api_key"
            })),
        }],
        vec![ai_route(false), ai_route(true)],
    );
    package.secrets = vec![SecretDeclaration {
        name: "openai.api_key".to_owned(),
        purpose: "Authenticate AI requests".to_owned(),
    }];
    let plan = ProviderRegistry::maintained()
        .admit(&package, &Default::default())
        .expect("maintained AI provider admits fixture");
    assert_eq!(plan.providers()[0].id, "ai");
    assert_eq!(plan.providers()[0].secrets, vec!["openai.api_key".to_owned()]);
    assert_eq!(plan.route_groups().len(), 2);
}

#[test]
fn unknown_revoked_outdated_and_incompatible_descriptors_fail_closed() {
    let package = manifest(
        vec![IntegrationReference {
            id: "github".to_owned(),
            version: "1.0.0".to_owned(),
            config: Some(json!({
                "clientId": "fixture-client",
                "clientSecretName": "github.oauth.client_secret"
            })),
        }],
        Vec::new(),
    );
    let registry = ProviderRegistry::maintained();
    let unknown = registry
        .resolve("not-installed", "1.0.0")
        .expect_err("unknown provider must fail closed");
    assert_eq!(unknown.code(), ProviderAdmissionErrorCode::UnknownProvider);
    let outdated = registry
        .resolve("github", "9.0.0")
        .expect_err("unknown version must fail closed");
    assert_eq!(outdated.code(), ProviderAdmissionErrorCode::OutdatedProvider);

    let mut revoked_registry = registry.clone();
    revoked_registry
        .set_state("github", "1.0.0", studio_package::ProviderDescriptorState::Revoked)
        .expect("fixture descriptor exists");
    assert_eq!(revoked_registry.admit(&package, &Default::default()).unwrap_err().code(), ProviderAdmissionErrorCode::RevokedProvider);

    let mut incompatible_registry = ProviderRegistry::maintained();
    incompatible_registry
        .set_state("github", "1.0.0", studio_package::ProviderDescriptorState::Incompatible)
        .expect("fixture descriptor exists");
    assert_eq!(incompatible_registry.admit(&package, &Default::default()).unwrap_err().code(), ProviderAdmissionErrorCode::IncompatibleProvider);
}

#[test]
fn route_and_secret_rejections_are_value_free() {
    let mut package = manifest(
        vec![IntegrationReference {
            id: "github".to_owned(),
            version: "1.0.0".to_owned(),
            config: Some(json!({
                "clientId": "fixture-client",
                "clientSecretName": "github.oauth.client_secret"
            })),
        }],
        vec![github_route("github.user", "/not-allowed")],
    );
    package.secrets[0].name = "super-secret-value".to_owned();
    let error = ProviderRegistry::maintained()
        .admit(&package, &Default::default())
        .expect_err("route and secret mismatch must fail closed");
    assert_eq!(error.code(), ProviderAdmissionErrorCode::RouteNotAllowed);
    assert!(!error.to_string().contains("super-secret-value"));
}
