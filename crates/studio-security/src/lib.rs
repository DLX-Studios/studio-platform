//! Plugin principals, capabilities, opaque secrets, and diagnostic redaction.

mod capability;
mod principal;
mod redaction;
mod registry;
mod secret;

pub use capability::{ActionGate, CapabilityId, SecurityError, SecurityErrorCode};
pub use principal::{PluginPrincipal, TrustMode};
pub use redaction::{ArtifactKind, RedactionError, SensitiveValueFilter};
pub use registry::SecretRegistry;
pub use secret::{OpaqueHandle, SecretError, SecretErrorCode, SecretPurpose};
