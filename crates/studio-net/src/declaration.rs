//! Signed route-group declaration schema and its compiled runtime form.
//!
//! Route groups enter this crate already admitted by package signing (the manifest and route
//! contributions travel inside the signed `.studio` package; signature verification belongs to
//! [`studio_package`](https://docs.rs/studio-package), upstream of this module). Admission here
//! re-validates everything defensively: shape, identity syntax, origin/path/header grammar,
//! bounded schema keywords, credential references, and host ceilings.
//!
//! Wire conventions mirror `studio-package` manifest-v1: camelCase keys, closed
//! (`deny_unknown_fields`) objects, and JSON-Schema-described request/response bodies using the
//! bounded keyword subset from [`crate::schema`].

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use studio_security::ProtectedSecretKey;

use crate::error::{BrokerError, BrokerErrorCode};
use crate::limits::{BrokerLimits, DeclaredLimits, EffectiveLimits};
use crate::schema::JsonSchema;

/// Closed HTTP method catalog admitted by route-group declarations.
#[derive(Clone, Copy, Debug, Eq, Hash, JsonSchema, PartialEq, Deserialize, Serialize)]
pub enum HttpMethod {
    /// GET.
    #[serde(rename = "GET")]
    Get,
    /// POST.
    #[serde(rename = "POST")]
    Post,
    /// PUT.
    #[serde(rename = "PUT")]
    Put,
    /// PATCH.
    #[serde(rename = "PATCH")]
    Patch,
    /// DELETE.
    #[serde(rename = "DELETE")]
    Delete,
    /// HEAD.
    #[serde(rename = "HEAD")]
    Head,
}

impl HttpMethod {
    /// Canonical uppercase wire name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
        }
    }
}

/// Normalized request origin: scheme, lowercase host, optional explicit port.
///
/// Origins compare after normalization, so `https://api.example.com`,
/// `HTTPS://API.EXAMPLE.COM`, and `https://api.example.com:443` denote one origin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Origin {
    scheme: &'static str,
    host: String,
    port: Option<u16>,
}

impl Origin {
    /// Parse and normalize one declared or requested origin string.
    ///
    /// # Errors
    ///
    /// Returns [`BrokerErrorCode::DeclarationInvalid`] for malformed schemes, hosts, or ports.
    pub fn parse(raw: &str) -> Result<Self, BrokerError> {
        let invalid =
            || BrokerError::with_detail(BrokerErrorCode::DeclarationInvalid, "invalid origin");
        let Some((scheme, remainder)) = raw.split_once("://") else {
            return Err(invalid());
        };
        let scheme = match scheme.to_ascii_lowercase().as_str() {
            "http" => "http",
            "https" => "https",
            _ => return Err(invalid()),
        };
        let authority = remainder.trim_end_matches('/');
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) => {
                let port = port.parse::<u16>().map_err(|_| invalid())?;
                if port == 0 {
                    return Err(invalid());
                }
                (host, Some(port))
            }
            None => (authority, None),
        };
        if !valid_host(host) {
            return Err(invalid());
        }
        Ok(Self {
            scheme,
            host: host.to_ascii_lowercase(),
            port,
        })
    }

    /// Whether this origin equals another parsed origin.
    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        self == other
    }

    /// Redacted-safe rendering for diagnostics: scheme and host only, never credentials.
    #[must_use]
    pub fn display(&self) -> String {
        match self.port {
            Some(port) => format!("{}://{self.host}:{port}", self.scheme),
            None => format!("{}://{self.host}", self.scheme),
        }
    }
}

fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b':'))
}

/// One compiled declared path pattern.
///
/// Segments are literal text, `{parameter}` slots matching exactly one non-empty segment, or a
/// final `**` segment absorbing any remainder (including none). Patterns always start with `/`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathPattern {
    segments: Vec<PathSegment>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PathSegment {
    Literal(String),
    Slot,
    Remainder,
}

impl PathPattern {
    /// Parse one declared pattern.
    ///
    /// # Errors
    ///
    /// Returns [`BrokerErrorCode::DeclarationInvalid`] for malformed segments or misplaced
    /// remainder wildcards.
    pub fn parse(pattern: &str) -> Result<Self, BrokerError> {
        let invalid =
            || BrokerError::with_detail(BrokerErrorCode::DeclarationInvalid, "invalid path pattern");
        if pattern.is_empty() || !pattern.starts_with('/') || pattern.contains("//") {
            return Err(invalid());
        }
        let mut segments = Vec::new();
        for raw in pattern.trim_start_matches('/').split('/') {
            if raw.is_empty() {
                continue;
            }
            if raw == "**" {
                if segments.last() == Some(&PathSegment::Remainder) {
                    return Err(invalid());
                }
                segments.push(PathSegment::Remainder);
            } else if raw.starts_with('{') && raw.ends_with('}') {
                segments.push(PathSegment::Slot);
            } else if valid_literal_segment(raw) {
                segments.push(PathSegment::Literal(raw.to_owned()));
            } else {
                return Err(invalid());
            }
        }
        Ok(Self { segments })
    }

    /// Whether a request path (no query string) is covered by this pattern.
    #[must_use]
    pub fn matches(&self, path: &str) -> bool {
        let request: Vec<&str> = path
            .trim_start_matches('/')
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        matches_from(&self.segments, &request)
    }
}

fn valid_literal_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.len() <= 128
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~'))
}

fn matches_from(pattern: &[PathSegment], request: &[&str]) -> bool {
    let Some(first) = pattern.first() else {
        return request.is_empty();
    };
    let rest = &pattern[1..];
    match first {
        PathSegment::Remainder => true,
        PathSegment::Slot => {
            !request.first().copied().unwrap_or_default().is_empty()
                && matches_from(rest, &request[1..])
        }
        PathSegment::Literal(literal) => {
            request.first().is_some_and(|head| head == literal)
                && matches_from(rest, &request[1..])
        }
    }
}

/// Declared credential source for one route group.
///
/// `Public` sends nothing. `oauthProviderSession` resolves through the typed host seam filled by
/// the OAuth provider-plugin milestone (ticket 21); until that lands the broker answers with
/// [`BrokerErrorCode::OauthSessionUnavailable`] rather than falling back to generic network
/// behavior. `namedSecret` names a package-declared protected value injected strictly at send
/// time through [`studio_security::BrokerSecretInjector`].
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Deserialize, Serialize)]
#[serde(tag = "source", rename_all_fields = "camelCase")]
pub enum CredentialSource {
    /// No credential material is attached.
    #[serde(rename = "public")]
    Public,
    /// An OAuth provider-plugin session supplies the credential at send time.
    #[serde(rename = "oauthProviderSession")]
    OauthProviderSession {
        /// Stable provider identifier from the enabled integration plugin.
        provider: String,
    },
    /// A package-declared protected secret is injected into one request header.
    #[serde(rename = "namedSecret")]
    NamedSecret {
        /// Package-declared protected secret name.
        name: String,
        /// Lowercase HTTP header receiving the injected value.
        header: String,
        /// Optional static prefix such as `Bearer ` applied before the secret bytes.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prefix: Option<String>,
    },
}

/// Declared streaming behavior for server-sent-event routes.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StreamingDeclaration {
    /// Bounded schema every validated chunk event satisfies before guest visibility.
    pub chunk_schema: Value,
    /// Maximum host-owned reconnect attempts after transport failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reconnects: Option<u32>,
    /// Base delay in milliseconds for the host-owned exponential backoff schedule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_base_delay_ms: Option<u64>,
}

/// Signed route-group declaration wire form.
///
/// A group admits requests whose normalized origin, method, and path all fall inside its lists,
/// whose headers stay within `allowedHeaders`, whose body satisfies `requestSchema`, and whose
/// responses satisfy `responseSchema` before any guest visibility.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteGroupDeclaration {
    /// Stable group identifier referenced by SDK helpers and diagnostics.
    pub id: String,
    /// Exact normalized origins covered by this group.
    pub origins: Vec<String>,
    /// Methods admitted inside this group.
    pub methods: Vec<HttpMethod>,
    /// Path patterns covered by this group.
    pub paths: Vec<String>,
    /// Additional request header names the guest may set (lowercase HTTP tokens).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_headers: Vec<String>,
    /// Credential resolution strategy for the whole group.
    pub credential: CredentialSource,
    /// Bounded request-body schema; absent means requests carry no body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_schema: Option<Value>,
    /// Bounded response-body schema enforced before guest visibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<Value>,
    /// Declared server-sent-event streaming mode; conflicts with `responseSchema`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streaming: Option<StreamingDeclaration>,
    /// Explicit narrower bounds; absent fields inherit generous host defaults.
    #[serde(default)]
    pub limits: DeclaredLimits,
}

/// Host-owned reconnect/retry policy resolved from a streaming declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    /// Maximum reconnect attempts after transport failure within one stream lifetime.
    pub max_reconnects: u32,
    /// Base backoff delay; doubles per attempt up to the idle timeout.
    pub base_delay: std::time::Duration,
}

/// Validated, pre-parsed route group executed by the broker.
///
/// Compilation happens once at broker construction so per-request admission performs no string
/// grammar work beyond matching.
#[derive(Debug)]
pub struct CompiledRouteGroup {
    id: String,
    origins: Vec<Origin>,
    methods: Vec<HttpMethod>,
    paths: Vec<PathPattern>,
    allowed_headers: BTreeSet<String>,
    credential: CompiledCredential,
    request_schema: Option<JsonSchema>,
    response_schema: Option<JsonSchema>,
    chunk_schema: Option<JsonSchema>,
    retry_policy: RetryPolicy,
    limits: EffectiveLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CompiledCredential {
    Public,
    OauthProviderSession { provider: String },
    NamedSecret { key: ProtectedSecretKey, header: String, prefix: Option<String> },
}

impl CompiledRouteGroup {
    /// Group identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Parsed origins covered by the group.
    #[must_use]
    pub fn origins(&self) -> &[Origin] {
        &self.origins
    }

    /// Methods admitted inside the group.
    #[must_use]
    pub fn methods(&self) -> &[HttpMethod] {
        &self.methods
    }

    /// Path patterns covered by the group.
    #[must_use]
    pub fn paths(&self) -> &[PathPattern] {
        &self.paths
    }

    /// Guest-settable request headers (lowercase).
    #[must_use]
    pub fn allowed_headers(&self) -> &BTreeSet<String> {
        &self.allowed_headers
    }

    /// Whether the group declares server-sent-event streaming.
    #[must_use]
    pub fn is_streaming(&self) -> bool {
        self.chunk_schema.is_some()
    }

    /// Chunk-event schema for streaming groups.
    #[must_use]
    pub fn chunk_schema(&self) -> Option<&JsonSchema> {
        self.chunk_schema.as_ref()
    }

    /// Host-owned reconnect/retry policy for streaming groups.
    #[must_use]
    pub const fn retry_policy(&self) -> RetryPolicy {
        self.retry_policy
    }

    /// Resolved effective limits.
    #[must_use]
    pub const fn limits(&self) -> &EffectiveLimits {
        &self.limits
    }

    /// Request-body schema, if declared.
    #[must_use]
    pub fn request_schema(&self) -> Option<&JsonSchema> {
        self.request_schema.as_ref()
    }

    /// Response-body schema, if declared.
    #[must_use]
    pub fn response_schema(&self) -> Option<&JsonSchema> {
        self.response_schema.as_ref()
    }

    /// Credential source kind for diagnostics and tests.
    #[must_use]
    pub fn credential_kind(&self) -> &'static str {
        match &self.credential {
            CompiledCredential::Public => "public",
            CompiledCredential::OauthProviderSession { .. } => "oauth-provider-session",
            CompiledCredential::NamedSecret { .. } => "named-secret",
        }
    }

    /// OAuth provider identifier when the group uses a provider session.
    #[must_use]
    pub fn oauth_provider(&self) -> Option<&str> {
        match &self.credential {
            CompiledCredential::OauthProviderSession { provider } => Some(provider),
            _ => None,
        }
    }

    /// Named-secret reference when the group uses protected injection.
    #[must_use]
    pub fn named_secret_key(&self) -> Option<&ProtectedSecretKey> {
        match &self.credential {
            CompiledCredential::NamedSecret { key, .. } => Some(key),
            _ => None,
        }
    }

    /// Credential target header (lowercase) for named-secret groups.
    #[must_use]
    pub fn named_secret_header(&self) -> Option<&str> {
        match &self.credential {
            CompiledCredential::NamedSecret { header, .. } => Some(header),
            _ => None,
        }
    }

    /// Static credential prefix for named-secret groups.
    #[must_use]
    pub fn named_secret_prefix(&self) -> Option<&str> {
        match &self.credential {
            CompiledCredential::NamedSecret { prefix, .. } => prefix.as_deref(),
            _ => None,
        }
    }
}

impl RouteGroupDeclaration {
    /// Defensively validate and compile one signed declaration against host ceilings.
    ///
    /// # Errors
    ///
    /// Returns [`BrokerErrorCode::DeclarationInvalid`] for any malformed identity, origin,
    /// path, header, credential, schema, streaming, or limit input.
    pub fn compile(
        &self,
        ceilings: &BrokerLimits,
    ) -> Result<CompiledRouteGroup, BrokerError> {
        let invalid =
            |detail: &'static str| BrokerError::with_detail(BrokerErrorCode::DeclarationInvalid, detail.to_owned());
        if !valid_group_id(&self.id) {
            return Err(invalid("invalid route group id"));
        }
        if self.origins.is_empty() || self.methods.is_empty() || self.paths.is_empty() {
            return Err(invalid("route group requires origins, methods, and paths"));
        }
        let mut origins = Vec::with_capacity(self.origins.len());
        for raw in &self.origins {
            let origin = Origin::parse(raw)?;
            if origins.iter().any(|seen: &Origin| seen.matches(&origin)) {
                return Err(invalid("duplicate declared origin"));
            }
            origins.push(origin);
        }
        let mut methods = Vec::with_capacity(self.methods.len());
        for method in &self.methods {
            if methods.contains(method) {
                return Err(invalid("duplicate declared method"));
            }
            methods.push(*method);
        }
        let mut paths = Vec::with_capacity(self.paths.len());
        for pattern in &self.paths {
            let parsed = PathPattern::parse(pattern)?;
            if paths.contains(&parsed) {
                return Err(invalid("duplicate declared path pattern"));
            }
            paths.push(parsed);
        }
        let mut allowed_headers = BTreeSet::new();
        for header in &self.allowed_headers {
            if !valid_header_name(header) || !allowed_headers.insert(header.to_ascii_lowercase()) {
                return Err(invalid("invalid or duplicate allowed header"));
            }
        }
        let credential = match &self.credential {
            CredentialSource::Public => CompiledCredential::Public,
            CredentialSource::OauthProviderSession { provider } => {
                if provider.is_empty() || provider.len() > 128 {
                    return Err(invalid("invalid oauth provider id"));
                }
                CompiledCredential::OauthProviderSession {
                    provider: provider.clone(),
                }
            }
            CredentialSource::NamedSecret { name, header, prefix } => {
                let Ok(key) = ProtectedSecretKey::new(name.clone(), format!(
                    "REST broker credential for route group {}",
                    self.id
                )) else {
                    return Err(invalid("invalid named secret reference"));
                };
                if !valid_header_name(header) {
                    return Err(invalid("invalid credential header"));
                }
                if let Some(prefix) = prefix
                    && (prefix.len() > 32
                        || prefix.chars().any(char::is_control)
                        || prefix.bytes().any(|byte| byte < 0x20))
                {
                    return Err(invalid("invalid credential prefix"));
                }
                CompiledCredential::NamedSecret {
                    key,
                    header: header.to_ascii_lowercase(),
                    prefix: prefix.clone(),
                }
            }
        };
        let request_schema = match &self.request_schema {
            Some(value) => Some(JsonSchema::new(value.clone()).map_err(map_schema_error)?),
            None => None,
        };
        let response_schema = match (&self.response_schema, &self.streaming) {
            (Some(_), Some(_)) => {
                return Err(invalid("streaming groups declare a chunk schema, not a response schema"));
            }
            (Some(value), None) => Some(JsonSchema::new(value.clone()).map_err(map_schema_error)?),
            (None, _) => None,
        };
        let (chunk_schema, retry_policy) = match &self.streaming {
            Some(streaming) => {
                if methods.as_slice() != [HttpMethod::Get] {
                    return Err(invalid("streaming routes must declare GET only"));
                }
                (
                    Some(JsonSchema::new(streaming.chunk_schema.clone()).map_err(map_schema_error)?),
                    RetryPolicy {
                        max_reconnects: streaming.reconnects.unwrap_or(3),
                        base_delay: std::time::Duration::from_millis(
                            streaming.retry_base_delay_ms.unwrap_or(500).max(50),
                        ),
                    },
                )
            }
            None => (
                None,
                RetryPolicy {
                    max_reconnects: 0,
                    base_delay: std::time::Duration::from_millis(500),
                },
            ),
        };
        let limits = EffectiveLimits::resolve(&self.limits, ceilings)?;
        // The credential header is implicitly allowed for named-secret groups.
        if let CompiledCredential::NamedSecret { header, .. } = &credential {
            allowed_headers.insert(header.clone());
        }
        Ok(CompiledRouteGroup {
            id: self.id.clone(),
            origins,
            methods,
            paths,
            allowed_headers,
            credential,
            request_schema,
            response_schema,
            chunk_schema,
            retry_policy,
            limits,
        })
    }
}

fn map_schema_error(error: crate::schema::SchemaError) -> BrokerError {
    BrokerError::with_detail(
        BrokerErrorCode::DeclarationInvalid,
        format!("bounded schema rejected: {error}"),
    )
}

fn valid_group_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.starts_with(|character: char| character.is_ascii_lowercase())
        && id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        })
}

/// RFC 7230 token grammar for header names.
#[must_use]
pub fn valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.'
                        | b'^' | b'_' | b'`' | b'|' | b'~'
                )
        })
}
