//! Host-neutral GitHub integration SDK and proof-journey state model.
//!
//! The SDK never owns an access token. GitHub requests travel through the restricted REST guest
//! facade, where the host resolves the provider session at send time. This keeps the same API
//! usable by a Runtime guest, a deterministic test harness, or a future Designer preview.

#![allow(missing_docs)]
#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::large_enum_variant,
    clippy::map_unwrap_or
)]

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use studio_net::credential::OAuthSessionResolver;
use studio_net::declaration::{CredentialSource, HttpMethod, RouteGroupDeclaration};
use studio_net::guest::BrokerRequest;
use studio_net::limits::DeclaredLimits;
use studio_net::{BrokerError, GuestRestApi};
use studio_security::BrokerCredentialSink;
use thiserror::Error;

/// Stable first-party provider identifier.
pub const GITHUB_PROVIDER_ID: &str = "github";
/// Version of the maintained provider descriptor shipped with this SDK.
pub const GITHUB_PROVIDER_VERSION: &str = "1.0.0";
/// GitHub API origin used by the proof application.
pub const GITHUB_API_ORIGIN: &str = "https://api.github.com";
/// Provider descriptor schema version.
pub const GITHUB_DESCRIPTOR_SCHEMA_VERSION: u16 = 1;

/// Host-owned client authentication mode declared by the GitHub provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientAuthentication {
    /// Authorization code exchange uses the protected client secret and a PKCE challenge.
    Pkce,
}

/// The host-owned GitHub OAuth provider descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GithubProviderDescriptor {
    /// Stable provider id.
    pub id: &'static str,
    /// Independently updateable descriptor version.
    pub version: &'static str,
    /// Authorization endpoint.
    pub authorization_endpoint: &'static str,
    /// Token exchange endpoint.
    pub token_endpoint: &'static str,
    /// User profile endpoint.
    pub profile_endpoint: &'static str,
    /// Requested OAuth scopes.
    pub scopes: &'static [&'static str],
    /// Authorization code protection strategy.
    pub client_authentication: ClientAuthentication,
    /// GitHub does not issue refresh tokens for this flow.
    pub refresh: RefreshSemantics,
    /// Provider-specific profile handling.
    pub profile: GithubProfileMapping,
}

/// OAuth refresh behavior declared by a provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshSemantics {
    /// The access session expires and must be signed in again.
    None,
}

/// GitHub profile quirks that the host handles before guest visibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GithubProfileMapping {
    /// Fallback field used when the public profile has no email.
    pub private_email_fallback: &'static str,
    /// The provider action used to obtain that fallback.
    pub email_endpoint: &'static str,
}

/// Return the maintained GitHub provider descriptor.
#[must_use]
pub const fn provider_descriptor() -> GithubProviderDescriptor {
    GithubProviderDescriptor {
        id: GITHUB_PROVIDER_ID,
        version: GITHUB_PROVIDER_VERSION,
        authorization_endpoint: "https://github.com/login/oauth/authorize",
        token_endpoint: "https://github.com/login/oauth/access_token",
        profile_endpoint: "/user",
        scopes: &["read:user", "user:email"],
        client_authentication: ClientAuthentication::Pkce,
        refresh: RefreshSemantics::None,
        profile: GithubProfileMapping {
            private_email_fallback: "primary verified email",
            email_endpoint: "/user/emails",
        },
    }
}

/// Serialize the maintained provider descriptor for signed package metadata.
#[must_use]
pub fn descriptor_document() -> Value {
    json!({
        "schemaVersion": GITHUB_DESCRIPTOR_SCHEMA_VERSION,
        "id": GITHUB_PROVIDER_ID,
        "version": GITHUB_PROVIDER_VERSION,
        "authorizationEndpoint": "https://github.com/login/oauth/authorize",
        "tokenEndpoint": "https://github.com/login/oauth/access_token",
        "profileEndpoint": "/user",
        "scopes": ["read:user", "user:email"],
        "clientAuthentication": "pkce",
        "refresh": "none",
        "privateEmailFallback": { "endpoint": "/user/emails", "field": "primary verified email" }
    })
}

/// Provider session state safe to expose to the application shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GithubSessionStatus {
    /// No session exists.
    SignedOut,
    /// A protected access session is ready for broker injection.
    Active,
    /// The session must be signed in again.
    Expired,
    /// The host revoked the session.
    Revoked,
}

/// Host implementation seam for one protected GitHub session.
pub trait GithubSession: Send + Sync {
    /// Safe status, with no token bytes.
    fn status(&self) -> GithubSessionStatus;
    /// Inject the access credential directly into the broker's send-time sink.
    fn inject(&self, sink: &mut dyn BrokerCredentialSink) -> Result<(), BrokerError>;
}

/// OAuth resolver bound to the maintained GitHub provider id.
pub struct GithubOAuthSessionResolver {
    session: std::sync::Arc<dyn GithubSession>,
}

impl std::fmt::Debug for GithubOAuthSessionResolver {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("GithubOAuthSessionResolver")
    }
}

impl GithubOAuthSessionResolver {
    /// Bind a host-owned protected session implementation.
    #[must_use]
    pub fn new(session: std::sync::Arc<dyn GithubSession>) -> Self {
        Self { session }
    }

    /// Current safe session status.
    #[must_use]
    pub fn status(&self) -> GithubSessionStatus {
        self.session.status()
    }
}

impl OAuthSessionResolver for GithubOAuthSessionResolver {
    fn inject_session(
        &self,
        provider: &str,
        _route_group_id: &str,
        sink: &mut dyn BrokerCredentialSink,
    ) -> Result<(), BrokerError> {
        if provider != GITHUB_PROVIDER_ID || self.session.status() != GithubSessionStatus::Active {
            return Err(BrokerError::new(
                studio_net::BrokerErrorCode::OauthSessionUnavailable,
            ));
        }
        self.session.inject(sink)
    }
}

/// A package reference that enables the first-party provider without embedding a secret.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GithubProviderReference {
    /// Provider id, always `github` for this package.
    pub provider: String,
    /// OAuth application client id; never a client secret.
    pub client_id: String,
    /// Descriptor version selected by the package.
    pub descriptor_version: String,
}

impl Default for GithubProviderReference {
    fn default() -> Self {
        Self {
            provider: GITHUB_PROVIDER_ID.to_owned(),
            client_id: String::new(),
            descriptor_version: GITHUB_PROVIDER_VERSION.to_owned(),
        }
    }
}

/// Route declarations contributed by the GitHub integration plugin.
#[must_use]
pub fn route_groups() -> Vec<RouteGroupDeclaration> {
    vec![
        route_group("github.user", "/user", github_user_schema()),
        route_group(
            "github.repositories",
            "/user/repos",
            github_repositories_schema(),
        ),
        route_group(
            "github.repository",
            "/repos/{owner}/{repo}",
            github_repository_schema(),
        ),
    ]
}

fn route_group(id: &str, path: &str, response_schema: Value) -> RouteGroupDeclaration {
    RouteGroupDeclaration {
        id: id.to_owned(),
        origins: vec![GITHUB_API_ORIGIN.to_owned()],
        methods: vec![HttpMethod::Get],
        paths: vec![path.to_owned()],
        allowed_headers: vec![
            "accept".to_owned(),
            "x-github-api-version".to_owned(),
            "user-agent".to_owned(),
        ],
        credential: CredentialSource::OauthProviderSession {
            provider: GITHUB_PROVIDER_ID.to_owned(),
        },
        request_schema: None,
        response_schema: Some(response_schema),
        streaming: None,
        limits: DeclaredLimits {
            max_response_bytes: Some(8 * 1024 * 1024),
            max_requests_per_window: Some(60),
            ..DeclaredLimits::default()
        },
    }
}

fn github_user_schema() -> Value {
    json!({ "type": "object", "required": ["id", "login"], "properties": {
        "id": { "type": "integer" }, "login": { "type": "string" },
        "name": {}, "email": {}, "avatar_url": {}
    }, "additionalProperties": true })
}

fn github_repositories_schema() -> Value {
    json!({ "type": "array", "items": github_repository_schema(), "maxItems": 100 })
}

fn github_repository_schema() -> Value {
    json!({ "type": "object", "required": ["id", "name", "full_name", "html_url", "owner"], "properties": {
        "id": { "type": "integer" }, "name": { "type": "string" }, "full_name": { "type": "string" },
        "html_url": { "type": "string" }, "owner": { "type": "object", "required": ["login"],
            "properties": { "login": { "type": "string" } }, "additionalProperties": true },
        "description": {}, "private": { "type": "boolean" },
        "default_branch": {}, "stargazers_count": { "type": "integer" },
        "forks_count": { "type": "integer" }, "open_issues_count": { "type": "integer" },
        "language": {}, "updated_at": {}
    }, "additionalProperties": true })
}

/// Safe user projection returned by `/user`.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GithubUser {
    /// Numeric provider id.
    pub id: u64,
    /// Login shown by the viewer.
    pub login: String,
    /// Display name, when configured.
    pub name: Option<String>,
    /// Public or host-resolved email.
    pub email: Option<String>,
    /// Avatar URL.
    pub avatar_url: Option<String>,
}

/// Safe repository summary returned by `/user/repos`.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GithubRepository {
    /// Numeric provider id.
    pub id: u64,
    /// Repository owner login.
    pub owner: String,
    /// Repository name.
    pub name: String,
    /// Fully qualified repository name.
    pub full_name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Whether the repository is private.
    pub private: bool,
    /// Web URL.
    pub html_url: String,
    /// Default branch.
    pub default_branch: Option<String>,
    /// Stargazer count.
    pub stars: u64,
    /// Fork count.
    pub forks: u64,
}

/// Repository detail projection used by the detail screen.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GithubRepositoryDetail {
    /// Repository summary.
    pub repository: GithubRepository,
    /// Open issue count.
    pub open_issues: u64,
    /// Repository language, when detected.
    pub language: Option<String>,
    /// Last update timestamp from the provider.
    pub updated_at: Option<String>,
}

/// Failure from a typed GitHub SDK operation.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum GithubError {
    /// The broker rejected or failed the host-mediated operation.
    #[error(transparent)]
    Broker(#[from] BrokerError),
    /// The host admitted a response that did not contain the required projection fields.
    #[error("github response projection invalid")]
    ResponseInvalid,
    /// A requested owner or repository name was not safe to place in a route path.
    #[error("github repository reference invalid")]
    RepositoryReferenceInvalid,
}

/// Typed SDK client over a host-provided restricted REST facade.
pub struct GithubClient<'api> {
    api: &'api GuestRestApi<'api>,
}

impl<'api> GithubClient<'api> {
    /// Bind the client to the host facade. No credential material is accepted or retained.
    #[must_use]
    pub const fn new(api: &'api GuestRestApi<'api>) -> Self {
        Self { api }
    }

    /// Fetch the authenticated GitHub profile.
    pub fn current_user(&self) -> Result<GithubUser, GithubError> {
        let value = self.get("/user", None)?.body().clone();
        parse_user(&value)
    }

    /// List the authenticated user's repositories in a stable provider order.
    pub fn repositories(&self) -> Result<Vec<GithubRepository>, GithubError> {
        let value = self
            .get(
                "/user/repos",
                Some("sort=updated&direction=desc&per_page=50"),
            )?
            .body()
            .clone();
        let values = value.as_array().ok_or(GithubError::ResponseInvalid)?;
        values.iter().map(parse_repository).collect()
    }

    /// Fetch one repository detail projection.
    pub fn repository(
        &self,
        owner: &str,
        name: &str,
    ) -> Result<GithubRepositoryDetail, GithubError> {
        if !valid_segment(owner) || !valid_segment(name) {
            return Err(GithubError::RepositoryReferenceInvalid);
        }
        let value = self
            .get(&format!("/repos/{owner}/{name}"), None)?
            .body()
            .clone();
        let repository = parse_repository(&value)?;
        Ok(GithubRepositoryDetail {
            repository,
            open_issues: value
                .get("open_issues_count")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            language: value
                .get("language")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            updated_at: value
                .get("updated_at")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        })
    }

    fn get(
        &self,
        path: &str,
        query: Option<&str>,
    ) -> Result<studio_net::TypedResponse, GithubError> {
        let mut request = BrokerRequest::new(GITHUB_API_ORIGIN, HttpMethod::Get, path)
            .with_header("accept", "application/vnd.github+json")
            .with_header("x-github-api-version", "2022-11-28")
            .with_header("user-agent", "studio-github-sdk/0.1");
        if let Some(query) = query {
            request = request.with_query(query);
        }
        Ok(self.api.execute(request)?)
    }
}

fn parse_user(value: &Value) -> Result<GithubUser, GithubError> {
    Ok(GithubUser {
        id: required_u64(value, "id")?,
        login: required_string(value, "login")?,
        name: optional_string(value, "name")?,
        email: optional_string(value, "email")?,
        avatar_url: optional_string(value, "avatar_url")?,
    })
}

fn parse_repository(value: &Value) -> Result<GithubRepository, GithubError> {
    let owner = value
        .get("owner")
        .and_then(|owner| owner.get("login"))
        .and_then(Value::as_str)
        .ok_or(GithubError::ResponseInvalid)?;
    Ok(GithubRepository {
        id: required_u64(value, "id")?,
        owner: owner.to_owned(),
        name: required_string(value, "name")?,
        full_name: required_string(value, "full_name")?,
        description: optional_string(value, "description")?,
        private: value
            .get("private")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        html_url: required_string(value, "html_url")?,
        default_branch: optional_string(value, "default_branch")?,
        stars: value
            .get("stargazers_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        forks: value
            .get("forks_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    })
}

fn required_string(value: &Value, field: &str) -> Result<String, GithubError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or(GithubError::ResponseInvalid)
}

fn optional_string(value: &Value, field: &str) -> Result<Option<String>, GithubError> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or(GithubError::ResponseInvalid),
    }
}

fn required_u64(value: &Value, field: &str) -> Result<u64, GithubError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or(GithubError::ResponseInvalid)
}

fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

/// Screen state for the deterministic proof application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GithubViewerScreen {
    /// No provider session has been established.
    SignIn,
    /// The host has opened the browser and is waiting for the callback.
    Authorizing,
    /// Provider session exists and repositories are being requested.
    LoadingRepositories { user: GithubUser },
    /// Authenticated repository list.
    Repositories {
        user: GithubUser,
        repositories: Vec<GithubRepository>,
    },
    /// One authenticated repository detail screen.
    RepositoryDetail {
        user: GithubUser,
        detail: GithubRepositoryDetail,
    },
    /// Safe failure state with no upstream payload.
    Error { code: &'static str },
}

/// Typed guest-to-host events emitted by the viewer. These events contain provider metadata and
/// repository identities only; browser URLs, callback state, and credentials remain host-owned.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum GithubGuestEvent {
    /// Begin the host-owned PKCE sign-in flow.
    SignInRequested {
        /// Provider registry identity.
        provider: String,
        /// Least-privilege scopes admitted by the descriptor.
        scopes: Vec<String>,
    },
    /// Ask the host broker for the authenticated repository list.
    RepositoriesRequested,
    /// Ask the host broker for one repository detail projection.
    RepositoryRequested { owner: String, name: String },
}

/// Typed host-to-guest events consumed by the viewer. Every payload is an approved projection;
/// no variant can carry an access or refresh token.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum GithubHostEvent {
    /// Browser handoff and callback capture are in progress on the host.
    SignInStarted,
    /// Host-approved profile claims are available to the guest.
    ProfileApproved { user: GithubUser },
    /// Host-approved repository list response.
    RepositoriesLoaded {
        user: GithubUser,
        repositories: Vec<GithubRepository>,
    },
    /// Host-approved repository detail response.
    RepositoryLoaded {
        user: GithubUser,
        detail: GithubRepositoryDetail,
    },
    /// Safe failure code with no upstream response or credential context.
    Failed { code: String, retryable: bool },
}

/// Host-neutral proof journey controller.
pub struct GithubViewer<'api> {
    client: GithubClient<'api>,
    screen: GithubViewerScreen,
}

impl<'api> GithubViewer<'api> {
    /// Create the signed-out initial state.
    #[must_use]
    pub const fn new(api: &'api GuestRestApi<'api>) -> Self {
        Self {
            client: GithubClient::new(api),
            screen: GithubViewerScreen::SignIn,
        }
    }

    /// Current screen state.
    #[must_use]
    pub const fn screen(&self) -> &GithubViewerScreen {
        &self.screen
    }

    /// Start the provider-owned sign-in handoff. Browser and callback capture stay host-owned.
    #[must_use]
    pub const fn sign_in_request() -> GithubSignInRequest {
        GithubSignInRequest {
            provider: GITHUB_PROVIDER_ID,
            scopes: provider_descriptor().scopes,
        }
    }

    /// Typed guest event starting the host-owned browser/PKCE flow.
    #[must_use]
    pub fn sign_in_event() -> GithubGuestEvent {
        GithubGuestEvent::SignInRequested {
            provider: GITHUB_PROVIDER_ID.to_owned(),
            scopes: provider_descriptor()
                .scopes
                .iter()
                .map(|scope| (*scope).to_owned())
                .collect(),
        }
    }

    /// Apply one host-approved event and perform the corresponding screen transition.
    ///
    /// # Errors
    ///
    /// Rejects a repository response before a profile/list screen exists.
    pub fn apply_host_event(&mut self, event: GithubHostEvent) -> Result<(), GithubError> {
        match event {
            GithubHostEvent::SignInStarted => {
                self.screen = GithubViewerScreen::Authorizing;
                Ok(())
            }
            GithubHostEvent::ProfileApproved { user } => {
                self.screen = GithubViewerScreen::LoadingRepositories { user };
                Ok(())
            }
            GithubHostEvent::RepositoriesLoaded { user, repositories } => {
                self.screen = GithubViewerScreen::Repositories { user, repositories };
                Ok(())
            }
            GithubHostEvent::RepositoryLoaded { user, detail } => {
                self.screen = GithubViewerScreen::RepositoryDetail { user, detail };
                Ok(())
            }
            GithubHostEvent::Failed { code, .. } => {
                self.screen = GithubViewerScreen::Error {
                    code: safe_error_code(&code),
                };
                Ok(())
            }
        }
    }

    /// Complete sign-in with the host-approved public profile and load repositories.
    pub fn complete_sign_in(&mut self, user: GithubUser) -> Result<(), GithubError> {
        self.apply_host_event(GithubHostEvent::ProfileApproved { user: user.clone() })?;
        match self.client.repositories() {
            Ok(repositories) => {
                self.apply_host_event(GithubHostEvent::RepositoriesLoaded { user, repositories })?;
                Ok(())
            }
            Err(error) => {
                self.screen = GithubViewerScreen::Error {
                    code: github_error_code(&error),
                };
                Err(error)
            }
        }
    }

    /// Open one repository from the authenticated list.
    pub fn open_repository(&mut self, owner: &str, name: &str) -> Result<(), GithubError> {
        let user = match &self.screen {
            GithubViewerScreen::Repositories { user, .. }
            | GithubViewerScreen::RepositoryDetail { user, .. } => user.clone(),
            _ => return Err(GithubError::ResponseInvalid),
        };
        match self.client.repository(owner, name) {
            Ok(detail) => {
                self.apply_host_event(GithubHostEvent::RepositoryLoaded { user, detail })?;
                Ok(())
            }
            Err(error) => {
                self.screen = GithubViewerScreen::Error {
                    code: github_error_code(&error),
                };
                Err(error)
            }
        }
    }
}

/// Host request emitted when the viewer begins OAuth.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GithubSignInRequest {
    /// Provider descriptor identity.
    pub provider: &'static str,
    /// Scopes requested by the maintained descriptor.
    pub scopes: &'static [&'static str],
}

/// Start the provider-owned sign-in handoff without requiring a client or a session token.
#[must_use]
pub const fn sign_in_request() -> GithubSignInRequest {
    GithubSignInRequest {
        provider: GITHUB_PROVIDER_ID,
        scopes: provider_descriptor().scopes,
    }
}

fn github_error_code(error: &GithubError) -> &'static str {
    match error {
        GithubError::Broker(error) => error.code().as_str(),
        GithubError::ResponseInvalid => "github.response.invalid",
        GithubError::RepositoryReferenceInvalid => "github.repository.invalid",
    }
}

fn safe_error_code(code: &str) -> &'static str {
    match code {
        "net.credential.oauth_session_unavailable" => "net.credential.oauth_session_unavailable",
        "net.route.origin_not_declared" => "net.route.origin_not_declared",
        "net.route.path_not_declared" => "net.route.path_not_declared",
        "net.route.method_not_allowed" => "net.route.method_not_allowed",
        "net.response.upstream_rejected" => "net.response.upstream_rejected",
        "net.response.malformed" => "net.response.malformed",
        "net.response.schema_mismatch" => "net.response.schema_mismatch",
        "net.response.too_large" => "net.response.too_large",
        "github.repository.invalid" => "github.repository.invalid",
        "github.response.invalid" => "github.response.invalid",
        _ => "github.request.failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_net::limits::BrokerLimits;

    #[test]
    fn descriptor_and_routes_are_stable_and_bounded() {
        assert_eq!(provider_descriptor().id, GITHUB_PROVIDER_ID);
        assert_eq!(descriptor_document()["clientAuthentication"], "pkce");
        let groups = route_groups();
        assert_eq!(groups.len(), 3);
        for group in groups {
            assert!(group.compile(&BrokerLimits::default()).is_ok());
        }
    }

    #[test]
    fn sign_in_request_contains_only_provider_metadata() {
        let request = sign_in_request();
        assert_eq!(request.provider, GITHUB_PROVIDER_ID);
        assert_eq!(request.scopes, &["read:user", "user:email"]);
    }

    #[test]
    fn typed_events_are_closed_and_token_free() {
        let request = GithubViewer::sign_in_event();
        let encoded = serde_json::to_value(&request).unwrap();
        assert_eq!(encoded["type"], "sign_in_requested");
        assert_eq!(encoded["scopes"], json!(["read:user", "user:email"]));

        let event = GithubHostEvent::Failed {
            code: String::from("net.response.upstream_rejected"),
            retryable: true,
        };
        let encoded = serde_json::to_string(&event).unwrap();
        assert!(!encoded.contains("token"));
        assert_eq!(
            safe_error_code("unexpected upstream detail"),
            "github.request.failed"
        );
    }
}
