//! Stable descriptor and registry rejection families.

use thiserror::Error;

use crate::descriptor::LifecycleHook;
use crate::lifecycle::ViolationReason;

/// Stable closed-schema rejection family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DescriptorErrorCode {
    /// Encoded descriptor exceeded the host byte ceiling.
    ByteLimitExceeded,
    /// Descriptor JSON could not be parsed or serialized.
    JsonInvalid,
    /// The descriptor carried a field outside the closed schema.
    SchemaUnknownField,
    /// A declared field failed host validation rules.
    SchemaFieldInvalid,
    /// Descriptor or compatibility versions are unsupported by the running host.
    VersionUnsupported,
    /// A contribution violated structural contribution rules.
    ContributionInvalid,
    /// Signature encoding or cryptographic verification failed.
    SignatureInvalid,
}

/// Detailed descriptor rejection.
#[derive(Clone, Debug, Error, PartialEq)]
#[error("plugin descriptor rejected ({code:?}): {detail}")]
pub struct DescriptorError {
    code: DescriptorErrorCode,
    detail: String,
}

impl DescriptorError {
    pub(crate) fn byte_limit() -> Self {
        Self {
            code: DescriptorErrorCode::ByteLimitExceeded,
            detail: "descriptor byte limit exceeded".to_owned(),
        }
    }

    pub(crate) fn json_invalid(detail: impl Into<String>) -> Self {
        Self {
            code: DescriptorErrorCode::JsonInvalid,
            detail: detail.into(),
        }
    }

    pub(crate) fn schema_unknown_field(detail: impl Into<String>) -> Self {
        Self {
            code: DescriptorErrorCode::SchemaUnknownField,
            detail: detail.into(),
        }
    }

    pub(crate) fn schema_field_invalid(detail: impl Into<String>) -> Self {
        Self {
            code: DescriptorErrorCode::SchemaFieldInvalid,
            detail: detail.into(),
        }
    }

    pub(crate) fn version_unsupported(detail: impl Into<String>) -> Self {
        Self {
            code: DescriptorErrorCode::VersionUnsupported,
            detail: detail.into(),
        }
    }

    pub(crate) fn contribution_invalid(detail: impl Into<String>) -> Self {
        Self {
            code: DescriptorErrorCode::ContributionInvalid,
            detail: detail.into(),
        }
    }

    pub(crate) fn duplicate_contribution(id: &str) -> Self {
        Self {
            code: DescriptorErrorCode::ContributionInvalid,
            detail: format!("duplicate contribution id {id}"),
        }
    }

    pub(crate) fn signature_invalid() -> Self {
        Self {
            code: DescriptorErrorCode::SignatureInvalid,
            detail: "descriptor signature invalid".to_owned(),
        }
    }

    /// Stable family for host diagnostics.
    #[must_use]
    pub const fn code(&self) -> DescriptorErrorCode {
        self.code
    }

    /// Non-sensitive rejection detail.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl Eq for DescriptorError {}

/// Stable registry operation rejection family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryErrorCode {
    /// Signature, trust lookup, or integrity verification failed at admission.
    AdmissionSignatureInvalid,
    /// The descriptor content failed closed-schema validation.
    DescriptorInvalid,
    /// Admitted content is incompatible with the running host.
    CompatibilityUnsupported,
    /// Contributions reference kinds outside the approved catalog.
    ContributionUnapproved,
    /// The plugin id is not admitted.
    UnknownPlugin,
    /// The operation is invalid for the plugin's current lifecycle state.
    StateInvalid,
    /// A requested capability has no recorded consent for the project.
    ConsentDenied,
    /// A lifecycle hook exceeded its declared budget or rejected its invocation.
    HookViolation,
    /// Removal is blocked because project artifacts still reference the extension.
    RemovalBlocked,
    /// Usage was recorded for an artifact the extension never declared.
    UsageUnknown,
    /// The pending removal plan is missing, stale, or superseded.
    RemovalPlanInvalid,
}

/// Detailed registry operation failure.
#[derive(Clone, Debug, Error, PartialEq)]
#[error("extension registry rejected operation ({code:?}): {detail}")]
pub struct RegistryError {
    code: RegistryErrorCode,
    detail: String,
}

impl RegistryError {
    pub(crate) fn admission_signature_invalid(detail: impl Into<String>) -> Self {
        Self {
            code: RegistryErrorCode::AdmissionSignatureInvalid,
            detail: detail.into(),
        }
    }

    pub(crate) fn descriptor_invalid(detail: impl Into<String>) -> Self {
        Self {
            code: RegistryErrorCode::DescriptorInvalid,
            detail: detail.into(),
        }
    }

    pub(crate) fn compatibility_unsupported(detail: impl Into<String>) -> Self {
        Self {
            code: RegistryErrorCode::CompatibilityUnsupported,
            detail: detail.into(),
        }
    }

    pub(crate) fn contribution_unapproved(kind: &str) -> Self {
        Self {
            code: RegistryErrorCode::ContributionUnapproved,
            detail: format!("kind `{kind}` is not in the approved primitive catalog"),
        }
    }

    pub(crate) fn unknown_plugin() -> Self {
        Self {
            code: RegistryErrorCode::UnknownPlugin,
            detail: "plugin is not admitted".to_owned(),
        }
    }

    pub(crate) fn state_invalid(detail: impl Into<String>) -> Self {
        Self {
            code: RegistryErrorCode::StateInvalid,
            detail: detail.into(),
        }
    }

    pub(crate) fn consent_denied(detail: impl Into<String>) -> Self {
        Self {
            code: RegistryErrorCode::ConsentDenied,
            detail: detail.into(),
        }
    }

    pub(crate) fn hook_violation(hook: LifecycleHook, reason: &ViolationReason) -> Self {
        Self {
            code: RegistryErrorCode::HookViolation,
            detail: format!("hook {} contained: {reason:?}", hook.name()),
        }
    }

    pub(crate) fn removal_blocked(count: usize) -> Self {
        Self {
            code: RegistryErrorCode::RemovalBlocked,
            detail: format!("{count} owned artifacts remain in the project"),
        }
    }

    pub(crate) fn usage_unknown(detail: impl Into<String>) -> Self {
        Self {
            code: RegistryErrorCode::UsageUnknown,
            detail: detail.into(),
        }
    }

    pub(crate) fn removal_plan_invalid(detail: impl Into<String>) -> Self {
        Self {
            code: RegistryErrorCode::RemovalPlanInvalid,
            detail: detail.into(),
        }
    }

    /// Stable family for host diagnostics.
    #[must_use]
    pub const fn code(&self) -> RegistryErrorCode {
        self.code
    }

    /// Non-sensitive rejection detail.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}
