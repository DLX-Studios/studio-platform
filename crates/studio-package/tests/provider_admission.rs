#![allow(missing_docs)]
#![allow(clippy::default_trait_access)]

use serde_json::json;
use studio_net::declaration::{
    CredentialSource, HttpMethod, RouteGroupDeclaration, StreamingDeclaration,
};
use studio_package::{
    BundleLimits, Capability, IntegrationReference, ManifestV1, ProviderAdmissionErrorCode,
    ProviderRegistry, Publisher, SecretDeclaration,
};

fn manifest(
    integrations: Vec<IntegrationReference>,
    routes: Vec<RouteGroupDeclaration>,
) -> ManifestV1 {
    ManifestV1 {
        schema_version: 1,
        id: "com.example.provider-admission".to_owned(),
        name: "Provider admission fixture".to_owned(),
        version: "0.1.0".to_owned(),
        publisher: Publisher {
            id: "example".to_owned(),
            key_id: "fixture".to_owned(),
        },
        entry: "module.wasm".to_owned(),
        sdk_version: "^0.1.0".to_owned(),
        protocol_version: 1,
        capabilities: vec![Capability::DataSurrealQuery],
        limits: BundleLimits {
            memory_mib: 16,
            event_fuel: 1_000_000,
        },
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
        credential: CredentialSource::OauthProviderSession {
            provider: "github".to_owned(),
        },
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
        methods: vec![if streaming {
            HttpMethod::Get
        } else {
            HttpMethod::Post
        }],
        paths: vec![
            if streaming {
                "/v1/chat/completions/stream"
            } else {
                "/v1/chat/completions"
            }
            .to_owned(),
        ],
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
            version: "1.1.0".to_owned(),
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
    assert_eq!(plan.providers()[0].version, "1.1.0");
    assert_eq!(plan.providers()[0].issuer, None);
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
    assert_eq!(
        plan.providers()[0].secrets,
        vec!["openai.api_key".to_owned()]
    );
    assert_eq!(plan.route_groups().len(), 2);
}

#[test]
fn unknown_revoked_outdated_and_incompatible_descriptors_fail_closed() {
    let package = manifest(
        vec![IntegrationReference {
            id: "github".to_owned(),
            version: "1.1.0".to_owned(),
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
    assert_eq!(
        outdated.code(),
        ProviderAdmissionErrorCode::OutdatedProvider
    );

    let mut revoked_registry = registry.clone();
    revoked_registry
        .set_state(
            "github",
            "1.1.0",
            studio_package::ProviderDescriptorState::Revoked,
        )
        .expect("fixture descriptor exists");
    assert_eq!(
        revoked_registry
            .admit(&package, &Default::default())
            .unwrap_err()
            .code(),
        ProviderAdmissionErrorCode::RevokedProvider
    );

    let mut incompatible_registry = ProviderRegistry::maintained();
    incompatible_registry
        .set_state(
            "github",
            "1.1.0",
            studio_package::ProviderDescriptorState::Incompatible,
        )
        .expect("fixture descriptor exists");
    assert_eq!(
        incompatible_registry
            .admit(&package, &Default::default())
            .unwrap_err()
            .code(),
        ProviderAdmissionErrorCode::IncompatibleProvider
    );
}

#[test]
fn route_and_secret_rejections_are_value_free() {
    let mut package = manifest(
        vec![IntegrationReference {
            id: "github".to_owned(),
            version: "1.1.0".to_owned(),
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

fn declared_issuer_integration(config: serde_json::Value) -> IntegrationReference {
    IntegrationReference {
        id: "oidc-acme".to_owned(),
        version: "1.0.0".to_owned(),
        config: Some(config),
    }
}

#[test]
fn declared_issuer_admits_with_its_own_issuer_and_standard_scopes() {
    let package = manifest(
        vec![declared_issuer_integration(json!({
            "issuer": "https://acme.example.com",
            "clientId": "acme-client",
            "scopes": ["openid", "profile"]
        }))],
        Vec::new(),
    );
    let plan = ProviderRegistry::maintained()
        .admit(&package, &Default::default())
        .expect("a declared issuer admits with the issuer it declared");
    let provider = &plan.providers()[0];
    assert_eq!(provider.id, "oidc-acme");
    assert_eq!(provider.version, "1.0.0");
    assert_eq!(provider.issuer.as_deref(), Some("https://acme.example.com"));
    assert_eq!(
        provider.scopes,
        vec!["openid".to_owned(), "profile".to_owned()]
    );
    assert!(provider.secrets.is_empty());
    assert!(plan.route_groups().is_empty());
}

#[test]
fn declared_issuer_scope_outside_the_standard_ceiling_is_refused() {
    let package = manifest(
        vec![declared_issuer_integration(json!({
            "issuer": "https://acme.example.com",
            "clientId": "acme-client",
            "scopes": ["openid", "acme.api.read"]
        }))],
        Vec::new(),
    );
    let error = ProviderRegistry::maintained()
        .admit(&package, &Default::default())
        .expect_err("a scope outside the host ceiling must fail closed");
    assert_eq!(error.code(), ProviderAdmissionErrorCode::ScopeNotAllowed);
}

#[test]
fn declared_issuer_with_a_non_https_issuer_is_refused() {
    let package = manifest(
        vec![declared_issuer_integration(json!({
            "issuer": "http://acme.example.com",
            "clientId": "acme-client"
        }))],
        Vec::new(),
    );
    let error = ProviderRegistry::maintained()
        .admit(&package, &Default::default())
        .expect_err("an issuer that is not an HTTPS URL must fail closed");
    assert_eq!(error.code(), ProviderAdmissionErrorCode::IntegrationInvalid);
}

#[test]
fn declared_issuer_without_a_client_id_is_refused() {
    let package = manifest(
        vec![declared_issuer_integration(json!({
            "issuer": "https://acme.example.com"
        }))],
        Vec::new(),
    );
    let error = ProviderRegistry::maintained()
        .admit(&package, &Default::default())
        .expect_err("a declared issuer without a client id must fail closed");
    assert_eq!(error.code(), ProviderAdmissionErrorCode::IntegrationInvalid);
}

#[test]
fn declared_issuer_naming_a_secret_is_refused() {
    let package = manifest(
        vec![declared_issuer_integration(json!({
            "issuer": "https://acme.example.com",
            "clientId": "acme-client",
            "clientSecretName": "acme.oauth.client_secret"
        }))],
        Vec::new(),
    );
    let error = ProviderRegistry::maintained()
        .admit(&package, &Default::default())
        .expect_err("a declared issuer carries no secret");
    assert_eq!(error.code(), ProviderAdmissionErrorCode::SecretNotAllowed);
}

#[test]
fn declared_issuer_id_may_not_shadow_a_maintained_provider() {
    let package = manifest(
        vec![IntegrationReference {
            id: "oidc-google".to_owned(),
            version: "1.0.0".to_owned(),
            config: Some(json!({
                "issuer": "https://acme.example.com",
                "clientId": "acme-client"
            })),
        }],
        Vec::new(),
    );
    let error = ProviderRegistry::maintained()
        .admit(&package, &Default::default())
        .expect_err("a declared issuer may not take a maintained provider name");
    assert_eq!(error.code(), ProviderAdmissionErrorCode::IntegrationInvalid);
}

#[test]
fn declared_issuer_with_an_uninstalled_version_is_refused() {
    let package = manifest(
        vec![IntegrationReference {
            id: "oidc-acme".to_owned(),
            version: "2.0.0".to_owned(),
            config: Some(json!({
                "issuer": "https://acme.example.com",
                "clientId": "acme-client"
            })),
        }],
        Vec::new(),
    );
    let error = ProviderRegistry::maintained()
        .admit(&package, &Default::default())
        .expect_err("a declared issuer has one descriptor version");
    assert_eq!(error.code(), ProviderAdmissionErrorCode::OutdatedProvider);
}

#[test]
fn declared_issuer_grants_no_route_group() {
    let package = manifest(
        vec![declared_issuer_integration(json!({
            "issuer": "https://acme.example.com",
            "clientId": "acme-client"
        }))],
        vec![RouteGroupDeclaration {
            id: "oidc-acme.user".to_owned(),
            origins: vec!["https://acme.example.com".to_owned()],
            methods: vec![HttpMethod::Get],
            paths: vec!["/user".to_owned()],
            allowed_headers: vec!["accept".to_owned()],
            credential: CredentialSource::OauthProviderSession {
                provider: "oidc-acme".to_owned(),
            },
            request_schema: None,
            response_schema: None,
            streaming: None,
            limits: Default::default(),
        }],
    );
    let error = ProviderRegistry::maintained()
        .admit(&package, &Default::default())
        .expect_err("a declared issuer grants no route group");
    assert_eq!(error.code(), ProviderAdmissionErrorCode::RouteNotAllowed);
}

#[test]
fn maintained_provider_admits_without_a_declared_issuer() {
    let mut package = manifest(
        vec![IntegrationReference {
            id: "google".to_owned(),
            version: "1.0.0".to_owned(),
            config: Some(json!({
                "clientId": "fixture-client",
                "clientSecretName": "google.oauth.client_secret",
                "scopes": ["openid", "email", "profile"]
            })),
        }],
        Vec::new(),
    );
    package.secrets = vec![SecretDeclaration {
        name: "google.oauth.client_secret".to_owned(),
        purpose: "Google OAuth client configuration".to_owned(),
    }];
    let plan = ProviderRegistry::maintained()
        .admit(&package, &Default::default())
        .expect("a maintained first-party provider still admits");
    let provider = &plan.providers()[0];
    assert_eq!(provider.id, "google");
    assert_eq!(provider.version, "1.0.0");
    assert_eq!(provider.issuer, None);
    assert_eq!(
        provider.scopes,
        vec![
            "openid".to_owned(),
            "email".to_owned(),
            "profile".to_owned()
        ]
    );
}
