//! Plugin principals, capabilities, opaque secrets, and diagnostic redaction.

mod capability;
mod principal;
mod protected;
mod redaction;
mod registry;
mod secret;

pub use capability::{ActionGate, CapabilityId, SecurityError, SecurityErrorCode};
pub use principal::{PluginPrincipal, TrustMode};
pub use protected::{
    ApplicationEnvironment, ApplicationSecretStore, BrokerCredentialError, BrokerCredentialSink,
    BrokerSecretInjectionHandle, BrokerSecretInjector, CredentialBackend, CredentialBackendError,
    CredentialBytes, CredentialLocator, GuestSecretStatusApi, GuestSecretStatusHandle,
    OsCredentialBackend, ProtectedSecretError, ProtectedSecretErrorCode, ProtectedSecretKey,
    ProtectedSecretState, ProtectedSecretStatus, ProtectedSecretStore, SecretInput,
};
pub use redaction::{ArtifactKind, RedactionError, SensitiveValueFilter};
pub use registry::SecretRegistry;
pub use secret::{OpaqueHandle, SecretError, SecretErrorCode, SecretPurpose};
