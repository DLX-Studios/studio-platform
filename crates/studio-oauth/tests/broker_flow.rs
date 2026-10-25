#![allow(missing_docs)]

//! End-to-end proof that a provider redirect survives a hosted redirect broker.
//!
//! The provider here answers with a form POST and sends the user's name exactly once, the way
//! Apple does. The browser is the real thing: it hands the provider's result to the broker,
//! follows the `302` the broker answers, and lands on the loopback listener. Nothing about the
//! flow is short-circuited: the code, the state, the one-time payload, and the token exchange
//! all travel the path they travel in production.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{Value, json};
use studio_net::{
    HttpsClient, IncomingResponse, OutgoingRequest, TransportError, TransportLimits,
    declaration::HttpMethod, transport::ByteStream,
};
use studio_oauth::{
    BrowserHandoff, ClaimPath, ClaimSource, ClientAuthentication, EmailTrust, HttpsJwksProvider,
    HttpsOAuthTransport, NameComposition, OAuthError, OAuthManager, OsEntropy, ProviderDescriptor,
    ProviderPackage, ProviderQuirks, ProviderRegistry, RedirectBroker, ResponseMode,
    TcpLoopbackListener, TokenEndpointAuthentication,
};
use studio_oauth_broker::{BrokerConfig, serve};
use studio_security::{
    ApplicationEnvironment, CredentialBackend, CredentialBackendError, CredentialBytes,
    CredentialLocator, PluginPrincipal, ProtectedSecretStore, TrustMode,
};

/// The secret the application and the broker share.
const BROKER_SECRET: &[u8] = b"a-shared-broker-secret-of-at-least-thirty-two-bytes";
/// The application identifier the developer registers with the provider.
const BROKER_APPLICATION_ID: &str = "com.example.formpost";
/// The public origin the developer registers with the provider.
const BROKER_BASE_URL: &str = "https://broker.test";
/// The provider's one-time identity payload, the way Apple sends it.
const ONE_TIME_PAYLOAD: &str = r#"{"name":{"firstName":"Avery","lastName":"River"}}"#;

/// The outcome of one completed browser redirect.
type RedirectOutcome = Result<(), String>;

/// A provider that answers with a form POST and names the user exactly once.
fn form_post_descriptor() -> ProviderDescriptor {
    let mut descriptor = ProviderDescriptor::github();
    descriptor.id = String::from("formpost");
    descriptor.version = String::from("1.0.0");
    descriptor.authorization_endpoint = String::from("https://formpost.example/authorize");
    descriptor.token_endpoint = String::from("https://formpost.example/token");
    descriptor.revocation_endpoint = None;
    descriptor.profile.endpoint = String::from("https://formpost.example/user");
    descriptor.profile.accept = String::from("application/json");
    descriptor.profile.subject = ClaimPath::new("id");
    descriptor.profile.login = Some(ClaimPath::new("handle"));
    descriptor.profile.display_name = Some(ClaimPath::new("name"));
    descriptor.profile.email = Some(ClaimPath::new("email"));
    descriptor.profile.avatar_url = None;
    descriptor.profile.profile_url = None;
    descriptor.profile.email_fallback = None;
    descriptor.profile.one_time_payload_field = Some(String::from("user"));
    descriptor.profile.name_composition = Some(NameComposition {
        first: ClaimPath::new("name.firstName"),
        last: ClaimPath::new("name.lastName"),
        separator: String::from(" "),
    });
    descriptor.claim_source = ClaimSource::ProfileEndpoint;
    descriptor.issuer = None;
    descriptor.jwks_uri = None;
    descriptor.response_mode = ResponseMode::FormPost;
    descriptor.scopes = vec![String::from("email")];
    descriptor.client_authentication = ClientAuthentication::Pkce;
    descriptor.token_endpoint_authentication = TokenEndpointAuthentication::RequestBody;
    descriptor.quirks = ProviderQuirks {
        no_refresh_tokens: true,
        private_email_fallback: false,
        email_trust: EmailTrust::Unstated,
    };
    descriptor
}

/// The deterministic upstream the form-post provider answers on.
struct FormPostProvider;

impl HttpsClient for FormPostProvider {
    fn execute(
        &self,
        request: OutgoingRequest,
        _limits: TransportLimits,
    ) -> Result<IncomingResponse, TransportError> {
        match (request.method, request.url.as_str()) {
            (HttpMethod::Post, "https://formpost.example/token") => {
                let body = String::from_utf8(request.body.as_deref().unwrap_or_default().to_vec())
                    .expect("token request body is text");
                // The exchange presents the redirect URI the provider was registered with, which
                // is the broker's, not the loopback listener's.
                assert!(
                    body.contains(
                        "redirect_uri=https%3A%2F%2Fbroker.test%2Fcb%2Fcom.example.formpost"
                    ),
                    "exchange sent the wrong redirect URI: {body}"
                );
                assert!(body.contains("code=one-time-code"));
                assert!(body.contains("code_verifier="));
                Ok(response(
                    200,
                    &json!({
                        "access_token": "form-post-access-token",
                        "token_type": "Bearer",
                    }),
                ))
            }
            (HttpMethod::Get, "https://formpost.example/user") => Ok(response(
                200,
                &json!({
                    "id": 42,
                    "handle": "formpost-user",
                    "email": "formpost@example.test",
                }),
            )),
            _ => Err(TransportError::ConnectionFailure),
        }
    }

    fn open_stream(
        &self,
        _request: OutgoingRequest,
        _limits: TransportLimits,
    ) -> Result<Box<dyn ByteStream>, TransportError> {
        Err(TransportError::ConnectionFailure)
    }
}

/// A JSON response from the deterministic upstream.
fn response(status: u16, value: &Value) -> IncomingResponse {
    IncomingResponse {
        status,
        media_type: Some(String::from("application/json")),
        body: serde_json::to_vec(value).expect("response body serializes"),
    }
}

/// The browser the host hands the authorization URL to.
///
/// It behaves the way a browser does: the provider's result goes to the registered redirect
/// URI, the broker answers with a `302`, and the browser follows it to the loopback listener.
struct BrokerBrowser {
    /// The public origin the provider was given.
    base_url: String,
    /// The local address that origin resolves to in this test.
    address: SocketAddr,
    /// Where the completed redirect is reported.
    outcome: Sender<RedirectOutcome>,
    /// The authorization URLs the host opened.
    authorization_urls: Mutex<Vec<String>>,
}

impl BrowserHandoff for BrokerBrowser {
    fn open(&self, authorization_url: &str) -> Result<(), OAuthError> {
        self.authorization_urls
            .lock()
            .push(authorization_url.to_owned());
        let registered = query_value(authorization_url, "redirect_uri");
        let state = query_value(authorization_url, "state");
        let outcome = self.outcome.clone();
        let base_url = self.base_url.clone();
        let address = self.address;
        std::thread::spawn(move || {
            let result = follow_the_redirect(&base_url, address, &registered, &state);
            let _ = outcome.send(result);
        });
        Ok(())
    }
}

/// Hand the provider's result to the broker and follow the redirect it answers with.
fn follow_the_redirect(
    base_url: &str,
    address: SocketAddr,
    registered: &str,
    state: &str,
) -> RedirectOutcome {
    let path = registered
        .strip_prefix(base_url)
        .ok_or_else(|| format!("redirect URI {registered} does not name the broker"))?;
    let target = format!("http://{address}{path}");
    let body = format!(
        "code=one-time-code&state={}&user={}",
        percent_encode(state),
        percent_encode(ONE_TIME_PAYLOAD)
    );
    let answer = http_request("POST", &target, &body)?;
    if answer.status != 302 {
        return Err(format!("broker answered {}", answer.status));
    }
    let location = answer
        .header("location")
        .ok_or_else(|| String::from("broker answered without a location"))?;
    if !location.starts_with("http://127.0.0.1:") {
        return Err(format!("broker answered with a foreign target: {location}"));
    }
    let landed = http_request("GET", location, "")?;
    if landed.status != 200 {
        return Err(format!("loopback listener answered {}", landed.status));
    }
    Ok(())
}

/// One HTTP response, reduced to what a browser reads.
struct HttpResponse {
    /// The status code.
    status: u16,
    /// The response headers, keyed by lowercase name.
    headers: HashMap<String, String>,
}

impl HttpResponse {
    /// One header value, by lowercase name.
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
}

/// Send one HTTP request and read the response.
fn http_request(method: &str, url: &str, body: &str) -> Result<HttpResponse, String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("test requests use plain HTTP: {url}"))?;
    let (authority, path) = rest
        .split_once('/')
        .ok_or_else(|| format!("request has no path: {url}"))?;
    let mut stream = TcpStream::connect(authority).map_err(|error| error.to_string())?;
    let mut request = format!("{method} /{path} HTTP/1.1\r\nHost: {authority}\r\n");
    if method == "POST" {
        request.push_str("Content-Type: application/x-www-form-urlencoded\r\n");
        request.push_str("Content-Length: ");
        request.push_str(&body.len().to_string());
        request.push_str("\r\n");
    }
    request.push_str("Connection: close\r\n\r\n");
    request.push_str(body);
    stream
        .write_all(request.as_bytes())
        .map_err(|error| error.to_string())?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|error| error.to_string())?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, _) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| String::from("response has no head"))?;
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| String::from("response has no status"))?;
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    Ok(HttpResponse { status, headers })
}

/// The value of one query parameter in an authorization URL.
fn query_value(url: &str, name: &str) -> String {
    url.split_once('?')
        .expect("authorization URL has a query")
        .1
        .split('&')
        .find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (percent_decode(key) == name).then(|| percent_decode(value))
        })
        .expect("authorization URL contains the parameter")
}

/// Percent-encode a value for a form-encoded payload.
fn percent_encode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(output, "%{byte:02X}").expect("writing into a String cannot fail");
        }
    }
    output
}

/// Percent-decode a value from a form-encoded payload.
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2]))
        {
            output.push(high * 16 + low);
            index += 3;
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

/// The value of a hexadecimal digit.
fn hex_digit(value: u8) -> Option<u8> {
    u8::try_from((value as char).to_digit(16)?).ok()
}

/// The principal the protected store scopes this session to.
fn principal() -> PluginPrincipal {
    PluginPrincipal::new_verified(
        "formpost.example",
        "formpost-signing-key",
        "com.example.formpost",
        [7; 32],
        [9; 16],
        TrustMode::Production,
    )
    .expect("principal is valid")
}

/// An in-memory credential backend for the protected store.
#[derive(Default)]
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
        self.records.lock().remove(locator);
        Ok(())
    }
}

/// Start the broker on a loopback port and hand back its address and a stop handle.
fn start_broker() -> (SocketAddr, JoinHandle<()>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("broker binds a loopback port");
    let address = listener.local_addr().expect("listener has an address");
    let config = BrokerConfig::new(BROKER_SECRET, &[]).expect("broker configuration is valid");
    let handle = std::thread::spawn(move || {
        let _ = serve(listener, &config);
    });
    (address, handle)
}

#[test]
fn a_form_post_redirect_survives_the_broker() {
    let (address, _broker) = start_broker();
    let (outcome, finished): (Sender<RedirectOutcome>, Receiver<RedirectOutcome>) = mpsc::channel();
    let browser = Arc::new(BrokerBrowser {
        base_url: String::from(BROKER_BASE_URL),
        address,
        outcome,
        authorization_urls: Mutex::new(Vec::new()),
    });

    let mut registry = ProviderRegistry::maintained();
    registry
        .install_scope_policy("formpost", &["email"])
        .expect("scope ceiling installs");
    registry
        .register(form_post_descriptor())
        .expect("descriptor is admitted");

    let protected_store = ProtectedSecretStore::new(MemoryBackend::default());
    let principal = principal();
    let token_store = studio_oauth::ProtectedOAuthTokenStore::new(
        protected_store,
        principal,
        ApplicationEnvironment::Production,
    );
    let broker = RedirectBroker::new(BROKER_BASE_URL, BROKER_APPLICATION_ID, BROKER_SECRET)
        .expect("broker configuration is valid");
    let mut manager = OAuthManager::new(
        registry,
        [ProviderPackage::latest("formpost", "formpost-client-id")],
        Arc::new(token_store),
        browser.clone(),
        Arc::new(TcpLoopbackListener),
        Arc::new(OsEntropy),
        Arc::new(HttpsOAuthTransport::new(Arc::new(FormPostProvider))),
        Arc::new(HttpsJwksProvider::new(Arc::new(FormPostProvider))),
    )
    .with_redirect_broker(broker);
    manager.set_callback_timeout(Duration::from_secs(30));

    let signed_in = manager
        .try_sign_in("formpost")
        .expect("sign-in succeeds through the broker");

    finished
        .recv_timeout(Duration::from_secs(10))
        .expect("the browser finished the redirect")
        .expect("the redirect completed");

    // The provider was given the broker's redirect URI, not the loopback listener's.
    let opened = browser
        .authorization_urls
        .lock()
        .first()
        .cloned()
        .expect("a URL was opened");
    assert_eq!(
        query_value(&opened, "redirect_uri"),
        "https://broker.test/cb/com.example.formpost"
    );

    let claims = signed_in.claims.as_ref().expect("claims were mapped");
    assert_eq!(claims.subject, "42");
    assert_eq!(claims.login.as_deref(), Some("formpost-user"));
    assert_eq!(claims.email.as_deref(), Some("formpost@example.test"));
    // The name arrived once, in the provider's form POST, and the broker relayed it.
    assert_eq!(claims.display_name.as_deref(), Some("Avery River"));
}

#[test]
fn a_forged_state_is_refused_by_the_broker() {
    let (address, _broker) = start_broker();

    // A state the broker did not mint names no port the application chose, so the broker
    // refuses it instead of aiming its redirect somewhere of the caller's choosing.
    let forged = "Zm9yZ2VkLXN0YXRlLXdpdGhvdXQtaW5lbnRpZHk";
    let target =
        format!("http://{address}/cb/{BROKER_APPLICATION_ID}?code=one-time-code&state={forged}");
    let answer = http_request("GET", &target, "").expect("the broker answers");
    assert_eq!(answer.status, 400);
    assert!(answer.header("location").is_none());
}
