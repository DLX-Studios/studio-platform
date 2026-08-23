//! Central rejection and redaction of raw secrets and active opaque references.

use std::{error::Error, fmt};

use zeroize::Zeroizing;

use crate::OpaqueHandle;

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
            safe = safe.replace(pattern.as_str(), "[REDACTED]");
        }
        safe
    }

    /// Reject a would-be persisted value if it contains any registered sensitive value.
    ///
    /// # Errors
    ///
    /// Returns [`RedactionError`] instead of writing redacted data when a match exists.
    pub fn validate_persistence(&self, value: &str) -> Result<(), RedactionError> {
        if self
            .patterns
            .iter()
            .any(|pattern| value.contains(pattern.as_str()))
        {
            Err(RedactionError)
        } else {
            Ok(())
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
