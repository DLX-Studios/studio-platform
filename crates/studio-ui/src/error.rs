//! Stable retained-UI developer diagnostics.

/// Bounded diagnostic emitted without node values or plugin-controlled text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiDiagnostic {
    code: &'static str,
    message: &'static str,
}

impl UiDiagnostic {
    pub(crate) const fn new(code: &'static str, message: &'static str) -> Self {
        Self { code, message }
    }

    /// Stable machine-readable code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        self.code
    }

    /// Safe corrective summary.
    #[must_use]
    pub const fn message(self) -> &'static str {
        self.message
    }
}
