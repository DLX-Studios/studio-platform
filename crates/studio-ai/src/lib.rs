//! OpenAI-compatible AI SDK skeleton.
//!
//! This package defines the stable request and chunk projections used by future agent surfaces.
//! It deliberately keeps transport and API keys outside the guest: completion requests use the
//! REST broker and streaming responses arrive as already-validated [`studio_net::StreamEvent`]
//! values. Providers can therefore be swapped by changing route configuration, not application
//! code.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use studio_net::{BrokerError, BrokerRequest, GuestRestApi, StreamEvent, StreamHandle, TypedResponse};
use studio_net::declaration::{CredentialSource, DeclaredLimits, HttpMethod, RouteGroupDeclaration, StreamingDeclaration};
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
    pub temperature: Option<f64>,
}

impl ChatRequest {
    /// Create a non-streaming request.
    #[must_use]
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self { model: model.into(), messages, stream: false, temperature: None }
    }

    /// Mark the request as streaming.
    #[must_use]
    pub const fn streaming(mut self) -> Self {
        self.stream = true;
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
        Self { origin: origin.into(), api_key_name: api_key_name.into() }
    }

    /// Signed route declarations for completion and future provider streaming.
    #[must_use]
    pub fn route_groups(&self) -> Vec<RouteGroupDeclaration> {
        let credential = CredentialSource::NamedSecret {
            name: self.api_key_name.clone(),
            header: "authorization".to_owned(),
            prefix: Some("Bearer ".to_owned()),
        };
        vec![RouteGroupDeclaration {
            id: "ai.chat.completions".to_owned(),
            origins: vec![self.origin.clone()],
            methods: vec![HttpMethod::Post],
            paths: vec![CHAT_COMPLETIONS_PATH.to_owned()],
            allowed_headers: vec!["content-type".to_owned(), "accept".to_owned()],
            credential: credential.clone(),
            request_schema: Some(chat_request_schema()),
            response_schema: Some(json!({ "type": "object", "additionalProperties": true })),
            streaming: None,
            limits: DeclaredLimits { max_request_bytes: Some(512 * 1024), max_response_bytes: Some(8 * 1024 * 1024), ..DeclaredLimits::default() },
        }, RouteGroupDeclaration {
            id: "ai.chat.completions.stream".to_owned(),
            origins: vec![self.origin.clone()],
            methods: vec![HttpMethod::Get],
            paths: vec!["/v1/chat/completions/stream".to_owned()],
            allowed_headers: vec!["accept".to_owned()],
            credential,
            request_schema: None,
            response_schema: None,
            streaming: Some(StreamingDeclaration {
                chunk_schema: chat_chunk_schema(),
                reconnects: Some(2),
                retry_base_delay_ms: Some(250),
            }),
            limits: DeclaredLimits { max_stream_bytes: Some(16 * 1024 * 1024), max_stream_events: Some(50_000), ..DeclaredLimits::default() },
        }]
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
        let body = serde_json::to_value(request).map_err(|_| AiError::PayloadInvalid)?;
        let response = self.api.execute(
            BrokerRequest::new(&self.endpoint.origin, HttpMethod::Post, CHAT_COMPLETIONS_PATH)
                .with_header("content-type", "application/json")
                .with_header("accept", "application/json")
                .with_body(body),
        )?;
        Ok(response)
    }

    /// Open the provider adapter's declared SSE route.
    ///
    /// The current broker's streaming contract is GET-only. The adapter endpoint is intentionally
    /// separate so a host can translate the canonical [`ChatRequest`] to its provider's streaming
    /// request without adding a socket or credential path to the guest.
    pub fn stream(&self, request: &ChatRequest) -> Result<AiStream, AiError> {
        let query = format!("model={}&stream=true", percent_encode(&request.model));
        let handle = self.api.open_stream(
            BrokerRequest::new(&self.endpoint.origin, HttpMethod::Get, "/v1/chat/completions/stream")
                .with_header("accept", "text/event-stream")
                .with_query(query),
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
            StreamEvent::Chunk(value) => Some(parse_chunk(&value).map(AiStreamEvent::Delta)),
            StreamEvent::RetryScheduled { attempt, delay_ms } => Some(Ok(AiStreamEvent::RetryScheduled { attempt, delay_ms })),
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
        }}, "stream": { "type": "boolean" }, "temperature": { "type": "number", "minimum": 0, "maximum": 2 }
    }, "additionalProperties": false })
}

fn chat_chunk_schema() -> Value {
    json!({ "type": "object", "properties": {
        "choices": { "type": "array", "maxItems": 32, "items": { "type": "object", "properties": {
            "index": { "type": "integer" }, "delta": { "type": "object", "properties": { "content": { "type": "string" } }, "additionalProperties": true }, "finish_reason": {}
        }, "additionalProperties": true }}
    }, "additionalProperties": true })
}

fn parse_chunk(value: &Value) -> Result<ChatDelta, AiError> {
    let choice = value.get("choices").and_then(Value::as_array).and_then(|choices| choices.first()).ok_or(AiError::PayloadInvalid)?;
    Ok(ChatDelta {
        choice_index: choice.get("index").and_then(Value::as_u64).unwrap_or(0),
        text: choice.get("delta").and_then(|delta| delta.get("content")).and_then(Value::as_str).map(ToOwned::to_owned),
        finish_reason: choice.get("finish_reason").and_then(Value::as_str).map(ToOwned::to_owned),
    })
}

fn percent_encode(value: &str) -> String {
    value.bytes().fold(String::new(), |mut result, byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            result.push(char::from(byte));
        } else {
            result.push_str(&format!("%{byte:02X}"));
        }
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_net::limits::BrokerLimits;

    #[test]
    fn request_shape_and_endpoint_routes_are_deterministic() {
        let request = ChatRequest::new("proof-model", vec![ChatMessage { role: "user".to_owned(), content: "hello".to_owned() }]).streaming();
        let json = AiClient::request_json(&request).expect("request is serializable");
        assert_eq!(json["model"], "proof-model");
        assert_eq!(json["stream"], true);
        let endpoint = AiEndpoint::new("https://ai.example.test", "ai.api_key");
        assert!(endpoint.route_groups().iter().all(|route| route.compile(&BrokerLimits::default()).is_ok()));
    }

    #[test]
    fn chunk_projection_normalizes_openai_delta() {
        let chunk = json!({ "choices": [{ "index": 1, "delta": { "content": "hi" }, "finish_reason": null }] });
        assert_eq!(parse_chunk(&chunk).expect("valid chunk"), ChatDelta { choice_index: 1, text: Some("hi".to_owned()), finish_reason: None });
    }
}
