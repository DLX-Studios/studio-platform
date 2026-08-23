use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ProtocolError, ProtocolLimits, validate_bounded_string};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionRequest {
    pub request_id: String,
    pub capability: String,
    pub operation: String,
    pub payload: Value,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActionResult {
    Success {
        request_id: String,
        payload: Value,
    },
    Failure {
        request_id: String,
        code: String,
        message: String,
        retryable: bool,
    },
}

/// Exact integer money used by receipt action payloads.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoneyPayload {
    pub currency: String,
    pub minor: i64,
}

/// One structured receipt line; raw printer bytes are not representable.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReceiptLinePayload {
    pub label: String,
    pub quantity: u32,
    pub unit_amount: MoneyPayload,
    pub line_total: MoneyPayload,
}

/// Secret-free structured approved receipt returned by the host.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReceiptPayload {
    pub receipt_id: String,
    pub merchant: String,
    pub lines: Vec<ReceiptLinePayload>,
    pub subtotal: MoneyPayload,
    pub discount: MoneyPayload,
    pub tax: MoneyPayload,
    pub total: MoneyPayload,
    pub result_reference: String,
    pub host_timestamp_millis: u64,
}

/// Closed printer-preview request containing identities only.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrintPreviewPayload {
    pub request_id: String,
    pub receipt_id: String,
}

pub(crate) fn validate_action_request(
    action: &ActionRequest,
    limits: ProtocolLimits,
) -> Result<(), ProtocolError> {
    validate_identifier(&action.request_id, "request_id")?;
    validate_identifier(&action.capability, "capability")?;
    validate_identifier(&action.operation, "operation")?;
    validate_bounded_string(&action.request_id, limits.max_string_bytes)
}

pub(crate) fn validate_action_result(
    result: &ActionResult,
    limits: ProtocolLimits,
) -> Result<(), ProtocolError> {
    match result {
        ActionResult::Success { request_id, .. } => validate_identifier(request_id, "request_id"),
        ActionResult::Failure {
            request_id,
            code,
            message,
            ..
        } => {
            validate_identifier(request_id, "request_id")?;
            validate_identifier(code, "code")?;
            validate_bounded_string(message, limits.max_string_bytes)
        }
    }
}

fn validate_identifier(value: &str, field: &'static str) -> Result<(), ProtocolError> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(ProtocolError::InvalidAction(field));
    }
    Ok(())
}
