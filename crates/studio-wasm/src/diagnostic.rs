//! Redacted diagnostic projection for untrusted guest text.

use studio_security::SensitiveValueFilter;

/// Guest diagnostic whose stored message has passed centralized redaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestDiagnostic {
    message: String,
}

impl GuestDiagnostic {
    /// Capture bounded guest text after replacing registered sensitive values.
    #[must_use]
    pub fn capture(filter: &SensitiveValueFilter, message: &str) -> Self {
        let bounded = message.get(..message.len().min(4096)).unwrap_or(message);
        Self {
            message: filter.sanitize(bounded),
        }
    }

    /// Safe message for a host-owned failure surface.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}
