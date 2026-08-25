//! Host ceilings and effective per-route-group limits.
//!
//! Defaults are generous for real application workloads while remaining explicit, declared, and
//! auditable: every bound is a number the host can print, and signed declarations may narrow but
//! never exceed these ceilings.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{BrokerError, BrokerErrorCode};

/// Host-fixed ceilings and defaults applied to every route group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrokerLimits {
    /// Maximum request body bytes.
    pub max_request_bytes: usize,
    /// Maximum response body bytes for non-streaming routes.
    pub max_response_bytes: usize,
    /// Request timeout for non-streaming routes.
    pub request_timeout: Duration,
    /// Maximum admitted requests per route group inside one rate window.
    pub max_requests_per_window: u32,
    /// Sliding rate-window length.
    pub rate_window: Duration,
    /// Maximum total bytes delivered across one stream.
    pub max_stream_bytes: usize,
    /// Maximum validated chunk events across one stream.
    pub max_stream_events: u64,
    /// Maximum wall-clock lifetime of one stream including host-owned reconnects.
    pub stream_max_duration: Duration,
    /// Maximum gap between stream bytes before the connection is considered dead.
    pub stream_idle_timeout: Duration,
}

impl Default for BrokerLimits {
    fn default() -> Self {
        Self {
            max_request_bytes: 1024 * 1024,
            max_response_bytes: 8 * 1024 * 1024,
            request_timeout: Duration::from_secs(30),
            max_requests_per_window: 120,
            rate_window: Duration::from_secs(60),
            max_stream_bytes: 64 * 1024 * 1024,
            max_stream_events: 100_000,
            stream_max_duration: Duration::from_secs(3600),
            stream_idle_timeout: Duration::from_secs(60),
        }
    }
}

impl BrokerLimits {
    /// Validate that configured ceilings are coherent and nonzero.
    ///
    /// # Errors
    ///
    /// Returns [`BrokerErrorCode::DeclarationInvalid`] when any ceiling is zero.
    pub fn validate(&self) -> Result<(), BrokerError> {
        if self.max_request_bytes == 0
            || self.max_response_bytes == 0
            || self.request_timeout.is_zero()
            || self.max_requests_per_window == 0
            || self.rate_window.is_zero()
            || self.max_stream_bytes == 0
            || self.max_stream_events == 0
            || self.stream_max_duration.is_zero()
            || self.stream_idle_timeout.is_zero()
        {
            return Err(BrokerError::new(BrokerErrorCode::DeclarationInvalid));
        }
        Ok(())
    }
}

/// Signed per-group narrowing limits; absent fields inherit host defaults.
///
/// Values above host ceilings are rejected at declaration admission rather than clamped so that
/// the signed package always states exactly what runs.
#[derive(Clone, Copy, Debug, Default, Eq, JsonSchema, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredLimits {
    /// Narrower maximum request body bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_request_bytes: Option<usize>,
    /// Narrower maximum response body bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_response_bytes: Option<usize>,
    /// Narrower request timeout in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    /// Narrower per-window request allowance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_requests_per_window: Option<u32>,
    /// Narrower total stream byte budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_stream_bytes: Option<usize>,
    /// Narrower validated chunk-event count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_stream_events: Option<u64>,
    /// Narrower stream lifetime in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_max_duration_ms: Option<u64>,
}

/// Limits resolved for one route group: declared narrower values or host defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectiveLimits {
    /// Maximum request body bytes.
    pub max_request_bytes: usize,
    /// Maximum response body bytes.
    pub max_response_bytes: usize,
    /// Request timeout.
    pub timeout: Duration,
    /// Per-route-group request allowance per window.
    pub max_requests_per_window: u32,
    /// Sliding rate-window length (host-owned; not declarable).
    pub rate_window: Duration,
    /// Total stream byte budget.
    pub max_stream_bytes: usize,
    /// Validated chunk-event count.
    pub max_stream_events: u64,
    /// Stream lifetime including host-owned reconnects.
    pub stream_max_duration: Duration,
    /// Idle gap before a stream is considered dead.
    pub stream_idle_timeout: Duration,
}

impl EffectiveLimits {
    /// Resolve declared group limits against host ceilings.
    ///
    /// # Errors
    ///
    /// Returns [`BrokerErrorCode::DeclarationInvalid`] when a declared value exceeds its host
    /// ceiling or is zero.
    pub fn resolve(declared: &DeclaredLimits, ceilings: &BrokerLimits) -> Result<Self, BrokerError> {
        let resolved = Self {
            max_request_bytes: declared
                .max_request_bytes
                .unwrap_or(ceilings.max_request_bytes),
            max_response_bytes: declared
                .max_response_bytes
                .unwrap_or(ceilings.max_response_bytes),
            timeout: declared
                .timeout_ms
                .map_or(ceilings.request_timeout, Duration::from_millis),
            max_requests_per_window: declared
                .max_requests_per_window
                .unwrap_or(ceilings.max_requests_per_window),
            rate_window: ceilings.rate_window,
            max_stream_bytes: declared.max_stream_bytes.unwrap_or(ceilings.max_stream_bytes),
            max_stream_events: declared
                .max_stream_events
                .unwrap_or(ceilings.max_stream_events),
            stream_max_duration: declared
                .stream_max_duration_ms
                .map_or(ceilings.stream_max_duration, Duration::from_millis),
            stream_idle_timeout: ceilings.stream_idle_timeout,
        };
        let within = resolved.max_request_bytes <= ceilings.max_request_bytes
            && resolved.max_response_bytes <= ceilings.max_response_bytes
            && resolved.timeout <= ceilings.request_timeout
            && resolved.max_requests_per_window <= ceilings.max_requests_per_window
            && resolved.max_stream_bytes <= ceilings.max_stream_bytes
            && resolved.max_stream_events <= ceilings.max_stream_events
            && resolved.stream_max_duration <= ceilings.stream_max_duration;
        if !within {
            return Err(BrokerError::with_detail(
                BrokerErrorCode::DeclarationInvalid,
                "declared limit exceeds host ceiling".to_owned(),
            ));
        }
        resolved.validate()
    }

    /// Validate coherence of the resolved set.
    ///
    /// # Errors
    ///
    /// Returns [`BrokerErrorCode::DeclarationInvalid`] when any value is zero.
    pub fn validate(&self) -> Result<(), BrokerError> {
        if self.max_request_bytes == 0
            || self.max_response_bytes == 0
            || self.timeout.is_zero()
            || self.max_requests_per_window == 0
            || self.max_stream_bytes == 0
            || self.max_stream_events == 0
            || self.stream_max_duration.is_zero()
        {
            return Err(BrokerError::new(BrokerErrorCode::DeclarationInvalid));
        }
        Ok(())
    }
}
