//! Plugin principals, capabilities, opaque secrets, and diagnostic redaction.

mod capability;
mod environment;
mod password;
mod principal;
mod protected;
mod redaction;
mod registry;
mod secret;
mod session;

pub use capability::{ActionGate, CapabilityId, SecurityError, SecurityErrorCode};
pub use environment::{
    ActiveEnvironment, EnvironmentDataKey, EnvironmentDataScope, EnvironmentDataStore,
    EnvironmentError, EnvironmentErrorCode, PromotionDirection, PromotionEntry, PromotionPlan,
    PromotionReceipt, ProtectedConfiguration, SecretFreeMetadata, apply_promotion,
    resolve_active_environment,
};
pub use password::{PasswordError, PasswordErrorCode, PasswordVerifier};
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
pub use session::{
    MemorySessionCredentialStore, OsSessionCredentialStore, SessionCredentialError,
    SessionCredentialErrorCode, SessionCredentialStore,
};
