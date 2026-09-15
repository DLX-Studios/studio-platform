//! Central rejection and redaction of raw secrets and active opaque references.

use std::{error::Error, fmt};

use serde_json::{Map, Value};
use zeroize::Zeroizing;

use crate::OpaqueHandle;

const REDACTED: &str = "[REDACTED]";
const SENSITIVE_LABELS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "api_key",
    "api-key",
    "apikey",
    "access_token",
    "access-token",
    "refresh_token",
    "refresh-token",
    "client_secret",
    "client-secret",
    "password",
    "passwd",
    "private_key",
    "private-key",
    "signing_key",
    "signing-key",
    "credential",
    "secret",
    "token",
];

/// Observable host artifact being sanitized.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactKind {
    /// Comparable UI/native state snapshot.
    Snapshot,
    /// Correlated guest-facing action result.
    ActionResult,
    /// Structured or rendered receipt content.
    Receipt,
}

/// Sensitive output was rejected at a persistence boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RedactionError;

impl fmt::Display for RedactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("sensitive output rejected")
    }
}

impl Error for RedactionError {}

/// Host-owned catalog of values that must not cross observable or durable boundaries.
pub struct SensitiveValueFilter {
    patterns: Vec<Zeroizing<String>>,
}

impl SensitiveValueFilter {
    /// Create an empty filter.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    /// Register host-owned UTF-8 secret bytes for later rejection/redaction.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, or non-UTF-8 values.
    pub fn register_secret(&mut self, secret: &[u8]) -> Result<(), RedactionError> {
        if secret.is_empty() || secret.len() > 4096 {
            return Err(RedactionError);
        }
        let value = std::str::from_utf8(secret).map_err(|_| RedactionError)?;
        self.register_pattern(value);
        Ok(())
    }

    /// Register one active opaque reference without exposing it through formatting.
    pub fn register_handle(&mut self, handle: &OpaqueHandle) {
        self.register_pattern(&handle.to_token());
    }

    /// Sanitize an observable structured artifact.
    #[must_use]
    pub fn sanitize_artifact(&self, _kind: ArtifactKind, value: &str) -> String {
        self.sanitize(value)
    }

    /// Sanitize untrusted log or diagnostic text.
    #[must_use]
    pub fn sanitize(&self, value: &str) -> String {
        let mut safe = value.to_owned();
        let mut patterns = self.patterns.iter().collect::<Vec<_>>();
        patterns.sort_by_key(|pattern| std::cmp::Reverse(pattern.len()));
        for pattern in patterns {
            safe = safe.replace(pattern.as_str(), REDACTED);
        }
        safe = scrub_labeled_values(&safe);
        scrub_key_shaped_tokens(&safe)
    }

    /// Recursively sanitize a JSON-like diagnostic or logging structure.
    ///
    /// Values under credential-shaped field names are replaced wholesale. Other string leaves
    /// pass through the exact-value and key-shape scrubber. The input is never modified.
    #[must_use]
    pub fn sanitize_json(&self, value: &Value) -> Value {
        match value {
            Value::Object(object) => Value::Object(
                object
                    .iter()
                    .map(|(key, value)| {
                        let value = if sensitive_field(key) {
                            Value::String(REDACTED.to_owned())
                        } else {
                            self.sanitize_json(value)
                        };
                        (key.clone(), value)
                    })
                    .collect::<Map<_, _>>(),
            ),
            Value::Array(values) => Value::Array(
                values
                    .iter()
                    .map(|value| self.sanitize_json(value))
                    .collect(),
            ),
            Value::String(value) => Value::String(self.sanitize(value)),
            Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
        }
    }

    /// Reject a would-be persisted value if it contains any registered sensitive value.
    ///
    /// # Errors
    ///
    /// Returns [`RedactionError`] instead of writing redacted data when a match exists.
    pub fn validate_persistence(&self, value: &str) -> Result<(), RedactionError> {
        if self.sanitize(value) == value {
            Ok(())
        } else {
            Err(RedactionError)
        }
    }

    fn register_pattern(&mut self, value: &str) {
        if !self
            .patterns
            .iter()
            .any(|pattern| pattern.as_str() == value)
        {
            self.patterns.push(Zeroizing::new(value.to_owned()));
        }
    }
}

fn sensitive_field(field: &str) -> bool {
    let normalized = field
        .bytes()
        .filter(u8::is_ascii_alphanumeric)
        .map(|byte| byte.to_ascii_lowercase())
        .collect::<Vec<_>>();
    matches!(
        normalized.as_slice(),
        b"authorization"
            | b"proxyauthorization"
            | b"apikey"
            | b"accesstoken"
            | b"refreshtoken"
            | b"clientsecret"
            | b"password"
            | b"passwd"
            | b"privatekey"
            | b"signingkey"
            | b"credential"
            | b"credentials"
            | b"secret"
            | b"token"
    )
}

fn scrub_labeled_values(value: &str) -> String {
    let lowercase = value.to_ascii_lowercase();
    let mut ranges = Vec::new();
    for label in SENSITIVE_LABELS {
        let mut search_from = 0;
        while let Some(relative) = lowercase[search_from..].find(label) {
            let label_start = search_from + relative;
            let after_label = label_start + label.len();
            search_from = after_label;
            if !is_label_boundary(lowercase.as_bytes(), label_start, after_label) {
                continue;
            }
            if let Some(range) = labeled_value_range(value, &lowercase, after_label) {
                ranges.push(range);
            }
        }
    }
    replace_ranges(value, ranges)
}

fn is_label_boundary(bytes: &[u8], start: usize, end: usize) -> bool {
    let left = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
    let right = end == bytes.len()
        || !bytes[end].is_ascii_alphanumeric()
        || matches!(bytes[end], b'_' | b'-');
    left && right
}

fn labeled_value_range(value: &str, lowercase: &str, mut cursor: usize) -> Option<(usize, usize)> {
    let bytes = value.as_bytes();
    while cursor < bytes.len()
        && (bytes[cursor].is_ascii_whitespace() || matches!(bytes[cursor], b'"' | b'\''))
    {
        cursor += 1;
    }
    if cursor >= bytes.len() || !matches!(bytes[cursor], b':' | b'=') {
        return None;
    }
    cursor += 1;
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    let quote = bytes
        .get(cursor)
        .copied()
        .filter(|byte| matches!(byte, b'"' | b'\''));
    if quote.is_some() {
        cursor += 1;
    }
    if lowercase[cursor..].starts_with("bearer ") {
        cursor += "bearer ".len();
    }
    let start = cursor;
    let end = if let Some(quote) = quote {
        bytes[start..]
            .iter()
            .position(|byte| *byte == quote)
            .map_or(bytes.len(), |relative| start + relative)
    } else {
        bytes[start..]
            .iter()
            .position(|byte| {
                byte.is_ascii_whitespace() || matches!(byte, b',' | b';' | b'&' | b'}' | b']')
            })
            .map_or(bytes.len(), |relative| start + relative)
    };
    (start < end && &value[start..end] != REDACTED).then_some((start, end))
}

fn scrub_key_shaped_tokens(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut ranges = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        while cursor < bytes.len() && !token_byte(bytes[cursor]) {
            cursor += 1;
        }
        let start = cursor;
        while cursor < bytes.len() && token_byte(bytes[cursor]) {
            cursor += 1;
        }
        if start < cursor && looks_key_shaped(&value[start..cursor]) {
            ranges.push((start, cursor));
        }
    }
    replace_ranges(value, ranges)
}

const fn token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
}

fn looks_key_shaped(value: &str) -> bool {
    if value == REDACTED || value.len() < 16 {
        return false;
    }
    let lowercase = value.to_ascii_lowercase();
    lowercase.starts_with("sk_live_")
        || lowercase.starts_with("sk_test_")
        || lowercase.starts_with("rk_live_")
        || lowercase.starts_with("github_pat_")
        || lowercase.starts_with("ghp_")
        || lowercase.starts_with("gho_")
        || lowercase.starts_with("xoxb-")
        || lowercase.starts_with("xoxp-")
        || (value.starts_with("AKIA") && value.len() == 20)
        || looks_like_jwt(value)
}

fn looks_like_jwt(value: &str) -> bool {
    let mut segments = value.split('.');
    let Some(first) = segments.next() else {
        return false;
    };
    let Some(second) = segments.next() else {
        return false;
    };
    let Some(third) = segments.next() else {
        return false;
    };
    segments.next().is_none()
        && first.len() >= 8
        && second.len() >= 8
        && third.len() >= 8
        && [first, second, third]
            .iter()
            .all(|segment| segment.bytes().all(base64_url_byte))
}

const fn base64_url_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn replace_ranges(value: &str, mut ranges: Vec<(usize, usize)>) -> String {
    ranges.sort_unstable();
    ranges.dedup();
    let mut safe = value.to_owned();
    let mut covered_start = safe.len();
    for (start, end) in ranges.into_iter().rev() {
        if end <= covered_start {
            safe.replace_range(start..end, REDACTED);
            covered_start = start;
        }
    }
    safe
}

impl Default for SensitiveValueFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for SensitiveValueFilter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SensitiveValueFilter")
            .field("registered_values", &self.patterns.len())
            .finish()
    }
}
