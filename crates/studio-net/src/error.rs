//! Stable, value-free broker failure family safe for guest surfaces and diagnostics.
//!
//! Every code is closed and carries no URL, header, credential, or upstream provider context.
//! Optional detail text is sanitized through the redaction scrubber by the broker before an
//! error is constructed, so diagnostics never echo injected credentials or key-shaped tokens.

use std::fmt;

/// Closed stable broker denial/failure codes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BrokerErrorCode {
    /// A signed declaration was malformed or exceeded a host ceiling.
    DeclarationInvalid,
    /// The request origin is not covered by any route-group declaration.
    OriginNotDeclared,
    /// No declared path pattern covers the request path in the matched origin group.
    PathNotDeclared,
    /// The matched group exists but does not declare the request method.
    MethodNotAllowed,
    /// The route is declared streaming and must be consumed through [`crate::guest`].
    RouteIsStreaming,
    /// The route is not declared streaming but was opened as a stream.
    RouteNotStreaming,
    /// The request carried a header the matched group does not declare.
    HeaderNotAllowed,
    /// The request body exceeded the effective byte bound.
    RequestTooLarge,
    /// The request body failed the declared request schema.
    RequestSchemaInvalid,
    /// The route group's rate allowance was exhausted inside its window.
    RateLimited,
    /// The transport did not complete inside the effective timeout.
    Timeout,
    /// The transport reported a connection-level failure.
    TransportFailure,
    /// A named secret was missing, revoked, or otherwise unavailable.
    CredentialUnavailable,
    /// Host-side credential injection was rejected while building the send.
    InjectionRejected,
    /// The OAuth provider-plugin session seam has not been fulfilled yet.
    OauthSessionUnavailable,
    /// The upstream answered outside the declared success window.
    UpstreamRejected,
    /// The response body exceeded the effective byte bound.
    ResponseTooLarge,
    /// The response body was not parseable as the declared media type.
    ResponseMalformed,
    /// The parsed response failed the declared response schema before guest visibility.
    ResponseSchemaMismatch,
    /// The validated response contained registered credential material and was discarded
    /// before guest visibility.
    SensitiveContentRejected,
    /// A stream exceeded one of its declared bounds.
    StreamExceeded,
    /// The guest cancelled an in-flight operation.
    Cancelled,
    /// An inbound webhook declaration was malformed or exceeded a host ceiling.
    WebhookDeclarationInvalid,
    /// No inbound webhook declaration covers the requested endpoint.
    WebhookEndpointNotDeclared,
    /// The inbound webhook method is not declared for the endpoint.
    WebhookMethodNotAllowed,
    /// The inbound webhook source proof was missing or invalid.
    WebhookSourceInvalid,
    /// The inbound webhook payload exceeded its declared byte bound.
    WebhookPayloadTooLarge,
    /// The inbound webhook payload was not valid JSON.
    WebhookPayloadMalformed,
    /// The inbound webhook payload failed its declared schema.
    WebhookSchemaMismatch,
    /// The inbound webhook endpoint rate allowance was exhausted.
    WebhookRateLimited,
    /// The inbound webhook declaration is outside its active lifetime.
    WebhookExpired,
    /// The host listener failed without exposing provider details.
    WebhookListenerFailure,
}

impl BrokerErrorCode {
    /// Stable machine-readable code string for action results and diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeclarationInvalid => "net.declaration.invalid",
            Self::OriginNotDeclared => "net.route.origin_not_declared",
            Self::PathNotDeclared => "net.route.path_not_declared",
            Self::MethodNotAllowed => "net.route.method_not_allowed",
            Self::RouteIsStreaming => "net.route.is_streaming",
            Self::RouteNotStreaming => "net.route.not_streaming",
            Self::HeaderNotAllowed => "net.route.header_not_allowed",
            Self::RequestTooLarge => "net.request.too_large",
            Self::RequestSchemaInvalid => "net.request.schema_invalid",
            Self::RateLimited => "net.request.rate_limited",
            Self::Timeout => "net.transport.timeout",
            Self::TransportFailure => "net.transport.failure",
            Self::CredentialUnavailable => "net.credential.unavailable",
            Self::InjectionRejected => "net.credential.injection_rejected",
            Self::OauthSessionUnavailable => "net.credential.oauth_session_unavailable",
            Self::UpstreamRejected => "net.response.upstream_rejected",
            Self::ResponseTooLarge => "net.response.too_large",
            Self::ResponseMalformed => "net.response.malformed",
            Self::ResponseSchemaMismatch => "net.response.schema_mismatch",
            Self::SensitiveContentRejected => "net.response.sensitive_rejected",
            Self::StreamExceeded => "net.stream.exceeded",
            Self::Cancelled => "net.operation.cancelled",
            Self::WebhookDeclarationInvalid => "webhook.declaration.invalid",
            Self::WebhookEndpointNotDeclared => "webhook.endpoint.not_declared",
            Self::WebhookMethodNotAllowed => "webhook.endpoint.method_not_allowed",
            Self::WebhookSourceInvalid => "webhook.source.invalid",
            Self::WebhookPayloadTooLarge => "webhook.payload.too_large",
            Self::WebhookPayloadMalformed => "webhook.payload.malformed",
            Self::WebhookSchemaMismatch => "webhook.payload.schema_mismatch",
            Self::WebhookRateLimited => "webhook.request.rate_limited",
            Self::WebhookExpired => "webhook.declaration.expired",
            Self::WebhookListenerFailure => "webhook.listener.failure",
        }
    }
}

impl fmt::Display for BrokerErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Safe broker error carrying a stable code and pre-sanitized optional detail.
///
/// Detail text must be passed through [`studio_security::SensitiveValueFilter`] by the caller
/// that observed raw context; this type intentionally offers no way to attach unsanitized data
/// after construction from untrusted sources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrokerError {
    code: BrokerErrorCode,
    detail: Option<String>,
}

impl BrokerError {
    /// Construct a bare error with only its stable code.
    #[must_use]
    pub const fn new(code: BrokerErrorCode) -> Self {
        Self { code, detail: None }
    }

    /// Construct an error whose detail was already sanitized by the caller.
    #[must_use]
    pub const fn with_detail(code: BrokerErrorCode, detail: String) -> Self {
        Self {
            code,
            detail: Some(detail),
        }
    }

    /// Stable code suitable for guest action results.
    #[must_use]
    pub const fn code(self) -> BrokerErrorCode {
        self.code
    }

    /// Pre-sanitized diagnostic detail, if any.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }
}

impl fmt::Display for BrokerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.code)?;
        if let Some(detail) = &self.detail {
            write!(formatter, ": {detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for BrokerError {}
