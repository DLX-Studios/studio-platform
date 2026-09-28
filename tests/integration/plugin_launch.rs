#![allow(missing_docs)]
#![allow(clippy::format_collect)]

use parking_lot::Mutex;
use std::{
    collections::{BTreeMap, HashMap},
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
use studio_oauth::{
    BrowserHandoff, Callback, CallbackListener, CallbackReceiver, EntropySource, OAuthError,
    OAuthErrorCode, OsEntropy,
};
use studio_security::{
    BrokerCredentialError, CredentialBackend, CredentialBackendError, CredentialBytes,
    CredentialLocator, ProtectedSecretKey, SecretInput,
};

use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
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

fn github_action_module() -> Vec<u8> {
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
            ("label".to_owned(), json!("Sign in with GitHub")),
            ("enabled".to_owned(), json!(true)),
        ]),
        children: Vec::new(),
    };
    let mount = GuestMessage::Mount(MountTree {
        protocol_version: 1,
        route: "/github".to_owned(),
        root: UiNode {
            id: "root".to_owned(),
            kind: NodeKind::Column,
            props: BTreeMap::new(),
            children: vec![
                sign_in_button,
                text_node("status", "Sign in to list repositories"),
                text_node("hint", "Repository access uses declared routes."),
            ],
        },
    });
    let action = GuestMessage::Action(studio_protocol::ActionRequest {
        request_id: String::from("github-sign-in"),
        capability: String::from("github.oauth"),
        operation: String::from("sign_in"),
        payload: json!({"provider": "github", "scopes": ["read:user", "user:email"]}),
    });
    let patch = json!({
        "type": "patch",
        "payload": {
            "sequence": 1,
            "operations": [
                {"op":"update_prop","node_id":"status","property":"text","value":"Signed in with GitHub"},
                {"op":"update_prop","node_id":"hint","property":"text","value":"Repositories: octocat/studio"}
            ]
        }
    });
    let mount =
        serde_json::to_string(&String::from_utf8(serde_json::to_vec(&mount).unwrap()).unwrap())
            .unwrap();
    let action =
        serde_json::to_string(&String::from_utf8(serde_json::to_vec(&action).unwrap()).unwrap())
            .unwrap();
    let patch = serde_json::to_string(&patch.to_string()).unwrap();
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
        mount_len = mount.len(),
        action_len = action.len(),
        patch_len = patch.len(),
    ))
    .unwrap()
}

fn github_signed_bundle(module: Vec<u8>, signing_key: &SigningKey) -> Vec<u8> {
    let manifest = github_manifest();
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
struct TestBrowser(Arc<Mutex<Option<String>>>);

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
        *self.0.lock() = Some(state.to_owned());
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
    fn redirect_uri(&self) -> &str {
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
        })
    }
}

struct TestGitHubHttpsClient;

impl HttpsClient for TestGitHubHttpsClient {
    fn execute(
        &self,
        request: OutgoingRequest,
        limits: TransportLimits,
    ) -> Result<IncomingResponse, TransportError> {
        assert!(limits.is_valid());
        let response = match (request.method, request.url.as_str()) {
            (
                studio_net::declaration::HttpMethod::Post,
                "https://github.com/login/oauth/access_token",
            ) => {
                let body = String::from_utf8(request.body.unwrap()).unwrap();
                assert!(body.contains("client_id=fixture-client"));
                assert!(body.contains("client_secret=fixture-github-secret"));
                assert!(body.contains("code_verifier="));
                assert!(body.contains("code=one-time-code"));
                incoming(
                    200,
                    json!({
                        "access_token": "test-access-token",
                        "scope": "read:user,user:email",
                        "expires_in": 3600
                    }),
                )
            }
            (studio_net::declaration::HttpMethod::Get, "https://api.github.com/user") => {
                assert_eq!(
                    header(&request, "authorization"),
                    Some("Bearer test-access-token")
                );
                incoming(
                    200,
                    json!({
                        "id": 17,
                        "login": "octocat",
                        "name": "Octocat",
                        "email": null,
                        "avatar_url": "https://avatars.example/octocat.png",
                        "html_url": "https://github.com/octocat"
                    }),
                )
            }
            (studio_net::declaration::HttpMethod::Get, "https://api.github.com/user/emails") => {
                assert_eq!(
                    header(&request, "authorization"),
                    Some("Bearer test-access-token")
                );
                incoming(
                    200,
                    json!([
                        {"email":"octocat@example.test","primary":true,"verified":true}
                    ]),
                )
            }
            (studio_net::declaration::HttpMethod::Get, url)
                if url.starts_with("https://api.github.com/user/repos?") =>
            {
                assert_eq!(
                    header(&request, "authorization"),
                    Some("Bearer test-access-token")
                );
                incoming(
                    200,
                    json!([
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
            _ => return Err(TransportError::ConnectionFailure),
        };
        Ok(response)
    }

    fn open_stream(
        &self,
        _request: OutgoingRequest,
        _limits: TransportLimits,
    ) -> Result<Box<dyn studio_net::transport::ByteStream>, TransportError> {
        Err(TransportError::ConnectionFailure)
    }
}

fn incoming(status: u16, value: serde_json::Value) -> IncomingResponse {
    IncomingResponse {
        status,
        media_type: Some(String::from("application/json")),
        body: serde_json::to_vec(&value).unwrap(),
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
