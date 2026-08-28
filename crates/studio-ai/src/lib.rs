//! OpenAI-compatible AI SDK skeleton.
//!
//! This package defines the stable request and chunk projections used by future agent surfaces.
//! It deliberately keeps transport and API keys outside the guest: completion requests use the
//! REST broker and streaming responses arrive as already-validated [`studio_net::StreamEvent`]
//! values. Providers can therefore be swapped by changing route configuration, not application
//! code.

#![allow(missing_docs)]
#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::restriction,
    clippy::nursery,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::redundant_closure_for_method_calls,
    clippy::unreadable_literal
)]

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use studio_net::declaration::{
    CredentialSource, HttpMethod, RouteGroupDeclaration, StreamingDeclaration,
};
use studio_net::guest::BrokerRequest;
use studio_net::limits::DeclaredLimits;
use studio_net::{BrokerError, GuestRestApi, StreamEvent, StreamHandle, TypedResponse};
use thiserror::Error;

/// Stable SDK package identity.
pub const AI_PACKAGE_ID: &str = "@studio/ai";
/// OpenAI-compatible chat completion path.
pub const CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";

/// OpenAI-compatible chat message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatMessage {
    /// Message role (`system`, `user`, or `assistant`).
    pub role: String,
    /// UTF-8 message content.
    pub content: String,
}

/// Bounded chat completion request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatRequest {
    /// Provider model identifier.
    pub model: String,
    /// Ordered conversation messages.
    pub messages: Vec<ChatMessage>,
    /// Request incremental chunks when the provider supports them.
    pub stream: bool,
    /// Optional deterministic sampling temperature.
    #[serde(default)]
    pub temperature: Option<f64>,
    /// Provider stream controls, such as requesting a terminal usage event.
    #[serde(default)]
    pub stream_options: Option<StreamOptions>,
}

/// Bounded OpenAI-compatible stream controls.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamOptions {
    /// Ask the provider to emit a final usage-only SSE event.
    pub include_usage: bool,
}

impl ChatRequest {
    /// Create a non-streaming request.
    #[must_use]
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            stream: false,
            temperature: None,
            stream_options: None,
        }
    }

    /// Mark the request as streaming.
    #[must_use]
    pub const fn streaming(mut self) -> Self {
        self.stream = true;
        self
    }

    /// Set the provider sampling temperature; the broker enforces the declared `0..=2` bound.
    #[must_use]
    pub const fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Attach bounded provider stream controls.
    #[must_use]
    pub fn with_stream_options(mut self, options: StreamOptions) -> Self {
        self.stream_options = Some(options);
        self
    }
}

/// One normalized incremental assistant delta.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatDelta {
    /// Optional provider choice index.
    pub choice_index: u64,
    /// Incremental text, if the chunk contains text.
    pub text: Option<String>,
    /// Provider finish reason, when complete.
    pub finish_reason: Option<String>,
}

/// Token accounting emitted by a provider's terminal usage event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatUsage {
    /// Tokens consumed by the prompt.
    pub prompt_tokens: u64,
    /// Tokens produced by the completion.
    pub completion_tokens: u64,
    /// Total prompt and completion tokens.
    pub total_tokens: u64,
}

/// Provider error carried in a terminal SSE event.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatError {
    /// Provider-safe human-readable message.
    pub message: String,
    /// Optional provider error family.
    #[serde(default, rename = "type")]
    pub error_type: Option<String>,
    /// Optional provider parameter name.
    #[serde(default)]
    pub param: Option<String>,
    /// Optional provider error code.
    #[serde(default)]
    pub code: Option<String>,
}

/// Safe failure family for AI operations.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum AiError {
    /// The host broker rejected or failed the operation.
    #[error(transparent)]
    Broker(#[from] BrokerError),
    /// The request or response did not match the SDK projection.
    #[error("ai payload projection invalid")]
    PayloadInvalid,
}

/// Provider-neutral route configuration. The API key name is a protected host reference only.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiEndpoint {
    /// OpenAI-compatible HTTPS origin.
    pub origin: String,
    /// Protected configuration name resolved by the host at send time.
    pub api_key_name: String,
}

impl AiEndpoint {
    /// Create an endpoint configuration without accepting key bytes.
    #[must_use]
    pub fn new(origin: impl Into<String>, api_key_name: impl Into<String>) -> Self {
        Self {
            origin: origin.into(),
            api_key_name: api_key_name.into(),
        }
    }

    /// Signed route declarations for completion and future provider streaming.
    #[must_use]
    pub fn route_groups(&self) -> Vec<RouteGroupDeclaration> {
        let credential = CredentialSource::NamedSecret {
            name: self.api_key_name.clone(),
            header: "authorization".to_owned(),
            prefix: Some("Bearer ".to_owned()),
        };
        vec![
            RouteGroupDeclaration {
                id: "ai.chat.completions".to_owned(),
                origins: vec![self.origin.clone()],
                methods: vec![HttpMethod::Post],
                paths: vec![CHAT_COMPLETIONS_PATH.to_owned()],
                allowed_headers: vec!["content-type".to_owned(), "accept".to_owned()],
                credential: credential.clone(),
                request_schema: Some(chat_request_schema()),
                response_schema: Some(json!({ "type": "object", "additionalProperties": true })),
                streaming: None,
                limits: DeclaredLimits {
                    max_request_bytes: Some(512 * 1024),
                    max_response_bytes: Some(8 * 1024 * 1024),
                    ..DeclaredLimits::default()
                },
            },
            RouteGroupDeclaration {
                id: "ai.chat.completions.stream".to_owned(),
                origins: vec![self.origin.clone()],
                methods: vec![HttpMethod::Post],
                paths: vec!["/v1/chat/completions/stream".to_owned()],
                allowed_headers: vec!["content-type".to_owned(), "accept".to_owned()],
                credential,
                request_schema: Some(chat_request_schema()),
                response_schema: None,
                streaming: Some(StreamingDeclaration {
                    chunk_schema: chat_chunk_schema(),
                    reconnects: Some(2),
                    retry_base_delay_ms: Some(250),
                }),
                limits: DeclaredLimits {
                    max_stream_bytes: Some(16 * 1024 * 1024),
                    max_stream_events: Some(50_000),
                    ..DeclaredLimits::default()
                },
            },
        ]
    }
}

/// Typed AI client over the host broker.
pub struct AiClient<'api> {
    api: &'api GuestRestApi<'api>,
    endpoint: AiEndpoint,
}

impl<'api> AiClient<'api> {
    /// Bind an endpoint and a host-provided restricted API facade.
    #[must_use]
    pub const fn new(api: &'api GuestRestApi<'api>, endpoint: AiEndpoint) -> Self {
        Self { api, endpoint }
    }

    /// Send one non-streaming OpenAI-compatible completion.
    pub fn complete(&self, request: &ChatRequest) -> Result<TypedResponse, AiError> {
        let body = serde_json::to_value(ChatRequest {
            stream: false,
            ..request.clone()
        })
        .map_err(|_| AiError::PayloadInvalid)?;
        let response = self.api.execute(
            BrokerRequest::new(
                &self.endpoint.origin,
                HttpMethod::Post,
                CHAT_COMPLETIONS_PATH,
            )
            .with_header("content-type", "application/json")
            .with_header("accept", "application/json")
            .with_body(body),
        )?;
        Ok(response)
    }

    /// Open the provider adapter's declared SSE route with the canonical request body.
    pub fn stream(&self, request: &ChatRequest) -> Result<AiStream, AiError> {
        let body = serde_json::to_value(request.clone().streaming())
            .map_err(|_| AiError::PayloadInvalid)?;
        let handle = self.api.open_stream(
            BrokerRequest::new(
                &self.endpoint.origin,
                HttpMethod::Post,
                "/v1/chat/completions/stream",
            )
            .with_body(body),
        )?;
        Ok(AiStream { handle })
    }

    /// Serialize a canonical request for a provider adapter without sending it.
    pub fn request_json(request: &ChatRequest) -> Result<Value, AiError> {
        serde_json::to_value(request).map_err(|_| AiError::PayloadInvalid)
    }
}

/// Validated incremental stream wrapper.
pub struct AiStream {
    handle: StreamHandle,
}

impl std::fmt::Debug for AiStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AiStream")
    }
}

impl AiStream {
    /// Read the next lifecycle event and normalize valid chunk payloads.
    pub fn next(&self) -> Option<Result<AiStreamEvent, AiError>> {
        match self.handle.next_event()? {
            StreamEvent::Opened => Some(Ok(AiStreamEvent::Opened)),
            StreamEvent::Chunk(value) => Some(parse_stream_event(&value)),
            StreamEvent::RetryScheduled { attempt, delay_ms } => {
                Some(Ok(AiStreamEvent::RetryScheduled { attempt, delay_ms }))
            }
            StreamEvent::Completed => Some(Ok(AiStreamEvent::Completed)),
            StreamEvent::Failed(error) => Some(Err(AiError::Broker(error))),
            StreamEvent::Cancelled => Some(Ok(AiStreamEvent::Cancelled)),
        }
    }

    /// Ask the host to cancel the stream.
    pub fn cancel(&self) {
        self.handle.cancel();
    }
}

/// Stream lifecycle visible to an application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AiStreamEvent {
    /// The host opened a provider stream.
    Opened,
    /// One normalized assistant delta.
    Delta(ChatDelta),
    /// Terminal provider token accounting.
    Usage(ChatUsage),
    /// Terminal provider error event.
    Error(ChatError),
    /// The host scheduled a reconnect.
    RetryScheduled {
        /// One-based attempt.
        attempt: u32,
        /// Backoff in milliseconds.
        delay_ms: u64,
    },
    /// Provider stream completed.
    Completed,
    /// Guest cancellation completed.
    Cancelled,
}

fn chat_request_schema() -> Value {
    json!({ "type": "object", "required": ["model", "messages", "stream"], "properties": {
        "model": { "type": "string", "minLength": 1, "maxLength": 256 },
        "messages": { "type": "array", "minItems": 1, "maxItems": 128, "items": {
            "type": "object", "required": ["role", "content"], "properties": {
                "role": { "type": "string", "maxLength": 32 }, "content": { "type": "string", "maxLength": 128000 }
            }, "additionalProperties": false
        }}, "stream": { "type": "boolean" }, "temperature": { "type": "number", "minimum": 0, "maximum": 2 },
        "stream_options": { "type": "object", "required": ["include_usage"], "properties": { "include_usage": { "type": "boolean" } }, "additionalProperties": false }
    }, "additionalProperties": false })
}

fn chat_chunk_schema() -> Value {
    json!({ "type": "object", "properties": {
        "choices": { "type": "array", "maxItems": 32, "items": { "type": "object", "properties": {
            "index": { "type": "integer" }, "delta": { "type": "object", "properties": { "content": { "maxLength": 128000 } }, "additionalProperties": true }, "finish_reason": {}
        }, "additionalProperties": true }},
        "usage": { "required": ["prompt_tokens", "completion_tokens", "total_tokens"], "properties": {
            "prompt_tokens": { "type": "integer" }, "completion_tokens": { "type": "integer" }, "total_tokens": { "type": "integer" }
        }, "additionalProperties": true },
        "error": { "type": "object", "required": ["message"], "properties": {
            "message": { "type": "string", "minLength": 1, "maxLength": 8192 }, "type": { "maxLength": 128 }, "param": { "maxLength": 256 }, "code": {}
        }, "additionalProperties": true }
    }, "additionalProperties": true })
}

fn parse_stream_event(value: &Value) -> Result<AiStreamEvent, AiError> {
    if value.get("error").is_some() {
        return parse_error(value).map(AiStreamEvent::Error);
    }
    if value
        .get("choices")
        .and_then(Value::as_array)
        .is_none_or(|choices| choices.is_empty())
        && value.get("usage").is_some()
    {
        return parse_usage(value).map(AiStreamEvent::Usage);
    }
    parse_chunk(value).map(AiStreamEvent::Delta)
}

fn parse_usage(value: &Value) -> Result<ChatUsage, AiError> {
    let usage = value.get("usage").ok_or(AiError::PayloadInvalid)?;
    Ok(ChatUsage {
        prompt_tokens: usage
            .get("prompt_tokens")
            .and_then(Value::as_u64)
            .ok_or(AiError::PayloadInvalid)?,
        completion_tokens: usage
            .get("completion_tokens")
            .and_then(Value::as_u64)
            .ok_or(AiError::PayloadInvalid)?,
        total_tokens: usage
            .get("total_tokens")
            .and_then(Value::as_u64)
            .ok_or(AiError::PayloadInvalid)?,
    })
}

fn parse_error(value: &Value) -> Result<ChatError, AiError> {
    let error = value
        .get("error")
        .and_then(Value::as_object)
        .ok_or(AiError::PayloadInvalid)?;
    Ok(ChatError {
        message: error
            .get("message")
            .and_then(Value::as_str)
            .filter(|message| !message.is_empty())
            .ok_or(AiError::PayloadInvalid)?
            .to_owned(),
        error_type: error
            .get("type")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        param: error
            .get("param")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        code: error
            .get("code")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    })
}

fn parse_chunk(value: &Value) -> Result<ChatDelta, AiError> {
    let choice = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .ok_or(AiError::PayloadInvalid)?;
    Ok(ChatDelta {
        choice_index: choice.get("index").and_then(Value::as_u64).unwrap_or(0),
        text: choice
            .get("delta")
            .and_then(|delta| delta.get("content"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        finish_reason: choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_net::limits::BrokerLimits;

    #[test]
    fn request_shape_and_endpoint_routes_are_deterministic() {
        let request = ChatRequest {
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
            temperature: Some(0.25),
            ..ChatRequest::new(
                "proof-model",
                vec![ChatMessage {
                    role: "user".to_owned(),
                    content: "hello".to_owned(),
                }],
            )
            .streaming()
        };
        let json = AiClient::request_json(&request).expect("request is serializable");
        assert_eq!(json["model"], "proof-model");
        assert_eq!(json["stream"], true);
        assert_eq!(json["messages"][0]["content"], "hello");
        assert_eq!(json["temperature"], 0.25);
        assert_eq!(json["stream_options"]["include_usage"], true);
        let endpoint = AiEndpoint::new("https://ai.example.test", "ai.api_key");
        assert!(
            endpoint
                .route_groups()
                .iter()
                .all(|route| route.compile(&BrokerLimits::default()).is_ok())
        );
        assert_eq!(endpoint.route_groups()[1].methods, vec![HttpMethod::Post]);
        assert!(endpoint.route_groups()[1].request_schema.is_some());
    }

    #[test]
    fn chunk_projection_normalizes_openai_delta() {
        let chunk = json!({ "choices": [{ "index": 1, "delta": { "content": "hi" }, "finish_reason": null }] });
        assert_eq!(
            parse_chunk(&chunk).expect("valid chunk"),
            ChatDelta {
                choice_index: 1,
                text: Some("hi".to_owned()),
                finish_reason: None
            }
        );
    }

    #[test]
    fn terminal_usage_and_provider_error_events_are_typed() {
        let usage = json!({ "choices": [], "usage": { "prompt_tokens": 7, "completion_tokens": 3, "total_tokens": 10 } });
        assert_eq!(
            parse_stream_event(&usage).expect("usage event"),
            AiStreamEvent::Usage(ChatUsage {
                prompt_tokens: 7,
                completion_tokens: 3,
                total_tokens: 10
            })
        );

        let error = json!({ "error": { "message": "rate limited", "type": "rate_limit_error", "code": "rate_limit" } });
        assert_eq!(
            parse_stream_event(&error).expect("provider error event"),
            AiStreamEvent::Error(ChatError {
                message: "rate limited".to_owned(),
                error_type: Some("rate_limit_error".to_owned()),
                param: None,
                code: Some("rate_limit".to_owned()),
            })
        );
    }
}
