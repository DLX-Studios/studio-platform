//! Host-owned OAuth provider plugins.
//!
//! Provider integrations are data, not application code: a signed application names a provider
//! and an exact descriptor version, while the host resolves that version from its current
//! descriptor catalog. The flow owns the browser, loopback listener, state, PKCE verifier, token
//! exchange, profile mapping, and protected token vault. Only [`ApprovedClaims`], lifecycle
//! [`OAuthStatus`], and [`OAuthActionResult`] cross the application boundary.

#![allow(missing_docs)]
#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::map_unwrap_or,
    clippy::duration_suboptimal_units,
    clippy::unused_self,
    clippy::ignored_unit_patterns,
    clippy::match_same_arms
)]

use std::{
    collections::BTreeMap,
    fmt,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use studio_net::error::{BrokerError, BrokerErrorCode};
use studio_security::{
    ApplicationEnvironment, BrokerCredentialSink, CredentialBackend, PluginPrincipal,
    ProtectedSecretError, ProtectedSecretErrorCode, ProtectedSecretKey, ProtectedSecretState,
    ProtectedSecretStore, SecretInput,
};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

/// The descriptor wire schema implemented by this host.
pub const DESCRIPTOR_SCHEMA_VERSION: u32 = 1;
/// The first-party GitHub provider identifier.
pub const GITHUB_PROVIDER_ID: &str = "github";
/// The current first-party GitHub descriptor version.
pub const GITHUB_DESCRIPTOR_VERSION: &str = "1.0.0";
const CALLBACK_PATH: &str = "/oauth/callback";
const MAX_CALLBACK_BYTES: usize = 16 * 1024;
const MAX_TOKEN_BYTES: usize = 4096;
const TOKEN_RECORD_PREFIX: &[u8] = b"studio.oauth.tokens.v2\0";

/// Closed, value-free OAuth failure codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum OAuthErrorCode {
    /// The provider descriptor could not be admitted.
    DescriptorInvalid,
    /// No descriptor exists for the requested provider.
    ProviderUnknown,
    /// The requested descriptor version is not installed.
    ProviderOutdated,
    /// The package's provider configuration is invalid.
    PackageInvalid,
    /// Randomness required for state or PKCE was unavailable.
    EntropyUnavailable,
    /// The host could not create its callback listener.
    CallbackBindFailed,
    /// The callback did not arrive or could not be parsed.
    CallbackFailed,
    /// The callback state did not match the host-generated state.
    StateMismatch,
    /// The user or provider denied authorization.
    AuthorizationDenied,
    /// The browser handoff was unavailable.
    BrowserUnavailable,
    /// The token endpoint rejected or malformed the exchange.
    TokenExchangeFailed,
    /// A protected token was unavailable.
    TokenUnavailable,
    /// The protected store could not be used.
    StorageUnavailable,
    /// A confidential client secret was not configured.
    ClientSecretUnavailable,
    /// The profile endpoint failed or returned malformed data.
    ProfileFailed,
    /// The provider returned no approved subject claim.
    ClaimsInvalid,
    /// The provider has no refresh behavior for this session.
    RefreshUnavailable,
    /// The upstream revoke operation failed.
    RevokeFailed,
    /// A host-side injection was rejected.
    InjectionRejected,
}

impl OAuthErrorCode {
    /// Stable code for diagnostics and action results.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DescriptorInvalid => "oauth.descriptor.invalid",
            Self::ProviderUnknown => "oauth.provider.unknown",
            Self::ProviderOutdated => "oauth.provider.outdated",
            Self::PackageInvalid => "oauth.package.invalid",
            Self::EntropyUnavailable => "oauth.entropy.unavailable",
            Self::CallbackBindFailed => "oauth.callback.bind_failed",
            Self::CallbackFailed => "oauth.callback.failed",
            Self::StateMismatch => "oauth.callback.state_mismatch",
            Self::AuthorizationDenied => "oauth.authorization.denied",
            Self::BrowserUnavailable => "oauth.browser.unavailable",
            Self::TokenExchangeFailed => "oauth.token.exchange_failed",
            Self::TokenUnavailable => "oauth.token.unavailable",
            Self::StorageUnavailable => "oauth.storage.unavailable",
            Self::ClientSecretUnavailable => "oauth.client_secret.unavailable",
            Self::ProfileFailed => "oauth.profile.failed",
            Self::ClaimsInvalid => "oauth.claims.invalid",
            Self::RefreshUnavailable => "oauth.refresh.unavailable",
            Self::RevokeFailed => "oauth.revoke.failed",
            Self::InjectionRejected => "oauth.injection.rejected",
        }
    }
}

impl fmt::Display for OAuthErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Safe OAuth error with no provider, URL, token, or upstream response context.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[error("{code}")]
pub struct OAuthError {
    code: OAuthErrorCode,
}

impl OAuthError {
    /// Construct a closed error from a stable code.
    #[must_use]
    pub const fn new(code: OAuthErrorCode) -> Self {
        Self { code }
    }

    /// Stable failure code.
    #[must_use]
    pub const fn code(self) -> OAuthErrorCode {
        self.code
    }
}

/// How a descriptor authenticates its token request.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ClientAuthentication {
    /// Public client using an S256 PKCE verifier.
    Pkce,
    /// Confidential client whose secret is supplied through protected configuration.
    ConfidentialClient,
}

/// Declarative provider-specific behavior flags.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderQuirks {
    /// The provider does not issue refresh tokens.
    pub no_refresh_tokens: bool,
    /// Fetch a primary verified private email from the fallback endpoint when profile email is
    /// absent.
    pub private_email_fallback: bool,
}

/// One dotted JSON claim path in a profile response.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ClaimPath(pub String);

impl ClaimPath {
    /// Construct a dotted path.
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    fn value<'a>(&self, root: &'a Value) -> Option<&'a Value> {
        let mut current = root;
        for component in self.0.split('.') {
            if component.is_empty() {
                return None;
            }
            current = current.get(component)?;
        }
        Some(current)
    }
}

/// Mapping for the optional private-email fallback collection.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmailFallbackMapping {
    /// Endpoint returning email records.
    pub endpoint: String,
    /// Email value path within each record.
    pub email: ClaimPath,
    /// Boolean path identifying the primary record.
    pub primary: ClaimPath,
    /// Boolean path identifying a verified record.
    pub verified: ClaimPath,
}

/// Approved profile fields and their provider response paths.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileMapping {
    /// Endpoint returning the profile object.
    pub endpoint: String,
    /// Stable provider subject path.
    pub subject: ClaimPath,
    /// Optional account/login path.
    pub login: Option<ClaimPath>,
    /// Optional display-name path.
    pub display_name: Option<ClaimPath>,
    /// Optional email path.
    pub email: Option<ClaimPath>,
    /// Optional avatar URL path.
    pub avatar_url: Option<ClaimPath>,
    /// Optional profile URL path.
    pub profile_url: Option<ClaimPath>,
    /// Fallback mapping for private email providers.
    pub email_fallback: Option<EmailFallbackMapping>,
}

/// Versioned, closed provider descriptor loaded by the host catalog.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderDescriptor {
    /// Descriptor wire schema version.
    pub schema_version: u32,
    /// Stable provider identifier.
    pub id: String,
    /// Semver descriptor version selected by applications.
    pub version: String,
    /// Authorization endpoint.
    pub authorization_endpoint: String,
    /// Token endpoint.
    pub token_endpoint: String,
    /// Optional revoke endpoint.
    pub revocation_endpoint: Option<String>,
    /// Profile and approved-claim mapping.
    pub profile: ProfileMapping,
    /// Requested scopes in stable order.
    pub scopes: Vec<String>,
    /// Client authentication behavior.
    pub client_authentication: ClientAuthentication,
    /// Provider-specific, declarative behavior.
    pub quirks: ProviderQuirks,
}

impl ProviderDescriptor {
    /// The maintained GitHub descriptor.
    #[must_use]
    pub fn github() -> Self {
        Self {
            schema_version: DESCRIPTOR_SCHEMA_VERSION,
            id: GITHUB_PROVIDER_ID.to_owned(),
            version: GITHUB_DESCRIPTOR_VERSION.to_owned(),
            authorization_endpoint: String::from("https://github.com/login/oauth/authorize"),
            token_endpoint: String::from("https://github.com/login/oauth/access_token"),
            revocation_endpoint: Some(String::from(
                "https://api.github.com/applications/{clientId}/token",
            )),
            profile: ProfileMapping {
                endpoint: String::from("https://api.github.com/user"),
                subject: ClaimPath::new("id"),
                login: Some(ClaimPath::new("login")),
                display_name: Some(ClaimPath::new("name")),
                email: Some(ClaimPath::new("email")),
                avatar_url: Some(ClaimPath::new("avatar_url")),
                profile_url: Some(ClaimPath::new("html_url")),
                email_fallback: Some(EmailFallbackMapping {
                    endpoint: String::from("https://api.github.com/user/emails"),
                    email: ClaimPath::new("email"),
                    primary: ClaimPath::new("primary"),
                    verified: ClaimPath::new("verified"),
                }),
            },
            // The viewer only reads the authenticated profile and private email. `repo` is a
            // broad repository-management grant and is intentionally not admitted here.
            scopes: vec![String::from("read:user"), String::from("user:email")],
            client_authentication: ClientAuthentication::Pkce,
            quirks: ProviderQuirks {
                no_refresh_tokens: true,
                private_email_fallback: true,
            },
        }
    }

    /// Validate the closed descriptor before installation.
    pub fn validate(&self) -> Result<(), OAuthError> {
        if self.schema_version != DESCRIPTOR_SCHEMA_VERSION
            || !valid_identifier(&self.id)
            || self.version.parse::<semver::Version>().is_err()
            || !https_url(&self.authorization_endpoint)
            || !https_url(&self.token_endpoint)
            || self.scopes.is_empty()
            || self.scopes.iter().any(|scope| !valid_scope(scope))
            || has_duplicate_strings(&self.scopes)
            || !https_url(&self.profile.endpoint)
            || !valid_claim_path(&self.profile.subject)
            || self
                .profile
                .login
                .as_ref()
                .is_some_and(|path| !valid_claim_path(path))
            || self
                .profile
                .display_name
                .as_ref()
                .is_some_and(|path| !valid_claim_path(path))
            || self
                .profile
                .email
                .as_ref()
                .is_some_and(|path| !valid_claim_path(path))
            || self
                .profile
                .avatar_url
                .as_ref()
                .is_some_and(|path| !valid_claim_path(path))
            || self
                .profile
                .profile_url
                .as_ref()
                .is_some_and(|path| !valid_claim_path(path))
        {
            return Err(OAuthError::new(OAuthErrorCode::DescriptorInvalid));
        }
        if self.id == GITHUB_PROVIDER_ID
            && self
                .scopes
                .iter()
                .any(|scope| !matches!(scope.as_str(), "read:user" | "user:email"))
        {
            return Err(OAuthError::new(OAuthErrorCode::DescriptorInvalid));
        }
        if self
            .revocation_endpoint
            .as_deref()
            .is_some_and(|endpoint| !https_url(endpoint))
        {
            return Err(OAuthError::new(OAuthErrorCode::DescriptorInvalid));
        }
        if self
            .profile
            .email_fallback
            .as_ref()
            .is_some_and(|fallback| {
                !https_url(&fallback.endpoint)
                    || !valid_claim_path(&fallback.email)
                    || !valid_claim_path(&fallback.primary)
                    || !valid_claim_path(&fallback.verified)
            })
        {
            return Err(OAuthError::new(OAuthErrorCode::DescriptorInvalid));
        }
        Ok(())
    }
}

/// Protected configuration reference declared by an application package.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProtectedSecretReference {
    /// Stable protected-store name.
    pub name: String,
    /// Human-readable purpose shown by host configuration UI.
    pub purpose: String,
}

impl ProtectedSecretReference {
    fn key(&self) -> Result<ProtectedSecretKey, OAuthError> {
        ProtectedSecretKey::new(self.name.clone(), self.purpose.clone())
            .map_err(|_| OAuthError::new(OAuthErrorCode::PackageInvalid))
    }
}

/// Application declaration enabling one provider descriptor version.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderPackage {
    /// Provider identifier in the host descriptor catalog.
    pub provider: String,
    /// Exact descriptor semver selected by the package.
    pub descriptor_version: String,
    /// OAuth client ID (never a client secret).
    pub client_id: String,
    /// Protected configuration reference required by confidential clients.
    pub client_secret: Option<ProtectedSecretReference>,
}

/// Alias for package manifests that call this object a provider configuration.
pub type ProviderConfiguration = ProviderPackage;

impl ProviderPackage {
    /// Create a public-client provider declaration.
    #[must_use]
    pub fn new(
        provider: impl Into<String>,
        descriptor_version: impl Into<String>,
        client_id: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            descriptor_version: descriptor_version.into(),
            client_id: client_id.into(),
            client_secret: None,
        }
    }

    /// Create a declaration that follows the newest compatible descriptor at runtime.
    ///
    /// This is useful for first-party packages that should receive a maintained descriptor
    /// behavior update without rebuilding the authored application. Packages that require a
    /// pinned behavior can continue to use [`Self::new`].
    #[must_use]
    pub fn latest(provider: impl Into<String>, client_id: impl Into<String>) -> Self {
        Self::new(provider, "latest", client_id)
    }

    /// Attach a protected confidential-client secret reference.
    #[must_use]
    pub fn with_client_secret(mut self, secret: ProtectedSecretReference) -> Self {
        self.client_secret = Some(secret);
        self
    }

    fn validate(&self, descriptor: &ProviderDescriptor) -> Result<(), OAuthError> {
        if self.provider != descriptor.id
            || (self.descriptor_version != "latest"
                && self.descriptor_version != descriptor.version)
            || self.client_id.is_empty()
            || self.client_id.len() > 256
            || self.client_id.chars().any(char::is_control)
            || (descriptor.client_authentication == ClientAuthentication::ConfidentialClient
                && self.client_secret.is_none())
            || (descriptor.client_authentication == ClientAuthentication::Pkce
                && self.client_secret.is_some())
        {
            return Err(OAuthError::new(OAuthErrorCode::PackageInvalid));
        }
        if let Some(secret) = &self.client_secret {
            let _ = secret.key()?;
        }
        Ok(())
    }
}

/// Runtime descriptor catalog. Versions are selected without rebuilding authored applications.
#[derive(Clone, Debug, Default)]
pub struct ProviderRegistry {
    descriptors: BTreeMap<String, BTreeMap<String, ProviderDescriptor>>,
}

/// Alias emphasizing that the catalog is the host's provider-plugin registry.
pub type ProviderDescriptorRegistry = ProviderRegistry;

impl ProviderRegistry {
    /// Construct a catalog containing maintained first-party providers.
    #[must_use]
    pub fn github() -> Self {
        let mut registry = Self::default();
        registry
            .register(ProviderDescriptor::github())
            .expect("built-in GitHub descriptor is valid");
        registry
    }

    /// Install one descriptor version into the host catalog.
    pub fn register(&mut self, descriptor: ProviderDescriptor) -> Result<(), OAuthError> {
        descriptor.validate()?;
        let versions = self.descriptors.entry(descriptor.id.clone()).or_default();
        if versions.contains_key(&descriptor.version) {
            return Err(OAuthError::new(OAuthErrorCode::DescriptorInvalid));
        }
        versions.insert(descriptor.version.clone(), descriptor);
        Ok(())
    }

    /// Resolve an exact provider descriptor, failing closed for unknown/outdated versions.
    pub fn resolve(&self, package: &ProviderPackage) -> Result<&ProviderDescriptor, OAuthError> {
        let Some(versions) = self.descriptors.get(&package.provider) else {
            return Err(OAuthError::new(OAuthErrorCode::ProviderUnknown));
        };
        let descriptor = if package.descriptor_version == "latest" {
            versions
                .values()
                .max_by(|left, right| {
                    left.version
                        .parse::<semver::Version>()
                        .ok()
                        .cmp(&right.version.parse::<semver::Version>().ok())
                })
                .ok_or_else(|| OAuthError::new(OAuthErrorCode::ProviderOutdated))?
        } else {
            versions
                .get(&package.descriptor_version)
                .ok_or_else(|| OAuthError::new(OAuthErrorCode::ProviderOutdated))?
        };
        package.validate(descriptor)?;
        Ok(descriptor)
    }
}

/// Guest-safe approved claims extracted by descriptor mapping.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedClaims {
    /// Stable provider subject.
    pub subject: String,
    /// Provider login, when available.
    pub login: Option<String>,
    /// Display name, when available.
    pub display_name: Option<String>,
    /// Avatar URL, when available.
    pub avatar_url: Option<String>,
    /// Profile URL, when available.
    pub profile_url: Option<String>,
    /// Primary verified email, when the provider exposes one.
    pub email: Option<String>,
}

/// Guest-safe lifecycle state for one provider session.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OAuthStatus {
    /// No local session is configured.
    SignedOut,
    /// Browser authorization is in progress on the host.
    Authorizing,
    /// A protected access token and approved claims are available.
    Authenticated,
    /// A session exists but needs refresh.
    Expired,
    /// Local revocation cleared the token and retained a revoked marker.
    Revoked,
    /// Provider configuration or protected storage is unavailable.
    Unavailable,
}

/// Host actions exposed as stable result records.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OAuthAction {
    /// Start an authorization-code sign-in.
    SignIn,
    /// Refresh an existing session.
    Refresh,
    /// Remove local session state without contacting the provider.
    SignOut,
    /// Revoke upstream and locally clear the session.
    Revoke,
}

/// Stable action outcome code with no token-bearing detail.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OAuthActionCode {
    /// The action completed.
    Success,
    /// The action was rejected with the corresponding safe error code.
    Failed(OAuthErrorCode),
}

impl OAuthActionCode {
    /// Stable action result string suitable for guest envelopes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "oauth.success",
            Self::Failed(error) => error.as_str(),
        }
    }
}

/// Result safe to pass through a guest action envelope.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthActionResult {
    /// Action that produced the result.
    pub action: OAuthAction,
    /// Result code, never a provider response or token.
    pub code: OAuthActionCode,
    /// Current provider lifecycle state.
    pub status: OAuthStatus,
    /// Approved profile claims on successful sign-in/refresh.
    pub claims: Option<ApprovedClaims>,
}

impl OAuthActionResult {
    fn success(action: OAuthAction, claims: Option<ApprovedClaims>) -> Self {
        Self {
            action,
            code: OAuthActionCode::Success,
            status: OAuthStatus::Authenticated,
            claims,
        }
    }

    fn failure(action: OAuthAction, error: OAuthErrorCode, status: OAuthStatus) -> Self {
        Self {
            action,
            code: OAuthActionCode::Failed(error),
            status,
            claims: None,
        }
    }

    /// Whether the action completed successfully.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self.code, OAuthActionCode::Success)
    }

    /// Stable string representation of the action result code.
    #[must_use]
    pub const fn code_str(&self) -> &'static str {
        self.code.as_str()
    }
}

/// Non-formatable, non-serializable secret view supplied only to host transports.
#[derive(Clone, Copy)]
pub struct SecretToken<'a>(&'a [u8]);

impl SecretToken<'_> {
    /// Borrow token bytes inside a host-only transport call.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0
    }
}

impl fmt::Debug for SecretToken<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretToken(REDACTED)")
    }
}

/// Secret-bearing token response retained only in protected host storage.
pub struct TokenResponse {
    access_token: Zeroizing<Vec<u8>>,
    refresh_token: Option<Zeroizing<Vec<u8>>>,
    scopes: Option<Vec<String>>,
    /// Provider-declared lifetime in seconds, when supplied.
    pub expires_in: Option<u64>,
}

impl TokenResponse {
    /// Construct a response from an injected token transport.
    pub fn new(
        access_token: Vec<u8>,
        refresh_token: Option<Vec<u8>>,
        expires_in: Option<u64>,
    ) -> Result<Self, OAuthError> {
        if access_token.is_empty()
            || access_token.len() > MAX_TOKEN_BYTES
            || refresh_token
                .as_ref()
                .is_some_and(|token| token.is_empty() || token.len() > MAX_TOKEN_BYTES)
        {
            return Err(OAuthError::new(OAuthErrorCode::TokenExchangeFailed));
        }
        Ok(Self {
            access_token: Zeroizing::new(access_token),
            refresh_token: refresh_token.map(Zeroizing::new),
            scopes: None,
            expires_in,
        })
    }

    /// Attach the provider-reported granted scopes for strict host-side validation.
    ///
    /// Transports should provide this when the token endpoint returns a scope field. Older
    /// providers that omit it remain supported; an explicitly returned scope set is never
    /// allowed to broaden or silently reduce the descriptor declaration.
    pub fn with_scopes<I, S>(mut self, scopes: I) -> Result<Self, OAuthError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let scopes = scopes.into_iter().map(Into::into).collect::<Vec<_>>();
        if scopes.is_empty() || scopes.iter().any(|scope| !valid_scope(scope)) {
            return Err(OAuthError::new(OAuthErrorCode::TokenExchangeFailed));
        }
        self.scopes = Some(scopes);
        Ok(self)
    }

    fn access(&self) -> SecretToken<'_> {
        SecretToken(self.access_token.as_slice())
    }

    fn refresh(&self) -> Option<SecretToken<'_>> {
        self.refresh_token
            .as_deref()
            .map(|token| SecretToken(token.as_slice()))
    }
}

impl fmt::Debug for TokenResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TokenResponse(REDACTED)")
    }
}

/// Callback payload captured by the host-owned listener.
#[derive(Clone, Eq, PartialEq)]
pub struct Callback {
    /// Authorization code, if the provider approved the request.
    pub code: Option<String>,
    /// Returned OAuth state.
    pub state: Option<String>,
    /// Provider error indicator, intentionally opaque.
    pub denied: bool,
}

impl fmt::Debug for Callback {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Callback")
            .field("code", &self.code.as_ref().map(|_| "REDACTED"))
            .field("state", &self.state.as_ref().map(|_| "REDACTED"))
            .field("denied", &self.denied)
            .finish()
    }
}

/// Browser handoff seam. Implementations must not log the authorization URL.
pub trait BrowserHandoff: Send + Sync {
    /// Open the host-generated authorization URL.
    fn open(&self, authorization_url: &str) -> Result<(), OAuthError>;
}

/// Host-owned callback receiver seam used by deterministic tests and production listeners.
pub trait CallbackReceiver: Send {
    /// Redirect URI registered in the authorization request.
    fn redirect_uri(&self) -> &str;
    /// Wait for one callback, bounded by the supplied timeout.
    fn wait(&mut self, timeout: Duration) -> Result<Callback, OAuthError>;
}

/// Factory for host-owned loopback callback listeners.
pub trait CallbackListener: Send + Sync {
    /// Bind an ephemeral loopback listener before browser handoff.
    fn bind(&self) -> Result<Box<dyn CallbackReceiver>, OAuthError>;
}

/// Injectable entropy seam for state and PKCE generation.
pub trait EntropySource: Send + Sync {
    /// Fill a host-owned random buffer.
    fn fill(&self, bytes: &mut [u8]) -> Result<(), OAuthError>;
}

/// Exchange and profile transport seam. Implementations own HTTP and never expose responses to
/// guests; tests can provide deterministic fakes without credentials or network access.
pub trait OAuthTransport: Send + Sync {
    /// Exchange an authorization code for tokens.
    fn exchange_code(&self, request: CodeExchangeRequest<'_>) -> Result<TokenResponse, OAuthError>;
    /// Refresh an access token.
    fn refresh(&self, request: RefreshRequest<'_>) -> Result<TokenResponse, OAuthError>;
    /// Fetch a JSON profile response using one protected access token.
    fn profile(&self, request: ProfileRequest<'_>) -> Result<Value, OAuthError>;
    /// Revoke one access token upstream.
    fn revoke(&self, request: RevokeRequest<'_>) -> Result<(), OAuthError>;
}

/// Host-only authorization-code request.
pub struct CodeExchangeRequest<'a> {
    /// Descriptor token endpoint.
    pub endpoint: &'a str,
    /// OAuth client ID.
    pub client_id: &'a str,
    /// Authorization code.
    pub code: &'a str,
    /// PKCE verifier, when the descriptor uses PKCE.
    pub verifier: Option<SecretToken<'a>>,
    /// Confidential client secret, when configured.
    pub client_secret: Option<SecretToken<'a>>,
    /// Host-owned loopback redirect URI.
    pub redirect_uri: &'a str,
}

impl fmt::Debug for CodeExchangeRequest<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodeExchangeRequest")
            .field("endpoint", &self.endpoint)
            .field("client_id", &self.client_id)
            .field("code", &"REDACTED")
            .field("verifier", &self.verifier.as_ref().map(|_| "REDACTED"))
            .field(
                "client_secret",
                &self.client_secret.as_ref().map(|_| "REDACTED"),
            )
            .finish_non_exhaustive()
    }
}

/// Host-only refresh request.
pub struct RefreshRequest<'a> {
    /// Descriptor token endpoint.
    pub endpoint: &'a str,
    /// OAuth client ID.
    pub client_id: &'a str,
    /// Protected refresh token.
    pub refresh_token: SecretToken<'a>,
    /// Confidential client secret, when configured.
    pub client_secret: Option<SecretToken<'a>>,
}

/// Host-only profile request.
pub struct ProfileRequest<'a> {
    /// Descriptor profile endpoint.
    pub endpoint: &'a str,
    /// Protected access token.
    pub access_token: SecretToken<'a>,
}

/// Host-only revoke request.
pub struct RevokeRequest<'a> {
    /// Descriptor revocation endpoint, if available.
    pub endpoint: &'a str,
    /// OAuth client ID.
    pub client_id: &'a str,
    /// Protected access token.
    pub access_token: SecretToken<'a>,
}

/// Operating-system entropy implementation used by production hosts.
#[derive(Clone, Copy, Debug, Default)]
pub struct OsEntropy;

impl EntropySource for OsEntropy {
    fn fill(&self, bytes: &mut [u8]) -> Result<(), OAuthError> {
        getrandom::fill(bytes).map_err(|_| OAuthError::new(OAuthErrorCode::EntropyUnavailable))
    }
}

/// Browser implementation using the desktop's `xdg-open` command.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemBrowser;

impl BrowserHandoff for SystemBrowser {
    fn open(&self, authorization_url: &str) -> Result<(), OAuthError> {
        std::process::Command::new("xdg-open")
            .arg(authorization_url)
            .spawn()
            .map(|_| ())
            .map_err(|_| OAuthError::new(OAuthErrorCode::BrowserUnavailable))
    }
}

/// A real host-owned loopback listener for desktop hosts.
#[derive(Clone, Copy, Debug, Default)]
pub struct TcpLoopbackListener;

impl CallbackListener for TcpLoopbackListener {
    fn bind(&self) -> Result<Box<dyn CallbackReceiver>, OAuthError> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|_| OAuthError::new(OAuthErrorCode::CallbackBindFailed))?;
        let port = listener
            .local_addr()
            .map_err(|_| OAuthError::new(OAuthErrorCode::CallbackBindFailed))?
            .port();
        Ok(Box::new(TcpCallbackReceiver {
            listener,
            redirect_uri: format!("http://127.0.0.1:{port}{CALLBACK_PATH}"),
        }))
    }
}

struct TcpCallbackReceiver {
    listener: TcpListener,
    redirect_uri: String,
}

impl CallbackReceiver for TcpCallbackReceiver {
    fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    fn wait(&mut self, timeout: Duration) -> Result<Callback, OAuthError> {
        self.listener
            .set_nonblocking(true)
            .map_err(|_| OAuthError::new(OAuthErrorCode::CallbackFailed))?;
        let deadline = Instant::now() + timeout;
        let (mut stream, _) = loop {
            match self.listener.accept() {
                Ok(connection) => break connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(OAuthError::new(OAuthErrorCode::CallbackFailed));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => return Err(OAuthError::new(OAuthErrorCode::CallbackFailed)),
            }
        };
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|_| OAuthError::new(OAuthErrorCode::CallbackFailed))?;
        parse_callback(&mut stream)
    }
}

fn parse_callback(stream: &mut TcpStream) -> Result<Callback, OAuthError> {
    let mut bytes = vec![0_u8; MAX_CALLBACK_BYTES];
    let count = stream
        .read(&mut bytes)
        .map_err(|_| OAuthError::new(OAuthErrorCode::CallbackFailed))?;
    bytes.truncate(count);
    if count == MAX_CALLBACK_BYTES {
        return Err(OAuthError::new(OAuthErrorCode::CallbackFailed));
    }
    let request =
        std::str::from_utf8(&bytes).map_err(|_| OAuthError::new(OAuthErrorCode::CallbackFailed))?;
    let target = request
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("GET "))
        .and_then(|line| line.split_whitespace().next())
        .ok_or_else(|| OAuthError::new(OAuthErrorCode::CallbackFailed))?;
    if !target.starts_with(CALLBACK_PATH)
        || target
            .as_bytes()
            .get(CALLBACK_PATH.len())
            .is_some_and(|byte| *byte != b'?')
    {
        return Err(OAuthError::new(OAuthErrorCode::CallbackFailed));
    }
    let mut query = BTreeMap::new();
    for part in target
        .split_once('?')
        .map_or("", |(_, query)| query)
        .split('&')
    {
        if part.is_empty() {
            continue;
        }
        let (key, value) = part
            .split_once('=')
            .ok_or_else(|| OAuthError::new(OAuthErrorCode::CallbackFailed))?;
        let key = percent_decode(key)?;
        let value = percent_decode(value)?;
        if !matches!(key.as_str(), "code" | "state" | "error" | "scope")
            || query.insert(key, value).is_some()
        {
            return Err(OAuthError::new(OAuthErrorCode::CallbackFailed));
        }
    }
    let denied = query.contains_key("error");
    if query
        .get("code")
        .is_some_and(|value| !valid_callback_value(value))
        || query
            .get("state")
            .is_some_and(|value| !valid_callback_value(value))
        || query
            .get("error")
            .is_some_and(|value| !valid_callback_value(value))
        || query.get("scope").is_some_and(|value| {
            value.is_empty()
                || value
                    .split_ascii_whitespace()
                    .any(|scope| !valid_scope(scope))
        })
    {
        return Err(OAuthError::new(OAuthErrorCode::CallbackFailed));
    }
    let callback = Callback {
        code: query.get("code").cloned(),
        state: query.get("state").cloned(),
        denied,
    };
    let _ = stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\nYou may close this window.",
    );
    Ok(callback)
}

/// Host-owned token store abstraction. Implementations must keep all token bytes in protected
/// storage and must invoke callbacks synchronously without retaining their arguments.
pub trait OAuthTokenStore: Send + Sync {
    /// Persist a token response.
    fn save(&self, provider: &str, response: &TokenResponse) -> Result<(), OAuthError>;
    /// Supply a protected access token to a host-only callback.
    fn with_access_token(
        &self,
        provider: &str,
        callback: &mut dyn FnMut(SecretToken<'_>) -> Result<(), OAuthError>,
    ) -> Result<(), OAuthError>;
    /// Supply a protected refresh token to a host-only callback.
    fn with_refresh_token(
        &self,
        provider: &str,
        callback: &mut dyn FnMut(SecretToken<'_>) -> Result<(), OAuthError>,
    ) -> Result<(), OAuthError>;
    /// Supply a configured client secret to a host-only callback.
    fn with_client_secret(
        &self,
        reference: &ProtectedSecretReference,
        callback: &mut dyn FnMut(SecretToken<'_>) -> Result<(), OAuthError>,
    ) -> Result<(), OAuthError>;
    /// Return the protected token lifecycle state.
    fn state(&self, provider: &str) -> Result<ProtectedSecretState, OAuthError>;
    /// Remove local state and leave no revoked marker.
    fn purge(&self, provider: &str) -> Result<(), OAuthError>;
    /// Revoke locally, deleting token material and retaining a revoked marker.
    fn revoke(&self, provider: &str) -> Result<(), OAuthError>;
}

/// Protected-store implementation for OAuth access/refresh tokens and confidential secrets.
pub struct ProtectedOAuthTokenStore<B> {
    store: ProtectedSecretStore<B>,
    principal: PluginPrincipal,
    environment: ApplicationEnvironment,
}

impl<B> ProtectedOAuthTokenStore<B> {
    /// Bind a protected store to one verified application/environment partition.
    #[must_use]
    pub fn new(
        store: ProtectedSecretStore<B>,
        principal: PluginPrincipal,
        environment: ApplicationEnvironment,
    ) -> Self {
        Self {
            store,
            principal,
            environment,
        }
    }
}

impl<B> fmt::Debug for ProtectedOAuthTokenStore<B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProtectedOAuthTokenStore(REDACTED)")
    }
}

impl<B: CredentialBackend + Send + Sync> OAuthTokenStore for ProtectedOAuthTokenStore<B> {
    fn save(&self, provider: &str, response: &TokenResponse) -> Result<(), OAuthError> {
        let key = token_key(provider)?;
        let mut encoded = Vec::with_capacity(TOKEN_RECORD_PREFIX.len() + 32);
        encoded.extend_from_slice(TOKEN_RECORD_PREFIX);
        append_bytes(&mut encoded, response.access_token.as_slice())?;
        match response.refresh() {
            Some(refresh) => append_bytes(&mut encoded, refresh.as_bytes())?,
            None => encoded.extend_from_slice(&u32::MAX.to_be_bytes()),
        }
        let expires_at = response
            .expires_in
            .map(|seconds| {
                unix_epoch_seconds()
                    .checked_add(seconds)
                    .ok_or_else(|| OAuthError::new(OAuthErrorCode::TokenExchangeFailed))
            })
            .transpose()?
            .unwrap_or(u64::MAX);
        encoded.extend_from_slice(&expires_at.to_be_bytes());
        let scope = self.scope()?;
        scope
            .configure(&key, SecretInput::new(encoded).map_err(map_secret_error)?)
            .map(|_| ())
            .map_err(map_secret_error)
    }

    fn with_access_token(
        &self,
        provider: &str,
        callback: &mut dyn FnMut(SecretToken<'_>) -> Result<(), OAuthError>,
    ) -> Result<(), OAuthError> {
        self.with_token(provider, callback, true)
    }

    fn with_refresh_token(
        &self,
        provider: &str,
        callback: &mut dyn FnMut(SecretToken<'_>) -> Result<(), OAuthError>,
    ) -> Result<(), OAuthError> {
        self.with_token(provider, callback, false)
    }

    fn with_client_secret(
        &self,
        reference: &ProtectedSecretReference,
        callback: &mut dyn FnMut(SecretToken<'_>) -> Result<(), OAuthError>,
    ) -> Result<(), OAuthError> {
        let key = reference.key()?;
        self.scope()?
            .with_configured_secret(&key, |secret| callback(SecretToken(secret)))
            .map_err(map_secret_error)?
    }

    fn state(&self, provider: &str) -> Result<ProtectedSecretState, OAuthError> {
        self.scope()?
            .status(&token_key(provider)?)
            .map(|status| status.state())
            .map_err(map_secret_error)
    }

    fn purge(&self, provider: &str) -> Result<(), OAuthError> {
        self.scope()?
            .purge(&token_key(provider)?)
            .map_err(map_secret_error)
    }

    fn revoke(&self, provider: &str) -> Result<(), OAuthError> {
        self.scope()?
            .revoke(&token_key(provider)?)
            .map(|_| ())
            .map_err(map_secret_error)
    }
}

impl<B: CredentialBackend> ProtectedOAuthTokenStore<B> {
    fn scope(&self) -> Result<studio_security::ApplicationSecretStore<'_, B>, OAuthError> {
        self.store
            .for_application(&self.principal, self.environment)
            .map_err(map_secret_error)
    }

    fn with_token(
        &self,
        provider: &str,
        callback: &mut dyn FnMut(SecretToken<'_>) -> Result<(), OAuthError>,
        access: bool,
    ) -> Result<(), OAuthError> {
        let key = token_key(provider)?;
        self.scope()?
            .with_configured_secret(&key, |encoded| {
                let record = decode_token_record(encoded)?;
                if token_expired(record.expires_at, unix_epoch_seconds()) {
                    return Err(OAuthError::new(OAuthErrorCode::TokenUnavailable));
                }
                let token = if access {
                    record.access.as_slice()
                } else {
                    record
                        .refresh
                        .as_deref()
                        .ok_or_else(|| OAuthError::new(OAuthErrorCode::RefreshUnavailable))?
                };
                callback(SecretToken(token))
            })
            .map_err(map_secret_error)?
    }
}

struct DecodedTokens {
    access: Zeroizing<Vec<u8>>,
    refresh: Option<Zeroizing<Vec<u8>>>,
    expires_at: Option<u64>,
}

fn append_bytes(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), OAuthError> {
    let length = u32::try_from(bytes.len())
        .map_err(|_| OAuthError::new(OAuthErrorCode::TokenExchangeFailed))?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(bytes);
    Ok(())
}

fn decode_token_record(bytes: &[u8]) -> Result<DecodedTokens, OAuthError> {
    if !bytes.starts_with(TOKEN_RECORD_PREFIX) {
        return Err(OAuthError::new(OAuthErrorCode::StorageUnavailable));
    }
    let mut cursor = TOKEN_RECORD_PREFIX.len();
    let access = read_bytes(bytes, &mut cursor)?;
    if access.is_empty() || access.len() > MAX_TOKEN_BYTES {
        return Err(OAuthError::new(OAuthErrorCode::StorageUnavailable));
    }
    let refresh_length = read_u32(bytes, &mut cursor)?;
    let refresh = if refresh_length == u32::MAX {
        None
    } else {
        let length = usize::try_from(refresh_length)
            .map_err(|_| OAuthError::new(OAuthErrorCode::StorageUnavailable))?;
        let end = cursor
            .checked_add(length)
            .ok_or_else(|| OAuthError::new(OAuthErrorCode::StorageUnavailable))?;
        if end > bytes.len() || length == 0 || length > MAX_TOKEN_BYTES {
            return Err(OAuthError::new(OAuthErrorCode::StorageUnavailable));
        }
        let value = Zeroizing::new(bytes[cursor..end].to_vec());
        cursor = end;
        Some(value)
    };
    let expires_at = read_u64(bytes, &mut cursor)?;
    if cursor != bytes.len() {
        return Err(OAuthError::new(OAuthErrorCode::StorageUnavailable));
    }
    Ok(DecodedTokens {
        access: Zeroizing::new(access),
        refresh,
        expires_at: (expires_at != u64::MAX).then_some(expires_at),
    })
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, OAuthError> {
    let end = cursor
        .checked_add(4)
        .ok_or_else(|| OAuthError::new(OAuthErrorCode::StorageUnavailable))?;
    let value = bytes
        .get(*cursor..end)
        .ok_or_else(|| OAuthError::new(OAuthErrorCode::StorageUnavailable))?
        .try_into()
        .map(u32::from_be_bytes)
        .map_err(|_| OAuthError::new(OAuthErrorCode::StorageUnavailable))?;
    *cursor = end;
    Ok(value)
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, OAuthError> {
    let end = cursor
        .checked_add(8)
        .ok_or_else(|| OAuthError::new(OAuthErrorCode::StorageUnavailable))?;
    let value = bytes
        .get(*cursor..end)
        .ok_or_else(|| OAuthError::new(OAuthErrorCode::StorageUnavailable))?
        .try_into()
        .map(u64::from_be_bytes)
        .map_err(|_| OAuthError::new(OAuthErrorCode::StorageUnavailable))?;
    *cursor = end;
    Ok(value)
}

fn read_bytes(bytes: &[u8], cursor: &mut usize) -> Result<Vec<u8>, OAuthError> {
    let length = usize::try_from(read_u32(bytes, cursor)?)
        .map_err(|_| OAuthError::new(OAuthErrorCode::StorageUnavailable))?;
    let end = cursor
        .checked_add(length)
        .ok_or_else(|| OAuthError::new(OAuthErrorCode::StorageUnavailable))?;
    let value = bytes
        .get(*cursor..end)
        .ok_or_else(|| OAuthError::new(OAuthErrorCode::StorageUnavailable))?
        .to_vec();
    *cursor = end;
    Ok(value)
}

fn unix_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn token_expired(expires_at: Option<u64>, now: u64) -> bool {
    expires_at.is_some_and(|expires_at| now >= expires_at)
}

fn token_key(provider: &str) -> Result<ProtectedSecretKey, OAuthError> {
    if !valid_identifier(provider) {
        return Err(OAuthError::new(OAuthErrorCode::ProviderUnknown));
    }
    ProtectedSecretKey::new(
        format!("oauth.{provider}.tokens"),
        format!("OAuth tokens for {provider}"),
    )
    .map_err(|_| OAuthError::new(OAuthErrorCode::StorageUnavailable))
}

fn map_secret_error(error: ProtectedSecretError) -> OAuthError {
    match error.code() {
        ProtectedSecretErrorCode::SecretUnavailable => {
            OAuthError::new(OAuthErrorCode::TokenUnavailable)
        }
        ProtectedSecretErrorCode::RequestInvalid => OAuthError::new(OAuthErrorCode::PackageInvalid),
        ProtectedSecretErrorCode::InjectionRejected => {
            OAuthError::new(OAuthErrorCode::InjectionRejected)
        }
        ProtectedSecretErrorCode::CredentialRejected
        | ProtectedSecretErrorCode::BackendUnavailable => {
            OAuthError::new(OAuthErrorCode::StorageUnavailable)
        }
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.starts_with(|character: char| character.is_ascii_lowercase())
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        })
}

fn valid_scope(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn has_duplicate_strings(values: &[String]) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    values.iter().any(|value| !seen.insert(value))
}

fn valid_loopback_redirect_uri(value: &str) -> bool {
    let Some(port_and_path) = value.strip_prefix("http://127.0.0.1:") else {
        return false;
    };
    let Some((port, path)) = port_and_path.split_once(CALLBACK_PATH) else {
        return false;
    };
    !port.is_empty() && port.parse::<u16>().is_ok_and(|port| port != 0) && path.is_empty()
}

fn valid_claim_path(path: &ClaimPath) -> bool {
    !path.0.is_empty()
        && path.0.len() <= 256
        && path.0.split('.').all(|component| {
            !component.is_empty()
                && component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
}

fn https_url(value: &str) -> bool {
    value.starts_with("https://")
        && value.len() <= 2048
        && !value.chars().any(char::is_control)
        && value[8..].contains('.')
}

fn percent_encode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(byte));
        } else {
            use fmt::Write as _;
            write!(output, "%{byte:02X}").expect("writing into a String cannot fail");
        }
    }
    output
}

fn percent_decode(value: &str) -> Result<String, OAuthError> {
    let mut output = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2])) else {
                return Err(OAuthError::new(OAuthErrorCode::CallbackFailed));
            };
            output.push(high * 16 + low);
            index += 3;
            continue;
        } else if bytes[index] == b'%' {
            return Err(OAuthError::new(OAuthErrorCode::CallbackFailed));
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(output).map_err(|_| OAuthError::new(OAuthErrorCode::CallbackFailed))
}

fn valid_callback_value(value: &str) -> bool {
    !value.is_empty() && value.len() <= 2048 && !value.chars().any(char::is_control)
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

/// Host-generated PKCE material. The verifier is never rendered, serialized, or logged.
pub struct PkcePair {
    verifier: Zeroizing<Vec<u8>>,
    challenge: String,
}

impl PkcePair {
    fn generate(entropy: &dyn EntropySource) -> Result<Self, OAuthError> {
        let mut random = [0_u8; 32];
        entropy.fill(&mut random)?;
        let verifier = Zeroizing::new(base64_url(&random).into_bytes());
        let challenge = base64_url(Sha256::digest(verifier.as_slice()).as_slice());
        random.zeroize();
        Ok(Self {
            verifier,
            challenge,
        })
    }

    /// Public challenge placed in the authorization URL.
    #[must_use]
    pub fn challenge(&self) -> &str {
        &self.challenge
    }

    fn verifier(&self) -> SecretToken<'_> {
        SecretToken(self.verifier.as_slice())
    }
}

impl fmt::Debug for PkcePair {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PkcePair(REDACTED)")
    }
}

fn authorization_url(
    descriptor: &ProviderDescriptor,
    package: &ProviderPackage,
    redirect_uri: &str,
    state: &str,
    pkce: &PkcePair,
) -> String {
    let scopes = package_scope_string(descriptor);
    let separator = if descriptor.authorization_endpoint.contains('?') {
        '&'
    } else {
        '?'
    };
    format!(
        "{}{separator}response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        descriptor.authorization_endpoint,
        percent_encode(&package.client_id),
        percent_encode(redirect_uri),
        percent_encode(&scopes),
        percent_encode(state),
        percent_encode(pkce.challenge()),
    )
}

fn package_scope_string(descriptor: &ProviderDescriptor) -> String {
    descriptor.scopes.join(" ")
}

/// Host manager coordinating provider resolution and OAuth actions.
pub struct OAuthManager {
    registry: RwLock<ProviderRegistry>,
    packages: RwLock<BTreeMap<String, ProviderPackage>>,
    store: Arc<dyn OAuthTokenStore>,
    browser: Arc<dyn BrowserHandoff>,
    listener: Arc<dyn CallbackListener>,
    entropy: Arc<dyn EntropySource>,
    transport: Arc<dyn OAuthTransport>,
    callback_timeout: Duration,
    sessions: Mutex<BTreeMap<String, ApprovedClaims>>,
}

/// Descriptive alias used by hosts that call the coordinator a provider manager.
pub type OAuthProviderManager = OAuthManager;

impl fmt::Debug for OAuthManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthManager")
            .field(
                "packages",
                &self.packages.read().map(|p| p.len()).unwrap_or(0),
            )
            .finish_non_exhaustive()
    }
}

impl OAuthManager {
    /// Construct a manager from injectable host seams.
    #[must_use]
    pub fn new(
        registry: ProviderRegistry,
        packages: impl IntoIterator<Item = ProviderPackage>,
        store: Arc<dyn OAuthTokenStore>,
        browser: Arc<dyn BrowserHandoff>,
        listener: Arc<dyn CallbackListener>,
        entropy: Arc<dyn EntropySource>,
        transport: Arc<dyn OAuthTransport>,
    ) -> Self {
        let packages = packages
            .into_iter()
            .map(|package| (package.provider.clone(), package))
            .collect();
        Self {
            registry: RwLock::new(registry),
            packages: RwLock::new(packages),
            store,
            browser,
            listener,
            entropy,
            transport,
            callback_timeout: Duration::from_secs(300),
            sessions: Mutex::new(BTreeMap::new()),
        }
    }

    /// Construct a manager with desktop browser, loopback, and operating-system entropy seams.
    #[must_use]
    pub fn with_defaults(
        registry: ProviderRegistry,
        packages: impl IntoIterator<Item = ProviderPackage>,
        store: Arc<dyn OAuthTokenStore>,
        transport: Arc<dyn OAuthTransport>,
    ) -> Self {
        Self::new(
            registry,
            packages,
            store,
            Arc::new(SystemBrowser),
            Arc::new(TcpLoopbackListener),
            Arc::new(OsEntropy),
            transport,
        )
    }

    /// Set the maximum time spent waiting for a host-owned callback.
    pub fn set_callback_timeout(&mut self, timeout: Duration) {
        self.callback_timeout = timeout;
    }

    /// Install a newer descriptor version without changing authored package code.
    pub fn register_descriptor(&self, descriptor: ProviderDescriptor) -> Result<(), OAuthError> {
        self.registry
            .write()
            .map_err(|_| OAuthError::new(OAuthErrorCode::DescriptorInvalid))?
            .register(descriptor)
    }

    /// Add or replace an enabled package declaration.
    pub fn enable_package(&self, package: ProviderPackage) -> Result<(), OAuthError> {
        let registry = self
            .registry
            .read()
            .map_err(|_| OAuthError::new(OAuthErrorCode::DescriptorInvalid))?;
        registry.resolve(&package)?;
        self.packages
            .write()
            .map_err(|_| OAuthError::new(OAuthErrorCode::PackageInvalid))?
            .insert(package.provider.clone(), package);
        Ok(())
    }

    /// Read current guest-safe status for one provider.
    #[must_use]
    pub fn status(&self, provider: &str) -> OAuthStatus {
        if self.resolve_package(provider).is_err() {
            return OAuthStatus::Unavailable;
        }
        match self.store.state(provider) {
            Ok(ProtectedSecretState::Configured) => {
                let mut probe = |_token: SecretToken<'_>| Ok(());
                match self.store.with_access_token(provider, &mut probe) {
                    Ok(()) => OAuthStatus::Authenticated,
                    Err(error) if error.code() == OAuthErrorCode::TokenUnavailable => {
                        OAuthStatus::Expired
                    }
                    Err(_) => OAuthStatus::Unavailable,
                }
            }
            Ok(ProtectedSecretState::Revoked) => OAuthStatus::Revoked,
            Ok(ProtectedSecretState::Missing) => OAuthStatus::SignedOut,
            Err(_) => OAuthStatus::Unavailable,
        }
    }

    /// Current approved claims, if a successful action mapped them in this host session.
    #[must_use]
    pub fn claims(&self, provider: &str) -> Option<ApprovedClaims> {
        self.sessions
            .lock()
            .ok()
            .and_then(|sessions| sessions.get(provider).cloned())
    }

    /// Start sign-in and return a guest-safe result record.
    #[must_use]
    pub fn sign_in(&self, provider: &str) -> OAuthActionResult {
        match self.try_sign_in(provider) {
            Ok(result) => result,
            Err(error) => OAuthActionResult::failure(
                OAuthAction::SignIn,
                error.code(),
                status_for_error(error.code()),
            ),
        }
    }

    /// Fallible host form of [`Self::sign_in`] for orchestration and deterministic tests.
    pub fn try_sign_in(&self, provider: &str) -> Result<OAuthActionResult, OAuthError> {
        let (descriptor, package) = self.resolve_package(provider)?;
        let mut callback = self
            .listener
            .bind()
            .map_err(|_| OAuthError::new(OAuthErrorCode::CallbackBindFailed))?;
        if !valid_loopback_redirect_uri(callback.redirect_uri()) {
            return Err(OAuthError::new(OAuthErrorCode::CallbackBindFailed));
        }
        let redirect_uri = callback.redirect_uri().to_owned();
        let pkce = PkcePair::generate(self.entropy.as_ref())?;
        let state = self.generate_state()?;
        let url = authorization_url(&descriptor, &package, &redirect_uri, &state, &pkce);
        self.browser.open(&url)?;
        let response = callback.wait(self.callback_timeout)?;
        if callback.redirect_uri() != redirect_uri
            || response
                .code
                .as_deref()
                .is_some_and(|code| !valid_callback_value(code))
        {
            return Err(OAuthError::new(OAuthErrorCode::CallbackFailed));
        }
        if response.state.as_deref() != Some(state.as_str()) {
            return Err(OAuthError::new(OAuthErrorCode::StateMismatch));
        }
        if response.denied || response.code.is_none() {
            return Err(OAuthError::new(OAuthErrorCode::AuthorizationDenied));
        }
        let token_response = self.exchange(
            &descriptor,
            &package,
            response.code.as_deref().unwrap_or_default(),
            &pkce,
            &redirect_uri,
        )?;
        validate_granted_scopes(&descriptor, &token_response)?;
        let claims = self.map_profile(&descriptor, provider, &token_response)?;
        self.store.save(provider, &token_response)?;
        self.sessions
            .lock()
            .map_err(|_| OAuthError::new(OAuthErrorCode::StorageUnavailable))?
            .insert(provider.to_owned(), claims.clone());
        Ok(OAuthActionResult::success(
            OAuthAction::SignIn,
            Some(claims),
        ))
    }

    fn generate_state(&self) -> Result<String, OAuthError> {
        let mut bytes = [0_u8; 32];
        self.entropy.fill(&mut bytes)?;
        let value = base64_url(&bytes);
        bytes.zeroize();
        Ok(value)
    }

    fn resolve_package(
        &self,
        provider: &str,
    ) -> Result<(ProviderDescriptor, ProviderPackage), OAuthError> {
        let packages = self
            .packages
            .read()
            .map_err(|_| OAuthError::new(OAuthErrorCode::PackageInvalid))?;
        let package = packages
            .get(provider)
            .ok_or_else(|| OAuthError::new(OAuthErrorCode::ProviderUnknown))?;
        let registry = self
            .registry
            .read()
            .map_err(|_| OAuthError::new(OAuthErrorCode::DescriptorInvalid))?;
        let descriptor = registry.resolve(package)?.clone();
        Ok((descriptor, package.clone()))
    }

    fn exchange(
        &self,
        descriptor: &ProviderDescriptor,
        package: &ProviderPackage,
        code: &str,
        pkce: &PkcePair,
        redirect_uri: &str,
    ) -> Result<TokenResponse, OAuthError> {
        match descriptor.client_authentication {
            ClientAuthentication::Pkce => self.transport.exchange_code(CodeExchangeRequest {
                endpoint: &descriptor.token_endpoint,
                client_id: &package.client_id,
                code,
                verifier: Some(pkce.verifier()),
                client_secret: None,
                redirect_uri,
            }),
            ClientAuthentication::ConfidentialClient => {
                let reference = package
                    .client_secret
                    .as_ref()
                    .ok_or_else(|| OAuthError::new(OAuthErrorCode::ClientSecretUnavailable))?;
                let mut result = None;
                self.store.with_client_secret(reference, &mut |secret| {
                    result = Some(self.transport.exchange_code(CodeExchangeRequest {
                        endpoint: &descriptor.token_endpoint,
                        client_id: &package.client_id,
                        code,
                        verifier: None,
                        client_secret: Some(secret),
                        redirect_uri,
                    }));
                    Ok(())
                })?;
                result.unwrap_or_else(|| Err(OAuthError::new(OAuthErrorCode::TokenExchangeFailed)))
            }
        }
    }

    fn map_profile(
        &self,
        descriptor: &ProviderDescriptor,
        provider: &str,
        response: &TokenResponse,
    ) -> Result<ApprovedClaims, OAuthError> {
        let mut profile = None;
        self.with_response_access_token(provider, response, &mut |token| {
            profile = Some(self.transport.profile(ProfileRequest {
                endpoint: &descriptor.profile.endpoint,
                access_token: token,
            }));
            Ok(())
        })?;
        let profile = profile.ok_or_else(|| OAuthError::new(OAuthErrorCode::ProfileFailed))??;
        let mut claims = map_claims(&descriptor.profile, &profile)?;
        if claims.email.is_none()
            && descriptor.quirks.private_email_fallback
            && let Some(fallback) = &descriptor.profile.email_fallback
        {
            let mut fallback_response = None;
            self.with_response_access_token(provider, response, &mut |token| {
                fallback_response = Some(self.transport.profile(ProfileRequest {
                    endpoint: &fallback.endpoint,
                    access_token: token,
                }));
                Ok(())
            })?;
            let fallback_response = fallback_response
                .ok_or_else(|| OAuthError::new(OAuthErrorCode::ProfileFailed))??;
            claims.email = fallback_email(fallback, &fallback_response);
        }
        Ok(claims)
    }

    fn with_response_access_token(
        &self,
        _provider: &str,
        response: &TokenResponse,
        callback: &mut dyn FnMut(SecretToken<'_>) -> Result<(), OAuthError>,
    ) -> Result<(), OAuthError> {
        callback(response.access())
    }

    /// Refresh a session and return a guest-safe result record.
    #[must_use]
    pub fn refresh(&self, provider: &str) -> OAuthActionResult {
        match self.try_refresh(provider) {
            Ok(result) => result,
            Err(error) => OAuthActionResult::failure(
                OAuthAction::Refresh,
                error.code(),
                status_for_error(error.code()),
            ),
        }
    }

    /// Fallible host form of [`Self::refresh`].
    pub fn try_refresh(&self, provider: &str) -> Result<OAuthActionResult, OAuthError> {
        let (descriptor, package) = self.resolve_package(provider)?;
        if descriptor.quirks.no_refresh_tokens {
            return Err(OAuthError::new(OAuthErrorCode::RefreshUnavailable));
        }
        let mut refreshed = None;
        self.store
            .with_refresh_token(provider, &mut |refresh_token| {
                let response = match descriptor.client_authentication {
                    ClientAuthentication::Pkce => self.transport.refresh(RefreshRequest {
                        endpoint: &descriptor.token_endpoint,
                        client_id: &package.client_id,
                        refresh_token,
                        client_secret: None,
                    }),
                    ClientAuthentication::ConfidentialClient => {
                        let reference = package.client_secret.as_ref().ok_or_else(|| {
                            OAuthError::new(OAuthErrorCode::ClientSecretUnavailable)
                        })?;
                        let mut result = None;
                        self.store.with_client_secret(reference, &mut |secret| {
                            result = Some(self.transport.refresh(RefreshRequest {
                                endpoint: &descriptor.token_endpoint,
                                client_id: &package.client_id,
                                refresh_token,
                                client_secret: Some(secret),
                            }));
                            Ok(())
                        })?;
                        result.unwrap_or_else(|| {
                            Err(OAuthError::new(OAuthErrorCode::TokenExchangeFailed))
                        })
                    }
                }?;
                refreshed = Some(response);
                Ok(())
            })?;
        let response =
            refreshed.ok_or_else(|| OAuthError::new(OAuthErrorCode::RefreshUnavailable))?;
        validate_granted_scopes(&descriptor, &response)?;
        let claims = self.map_profile(&descriptor, provider, &response)?;
        self.store.save(provider, &response)?;
        self.sessions
            .lock()
            .map_err(|_| OAuthError::new(OAuthErrorCode::StorageUnavailable))?
            .insert(provider.to_owned(), claims.clone());
        Ok(OAuthActionResult::success(
            OAuthAction::Refresh,
            Some(claims),
        ))
    }
}

fn base64_url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut output = String::with_capacity((bytes.len() * 4).div_ceil(3));
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let value = u32::from(bytes[index]) << 16
            | u32::from(bytes[index + 1]) << 8
            | u32::from(bytes[index + 2]);
        output.push(char::from(ALPHABET[((value >> 18) & 63) as usize]));
        output.push(char::from(ALPHABET[((value >> 12) & 63) as usize]));
        output.push(char::from(ALPHABET[((value >> 6) & 63) as usize]));
        output.push(char::from(ALPHABET[(value & 63) as usize]));
        index += 3;
    }
    let remainder = bytes.len() - index;
    if remainder == 1 {
        let value = u32::from(bytes[index]) << 16;
        output.push(char::from(ALPHABET[((value >> 18) & 63) as usize]));
        output.push(char::from(ALPHABET[((value >> 12) & 63) as usize]));
    } else if remainder == 2 {
        let value = u32::from(bytes[index]) << 16 | u32::from(bytes[index + 1]) << 8;
        output.push(char::from(ALPHABET[((value >> 18) & 63) as usize]));
        output.push(char::from(ALPHABET[((value >> 12) & 63) as usize]));
        output.push(char::from(ALPHABET[((value >> 6) & 63) as usize]));
    }
    output
}

impl OAuthManager {
    /// Remove local credentials without making a provider request.
    #[must_use]
    pub fn sign_out(&self, provider: &str) -> OAuthActionResult {
        match self.store.purge(provider) {
            Ok(()) => {
                if let Ok(mut sessions) = self.sessions.lock() {
                    sessions.remove(provider);
                }
                OAuthActionResult {
                    action: OAuthAction::SignOut,
                    code: OAuthActionCode::Success,
                    status: OAuthStatus::SignedOut,
                    claims: None,
                }
            }
            Err(error) => OAuthActionResult::failure(
                OAuthAction::SignOut,
                error.code(),
                OAuthStatus::Unavailable,
            ),
        }
    }

    /// Revoke upstream when possible, then always clear local token material and retain a revoked
    /// marker so the application observes the revoked state.
    #[must_use]
    pub fn revoke(&self, provider: &str) -> OAuthActionResult {
        let (descriptor, package) = match self.resolve_package(provider) {
            Ok(value) => value,
            Err(error) => {
                return OAuthActionResult::failure(
                    OAuthAction::Revoke,
                    error.code(),
                    OAuthStatus::Unavailable,
                );
            }
        };
        let mut upstream = Ok(());
        if let Some(endpoint) = descriptor.revocation_endpoint.as_deref() {
            let endpoint = endpoint.replace("{clientId}", &percent_encode(&package.client_id));
            let mut result = None;
            let callback_result = self.store.with_access_token(provider, &mut |token| {
                result = Some(self.transport.revoke(RevokeRequest {
                    endpoint: &endpoint,
                    client_id: &package.client_id,
                    access_token: token,
                }));
                Ok(())
            });
            upstream = callback_result.and_then(|_| {
                result.unwrap_or_else(|| Err(OAuthError::new(OAuthErrorCode::TokenUnavailable)))
            });
        }
        let local = self.store.revoke(provider);
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.remove(provider);
        }
        match (upstream, local) {
            (_, Err(error)) => OAuthActionResult::failure(
                OAuthAction::Revoke,
                error.code(),
                OAuthStatus::Unavailable,
            ),
            (Err(_), Ok(())) => OAuthActionResult::failure(
                OAuthAction::Revoke,
                OAuthErrorCode::RevokeFailed,
                OAuthStatus::Revoked,
            ),
            (Ok(()), Ok(())) => OAuthActionResult {
                action: OAuthAction::Revoke,
                code: OAuthActionCode::Success,
                status: OAuthStatus::Revoked,
                claims: None,
            },
        }
    }

    fn inject(
        &self,
        provider: &str,
        sink: &mut dyn BrokerCredentialSink,
    ) -> Result<(), BrokerError> {
        if self.resolve_package(provider).is_err() {
            return Err(BrokerError::new(BrokerErrorCode::OauthSessionUnavailable));
        }
        self.store
            .with_access_token(provider, &mut |token| {
                sink.inject(token.as_bytes())
                    .map_err(|_| OAuthError::new(OAuthErrorCode::InjectionRejected))
            })
            .map_err(|_| BrokerError::new(BrokerErrorCode::OauthSessionUnavailable))
    }
}

impl studio_net::credential::OAuthSessionResolver for OAuthManager {
    fn inject_session(
        &self,
        provider: &str,
        _route_group_id: &str,
        sink: &mut dyn BrokerCredentialSink,
    ) -> Result<(), BrokerError> {
        self.inject(provider, sink)
    }
}

fn status_for_error(code: OAuthErrorCode) -> OAuthStatus {
    match code {
        OAuthErrorCode::ProviderUnknown | OAuthErrorCode::ProviderOutdated => {
            OAuthStatus::Unavailable
        }
        OAuthErrorCode::StateMismatch
        | OAuthErrorCode::AuthorizationDenied
        | OAuthErrorCode::BrowserUnavailable
        | OAuthErrorCode::CallbackBindFailed
        | OAuthErrorCode::CallbackFailed => OAuthStatus::SignedOut,
        OAuthErrorCode::TokenUnavailable | OAuthErrorCode::RefreshUnavailable => {
            OAuthStatus::Expired
        }
        OAuthErrorCode::StorageUnavailable => OAuthStatus::Unavailable,
        _ => OAuthStatus::SignedOut,
    }
}

fn validate_granted_scopes(
    descriptor: &ProviderDescriptor,
    response: &TokenResponse,
) -> Result<(), OAuthError> {
    let Some(granted) = &response.scopes else {
        return Ok(());
    };
    if has_duplicate_strings(granted)
        || granted.len() != descriptor.scopes.len()
        || descriptor
            .scopes
            .iter()
            .any(|scope| !granted.iter().any(|candidate| candidate == scope))
    {
        return Err(OAuthError::new(OAuthErrorCode::TokenExchangeFailed));
    }
    Ok(())
}

fn map_claims(mapping: &ProfileMapping, profile: &Value) -> Result<ApprovedClaims, OAuthError> {
    let subject = claim_subject(&mapping.subject, profile)
        .ok_or_else(|| OAuthError::new(OAuthErrorCode::ClaimsInvalid))?;
    Ok(ApprovedClaims {
        subject,
        login: mapping
            .login
            .as_ref()
            .and_then(|path| claim_string(path, profile)),
        display_name: mapping
            .display_name
            .as_ref()
            .and_then(|path| claim_string(path, profile)),
        avatar_url: mapping
            .avatar_url
            .as_ref()
            .and_then(|path| claim_string(path, profile)),
        email: mapping
            .email
            .as_ref()
            .and_then(|path| claim_string(path, profile)),
        profile_url: mapping
            .profile_url
            .as_ref()
            .and_then(|path| claim_string(path, profile)),
    })
}

fn claim_subject(path: &ClaimPath, root: &Value) -> Option<String> {
    let value = path.value(root)?;
    match value {
        Value::String(value)
            if !value.is_empty() && value.len() <= 2048 && !value.chars().any(char::is_control) =>
        {
            Some(value.clone())
        }
        Value::Number(value) => {
            let value = value.to_string();
            (value.len() <= 2048).then_some(value)
        }
        _ => None,
    }
}

fn claim_string(path: &ClaimPath, root: &Value) -> Option<String> {
    let value = path.value(root)?.as_str()?;
    if value.is_empty() || value.len() > 2048 || value.chars().any(char::is_control) {
        return None;
    }
    Some(value.to_owned())
}

fn fallback_email(mapping: &EmailFallbackMapping, response: &Value) -> Option<String> {
    response.as_array()?.iter().find_map(|entry| {
        let primary = mapping.primary.value(entry)?.as_bool()?;
        let verified = mapping.verified.value(entry)?.as_bool()?;
        if primary && verified {
            claim_string(&mapping.email, entry)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_descriptor_is_read_only_and_rejects_broad_repository_scope() {
        let descriptor = ProviderDescriptor::github();
        descriptor.validate().unwrap();
        assert_eq!(
            descriptor.scopes,
            vec![String::from("read:user"), String::from("user:email")]
        );

        let mut broad = descriptor;
        broad.scopes.push(String::from("repo"));
        assert_eq!(
            broad.validate().unwrap_err().code(),
            OAuthErrorCode::DescriptorInvalid
        );
    }

    #[test]
    fn callback_decoding_is_strict_and_redirects_are_loopback_only() {
        assert_eq!(percent_decode("read%3Auser").unwrap(), "read:user");
        assert!(percent_decode("bad%2").is_err());
        assert!(valid_loopback_redirect_uri(
            "http://127.0.0.1:43121/oauth/callback"
        ));
        for redirect in [
            "http://localhost:43121/oauth/callback",
            "https://127.0.0.1:43121/oauth/callback",
            "http://127.0.0.1:0/oauth/callback",
            "http://127.0.0.1:43121/oauth/callback?state=leak",
        ] {
            assert!(!valid_loopback_redirect_uri(redirect));
        }
    }

    #[test]
    fn granted_scopes_must_match_the_descriptor_when_reported() {
        let descriptor = ProviderDescriptor::github();
        let response = TokenResponse::new(b"access".to_vec(), None, None)
            .unwrap()
            .with_scopes(["read:user", "user:email"])
            .unwrap();
        validate_granted_scopes(&descriptor, &response).unwrap();

        let broad = TokenResponse::new(b"access".to_vec(), None, None)
            .unwrap()
            .with_scopes(["read:user", "repo"])
            .unwrap();
        assert_eq!(
            validate_granted_scopes(&descriptor, &broad)
                .unwrap_err()
                .code(),
            OAuthErrorCode::TokenExchangeFailed
        );
    }

    #[test]
    fn persisted_token_expiry_is_checked_before_injection() {
        assert!(!token_expired(None, u64::MAX));
        assert!(!token_expired(Some(101), 100));
        assert!(token_expired(Some(100), 100));
    }

    #[test]
    fn authorization_url_contains_only_the_declared_scopes() {
        let descriptor = ProviderDescriptor::github();
        let package = ProviderPackage::new("github", "1.0.0", "client");
        let pkce = PkcePair {
            verifier: Zeroizing::new(b"verifier".to_vec()),
            challenge: String::from("challenge"),
        };
        let url = authorization_url(
            &descriptor,
            &package,
            "http://127.0.0.1:43121/oauth/callback",
            "state",
            &pkce,
        );
        assert!(url.contains("scope=read%3Auser%20user%3Aemail"));
        assert!(!url.contains("repo"));
    }

    #[test]
    fn github_numeric_subjects_map_to_guest_safe_claims() {
        let descriptor = ProviderDescriptor::github();
        let claims = map_claims(
            &descriptor.profile,
            &serde_json::json!({"id": 42, "login": "octocat"}),
        )
        .unwrap();
        assert_eq!(claims.subject, "42");
        assert_eq!(claims.login.as_deref(), Some("octocat"));
    }
}
