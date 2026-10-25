#![allow(missing_docs)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::format_collect)]

use parking_lot::Mutex;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use studio_github::route_groups as github_route_groups;
use studio_net::declaration::HttpMethod;
use studio_net::{HttpsClient, IncomingResponse, OutgoingRequest, TransportError, TransportLimits};
use studio_oauth::{BrowserHandoff, Callback, CallbackListener, CallbackReceiver, OsEntropy};
use studio_security::{
    CredentialBackend, CredentialBackendError, CredentialBytes, CredentialLocator,
    ProtectedSecretKey, SecretInput,
};

use ed25519_dalek::{Signer, SigningKey};
use serde_json::json;
use studio_app::{
    cli::{LaunchMode, LaunchRequest},
    host::{HostConfig, LaunchErrorCode, StudioHost, WaylandAvailability},
    plugin_surface::DEVELOPMENT_WARNING,
};
use studio_components::InputAction;
use studio_package::{
    ArchiveFiles, ArchivePolicy, CanonicalBundleInput, TrustStore, TrustedPublisherKey,
    build_archive, canonical_bundle_document,
};
use studio_protocol::{GuestMessage, HostEvent, MountTree, NodeKind, UiNode};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct FixtureDirectory(PathBuf);

impl FixtureDirectory {
    fn new() -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "studio-plugin-launch-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for FixtureDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn mount_message() -> Vec<u8> {
    let node = |id: &str, kind: NodeKind, label: Option<&str>| UiNode {
        id: id.to_owned(),
        kind,
        props: label
            .map(|label| BTreeMap::from([("label".to_owned(), json!(label))]))
            .unwrap_or_default(),
        children: Vec::new(),
    };
    serde_json::to_vec(&GuestMessage::Mount(MountTree {
        protocol_version: 1,
        route: "/catalog".to_owned(),
        root: UiNode {
            id: "root".to_owned(),
            kind: NodeKind::Column,
            props: BTreeMap::new(),
            children: vec![node("checkout", NodeKind::Button, Some("Checkout"))],
        },
    }))
    .unwrap()
}

fn plugin_module() -> Vec<u8> {
    let mount = String::from_utf8(mount_message()).unwrap();
    let encoded_mount = serde_json::to_string(&mount).unwrap();
    wat::parse_str(format!(
        r#"(module
          (import "studio_host" "emit" (func $emit (param i32 i32) (result i32)))
          (memory (export "memory") 1 1)
          (table 1 1 funcref)
          (data (i32.const 0) {encoded_mount})
          (func (export "studio_alloc") (param i32) (result i32) i32.const 4096)
          (func (export "studio_dealloc") (param i32 i32))
          (func (export "studio_init") (param i32 i32) (result i32)
            i32.const 0 i32.const {mount_length} call $emit)
          (func (export "studio_event") (param i32 i32) (result i32) i32.const 0))"#,
        mount_length = mount.len(),
    ))
    .unwrap()
}

fn github_manifest() -> serde_json::Value {
    let mut manifest = manifest();
    manifest["id"] = json!("com.example.github-viewer");
    manifest["name"] = json!("Example GitHub Viewer");
    manifest["secrets"] = json!([
        {
            "name": "github.oauth.client_secret",
            "purpose": "GitHub OAuth client configuration"
        }
    ]);
    manifest["integrations"] = json!([
        {
            "id": "github",
            "version": "1.1.0",
            "config": {
                "clientId": "fixture-client",
                "clientSecretName": "github.oauth.client_secret",
                "scopes": ["read:user", "user:email"]
            }
        }
    ]);
    manifest["routes"] = serde_json::to_value(github_route_groups()).unwrap();
    manifest
}

fn discord_manifest() -> serde_json::Value {
    let mut manifest = manifest();
    manifest["id"] = json!("com.example.discord-viewer");
    manifest["name"] = json!("Example Discord Viewer");
    manifest["secrets"] = json!([
        {
            "name": "discord.oauth.client_secret",
            "purpose": "Discord OAuth client configuration"
        }
    ]);
    manifest["integrations"] = json!([
        {
            "id": "discord",
            "version": "1.0.0",
            "config": {
                "clientId": "fixture-client",
                "clientSecretName": "discord.oauth.client_secret",
                "scopes": ["email", "identify"]
            }
        }
    ]);
    manifest
}

/// A manifest that declares one OpenID Connect issuer of its own.
///
/// The application is a public client of the issuer: it holds a client id and no secret, and it
/// asks only for the OpenID Connect standard scopes.
fn declared_issuer_manifest() -> serde_json::Value {
    let mut manifest = manifest();
    manifest["id"] = json!("com.example.acme-viewer");
    manifest["name"] = json!("Example Acme Viewer");
    manifest["integrations"] = json!([
        {
            "id": "oidc-acme",
            "version": "1.0.0",
            "config": {
                "issuer": "https://acme.example.com",
                "clientId": "fixture-client",
                "scopes": ["openid", "profile"]
            }
        }
    ]);
    manifest
}

fn provider_action_module(capability: &str, provider: &str, label: &str) -> Vec<u8> {
    let text_node = |id: &str, text: &str| UiNode {
        id: id.to_owned(),
        kind: NodeKind::Text,
        props: BTreeMap::from([("text".to_owned(), json!(text))]),
        children: Vec::new(),
    };
    let sign_in_button = UiNode {
        id: String::from("signin"),
        kind: NodeKind::Button,
        props: BTreeMap::from([
            ("label".to_owned(), json!(label)),
            ("enabled".to_owned(), json!(true)),
        ]),
        children: Vec::new(),
    };
    let mount = GuestMessage::Mount(MountTree {
        protocol_version: 1,
        route: format!("/{provider}"),
        root: UiNode {
            id: String::from("root"),
            kind: NodeKind::Column,
            props: BTreeMap::new(),
            children: vec![
                sign_in_button,
                text_node("status", &format!("Sign in with {label}")),
                text_node("hint", "Signed-in claims are delivered to the guest."),
            ],
        },
    });
    let action = GuestMessage::Action(studio_protocol::ActionRequest {
        request_id: format!("{provider}-sign-in"),
        capability: capability.to_owned(),
        operation: String::from("sign_in"),
        payload: json!({"provider": provider}),
    });
    let patch = json!({
        "type": "patch",
        "payload": {
            "sequence": 1,
            "operations": [
                {"op":"update_prop","node_id":"status","property":"text","value":format!("Signed in with {label}")},
                {"op":"update_prop","node_id":"hint","property":"text","value":"The host answered the sign-in action."}
            ]
        }
    });
    let (mount, mount_len) = wat_data(&mount);
    let (action, action_len) = wat_data(&action);
    let (patch, patch_len) = wat_data(&patch);
    wat::parse_str(format!(
        r#"(module
          (import "studio_host" "emit" (func $emit (param i32 i32) (result i32)))
          (memory (export "memory") 1 1)
          (table 1 1 funcref)
          (global $phase (mut i32) (i32.const 0))
          (data (i32.const 0) {mount})
          (data (i32.const 8192) {action})
          (data (i32.const 16384) {patch})
          (func (export "studio_alloc") (param i32) (result i32) i32.const 32768)
          (func (export "studio_dealloc") (param i32 i32))
          (func (export "studio_init") (param i32 i32) (result i32)
            i32.const 0 i32.const {mount_len} call $emit)
          (func (export "studio_event") (param i32 i32) (result i32)
            global.get $phase
            if (result i32)
              i32.const 16384 i32.const {patch_len} call $emit
            else
              i32.const 1 global.set $phase
              i32.const 8192 i32.const {action_len} call $emit
            end))"#,
    ))
    .unwrap()
}

/// Encode one protocol message for a guest data segment.
///
/// The segment holds the message's JSON, and the length is that JSON's byte length. The text is
/// escaped for the WebAssembly text format, where `\"` and `\\` are the only escapes a message
/// needs.
fn wat_data(message: &impl serde::Serialize) -> (String, usize) {
    let json = serde_json::to_string(message).unwrap();
    let length = json.len();
    let escaped = json.replace('\\', "\\\\").replace('"', "\\\"");
    (format!("\"{escaped}\""), length)
}

fn provider_signed_bundle(
    manifest: serde_json::Value,
    module: Vec<u8>,
    signing_key: &SigningKey,
) -> Vec<u8> {
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    let input = CanonicalBundleInput {
        manifest,
        module_path: "module.wasm".to_owned(),
        module: module.clone(),
        assets: BTreeMap::new(),
    };
    let signature = signing_key
        .sign(&canonical_bundle_document(&input).unwrap())
        .to_bytes();
    build_archive(
        &ArchiveFiles {
            manifest: manifest_bytes,
            module,
            signature: signature.to_vec(),
            assets: BTreeMap::new(),
        },
        ArchivePolicy::default(),
    )
    .unwrap()
}

#[derive(Clone, Default)]
struct TestCredentialBackend {
    values: Arc<Mutex<std::collections::HashMap<CredentialLocator, Vec<u8>>>>,
}

impl CredentialBackend for TestCredentialBackend {
    fn set_secret(
        &self,
        locator: &CredentialLocator,
        secret: &[u8],
    ) -> Result<(), CredentialBackendError> {
        self.values.lock().insert(locator.clone(), secret.to_vec());
        Ok(())
    }

    fn get_secret(
        &self,
        locator: &CredentialLocator,
    ) -> Result<CredentialBytes, CredentialBackendError> {
        self.values
            .lock()
            .get(locator)
            .cloned()
            .map(CredentialBytes::new)
            .ok_or(CredentialBackendError::NotFound)
    }

    fn delete_secret(&self, locator: &CredentialLocator) -> Result<(), CredentialBackendError> {
        self.values
            .lock()
            .remove(locator)
            .map(|_| ())
            .ok_or(CredentialBackendError::NotFound)
    }
}

#[derive(Clone)]
struct TestBrowser {
    state: Arc<Mutex<Option<String>>>,
    authorization: Arc<Mutex<Option<String>>>,
}

impl BrowserHandoff for TestBrowser {
    fn open(&self, authorization_url: &str) -> Result<(), studio_oauth::OAuthError> {
        let state = authorization_url
            .split('?')
            .nth(1)
            .and_then(|query| {
                query
                    .split('&')
                    .find_map(|part| part.strip_prefix("state="))
            })
            .ok_or_else(|| {
                studio_oauth::OAuthError::new(studio_oauth::OAuthErrorCode::BrowserUnavailable)
            })?;
        *self.authorization.lock() = Some(authorization_url.to_owned());
        *self.state.lock() = Some(state.to_owned());
        Ok(())
    }
}

#[derive(Clone)]
struct TestCallbackListener(Arc<Mutex<Option<String>>>);

impl CallbackListener for TestCallbackListener {
    fn bind(&self) -> Result<Box<dyn CallbackReceiver>, studio_oauth::OAuthError> {
        Ok(Box::new(TestCallbackReceiver(Arc::clone(&self.0))))
    }
}

struct TestCallbackReceiver(Arc<Mutex<Option<String>>>);

impl CallbackReceiver for TestCallbackReceiver {
    fn redirect_uri(&self) -> &'static str {
        "http://127.0.0.1:43123/oauth/callback"
    }

    fn wait(&mut self, _timeout: Duration) -> Result<Callback, studio_oauth::OAuthError> {
        let state = self.0.lock().take().ok_or_else(|| {
            studio_oauth::OAuthError::new(studio_oauth::OAuthErrorCode::CallbackFailed)
        })?;
        Ok(Callback {
            code: Some(String::from("one-time-code")),
            state: Some(state),
            denied: false,
            form_fields: BTreeMap::new(),
        })
    }
}

/// The deterministic upstream the provider tests sign in against.
///
/// It answers the token and profile endpoints of the providers under test and records every
/// request it served, so a test can prove that a sign-in really travelled the provider's own
/// endpoints instead of short-circuiting somewhere.
struct TestProviderHttpsClient {
    served: Arc<Mutex<Vec<String>>>,
    bodies: Arc<Mutex<Vec<String>>>,
}

impl HttpsClient for TestProviderHttpsClient {
    fn execute(
        &self,
        request: OutgoingRequest,
        limits: TransportLimits,
    ) -> Result<IncomingResponse, TransportError> {
        assert!(limits.is_valid());
        self.served.lock().push(request.url.clone());
        self.bodies.lock().push(form_body(&request));
        github_route(&request)
            .or_else(|| discord_route(&request))
            .or_else(|| declared_issuer_route(&request))
            .ok_or(TransportError::ConnectionFailure)
    }

    fn open_stream(
        &self,
        _request: OutgoingRequest,
        _limits: TransportLimits,
    ) -> Result<Box<dyn studio_net::transport::ByteStream>, TransportError> {
        Err(TransportError::ConnectionFailure)
    }
}

/// The GitHub endpoints the host signs in against.
fn github_route(request: &OutgoingRequest) -> Option<IncomingResponse> {
    let response = match (request.method, request.url.as_str()) {
        (HttpMethod::Post, "https://github.com/login/oauth/access_token") => {
            let body = form_body(request);
            assert!(body.contains("client_id=fixture-client"));
            assert!(body.contains("client_secret=fixture-github-secret"));
            assert!(body.contains("code_verifier="));
            assert!(body.contains("code=one-time-code"));
            incoming(
                200,
                &json!({
                    "access_token": "test-access-token",
                    "scope": "read:user,user:email",
                    "expires_in": 3600
                }),
            )
        }
        (HttpMethod::Get, "https://api.github.com/user") => {
            assert_eq!(
                header(request, "authorization"),
                Some("Bearer test-access-token")
            );
            incoming(
                200,
                &json!({
                    "id": 17,
                    "login": "octocat",
                    "name": "Octocat",
                    "email": null,
                    "avatar_url": "https://avatars.example/octocat.png",
                    "html_url": "https://github.com/octocat"
                }),
            )
        }
        (HttpMethod::Get, "https://api.github.com/user/emails") => {
            assert_eq!(
                header(request, "authorization"),
                Some("Bearer test-access-token")
            );
            incoming(
                200,
                &json!([
                    {"email":"octocat@example.test","primary":true,"verified":true}
                ]),
            )
        }
        (HttpMethod::Get, url) if url.starts_with("https://api.github.com/user/repos?") => {
            // The host broker injects the session credential with its bearer scheme.
            let authorization = header(request, "authorization").unwrap_or_default();
            assert_eq!(authorization, "Bearer test-access-token");
            incoming(
                200,
                &json!([
                    {
                        "id": 1,
                        "name": "studio",
                        "full_name": "octocat/studio",
                        "html_url": "https://github.com/octocat/studio",
                        "owner": {"login": "octocat"},
                        "private": false,
                        "stargazers_count": 3,
                        "forks_count": 1
                    }
                ]),
            )
        }
        _ => return None,
    };
    Some(response)
}

/// The Discord endpoints the host signs in against.
fn discord_route(request: &OutgoingRequest) -> Option<IncomingResponse> {
    let response = match (request.method, request.url.as_str()) {
        (HttpMethod::Post, "https://discord.com/api/oauth2/token") => {
            let body = form_body(request);
            assert!(body.contains("client_id=fixture-client"));
            assert!(body.contains("client_secret=fixture-discord-secret"));
            assert!(body.contains("code_verifier="));
            assert!(body.contains("code=one-time-code"));
            incoming(
                200,
                &json!({
                    "access_token": "test-discord-access-token",
                    "token_type": "Bearer",
                    "scope": "email identify",
                    "expires_in": 3600
                }),
            )
        }
        (HttpMethod::Get, "https://discord.com/api/v10/users/@me") => {
            assert_eq!(
                header(request, "authorization"),
                Some("Bearer test-discord-access-token")
            );
            incoming(
                200,
                &json!({
                    "id": "80351110224678912",
                    "username": "discord-user",
                    "global_name": "Discord User",
                    "email": "discord@example.test",
                    "verified": true
                }),
            )
        }
        _ => return None,
    };
    Some(response)
}

/// The endpoints a declared OpenID Connect issuer publishes.
///
/// The discovery document names the issuer the application declared and publishes every
/// endpoint under that issuer's own origin, which is what the host pins.
fn declared_issuer_route(request: &OutgoingRequest) -> Option<IncomingResponse> {
    let response = match (request.method, request.url.as_str()) {
        (HttpMethod::Get, "https://acme.example.com/.well-known/openid-configuration") => incoming(
            200,
            &json!({
                "issuer": "https://acme.example.com",
                "authorization_endpoint": "https://acme.example.com/authorize",
                "token_endpoint": "https://acme.example.com/token",
                "jwks_uri": "https://acme.example.com/jwks"
            }),
        ),
        (HttpMethod::Get, "https://acme.example.com/jwks") => incoming(200, &json!({"keys": []})),
        (HttpMethod::Post, "https://acme.example.com/token") => {
            let body = form_body(request);
            // A declared issuer is a public client: the exchange carries the client id and the
            // PKCE verifier, and no client secret anywhere.
            assert!(body.contains("client_id=fixture-client"));
            assert!(body.contains("code_verifier="));
            assert!(!body.contains("client_secret"));
            incoming(
                200,
                &json!({
                    "access_token": "test-declared-access-token",
                    "token_type": "Bearer",
                    "scope": "openid profile",
                    "expires_in": 3600
                }),
            )
        }
        _ => return None,
    };
    Some(response)
}

/// An upstream whose discovery document names an issuer the application did not declare.
///
/// The host pins the issuer it was given, so a document that points elsewhere is refused
/// rather than followed.
struct MismatchedIssuerHttpsClient;

impl HttpsClient for MismatchedIssuerHttpsClient {
    fn execute(
        &self,
        request: OutgoingRequest,
        limits: TransportLimits,
    ) -> Result<IncomingResponse, TransportError> {
        assert!(limits.is_valid());
        if request.url == "https://acme.example.com/.well-known/openid-configuration" {
            return Ok(incoming(
                200,
                &json!({
                    "issuer": "https://elsewhere.example.com",
                    "authorization_endpoint": "https://elsewhere.example.com/authorize",
                    "token_endpoint": "https://elsewhere.example.com/token",
                    "jwks_uri": "https://elsewhere.example.com/jwks"
                }),
            ));
        }
        Err(TransportError::ConnectionFailure)
    }

    fn open_stream(
        &self,
        _request: OutgoingRequest,
        _limits: TransportLimits,
    ) -> Result<Box<dyn studio_net::transport::ByteStream>, TransportError> {
        Err(TransportError::ConnectionFailure)
    }
}

/// An upstream that answers nothing at all.
///
/// The host cannot learn an issuer's endpoints without reaching it, so an issuer that cannot be
/// reached is an issuer the host cannot configure.
struct UnreachableHttpsClient;

impl HttpsClient for UnreachableHttpsClient {
    fn execute(
        &self,
        _request: OutgoingRequest,
        _limits: TransportLimits,
    ) -> Result<IncomingResponse, TransportError> {
        Err(TransportError::ConnectionFailure)
    }

    fn open_stream(
        &self,
        _request: OutgoingRequest,
        _limits: TransportLimits,
    ) -> Result<Box<dyn studio_net::transport::ByteStream>, TransportError> {
        Err(TransportError::ConnectionFailure)
    }
}

/// Decode a form-encoded request body without taking ownership of it.
fn form_body(request: &OutgoingRequest) -> String {
    String::from_utf8(request.body.as_deref().unwrap_or_default().to_vec()).unwrap()
}

fn incoming(status: u16, value: &serde_json::Value) -> IncomingResponse {
    IncomingResponse {
        status,
        media_type: Some(String::from("application/json")),
        body: serde_json::to_vec(value).unwrap(),
    }
}

fn header<'a>(request: &'a OutgoingRequest, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find_map(|(header_name, value)| (header_name == name).then_some(value.as_str()))
}

fn manifest() -> serde_json::Value {
    json!({
        "schemaVersion": 1,
        "id": "com.example.pos",
        "name": "Example POS",
        "version": "0.1.0",
        "publisher": {"id": "example", "keyId": "key-1"},
        "entry": "module.wasm",
        "sdkVersion": "^0.1.0",
        "protocolVersion": 1,
        "capabilities": [],
        "limits": {"memoryMiB": 16, "eventFuel": 10_000_000},
        "assets": []
    })
}

fn signed_bundle(module: Vec<u8>, signing_key: &SigningKey, valid_signature: bool) -> Vec<u8> {
    let manifest = manifest();
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    let input = CanonicalBundleInput {
        manifest,
        module_path: "module.wasm".to_owned(),
        module: module.clone(),
        assets: BTreeMap::new(),
    };
    let mut signature = signing_key
        .sign(&canonical_bundle_document(&input).unwrap())
        .to_bytes();
    if !valid_signature {
        signature[0] ^= 1;
    }
    build_archive(
        &ArchiveFiles {
            manifest: manifest_bytes,
            module,
            signature: signature.to_vec(),
            assets: BTreeMap::new(),
        },
        ArchivePolicy::default(),
    )
    .unwrap()
}

fn trust(signing_key: &SigningKey) -> TrustStore {
    TrustStore::from_keys([TrustedPublisherKey {
        publisher_id: "example".to_owned(),
        key_id: "key-1".to_owned(),
        verifying_key: signing_key.verifying_key().to_bytes(),
        enabled: true,
    }])
    .unwrap()
}

fn provisioned_trust(signing_key: &SigningKey) -> TrustStore {
    let public_key = signing_key
        .verifying_key()
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let snapshot = json!({
        "schemaVersion": 1,
        "snapshotId": "test-launch-1",
        "version": 1,
        "validFrom": 100,
        "expiresAt": 300,
        "keys": [{
            "publisherId": "example",
            "keyId": "key-1",
            "publicKey": public_key,
            "validFrom": 100,
            "expiresAt": 300
        }],
        "revocations": []
    });
    TrustStore::from_json_at(&serde_json::to_vec(&snapshot).unwrap(), 200).unwrap()
}

fn production_request(path: &Path) -> LaunchRequest {
    LaunchRequest::parse_from(["studio", "--bundle", path.to_str().unwrap()]).unwrap()
}

#[test]
fn production_requires_an_absolute_regular_file_selection() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let host = StudioHost::new(
        HostConfig::new(trust(&signing_key)),
        WaylandAvailability::Available,
    );
    let relative = LaunchRequest::parse_from(["studio", "--bundle", "relative.studio"]).unwrap();
    assert_eq!(
        host.prepare(relative).unwrap_err().code(),
        LaunchErrorCode::PathInvalid
    );

    let fixtures = FixtureDirectory::new();
    let directory = production_request(&fixtures.0);
    assert_eq!(
        host.prepare(directory).unwrap_err().code(),
        LaunchErrorCode::PathInvalid
    );
}

#[test]
fn valid_signed_bundle_mounts_before_exposing_the_surface() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "valid.studio",
        &signed_bundle(plugin_module(), &signing_key, true),
    );
    let host = StudioHost::new(
        HostConfig::new(provisioned_trust(&signing_key)),
        WaylandAvailability::Available,
    );
    let surface = host.prepare(production_request(&path)).unwrap();

    assert_eq!(surface.mode(), LaunchMode::Production);
    assert_eq!(surface.registry().root_id(), Some("root"));
    assert!(surface.warning().is_none());
}

#[test]
fn production_rejects_empty_default_trust_before_guest_execution() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "valid.studio",
        &signed_bundle(plugin_module(), &signing_key, true),
    );
    let host = StudioHost::new(
        HostConfig::new(TrustStore::default()),
        WaylandAvailability::Available,
    );
    assert_eq!(
        host.prepare(production_request(&path)).unwrap_err().code(),
        LaunchErrorCode::TrustConfigurationInvalid
    );
}

#[test]
fn signature_mutation_is_rejected_before_guest_execution() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "mutated.studio",
        &signed_bundle(plugin_module(), &signing_key, false),
    );
    let host = StudioHost::new(
        HostConfig::new(trust(&signing_key)),
        WaylandAvailability::Available,
    );
    assert_eq!(
        host.prepare(production_request(&path)).unwrap_err().code(),
        LaunchErrorCode::IntegrityInvalid
    );
}

#[test]
fn unsigned_development_launch_requires_explicit_dev_and_keeps_warning() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "unsigned.studio",
        &signed_bundle(plugin_module(), &signing_key, false),
    );
    let request = LaunchRequest::parse_from(["studio", "--dev", path.to_str().unwrap()]).unwrap();
    let host = StudioHost::new(
        HostConfig::new(TrustStore::default()),
        WaylandAvailability::Available,
    );
    let surface = host.prepare(request).unwrap();
    assert_eq!(surface.mode(), LaunchMode::Development);
    assert_eq!(surface.warning(), Some(DEVELOPMENT_WARNING));
}

#[test]
fn no_wayland_session_rejects_launch_without_x11_fallback() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "valid.studio",
        &signed_bundle(plugin_module(), &signing_key, true),
    );
    let host = StudioHost::new(
        HostConfig::new(trust(&signing_key)),
        WaylandAvailability::Unavailable,
    );
    assert_eq!(
        host.prepare(production_request(&path)).unwrap_err().code(),
        LaunchErrorCode::WaylandUnavailable
    );
}

#[test]
fn mounted_catalog_accepts_pointer_and_keyboard_activation() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "valid.studio",
        &signed_bundle(plugin_module(), &signing_key, true),
    );
    let host = StudioHost::new(
        HostConfig::new(trust(&signing_key)),
        WaylandAvailability::Available,
    );
    let surface = host.prepare(production_request(&path)).unwrap();
    for action in [InputAction::PointerClick, InputAction::KeyboardActivate] {
        let HostEvent::Ui(event) = surface.dispatch_input("checkout", action).unwrap() else {
            panic!("expected UI event");
        };
        assert_eq!(event.event, "pressed");
    }
}

/// One provider's sign-in fixture.
struct ProviderFixture {
    manifest: serde_json::Value,
    capability: &'static str,
    provider: &'static str,
    label: &'static str,
    secret_name: &'static str,
    purpose: &'static str,
    secret: &'static str,
}

/// The traffic one sign-in produced at the deterministic upstream.
#[derive(Debug)]
struct UpstreamTraffic {
    /// Every URL the upstream was asked for.
    urls: Vec<String>,
    /// Every request body the upstream was asked to serve.
    bodies: Vec<String>,
    /// The authorization request the host opened in the browser, when it opened one.
    authorization: Option<String>,
}

/// Sign in with one provider through the host action route and return the requests the
/// deterministic upstream answered.
///
/// The guest asks for sign-in on its first event, the host runs the action on its own thread,
/// and the result is delivered back to the guest. The upstream records what it served, so the
/// test can tell a sign-in that really travelled the provider's endpoints from one that did
/// not happen at all.
fn sign_in_through_action_route(fixture: ProviderFixture, signing_key: &SigningKey) -> Vec<String> {
    let ProviderFixture {
        manifest,
        capability,
        provider,
        label,
        secret_name,
        purpose,
        secret,
    } = fixture;
    run_provider_sign_in(
        manifest,
        capability,
        provider,
        label,
        Some((secret_name, purpose, secret)),
        signing_key,
    )
    .urls
}

/// Sign in with one provider and return the traffic the deterministic upstream saw.
///
/// A provider that is a public client declares no secret, so the protected store is left
/// empty for it.
fn run_provider_sign_in(
    manifest: serde_json::Value,
    capability: &str,
    provider: &str,
    label: &str,
    secret: Option<(&str, &str, &str)>,
    signing_key: &SigningKey,
) -> UpstreamTraffic {
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "provider.studio",
        &provider_signed_bundle(
            manifest,
            provider_action_module(capability, provider, label),
            signing_key,
        ),
    );
    let served = Arc::new(Mutex::new(Vec::new()));
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let authorization = Arc::new(Mutex::new(None));
    let state = Arc::new(Mutex::new(None));
    let mut host = StudioHost::new(
        HostConfig::new(trust(signing_key)),
        WaylandAvailability::Available,
    )
    .with_https_client(Arc::new(TestProviderHttpsClient {
        served: Arc::clone(&served),
        bodies: Arc::clone(&bodies),
    }))
    .with_credential_backend(Arc::new(TestCredentialBackend::default()))
    .with_oauth_adapters(
        Arc::new(TestBrowser {
            state: Arc::clone(&state),
            authorization: Arc::clone(&authorization),
        }),
        Arc::new(TestCallbackListener(Arc::clone(&state))),
        Arc::new(OsEntropy),
    );
    if let Some((name, purpose, value)) = secret {
        host = host.with_protected_secret(
            ProtectedSecretKey::new(name, purpose).unwrap(),
            SecretInput::new(value.as_bytes().to_vec()).unwrap(),
        );
    }
    let mut surface = host.prepare(production_request(&path)).unwrap();
    surface
        .process_input("signin", InputAction::PointerClick)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while !surface.poll_action_results().unwrap() {
        assert!(
            Instant::now() < deadline,
            "the sign-in action never produced a result"
        );
        thread::sleep(Duration::from_millis(20));
    }
    UpstreamTraffic {
        urls: served.lock().clone(),
        bodies: bodies.lock().clone(),
        authorization: authorization.lock().clone(),
    }
}

#[test]
fn a_non_github_provider_signs_in_through_the_generic_action_route() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let served = sign_in_through_action_route(
        ProviderFixture {
            manifest: discord_manifest(),
            capability: "discord.oauth",
            provider: "discord",
            label: "Discord",
            secret_name: "discord.oauth.client_secret",
            purpose: "Discord OAuth client configuration",
            secret: "fixture-discord-secret",
        },
        &signing_key,
    );
    assert!(
        served
            .iter()
            .any(|url| url == "https://discord.com/api/oauth2/token"),
        "the provider's token endpoint was never called: {served:?}"
    );
    assert!(
        served
            .iter()
            .any(|url| url == "https://discord.com/api/v10/users/@me"),
        "the provider's profile endpoint was never called: {served:?}"
    );
}

#[test]
fn github_sign_in_still_reaches_the_repository_route_group() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let served = sign_in_through_action_route(
        ProviderFixture {
            manifest: github_manifest(),
            capability: "github.oauth",
            provider: "github",
            label: "GitHub",
            secret_name: "github.oauth.client_secret",
            purpose: "GitHub OAuth client configuration",
            secret: "fixture-github-secret",
        },
        &signing_key,
    );
    assert!(
        served
            .iter()
            .any(|url| url == "https://github.com/login/oauth/access_token"),
        "the provider's token endpoint was never called: {served:?}"
    );
    assert!(
        served
            .iter()
            .any(|url| url.starts_with("https://api.github.com/user/repos?")),
        "the repository route group was never called: {served:?}"
    );
}

/// A declared issuer signs in through the same host action route a maintained provider uses.
///
/// The host pins the issuer it was given: it fetches that issuer's own discovery document,
/// builds the authorization request from the endpoints the document published, and exchanges
/// the code at the issuer's own token endpoint as a public client. The signed ID token itself
/// is the OAuth engine's business, so this test stops at the exchange the host really made.
#[test]
fn a_declared_issuer_signs_in_through_the_generic_action_route() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let traffic = run_provider_sign_in(
        declared_issuer_manifest(),
        "oidc-acme.oauth",
        "oidc-acme",
        "Acme",
        None,
        &signing_key,
    );
    assert!(
        traffic
            .urls
            .iter()
            .any(|url| { url == "https://acme.example.com/.well-known/openid-configuration" }),
        "the issuer's discovery document was never fetched: {traffic:?}"
    );
    assert!(
        traffic
            .urls
            .iter()
            .any(|url| url == "https://acme.example.com/token"),
        "the issuer's token endpoint was never called: {traffic:?}"
    );
    let authorization = traffic
        .authorization
        .as_deref()
        .expect("the host never opened an authorization request");
    assert!(
        authorization.starts_with("https://acme.example.com/authorize?"),
        "the authorization request did not use the issuer's own endpoint: {authorization}"
    );
    assert!(
        authorization.contains("client_id=fixture-client"),
        "the authorization request did not name the declared client id: {authorization}"
    );
    assert!(
        authorization.contains("&scope=openid%20profile&"),
        "the authorization request did not ask for exactly the declared scopes: {authorization}"
    );
    let exchange = traffic
        .bodies
        .iter()
        .find(|body| body.contains("grant_type=authorization_code"))
        .expect("the token exchange was never sent");
    assert!(
        exchange.contains("client_id=fixture-client"),
        "the exchange did not name the declared client id: {exchange}"
    );
    assert!(
        exchange.contains("code_verifier="),
        "the exchange was not bound by PKCE: {exchange}"
    );
    assert!(
        !exchange.contains("client_secret"),
        "a declared issuer is a public client, so the exchange carried a secret: {exchange}"
    );
}

#[test]
fn a_declared_issuer_that_names_another_issuer_fails_the_launch_closed() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "declared.studio",
        &provider_signed_bundle(declared_issuer_manifest(), plugin_module(), &signing_key),
    );
    let host = StudioHost::new(
        HostConfig::new(trust(&signing_key)),
        WaylandAvailability::Available,
    )
    .with_https_client(Arc::new(MismatchedIssuerHttpsClient));
    assert_eq!(
        host.prepare(production_request(&path)).unwrap_err().code(),
        LaunchErrorCode::ProviderConfigurationInvalid
    );
}

#[test]
fn a_declared_issuer_whose_discovery_cannot_be_fetched_fails_the_launch_closed() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "declared.studio",
        &provider_signed_bundle(declared_issuer_manifest(), plugin_module(), &signing_key),
    );
    let host = StudioHost::new(
        HostConfig::new(trust(&signing_key)),
        WaylandAvailability::Available,
    )
    .with_https_client(Arc::new(UnreachableHttpsClient));
    assert_eq!(
        host.prepare(production_request(&path)).unwrap_err().code(),
        LaunchErrorCode::ProviderConfigurationInvalid
    );
}

#[test]
fn a_declared_issuer_needs_a_client_that_can_reach_its_issuer() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "declared.studio",
        &provider_signed_bundle(declared_issuer_manifest(), plugin_module(), &signing_key),
    );
    let host = StudioHost::new(
        HostConfig::new(trust(&signing_key)),
        WaylandAvailability::Available,
    );
    assert_eq!(
        host.prepare(production_request(&path)).unwrap_err().code(),
        LaunchErrorCode::ProviderConfigurationInvalid
    );
}
