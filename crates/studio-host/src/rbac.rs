//! Host-owned application authentication and row-scoped authorization.
//!
//! Application users are deliberately distinct from Studio identities and plugin principals.
//! This module keeps credential verifiers, membership, and lockout state in a host-private batch
//! inside the application's existing ticket-15 namespace. Guests receive only an authenticated
//! session and an [`AuthorizedApplicationDataHandle`]; they never receive this state or a storage
//! engine handle.

#![allow(missing_docs)]
#![allow(clippy::all, clippy::pedantic, clippy::restriction, clippy::nursery)]

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::{
    ApplicationDataError, ApplicationDataErrorCode, ApplicationDataGuestApi, ApplicationDataHandle,
    ApplicationDataHost, CollectionRequest, CollectionResponse, GuestDataRequest, LocalStore,
    PatchOperation, RecordId, StoreBatchEntry, StoredRecord,
};

const RBAC_BATCH: &str = "__rbac";
const RBAC_FORMAT_VERSION: u16 = 1;
const MAX_ROLES: usize = 256;
const MAX_USERS: usize = 10_000;
const MAX_CREDENTIALS: usize = 3;
const MAX_HASH_ROUNDS: u32 = 65_536;
const MAX_LOGIN_LENGTH: usize = 256;
const MAX_DISPLAY_NAME_LENGTH: usize = 256;

/// The credential channel used for an application login.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CredentialKind {
    /// A short numeric employee PIN.
    Pin,
    /// A host-validated badge or employee token.
    Badge,
    /// An email/password or equivalent password login.
    Password,
}

/// Secret input accepted only while the host derives a verifier.
///
/// The type intentionally has no `Serialize`, `Display`, or public byte accessor. Callers should
/// pass borrowed bytes from a host-owned capture surface and discard them after the call.
pub enum CredentialInput<'a> {
    /// Numeric PIN bytes.
    Pin(&'a [u8]),
    /// Badge/token bytes.
    Badge(&'a [u8]),
    /// Password bytes.
    Password(&'a [u8]),
}

impl fmt::Debug for CredentialInput<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CredentialInput(REDACTED)")
    }
}

impl CredentialInput<'_> {
    fn kind(&self) -> CredentialKind {
        match self {
            Self::Pin(_) => CredentialKind::Pin,
            Self::Badge(_) => CredentialKind::Badge,
            Self::Password(_) => CredentialKind::Password,
        }
    }

    fn secret(&self) -> &[u8] {
        match self {
            Self::Pin(secret) | Self::Badge(secret) | Self::Password(secret) => secret,
        }
    }
}

/// A bounded failed-login throttle policy declared by the host for one application.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThrottlePolicy {
    maximum_failures: u32,
    lockout: Duration,
}

impl ThrottlePolicy {
    /// Create a policy. At least one failure and a nonzero lockout are required.
    pub fn new(maximum_failures: u32, lockout: Duration) -> Result<Self, RbacError> {
        if maximum_failures == 0 || lockout.is_zero() || lockout > Duration::from_secs(86_400) {
            return Err(RbacError::new(RbacErrorCode::RequestInvalid));
        }
        Ok(Self {
            maximum_failures,
            lockout,
        })
    }

    /// Default production policy: five failures trigger a five-minute lockout.
    #[must_use]
    pub const fn default_policy() -> Self {
        Self {
            maximum_failures: 5,
            lockout: Duration::from_secs(300),
        }
    }

    /// Maximum failures before lockout.
    #[must_use]
    pub const fn maximum_failures(self) -> u32 {
        self.maximum_failures
    }

    /// Lockout duration.
    #[must_use]
    pub const fn lockout(self) -> Duration {
        self.lockout
    }
}

impl Default for ThrottlePolicy {
    fn default() -> Self {
        Self::default_policy()
    }
}

/// Data verbs a role may grant for one declared collection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DataOperation {
    /// Read one record.
    Select,
    /// Read all records permitted by the row scope.
    List,
    /// Create a record.
    Create,
    /// Merge or patch an existing record.
    Update,
    /// Delete an existing record.
    Delete,
}

/// Declarative row predicate evaluated by the host for every data operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RowScope {
    /// Every row in the granted collection.
    Any,
    /// Rows whose field equals the authenticated application user ID.
    Own {
        /// Record field containing the application user ID.
        field: String,
    },
    /// An explicit bounded set of record IDs.
    RecordIds(Vec<String>),
    /// Rows whose field equals this literal JSON value.
    FieldEquals {
        /// Record field to compare.
        field: String,
        /// Expected JSON value.
        value: Value,
    },
}

impl RowScope {
    /// Scope every row in a collection.
    #[must_use]
    pub const fn any() -> Self {
        Self::Any
    }

    /// Scope rows to the authenticated user's ID in `field`.
    pub fn own(field: impl Into<String>) -> Result<Self, RbacError> {
        let field = field.into();
        validate_name(&field, 64)?;
        Ok(Self::Own { field })
    }

    /// Scope rows to a bounded set of valid record IDs.
    pub fn record_ids(ids: impl IntoIterator<Item = String>) -> Result<Self, RbacError> {
        let mut unique = BTreeSet::new();
        for id in ids {
            RecordId::new(id.clone()).map_err(|_| RbacError::new(RbacErrorCode::RequestInvalid))?;
            if unique.len() >= 10_000 {
                return Err(RbacError::new(RbacErrorCode::RequestInvalid));
            }
            unique.insert(id);
        }
        Ok(Self::RecordIds(unique.into_iter().collect()))
    }

    /// Scope rows whose `field` equals `value`.
    pub fn field_equals(field: impl Into<String>, value: Value) -> Result<Self, RbacError> {
        let field = field.into();
        validate_name(&field, 64)?;
        Ok(Self::FieldEquals { field, value })
    }
}

/// One collection/verb/row-scope grant attached to a role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionGrant {
    collection: String,
    operations: BTreeSet<DataOperation>,
    row_scope: RowScope,
}

impl CollectionGrant {
    /// Construct a grant for a declared collection.
    pub fn new(
        collection: impl Into<String>,
        operations: impl IntoIterator<Item = DataOperation>,
        row_scope: RowScope,
    ) -> Result<Self, RbacError> {
        let collection = collection.into();
        validate_name(&collection, 64)?;
        let operations: BTreeSet<DataOperation> = operations.into_iter().collect();
        if operations.is_empty() {
            return Err(RbacError::new(RbacErrorCode::RequestInvalid));
        }
        Ok(Self {
            collection,
            operations,
            row_scope,
        })
    }

    /// Collection targeted by this grant.
    #[must_use]
    pub fn collection(&self) -> &str {
        &self.collection
    }

    /// Operations admitted by this grant.
    #[must_use]
    pub fn operations(&self) -> &BTreeSet<DataOperation> {
        &self.operations
    }

    /// Row predicate evaluated by the host.
    #[must_use]
    pub const fn row_scope(&self) -> &RowScope {
        &self.row_scope
    }
}

/// A named application role with route, screen, action, and data bindings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleDefinition {
    name: String,
    routes: BTreeSet<String>,
    screens: BTreeSet<String>,
    actions: BTreeSet<String>,
    collections: Vec<CollectionGrant>,
}

impl RoleDefinition {
    /// Construct an empty role.
    pub fn new(name: impl Into<String>) -> Result<Self, RbacError> {
        let name = name.into();
        validate_name(&name, 64)?;
        Ok(Self {
            name,
            routes: BTreeSet::new(),
            screens: BTreeSet::new(),
            actions: BTreeSet::new(),
            collections: Vec::new(),
        })
    }

    /// Role name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Add a route binding in place.
    pub fn grant_route(&mut self, route: impl Into<String>) -> Result<(), RbacError> {
        let route = route.into();
        validate_name(&route, 128)?;
        self.routes.insert(route);
        Ok(())
    }

    /// Builder form of [`Self::grant_route`].
    pub fn with_route(mut self, route: impl Into<String>) -> Result<Self, RbacError> {
        self.grant_route(route)?;
        Ok(self)
    }

    /// Add a screen binding in place.
    pub fn grant_screen(&mut self, screen: impl Into<String>) -> Result<(), RbacError> {
        let screen = screen.into();
        validate_name(&screen, 128)?;
        self.screens.insert(screen);
        Ok(())
    }

    /// Builder form of [`Self::grant_screen`].
    pub fn with_screen(mut self, screen: impl Into<String>) -> Result<Self, RbacError> {
        self.grant_screen(screen)?;
        Ok(self)
    }

    /// Add an action binding in place.
    pub fn grant_action(&mut self, action: impl Into<String>) -> Result<(), RbacError> {
        let action = action.into();
        validate_name(&action, 128)?;
        self.actions.insert(action);
        Ok(())
    }

    /// Builder form of [`Self::grant_action`].
    pub fn with_action(mut self, action: impl Into<String>) -> Result<Self, RbacError> {
        self.grant_action(action)?;
        Ok(self)
    }

    /// Add a collection grant in place.
    pub fn grant_collection(&mut self, grant: CollectionGrant) {
        self.collections.push(grant);
    }

    /// Builder form of [`Self::grant_collection`].
    #[must_use]
    pub fn with_collection(mut self, grant: CollectionGrant) -> Self {
        self.grant_collection(grant);
        self
    }
}

/// Target category used for host-side route/screen/action authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorizationTarget {
    /// A declared application route.
    Route(String),
    /// A declared application screen.
    Screen(String),
    /// A declared application action.
    Action(String),
}

/// A successful host-authenticated application session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationSession {
    namespace: crate::ApplicationDataNamespace,
    user_id: String,
    generation: u64,
    nonce: [u8; 16],
}

impl ApplicationSession {
    /// Stable application user ID associated with the session.
    #[must_use]
    pub fn user_id(&self) -> &str {
        &self.user_id
    }
}

/// Security-relevant application event emitted by an optional host audit sink.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationAuditEvent {
    kind: ApplicationAuditEventKind,
    outcome: ApplicationAuditOutcome,
    actor: Option<String>,
    subject: Option<String>,
    target: Option<String>,
    occurred_at: SystemTime,
}

impl ApplicationAuditEvent {
    /// Event class.
    #[must_use]
    pub const fn kind(&self) -> ApplicationAuditEventKind {
        self.kind
    }

    /// Event outcome.
    #[must_use]
    pub const fn outcome(&self) -> ApplicationAuditOutcome {
        self.outcome
    }

    /// Authenticated actor, when one exists.
    #[must_use]
    pub fn actor(&self) -> Option<&str> {
        self.actor.as_deref()
    }

    /// Affected subject, when one exists.
    #[must_use]
    pub fn subject(&self) -> Option<&str> {
        self.subject.as_deref()
    }

    /// Safe target identifier, when one exists.
    #[must_use]
    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }

    /// Host timestamp at emission.
    #[must_use]
    pub const fn occurred_at(&self) -> SystemTime {
        self.occurred_at
    }
}

/// Closed audit event classes emitted by this module.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ApplicationAuditEventKind {
    /// A credential verification was attempted.
    Authentication,
    /// A role definition was created.
    RoleCreated,
    /// A user was created.
    UserCreated,
    /// A role was assigned to a user.
    RoleAssigned,
    /// A role was removed from a user.
    RoleRevoked,
    /// A user was disabled.
    UserDisabled,
}

/// Safe result classification attached to an audit event.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ApplicationAuditOutcome {
    /// The operation completed.
    Success,
    /// A supplied credential or authorization was rejected.
    Denied,
    /// A verification was blocked by the throttle policy.
    Throttled,
}

/// Optional host-owned audit integration point.
pub trait ApplicationAuditSink: Send + Sync {
    /// Receive a value-free, redaction-safe security event.
    fn record(&self, event: ApplicationAuditEvent);
}

/// Configuration for one application RBAC binding.
pub struct ApplicationRbacSettings {
    throttle: ThrottlePolicy,
    audit_sink: Option<Arc<dyn ApplicationAuditSink>>,
}

impl Clone for ApplicationRbacSettings {
    fn clone(&self) -> Self {
        Self {
            throttle: self.throttle,
            audit_sink: self.audit_sink.clone(),
        }
    }
}

impl Default for ApplicationRbacSettings {
    fn default() -> Self {
        Self {
            throttle: ThrottlePolicy::default(),
            audit_sink: None,
        }
    }
}

impl ApplicationRbacSettings {
    /// Set the declared failed-verification policy.
    #[must_use]
    pub fn with_throttle(mut self, throttle: ThrottlePolicy) -> Self {
        self.throttle = throttle;
        self
    }

    /// Install an optional audit sink used for security events.
    #[must_use]
    pub fn with_audit_sink(mut self, sink: Arc<dyn ApplicationAuditSink>) -> Self {
        self.audit_sink = Some(sink);
        self
    }

    /// Current throttle policy.
    #[must_use]
    pub const fn throttle(&self) -> ThrottlePolicy {
        self.throttle
    }
}

/// Stable, safe RBAC/authentication failure family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RbacErrorCode {
    /// Input violated a host bound.
    RequestInvalid,
    /// Storage was unavailable or malformed.
    StorageUnavailable,
    /// A role already exists.
    RoleAlreadyExists,
    /// A role or user was not found for a management operation.
    ManagementTargetNotFound,
    /// A user already exists.
    UserAlreadyExists,
    /// The supplied credentials did not verify.
    AuthenticationInvalid,
    /// Verification is currently blocked by lockout.
    AuthenticationThrottled,
    /// The authenticated session is stale, disabled, or from another app.
    SessionInvalid,
    /// The session lacks the requested route/screen/action/data grant.
    AuthorizationDenied,
    /// A membership operation would make no state change.
    MembershipUnchanged,
}

/// Safe RBAC/authentication error with no credential, record, or storage detail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RbacError {
    code: RbacErrorCode,
}

impl RbacError {
    const fn new(code: RbacErrorCode) -> Self {
        Self { code }
    }

    /// Stable failure code.
    #[must_use]
    pub const fn code(self) -> RbacErrorCode {
        self.code
    }
}

impl fmt::Display for RbacError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            RbacErrorCode::RequestInvalid => "application authorization request invalid",
            RbacErrorCode::StorageUnavailable => "application authorization unavailable",
            RbacErrorCode::RoleAlreadyExists => "application role already exists",
            RbacErrorCode::ManagementTargetNotFound => "application authorization target not found",
            RbacErrorCode::UserAlreadyExists => "application user already exists",
            RbacErrorCode::AuthenticationInvalid => "application authentication invalid",
            RbacErrorCode::AuthenticationThrottled => "application authentication throttled",
            RbacErrorCode::SessionInvalid => "application session invalid",
            RbacErrorCode::AuthorizationDenied => "application authorization denied",
            RbacErrorCode::MembershipUnchanged => "application membership unchanged",
        })
    }
}

impl Error for RbacError {}

/// Host-side RBAC state bound to one ticket-15 application data namespace.
pub struct ApplicationRbacHandle<'a, S> {
    data: ApplicationDataHandle<'a, S>,
    settings: ApplicationRbacSettings,
    state: Mutex<Option<RbacState>>,
}

/// Bind RBAC to the existing application-data host and collection declarations.
impl<S: LocalStore> ApplicationDataHost<S> {
    /// Bind users/roles to this verified application's namespace.
    pub fn bind_rbac<'a>(
        &'a self,
        principal: &studio_security::PluginPrincipal,
        declarations: impl IntoIterator<Item = crate::CollectionDeclaration>,
        settings: ApplicationRbacSettings,
    ) -> Result<ApplicationRbacHandle<'a, S>, ApplicationDataError> {
        Ok(ApplicationRbacHandle {
            data: self.bind(principal, declarations)?,
            settings,
            state: Mutex::const_new(None),
        })
    }
}

impl<'host, S: LocalStore> ApplicationRbacHandle<'host, S> {
    /// Application namespace bound to this host service.
    #[must_use]
    pub const fn namespace(&self) -> crate::ApplicationDataNamespace {
        self.data.namespace()
    }

    /// Define and persist one role. This is a host-management operation, not a guest call.
    pub async fn define_role(&self, role: RoleDefinition) -> Result<(), RbacError> {
        let name = role.name.clone();
        let mut state = self.loaded_state().await?;
        let state = state.as_mut().expect("loaded state is initialized");
        if state.roles.contains_key(&name) || state.roles.len() >= MAX_ROLES {
            return Err(RbacError::new(RbacErrorCode::RoleAlreadyExists));
        }
        state.roles.insert(name.clone(), role);
        self.persist(&state).await?;
        self.emit(
            ApplicationAuditEventKind::RoleCreated,
            ApplicationAuditOutcome::Success,
            None,
            None,
            Some(name),
        );
        Ok(())
    }

    /// Create a user and immediately hash its supplied credentials in host memory.
    pub async fn create_user<'secret>(
        &self,
        user_id: impl Into<String>,
        login: impl Into<String>,
        display_name: impl Into<String>,
        credentials: impl IntoIterator<Item = CredentialInput<'secret>>,
    ) -> Result<(), RbacError> {
        let user_id = user_id.into();
        let login = login.into();
        let display_name = display_name.into();
        validate_name(&user_id, 128)?;
        validate_name(&login, MAX_LOGIN_LENGTH)?;
        if display_name.is_empty() || display_name.len() > MAX_DISPLAY_NAME_LENGTH {
            return Err(RbacError::new(RbacErrorCode::RequestInvalid));
        }
        let mut credential_records = Vec::new();
        let mut kinds = BTreeSet::new();
        for credential in credentials {
            if credential_records.len() >= MAX_CREDENTIALS
                || !kinds.insert(credential.kind())
                || !valid_secret(credential.kind(), credential.secret())
            {
                return Err(RbacError::new(RbacErrorCode::RequestInvalid));
            }
            credential_records.push(PersistedCredential::derive(
                credential.kind(),
                credential.secret(),
            )?);
        }
        if credential_records.is_empty() {
            return Err(RbacError::new(RbacErrorCode::RequestInvalid));
        }

        let mut state = self.loaded_state().await?;
        let state = state.as_mut().expect("loaded state is initialized");
        if state.users.len() >= MAX_USERS
            || state
                .users
                .values()
                .any(|user| user.user_id == user_id || user.login == login)
        {
            return Err(RbacError::new(RbacErrorCode::UserAlreadyExists));
        }
        state.users.insert(
            user_id.clone(),
            PersistedUser {
                user_id: user_id.clone(),
                login,
                display_name,
                enabled: true,
                generation: 0,
                roles: BTreeSet::new(),
                credentials: credential_records,
                failures: 0,
                locked_until_ms: None,
            },
        );
        self.persist(&state).await?;
        self.emit(
            ApplicationAuditEventKind::UserCreated,
            ApplicationAuditOutcome::Success,
            None,
            Some(user_id),
            None,
        );
        Ok(())
    }

    /// Convenience registration for a PIN employee account.
    pub async fn create_pin_user(
        &self,
        user_id: impl Into<String>,
        login: impl Into<String>,
        display_name: impl Into<String>,
        pin: &[u8],
    ) -> Result<(), RbacError> {
        self.create_user(user_id, login, display_name, [CredentialInput::Pin(pin)])
            .await
    }

    /// Convenience registration for an email/password account.
    pub async fn create_password_user(
        &self,
        user_id: impl Into<String>,
        email: impl Into<String>,
        display_name: impl Into<String>,
        password: &[u8],
    ) -> Result<(), RbacError> {
        self.create_user(
            user_id,
            email,
            display_name,
            [CredentialInput::Password(password)],
        )
        .await
    }

    /// Convenience registration for a badge-entry account.
    pub async fn create_badge_user(
        &self,
        user_id: impl Into<String>,
        login: impl Into<String>,
        display_name: impl Into<String>,
        badge: &[u8],
    ) -> Result<(), RbacError> {
        self.create_user(
            user_id,
            login,
            display_name,
            [CredentialInput::Badge(badge)],
        )
        .await
    }

    /// Assign a role to a user and invalidate that user's existing sessions.
    pub async fn assign_role(&self, user_id: &str, role_name: &str) -> Result<(), RbacError> {
        let mut state = self.loaded_state().await?;
        let state = state.as_mut().expect("loaded state is initialized");
        if !state.roles.contains_key(role_name) {
            return Err(RbacError::new(RbacErrorCode::ManagementTargetNotFound));
        }
        let user = state
            .users
            .get_mut(user_id)
            .ok_or(RbacError::new(RbacErrorCode::ManagementTargetNotFound))?;
        if !user.roles.insert(role_name.to_owned()) {
            return Err(RbacError::new(RbacErrorCode::MembershipUnchanged));
        }
        user.generation = user.generation.wrapping_add(1);
        self.persist(&state).await?;
        self.emit(
            ApplicationAuditEventKind::RoleAssigned,
            ApplicationAuditOutcome::Success,
            None,
            Some(user_id.to_owned()),
            Some(role_name.to_owned()),
        );
        Ok(())
    }

    /// Revoke a role from a user and invalidate that user's existing sessions.
    pub async fn revoke_role(&self, user_id: &str, role_name: &str) -> Result<(), RbacError> {
        let mut state = self.loaded_state().await?;
        let state = state.as_mut().expect("loaded state is initialized");
        let user = state
            .users
            .get_mut(user_id)
            .ok_or(RbacError::new(RbacErrorCode::ManagementTargetNotFound))?;
        if !user.roles.remove(role_name) {
            return Err(RbacError::new(RbacErrorCode::MembershipUnchanged));
        }
        user.generation = user.generation.wrapping_add(1);
        self.persist(&state).await?;
        self.emit(
            ApplicationAuditEventKind::RoleRevoked,
            ApplicationAuditOutcome::Success,
            None,
            Some(user_id.to_owned()),
            Some(role_name.to_owned()),
        );
        Ok(())
    }

    /// Disable an application user and invalidate all sessions for that user.
    pub async fn disable_user(&self, user_id: &str) -> Result<(), RbacError> {
        let mut state = self.loaded_state().await?;
        let state = state.as_mut().expect("loaded state is initialized");
        let user = state
            .users
            .get_mut(user_id)
            .ok_or(RbacError::new(RbacErrorCode::ManagementTargetNotFound))?;
        if !user.enabled {
            return Err(RbacError::new(RbacErrorCode::MembershipUnchanged));
        }
        user.enabled = false;
        user.generation = user.generation.wrapping_add(1);
        self.persist(&state).await?;
        self.emit(
            ApplicationAuditEventKind::UserDisabled,
            ApplicationAuditOutcome::Success,
            None,
            Some(user_id.to_owned()),
            None,
        );
        Ok(())
    }

    /// Verify a PIN, badge, or password entirely in the host process and return a session.
    pub async fn authenticate(
        &self,
        login: &str,
        credential: CredentialInput<'_>,
    ) -> Result<ApplicationSession, RbacError> {
        self.authenticate_at(login, credential, SystemTime::now())
            .await
    }

    /// Verify a PIN login using the host-owned offline user store.
    pub async fn authenticate_pin(
        &self,
        login: &str,
        pin: &[u8],
    ) -> Result<ApplicationSession, RbacError> {
        self.authenticate(login, CredentialInput::Pin(pin)).await
    }

    /// Verify a badge login using the host-owned offline user store.
    pub async fn authenticate_badge(
        &self,
        login: &str,
        badge: &[u8],
    ) -> Result<ApplicationSession, RbacError> {
        self.authenticate(login, CredentialInput::Badge(badge))
            .await
    }

    /// Verify an email/password login using the host-owned offline user store.
    pub async fn authenticate_password(
        &self,
        email: &str,
        password: &[u8],
    ) -> Result<ApplicationSession, RbacError> {
        self.authenticate(email, CredentialInput::Password(password))
            .await
    }

    /// Deterministic-time authentication hook for host tests and replay harnesses.
    pub async fn authenticate_at(
        &self,
        login: &str,
        credential: CredentialInput<'_>,
        now: SystemTime,
    ) -> Result<ApplicationSession, RbacError> {
        if login.is_empty() || login.len() > MAX_LOGIN_LENGTH || login.chars().any(char::is_control)
        {
            return Err(RbacError::new(RbacErrorCode::AuthenticationInvalid));
        }
        let mut state = self.loaded_state().await?;
        let state = state.as_mut().expect("loaded state is initialized");
        let now_ms = epoch_millis(now);
        let Some(user_id) = state
            .users
            .values()
            .find(|user| user.login == login)
            .map(|user| user.user_id.clone())
        else {
            self.emit(
                ApplicationAuditEventKind::Authentication,
                ApplicationAuditOutcome::Denied,
                None,
                None,
                None,
            );
            return Err(RbacError::new(RbacErrorCode::AuthenticationInvalid));
        };
        let user = state.users.get_mut(&user_id).expect("user selected above");
        if !user.enabled {
            self.emit(
                ApplicationAuditEventKind::Authentication,
                ApplicationAuditOutcome::Denied,
                None,
                Some(user_id),
                None,
            );
            return Err(RbacError::new(RbacErrorCode::AuthenticationInvalid));
        }
        if user.locked_until_ms.is_some_and(|locked| locked > now_ms) {
            self.emit(
                ApplicationAuditEventKind::Authentication,
                ApplicationAuditOutcome::Throttled,
                None,
                Some(user_id),
                None,
            );
            return Err(RbacError::new(RbacErrorCode::AuthenticationThrottled));
        }
        let kind = credential.kind();
        let valid = user
            .credentials
            .iter()
            .find(|stored| stored.kind == kind.into())
            .is_some_and(|stored| stored.verify(credential.secret()));
        if !valid {
            user.failures = user.failures.saturating_add(1);
            let throttled = user.failures >= self.settings.throttle.maximum_failures;
            if throttled {
                user.locked_until_ms =
                    Some(now_ms.saturating_add(duration_millis(self.settings.throttle.lockout)));
            }
            self.persist(&state).await?;
            self.emit(
                ApplicationAuditEventKind::Authentication,
                if throttled {
                    ApplicationAuditOutcome::Throttled
                } else {
                    ApplicationAuditOutcome::Denied
                },
                None,
                Some(user_id),
                None,
            );
            return Err(RbacError::new(if throttled {
                RbacErrorCode::AuthenticationThrottled
            } else {
                RbacErrorCode::AuthenticationInvalid
            }));
        }
        user.failures = 0;
        user.locked_until_ms = None;
        let generation = user.generation;
        self.persist(&state).await?;
        let mut nonce = [0_u8; 16];
        getrandom::fill(&mut nonce)
            .map_err(|_| RbacError::new(RbacErrorCode::StorageUnavailable))?;
        self.emit(
            ApplicationAuditEventKind::Authentication,
            ApplicationAuditOutcome::Success,
            Some(user_id.clone()),
            Some(user_id.clone()),
            None,
        );
        Ok(ApplicationSession {
            namespace: self.namespace(),
            user_id,
            generation,
            nonce,
        })
    }

    /// Bind a verified session to the host-enforced collection helper surface.
    pub async fn data_for<'call>(
        &'call self,
        session: &ApplicationSession,
    ) -> Result<AuthorizedApplicationDataHandle<'call, 'host, S>, RbacError> {
        self.validate_session(session).await?;
        Ok(AuthorizedApplicationDataHandle {
            rbac: self,
            session: session.clone(),
        })
    }

    /// Alias for [`Self::data_for`] for host adapters that call the result an authorized handle.
    pub async fn authorized_data<'call>(
        &'call self,
        session: &ApplicationSession,
    ) -> Result<AuthorizedApplicationDataHandle<'call, 'host, S>, RbacError> {
        self.data_for(session).await
    }

    /// Check a route, screen, or action grant without consulting interface visibility.
    pub async fn authorize(
        &self,
        session: &ApplicationSession,
        target: AuthorizationTarget,
    ) -> Result<(), RbacError> {
        let state = self.loaded_state().await?;
        let state = state.as_ref().expect("loaded state is initialized");
        let user = self.validated_user(&state, session)?;
        let allowed = user.roles.iter().any(|role_name| {
            let Some(role) = state.roles.get(role_name) else {
                return false;
            };
            match &target {
                AuthorizationTarget::Route(value) => role.routes.contains(value),
                AuthorizationTarget::Screen(value) => role.screens.contains(value),
                AuthorizationTarget::Action(value) => role.actions.contains(value),
            }
        });
        if allowed {
            Ok(())
        } else {
            Err(RbacError::new(RbacErrorCode::AuthorizationDenied))
        }
    }

    /// Authorize a route without requiring callers to construct a target enum.
    pub async fn authorize_route(
        &self,
        session: &ApplicationSession,
        route: &str,
    ) -> Result<(), RbacError> {
        self.authorize(session, AuthorizationTarget::Route(route.to_owned()))
            .await
    }

    /// Authorize a screen without requiring callers to construct a target enum.
    pub async fn authorize_screen(
        &self,
        session: &ApplicationSession,
        screen: &str,
    ) -> Result<(), RbacError> {
        self.authorize(session, AuthorizationTarget::Screen(screen.to_owned()))
            .await
    }

    /// Authorize an action without requiring callers to construct a target enum.
    pub async fn authorize_action(
        &self,
        session: &ApplicationSession,
        action: &str,
    ) -> Result<(), RbacError> {
        self.authorize(session, AuthorizationTarget::Action(action.to_owned()))
            .await
    }

    async fn validate_session(&self, session: &ApplicationSession) -> Result<(), RbacError> {
        let state = self.loaded_state().await?;
        let state = state.as_ref().expect("loaded state is initialized");
        self.validated_user(&state, session).map(|_| ())
    }

    fn validated_user<'a>(
        &self,
        state: &'a RbacState,
        session: &ApplicationSession,
    ) -> Result<&'a PersistedUser, RbacError> {
        if session.namespace != self.namespace() || session.nonce == [0; 16] {
            return Err(RbacError::new(RbacErrorCode::SessionInvalid));
        }
        let user = state
            .users
            .get(&session.user_id)
            .ok_or(RbacError::new(RbacErrorCode::SessionInvalid))?;
        if !user.enabled || user.generation != session.generation {
            return Err(RbacError::new(RbacErrorCode::SessionInvalid));
        }
        Ok(user)
    }

    async fn loaded_state(
        &self,
    ) -> Result<tokio::sync::MutexGuard<'_, Option<RbacState>>, RbacError> {
        let mut state = self.state.lock().await;
        let needs_load = state.is_none();
        if needs_load {
            *state = Some(self.load().await?);
        }
        Ok(state)
    }

    async fn load(&self) -> Result<RbacState, RbacError> {
        let entries = self
            .data
            .internal_batch_entries(RBAC_BATCH)
            .await
            .map_err(|_| RbacError::new(RbacErrorCode::StorageUnavailable))?;
        if entries.is_empty() {
            return Ok(RbacState::default());
        }
        if entries.len() != 2 {
            return Err(RbacError::new(RbacErrorCode::StorageUnavailable));
        }
        let PersistedEntry::Header { format_version } = decode(&entries[0])? else {
            return Err(RbacError::new(RbacErrorCode::StorageUnavailable));
        };
        if format_version != RBAC_FORMAT_VERSION {
            return Err(RbacError::new(RbacErrorCode::StorageUnavailable));
        }
        let PersistedEntry::State { state } = decode(&entries[1])? else {
            return Err(RbacError::new(RbacErrorCode::StorageUnavailable));
        };
        Ok(state.into_runtime()?)
    }

    async fn persist(&self, state: &RbacState) -> Result<(), RbacError> {
        let payload = serde_json::to_value(PersistedRbacState::from(state))
            .map_err(|_| RbacError::new(RbacErrorCode::StorageUnavailable))?;
        self.data
            .internal_write_batch(
                RBAC_BATCH,
                [
                    StoreBatchEntry { ordinal: 0, payload: serde_json::json!({ "kind": "header", "format_version": RBAC_FORMAT_VERSION }) },
                    StoreBatchEntry { ordinal: 1, payload: serde_json::json!({ "kind": "state", "state": payload }) },
                ],
            )
            .await
            .map_err(|_| RbacError::new(RbacErrorCode::StorageUnavailable))
    }

    fn emit(
        &self,
        kind: ApplicationAuditEventKind,
        outcome: ApplicationAuditOutcome,
        actor: Option<String>,
        subject: Option<String>,
        target: Option<String>,
    ) {
        if let Some(sink) = &self.settings.audit_sink {
            sink.record(ApplicationAuditEvent {
                kind,
                outcome,
                actor,
                subject,
                target,
                occurred_at: SystemTime::now(),
            });
        }
    }
}

/// User-bound collection helpers. Every call revalidates the session and row predicate in host code.
pub struct AuthorizedApplicationDataHandle<'a, 'b, S> {
    rbac: &'a ApplicationRbacHandle<'b, S>,
    session: ApplicationSession,
}

impl<S: LocalStore> AuthorizedApplicationDataHandle<'_, '_, S> {
    /// Select one row after host-side role and row-scope checks.
    pub async fn select(
        &self,
        collection: impl Into<String>,
        id: RecordId,
    ) -> Result<Option<StoredRecord>, RbacError> {
        let collection = collection.into();
        let scopes = self.scopes(DataOperation::Select, &collection).await?;
        let result = self
            .rbac
            .data
            .select(collection, id)
            .await
            .map_err(map_data_error)?;
        if result.as_ref().is_some_and(|record| {
            scopes
                .iter()
                .any(|scope| row_allowed(scope, &self.session, record))
        }) {
            Ok(result)
        } else if result.is_none() {
            Ok(None)
        } else {
            Err(RbacError::new(RbacErrorCode::AuthorizationDenied))
        }
    }

    /// List only rows admitted by the host-side role and row scopes.
    pub async fn list(
        &self,
        collection: impl Into<String>,
    ) -> Result<Vec<StoredRecord>, RbacError> {
        let collection = collection.into();
        let scopes = self.scopes(DataOperation::List, &collection).await?;
        Ok(self
            .rbac
            .data
            .list(collection)
            .await
            .map_err(map_data_error)?
            .into_iter()
            .filter(|record| {
                scopes
                    .iter()
                    .any(|scope| row_allowed(scope, &self.session, record))
            })
            .collect())
    }

    /// Create a row only if its submitted values satisfy the row scope.
    pub async fn create(
        &self,
        collection: impl Into<String>,
        id: RecordId,
        record: Value,
    ) -> Result<StoredRecord, RbacError> {
        let collection = collection.into();
        let scopes = self.scopes(DataOperation::Create, &collection).await?;
        if !scopes
            .iter()
            .any(|scope| row_allowed_value(scope, &self.session, &id, &record))
        {
            return Err(RbacError::new(RbacErrorCode::AuthorizationDenied));
        }
        self.rbac
            .data
            .create(collection, id, record)
            .await
            .map_err(map_data_error)
    }

    /// Merge fields after checking both current and resulting row scope.
    pub async fn update_merge(
        &self,
        collection: impl Into<String>,
        id: RecordId,
        fields: Value,
    ) -> Result<StoredRecord, RbacError> {
        let collection = collection.into();
        let scopes = self.scopes(DataOperation::Update, &collection).await?;
        let current = self
            .rbac
            .data
            .select(collection.clone(), id.clone())
            .await
            .map_err(map_data_error)?
            .ok_or(RbacError::new(RbacErrorCode::AuthorizationDenied))?;
        let projected = merge_value(&current.value, &fields)
            .ok_or(RbacError::new(RbacErrorCode::AuthorizationDenied))?;
        if !scopes.iter().any(|scope| {
            row_allowed(scope, &self.session, &current)
                && row_allowed_value(scope, &self.session, &id, &projected)
        }) {
            return Err(RbacError::new(RbacErrorCode::AuthorizationDenied));
        }
        self.rbac
            .data
            .update_merge(collection, id, fields)
            .await
            .map_err(map_data_error)
    }

    /// Apply a patch only when the resulting row remains in scope.
    pub async fn update_patch(
        &self,
        collection: impl Into<String>,
        id: RecordId,
        operations: Vec<PatchOperation>,
    ) -> Result<StoredRecord, RbacError> {
        let collection = collection.into();
        let scopes = self.scopes(DataOperation::Update, &collection).await?;
        let current = self
            .rbac
            .data
            .select(collection.clone(), id.clone())
            .await
            .map_err(map_data_error)?
            .ok_or(RbacError::new(RbacErrorCode::AuthorizationDenied))?;
        let projected = patch_value(&current.value, &operations)
            .ok_or(RbacError::new(RbacErrorCode::AuthorizationDenied))?;
        if !scopes.iter().any(|scope| {
            row_allowed(scope, &self.session, &current)
                && row_allowed_value(scope, &self.session, &id, &projected)
        }) {
            return Err(RbacError::new(RbacErrorCode::AuthorizationDenied));
        }
        self.rbac
            .data
            .update_patch(collection, id, operations)
            .await
            .map_err(map_data_error)
    }

    /// Delete only a row already admitted by the role and scope.
    pub async fn delete(
        &self,
        collection: impl Into<String>,
        id: RecordId,
    ) -> Result<bool, RbacError> {
        let collection = collection.into();
        let scopes = self.scopes(DataOperation::Delete, &collection).await?;
        let current = self
            .rbac
            .data
            .select(collection.clone(), id.clone())
            .await
            .map_err(map_data_error)?;
        if let Some(record) = current {
            if !scopes
                .iter()
                .any(|scope| row_allowed(scope, &self.session, &record))
            {
                return Err(RbacError::new(RbacErrorCode::AuthorizationDenied));
            }
        }
        self.rbac
            .data
            .delete(collection, id)
            .await
            .map_err(map_data_error)
    }

    async fn scopes(
        &self,
        operation: DataOperation,
        collection: &str,
    ) -> Result<Vec<RowScope>, RbacError> {
        let state = self.rbac.loaded_state().await?;
        let state = state.as_ref().expect("loaded state is initialized");
        let user = self.rbac.validated_user(&state, &self.session)?;
        let scopes = user
            .roles
            .iter()
            .flat_map(|role_name| state.roles.get(role_name))
            .flat_map(|role| role.collections.iter())
            .filter(|grant| grant.collection == collection && grant.operations.contains(&operation))
            .map(|grant| grant.row_scope.clone())
            .collect::<Vec<_>>();
        if scopes.is_empty() {
            Err(RbacError::new(RbacErrorCode::AuthorizationDenied))
        } else {
            Ok(scopes)
        }
    }
}

impl<S: LocalStore> ApplicationDataGuestApi for AuthorizedApplicationDataHandle<'_, '_, S> {
    fn execute(
        &self,
        request: GuestDataRequest,
    ) -> impl std::future::Future<Output = Result<CollectionResponse, ApplicationDataError>> + Send
    {
        async move {
            match request {
                GuestDataRequest::Collection(request) => self
                    .execute_collection(request)
                    .await
                    .map_err(to_data_error),
                GuestDataRequest::Forbidden(operation) => {
                    self.rbac
                        .data
                        .execute(GuestDataRequest::Forbidden(operation))
                        .await
                }
            }
        }
    }
}

impl<S: LocalStore> AuthorizedApplicationDataHandle<'_, '_, S> {
    async fn execute_collection(
        &self,
        request: CollectionRequest,
    ) -> Result<CollectionResponse, RbacError> {
        match request {
            CollectionRequest::Select { collection, id } => Ok(CollectionResponse::Selected(
                self.select(collection, id).await?,
            )),
            CollectionRequest::List { collection } => {
                Ok(CollectionResponse::Listed(self.list(collection).await?))
            }
            CollectionRequest::Create {
                collection,
                id,
                record,
            } => Ok(CollectionResponse::Written(
                self.create(collection, id, record).await?,
            )),
            CollectionRequest::UpdateMerge {
                collection,
                id,
                fields,
            } => Ok(CollectionResponse::Written(
                self.update_merge(collection, id, fields).await?,
            )),
            CollectionRequest::UpdatePatch {
                collection,
                id,
                operations,
            } => Ok(CollectionResponse::Written(
                self.update_patch(collection, id, operations).await?,
            )),
            CollectionRequest::Delete { collection, id } => Ok(CollectionResponse::Deleted(
                self.delete(collection, id).await?,
            )),
        }
    }
}

#[derive(Default)]
struct RbacState {
    roles: BTreeMap<String, RoleDefinition>,
    users: BTreeMap<String, PersistedUser>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PersistedEntry {
    Header { format_version: u16 },
    State { state: PersistedRbacState },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PersistedRbacState {
    roles: Vec<PersistedRole>,
    users: Vec<PersistedUser>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PersistedRole {
    name: String,
    routes: Vec<String>,
    screens: Vec<String>,
    actions: Vec<String>,
    collections: Vec<PersistedGrant>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PersistedGrant {
    collection: String,
    operations: Vec<DataOperationPersisted>,
    row_scope: PersistedRowScope,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
enum DataOperationPersisted {
    Select,
    List,
    Create,
    Update,
    Delete,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
enum PersistedRowScope {
    Any,
    Own { field: String },
    RecordIds { ids: Vec<String> },
    FieldEquals { field: String, value: Value },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PersistedUser {
    user_id: String,
    login: String,
    display_name: String,
    enabled: bool,
    generation: u64,
    roles: BTreeSet<String>,
    credentials: Vec<PersistedCredential>,
    failures: u32,
    locked_until_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PersistedCredential {
    kind: CredentialKindPersisted,
    salt: [u8; 16],
    digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum CredentialKindPersisted {
    Pin,
    Badge,
    Password,
}

impl PersistedCredential {
    fn derive(kind: CredentialKind, secret: &[u8]) -> Result<Self, RbacError> {
        let mut salt = [0_u8; 16];
        getrandom::fill(&mut salt)
            .map_err(|_| RbacError::new(RbacErrorCode::StorageUnavailable))?;
        Ok(Self {
            kind: kind.into(),
            salt,
            digest: derive_digest(kind, &salt, secret),
        })
    }

    fn verify(&self, secret: &[u8]) -> bool {
        constant_time_eq(
            &self.digest,
            &derive_digest(self.kind.into(), &self.salt, secret),
        )
    }
}

impl From<CredentialKind> for CredentialKindPersisted {
    fn from(value: CredentialKind) -> Self {
        match value {
            CredentialKind::Pin => Self::Pin,
            CredentialKind::Badge => Self::Badge,
            CredentialKind::Password => Self::Password,
        }
    }
}

impl From<CredentialKindPersisted> for CredentialKind {
    fn from(value: CredentialKindPersisted) -> Self {
        match value {
            CredentialKindPersisted::Pin => Self::Pin,
            CredentialKindPersisted::Badge => Self::Badge,
            CredentialKindPersisted::Password => Self::Password,
        }
    }
}

impl From<DataOperation> for DataOperationPersisted {
    fn from(value: DataOperation) -> Self {
        match value {
            DataOperation::Select => Self::Select,
            DataOperation::List => Self::List,
            DataOperation::Create => Self::Create,
            DataOperation::Update => Self::Update,
            DataOperation::Delete => Self::Delete,
        }
    }
}

impl From<DataOperationPersisted> for DataOperation {
    fn from(value: DataOperationPersisted) -> Self {
        match value {
            DataOperationPersisted::Select => Self::Select,
            DataOperationPersisted::List => Self::List,
            DataOperationPersisted::Create => Self::Create,
            DataOperationPersisted::Update => Self::Update,
            DataOperationPersisted::Delete => Self::Delete,
        }
    }
}

impl From<&RowScope> for PersistedRowScope {
    fn from(value: &RowScope) -> Self {
        match value {
            RowScope::Any => Self::Any,
            RowScope::Own { field } => Self::Own {
                field: field.clone(),
            },
            RowScope::RecordIds(ids) => Self::RecordIds { ids: ids.clone() },
            RowScope::FieldEquals { field, value } => Self::FieldEquals {
                field: field.clone(),
                value: value.clone(),
            },
        }
    }
}

impl TryFrom<PersistedRowScope> for RowScope {
    type Error = RbacError;
    fn try_from(value: PersistedRowScope) -> Result<Self, Self::Error> {
        match value {
            PersistedRowScope::Any => Ok(Self::Any),
            PersistedRowScope::Own { field } => Self::own(field),
            PersistedRowScope::RecordIds { ids } => Self::record_ids(ids),
            PersistedRowScope::FieldEquals { field, value } => Self::field_equals(field, value),
        }
    }
}

impl From<&RbacState> for PersistedRbacState {
    fn from(value: &RbacState) -> Self {
        Self {
            roles: value
                .roles
                .values()
                .map(|role| PersistedRole {
                    name: role.name.clone(),
                    routes: role.routes.iter().cloned().collect(),
                    screens: role.screens.iter().cloned().collect(),
                    actions: role.actions.iter().cloned().collect(),
                    collections: role
                        .collections
                        .iter()
                        .map(|grant| PersistedGrant {
                            collection: grant.collection.clone(),
                            operations: grant.operations.iter().copied().map(Into::into).collect(),
                            row_scope: (&grant.row_scope).into(),
                        })
                        .collect(),
                })
                .collect(),
            users: value.users.values().cloned().collect(),
        }
    }
}

impl PersistedRbacState {
    fn into_runtime(self) -> Result<RbacState, RbacError> {
        let mut roles = BTreeMap::new();
        for role in self.roles {
            let mut definition = RoleDefinition::new(role.name.clone())?;
            for route in role.routes {
                definition.grant_route(route)?;
            }
            for screen in role.screens {
                definition.grant_screen(screen)?;
            }
            for action in role.actions {
                definition.grant_action(action)?;
            }
            for grant in role.collections {
                let operations = grant.operations.into_iter().map(Into::into);
                definition.grant_collection(CollectionGrant::new(
                    grant.collection,
                    operations,
                    grant.row_scope.try_into()?,
                )?);
            }
            if roles.insert(role.name, definition).is_some() {
                return Err(RbacError::new(RbacErrorCode::StorageUnavailable));
            }
        }
        let users = self
            .users
            .into_iter()
            .map(|user| (user.user_id.clone(), user))
            .collect();
        Ok(RbacState { roles, users })
    }
}

fn decode(entry: &StoreBatchEntry) -> Result<PersistedEntry, RbacError> {
    serde_json::from_value(entry.payload.clone())
        .map_err(|_| RbacError::new(RbacErrorCode::StorageUnavailable))
}

fn row_allowed(scope: &RowScope, session: &ApplicationSession, record: &StoredRecord) -> bool {
    row_allowed_value(scope, session, &record.id, &record.value)
}

fn row_allowed_value(
    scope: &RowScope,
    session: &ApplicationSession,
    id: &RecordId,
    value: &Value,
) -> bool {
    match scope {
        RowScope::Any => true,
        RowScope::Own { field } => value
            .get(field)
            .and_then(Value::as_str)
            .is_some_and(|value| value == session.user_id()),
        RowScope::RecordIds(ids) => ids.iter().any(|allowed| allowed == id.as_str()),
        RowScope::FieldEquals {
            field,
            value: expected,
        } => value.get(field).is_some_and(|value| value == expected),
    }
}

fn merge_value(current: &Value, fields: &Value) -> Option<Value> {
    let mut current = current.as_object()?.clone();
    for (field, value) in fields.as_object()? {
        current.insert(field.clone(), value.clone());
    }
    Some(Value::Object(current))
}

fn patch_value(current: &Value, operations: &[PatchOperation]) -> Option<Value> {
    let mut current = current.as_object()?.clone();
    for operation in operations {
        match operation {
            PatchOperation::Set { field, value } => {
                current.insert(field.clone(), value.clone());
            }
            PatchOperation::Remove { field } => {
                current.remove(field);
            }
        }
    }
    Some(Value::Object(current))
}

fn valid_secret(kind: CredentialKind, secret: &[u8]) -> bool {
    match kind {
        CredentialKind::Pin => {
            (4..=16).contains(&secret.len()) && secret.iter().all(u8::is_ascii_digit)
        }
        CredentialKind::Badge => !secret.is_empty() && secret.len() <= 256,
        CredentialKind::Password => (8..=256).contains(&secret.len()),
    }
}

fn derive_digest(kind: CredentialKind, salt: &[u8; 16], secret: &[u8]) -> [u8; 32] {
    let mut input = Vec::with_capacity(64 + secret.len());
    input.extend_from_slice(b"studio.application-auth.pbkdf2-sha256.v1");
    input.push(kind as u8);
    input.extend_from_slice(salt);
    input.extend_from_slice(secret);
    let mut digest: [u8; 32] = Sha256::digest(&input).into();
    for round in 1..MAX_HASH_ROUNDS {
        let mut next = Sha256::new();
        next.update(b"studio.application-auth.pbkdf2-sha256.v1");
        next.update(round.to_be_bytes());
        next.update(salt);
        next.update(digest);
        next.update(secret);
        digest = next.finalize().into();
    }
    digest
}

fn constant_time_eq(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn validate_name(value: &str, max: usize) -> Result<(), RbacError> {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        Err(RbacError::new(RbacErrorCode::RequestInvalid))
    } else {
        Ok(())
    }
}

fn epoch_millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn map_data_error(error: ApplicationDataError) -> RbacError {
    match error.code() {
        ApplicationDataErrorCode::RecordNotFound => {
            RbacError::new(RbacErrorCode::AuthorizationDenied)
        }
        _ => RbacError::new(RbacErrorCode::StorageUnavailable),
    }
}

fn to_data_error(_error: RbacError) -> ApplicationDataError {
    ApplicationDataError::authorization_denied()
}
