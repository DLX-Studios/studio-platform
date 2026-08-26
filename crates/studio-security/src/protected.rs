//! Host-owned, application-scoped protected configuration.

use std::{collections::HashSet, error::Error, fmt, marker::PhantomData};

use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

use crate::{PluginPrincipal, TrustMode};

const PARTITION_DOMAIN: &[u8] = b"studio.protected-secret.partition.v1";
const CREDENTIAL_DOMAIN: &[u8] = b"studio.protected-secret.credential.v1";
const CONFIGURED_RECORD: &[u8] = b"studio.protected-secret.configured.v1\0";
const REVOKED_RECORD: &[u8] = b"studio.protected-secret.revoked.v1\0";
const MAX_SECRET_BYTES: usize = 4096;
const CREDENTIAL_SERVICE: &str = "com.dlx-studios.studio.protected-secrets.v1";

const KNOWN_DEFAULT_DIGESTS: [[u8; 32]; 6] = [
    [
        0x5e, 0x88, 0x48, 0x98, 0xda, 0x28, 0x04, 0x71, 0x51, 0xd0, 0xe5, 0x6f, 0x8d, 0xc6, 0x29,
        0x27, 0x73, 0x60, 0x3d, 0x0d, 0x6a, 0xab, 0xbd, 0xd6, 0x2a, 0x11, 0xef, 0x72, 0x1d, 0x15,
        0x42, 0xd8,
    ],
    [
        0x8c, 0x69, 0x76, 0xe5, 0xb5, 0x41, 0x04, 0x15, 0xbd, 0xe9, 0x08, 0xbd, 0x4d, 0xee, 0x15,
        0xdf, 0xb1, 0x67, 0xa9, 0xc8, 0x73, 0xfc, 0x4b, 0xb8, 0xa8, 0x1f, 0x6f, 0x2a, 0xb4, 0x48,
        0xa9, 0x18,
    ],
    [
        0x05, 0x7b, 0xa0, 0x3d, 0x6c, 0x44, 0x10, 0x48, 0x63, 0xdc, 0x73, 0x61, 0xfe, 0x45, 0x78,
        0x96, 0x5d, 0x18, 0x87, 0x36, 0x0f, 0x90, 0xa0, 0x89, 0x58, 0x82, 0xe5, 0x8a, 0x62, 0x48,
        0xfc, 0x86,
    ],
    [
        0x8d, 0x96, 0x9e, 0xef, 0x6e, 0xca, 0xd3, 0xc2, 0x9a, 0x3a, 0x62, 0x92, 0x80, 0xe6, 0x86,
        0xcf, 0x0c, 0x3f, 0x5d, 0x5a, 0x86, 0xaf, 0xf3, 0xca, 0x12, 0x02, 0x0c, 0x92, 0x3a, 0xdc,
        0x6c, 0x92,
    ],
    [
        0x37, 0xa8, 0xee, 0xc1, 0xce, 0x19, 0x68, 0x7d, 0x13, 0x2f, 0xe2, 0x90, 0x51, 0xdc, 0xa6,
        0x29, 0xd1, 0x64, 0xe2, 0xc4, 0x95, 0x8b, 0xa1, 0x41, 0xd5, 0xf4, 0x13, 0x3a, 0x33, 0xf0,
        0x68, 0x8f,
    ],
    [
        0x62, 0xfd, 0x11, 0x15, 0xcc, 0x66, 0x90, 0x6e, 0x3f, 0xde, 0x57, 0x46, 0x24, 0x01, 0x27,
        0xe4, 0x6e, 0xbf, 0x3d, 0xba, 0xe3, 0x48, 0x1a, 0x88, 0xf4, 0x98, 0x4f, 0x16, 0xd6, 0x73,
        0x00, 0xe2,
    ],
];

/// Deployment boundary included in protected application partitions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ApplicationEnvironment {
    /// Local or explicitly enabled developer environment.
    Development,
    /// Pre-production environment with independent credentials.
    Staging,
    /// Production environment.
    Production,
}

impl ApplicationEnvironment {
    const fn label(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }
}

/// Validated name and purpose of one package-declared protected value.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProtectedSecretKey {
    name: String,
    purpose: String,
}

impl ProtectedSecretKey {
    /// Validate package-derived secret metadata.
    ///
    /// # Errors
    ///
    /// Returns a safe request error for malformed names or purposes.
    pub fn new(
        name: impl Into<String>,
        purpose: impl Into<String>,
    ) -> Result<Self, ProtectedSecretError> {
        let name = name.into();
        let purpose = purpose.into();
        if !valid_secret_name(&name) || !valid_safe_text(&purpose, 256) {
            return Err(ProtectedSecretError::request_invalid());
        }
        Ok(Self { name, purpose })
    }

    /// Package-declared stable name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Package-declared safe human-readable purpose.
    #[must_use]
    pub fn purpose(&self) -> &str {
        &self.purpose
    }
}

/// One captured value that cannot be cloned, formatted, or serialized.
pub struct SecretInput(Zeroizing<Vec<u8>>);

impl SecretInput {
    /// Move a newly captured value into protected memory.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, or known default credentials.
    pub fn new(value: Vec<u8>) -> Result<Self, ProtectedSecretError> {
        if value.is_empty() || value.len() > MAX_SECRET_BYTES {
            return Err(ProtectedSecretError::request_invalid());
        }
        if is_known_default(&value) {
            return Err(ProtectedSecretError::credential_rejected());
        }
        Ok(Self(Zeroizing::new(value)))
    }
}

impl fmt::Debug for SecretInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretInput(REDACTED)")
    }
}

/// Opaque credential-facility lookup key.
///
/// Implementations may compare and clone this key, but cannot construct a locator for another
/// application partition or inspect its derivation inputs.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct CredentialLocator {
    account: String,
}

impl fmt::Debug for CredentialLocator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CredentialLocator(REDACTED)")
    }
}

/// Zeroizing, non-formatable byte buffer returned by credential backend implementations.
pub struct CredentialBytes(Zeroizing<Vec<u8>>);

impl CredentialBytes {
    /// Move bytes returned by a credential facility into a redacted zeroizing wrapper.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(Zeroizing::new(bytes))
    }
}

impl fmt::Debug for CredentialBytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CredentialBytes(REDACTED)")
    }
}

/// Safe failure from an operating-system credential facility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialBackendError {
    /// No credential exists at the exact opaque locator.
    NotFound,
    /// The credential facility is unavailable or returned malformed data.
    Unavailable,
}

impl fmt::Display for CredentialBackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotFound => "credential not found",
            Self::Unavailable => "credential facility unavailable",
        })
    }
}

impl Error for CredentialBackendError {}

/// Host credential-facility abstraction.
///
/// Production uses [`OsCredentialBackend`]. Tests supply deterministic implementations. Locators
/// are derived only by [`ProtectedSecretStore`], and backend instances are never exposed through
/// guest handles.
pub trait CredentialBackend: Send + Sync {
    /// Atomically replace the bytes at one opaque locator.
    ///
    /// # Errors
    ///
    /// Returns a closed, value-free backend error.
    fn set_secret(
        &self,
        locator: &CredentialLocator,
        secret: &[u8],
    ) -> Result<(), CredentialBackendError>;

    /// Load bytes from one exact opaque locator into zeroizing memory.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialBackendError::NotFound`] for an absent credential and a single safe
    /// unavailable error for all platform failures.
    fn get_secret(
        &self,
        locator: &CredentialLocator,
    ) -> Result<CredentialBytes, CredentialBackendError>;

    /// Delete one exact opaque credential.
    ///
    /// # Errors
    ///
    /// Absence and platform failure retain their closed backend codes.
    fn delete_secret(&self, locator: &CredentialLocator) -> Result<(), CredentialBackendError>;
}

/// Platform credential backend shipped by Studio.
///
/// The keyring adapter selects macOS Keychain Services, Windows Credential Manager, or the
/// freedesktop Secret Service on other Unix desktops. Mobile targets have no shipped backend.
#[derive(Clone, Copy, Debug, Default)]
pub struct OsCredentialBackend;

impl OsCredentialBackend {
    /// Whether the platform adapter initialized successfully in this process.
    #[must_use]
    pub fn is_available() -> bool {
        keyring::Entry::store_status().is_ok()
    }

    /// Name of the credential facility shipped for this compilation target.
    #[must_use]
    pub const fn shipped_facility() -> Option<&'static str> {
        #[cfg(target_os = "macos")]
        {
            Some("macOS Keychain Services")
        }
        #[cfg(target_os = "windows")]
        {
            Some("Windows Credential Manager")
        }
        #[cfg(all(
            unix,
            not(any(target_os = "macos", target_os = "ios", target_os = "android"))
        ))]
        {
            Some("freedesktop Secret Service")
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            all(
                unix,
                not(any(target_os = "macos", target_os = "ios", target_os = "android"))
            )
        )))]
        {
            None
        }
    }

    fn entry(locator: &CredentialLocator) -> Result<keyring::Entry, CredentialBackendError> {
        keyring::Entry::new(CREDENTIAL_SERVICE, &locator.account).map_err(map_keyring_error)
    }
}

impl CredentialBackend for OsCredentialBackend {
    fn set_secret(
        &self,
        locator: &CredentialLocator,
        secret: &[u8],
    ) -> Result<(), CredentialBackendError> {
        Self::entry(locator)?
            .set_secret(secret)
            .map_err(map_keyring_error)
    }

    fn get_secret(
        &self,
        locator: &CredentialLocator,
    ) -> Result<CredentialBytes, CredentialBackendError> {
        Self::entry(locator)?
            .get_secret()
            .map(CredentialBytes::new)
            .map_err(map_keyring_error)
    }

    fn delete_secret(&self, locator: &CredentialLocator) -> Result<(), CredentialBackendError> {
        Self::entry(locator)?
            .delete_credential()
            .map_err(map_keyring_error)
    }
}

fn map_keyring_error(error: keyring::Error) -> CredentialBackendError {
    match error {
        keyring::Error::NoEntry => CredentialBackendError::NotFound,
        keyring::Error::BadEncoding(mut bytes) | keyring::Error::BadDataFormat(mut bytes, _) => {
            bytes.zeroize();
            CredentialBackendError::Unavailable
        }
        _ => CredentialBackendError::Unavailable,
    }
}

/// Guest-visible lifecycle state with no value-bearing variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedSecretState {
    /// No value has been supplied in this application/environment partition.
    Missing,
    /// A value is present in the protected credential facility.
    Configured,
    /// A previously configured value has been explicitly revoked.
    Revoked,
}

/// Guest-safe protected configuration metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedSecretStatus {
    key: ProtectedSecretKey,
    state: ProtectedSecretState,
    revision: Option<u64>,
}

impl ProtectedSecretStatus {
    /// Declared name and purpose.
    #[must_use]
    pub const fn key(&self) -> &ProtectedSecretKey {
        &self.key
    }

    /// Current lifecycle state.
    #[must_use]
    pub const fn state(&self) -> ProtectedSecretState {
        self.state
    }

    /// Monotonic partition-local rotation revision, absent while missing.
    #[must_use]
    pub const fn revision(&self) -> Option<u64> {
        self.revision
    }
}

/// Stable, value-free protected-store failure family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedSecretErrorCode {
    /// Package-derived metadata or environment authority was invalid.
    RequestInvalid,
    /// A known default credential was rejected.
    CredentialRejected,
    /// A value was missing, revoked, or otherwise unavailable to a broker.
    SecretUnavailable,
    /// The operating-system credential facility was unavailable or corrupt.
    BackendUnavailable,
    /// The broker declined injection without returning provider context.
    InjectionRejected,
}

/// Safe protected-store error without raw provider or credential context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedSecretError {
    code: ProtectedSecretErrorCode,
}

impl ProtectedSecretError {
    const fn request_invalid() -> Self {
        Self {
            code: ProtectedSecretErrorCode::RequestInvalid,
        }
    }

    const fn credential_rejected() -> Self {
        Self {
            code: ProtectedSecretErrorCode::CredentialRejected,
        }
    }

    const fn secret_unavailable() -> Self {
        Self {
            code: ProtectedSecretErrorCode::SecretUnavailable,
        }
    }

    const fn backend_unavailable() -> Self {
        Self {
            code: ProtectedSecretErrorCode::BackendUnavailable,
        }
    }

    const fn injection_rejected() -> Self {
        Self {
            code: ProtectedSecretErrorCode::InjectionRejected,
        }
    }

    /// Stable code suitable for guest action results and diagnostics.
    #[must_use]
    pub const fn code(self) -> ProtectedSecretErrorCode {
        self.code
    }
}

impl fmt::Display for ProtectedSecretError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            ProtectedSecretErrorCode::RequestInvalid => "protected secret request invalid",
            ProtectedSecretErrorCode::CredentialRejected => "credential rejected",
            ProtectedSecretErrorCode::SecretUnavailable => "protected secret unavailable",
            ProtectedSecretErrorCode::BackendUnavailable => "credential facility unavailable",
            ProtectedSecretErrorCode::InjectionRejected => "credential injection rejected",
        })
    }
}

impl Error for ProtectedSecretError {}

/// Single host-owned protected store over one operating-system credential facility.
pub struct ProtectedSecretStore<B> {
    backend: B,
}

impl<B> ProtectedSecretStore<B> {
    /// Wrap a production or deterministic credential backend.
    #[must_use]
    pub const fn new(backend: B) -> Self {
        Self { backend }
    }
}

impl<B: CredentialBackend> ProtectedSecretStore<B> {
    /// Bind an application/environment scope from an already verified principal.
    ///
    /// Development principals are deliberately rejected outside the development environment.
    ///
    /// # Errors
    ///
    /// Returns a value-free request error for a development principal attempting to address
    /// staging or production storage.
    pub fn for_application(
        &self,
        principal: &PluginPrincipal,
        environment: ApplicationEnvironment,
    ) -> Result<ApplicationSecretStore<'_, B>, ProtectedSecretError> {
        if principal.trust_mode() == TrustMode::Development
            && environment != ApplicationEnvironment::Development
        {
            return Err(ProtectedSecretError::request_invalid());
        }
        Ok(ApplicationSecretStore {
            store: self,
            partition: ApplicationPartition::derive(principal, environment),
        })
    }
}

impl<B> fmt::Debug for ProtectedSecretStore<B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProtectedSecretStore(REDACTED)")
    }
}

/// Host configuration surface bound to one verified application/environment partition.
pub struct ApplicationSecretStore<'a, B> {
    store: &'a ProtectedSecretStore<B>,
    partition: ApplicationPartition,
}

impl<B: CredentialBackend> ApplicationSecretStore<'_, B> {
    /// Capture or replace a value supplied through a host-owned entry surface.
    ///
    /// # Errors
    ///
    /// Returns a closed backend or input error without formatting the value.
    pub fn configure(
        &self,
        key: &ProtectedSecretKey,
        input: SecretInput,
    ) -> Result<ProtectedSecretStatus, ProtectedSecretError> {
        let revision = self.next_revision(key)?;
        self.store_configured(key, revision, input)?;
        Ok(status(
            key,
            ProtectedSecretState::Configured,
            Some(revision),
        ))
    }

    /// Replace an existing configured value without changing package contents.
    ///
    /// # Errors
    ///
    /// Missing and revoked values share one unavailable error. Backend and input failures remain
    /// closed and value-free.
    pub fn rotate(
        &self,
        key: &ProtectedSecretKey,
        input: SecretInput,
    ) -> Result<ProtectedSecretStatus, ProtectedSecretError> {
        let revision = match self.load(key)? {
            StoredSecret::Configured { revision, .. } => revision.saturating_add(1),
            StoredSecret::Missing | StoredSecret::Revoked { .. } => {
                return Err(ProtectedSecretError::secret_unavailable());
            }
        };
        self.store_configured(key, revision, input)?;
        Ok(status(
            key,
            ProtectedSecretState::Configured,
            Some(revision),
        ))
    }

    /// Replace any value with a persistent revoked marker.
    ///
    /// # Errors
    ///
    /// Returns a closed backend failure if the marker cannot be written.
    pub fn revoke(
        &self,
        key: &ProtectedSecretKey,
    ) -> Result<ProtectedSecretStatus, ProtectedSecretError> {
        let revision = self.next_revision(key)?;
        let mut encoded = Zeroizing::new(Vec::with_capacity(REVOKED_RECORD.len() + 8));
        encoded.extend_from_slice(REVOKED_RECORD);
        encoded.extend_from_slice(&revision.to_be_bytes());
        self.backend_set(key, encoded.as_slice())?;
        Ok(status(key, ProtectedSecretState::Revoked, Some(revision)))
    }

    /// Remove protected state entirely, leaving the declaration missing.
    ///
    /// # Errors
    ///
    /// Returns a closed backend failure. Deleting an already missing value succeeds.
    pub fn purge(&self, key: &ProtectedSecretKey) -> Result<(), ProtectedSecretError> {
        let locator = self.partition.locator(key);
        match self.store.backend.delete_secret(&locator) {
            Ok(()) | Err(CredentialBackendError::NotFound) => Ok(()),
            Err(CredentialBackendError::Unavailable) => {
                Err(ProtectedSecretError::backend_unavailable())
            }
        }
    }

    /// Query safe status from the host configuration surface.
    ///
    /// # Errors
    ///
    /// Returns a closed backend failure if the credential facility is unavailable.
    pub fn status(
        &self,
        key: &ProtectedSecretKey,
    ) -> Result<ProtectedSecretStatus, ProtectedSecretError> {
        match self.load(key)? {
            StoredSecret::Missing => Ok(status(key, ProtectedSecretState::Missing, None)),
            StoredSecret::Configured { revision, .. } => Ok(status(
                key,
                ProtectedSecretState::Configured,
                Some(revision),
            )),
            StoredSecret::Revoked { revision } => {
                Ok(status(key, ProtectedSecretState::Revoked, Some(revision)))
            }
        }
    }

    /// Create the only guest-facing handle, restricted to signed declarations.
    #[must_use]
    pub fn guest_status_handle(
        &self,
        declarations: impl IntoIterator<Item = ProtectedSecretKey>,
    ) -> GuestSecretStatusHandle<'_, B> {
        GuestSecretStatusHandle {
            scope: ApplicationSecretStore {
                store: self.store,
                partition: self.partition.clone(),
            },
            declarations: declarations.into_iter().collect(),
        }
    }

    /// Create a separate host-only send-time injection handle.
    #[must_use]
    pub fn broker_injection_handle(
        &self,
        declarations: impl IntoIterator<Item = ProtectedSecretKey>,
    ) -> BrokerSecretInjectionHandle<'_, B> {
        BrokerSecretInjectionHandle {
            scope: ApplicationSecretStore {
                store: self.store,
                partition: self.partition.clone(),
            },
            declarations: declarations.into_iter().collect(),
        }
    }

    fn next_revision(&self, key: &ProtectedSecretKey) -> Result<u64, ProtectedSecretError> {
        Ok(match self.load(key)? {
            StoredSecret::Missing => 1,
            StoredSecret::Configured { revision, .. } | StoredSecret::Revoked { revision } => {
                revision.saturating_add(1)
            }
        })
    }

    fn store_configured(
        &self,
        key: &ProtectedSecretKey,
        revision: u64,
        input: SecretInput,
    ) -> Result<(), ProtectedSecretError> {
        let SecretInput(input) = input;
        let mut encoded = Zeroizing::new(Vec::with_capacity(
            CONFIGURED_RECORD.len() + 8 + input.len(),
        ));
        encoded.extend_from_slice(CONFIGURED_RECORD);
        encoded.extend_from_slice(&revision.to_be_bytes());
        encoded.extend_from_slice(input.as_slice());
        self.backend_set(key, encoded.as_slice())
    }

    fn backend_set(
        &self,
        key: &ProtectedSecretKey,
        value: &[u8],
    ) -> Result<(), ProtectedSecretError> {
        self.store
            .backend
            .set_secret(&self.partition.locator(key), value)
            .map_err(|_| ProtectedSecretError::backend_unavailable())
    }

    fn load(&self, key: &ProtectedSecretKey) -> Result<StoredSecret, ProtectedSecretError> {
        let encoded = match self.store.backend.get_secret(&self.partition.locator(key)) {
            Ok(encoded) => encoded,
            Err(CredentialBackendError::NotFound) => return Ok(StoredSecret::Missing),
            Err(CredentialBackendError::Unavailable) => {
                return Err(ProtectedSecretError::backend_unavailable());
            }
        };
        decode_record(encoded)
    }
}

impl<B> fmt::Debug for ApplicationSecretStore<'_, B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApplicationSecretStore(REDACTED)")
    }
}

mod sealed {
    pub trait GuestStatus {}
    pub trait BrokerInjection {}
}

/// Status-only interface passed to guests.
///
/// This sealed trait contains no secret material type, broker callback, backend accessor, or
/// conversion to a host handle.
pub trait GuestSecretStatusApi: sealed::GuestStatus {
    /// Query one signed declaration in the current application/environment partition.
    ///
    /// # Errors
    ///
    /// Undeclared names and backend failures return safe codes.
    fn secret_status(
        &self,
        key: &ProtectedSecretKey,
    ) -> Result<ProtectedSecretStatus, ProtectedSecretError>;
}

/// Concrete status-only guest capability.
pub struct GuestSecretStatusHandle<'a, B> {
    scope: ApplicationSecretStore<'a, B>,
    declarations: HashSet<ProtectedSecretKey>,
}

impl<B> sealed::GuestStatus for GuestSecretStatusHandle<'_, B> {}

impl<B: CredentialBackend> GuestSecretStatusApi for GuestSecretStatusHandle<'_, B> {
    fn secret_status(
        &self,
        key: &ProtectedSecretKey,
    ) -> Result<ProtectedSecretStatus, ProtectedSecretError> {
        if !self.declarations.contains(key) {
            return Err(ProtectedSecretError::request_invalid());
        }
        self.scope.status(key)
    }
}

impl<B> fmt::Debug for GuestSecretStatusHandle<'_, B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GuestSecretStatusHandle")
            .field("declarations", &self.declarations.len())
            .finish_non_exhaustive()
    }
}

/// Safe broker callback rejection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BrokerCredentialError;

impl fmt::Display for BrokerCredentialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("broker credential injection rejected")
    }
}

impl Error for BrokerCredentialError {}

/// Host network-broker sink invoked only at request send time.
pub trait BrokerCredentialSink {
    /// Attach borrowed credential bytes to the bounded host request under construction.
    ///
    /// Implementations must neither retain nor format the borrowed bytes.
    ///
    /// # Errors
    ///
    /// Returns a value-free rejection if the bounded request cannot accept the credential.
    fn inject(&mut self, secret: &[u8]) -> Result<(), BrokerCredentialError>;
}

/// Host-only broker hook, intentionally distinct from [`GuestSecretStatusApi`].
pub trait BrokerSecretInjector: sealed::BrokerInjection {
    /// Load and inject a declared configured value during a broker's send operation.
    ///
    /// # Errors
    ///
    /// Missing, revoked, default, backend, and sink failures return only stable safe codes.
    fn inject_at_send_time(
        &self,
        key: &ProtectedSecretKey,
        sink: &mut dyn BrokerCredentialSink,
    ) -> Result<(), ProtectedSecretError>;
}

/// Concrete app-scoped host broker capability.
pub struct BrokerSecretInjectionHandle<'a, B> {
    scope: ApplicationSecretStore<'a, B>,
    declarations: HashSet<ProtectedSecretKey>,
}

impl<B> sealed::BrokerInjection for BrokerSecretInjectionHandle<'_, B> {}

impl<B: CredentialBackend> BrokerSecretInjector for BrokerSecretInjectionHandle<'_, B> {
    fn inject_at_send_time(
        &self,
        key: &ProtectedSecretKey,
        sink: &mut dyn BrokerCredentialSink,
    ) -> Result<(), ProtectedSecretError> {
        if !self.declarations.contains(key) {
            return Err(ProtectedSecretError::request_invalid());
        }
        let StoredSecret::Configured { secret, .. } = self.scope.load(key)? else {
            return Err(ProtectedSecretError::secret_unavailable());
        };
        if is_known_default(secret.as_slice()) {
            return Err(ProtectedSecretError::credential_rejected());
        }
        sink.inject(secret.as_slice())
            .map_err(|_| ProtectedSecretError::injection_rejected())
    }
}

impl<B> fmt::Debug for BrokerSecretInjectionHandle<'_, B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrokerSecretInjectionHandle")
            .field("declarations", &self.declarations.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct ApplicationPartition {
    digest: [u8; 32],
    marker: PhantomData<PluginPrincipal>,
}

impl ApplicationPartition {
    fn derive(principal: &PluginPrincipal, environment: ApplicationEnvironment) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(PARTITION_DOMAIN);
        hash_field(&mut hasher, principal.publisher_id().as_bytes());
        hash_field(&mut hasher, principal.plugin_id().as_bytes());
        hash_field(&mut hasher, environment.label().as_bytes());
        Self {
            digest: hasher.finalize().into(),
            marker: PhantomData,
        }
    }

    fn locator(&self, key: &ProtectedSecretKey) -> CredentialLocator {
        let mut hasher = Sha256::new();
        hasher.update(CREDENTIAL_DOMAIN);
        hash_field(&mut hasher, &self.digest);
        hash_field(&mut hasher, key.name.as_bytes());
        hash_field(&mut hasher, key.purpose.as_bytes());
        CredentialLocator {
            account: encode_hex(&hasher.finalize()),
        }
    }
}

enum StoredSecret {
    Missing,
    Configured {
        revision: u64,
        secret: Zeroizing<Vec<u8>>,
    },
    Revoked {
        revision: u64,
    },
}

fn decode_record(encoded: CredentialBytes) -> Result<StoredSecret, ProtectedSecretError> {
    let CredentialBytes(mut encoded) = encoded;
    if encoded.starts_with(CONFIGURED_RECORD) {
        let header_length = CONFIGURED_RECORD.len() + 8;
        if encoded.len() <= header_length || encoded.len() > header_length + MAX_SECRET_BYTES {
            return Err(ProtectedSecretError::backend_unavailable());
        }
        let revision = decode_revision(&encoded[CONFIGURED_RECORD.len()..header_length])?;
        let secret = Zeroizing::new(encoded.split_off(header_length));
        return Ok(StoredSecret::Configured { revision, secret });
    }
    if encoded.starts_with(REVOKED_RECORD) && encoded.len() == REVOKED_RECORD.len() + 8 {
        let revision = decode_revision(&encoded[REVOKED_RECORD.len()..])?;
        return Ok(StoredSecret::Revoked { revision });
    }
    Err(ProtectedSecretError::backend_unavailable())
}

fn decode_revision(bytes: &[u8]) -> Result<u64, ProtectedSecretError> {
    let bytes: [u8; 8] = bytes
        .try_into()
        .map_err(|_| ProtectedSecretError::backend_unavailable())?;
    let revision = u64::from_be_bytes(bytes);
    if revision == 0 {
        return Err(ProtectedSecretError::backend_unavailable());
    }
    Ok(revision)
}

fn status(
    key: &ProtectedSecretKey,
    state: ProtectedSecretState,
    revision: Option<u64>,
) -> ProtectedSecretStatus {
    ProtectedSecretStatus {
        key: key.clone(),
        state,
        revision,
    }
}

fn is_known_default(secret: &[u8]) -> bool {
    let digest: [u8; 32] = Sha256::digest(secret).into();
    KNOWN_DEFAULT_DIGESTS.contains(&digest)
}

fn valid_secret_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.starts_with(|character: char| character.is_ascii_lowercase())
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        })
}

fn valid_safe_text(value: &str, maximum_bytes: usize) -> bool {
    !value.is_empty() && value.len() <= maximum_bytes && !value.chars().any(char::is_control)
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing into a String cannot fail");
            output
        },
    )
}
