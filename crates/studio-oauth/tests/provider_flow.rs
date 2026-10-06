#![allow(missing_docs)]

use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use base64::Engine as _;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode, jwk::JwkSet};
use serde_json::{Value, json};
use studio_net::{
    HttpsClient, IncomingResponse, OutgoingRequest, TransportError, TransportLimits,
    declaration::HttpMethod, transport::ByteStream,
};
use studio_oauth::{
    BrowserHandoff, HttpsOAuthTransport, JwksProvider, OAuthError, OAuthErrorCode, OAuthManager,
    OsEntropy, ProtectedSecretReference, ProviderDescriptor, ProviderPackage, ProviderRegistry,
    TcpLoopbackListener,
};
use studio_security::{
    ApplicationEnvironment, BrokerCredentialError, BrokerCredentialSink, CredentialBackend,
    CredentialBackendError, CredentialBytes, CredentialLocator, PluginPrincipal,
    ProtectedSecretKey, ProtectedSecretStore, SecretInput, TrustMode,
};

const ACCESS_TOKEN: &[u8] = b"host-only-access-token-sentinel";
const CLIENT_SECRET: &[u8] = b"host-only-client-secret-sentinel";

/// RSA key material used only by this test file to stand in for Apple's published signing key.
/// It was generated for these deterministic tests and is not a credential for any real service.
const APPLE_TEST_SIGNING_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQCsX5Md9P4fSLm7
MZ9hQXQSmCQRcQxdChyQfoum7ZD6jiRTWzTTMJXWc2HEpL1ma1pY2baEeNFv+Ra1
2C2KK5WO3As3Stux7JlrXt+RWq3NvVNwGdHueo1cxW2n5ysNtOoI/92fnNmi9ZKR
2R7EVd/1LnoHan9uQVjcxUerWqoB805Dsj31/H5bXUbIXD0cboZWviOiQy7P0hqg
u2zYnkc2ehDniTkCc2rNNxIGGNevJXmNv0lae/QMjBmf/+WmZFHAFEuZX2vj8wbM
WM36qtTe+QEbg4a7YoSQU/TzD/rXxeuxbe1veUF2kBAWmb8nqCXzNSfy5MfIhd0S
a1Wk6TqRAgMBAAECggEACGsN/6RpfTxjYrSekqS8TxSqTUZCuu/9m2JsvlEjl5L1
y/LcOnSo2q1toMmFXPFA+y4dw2w1mUT2UpjtR25Pumd9bjy8DD1QdE2QuFPwAUhd
4O+H4MBQM0/KhkzMPdoAJIKfb5Y8zEA3QWNzxNWnT31utPpC4S70kKqbUI9n1lq3
qib69Y8RB71cri2CMFoZb2mziNnY4cCFOtG68SRZ83R0p06/w5Z2v9gl2sauB+bF
7DGGwucivXLzBK3EXSKOF9Bz+ZzO8ABrWC+n8Bzz9e+cPJX8+sfYCehfQYBJoIIA
07hLC32oC+GcAKQZ3VTrYy2FVlsVZfVzbvuVT0JzAQKBgQDV2EvHUGLYbZvnysIF
AeYF3Qz0c0VpA8UQTGsHacmmtkwS3yYoIbAGTADK8PI6Q5qDdDZ213gvwzjC965X
Je32MwBdGafL2KN1PkZDpIJCrSg2GwojKgQD3SWg0Hhwld07nb+8l8hthsFTHics
FqtXsPmOKyQRpsOJWuPtTD/DgQKBgQDOWml2xhJgoLWCgCG+THC+V6C7Um8P2DyD
NWSubcwcdkThET43p0lHi7VMBIqUsf6nHirqDYahN2yEJfswqwAh6a0UHcYUDoWK
U/QoCMMnrJ2t3KIYgtDswjmksxSLn6FVvyW3XRGOVlrmbCBuGVVKd3hB9RH8OYvo
ANVRqdC/EQKBgQC9Dzan530MZZOh0VPJg40u/+jVMhhaqNeP+k/hxPhgKOmHAbzF
XEs4tfc5dN5i/qPbtmH0nbkHWSyUys9bAxCoSxz2Mpl0EnspS75pTUdQ1bAoba1/
u0TDecArtkPVHdnzPBtpDDRKCQpghtcRFpWzuKZZt/BynqJfjHjYskHpgQKBgQCG
E2yvBadvLTyCXGqQUO54XktLKxaKfv9iaVDPnPugCqBadG+ujX0phXb4u4Kndrd1
Mj7D8KHnIjHZ8yuwxYrCfig9B9NpuL2/0Si00myq3JdfHhocPvMswk3d25Wf2spj
Al2pNElx6F4LlXnEz6UBS3rRsEBNV761scTo2KOPYQKBgCx8ee5U+mDxyrKmgCcq
y1FCwygfUT4Kzd8I8z0IU0vvw4BegGvH7XIxKFBS5LbKLCMnU5X/gD81VHOc+A2c
UoZldU0/Xyzcjypjx5UURenFi8GnjmQTtkppy2er6kSnwSJ1S5atDMlTlKdhxbbc
v7aKng5qCemoTA6DgDsDkMeE
-----END PRIVATE KEY-----";

const APPLE_TEST_KEY_ID: &str = "apple-test-key-1";

/// The published key set the fake provider serves, matching [`APPLE_TEST_SIGNING_KEY`].
fn apple_test_jwks() -> JwkSet {
    serde_json::from_value(json!({
        "keys": [{
            "kty": "RSA",
            "kid": APPLE_TEST_KEY_ID,
            "use": "sig",
            "alg": "RS256",
            "n": "rF-THfT-H0i5uzGfYUF0EpgkEXEMXQockH6Lpu2Q-o4kU1s00zCV1nNhxKS9ZmtaWNm2hHjRb_kWtdgtiiuVjtwLN0rbseyZa17fkVqtzb1TcBnR7nqNXMVtp-crDbTqCP_dn5zZovWSkdkexFXf9S56B2p_bkFY3MVHq1qqAfNOQ7I99fx-W11GyFw9HG6GVr4jokMuz9IaoLts2J5HNnoQ54k5AnNqzTcSBhjXryV5jb9JWnv0DIwZn__lpmRRwBRLmV9r4_MGzFjN-qrU3vkBG4OGu2KEkFP08w_618XrsW3tb3lBdpAQFpm_J6gl8zUn8uTHyIXdEmtVpOk6kQ",
            "e": "AQAB"
        }]
    }))
    .expect("test key set parses")
}

/// Deterministic key-set source standing in for Apple's published JWKS endpoint.
#[derive(Default)]
struct TestJwksProvider {
    fetches: Mutex<usize>,
}

impl JwksProvider for TestJwksProvider {
    fn fetch(&self, uri: &str) -> Result<JwkSet, studio_oauth::OAuthError> {
        assert_eq!(uri, "https://appleid.apple.com/auth/keys");
        *self.fetches.lock() += 1;
        Ok(apple_test_jwks())
    }
}

/// Sign an Apple-shaped ID token bound to the nonce the host generated for this request.
fn apple_test_id_token(client_id: &str, nonce: &str) -> String {
    let now = i64::try_from(jsonwebtoken::get_current_timestamp()).unwrap();
    encode(
        &Header {
            alg: Algorithm::RS256,
            kid: Some(String::from(APPLE_TEST_KEY_ID)),
            ..Header::new(Algorithm::RS256)
        },
        &json!({
            "iss": "https://appleid.apple.com",
            "aud": client_id,
            "exp": now + 3600,
            "iat": now,
            "sub": "001234.abcdef0123456789abcdef",
            "nonce": nonce,
            "email": "avery@example.test",
            // Apple states verification as a string, not a boolean.
            "email_verified": "true",
        }),
        &EncodingKey::from_rsa_pem(APPLE_TEST_SIGNING_KEY.as_bytes()).unwrap(),
    )
    .expect("test ID token signs")
}
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
    /// The nonce the host generated for the current request, if the provider needed one.
    ///
    /// The fake upstream reads this to sign a token bound to it, which is what makes the
    /// nonce round trip a real assertion rather than a matching pair of literals.
    nonces: Arc<Mutex<Vec<String>>>,
    /// One-time identity payload the provider would send only on a first authorization.
    first_authorization_user: Option<String>,
}

impl CallbackBrowser {
    fn join_callbacks(&self) {
        for handle in self.callback_threads.lock().drain(..) {
            handle.join().unwrap().unwrap();
        }
    }

    fn last_nonce(&self) -> String {
        self.nonces
            .lock()
            .last()
            .cloned()
            .expect("authorization URL carried a nonce")
    }
}

impl BrowserHandoff for CallbackBrowser {
    fn open(&self, authorization_url: &str) -> Result<(), OAuthError> {
        self.authorization_urls
            .lock()
            .push(authorization_url.to_owned());
        let redirect = query_value(authorization_url, "redirect_uri");
        let state = query_value(authorization_url, "state");
        let form_post = optional_query_value(authorization_url, "response_mode").as_deref()
            == Some("form_post");
        if let Some(nonce) = optional_query_value(authorization_url, "nonce") {
            self.nonces.lock().push(nonce);
        }
        let user = self.first_authorization_user.clone();
        let handle = std::thread::spawn(move || {
            send_loopback_callback(&redirect, &state, form_post, user.as_deref())
        });
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

fn optional_query_value(url: &str, name: &str) -> Option<String> {
    url.split_once('?')
        .map(|(_, query)| query)
        .unwrap_or_default()
        .split('&')
        .find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (decode_component(key) == name).then(|| decode_component(value))
        })
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

fn send_loopback_callback(
    redirect: &str,
    state: &str,
    form_post: bool,
    one_time_user: Option<&str>,
) -> Result<(), String> {
    let authority_and_path = redirect
        .strip_prefix("http://")
        .ok_or_else(|| String::from("redirect was not HTTP loopback"))?;
    let (authority, path) = authority_and_path
        .split_once('/')
        .ok_or_else(|| String::from("redirect path missing"))?;
    let address = authority
        .parse::<SocketAddr>()
        .map_err(|_| String::from("loopback address invalid"))?;
    // Generous on purpose: this callback crosses a real socket and a thread boundary, and a
    // workspace run puts many test binaries in parallel. A tight deadline here surfaces as a
    // flake under load rather than as a signal about the code under test.
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(15))
        .map_err(|_| String::from("loopback callback connection failed"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|_| String::from("loopback callback timeout could not be set"))?;

    if form_post {
        let mut body = format!("code=one-time-code&state={state}");
        if let Some(user) = one_time_user {
            body.push_str("&user=");
            body.push_str(&percent_encode_component(user));
        }
        write!(
            stream,
            "POST /{path} HTTP/1.1\r\nHost: {authority}\r\nContent-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    } else {
        write!(
            stream,
            "GET /{path}?code=one-time-code&state={state} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
        )
    }
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

fn percent_encode_component(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(byte));
        } else {
            output.push('%');
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    output
}

struct TestHttpsClient {
    /// Nonces the host generated, captured from the authorization URL it opened.
    nonces: Arc<Mutex<Vec<String>>>,
}

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
        route(&request, &self.nonces)
    }

    fn open_stream(
        &self,
        _request: OutgoingRequest,
        _limits: TransportLimits,
    ) -> Result<Box<dyn ByteStream>, TransportError> {
        Err(TransportError::ConnectionFailure)
    }
}

/// Deterministic upstream routing for the maintained providers under test.
fn route(
    request: &OutgoingRequest,
    nonces: &Mutex<Vec<String>>,
) -> Result<IncomingResponse, TransportError> {
    github_route(request)
        .or_else(|| google_route(request))
        .or_else(|| facebook_route(request))
        .or_else(|| apple_route(request, nonces))
        .ok_or(TransportError::ConnectionFailure)
}

/// Decode a form-encoded request body without taking ownership of it.
fn form_body(request: &OutgoingRequest) -> String {
    String::from_utf8(request.body.as_deref().unwrap().to_vec()).unwrap()
}

fn github_route(request: &OutgoingRequest) -> Option<IncomingResponse> {
    let response = match (request.method, request.url.as_str()) {
        (HttpMethod::Post, "https://github.com/login/oauth/access_token") => {
            let body = form_body(request);
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
                header(request, "authorization"),
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
                header(request, "authorization"),
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
            let authorization = header(request, "authorization");
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
        _ => return None,
    };
    Some(response)
}

fn google_route(request: &OutgoingRequest) -> Option<IncomingResponse> {
    let response = match (request.method, request.url.as_str()) {
        (HttpMethod::Post, "https://oauth2.googleapis.com/token") => {
            let body = form_body(request);
            assert!(body.contains("client_secret=host-only-client-secret-sentinel"));
            let client_id = body
                .split("client_id=")
                .nth(1)
                .and_then(|rest| rest.split('&').next())
                .expect("client id present in token exchange")
                .to_owned();
            if body.contains("grant_type=authorization_code") {
                assert!(body.contains("code_verifier="));
                response(
                    200,
                    &json!({
                        "access_token": format!("host-only-google-token-{client_id}"),
                        "refresh_token": "host-only-refresh-token-sentinel",
                        "scope": "openid email profile",
                        "expires_in": 3600
                    }),
                )
            } else {
                assert!(body.contains("grant_type=refresh_token"));
                assert!(body.contains("refresh_token=host-only-refresh-token-sentinel"));
                // Google rotates the access token and returns no new refresh token.
                response(
                    200,
                    &json!({
                        "access_token": format!("host-only-google-token-{client_id}"),
                        "scope": "openid email profile",
                        "expires_in": 3600
                    }),
                )
            }
        }
        (HttpMethod::Get, "https://openidconnect.googleapis.com/v1/userinfo") => {
            // The accept media type comes from the descriptor, not the adapter.
            assert_eq!(header(request, "accept"), "application/json");
            // A client id ending in `-unverified` models an account Google has not verified, so
            // the descriptor's trust rule must withhold the address it still returns.
            let verified = !header(request, "authorization").ends_with("client-id-unverified");
            response(
                200,
                &json!({
                    "sub": "107346332299",
                    "name": "Avery Rivera",
                    "email": "avery@example.test",
                    "email_verified": verified,
                    "picture": "https://avatars.example/avery.png",
                    "profile": "https://profiles.example/avery"
                }),
            )
        }
        _ => return None,
    };
    Some(response)
}

fn facebook_route(request: &OutgoingRequest) -> Option<IncomingResponse> {
    let response = match (request.method, request.url.as_str()) {
        (HttpMethod::Post, "https://graph.facebook.com/v21.0/oauth/access_token") => {
            let body = form_body(request);
            assert!(body.contains("grant_type=authorization_code"));
            assert!(body.contains("code_verifier="));
            assert!(body.contains("client_secret=host-only-client-secret-sentinel"));
            let client_id = body
                .split("client_id=")
                .nth(1)
                .and_then(|rest| rest.split('&').next())
                .expect("client id present in token exchange");
            // Echo the client id into the opaque access token so the profile call can model the
            // two upstream cases through the host exactly as a real token would.
            response(
                200,
                &json!({
                    "access_token": format!("host-only-facebook-token-{client_id}"),
                    "token_type": "bearer",
                    "expires_in": 5_183_944
                }),
            )
        }
        (
            HttpMethod::Get,
            "https://graph.facebook.com/v21.0/me?fields=id,name,email,picture,link",
        ) => {
            let token = header(request, "authorization");
            assert!(
                token.starts_with("Bearer host-only-facebook-token-"),
                "profile call carried {token}"
            );
            // A client id ending in `-verified` models an account that proved its address, the
            // one case where the host must surface the email rather than withhold it.
            let provider_supplied = !token.ends_with("client-id-verified");
            response(
                200,
                &json!({
                    "id": "10841234567890123",
                    "name": "Sam Okafor",
                    "email": "sam@example.test",
                    "oauth_provided_email": provider_supplied,
                    "picture": { "data": { "url": "https://cdn.example/sam.png" } },
                    "link": "https://facebook.example/sam"
                }),
            )
        }
        _ => return None,
    };
    Some(response)
}

fn apple_route(request: &OutgoingRequest, nonces: &Mutex<Vec<String>>) -> Option<IncomingResponse> {
    let (HttpMethod::Post, "https://appleid.apple.com/auth/token") =
        (request.method, request.url.as_str())
    else {
        return None;
    };
    let body = form_body(request);
    assert!(body.contains("client_secret=host-only-client-secret-sentinel"));
    let client_id = body
        .split("client_id=")
        .nth(1)
        .and_then(|rest| rest.split('&').next())
        .expect("client id present in token exchange")
        .to_owned();
    // The token is signed with the nonce the host actually generated for this request, so the
    // host's nonce assertion is exercised end to end rather than against a fixed literal.
    let nonce = nonces.lock().last().cloned().unwrap_or_default();
    // A client id ending in `-replayed` models a token captured from a different authorization:
    // correctly signed, right issuer and audience, but bound to a nonce this host never sent.
    let nonce = if client_id.ends_with("-replayed") {
        format!("{nonce}-from-another-session")
    } else {
        nonce
    };
    let document = if body.contains("grant_type=authorization_code") {
        assert!(body.contains("code_verifier="));
        assert!(body.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A"));
        json!({
            "access_token": "host-only-access-token-sentinel",
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": "host-only-refresh-token-sentinel",
            "id_token": apple_test_id_token(&client_id, &nonce),
        })
    } else {
        assert!(body.contains("grant_type=refresh_token"));
        json!({
            "access_token": "host-only-access-token-sentinel",
            "token_type": "Bearer",
            "expires_in": 3600,
            "id_token": apple_test_id_token(&client_id, &nonce),
        })
    };
    Some(response(200, &document))
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
    manager_for(
        browser,
        "github",
        "github-client-id",
        ProviderRegistry::github(),
    )
    .0
}

fn manager_for(
    browser: &Arc<CallbackBrowser>,
    provider: &str,
    client_id: &str,
    registry: ProviderRegistry,
) -> (OAuthManager, Arc<TestJwksProvider>) {
    let secret_name = format!("{provider}.oauth.client_secret");
    let secret_purpose = format!("{provider} OAuth client configuration");
    let protected_store = ProtectedSecretStore::new(MemoryBackend::default());
    let principal = principal();
    let client_secret =
        ProtectedSecretKey::new(secret_name.clone(), secret_purpose.clone()).unwrap();
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
    let jwks = Arc::new(TestJwksProvider::default());
    let package =
        ProviderPackage::latest(provider, client_id).with_client_secret(ProtectedSecretReference {
            name: secret_name,
            purpose: secret_purpose,
        });
    let mut manager = OAuthManager::new(
        registry,
        [package],
        Arc::new(token_store),
        browser.clone(),
        Arc::new(TcpLoopbackListener),
        Arc::new(OsEntropy),
        Arc::new(HttpsOAuthTransport::new(Arc::new(TestHttpsClient {
            nonces: Arc::clone(&browser.nonces),
        }))),
        Arc::clone(&jwks) as Arc<dyn JwksProvider>,
    );
    manager.set_callback_timeout(Duration::from_secs(30));
    (manager, jwks)
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

#[test]
fn google_sign_in_threads_descriptor_params_maps_oidc_claims_and_refreshes() {
    let browser = Arc::new(CallbackBrowser::default());
    let manager = manager_for(
        &browser,
        "google",
        "google-client-id",
        ProviderRegistry::maintained(),
    )
    .0;

    // GitHub and Google share one catalog and one adapter; nothing provider-specific is wired
    // in the test or the manager.
    let signed_in = manager.sign_in("google");
    browser.join_callbacks();
    assert!(
        signed_in.is_success(),
        "google sign-in failed: {signed_in:?}"
    );
    assert_eq!(signed_in.status, studio_oauth::OAuthStatus::Authenticated);

    let claims = signed_in.claims.as_ref().unwrap();
    assert_eq!(claims.subject, "107346332299");
    assert_eq!(claims.login, None);
    assert_eq!(claims.display_name.as_deref(), Some("Avery Rivera"));
    assert_eq!(claims.email.as_deref(), Some("avery@example.test"));
    assert_eq!(
        claims.profile_url.as_deref(),
        Some("https://profiles.example/avery")
    );

    let authorization_url = browser.authorization_urls.lock()[0].clone();
    assert!(authorization_url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
    assert!(authorization_url.contains("scope=openid%20email%20profile"));
    // The descriptor's params must reach the provider or Google returns a session that cannot
    // be refreshed.
    assert!(authorization_url.contains("access_type=offline"));
    assert!(authorization_url.contains("include_granted_scopes=true"));
    // Host-owned PKCE is still present alongside them.
    assert!(authorization_url.contains("code_challenge="));

    let mut credential = CaptureCredential::default();
    studio_net::credential::OAuthSessionResolver::inject_session(
        &manager,
        "google",
        "google.profile",
        &mut credential,
    )
    .unwrap();
    assert_eq!(credential.0, b"host-only-google-token-google-client-id");

    // Google declares no revocation endpoint, so refresh is the session-extension path.
    let refreshed = manager.refresh("google");
    assert!(
        refreshed.is_success(),
        "google refresh failed: {refreshed:?}"
    );
    assert_eq!(refreshed.status, studio_oauth::OAuthStatus::Authenticated);

    let revoked = manager.revoke("google");
    assert!(revoked.is_success());
    assert_eq!(revoked.status, studio_oauth::OAuthStatus::Revoked);
    let mut after_revoke = CaptureCredential::default();
    assert!(
        studio_net::credential::OAuthSessionResolver::inject_session(
            &manager,
            "google",
            "google.profile",
            &mut after_revoke,
        )
        .is_err()
    );
    assert!(after_revoke.0.is_empty());
}

#[test]
fn google_withholds_an_address_it_has_not_verified() {
    let browser = Arc::new(CallbackBrowser::default());
    let manager = manager_for(
        &browser,
        "google",
        "google-client-id-unverified",
        ProviderRegistry::maintained(),
    )
    .0;

    let signed_in = manager.sign_in("google");
    browser.join_callbacks();
    assert!(signed_in.is_success());
    let claims = signed_in.claims.as_ref().unwrap();
    // Google still returns an address; the host refuses to vouch for it because
    // `email_verified` is false.
    assert_eq!(claims.email, None);
    assert_eq!(claims.subject, "107346332299");
}

#[test]
fn google_surfaces_a_verified_address() {
    let browser = Arc::new(CallbackBrowser::default());
    let manager = manager_for(
        &browser,
        "google",
        "google-client-id",
        ProviderRegistry::maintained(),
    )
    .0;

    let signed_in = manager.sign_in("google");
    browser.join_callbacks();
    assert!(signed_in.is_success());
    // Same descriptor and claim path as the test above; only `email_verified` differs. The
    // address surviving here proves the withheld one was the trust rule, not a broken mapping.
    assert_eq!(
        signed_in.claims.as_ref().unwrap().email.as_deref(),
        Some("avery@example.test")
    );
}

#[test]
fn facebook_sign_in_maps_nested_claims_and_withholds_provider_supplied_email() {
    let browser = Arc::new(CallbackBrowser::default());
    let manager = manager_for(
        &browser,
        "facebook",
        "facebook-client-id",
        ProviderRegistry::maintained(),
    )
    .0;

    let signed_in = manager.sign_in("facebook");
    browser.join_callbacks();
    assert!(
        signed_in.is_success(),
        "facebook sign-in failed: {signed_in:?}"
    );

    let claims = signed_in.claims.as_ref().unwrap();
    assert_eq!(claims.subject, "10841234567890123");
    assert_eq!(claims.display_name.as_deref(), Some("Sam Okafor"));
    // `picture` is a nested object in Graph, reached through a dotted claim path.
    assert_eq!(
        claims.avatar_url.as_deref(),
        Some("https://cdn.example/sam.png")
    );
    assert_eq!(
        claims.profile_url.as_deref(),
        Some("https://facebook.example/sam")
    );
    // `oauth_provided_email: true` means Facebook never verified this address, so the host must
    // not hand it to a guest as a verified claim.
    assert_eq!(claims.email, None);

    let authorization_url = browser.authorization_urls.lock()[0].clone();
    assert!(authorization_url.starts_with("https://www.facebook.com/v21.0/dialog/oauth?"));
    assert!(authorization_url.contains("scope=email%20public_profile"));
    assert!(authorization_url.contains("code_challenge="));

    let mut credential = CaptureCredential::default();
    studio_net::credential::OAuthSessionResolver::inject_session(
        &manager,
        "facebook",
        "facebook.profile",
        &mut credential,
    )
    .unwrap();
    assert_eq!(credential.0, b"host-only-facebook-token-facebook-client-id");

    // Facebook issues no refresh token for this flow, so refresh must fail closed.
    let refresh = manager.refresh("facebook");
    assert_eq!(
        refresh.code_str(),
        OAuthErrorCode::RefreshUnavailable.as_str()
    );

    let revoked = manager.revoke("facebook");
    assert!(revoked.is_success());
    assert_eq!(revoked.status, studio_oauth::OAuthStatus::Revoked);
    let mut after_revoke = CaptureCredential::default();
    assert!(
        studio_net::credential::OAuthSessionResolver::inject_session(
            &manager,
            "facebook",
            "facebook.profile",
            &mut after_revoke,
        )
        .is_err()
    );
    assert!(after_revoke.0.is_empty());
}

#[test]
fn facebook_surfaces_the_email_when_the_provider_vouches_for_it() {
    let browser = Arc::new(CallbackBrowser::default());
    let manager = manager_for(
        &browser,
        "facebook",
        "facebook-client-id-verified",
        ProviderRegistry::maintained(),
    )
    .0;

    let signed_in = manager.sign_in("facebook");
    browser.join_callbacks();
    assert!(signed_in.is_success());
    // Same descriptor, same mapping, same flag path: only the upstream flag value differs. The
    // address surviving here proves the first test's `None` came from the flag, not a broken
    // claim path.
    assert_eq!(
        signed_in.claims.as_ref().unwrap().email.as_deref(),
        Some("sam@example.test")
    );
}

#[test]
fn apple_sign_in_verifies_the_id_token_and_composes_the_one_time_name() {
    // Apple returns the account name exactly once, as a JSON `user` field in a form POST. The
    // host only gets it if the descriptor's response mode reached the authorization URL.
    let browser = Arc::new(CallbackBrowser {
        first_authorization_user: Some(
            json!({"name": {"firstName": "Avery", "lastName": "Rivera"}}).to_string(),
        ),
        ..CallbackBrowser::default()
    });
    let (manager, jwks) = manager_for(
        &browser,
        "apple",
        "apple-client-id",
        ProviderRegistry::maintained(),
    );

    let signed_in = manager.sign_in("apple");
    browser.join_callbacks();
    assert!(
        signed_in.is_success(),
        "apple sign-in failed: {signed_in:?}"
    );

    let claims = signed_in.claims.as_ref().unwrap();
    // The subject and address come from the verified token, never from the unsigned form body.
    assert_eq!(claims.subject, "001234.abcdef0123456789abcdef");
    assert_eq!(claims.email.as_deref(), Some("avery@example.test"));
    // The name arrived only in the one-time payload and is composed from two separate claims.
    assert_eq!(claims.display_name.as_deref(), Some("Avery Rivera"));

    let authorization_url = browser.authorization_urls.lock()[0].clone();
    assert!(authorization_url.starts_with("https://appleid.apple.com/auth/authorize?"));
    assert!(authorization_url.contains("response_mode=form_post"));
    assert!(authorization_url.contains("scope=name%20email"));
    // The nonce is host-generated and must be present in the request the token is bound to.
    assert!(!browser.last_nonce().is_empty());

    // One key-set fetch serves the sign-in and the refresh below.
    assert_eq!(*jwks.fetches.lock(), 1);
    let refreshed = manager.refresh("apple");
    assert!(
        refreshed.is_success(),
        "apple refresh failed: {refreshed:?}"
    );
    assert_eq!(*jwks.fetches.lock(), 1);
    assert_eq!(
        refreshed.claims.as_ref().unwrap().subject,
        "001234.abcdef0123456789abcdef"
    );

    let mut credential = CaptureCredential::default();
    studio_net::credential::OAuthSessionResolver::inject_session(
        &manager,
        "apple",
        "apple.profile",
        &mut credential,
    )
    .unwrap();
    assert_eq!(credential.0, ACCESS_TOKEN);

    let revoked = manager.revoke("apple");
    assert!(revoked.is_success());
    assert_eq!(revoked.status, studio_oauth::OAuthStatus::Revoked);
}

#[test]
fn apple_rejects_an_id_token_bound_to_a_different_nonce() {
    let browser = Arc::new(CallbackBrowser::default());
    // This client id makes the fake upstream sign the token with a nonce the host never sent,
    // which is exactly what a token captured from another session looks like.
    let (manager, _jwks) = manager_for(
        &browser,
        "apple",
        "apple-client-id-replayed",
        ProviderRegistry::maintained(),
    );

    let result = manager.sign_in("apple");
    browser.join_callbacks();
    assert!(!result.is_success());
    assert_eq!(result.code_str(), OAuthErrorCode::IdTokenInvalid.as_str());
    assert_eq!(result.status, studio_oauth::OAuthStatus::Unavailable);
    assert_eq!(
        manager.status("apple"),
        studio_oauth::OAuthStatus::SignedOut
    );
}
