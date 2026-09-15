//! Stable Designer diagnostics with centralized sensitive-value redaction.

use studio_security::SensitiveValueFilter;

/// Diagnostic safe to present on the trusted Designer surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeDiagnostic {
    code: String,
    message: String,
}

impl SafeDiagnostic {
    /// Capture a stable code and sanitize all untrusted context.
    #[must_use]
    pub fn capture(filter: &SensitiveValueFilter, code: &str, message: &str) -> Self {
        let code = if code.is_empty() || code.chars().any(char::is_control) {
            "diagnostic_invalid"
        } else {
            code
        };
        Self {
            code: code.to_owned(),
            message: filter.sanitize(message),
        }
    }

    /// Stable non-sensitive diagnostic code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Redacted operator-facing message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}
