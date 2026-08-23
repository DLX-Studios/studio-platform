//! Stable package validation failures.

use thiserror::Error;

/// Stable closed-manifest failure family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManifestErrorCode {
    /// JSON syntax, shape, type, duplicate, or unknown-field failure.
    InvalidJson,
    /// Identity, semantic version, or display metadata failure.
    ManifestInvalid,
    /// Schema or protocol major version is unsupported.
    VersionUnsupported,
    /// Capability is unknown or duplicated.
    CapabilityInvalid,
    /// Requested resources exceed or invalidate host ceilings.
    LimitInvalid,
    /// Entry or asset path is unsafe or inconsistent.
    PathInvalid,
}

/// Detailed closed-manifest validation failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ManifestError {
    /// JSON could not be decoded into the closed v1 shape.
    #[error("invalid manifest JSON: {0}")]
    InvalidJson(String),
    /// Identity or semantic metadata is invalid.
    #[error("invalid manifest field: {0}")]
    ManifestInvalid(&'static str),
    /// Closed major version is unsupported.
    #[error("unsupported {field} version {actual}")]
    VersionUnsupported {
        /// Version selector field.
        field: &'static str,
        /// Unsupported numeric version.
        actual: u16,
    },
    /// Capability catalog violation.
    #[error("invalid capability declaration")]
    CapabilityInvalid,
    /// Resource ceiling violation.
    #[error("invalid manifest resource limits")]
    LimitInvalid,
    /// Unsafe or inconsistent archive path declaration.
    #[error("invalid manifest path: {0}")]
    PathInvalid(String),
}

impl ManifestError {
    /// Return the stable family for this detailed manifest rejection.
    #[must_use]
    pub const fn code(&self) -> ManifestErrorCode {
        match self {
            Self::InvalidJson(_) => ManifestErrorCode::InvalidJson,
            Self::ManifestInvalid(_) => ManifestErrorCode::ManifestInvalid,
            Self::VersionUnsupported { .. } => ManifestErrorCode::VersionUnsupported,
            Self::CapabilityInvalid => ManifestErrorCode::CapabilityInvalid,
            Self::LimitInvalid => ManifestErrorCode::LimitInvalid,
            Self::PathInvalid(_) => ManifestErrorCode::PathInvalid,
        }
    }
}
