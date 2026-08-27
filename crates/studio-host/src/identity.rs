//! Host-owned local identities and offline session authentication.
//!
//! This module is deliberately independent of a UI toolkit and networking. The
//! local store contains only identity metadata and salted password verifiers;
//! remembered session tokens are handed to the protected credential store and
//! never enter the local-store catalog.

use std::{
    collections::HashMap,
    fmt::Write as _,
    sync::{Arc, Mutex},
};

use getrandom::fill;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use studio_security::{
    MemorySessionCredentialStore, PasswordError, PasswordVerifier, SessionCredentialError,
    SessionCredentialStore,
};
use thiserror::Error;
use zeroize::Zeroize;

use crate::{LocalStore, LocalStoreError, StoreBatch, StoreBatchEntry};

const IDENTITY_CATALOG_BATCH: &str = "studio-identity-catalog-v1";
const CATALOG_SCHEMA_VERSION: u16 = 1;
const ID_BYTES: usize = 16;
const SESSION_TOKEN_BYTES: usize = 32;
const MAX_DISPLAY_NAME_BYTES: usize = 256;
const MAX_EMAIL_BYTES: usize = 320;
const MAX_AVATAR_BYTES: usize = 4096;
const MAX_PROJECT_ID_BYTES: usize = 128;

/// The local account kind shown by the identity chooser.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityKind {
    /// A device-local account that works without network access.
    Local,
}

/// Authentication state visible to account-chooser callers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityState {
    /// The identity can be selected for password authentication.
    Available,
    /// A failed password attempt has locked the identity until its password is
    /// supplied through [`IdentityService::unlock`].
    Locked,
}

/// Public identity metadata safe for the app-facing chooser.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IdentitySummary {
    /// Stable host-generated identity identifier.
    pub identity_id: String,
    /// Distinguishes local identities from future account providers.
    pub kind: IdentityKind,
    /// Name shown in the chooser.
    pub display_name: String,
    /// Optional email shown in account details.
    pub email: Option<String>,
    /// Optional local avatar reference. Image bytes are not stored here.
    pub avatar: Option<String>,
    /// Current password-gate state.
    pub state: IdentityState,
}

/// State of one remembered session in the chooser.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// The credential is available and may be resumed.
    Available,
    /// The session is currently active in this host process.
    Active,
    /// The credential was explicitly revoked and cannot be resumed.
    Revoked,
}

/// Public remembered-session metadata; token bytes never cross this type.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionSummary {
    /// Stable host-generated session identifier.
    pub session_id: String,
    /// Identity which owns the session.
    pub identity_id: String,
    /// Whether the session is protected for restart/resume.
    pub remembered: bool,
    /// Current chooser-visible state.
    pub state: SessionState,
}

/// Snapshot consumed by the app-facing welcome and account chooser surfaces.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IdentitySnapshot {
    /// Whether the product welcome has been dismissed on this device.
    pub welcome_dismissed: bool,
    /// All identities, without verifiers or secrets.
    pub identities: Vec<IdentitySummary>,
    /// Remembered sessions, including revoked entries for explicit state.
    pub sessions: Vec<SessionSummary>,
}

/// Opaque proof of one authenticated identity session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentitySession {
    session_id: String,
    identity_id: String,
}

impl IdentitySession {
    /// Stable session identifier suitable for diagnostics and revocation.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Identity authorized by this session.
    #[must_use]
    pub fn identity_id(&self) -> &str {
        &self.identity_id
    }
}

/// Input for creating a new local identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateIdentityRequest {
    /// Name shown in the chooser.
    pub display_name: String,
    /// Optional email metadata.
    pub email: Option<String>,
    /// Optional avatar reference or path metadata.
    pub avatar: Option<String>,
    /// Password bytes are consumed only while deriving the verifier.
    pub password: Vec<u8>,
}

/// Stable error categories for identity operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityErrorCode {
    /// A bounded input or persisted catalog shape is invalid.
    InvalidInput,
    /// The requested identity or session does not exist.
    NotFound,
    /// Authentication failed; the identity remains locked after the failure.
    WrongPassword,
    /// Authentication was attempted while the identity was locked.
    Locked,
    /// The protected credential facility could not complete an operation.
    CredentialUnavailable,
    /// The local store could not complete an operation.
    StoreUnavailable,
    /// The catalog cannot be trusted or decoded.
    CatalogCorrupt,
    /// The host could not obtain a cryptographically secure random value.
    EntropyUnavailable,
}

/// Failure returned by the host identity service.
#[derive(Debug, Error)]
pub enum IdentityError {
    /// Input did not satisfy the host's bounded identity contract.
    #[error("identity input invalid")]
    InvalidInput,
    /// An identity or session was not found.
    #[error("identity or session not found")]
    NotFound,
    /// Password verification failed and the identity is now locked.
    #[error("password verification failed; identity is locked")]
    WrongPassword,
    /// A locked identity must pass the explicit unlock gate.
    #[error("identity is locked")]
    Locked,
    /// Protected session credentials could not be accessed.
    #[error(transparent)]
    Credential(#[from] SessionCredentialError),
    /// The local identity catalog could not be persisted or read.
    #[error(transparent)]
    Store(#[from] LocalStoreError),
    /// The persisted identity catalog failed closed validation.
    #[error("identity catalog is corrupt")]
    CatalogCorrupt,
    /// Secure random generation was unavailable.
    #[error("secure entropy unavailable")]
    EntropyUnavailable,
}

impl IdentityError {
    /// Stable category suitable for app diagnostics.
    #[must_use]
    pub const fn code(&self) -> IdentityErrorCode {
        match self {
            Self::InvalidInput => IdentityErrorCode::InvalidInput,
            Self::NotFound => IdentityErrorCode::NotFound,
            Self::WrongPassword => IdentityErrorCode::WrongPassword,
            Self::Locked => IdentityErrorCode::Locked,
            Self::Credential(_) => IdentityErrorCode::CredentialUnavailable,
            Self::Store(_) => IdentityErrorCode::StoreUnavailable,
            Self::CatalogCorrupt => IdentityErrorCode::CatalogCorrupt,
            Self::EntropyUnavailable => IdentityErrorCode::EntropyUnavailable,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedIdentity {
    identity_id: String,
    kind: IdentityKind,
    display_name: String,
    email: Option<String>,
    avatar: Option<String>,
    verifier: PasswordVerifier,
    locked: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedSession {
    session_id: String,
    identity_id: String,
    remembered: bool,
    revoked: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedCatalog {
    schema_version: u16,
    welcome_dismissed: bool,
    identities: Vec<PersistedIdentity>,
    sessions: Vec<PersistedSession>,
}

impl Default for PersistedCatalog {
    fn default() -> Self {
        Self {
            schema_version: CATALOG_SCHEMA_VERSION,
            welcome_dismissed: false,
            identities: Vec::new(),
            sessions: Vec::new(),
        }
    }
}

struct ActiveSession {
    identity_id: String,
    remembered: bool,
    token: [u8; SESSION_TOKEN_BYTES],
}

impl Drop for ActiveSession {
    fn drop(&mut self) {
        self.token.zeroize();
    }
}

/// Offline identity service backed by a host [`LocalStore`].
pub struct IdentityService<S, C = MemorySessionCredentialStore>
where
    S: LocalStore,
    C: SessionCredentialStore,
{
    store: Arc<S>,
    credentials: C,
    active_sessions: Mutex<HashMap<String, ActiveSession>>,
}

impl<S> IdentityService<S, MemorySessionCredentialStore>
where
    S: LocalStore,
{
    /// Construct a service using deterministic in-memory protected credentials.
    ///
    /// This constructor is intended for tests and disposable hosts. Production
    /// callers should use [`Self::with_credentials`] with
    /// [`studio_security::OsSessionCredentialStore`].
    pub fn new(store: Arc<S>) -> Self {
        Self::with_credentials(store, MemorySessionCredentialStore::default())
    }
}

impl<S, C> IdentityService<S, C>
where
    S: LocalStore,
    C: SessionCredentialStore,
{
    /// Construct a service with an explicitly selected protected credential
    /// implementation.
    #[must_use]
    pub fn with_credentials(store: Arc<S>, credentials: C) -> Self {
        Self {
            store,
            credentials,
            active_sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Read the persisted welcome, identities, and session metadata.
    pub async fn snapshot(&self) -> Result<IdentitySnapshot, IdentityError> {
        let catalog = self.load_catalog().await?;
        Ok(self.snapshot_from_catalog(&catalog))
    }

    /// Persist that the product welcome has been dismissed.
    pub async fn dismiss_welcome(&self) -> Result<(), IdentityError> {
        let mut catalog = self.load_catalog().await?;
        catalog.welcome_dismissed = true;
        self.save_catalog(&catalog).await
    }

    /// Persist that the product welcome should be shown again.
    pub async fn revisit_welcome(&self) -> Result<(), IdentityError> {
        let mut catalog = self.load_catalog().await?;
        catalog.welcome_dismissed = false;
        self.save_catalog(&catalog).await
    }

    /// Create and persist one local identity without retaining its password.
    pub async fn create_identity(
        &self,
        request: CreateIdentityRequest,
    ) -> Result<IdentitySummary, IdentityError> {
        validate_identity_input(&request)?;
        let verifier = PasswordVerifier::derive(&request.password).map_err(map_password_error)?;
        let mut catalog = self.load_catalog().await?;
        let identity_id = random_id("local")?;
        let identity = PersistedIdentity {
            identity_id,
            kind: IdentityKind::Local,
            display_name: request.display_name,
            email: request.email,
            avatar: request.avatar,
            verifier,
            locked: false,
        };
        let summary = identity_summary(&identity);
        catalog.identities.push(identity);
        self.save_catalog(&catalog).await?;
        Ok(summary)
    }

    /// Sign in to an available local identity, optionally remembering the
    /// session in the protected credential facility.
    pub async fn sign_in(
        &self,
        identity_id: &str,
        password: &[u8],
        remember: bool,
    ) -> Result<IdentitySession, IdentityError> {
        self.authenticate(identity_id, password, remember, false).await
    }

    /// Unlock a locked identity with its password and create a new session.
    pub async fn unlock(
        &self,
        identity_id: &str,
        password: &[u8],
        remember: bool,
    ) -> Result<IdentitySession, IdentityError> {
        self.authenticate(identity_id, password, remember, true).await
    }

    /// Resume a remembered session after a process restart.
    pub async fn resume(&self, session_id: &str) -> Result<IdentitySession, IdentityError> {
        if session_id.is_empty() || session_id.len() > 128 {
            return Err(IdentityError::InvalidInput);
        }
        let catalog = self.load_catalog().await?;
        let session = catalog
            .sessions
            .iter()
            .find(|session| session.session_id == session_id)
            .ok_or(IdentityError::NotFound)?;
        if !session.remembered || session.revoked {
            return Err(if session.revoked {
                IdentityError::Locked
            } else {
                IdentityError::NotFound
            });
        }
        let Some(token) = self.credentials.load(&session.identity_id, session_id)? else {
            return Err(IdentityError::NotFound);
        };
        if token.len() != SESSION_TOKEN_BYTES {
            return Err(IdentityError::CatalogCorrupt);
        }
        let active = IdentitySession {
            session_id: session_id.to_owned(),
            identity_id: session.identity_id.clone(),
        };
        self.active_sessions
            .lock()
            .expect("identity session lock is not poisoned")
            .insert(
                active.session_id.clone(),
                ActiveSession {
                    identity_id: active.identity_id.clone(),
                    remembered: true,
                    token,
                },
            );
        Ok(active)
    }

    /// End this process's use of a session while leaving a remembered
    /// credential available for a later explicit resume.
    pub fn sign_out(&self, session: &IdentitySession) -> Result<(), IdentityError> {
        self.require_active(session)?;
        self.active_sessions
            .lock()
            .expect("identity session lock is not poisoned")
            .remove(session.session_id());
        Ok(())
    }

    /// Revoke a remembered session, including one active in another view.
    pub async fn revoke_session(&self, session_id: &str) -> Result<(), IdentityError> {
        if session_id.is_empty() || session_id.len() > 128 {
            return Err(IdentityError::InvalidInput);
        }
        let mut catalog = self.load_catalog().await?;
        let session = catalog
            .sessions
            .iter_mut()
            .find(|session| session.session_id == session_id)
            .ok_or(IdentityError::NotFound)?;
        let identity_id = session.identity_id.clone();
        session.revoked = true;
        self.save_catalog(&catalog).await?;
        self.credentials.revoke(&identity_id, session_id)?;
        self.active_sessions
            .lock()
            .expect("identity session lock is not poisoned")
            .remove(session_id);
        Ok(())
    }

    /// List public remembered-session metadata for the account chooser.
    pub async fn sessions(&self) -> Result<Vec<SessionSummary>, IdentityError> {
        Ok(self.snapshot().await?.sessions)
    }

    /// Read one identity-scoped project batch through an authenticated session.
    pub async fn project_entries(
        &self,
        session: &IdentitySession,
        project_id: &str,
    ) -> Result<Vec<StoreBatchEntry>, IdentityError> {
        self.require_active(session)?;
        let batch_id = scoped_batch_id(session.identity_id(), project_id)?;
        Ok(self.store.batch_entries(&batch_id).await?)
    }

    /// Atomically write one identity-scoped project batch through an
    /// authenticated session.
    pub async fn write_project_batch(
        &self,
        session: &IdentitySession,
        project_id: &str,
        entries: impl IntoIterator<Item = StoreBatchEntry>,
    ) -> Result<(), IdentityError> {
        self.require_active(session)?;
        let batch = StoreBatch::new(scoped_batch_id(session.identity_id(), project_id)?, entries)?;
        self.store.write_batch(&batch).await?;
        Ok(())
    }

    fn require_active(&self, session: &IdentitySession) -> Result<(), IdentityError> {
        let active = self
            .active_sessions
            .lock()
            .expect("identity session lock is not poisoned");
        let Some(found) = active.get(session.session_id()) else {
            return Err(IdentityError::NotFound);
        };
        if found.identity_id != session.identity_id() {
            return Err(IdentityError::NotFound);
        }
        if found.remembered
            && !self
                .credentials
                .matches(&found.identity_id, session.session_id(), &found.token)?
        {
            return Err(IdentityError::NotFound);
        }
        Ok(())
    }

    fn invalidate_active_sessions(&self, identity_id: &str) {
        self.active_sessions
            .lock()
            .expect("identity session lock is not poisoned")
            .retain(|_, session| session.identity_id != identity_id);
    }

    async fn authenticate(
        &self,
        identity_id: &str,
        password: &[u8],
        remember: bool,
        unlocking: bool,
    ) -> Result<IdentitySession, IdentityError> {
        if identity_id.is_empty() || identity_id.len() > 128 {
            return Err(IdentityError::InvalidInput);
        }
        let mut catalog = self.load_catalog().await?;
        let identity_index = catalog
            .identities
            .iter()
            .position(|identity| identity.identity_id == identity_id)
            .ok_or(IdentityError::NotFound)?;
        if catalog.identities[identity_index].locked && !unlocking {
            return Err(IdentityError::Locked);
        }
        let password_valid = catalog.identities[identity_index].verifier.verify(password);
        if !password_valid {
            catalog.identities[identity_index].locked = true;
            let remembered_sessions = catalog
                .sessions
                .iter_mut()
                .filter(|session| session.identity_id == identity_id && !session.revoked)
                .map(|session| {
                    session.revoked = true;
                    session.session_id.clone()
                })
                .collect::<Vec<_>>();
            self.save_catalog(&catalog).await?;
            self.invalidate_active_sessions(identity_id);
            for session_id in remembered_sessions {
                self.credentials.revoke(identity_id, &session_id)?;
            }
            return Err(IdentityError::WrongPassword);
        }
        if unlocking {
            catalog.identities[identity_index].locked = false;
        }
        let session_id = random_id("session")?;
        let mut token = [0; SESSION_TOKEN_BYTES];
        fill(&mut token).map_err(|_| IdentityError::EntropyUnavailable)?;
        if remember {
            self.credentials.store(identity_id, &session_id, &token)?;
            catalog.sessions.push(PersistedSession {
                session_id: session_id.clone(),
                identity_id: identity_id.to_owned(),
                remembered: true,
                revoked: false,
            });
        }
        self.save_catalog(&catalog).await?;
        self.active_sessions
            .lock()
            .expect("identity session lock is not poisoned")
            .insert(
                session_id.clone(),
                ActiveSession {
                    identity_id: identity_id.to_owned(),
                    remembered: remember,
                    token,
                },
            );
        Ok(IdentitySession {
            session_id,
            identity_id: identity_id.to_owned(),
        })
    }

    async fn load_catalog(&self) -> Result<PersistedCatalog, IdentityError> {
        let entries = self.store.batch_entries(IDENTITY_CATALOG_BATCH).await?;
        if entries.is_empty() {
            return Ok(PersistedCatalog::default());
        }
        let [entry] = entries.as_slice() else {
            return Err(IdentityError::CatalogCorrupt);
        };
        if entry.ordinal != 0 {
            return Err(IdentityError::CatalogCorrupt);
        }
        let catalog: PersistedCatalog =
            serde_json::from_value(entry.payload.clone()).map_err(|_| IdentityError::CatalogCorrupt)?;
        validate_catalog(&catalog)?;
        Ok(catalog)
    }

    async fn save_catalog(&self, catalog: &PersistedCatalog) -> Result<(), IdentityError> {
        validate_catalog(catalog)?;
        let payload = serde_json::to_value(catalog).map_err(|_| IdentityError::CatalogCorrupt)?;
        let batch = StoreBatch::new(
            IDENTITY_CATALOG_BATCH,
            [StoreBatchEntry { ordinal: 0, payload }],
        )?;
        self.store.write_batch(&batch).await?;
        Ok(())
    }

    fn snapshot_from_catalog(&self, catalog: &PersistedCatalog) -> IdentitySnapshot {
        let active = self
            .active_sessions
            .lock()
            .expect("identity session lock is not poisoned");
        IdentitySnapshot {
            welcome_dismissed: catalog.welcome_dismissed,
            identities: catalog.identities.iter().map(identity_summary).collect(),
            sessions: catalog
                .sessions
                .iter()
                .map(|session| SessionSummary {
                    session_id: session.session_id.clone(),
                    identity_id: session.identity_id.clone(),
                    remembered: session.remembered,
                    state: if session.revoked {
                        SessionState::Revoked
                    } else if active.contains_key(&session.session_id) {
                        SessionState::Active
                    } else {
                        SessionState::Available
                    },
                })
                .collect(),
        }
    }
}

fn validate_identity_input(request: &CreateIdentityRequest) -> Result<(), IdentityError> {
    if !bounded_text(&request.display_name, 1, MAX_DISPLAY_NAME_BYTES)
        || !optional_bounded_text(request.email.as_deref(), MAX_EMAIL_BYTES)
        || !optional_bounded_text(request.avatar.as_deref(), MAX_AVATAR_BYTES)
    {
        return Err(IdentityError::InvalidInput);
    }
    Ok(())
}

fn optional_bounded_text(value: Option<&str>, max: usize) -> bool {
    value.is_none_or(|value| bounded_text(value, 1, max))
}

fn bounded_text(value: &str, min: usize, max: usize) -> bool {
    value.len() >= min && value.len() <= max && !value.chars().any(char::is_control)
}

fn validate_catalog(catalog: &PersistedCatalog) -> Result<(), IdentityError> {
    if catalog.schema_version != CATALOG_SCHEMA_VERSION {
        return Err(IdentityError::CatalogCorrupt);
    }
    let mut identities = std::collections::BTreeSet::new();
    for identity in &catalog.identities {
        if !identities.insert(&identity.identity_id)
            || identity.kind != IdentityKind::Local
            || !bounded_text(&identity.identity_id, 1, 128)
            || !bounded_text(&identity.display_name, 1, MAX_DISPLAY_NAME_BYTES)
            || !optional_bounded_text(identity.email.as_deref(), MAX_EMAIL_BYTES)
            || !optional_bounded_text(identity.avatar.as_deref(), MAX_AVATAR_BYTES)
        {
            return Err(IdentityError::CatalogCorrupt);
        }
    }
    let mut sessions = std::collections::BTreeSet::new();
    for session in &catalog.sessions {
        if !sessions.insert(&session.session_id)
            || session.session_id.is_empty()
            || !identities.contains(&session.identity_id)
            || !session.remembered
        {
            return Err(IdentityError::CatalogCorrupt);
        }
    }
    Ok(())
}

fn identity_summary(identity: &PersistedIdentity) -> IdentitySummary {
    IdentitySummary {
        identity_id: identity.identity_id.clone(),
        kind: identity.kind,
        display_name: identity.display_name.clone(),
        email: identity.email.clone(),
        avatar: identity.avatar.clone(),
        state: if identity.locked {
            IdentityState::Locked
        } else {
            IdentityState::Available
        },
    }
}

fn random_id(prefix: &str) -> Result<String, IdentityError> {
    let mut bytes = [0; ID_BYTES];
    fill(&mut bytes).map_err(|_| IdentityError::EntropyUnavailable)?;
    let mut id = String::with_capacity(prefix.len() + 1 + ID_BYTES * 2);
    id.push_str(prefix);
    id.push('-');
    for byte in bytes {
        write!(&mut id, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(id)
}

fn scoped_batch_id(identity_id: &str, project_id: &str) -> Result<String, IdentityError> {
    if identity_id.is_empty()
        || project_id.is_empty()
        || project_id.len() > MAX_PROJECT_ID_BYTES
        || project_id.chars().any(char::is_control)
    {
        return Err(IdentityError::InvalidInput);
    }
    let mut digest = Sha256::new();
    digest.update(b"studio.identity-project.v1");
    digest.update(identity_id.as_bytes());
    digest.update([0]);
    digest.update(project_id.as_bytes());
    let digest = digest.finalize();
    let mut id = String::from("identity-project-");
    for byte in digest {
        write!(&mut id, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(id)
}

fn map_password_error(error: PasswordError) -> IdentityError {
    match error.code() {
        studio_security::PasswordErrorCode::InvalidInput => IdentityError::InvalidInput,
        studio_security::PasswordErrorCode::EntropyUnavailable => {
            IdentityError::EntropyUnavailable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::future::Future;

    #[derive(Default)]
    struct MemoryStore {
        batches: Mutex<HashMap<String, Vec<StoreBatchEntry>>>,
    }

    impl LocalStore for MemoryStore {
        fn metadata(&self) -> impl Future<Output = Result<crate::StoreMetadata, LocalStoreError>> + Send {
            async { panic!("identity tests do not use metadata") }
        }

        fn write_batch(
            &self,
            batch: &StoreBatch,
        ) -> impl Future<Output = Result<(), LocalStoreError>> + Send {
            let mut batches = self.batches.lock().expect("memory store lock");
            batches.insert(batch.id().to_owned(), batch.entries().to_vec());
            async { Ok(()) }
        }

        fn batch_entries(
            &self,
            batch_id: &str,
        ) -> impl Future<Output = Result<Vec<StoreBatchEntry>, LocalStoreError>> + Send {
            let entries = self
                .batches
                .lock()
                .expect("memory store lock")
                .get(batch_id)
                .cloned()
                .unwrap_or_default();
            async move { Ok(entries) }
        }

        fn close(self) -> impl Future<Output = Result<(), LocalStoreError>> + Send {
            async { Ok(()) }
        }
    }

    fn request(name: &str, password: &[u8]) -> CreateIdentityRequest {
        CreateIdentityRequest {
            display_name: name.to_owned(),
            email: Some(format!("{name}@example.test")),
            avatar: None,
            password: password.to_vec(),
        }
    }

    #[tokio::test]
    async fn local_identity_locks_unlocks_and_keeps_password_out_of_catalog() {
        let store = Arc::new(MemoryStore::default());
        let service = IdentityService::new(Arc::clone(&store));
        let identity = service
            .create_identity(request("Alice", b"correct horse"))
            .await
            .expect("identity creates");
        assert!(service.sign_in(&identity.identity_id, b"wrong", false).await.is_err());
        assert_eq!(
            service.snapshot().await.expect("snapshot").identities[0].state,
            IdentityState::Locked
        );
        let session = service
            .unlock(&identity.identity_id, b"correct horse", false)
            .await
            .expect("unlock succeeds");
        assert_eq!(session.identity_id(), identity.identity_id);
        let catalog = store
            .batches
            .lock()
            .expect("memory store lock")
            .get(IDENTITY_CATALOG_BATCH)
            .expect("catalog");
        let encoded = serde_json::to_string(catalog).expect("catalog encodes");
        assert!(!encoded.contains("correct horse"));
    }

    #[tokio::test]
    async fn remembered_sessions_resume_revoke_and_isolate_projects() {
        let store = Arc::new(MemoryStore::default());
        let credentials = MemorySessionCredentialStore::default();
        let service = IdentityService::with_credentials(Arc::clone(&store), credentials.clone());
        service.dismiss_welcome().await.expect("welcome dismissal persists");
        assert!(service.snapshot().await.expect("snapshot").welcome_dismissed);
        let alice = service
            .create_identity(request("Alice", b"alice password"))
            .await
            .expect("alice creates");
        let bob = service
            .create_identity(request("Bob", b"bob password"))
            .await
            .expect("bob creates");
        let alice_session = service
            .sign_in(&alice.identity_id, b"alice password", true)
            .await
            .expect("alice signs in");
        let bob_session = service
            .sign_in(&bob.identity_id, b"bob password", false)
            .await
            .expect("bob signs in");
        service
            .write_project_batch(
                &alice_session,
                "project",
                [StoreBatchEntry {
                    ordinal: 0,
                    payload: serde_json::json!({"owner": "alice"}),
                }],
            )
            .await
            .expect("alice writes project");
        assert!(service
            .project_entries(&bob_session, "project")
            .await
            .expect("bob reads isolated project")
            .is_empty());
        service.sign_out(&alice_session).expect("alice signs out");
        let restarted = IdentityService::with_credentials(Arc::clone(&store), credentials.clone());
        let resumed = restarted
            .resume(alice_session.session_id())
            .await
            .expect("remembered session resumes");
        assert_eq!(resumed.identity_id(), alice.identity_id);
        service
            .revoke_session(alice_session.session_id())
            .await
            .expect("session revokes");
        assert!(restarted
            .project_entries(&resumed, "project")
            .await
            .is_err());
        assert!(restarted.resume(alice_session.session_id()).await.is_err());
        assert_eq!(credentials.len(), 0);
    }
}
