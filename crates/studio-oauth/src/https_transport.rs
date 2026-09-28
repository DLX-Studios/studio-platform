//! GitHub OAuth protocol adapter over the host's certificate-validating HTTPS client.

use std::{sync::Arc, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use studio_net::{
    HttpsClient, IncomingResponse, OutgoingRequest, ProductionHttpTransport, TransportLimits,
};
use studio_net::{declaration::HttpMethod, transport::HttpTransport};
use zeroize::{Zeroize, Zeroizing};

use crate::{
    CodeExchangeRequest, OAuthError, OAuthErrorCode, OAuthTransport, ProfileRequest,
    RefreshRequest, RevokeRequest, TokenResponse, https_url,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_REQUEST_BYTES: usize = 16 * 1024;

/// GitHub OAuth operations sent through the host's certificate-validating HTTPS client.
///
/// The adapter never logs request bodies or credential headers. The injected client must enforce
/// the supplied transport limits and must not retain credential-bearing requests after `execute`
/// returns.
pub struct GithubHttpsOAuthTransport {
    http: ProductionHttpTransport,
}

impl std::fmt::Debug for GithubHttpsOAuthTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("GithubHttpsOAuthTransport(REDACTED)")
    }
}

impl GithubHttpsOAuthTransport {
    /// Construct the GitHub adapter over a host-provided TLS client.
    #[must_use]
    pub fn new(client: Arc<dyn HttpsClient>) -> Self {
        let limits = TransportLimits {
            max_response_bytes: MAX_RESPONSE_BYTES,
            ..TransportLimits::default()
        };
        Self {
            http: ProductionHttpTransport::new(client, limits),
        }
    }

    fn execute(
        &self,
        method: HttpMethod,
        endpoint: &str,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
        failure: OAuthErrorCode,
    ) -> Result<IncomingResponse, OAuthError> {
        if !https_url(endpoint)
            || body
                .as_ref()
                .is_some_and(|body| body.len() > MAX_REQUEST_BYTES)
        {
            return Err(OAuthError::new(failure));
        }
        let response = self
            .http
            .execute(OutgoingRequest {
                method,
                url: endpoint.to_owned(),
                headers,
                body,
                timeout: REQUEST_TIMEOUT,
            })
            .map_err(|_| OAuthError::new(failure))?;
        if !(200..300).contains(&response.status) || response.body.len() > MAX_RESPONSE_BYTES {
            return Err(OAuthError::new(failure));
        }
        Ok(response)
    }

    fn token_request(
        &self,
        endpoint: &str,
        client_id: &str,
        grant_type: &str,
        parameters: &[(&str, &[u8])],
    ) -> Result<TokenResponse, OAuthError> {
        let mut body = String::new();
        append_form_field(&mut body, "grant_type", grant_type.as_bytes());
        append_form_field(&mut body, "client_id", client_id.as_bytes());
        for (key, value) in parameters {
            append_form_field(&mut body, key, value);
        }
        let response = self.execute(
            HttpMethod::Post,
            endpoint,
            vec![
                (String::from("accept"), String::from("application/json")),
                (
                    String::from("content-type"),
                    String::from("application/x-www-form-urlencoded"),
                ),
            ],
            Some(body.into_bytes()),
            OAuthErrorCode::TokenExchangeFailed,
        )?;
        decode_token_response(response.body)
    }
}

impl OAuthTransport for GithubHttpsOAuthTransport {
    fn exchange_code(&self, request: CodeExchangeRequest<'_>) -> Result<TokenResponse, OAuthError> {
        let verifier = request.verifier;
        let client_secret = request.client_secret;
        let mut parameters = vec![
            ("code", request.code.as_bytes()),
            ("redirect_uri", request.redirect_uri.as_bytes()),
        ];
        if let Some(verifier) = verifier.as_ref() {
            parameters.push(("code_verifier", verifier.as_bytes()));
        }
        if let Some(secret) = client_secret.as_ref() {
            parameters.push(("client_secret", secret.as_bytes()));
        }
        self.token_request(
            request.endpoint,
            request.client_id,
            "authorization_code",
            &parameters,
        )
    }

    fn refresh(&self, request: RefreshRequest<'_>) -> Result<TokenResponse, OAuthError> {
        let client_secret = request.client_secret;
        let mut parameters = vec![("refresh_token", request.refresh_token.as_bytes())];
        if let Some(secret) = client_secret.as_ref() {
            parameters.push(("client_secret", secret.as_bytes()));
        }
        self.token_request(
            request.endpoint,
            request.client_id,
            "refresh_token",
            &parameters,
        )
    }

    fn profile(&self, request: ProfileRequest<'_>) -> Result<Value, OAuthError> {
        let token = std::str::from_utf8(request.access_token.as_bytes())
            .map_err(|_| OAuthError::new(OAuthErrorCode::ProfileFailed))?;
        let response = self.execute(
            HttpMethod::Get,
            request.endpoint,
            vec![
                (
                    String::from("accept"),
                    String::from("application/vnd.github+json"),
                ),
                (String::from("authorization"), format!("Bearer {token}")),
                (String::from("user-agent"), String::from("studio-oauth/1.0")),
            ],
            None,
            OAuthErrorCode::ProfileFailed,
        )?;
        serde_json::from_slice(&response.body)
            .map_err(|_| OAuthError::new(OAuthErrorCode::ProfileFailed))
    }

    fn revoke(&self, request: RevokeRequest<'_>) -> Result<(), OAuthError> {
        let secret = request
            .client_secret
            .ok_or_else(|| OAuthError::new(OAuthErrorCode::RevokeFailed))?;
        let secret = std::str::from_utf8(secret.as_bytes())
            .map_err(|_| OAuthError::new(OAuthErrorCode::RevokeFailed))?;
        let mut credentials = Zeroizing::new(Vec::with_capacity(
            request.client_id.len() + secret.len() + 1,
        ));
        credentials.extend_from_slice(request.client_id.as_bytes());
        credentials.push(b':');
        credentials.extend_from_slice(secret.as_bytes());
        let authorization = format!("Basic {}", STANDARD.encode(&*credentials));
        let access_token = std::str::from_utf8(request.access_token.as_bytes())
            .map_err(|_| OAuthError::new(OAuthErrorCode::RevokeFailed))?;
        let body = serde_json::to_vec(&RevocationBody { access_token })
            .map_err(|_| OAuthError::new(OAuthErrorCode::RevokeFailed))?;
        self.execute(
            HttpMethod::Delete,
            request.endpoint,
            vec![
                (
                    String::from("accept"),
                    String::from("application/vnd.github+json"),
                ),
                (String::from("authorization"), authorization),
                (
                    String::from("content-type"),
                    String::from("application/json"),
                ),
                (String::from("user-agent"), String::from("studio-oauth/1.0")),
            ],
            Some(body),
            OAuthErrorCode::RevokeFailed,
        )?;
        Ok(())
    }
}

#[derive(Serialize)]
struct RevocationBody<'a> {
    access_token: &'a str,
}

#[derive(Deserialize)]
struct TokenDocument {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
}

impl Drop for TokenDocument {
    fn drop(&mut self) {
        self.access_token.zeroize();
        if let Some(refresh) = &mut self.refresh_token {
            refresh.zeroize();
        }
    }
}

fn decode_token_response(body: Vec<u8>) -> Result<TokenResponse, OAuthError> {
    let body = Zeroizing::new(body);
    let mut document: TokenDocument = serde_json::from_slice(&body)
        .map_err(|_| OAuthError::new(OAuthErrorCode::TokenExchangeFailed))?;
    let mut access = Zeroizing::new(std::mem::take(&mut document.access_token).into_bytes());
    let mut refresh = document
        .refresh_token
        .take()
        .map(|token| Zeroizing::new(token.into_bytes()));
    if access.is_empty()
        || access.len() > crate::MAX_TOKEN_BYTES
        || refresh
            .as_ref()
            .is_some_and(|token| token.is_empty() || token.len() > crate::MAX_TOKEN_BYTES)
    {
        return Err(OAuthError::new(OAuthErrorCode::TokenExchangeFailed));
    }
    let access = std::mem::take(&mut *access);
    let refresh = refresh.as_mut().map(|token| std::mem::take(&mut **token));
    let mut response = TokenResponse::new(access, refresh, document.expires_in)?;
    if let Some(scope) = document.scope.take() {
        let scopes = scope
            .split(|character: char| character == ',' || character.is_ascii_whitespace())
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        response = response.with_scopes(scopes)?;
    }
    Ok(response)
}

fn append_form_field(body: &mut String, key: &str, value: &[u8]) {
    if !body.is_empty() {
        body.push('&');
    }
    body.push_str(key);
    body.push('=');
    for byte in value {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            body.push(char::from(*byte));
        } else if *byte == b' ' {
            body.push('+');
        } else {
            use std::fmt::Write as _;
            write!(body, "%{byte:02X}").expect("writing into a String cannot fail");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use studio_net::{
        HttpsClient, IncomingResponse, OutgoingRequest, TransportError, TransportLimits,
        declaration::HttpMethod, transport::ByteStream,
    };

    use crate::{OAuthTransport, RefreshRequest, SecretToken};

    use super::{GithubHttpsOAuthTransport, append_form_field};

    struct RefreshHttpsClient;

    impl HttpsClient for RefreshHttpsClient {
        fn execute(
            &self,
            request: OutgoingRequest,
            limits: TransportLimits,
        ) -> Result<IncomingResponse, TransportError> {
            assert!(limits.is_valid());
            assert_eq!(request.method, HttpMethod::Post);
            assert_eq!(request.url, "https://github.com/login/oauth/access_token");
            let body = String::from_utf8(request.body.unwrap()).unwrap();
            assert!(body.contains("grant_type=refresh_token"));
            assert!(body.contains("client_id=github-client-id"));
            assert!(body.contains("refresh_token=host-only-refresh-token"));
            assert!(body.contains("client_secret=host-only-client-secret"));
            let value = json!({
                "access_token": "rotated-access-token",
                "refresh_token": "rotated-refresh-token",
                "scope": "read:user,user:email",
                "expires_in": 3600
            });
            Ok(IncomingResponse {
                status: 200,
                media_type: Some(String::from("application/json")),
                body: serde_json::to_vec(&value).unwrap(),
            })
        }

        fn open_stream(
            &self,
            _request: OutgoingRequest,
            _limits: TransportLimits,
        ) -> Result<Box<dyn ByteStream>, TransportError> {
            Err(TransportError::ConnectionFailure)
        }
    }

    #[test]
    fn form_fields_encode_separators_and_spaces() {
        let mut body = String::new();
        append_form_field(&mut body, "scope", b"read:user user:email");
        append_form_field(&mut body, "client_secret", b"a&b=c");
        assert_eq!(
            body,
            "scope=read%3Auser+user%3Aemail&client_secret=a%26b%3Dc"
        );
    }

    #[test]
    fn refresh_uses_protected_client_credentials_and_parses_rotation() {
        let transport = GithubHttpsOAuthTransport::new(Arc::new(RefreshHttpsClient));
        let result = transport
            .refresh(RefreshRequest {
                endpoint: "https://github.com/login/oauth/access_token",
                client_id: "github-client-id",
                refresh_token: SecretToken(b"host-only-refresh-token"),
                client_secret: Some(SecretToken(b"host-only-client-secret")),
            })
            .unwrap();
        assert_eq!(format!("{result:?}"), "TokenResponse(REDACTED)");
    }
}
