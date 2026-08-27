//! Provider capability admission for signed application manifests.
//!
//! Provider descriptors are host-maintained policy, not package-supplied code. Admission resolves
//! an exact integration id/version, checks its state and compatibility, then compiles every
//! declared route once. The resulting plan is safe to hand to the broker: it contains no raw
//! manifest strings that need to be interpreted again at request time.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use semver::Version;
use studio_net::broker::RestBroker;
use studio_net::BrokerError;
use studio_net::declaration::{CredentialSource, HttpMethod, RouteGroupDeclaration};

use crate::{Capability, IntegrationReference, ManifestV1};

/// Current provider descriptor schema understood by the host.
pub const PROVIDER_DESCRIPTOR_SCHEMA_VERSION: u16 = 1;
/// Maintained GitHub integration id.
pub const GITHUB_PROVIDER_ID: &str = "github";
/// Maintained GitHub integration descriptor version.
pub const GITHUB_PROVIDER_VERSION: &str = "1.0.0";
/// Maintained provider-neutral AI integration id.
pub const AI_PROVIDER_ID: &str = "ai";
/// Maintained provider-neutral AI integration descriptor version.
pub const AI_PROVIDER_VERSION: &str = "1.0.0";

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
        descriptor.scopes = ["read:user", "user:email", "repo"]
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

    fn validate(&self) -> Result<(), ProviderAdmissionError> {
        if self.schema_version != PROVIDER_DESCRIPTOR_SCHEMA_VERSION
            || !valid_identifier(&self.id)
            || Version::parse(&self.version).is_err()
            || self.routes.is_empty()
            || self.routes.iter().any(|route| {
                route.id.is_empty()
                    || route.origins.is_empty()
                    || route.methods.is_empty()
                    || route.paths.is_empty()
            })
        {
            return Err(ProviderAdmissionError::descriptor_invalid(&self.id, &self.version));
        }
        Ok(())
    }
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
    /// Construct a registry with maintained GitHub and AI descriptors.
    #[must_use]
    pub fn maintained() -> Self {
        let mut registry = Self {
            descriptors: BTreeMap::new(),
        };
        registry
            .register(ProviderDescriptor::github())
            .expect("maintained GitHub descriptor is valid");
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
    pub fn register(&mut self, descriptor: ProviderDescriptor) -> Result<(), ProviderAdmissionError> {
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
            let descriptor = self.resolve(&integration.id, &integration.version)?;
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
            });
        }

        for route in &manifest.routes {
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
            route_groups.push(
                route
                    .compile(ceilings)
                    .map_err(|_| ProviderAdmissionError::new(
                        ProviderAdmissionErrorCode::RouteNotAllowed,
                        "route failed broker policy",
                        Some(&descriptor.id),
                        Some(&descriptor.version),
                        Some(&route.id),
                    ))?,
            );
        }

        validate_secret_declarations(manifest, &resolved, &self.descriptors)?;
        Ok(ProviderAdmissionPlan {
            providers: resolved,
            route_groups,
            secrets: manifest.secrets.iter().map(|secret| secret.name.clone()).collect(),
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

fn route_belongs_to_provider(
    route: &RouteGroupDeclaration,
    integration: &IntegrationReference,
    registry: &ProviderRegistry,
) -> bool {
    registry
        .descriptors
        .get(&integration.id)
        .and_then(|versions| versions.get(&integration.version))
        .is_some_and(|descriptor| descriptor.routes.iter().any(|candidate| candidate.id == route.id))
}

fn materialize_route(
    route: &ProviderRouteDescriptor,
    integration: &IntegrationReference,
    descriptor: &ProviderDescriptor,
) -> ProviderRouteDescriptor {
    let mut materialized = route.clone();
    if let Some(key) = &descriptor.secret_config_key
        && let Some(name) = integration.config.as_ref().and_then(|config| config.get(key)).and_then(|value| value.as_str())
        && let ProviderCredentialPolicy::NamedSecret { name: expected, .. } = &mut materialized.credential
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
        ) => actual_name == expected_name && actual_header == expected_header && actual_prefix == expected_prefix,
        (CredentialSource::Public, ProviderCredentialPolicy::Public) => true,
        _ => false,
    }
}

fn route_error(descriptor: &ProviderDescriptor, route: &str, detail: &'static str) -> ProviderAdmissionError {
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
                if !manifest
                    .secrets
                    .iter()
                    .any(|secret| {
                        secret.name.as_str() == name.as_str()
                            && secret.purpose.as_str() == purpose.as_str()
                    })
                {
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
    pub fn install_into<'store>(
        &self,
        broker: &mut RestBroker<'store>,
    ) -> Result<(), BrokerError> {
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
