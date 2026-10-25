//! Provider capability admission for signed application manifests.
//!
//! Provider descriptors are host-maintained policy, not package-supplied code. Admission resolves
//! an exact integration id/version, checks its state and compatibility, then compiles every
//! declared route once. The resulting plan is safe to hand to the broker: it contains no raw
//! manifest strings that need to be interpreted again at request time.

#![allow(clippy::assigning_clones)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::too_many_lines)]

use std::collections::{BTreeMap, BTreeSet};

use semver::Version;
use studio_net::BrokerError;
use studio_net::broker::RestBroker;
use studio_net::declaration::{CredentialSource, HttpMethod, RouteGroupDeclaration};

use crate::{Capability, IntegrationReference, ManifestV1};

/// Current provider descriptor schema understood by the host.
pub const PROVIDER_DESCRIPTOR_SCHEMA_VERSION: u16 = 1;
/// Maintained GitHub integration id.
pub const GITHUB_PROVIDER_ID: &str = "github";
/// Maintained GitHub integration descriptor version.
pub const GITHUB_PROVIDER_VERSION: &str = "1.1.0";
/// Maintained Apple integration id.
pub const APPLE_PROVIDER_ID: &str = "apple";
/// Maintained Apple integration descriptor version.
pub const APPLE_PROVIDER_VERSION: &str = "1.0.0";
/// Scopes any maintained Apple descriptor version may request.
pub const APPLE_ALLOWED_SCOPES: &[&str] = &["name", "email"];
/// Maintained Discord integration id.
pub const DISCORD_PROVIDER_ID: &str = "discord";
/// Maintained Discord integration descriptor version.
pub const DISCORD_PROVIDER_VERSION: &str = "1.0.0";
/// Scopes any maintained Discord descriptor version may request.
pub const DISCORD_ALLOWED_SCOPES: &[&str] = &["email", "identify"];
/// Maintained Facebook integration id.
pub const FACEBOOK_PROVIDER_ID: &str = "facebook";
/// Maintained Facebook integration descriptor version.
pub const FACEBOOK_PROVIDER_VERSION: &str = "1.0.0";
/// Scopes any maintained Facebook descriptor version may request.
pub const FACEBOOK_ALLOWED_SCOPES: &[&str] = &["email", "public_profile"];
/// Maintained GitLab integration id.
pub const GITLAB_PROVIDER_ID: &str = "gitlab";
/// Maintained GitLab integration descriptor version.
pub const GITLAB_PROVIDER_VERSION: &str = "1.0.0";
/// Scopes any maintained GitLab descriptor version may request.
pub const GITLAB_ALLOWED_SCOPES: &[&str] = &["email", "openid", "profile"];
/// Maintained Google integration id.
pub const GOOGLE_PROVIDER_ID: &str = "google";
/// Maintained Google integration descriptor version.
pub const GOOGLE_PROVIDER_VERSION: &str = "1.0.0";
/// Scopes any maintained Google descriptor version may request.
pub const GOOGLE_ALLOWED_SCOPES: &[&str] = &["openid", "email", "profile"];
/// Maintained `LinkedIn` integration id.
pub const LINKEDIN_PROVIDER_ID: &str = "linkedin";
/// Maintained `LinkedIn` integration descriptor version.
pub const LINKEDIN_PROVIDER_VERSION: &str = "1.0.0";
/// Scopes any maintained `LinkedIn` descriptor version may request.
pub const LINKEDIN_ALLOWED_SCOPES: &[&str] = &["email", "openid", "profile"];
/// Maintained Microsoft identity integration id.
pub const MICROSOFT_PROVIDER_ID: &str = "microsoft";
/// Maintained Microsoft identity integration descriptor version.
pub const MICROSOFT_PROVIDER_VERSION: &str = "1.0.0";
/// Scopes any maintained Microsoft identity descriptor version may request.
pub const MICROSOFT_ALLOWED_SCOPES: &[&str] = &["email", "offline_access", "openid", "profile"];
/// Maintained Spotify integration id.
pub const SPOTIFY_PROVIDER_ID: &str = "spotify";
/// Maintained Spotify integration descriptor version.
pub const SPOTIFY_PROVIDER_VERSION: &str = "1.0.0";
/// Scopes any maintained Spotify descriptor version may request.
pub const SPOTIFY_ALLOWED_SCOPES: &[&str] = &["email", "openid", "profile"];
/// Maintained provider-neutral AI integration id.
pub const AI_PROVIDER_ID: &str = "ai";
/// Maintained provider-neutral AI integration descriptor version.
pub const AI_PROVIDER_VERSION: &str = "1.0.0";
/// Reserved integration id prefix for an application-declared `OpenID` Connect issuer.
///
/// A declared issuer is not in the maintained provider catalog: the application names the
/// issuer it wants to sign in with, and the host admits it as a public, identity-only client.
pub const DECLARED_OIDC_ID_PREFIX: &str = "oidc-";
/// The one descriptor version a declared issuer integration may declare.
pub const DECLARED_OIDC_DESCRIPTOR_VERSION: &str = "1.0.0";
/// The `OpenID` Connect standard scopes the host grants a declared issuer.
///
/// The host owns this ceiling: a declared issuer's own API scopes are not reachable, however
/// the application asks for them.
pub const DECLARED_OIDC_ALLOWED_SCOPES: &[&str] = &["openid", "profile", "email", "offline_access"];

/// Host state of an installed descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderDescriptorState {
    /// Descriptor may be selected by a package.
    Active,
    /// Descriptor has been withdrawn and cannot be selected.
    Revoked,
    /// Descriptor is retained for diagnostics but is no longer installable.
    Outdated,
    /// Descriptor is present but incompatible with this host policy.
    Incompatible,
}

/// One provider-owned route policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderRouteDescriptor {
    /// Route group identity.
    pub id: String,
    /// Exact origins allowed by the descriptor.
    pub origins: Vec<String>,
    /// Allowed request methods.
    pub methods: Vec<HttpMethod>,
    /// Exact path patterns allowed by the descriptor.
    pub paths: Vec<String>,
    /// Guest-settable headers allowed by the descriptor.
    pub allowed_headers: Vec<String>,
    /// Required credential strategy.
    pub credential: ProviderCredentialPolicy,
    /// Whether this route is an admitted server-sent-event stream.
    pub streaming: bool,
}

/// Provider-owned credential requirement for one route.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(missing_docs)]
pub enum ProviderCredentialPolicy {
    /// Route uses the host session for this provider.
    OauthProviderSession { provider: String },
    /// Route uses a named protected secret and fixed header mapping.
    NamedSecret {
        /// Protected secret name, or a value supplied by the integration config.
        name: String,
        /// Header receiving the host-injected value.
        header: String,
        /// Optional static header prefix.
        prefix: Option<String>,
    },
    /// Route sends no credentials.
    Public,
}

/// A host-maintained provider descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDescriptor {
    /// Descriptor schema major.
    pub schema_version: u16,
    /// Stable integration id.
    pub id: String,
    /// Exact semantic descriptor version.
    pub version: String,
    /// Admission state.
    pub state: ProviderDescriptorState,
    /// Whether the provider enables sign-in without granting any REST route group.
    pub identity_only: bool,
    /// Provider route policies.
    pub routes: Vec<ProviderRouteDescriptor>,
    /// Scopes accepted in integration configuration.
    pub scopes: Vec<String>,
    /// Protected secret names required by the integration configuration.
    pub required_secret_names: Vec<String>,
    /// Optional exact purposes required for fixed provider secret names.
    pub required_secret_purposes: BTreeMap<String, String>,
    /// Config key containing a dynamic route origin, used by provider-neutral endpoints.
    pub origin_config_key: Option<String>,
    /// Config key containing the protected API key name, when applicable.
    pub secret_config_key: Option<String>,
    /// Config key containing a public client id, when applicable.
    pub client_id_config_key: Option<String>,
}

impl ProviderDescriptor {
    /// Construct a descriptor from explicit host policy.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        routes: Vec<ProviderRouteDescriptor>,
    ) -> Self {
        Self {
            schema_version: PROVIDER_DESCRIPTOR_SCHEMA_VERSION,
            id: id.into(),
            version: version.into(),
            state: ProviderDescriptorState::Active,
            identity_only: false,
            routes,
            scopes: Vec::new(),
            required_secret_names: Vec::new(),
            required_secret_purposes: BTreeMap::new(),
            origin_config_key: None,
            secret_config_key: None,
            client_id_config_key: None,
        }
    }

    /// Return the maintained GitHub descriptor.
    #[must_use]
    pub fn github() -> Self {
        let mut descriptor = Self::new(
            GITHUB_PROVIDER_ID,
            GITHUB_PROVIDER_VERSION,
            vec![
                github_route("github.user", "/user"),
                github_route("github.repositories", "/user/repos"),
                github_route("github.repository", "/repos/{owner}/{repo}"),
            ],
        );
        descriptor.scopes = ["read:user", "user:email"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        descriptor.required_secret_names = vec!["github.oauth.client_secret".to_owned()];
        descriptor.required_secret_purposes.insert(
            "github.oauth.client_secret".to_owned(),
            "GitHub OAuth client configuration".to_owned(),
        );
        descriptor.secret_config_key = Some("clientSecretName".to_owned());
        descriptor.client_id_config_key = Some("clientId".to_owned());
        descriptor
    }

    /// Return the maintained Apple identity descriptor.
    ///
    /// Apple grants identity only: the descriptor declares no REST route group, so an
    /// application may sign in but cannot call the provider's API through the host broker.
    #[must_use]
    pub fn apple() -> Self {
        identity_descriptor(
            APPLE_PROVIDER_ID,
            APPLE_PROVIDER_VERSION,
            APPLE_ALLOWED_SCOPES,
            "Apple OAuth client configuration",
        )
    }

    /// Return the maintained Discord identity descriptor.
    #[must_use]
    pub fn discord() -> Self {
        identity_descriptor(
            DISCORD_PROVIDER_ID,
            DISCORD_PROVIDER_VERSION,
            DISCORD_ALLOWED_SCOPES,
            "Discord OAuth client configuration",
        )
    }

    /// Return the maintained Facebook identity descriptor.
    #[must_use]
    pub fn facebook() -> Self {
        identity_descriptor(
            FACEBOOK_PROVIDER_ID,
            FACEBOOK_PROVIDER_VERSION,
            FACEBOOK_ALLOWED_SCOPES,
            "Facebook OAuth client configuration",
        )
    }

    /// Return the maintained GitLab identity descriptor.
    #[must_use]
    pub fn gitlab() -> Self {
        identity_descriptor(
            GITLAB_PROVIDER_ID,
            GITLAB_PROVIDER_VERSION,
            GITLAB_ALLOWED_SCOPES,
            "GitLab OAuth client configuration",
        )
    }

    /// Return the maintained Google identity descriptor.
    #[must_use]
    pub fn google() -> Self {
        identity_descriptor(
            GOOGLE_PROVIDER_ID,
            GOOGLE_PROVIDER_VERSION,
            GOOGLE_ALLOWED_SCOPES,
            "Google OAuth client configuration",
        )
    }

    /// Return the maintained `LinkedIn` identity descriptor.
    #[must_use]
    pub fn linkedin() -> Self {
        identity_descriptor(
            LINKEDIN_PROVIDER_ID,
            LINKEDIN_PROVIDER_VERSION,
            LINKEDIN_ALLOWED_SCOPES,
            "LinkedIn OAuth client configuration",
        )
    }

    /// Return the maintained Microsoft identity descriptor.
    #[must_use]
    pub fn microsoft() -> Self {
        identity_descriptor(
            MICROSOFT_PROVIDER_ID,
            MICROSOFT_PROVIDER_VERSION,
            MICROSOFT_ALLOWED_SCOPES,
            "Microsoft OAuth client configuration",
        )
    }

    /// Return the maintained Spotify identity descriptor.
    #[must_use]
    pub fn spotify() -> Self {
        identity_descriptor(
            SPOTIFY_PROVIDER_ID,
            SPOTIFY_PROVIDER_VERSION,
            SPOTIFY_ALLOWED_SCOPES,
            "Spotify OAuth client configuration",
        )
    }

    /// Return the maintained provider-neutral AI descriptor.
    #[must_use]
    pub fn ai() -> Self {
        let mut descriptor = Self::new(
            AI_PROVIDER_ID,
            AI_PROVIDER_VERSION,
            vec![
                ProviderRouteDescriptor {
                    id: "ai.chat.completions".to_owned(),
                    origins: vec!["https://api.openai.com".to_owned()],
                    methods: vec![HttpMethod::Post],
                    paths: vec!["/v1/chat/completions".to_owned()],
                    allowed_headers: vec!["content-type".to_owned(), "accept".to_owned()],
                    credential: ProviderCredentialPolicy::NamedSecret {
                        name: "openai.api_key".to_owned(),
                        header: "authorization".to_owned(),
                        prefix: Some("Bearer ".to_owned()),
                    },
                    streaming: false,
                },
                ProviderRouteDescriptor {
                    id: "ai.chat.completions.stream".to_owned(),
                    origins: vec!["https://api.openai.com".to_owned()],
                    methods: vec![HttpMethod::Get],
                    paths: vec!["/v1/chat/completions/stream".to_owned()],
                    allowed_headers: vec!["accept".to_owned()],
                    credential: ProviderCredentialPolicy::NamedSecret {
                        name: "openai.api_key".to_owned(),
                        header: "authorization".to_owned(),
                        prefix: Some("Bearer ".to_owned()),
                    },
                    streaming: true,
                },
            ],
        );
        descriptor.origin_config_key = Some("origin".to_owned());
        descriptor.secret_config_key = Some("apiKeyName".to_owned());
        descriptor
    }

    /// Construct the admission descriptor for an application-declared `OpenID` Connect issuer.
    ///
    /// A declared issuer is a public client: the application holds a client id and no secret, so
    /// the descriptor requires no protected secret. It grants no route group either, because the
    /// host does not know the issuer's API surface, and the scopes it accepts are the `OpenID`
    /// Connect standard set the host owns.
    #[must_use]
    pub fn declared_oidc(id: &str, version: &str) -> Self {
        let mut descriptor = Self::new(id, version, Vec::new());
        descriptor.identity_only = true;
        descriptor.scopes = DECLARED_OIDC_ALLOWED_SCOPES
            .iter()
            .map(|scope| (*scope).to_owned())
            .collect();
        descriptor.client_id_config_key = Some("clientId".to_owned());
        descriptor
    }

    fn validate(&self) -> Result<(), ProviderAdmissionError> {
        // An identity-only provider declares no route group at all, and a provider that grants
        // REST access must declare at least one. Either way, a declared route has to be
        // well-formed: the route policy is compiled into the broker, so a malformed one is
        // refused rather than admitted and rejected later at request time.
        let route_count_matches = if self.identity_only {
            self.routes.is_empty()
        } else {
            !self.routes.is_empty()
        };
        if self.schema_version != PROVIDER_DESCRIPTOR_SCHEMA_VERSION
            || !valid_identifier(&self.id)
            || Version::parse(&self.version).is_err()
            || !route_count_matches
            || self.routes.iter().any(|route| {
                route.id.is_empty()
                    || route.origins.is_empty()
                    || route.methods.is_empty()
                    || route.paths.is_empty()
            })
        {
            return Err(ProviderAdmissionError::descriptor_invalid(
                &self.id,
                &self.version,
            ));
        }
        Ok(())
    }
}

/// Build the maintained identity-only admission descriptor for one OAuth provider.
///
/// An identity-only provider enables sign-in without granting any REST surface: the
/// application authenticates the account and reads the approved claims, but the host broker
/// holds no route group for the provider, so the provider's API is unreachable from the guest.
fn identity_descriptor(
    id: &str,
    version: &str,
    scopes: &[&str],
    purpose: &str,
) -> ProviderDescriptor {
    let mut descriptor = ProviderDescriptor::new(id, version, Vec::new());
    descriptor.identity_only = true;
    descriptor.scopes = scopes.iter().map(|scope| (*scope).to_owned()).collect();
    let secret_name = format!("{id}.oauth.client_secret");
    descriptor.required_secret_names = vec![secret_name.clone()];
    descriptor
        .required_secret_purposes
        .insert(secret_name, purpose.to_owned());
    descriptor.secret_config_key = Some("clientSecretName".to_owned());
    descriptor.client_id_config_key = Some("clientId".to_owned());
    descriptor
}

/// Return the maintained OAuth admission descriptors in stable order.
///
/// The host uses this list to provision one protected client secret per provider without
/// re-spelling any provider id, secret name, or purpose, so the admission policy and the
/// host's secret provisioning cannot drift apart.
#[must_use]
pub fn maintained_oauth_descriptors() -> Vec<ProviderDescriptor> {
    vec![
        ProviderDescriptor::github(),
        ProviderDescriptor::apple(),
        ProviderDescriptor::discord(),
        ProviderDescriptor::facebook(),
        ProviderDescriptor::gitlab(),
        ProviderDescriptor::google(),
        ProviderDescriptor::linkedin(),
        ProviderDescriptor::microsoft(),
        ProviderDescriptor::spotify(),
    ]
}

fn github_route(id: &str, path: &str) -> ProviderRouteDescriptor {
    ProviderRouteDescriptor {
        id: id.to_owned(),
        origins: vec!["https://api.github.com".to_owned()],
        methods: vec![HttpMethod::Get],
        paths: vec![path.to_owned()],
        allowed_headers: vec![
            "accept".to_owned(),
            "x-github-api-version".to_owned(),
            "user-agent".to_owned(),
        ],
        credential: ProviderCredentialPolicy::OauthProviderSession {
            provider: GITHUB_PROVIDER_ID.to_owned(),
        },
        streaming: false,
    }
}

/// Registry rejection categories. All variants are safe to expose in diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderAdmissionErrorCode {
    /// No descriptor exists for the integration id.
    UnknownProvider,
    /// The requested exact version is not installed.
    OutdatedProvider,
    /// The descriptor is explicitly revoked.
    RevokedProvider,
    /// The descriptor is incompatible with the host policy.
    IncompatibleProvider,
    /// The manifest's integration configuration is invalid.
    IntegrationInvalid,
    /// A declared route is not owned by the resolved provider policy.
    RouteNotAllowed,
    /// A declared secret is missing or not allowed by provider policy.
    SecretNotAllowed,
    /// A declared scope is outside the provider policy.
    ScopeNotAllowed,
    /// The provider descriptor itself is malformed.
    DescriptorInvalid,
}

impl ProviderAdmissionErrorCode {
    /// Stable machine-readable diagnostic code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownProvider => "provider.unknown",
            Self::OutdatedProvider => "provider.version_outdated",
            Self::RevokedProvider => "provider.revoked",
            Self::IncompatibleProvider => "provider.incompatible",
            Self::IntegrationInvalid => "provider.integration_invalid",
            Self::RouteNotAllowed => "provider.route_not_allowed",
            Self::SecretNotAllowed => "provider.secret_not_allowed",
            Self::ScopeNotAllowed => "provider.scope_not_allowed",
            Self::DescriptorInvalid => "provider.descriptor_invalid",
        }
    }
}

/// Safe provider admission rejection. It never carries config values or secret bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderAdmissionError {
    code: ProviderAdmissionErrorCode,
    detail: &'static str,
    provider: Option<String>,
    version: Option<String>,
    route: Option<String>,
}

impl ProviderAdmissionError {
    fn new(
        code: ProviderAdmissionErrorCode,
        detail: &'static str,
        provider: Option<&str>,
        version: Option<&str>,
        route: Option<&str>,
    ) -> Self {
        Self {
            code,
            detail,
            provider: provider.map(str::to_owned),
            version: version.map(str::to_owned),
            route: route.map(str::to_owned),
        }
    }

    fn descriptor_invalid(provider: &str, version: &str) -> Self {
        Self::new(
            ProviderAdmissionErrorCode::DescriptorInvalid,
            "provider descriptor failed host validation",
            Some(provider),
            Some(version),
            None,
        )
    }

    /// Stable rejection code.
    #[must_use]
    pub const fn code(&self) -> ProviderAdmissionErrorCode {
        self.code
    }

    /// Safe provider id, when resolution reached a descriptor.
    #[must_use]
    pub fn provider(&self) -> Option<&str> {
        self.provider.as_deref()
    }

    /// Safe requested/resolved version, when available.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Safe route id, when the route policy failed.
    #[must_use]
    pub fn route(&self) -> Option<&str> {
        self.route.as_deref()
    }
}

impl std::fmt::Display for ProviderAdmissionErrorCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Provider registry keyed by exact integration id and descriptor version.
#[derive(Clone, Debug)]
pub struct ProviderRegistry {
    descriptors: BTreeMap<String, BTreeMap<String, ProviderDescriptor>>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::maintained()
    }
}

impl ProviderRegistry {
    /// Construct a registry with the maintained OAuth and AI descriptors.
    #[must_use]
    pub fn maintained() -> Self {
        let mut registry = Self {
            descriptors: BTreeMap::new(),
        };
        for descriptor in maintained_oauth_descriptors() {
            registry
                .register(descriptor)
                .expect("maintained OAuth descriptor is valid");
        }
        registry
            .register(ProviderDescriptor::ai())
            .expect("maintained AI descriptor is valid");
        registry
    }

    /// Alias for callers that only need the first-party GitHub catalog.
    #[must_use]
    pub fn github() -> Self {
        let mut registry = Self {
            descriptors: BTreeMap::new(),
        };
        registry
            .register(ProviderDescriptor::github())
            .expect("maintained GitHub descriptor is valid");
        registry
    }

    /// Install one exact descriptor version.
    pub fn register(
        &mut self,
        descriptor: ProviderDescriptor,
    ) -> Result<(), ProviderAdmissionError> {
        descriptor.validate()?;
        let versions = self.descriptors.entry(descriptor.id.clone()).or_default();
        if versions.contains_key(&descriptor.version) {
            return Err(ProviderAdmissionError::new(
                ProviderAdmissionErrorCode::DescriptorInvalid,
                "duplicate provider descriptor version",
                Some(&descriptor.id),
                Some(&descriptor.version),
                None,
            ));
        }
        versions.insert(descriptor.version.clone(), descriptor);
        Ok(())
    }

    /// Change an installed descriptor's state without removing its diagnostic identity.
    pub fn set_state(
        &mut self,
        provider: &str,
        version: &str,
        state: ProviderDescriptorState,
    ) -> Result<(), ProviderAdmissionError> {
        let descriptor = self
            .descriptors
            .get_mut(provider)
            .and_then(|versions| versions.get_mut(version))
            .ok_or_else(|| {
                ProviderAdmissionError::new(
                    ProviderAdmissionErrorCode::OutdatedProvider,
                    "provider descriptor version is not installed",
                    Some(provider),
                    Some(version),
                    None,
                )
            })?;
        descriptor.state = state;
        Ok(())
    }

    /// Revoke an installed descriptor version while retaining its safe diagnostic identity.
    pub fn revoke(&mut self, provider: &str, version: &str) -> Result<(), ProviderAdmissionError> {
        self.set_state(provider, version, ProviderDescriptorState::Revoked)
    }

    /// Resolve an exact descriptor version.
    pub fn resolve(
        &self,
        provider: &str,
        version: &str,
    ) -> Result<&ProviderDescriptor, ProviderAdmissionError> {
        let Some(versions) = self.descriptors.get(provider) else {
            return Err(ProviderAdmissionError::new(
                ProviderAdmissionErrorCode::UnknownProvider,
                "provider integration is not installed",
                Some(provider),
                Some(version),
                None,
            ));
        };
        let Some(descriptor) = versions.get(version) else {
            return Err(ProviderAdmissionError::new(
                ProviderAdmissionErrorCode::OutdatedProvider,
                "provider descriptor version is not installed",
                Some(provider),
                Some(version),
                None,
            ));
        };
        match descriptor.state {
            ProviderDescriptorState::Active => Ok(descriptor),
            ProviderDescriptorState::Revoked => Err(ProviderAdmissionError::new(
                ProviderAdmissionErrorCode::RevokedProvider,
                "provider descriptor has been revoked",
                Some(provider),
                Some(version),
                None,
            )),
            ProviderDescriptorState::Outdated => Err(ProviderAdmissionError::new(
                ProviderAdmissionErrorCode::OutdatedProvider,
                "provider descriptor is no longer current",
                Some(provider),
                Some(version),
                None,
            )),
            ProviderDescriptorState::Incompatible => Err(ProviderAdmissionError::new(
                ProviderAdmissionErrorCode::IncompatibleProvider,
                "provider descriptor is incompatible with this host",
                Some(provider),
                Some(version),
                None,
            )),
        }
    }

    /// Resolve an integration reference using its exact manifest id and version.
    pub fn resolve_integration(
        &self,
        integration: &IntegrationReference,
    ) -> Result<&ProviderDescriptor, ProviderAdmissionError> {
        self.resolve(&integration.id, &integration.version)
    }

    /// Build the admission descriptor of one application-declared `OpenID` Connect issuer.
    ///
    /// A declared issuer has no maintained descriptor, so the registry synthesizes one from
    /// what the application declared. The declared id must be a valid identifier that names no
    /// maintained provider, because a first-party id is never shadowed by a declared issuer, and
    /// the declared version must be the one descriptor version a declared issuer has.
    fn declared_descriptor(
        &self,
        integration: &IntegrationReference,
    ) -> Result<ProviderDescriptor, ProviderAdmissionError> {
        if self.descriptors.contains_key(&integration.id) {
            return Err(integration_invalid(
                integration,
                "declared issuer id is already installed as a provider descriptor",
            ));
        }
        let suffix = integration
            .id
            .strip_prefix(DECLARED_OIDC_ID_PREFIX)
            .unwrap_or_default();
        if suffix.is_empty() || !valid_identifier(suffix) {
            return Err(integration_invalid(
                integration,
                "declared issuer id is not a valid provider identifier",
            ));
        }
        if is_maintained_provider_id(suffix) {
            return Err(integration_invalid(
                integration,
                "declared issuer id names a maintained provider",
            ));
        }
        if integration.version != DECLARED_OIDC_DESCRIPTOR_VERSION {
            return Err(ProviderAdmissionError::new(
                ProviderAdmissionErrorCode::OutdatedProvider,
                "declared issuer descriptor version is not installed",
                Some(&integration.id),
                Some(&integration.version),
                None,
            ));
        }
        let descriptor = ProviderDescriptor::declared_oidc(&integration.id, &integration.version);
        descriptor.validate()?;
        Ok(descriptor)
    }

    /// Resolve and admit all integrations and routes in a parsed manifest.
    pub fn admit(
        &self,
        manifest: &ManifestV1,
        ceilings: &studio_net::limits::BrokerLimits,
    ) -> Result<ProviderAdmissionPlan, ProviderAdmissionError> {
        let mut resolved = Vec::with_capacity(manifest.integrations.len());
        let mut route_groups = Vec::with_capacity(manifest.routes.len());
        let mut integration_ids = BTreeSet::new();

        for integration in &manifest.integrations {
            // A declared issuer is not in the maintained catalog, so the registry synthesizes
            // its descriptor from what the application declared instead of resolving one.
            let synthesized;
            let (descriptor, issuer) = if is_declared_oidc_id(&integration.id) {
                synthesized = self.declared_descriptor(integration)?;
                let issuer = declared_issuer(integration)?;
                (&synthesized, Some(issuer))
            } else {
                (self.resolve(&integration.id, &integration.version)?, None)
            };
            if !integration_ids.insert(integration.id.as_str()) {
                return Err(ProviderAdmissionError::new(
                    ProviderAdmissionErrorCode::IntegrationInvalid,
                    "integration id is duplicated",
                    Some(&integration.id),
                    Some(&integration.version),
                    None,
                ));
            }
            validate_configuration(integration, descriptor)?;
            resolved.push(ResolvedProvider {
                id: descriptor.id.clone(),
                version: descriptor.version.clone(),
                scopes: configured_scopes(integration),
                secrets: configured_secrets(integration, descriptor),
                issuer,
            });
        }

        for route in &manifest.routes {
            if is_declared_oidc_id(&route.id) {
                return Err(ProviderAdmissionError::new(
                    ProviderAdmissionErrorCode::RouteNotAllowed,
                    "a declared issuer grants no route group",
                    None,
                    None,
                    Some(&route.id),
                ));
            }
            let integration = manifest
                .integrations
                .iter()
                .find(|integration| route_belongs_to_provider(route, integration, self))
                .ok_or_else(|| {
                    ProviderAdmissionError::new(
                        ProviderAdmissionErrorCode::RouteNotAllowed,
                        "route has no admitted provider integration",
                        None,
                        None,
                        Some(&route.id),
                    )
                })?;
            let descriptor = self.resolve(&integration.id, &integration.version)?;
            let expected = descriptor
                .routes
                .iter()
                .find(|candidate| candidate.id == route.id)
                .ok_or_else(|| {
                    ProviderAdmissionError::new(
                        ProviderAdmissionErrorCode::RouteNotAllowed,
                        "route is not declared by provider descriptor",
                        Some(&descriptor.id),
                        Some(&descriptor.version),
                        Some(&route.id),
                    )
                })?;
            let expected = materialize_route(expected, integration, descriptor);
            validate_route(route, &expected, descriptor)?;
            route_groups.push(route.compile(ceilings).map_err(|_| {
                ProviderAdmissionError::new(
                    ProviderAdmissionErrorCode::RouteNotAllowed,
                    "route failed broker policy",
                    Some(&descriptor.id),
                    Some(&descriptor.version),
                    Some(&route.id),
                )
            })?);
        }

        validate_secret_declarations(manifest, &resolved, &self.descriptors)?;
        Ok(ProviderAdmissionPlan {
            providers: resolved,
            route_groups,
            secrets: manifest
                .secrets
                .iter()
                .map(|secret| secret.name.clone())
                .collect(),
            capabilities: manifest.capabilities.clone(),
        })
    }

    /// Alias emphasizing that the input is a parsed package manifest.
    pub fn admit_manifest(
        &self,
        manifest: &ManifestV1,
        ceilings: &studio_net::limits::BrokerLimits,
    ) -> Result<ProviderAdmissionPlan, ProviderAdmissionError> {
        self.admit(manifest, ceilings)
    }
}

/// Whether one integration or route id names an application-declared `OpenID` Connect issuer.
///
/// The reserved prefix marks a declared issuer, which the host admits as a public, identity-only
/// client with no route group.
#[must_use]
pub fn is_declared_oidc_id(id: &str) -> bool {
    id.starts_with(DECLARED_OIDC_ID_PREFIX)
}

/// Reject one integration declaration that the host cannot admit.
fn integration_invalid(
    integration: &IntegrationReference,
    detail: &'static str,
) -> ProviderAdmissionError {
    ProviderAdmissionError::new(
        ProviderAdmissionErrorCode::IntegrationInvalid,
        detail,
        Some(&integration.id),
        Some(&integration.version),
        None,
    )
}

/// Read and validate the issuer one declared issuer integration declared.
///
/// The issuer is the host's anchor: it must be an HTTPS URL the host can pin, and a declared
/// issuer carries no secret, so a configuration that names one is refused outright.
fn declared_issuer(integration: &IntegrationReference) -> Result<String, ProviderAdmissionError> {
    let Some(config) = integration.config.as_ref() else {
        return Err(integration_invalid(
            integration,
            "declared issuer configuration is required",
        ));
    };
    let Some(config) = config.as_object() else {
        return Err(integration_invalid(
            integration,
            "declared issuer configuration must be an object",
        ));
    };
    if config.keys().any(|key| names_secret_config_key(key)) {
        return Err(ProviderAdmissionError::new(
            ProviderAdmissionErrorCode::SecretNotAllowed,
            "a declared issuer carries no secret",
            Some(&integration.id),
            Some(&integration.version),
            None,
        ));
    }
    let Some(issuer) = config.get("issuer").and_then(|value| value.as_str()) else {
        return Err(integration_invalid(
            integration,
            "declared issuer configuration is required",
        ));
    };
    if !valid_issuer_url(issuer) {
        return Err(integration_invalid(
            integration,
            "declared issuer configuration is invalid",
        ));
    }
    Ok(issuer.to_owned())
}

/// Whether one configuration key names a protected secret.
fn names_secret_config_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    lowered.contains("secret") || lowered.contains("apikey")
}

/// Whether one declared issuer is an HTTPS URL the host can pin.
///
/// The issuer has to name a host, because the host fetches the issuer's discovery document
/// there, and it has to be an HTTPS URL, because the tokens it issues travel over TLS.
fn valid_issuer_url(value: &str) -> bool {
    const MAX_ISSUER_LENGTH: usize = 2048;
    if value.is_empty()
        || value.len() > MAX_ISSUER_LENGTH
        || value.chars().any(char::is_whitespace)
        || value.chars().any(char::is_control)
    {
        return false;
    }
    let Some(rest) = value.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() || authority.contains('@') {
        return false;
    }
    let (host, port) = if let Some(end) = authority.find(']') {
        let host = &authority[..=end];
        let remainder = &authority[end + 1..];
        let port = match remainder.strip_prefix(':') {
            Some(port) => port,
            None if remainder.is_empty() => "",
            None => return false,
        };
        (host, port)
    } else {
        match authority.rsplit_once(':') {
            Some((host, port)) => (host, port),
            None => (authority, ""),
        }
    };
    let host_is_valid = if host.starts_with('[') {
        host.ends_with(']') && host.len() > 2
    } else {
        !host.is_empty()
            && host.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '-')
            })
    };
    host_is_valid && (port.is_empty() || port.parse::<u16>().is_ok())
}

/// Whether one id names a maintained first-party provider.
fn is_maintained_provider_id(id: &str) -> bool {
    matches!(
        id,
        GITHUB_PROVIDER_ID
            | APPLE_PROVIDER_ID
            | DISCORD_PROVIDER_ID
            | FACEBOOK_PROVIDER_ID
            | GITLAB_PROVIDER_ID
            | GOOGLE_PROVIDER_ID
            | LINKEDIN_PROVIDER_ID
            | MICROSOFT_PROVIDER_ID
            | SPOTIFY_PROVIDER_ID
            | AI_PROVIDER_ID
    )
}

fn route_belongs_to_provider(
    route: &RouteGroupDeclaration,
    integration: &IntegrationReference,
    registry: &ProviderRegistry,
) -> bool {
    // A declared issuer grants no route group, so it never owns one, and a maintained
    // descriptor is never resolved in its place.
    if is_declared_oidc_id(&integration.id) {
        return false;
    }
    registry
        .descriptors
        .get(&integration.id)
        .and_then(|versions| versions.get(&integration.version))
        .is_some_and(|descriptor| {
            descriptor
                .routes
                .iter()
                .any(|candidate| candidate.id == route.id)
        })
}

fn materialize_route(
    route: &ProviderRouteDescriptor,
    integration: &IntegrationReference,
    descriptor: &ProviderDescriptor,
) -> ProviderRouteDescriptor {
    let mut materialized = route.clone();
    if let Some(key) = &descriptor.secret_config_key
        && let Some(name) = integration
            .config
            .as_ref()
            .and_then(|config| config.get(key))
            .and_then(|value| value.as_str())
        && let ProviderCredentialPolicy::NamedSecret { name: expected, .. } =
            &mut materialized.credential
    {
        *expected = name.to_owned();
    }
    materialized
}

fn validate_configuration(
    integration: &IntegrationReference,
    descriptor: &ProviderDescriptor,
) -> Result<(), ProviderAdmissionError> {
    let Some(config) = integration.config.as_ref() else {
        if descriptor.origin_config_key.is_some() || descriptor.secret_config_key.is_some() {
            return Err(ProviderAdmissionError::new(
                ProviderAdmissionErrorCode::IntegrationInvalid,
                "provider integration configuration is required",
                Some(&descriptor.id),
                Some(&descriptor.version),
                None,
            ));
        }
        return Ok(());
    };
    let Some(config) = config.as_object() else {
        return Err(ProviderAdmissionError::new(
            ProviderAdmissionErrorCode::IntegrationInvalid,
            "provider integration configuration must be an object",
            Some(&descriptor.id),
            Some(&descriptor.version),
            None,
        ));
    };
    if let Some(origin_key) = &descriptor.origin_config_key {
        let valid = config
            .get(origin_key)
            .and_then(|value| value.as_str())
            .is_some_and(|origin| studio_net::declaration::Origin::parse(origin).is_ok());
        if !valid {
            return Err(ProviderAdmissionError::new(
                ProviderAdmissionErrorCode::IntegrationInvalid,
                "provider origin configuration is invalid",
                Some(&descriptor.id),
                Some(&descriptor.version),
                None,
            ));
        }
    }
    if let Some(secret_key) = &descriptor.secret_config_key {
        let configured_name = config
            .get(secret_key)
            .and_then(|value| value.as_str())
            .filter(|name| valid_secret_name(name));
        if configured_name.is_none() {
            return Err(ProviderAdmissionError::new(
                ProviderAdmissionErrorCode::SecretNotAllowed,
                "provider secret configuration is invalid",
                Some(&descriptor.id),
                Some(&descriptor.version),
                None,
            ));
        }
        if !descriptor.required_secret_names.is_empty()
            && !descriptor
                .required_secret_names
                .iter()
                .any(|name| Some(name.as_str()) == configured_name)
        {
            return Err(ProviderAdmissionError::new(
                ProviderAdmissionErrorCode::SecretNotAllowed,
                "provider secret is not allowed by descriptor",
                Some(&descriptor.id),
                Some(&descriptor.version),
                None,
            ));
        }
    }
    if let Some(client_id_key) = &descriptor.client_id_config_key {
        let valid = config
            .get(client_id_key)
            .and_then(|value| value.as_str())
            .is_some_and(|client_id| {
                !client_id.is_empty()
                    && client_id.len() <= 256
                    && !client_id.chars().any(char::is_control)
            });
        if !valid {
            return Err(ProviderAdmissionError::new(
                ProviderAdmissionErrorCode::IntegrationInvalid,
                "provider client id configuration is invalid",
                Some(&descriptor.id),
                Some(&descriptor.version),
                None,
            ));
        }
    }
    if let Some(scopes) = config.get("scopes") {
        let Some(scopes) = scopes.as_array() else {
            return Err(ProviderAdmissionError::new(
                ProviderAdmissionErrorCode::ScopeNotAllowed,
                "provider scopes must be an array",
                Some(&descriptor.id),
                Some(&descriptor.version),
                None,
            ));
        };
        let mut seen = BTreeSet::new();
        for scope in scopes {
            let Some(scope) = scope.as_str() else {
                return Err(ProviderAdmissionError::new(
                    ProviderAdmissionErrorCode::ScopeNotAllowed,
                    "provider scope is invalid",
                    Some(&descriptor.id),
                    Some(&descriptor.version),
                    None,
                ));
            };
            if !descriptor.scopes.iter().any(|allowed| allowed == scope) || !seen.insert(scope) {
                return Err(ProviderAdmissionError::new(
                    ProviderAdmissionErrorCode::ScopeNotAllowed,
                    "provider scope is not allowed",
                    Some(&descriptor.id),
                    Some(&descriptor.version),
                    None,
                ));
            }
        }
    }
    Ok(())
}

fn configured_scopes(integration: &IntegrationReference) -> Vec<String> {
    integration
        .config
        .as_ref()
        .and_then(|config| config.get("scopes"))
        .and_then(|scopes| scopes.as_array())
        .map(|scopes| {
            scopes
                .iter()
                .filter_map(|scope| scope.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn configured_secrets(
    integration: &IntegrationReference,
    descriptor: &ProviderDescriptor,
) -> Vec<String> {
    let mut secrets = descriptor.required_secret_names.clone();
    if let Some(key) = &descriptor.secret_config_key
        && let Some(name) = integration
            .config
            .as_ref()
            .and_then(|config| config.get(key))
            .and_then(|value| value.as_str())
        && !secrets.iter().any(|existing| existing == name)
    {
        secrets.push(name.to_owned());
    }
    secrets.sort();
    secrets.dedup();
    secrets
}

fn validate_route(
    actual: &RouteGroupDeclaration,
    expected: &ProviderRouteDescriptor,
    descriptor: &ProviderDescriptor,
) -> Result<(), ProviderAdmissionError> {
    let actual_origins = actual
        .origins
        .iter()
        .map(|origin| studio_net::declaration::Origin::parse(origin))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| route_error(descriptor, &actual.id, "route origin is invalid"))?;
    let expected_origins = expected
        .origins
        .iter()
        .map(|origin| studio_net::declaration::Origin::parse(origin))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| route_error(descriptor, &actual.id, "provider origin policy is invalid"))?;
    if actual_origins != expected_origins
        || actual.methods != expected.methods
        || actual.paths != expected.paths
        || actual.streaming.is_some() != expected.streaming
        || actual.allowed_headers.iter().collect::<BTreeSet<_>>()
            != expected.allowed_headers.iter().collect::<BTreeSet<_>>()
        || !credential_matches(&actual.credential, &expected.credential)
    {
        return Err(route_error(
            descriptor,
            &actual.id,
            "route declaration is incompatible with provider policy",
        ));
    }
    Ok(())
}

fn credential_matches(actual: &CredentialSource, expected: &ProviderCredentialPolicy) -> bool {
    match (actual, expected) {
        (
            CredentialSource::OauthProviderSession { provider: actual },
            ProviderCredentialPolicy::OauthProviderSession { provider: expected },
        ) => actual == expected,
        (
            CredentialSource::NamedSecret {
                name: actual_name,
                header: actual_header,
                prefix: actual_prefix,
            },
            ProviderCredentialPolicy::NamedSecret {
                name: expected_name,
                header: expected_header,
                prefix: expected_prefix,
            },
        ) => {
            actual_name == expected_name
                && actual_header == expected_header
                && actual_prefix == expected_prefix
        }
        (CredentialSource::Public, ProviderCredentialPolicy::Public) => true,
        _ => false,
    }
}

fn route_error(
    descriptor: &ProviderDescriptor,
    route: &str,
    detail: &'static str,
) -> ProviderAdmissionError {
    ProviderAdmissionError::new(
        ProviderAdmissionErrorCode::RouteNotAllowed,
        detail,
        Some(&descriptor.id),
        Some(&descriptor.version),
        Some(route),
    )
}

fn validate_secret_declarations(
    manifest: &ManifestV1,
    providers: &[ResolvedProvider],
    descriptors: &BTreeMap<String, BTreeMap<String, ProviderDescriptor>>,
) -> Result<(), ProviderAdmissionError> {
    let declared = manifest
        .secrets
        .iter()
        .map(|secret| secret.name.as_str())
        .collect::<BTreeSet<_>>();
    let mut required = BTreeSet::new();
    for provider in providers {
        if let Some(descriptor) = descriptors
            .get(&provider.id)
            .and_then(|versions| versions.get(&provider.version))
        {
            required.extend(descriptor.required_secret_names.iter().map(String::as_str));
        }
    }
    for route in &manifest.routes {
        if let CredentialSource::NamedSecret { name, .. } = &route.credential {
            required.insert(name.as_str());
        }
    }
    if required.iter().any(|name| !declared.contains(name)) {
        return Err(ProviderAdmissionError::new(
            ProviderAdmissionErrorCode::SecretNotAllowed,
            "provider-required protected secret is not declared",
            None,
            None,
            None,
        ));
    }
    for provider in providers {
        if let Some(descriptor) = descriptors
            .get(&provider.id)
            .and_then(|versions| versions.get(&provider.version))
        {
            for (name, purpose) in &descriptor.required_secret_purposes {
                if !manifest.secrets.iter().any(|secret| {
                    secret.name.as_str() == name.as_str()
                        && secret.purpose.as_str() == purpose.as_str()
                }) {
                    return Err(ProviderAdmissionError::new(
                        ProviderAdmissionErrorCode::SecretNotAllowed,
                        "provider secret purpose does not match descriptor policy",
                        Some(&descriptor.id),
                        Some(&descriptor.version),
                        None,
                    ));
                }
            }
        }
    }
    Ok(())
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

fn valid_secret_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.starts_with(|character: char| character.is_ascii_lowercase())
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        })
}

/// One provider resolved during admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProvider {
    /// Exact provider id.
    pub id: String,
    /// Exact descriptor version selected by the package.
    pub version: String,
    /// Scopes admitted for this integration.
    pub scopes: Vec<String>,
    /// Protected secret references admitted for this integration.
    pub secrets: Vec<String>,
    /// Issuer URL of an application-declared `OpenID` Connect issuer.
    ///
    /// It is the issuer the application declared, for a declared issuer only. A maintained
    /// first-party provider names no issuer, so it carries none.
    pub issuer: Option<String>,
}

/// Immutable host-consumable result of provider capability admission.
#[derive(Clone, Debug)]
pub struct ProviderAdmissionPlan {
    providers: Vec<ResolvedProvider>,
    route_groups: Vec<studio_net::declaration::CompiledRouteGroup>,
    secrets: Vec<String>,
    capabilities: Vec<Capability>,
}

impl ProviderAdmissionPlan {
    /// Resolved providers in manifest order.
    #[must_use]
    pub fn providers(&self) -> &[ResolvedProvider] {
        &self.providers
    }

    /// Alias for callers that want to emphasize descriptor resolution.
    #[must_use]
    pub fn resolved_providers(&self) -> &[ResolvedProvider] {
        self.providers()
    }

    /// Compiled route groups ready for broker installation.
    #[must_use]
    pub fn route_groups(&self) -> &[studio_net::declaration::CompiledRouteGroup] {
        &self.route_groups
    }

    /// Alias for the broker-ready route groups.
    #[must_use]
    pub fn compiled_routes(&self) -> &[studio_net::declaration::CompiledRouteGroup] {
        self.route_groups()
    }

    /// Protected secret names referenced by the admitted package.
    #[must_use]
    pub fn secrets(&self) -> &[String] {
        &self.secrets
    }

    /// General host capabilities retained with the admission result.
    #[must_use]
    pub fn capabilities(&self) -> &[Capability] {
        &self.capabilities
    }

    /// Install compiled groups without reinterpreting manifest route strings.
    pub fn install_into(&self, broker: &mut RestBroker<'_>) -> Result<(), BrokerError> {
        for group in &self.route_groups {
            broker.install_compiled_group(group.clone())?;
        }
        Ok(())
    }
}

impl std::fmt::Display for ProviderAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code.as_str())?;
        formatter.write_str(": ")?;
        formatter.write_str(self.detail)?;
        if let Some(provider) = &self.provider {
            write!(formatter, " provider={provider}")?;
        }
        if let Some(version) = &self.version {
            write!(formatter, " version={version}")?;
        }
        if let Some(route) = &self.route {
            write!(formatter, " route={route}")?;
        }
        Ok(())
    }
}
