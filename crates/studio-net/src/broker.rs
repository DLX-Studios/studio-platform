//! Host-owned REST broker execution pipeline.
//!
//! Order of operations for every request, all host-side:
//!
//! 1. Admission against compiled signed route groups (origin, method, path, headers).
//! 2. Rate accounting per group inside its sliding window.
//! 3. Request-body bounding and declared-schema validation.
//! 4. Credential resolution strictly at send time (public / provider session / named secret).
//! 5. Transport exchange under the effective timeout and response bound.
//! 6. Response validation against the declared schema before any guest visibility.
//!
//! Every diagnostic detail passes through [`studio_security::SensitiveValueFilter`], which also
//! accumulates injected credential patterns so later logs can never echo them.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use studio_security::SensitiveValueFilter;

use crate::admission;
use crate::credential::{self, NamedSecretInjector, OAuthSessionResolver};
use crate::declaration::{CompiledRouteGroup, RouteGroupDeclaration};
use crate::error::{BrokerError, BrokerErrorCode};
use crate::guest::{BrokerRequest, GuestRestApi, StreamHandle, TypedResponse};
use crate::limits::BrokerLimits;
use crate::streaming;

/// One logical REST broker owned by the Runtime host.
///
/// The broker borrows the protected secret store through its injection handle, so the handle is
/// bound after construction behind interior mutability; credential resolution reads it under a
/// short lock at send time.
pub struct RestBroker<'store> {
    groups: Vec<CompiledRouteGroup>,
    transport: Arc<dyn crate::transport::HttpTransport>,
    injector: Mutex<Option<Arc<dyn NamedSecretInjector + 'store>>>,
    oauth: Mutex<Option<Arc<dyn OAuthSessionResolver>>>,
    filter: Arc<Mutex<SensitiveValueFilter>>,
    rate_windows: Mutex<HashMap<String, VecDeque<Instant>>>,
    ceilings: BrokerLimits,
}

impl std::fmt::Debug for RestBroker<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RestBroker")
            .field("groups", &self.groups.len())
            .finish_non_exhaustive()
    }
}

impl<'store> RestBroker<'store> {
    /// Create a broker over one transport and explicit host ceilings.
    #[must_use]
    pub fn new(
        transport: Arc<dyn crate::transport::HttpTransport>,
        ceilings: BrokerLimits,
    ) -> Self {
        Self {
            groups: Vec::new(),
            transport,
            injector: Mutex::new(None),
            oauth: Mutex::new(None),
            filter: Arc::new(Mutex::new(SensitiveValueFilter::new())),
            rate_windows: Mutex::new(HashMap::new()),
            ceilings,
        }
    }

    /// Validate, compile, and install one signed route-group declaration.
    ///
    /// # Errors
    ///
    /// Returns [`BrokerErrorCode::DeclarationInvalid`] for any malformed input or duplicate
    /// group identifier.
    pub fn declare_group(
        &mut self,
        declaration: &RouteGroupDeclaration,
    ) -> Result<(), BrokerError> {
        let compiled = declaration.compile(&self.ceilings)?;
        if self.groups.iter().any(|group| group.id() == compiled.id()) {
            return Err(BrokerError::with_detail(
                BrokerErrorCode::DeclarationInvalid,
                "duplicate route group id".to_owned(),
            ));
        }
        self.groups.push(compiled);
        Ok(())
    }

    /// Bind the protected-secret send-time injector for named-secret groups.
    pub fn set_named_secret_injector(&self, injector: Arc<dyn NamedSecretInjector + 'store>) {
        *self
            .injector
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(injector);
    }

    /// Bind the OAuth provider-plugin session resolver (ticket 21 seam).
    pub fn set_oauth_resolver(&self, resolver: Arc<dyn OAuthSessionResolver>) {
        *self
            .oauth
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(resolver);
    }

    /// Expose the restricted guest facade over this broker.
    #[must_use]
    pub fn guest_api(self: &Arc<Self>) -> GuestRestApi<'store> {
        GuestRestApi::new(Arc::clone(self))
    }

    /// Execute one request through the full pipeline.
    ///
    /// # Errors
    ///
    /// Returns stable [`BrokerError`] codes; responses failing declared validation never reach
    /// the caller.
    pub fn execute(&self, request: BrokerRequest) -> Result<TypedResponse, BrokerError> {
        let group = self.admit_request(&request)?;
        if group.is_streaming() {
            return Err(BrokerError::new(BrokerErrorCode::RouteIsStreaming));
        }
        let limits = *group.limits();
        self.check_rate(
            group.id(),
            limits.max_requests_per_window,
            limits.rate_window,
        )?;
        let body_bytes = self.prepare_body(
            request.body.as_ref(),
            group.request_schema(),
            limits.max_request_bytes,
        )?;
        let mut headers = guest_headers(&request);
        if !body_bytes.is_empty() {
            headers.push((
                String::from("content-type"),
                String::from("application/json"),
            ));
        }
        credential::resolve(
            group,
            self.injector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref(),
            self.oauth
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref(),
            &mut headers,
            &self.filter,
        )?;
        let outgoing = crate::transport::OutgoingRequest {
            method: request.method,
            url: build_url(
                request.origin.trim_end_matches('/'),
                &request.path,
                request.query.as_deref(),
            ),
            headers,
            body: (!body_bytes.is_empty()).then_some(body_bytes),
            timeout: limits.timeout,
        };
        let response = self
            .transport
            .execute(outgoing)
            .map_err(map_transport_error)?;
        if !(200..=299).contains(&response.status) {
            return Err(BrokerError::with_detail(
                BrokerErrorCode::UpstreamRejected,
                self.sanitize(&format!("upstream status {}", response.status)),
            ));
        }
        if response.body.len() > limits.max_response_bytes {
            return Err(BrokerError::new(BrokerErrorCode::ResponseTooLarge));
        }
        let body = self.validate_response_body(&response.body, group.response_schema())?;
        Ok(TypedResponse::new(response.status, body))
    }

    /// Open one declared server-sent-event stream, optionally carrying a bounded request body.
    ///
    /// # Errors
    ///
    /// Returns stable admission codes before any connection opens.
    pub fn open_stream(&self, request: BrokerRequest) -> Result<StreamHandle, BrokerError> {
        let group = self.admit_request(&request)?;
        if !group.is_streaming() {
            return Err(BrokerError::new(BrokerErrorCode::RouteNotStreaming));
        }
        let limits = *group.limits();
        self.check_rate(
            group.id(),
            limits.max_requests_per_window,
            limits.rate_window,
        )?;
        let body_bytes = self.prepare_body(
            request.body.as_ref(),
            group.request_schema(),
            limits.max_request_bytes,
        )?;
        let mut headers = guest_headers(&request);
        headers.push((String::from("accept"), String::from("text/event-stream")));
        if !body_bytes.is_empty() {
            headers.push((String::from("content-type"), String::from("application/json")));
        }
        credential::resolve(
            group,
            self.injector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref(),
            self.oauth
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref(),
            &mut headers,
            &self.filter,
        )?;
        let outgoing = crate::transport::OutgoingRequest {
            method: request.method,
            url: build_url(
                request.origin.trim_end_matches('/'),
                &request.path,
                request.query.as_deref(),
            ),
            headers,
            body: (!body_bytes.is_empty()).then_some(body_bytes),
            timeout: limits.stream_idle_timeout,
        };
        Ok(streaming::spawn_stream(
            Arc::clone(&self.transport),
            group.clone(),
            outgoing,
            Arc::clone(&self.filter),
        ))
    }

    fn admit_request(&self, request: &BrokerRequest) -> Result<&CompiledRouteGroup, BrokerError> {
        let header_names: Vec<String> = request
            .headers
            .iter()
            .map(|(name, _value)| name.to_ascii_lowercase())
            .collect();
        admission::admit(
            &self.groups,
            &request.origin,
            request.method,
            &request.path,
            &header_names,
        )
    }

    fn check_rate(
        &self,
        group_id: &str,
        max_requests_per_window: u32,
        window: Duration,
    ) -> Result<(), BrokerError> {
        let now = Instant::now();
        let mut windows = self
            .rate_windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let stamps = windows.entry(group_id.to_owned()).or_default();
        while let Some(oldest) = stamps.front() {
            if now.duration_since(*oldest) > window {
                stamps.pop_front();
            } else {
                break;
            }
        }
        if stamps.len() >= usize::try_from(max_requests_per_window).unwrap_or(usize::MAX) {
            return Err(BrokerError::new(BrokerErrorCode::RateLimited));
        }
        stamps.push_back(now);
        Ok(())
    }

    fn prepare_body(
        &self,
        body: Option<&Value>,
        schema: Option<&crate::schema::JsonSchema>,
        max_request_bytes: usize,
    ) -> Result<Vec<u8>, BrokerError> {
        match (body, schema) {
            (None, None) => Ok(Vec::new()),
            (Some(_), None) => Err(BrokerError::with_detail(
                BrokerErrorCode::RequestSchemaInvalid,
                String::from("route declares no request schema"),
            )),
            (None, Some(_)) => Err(BrokerError::with_detail(
                BrokerErrorCode::RequestSchemaInvalid,
                String::from("route declares a request schema but no body was supplied"),
            )),
            (Some(value), Some(schema)) => {
                schema.validate(value).map_err(|error| {
                    BrokerError::with_detail(
                        BrokerErrorCode::RequestSchemaInvalid,
                        self.sanitize(&error.to_string()),
                    )
                })?;
                let bytes = serde_json::to_vec(value)
                    .map_err(|_| BrokerError::new(BrokerErrorCode::RequestSchemaInvalid))?;
                if bytes.len() > max_request_bytes {
                    return Err(BrokerError::new(BrokerErrorCode::RequestTooLarge));
                }
                Ok(bytes)
            }
        }
    }

    fn validate_response_body(
        &self,
        raw: &[u8],
        schema: Option<&crate::schema::JsonSchema>,
    ) -> Result<Value, BrokerError> {
        let value: Value = serde_json::from_slice(raw).map_err(|error| {
            BrokerError::with_detail(
                BrokerErrorCode::ResponseMalformed,
                self.sanitize(&error.to_string()),
            )
        })?;
        // Declared-schema enforcement happens here, strictly before any guest visibility.
        if let Some(schema) = schema {
            schema.validate(&value).map_err(|error| {
                BrokerError::with_detail(
                    BrokerErrorCode::ResponseSchemaMismatch,
                    self.sanitize(&error.to_string()),
                )
            })?;
        }
        // A response echoing registered credential material is discarded wholesale; guests never
        // observe it and diagnostics stay value-free.
        let rendered = serde_json::to_string(&value)
            .map_err(|_| BrokerError::new(BrokerErrorCode::ResponseMalformed))?;
        self.filter
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .validate_persistence(&rendered)
            .map_err(|_| {
                BrokerError::with_detail(
                    BrokerErrorCode::SensitiveContentRejected,
                    String::from("response contained protected material"),
                )
            })?;
        Ok(value)
    }

    fn sanitize(&self, text: &str) -> String {
        self.filter
            .lock()
            .map(|filter| filter.sanitize(text))
            .unwrap_or_else(|poison| poison.into_inner().sanitize(text))
    }
}

fn guest_headers(request: &BrokerRequest) -> Vec<(String, String)> {
    request
        .headers
        .iter()
        .map(|(name, value)| (name.to_ascii_lowercase(), value.clone()))
        .collect()
}

fn build_url(origin_root: &str, path: &str, query: Option<&str>) -> String {
    match query {
        Some(query) => format!("{origin_root}{path}?{query}"),
        None => format!("{origin_root}{path}"),
    }
}

fn map_transport_error(error: crate::transport::TransportError) -> BrokerError {
    match error {
        crate::transport::TransportError::TimedOut => BrokerError::new(BrokerErrorCode::Timeout),
        crate::transport::TransportError::ConnectionFailure => {
            BrokerError::new(BrokerErrorCode::TransportFailure)
        }
        crate::transport::TransportError::BodyTooLarge => {
            BrokerError::new(BrokerErrorCode::ResponseTooLarge)
        }
    }
}
