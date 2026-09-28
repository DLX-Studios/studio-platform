#![allow(missing_docs)]

use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use base64::Engine as _;
use serde_json::{Value, json};
use studio_net::{
    HttpsClient, IncomingResponse, OutgoingRequest, TransportError, TransportLimits,
    declaration::HttpMethod, transport::ByteStream,
};
use studio_oauth::{
    BrowserHandoff, GithubHttpsOAuthTransport, OAuthError, OAuthErrorCode, OAuthManager, OsEntropy,
    ProtectedSecretReference, ProviderDescriptor, ProviderPackage, ProviderRegistry,
    TcpLoopbackListener,
};
use studio_security::{
    ApplicationEnvironment, BrokerCredentialError, BrokerCredentialSink, CredentialBackend,
    CredentialBackendError, CredentialBytes, CredentialLocator, PluginPrincipal,
    ProtectedSecretKey, ProtectedSecretStore, SecretInput, TrustMode,
};

const ACCESS_TOKEN: &[u8] = b"host-only-access-token-sentinel";
const CLIENT_SECRET: &[u8] = b"host-only-client-secret-sentinel";

#[derive(Clone, Default)]
struct MemoryBackend {
    records: Arc<Mutex<HashMap<CredentialLocator, Vec<u8>>>>,
}

impl CredentialBackend for MemoryBackend {
    fn set_secret(
        &self,
        locator: &CredentialLocator,
        secret: &[u8],
    ) -> Result<(), CredentialBackendError> {
        self.records.lock().insert(locator.clone(), secret.to_vec());
        Ok(())
    }

    fn get_secret(
        &self,
        locator: &CredentialLocator,
    ) -> Result<CredentialBytes, CredentialBackendError> {
        self.records
            .lock()
            .get(locator)
            .cloned()
            .map(CredentialBytes::new)
            .ok_or(CredentialBackendError::NotFound)
    }

    fn delete_secret(&self, locator: &CredentialLocator) -> Result<(), CredentialBackendError> {
        self.records
            .lock()
            .remove(locator)
            .map(|_| ())
            .ok_or(CredentialBackendError::NotFound)
    }
}

#[derive(Default)]
struct CallbackBrowser {
    authorization_urls: Mutex<Vec<String>>,
    callback_threads: Mutex<Vec<JoinHandle<Result<(), String>>>>,
}

impl CallbackBrowser {
    fn join_callbacks(&self) {
        for handle in self.callback_threads.lock().drain(..) {
            handle.join().unwrap().unwrap();
        }
    }
}

impl BrowserHandoff for CallbackBrowser {
    fn open(&self, authorization_url: &str) -> Result<(), OAuthError> {
        self.authorization_urls
            .lock()
            .push(authorization_url.to_owned());
        let redirect = query_value(authorization_url, "redirect_uri");
        let state = query_value(authorization_url, "state");
        let handle = std::thread::spawn(move || send_loopback_callback(&redirect, &state));
        self.callback_threads.lock().push(handle);
        Ok(())
    }
}

fn query_value(url: &str, name: &str) -> String {
    url.split_once('?')
        .expect("authorization URL has query")
        .1
        .split('&')
        .find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (decode_component(key) == name).then(|| decode_component(value))
        })
        .expect("authorization URL contains required parameter")
}

fn decode_component(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let high = hex(bytes[index + 1]).expect("valid percent encoding");
                let low = hex(bytes[index + 2]).expect("valid percent encoding");
                decoded.push((high << 4) | low);
                index += 3;
            }
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).expect("authorization URL contains UTF-8 values")
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn send_loopback_callback(redirect: &str, state: &str) -> Result<(), String> {
    let authority_and_path = redirect
        .strip_prefix("http://")
        .ok_or_else(|| String::from("redirect was not HTTP loopback"))?;
    let (authority, path) = authority_and_path
        .split_once('/')
        .ok_or_else(|| String::from("redirect path missing"))?;
    let address = authority
        .parse::<SocketAddr>()
        .map_err(|_| String::from("loopback address invalid"))?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))
        .map_err(|_| String::from("loopback callback connection failed"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| String::from("loopback callback timeout could not be set"))?;
    write!(
        stream,
        "GET /{path}?code=one-time-code&state={state} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|_| String::from("loopback callback request failed"))?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|_| String::from("loopback callback request shutdown failed"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|_| String::from("loopback callback response failed"))?;
    if response.starts_with("HTTP/1.1 200 OK") {
        Ok(())
    } else {
        Err(String::from("loopback callback was not accepted"))
    }
}

struct TestHttpsClient;

impl HttpsClient for TestHttpsClient {
    fn execute(
        &self,
        request: OutgoingRequest,
        limits: TransportLimits,
    ) -> Result<IncomingResponse, TransportError> {
        assert!(limits.is_valid());
        let debug = format!("{request:?}");
        assert!(!debug.contains("host-only-access-token-sentinel"));
        assert!(!debug.contains("host-only-client-secret-sentinel"));
        let response = match (request.method, request.url.as_str()) {
            (HttpMethod::Post, "https://github.com/login/oauth/access_token") => {
                let body = String::from_utf8(request.body.unwrap()).unwrap();
                assert!(body.contains("grant_type=authorization_code"));
                assert!(body.contains("client_id=github-client-id"));
                assert!(body.contains("code=one-time-code"));
                assert!(body.contains("code_verifier="));
                assert!(body.contains("client_secret=host-only-client-secret-sentinel"));
                response(
                    200,
                    &json!({
                        "access_token": "host-only-access-token-sentinel",
                        "scope": "read:user,user:email",
                        "expires_in": 3600
                    }),
                )
            }
            (HttpMethod::Get, "https://api.github.com/user") => {
                assert_eq!(
                    header(&request, "authorization"),
                    "Bearer host-only-access-token-sentinel"
                );
                response(
                    200,
                    &json!({
                        "id": 12345,
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
                    header(&request, "authorization"),
                    "Bearer host-only-access-token-sentinel"
                );
                response(
                    200,
                    &json!([
                        {"email":"octocat@example.test","primary":true,"verified":true},
                        {"email":"unverified@example.test","primary":true,"verified":false}
                    ]),
                )
            }
            (HttpMethod::Delete, "https://api.github.com/applications/github-client-id/token") => {
                let authorization = header(&request, "authorization");
                let encoded = authorization.strip_prefix("Basic ").unwrap();
                assert_eq!(
                    base64::engine::general_purpose::STANDARD
                        .decode(encoded)
                        .unwrap(),
                    b"github-client-id:host-only-client-secret-sentinel"
                );
                let body: Value = serde_json::from_slice(request.body.as_deref().unwrap()).unwrap();
                assert_eq!(body["access_token"], "host-only-access-token-sentinel");
                IncomingResponse {
                    status: 204,
                    media_type: None,
                    body: Vec::new(),
                }
            }
            _ => return Err(TransportError::ConnectionFailure),
        };
        Ok(response)
    }

    fn open_stream(
        &self,
        _request: OutgoingRequest,
        _limits: TransportLimits,
    ) -> Result<Box<dyn ByteStream>, TransportError> {
        Err(TransportError::ConnectionFailure)
    }
}

fn response(status: u16, value: &Value) -> IncomingResponse {
    IncomingResponse {
        status,
        media_type: Some(String::from("application/json")),
        body: serde_json::to_vec(value).unwrap(),
    }
}

fn header<'a>(request: &'a OutgoingRequest, name: &str) -> &'a str {
    request
        .headers
        .iter()
        .find_map(|(header_name, value)| (header_name == name).then_some(value.as_str()))
        .unwrap()
}

#[derive(Default)]
struct CaptureCredential(Vec<u8>);

impl BrokerCredentialSink for CaptureCredential {
    fn inject(&mut self, secret: &[u8]) -> Result<(), BrokerCredentialError> {
        self.0.extend_from_slice(secret);
        Ok(())
    }
}

fn principal() -> PluginPrincipal {
    PluginPrincipal::new_verified(
        "github.example",
        "github-signing-key",
        "com.example.github-viewer",
        [7; 32],
        [9; 16],
        TrustMode::Production,
    )
    .unwrap()
}

fn configured_manager(browser: &Arc<CallbackBrowser>) -> OAuthManager {
    let protected_store = ProtectedSecretStore::new(MemoryBackend::default());
    let principal = principal();
    let client_secret = ProtectedSecretKey::new(
        "github.oauth.client_secret",
        "GitHub OAuth client configuration",
    )
    .unwrap();
    protected_store
        .for_application(&principal, ApplicationEnvironment::Production)
        .unwrap()
        .configure(
            &client_secret,
            SecretInput::new(CLIENT_SECRET.to_vec()).unwrap(),
        )
        .unwrap();
    let token_store = studio_oauth::ProtectedOAuthTokenStore::new(
        protected_store,
        principal,
        ApplicationEnvironment::Production,
    );
    let package = ProviderPackage::latest("github", "github-client-id").with_client_secret(
        ProtectedSecretReference {
            name: String::from("github.oauth.client_secret"),
            purpose: String::from("GitHub OAuth client configuration"),
        },
    );
    let mut manager = OAuthManager::new(
        ProviderRegistry::github(),
        [package],
        Arc::new(token_store),
        browser.clone(),
        Arc::new(TcpLoopbackListener),
        Arc::new(OsEntropy),
        Arc::new(GithubHttpsOAuthTransport::new(Arc::new(TestHttpsClient))),
    );
    manager.set_callback_timeout(Duration::from_secs(5));
    manager
}

#[test]
fn github_sign_in_private_email_session_injection_hot_update_and_revocation() {
    let browser = Arc::new(CallbackBrowser::default());
    let manager = configured_manager(&browser);

    let unknown = manager.sign_in("not-installed");
    assert!(!unknown.is_success());
    assert_eq!(
        manager.status("github"),
        studio_oauth::OAuthStatus::SignedOut
    );
    assert_eq!(
        manager
            .enable_package(ProviderPackage::new("github", "9.9.9", "github-client-id"))
            .unwrap_err()
            .code(),
        OAuthErrorCode::ProviderOutdated
    );

    let mut newer = ProviderDescriptor::github();
    newer.version = String::from("1.2.0");
    newer.authorization_endpoint = String::from("https://login.example.test/oauth/authorize");
    manager.register_descriptor(newer).unwrap();

    let signed_in = manager.sign_in("github");
    browser.join_callbacks();
    assert!(signed_in.is_success());
    assert_eq!(signed_in.status, studio_oauth::OAuthStatus::Authenticated);
    let claims = signed_in.claims.as_ref().unwrap();
    assert_eq!(claims.subject, "12345");
    assert_eq!(claims.email.as_deref(), Some("octocat@example.test"));
    assert_eq!(
        manager.status("github"),
        studio_oauth::OAuthStatus::Authenticated
    );
    assert!(
        browser.authorization_urls.lock()[0]
            .starts_with("https://login.example.test/oauth/authorize?")
    );
    assert!(browser.authorization_urls.lock()[0].contains("scope=read%3Auser%20user%3Aemail"));

    let encoded_result = serde_json::to_string(&signed_in).unwrap();
    assert!(!encoded_result.contains("host-only-access-token-sentinel"));
    assert!(!format!("{manager:?}").contains("host-only-access-token-sentinel"));

    let mut credential = CaptureCredential::default();
    studio_net::credential::OAuthSessionResolver::inject_session(
        &manager,
        "github",
        "github.repositories",
        &mut credential,
    )
    .unwrap();
    assert_eq!(credential.0, ACCESS_TOKEN);

    let refresh = manager.refresh("github");
    assert_eq!(
        refresh.code_str(),
        OAuthErrorCode::RefreshUnavailable.as_str()
    );

    let revoked = manager.revoke("github");
    assert!(revoked.is_success());
    assert_eq!(revoked.status, studio_oauth::OAuthStatus::Revoked);
    assert_eq!(manager.status("github"), studio_oauth::OAuthStatus::Revoked);
    let mut after_revoke = CaptureCredential::default();
    assert_eq!(
        studio_net::credential::OAuthSessionResolver::inject_session(
            &manager,
            "github",
            "github.repositories",
            &mut after_revoke,
        )
        .unwrap_err()
        .code(),
        studio_net::BrokerErrorCode::OauthSessionUnavailable
    );
    assert!(after_revoke.0.is_empty());
}
