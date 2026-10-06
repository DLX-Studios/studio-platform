//! Secure bundle-to-policy-to-instance-to-mount startup orchestration.

use parking_lot::Mutex;
use std::{ffi::OsStr, fs, sync::Arc};

use serde_json::Value;
use sha2::{Digest, Sha256};
use studio_actions::Checkout;
use studio_components::{HostEventDispatcher, NativeStateStore};
use studio_host::{LocalStore, MigrationError, MigrationRunner, MigrationStepError};
use studio_net::limits::BrokerLimits;
use studio_net::{
    BrokerError, HttpsClient, ProductionHttpTransport, RestBroker, RestBrokerConfig,
    TransportLimits,
};
use studio_oauth::{
    BrowserHandoff, CallbackListener, EntropySource, HttpsJwksProvider, HttpsOAuthTransport,
    OAuthManager, OsEntropy, ProtectedOAuthTokenStore, ProtectedSecretReference, ProviderPackage,
    ProviderRegistry as OAuthProviderRegistry, SystemBrowser, TcpLoopbackListener,
};
use studio_package::{
    ArchivePolicy, CanonicalBundleInput, ManifestPolicy, ProviderRegistry, TrustStore,
    TrustStoreError, VerifiedMigrationBundle, canonical_bundle_document, inspect_archive,
    parse_manifest, verify_bundle_signature,
};
use studio_protocol::{GuestMessage, MountTree, ProtocolLimits, UiNode, decode_guest_message};
use studio_security::{
    ApplicationEnvironment, CredentialBackend, CredentialBackendError, CredentialBytes,
    CredentialLocator, OsCredentialBackend, PluginPrincipal, ProtectedSecretKey,
    ProtectedSecretStore, SecretInput, TrustMode,
};
use studio_ui::{InstanceId, UiRegistry};
use studio_wasm::{ModulePolicy, PluginInstance, RuntimeBudgets, SandboxEngine};
use thiserror::Error;

use crate::{
    cli::{LaunchMode, LaunchRequest},
    plugin_surface::PluginSurface,
};

/// Whether a native Wayland endpoint is available for this launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaylandAvailability {
    /// Native Wayland display or inherited socket is available.
    Available,
    /// No native Wayland endpoint is present; no fallback is allowed.
    Unavailable,
}

impl WaylandAvailability {
    /// Detect the two native endpoint forms recognized by GPUI's Wayland backend.
    #[must_use]
    pub fn from_environment() -> Self {
        if endpoint_present(std::env::var_os("WAYLAND_DISPLAY").as_deref())
            || endpoint_present(std::env::var_os("WAYLAND_SOCKET").as_deref())
        {
            Self::Available
        } else {
            Self::Unavailable
        }
    }
}

fn endpoint_present(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

/// Immutable host policy and provisioned trust snapshot for one launch.
#[derive(Clone, Debug)]
pub struct HostConfig {
    /// Provisioned publisher verification keys.
    pub trust_store: TrustStore,
    /// Archive resource ceilings.
    pub archive_policy: ArchivePolicy,
    /// Closed manifest and requested-resource ceilings.
    pub manifest_policy: ManifestPolicy,
    /// Host–guest message and UI ceilings.
    pub protocol_limits: ProtocolLimits,
    /// Host-maintained provider descriptor and capability policy.
    pub provider_registry: ProviderRegistry,
}

impl HostConfig {
    /// Create the default milestone-one policy with a supplied trust snapshot.
    #[must_use]
    pub fn new(trust_store: TrustStore) -> Self {
        Self {
            trust_store,
            archive_policy: ArchivePolicy::default(),
            manifest_policy: ManifestPolicy::default(),
            protocol_limits: ProtocolLimits::default(),
            provider_registry: ProviderRegistry::maintained(),
        }
    }
}

/// Stable host-owned startup failure family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchErrorCode {
    /// CLI selector or arity is invalid.
    ArgumentsInvalid,
    /// Selected path violates mode or regular-file requirements.
    PathInvalid,
    /// Native Wayland is unavailable.
    WaylandUnavailable,
    /// Archive or closed manifest validation failed.
    BundleInvalid,
    /// Production publisher signature/trust validation failed.
    IntegrityInvalid,
    /// Operator publisher trust configuration is absent or unusable.
    TrustConfigurationInvalid,
    /// WebAssembly policy, instantiation, or initialization failed.
    GuestInvalid,
    /// Initial guest output or retained tree is invalid.
    UiInvalid,
    /// A signed application migration must run before guest access.
    MigrationRequired,
    /// A required application migration failed or was quarantined.
    MigrationInvalid,
}

/// Detailed host-owned startup rejection.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LaunchError {
    /// CLI did not select exactly one explicit mode and path.
    #[error("usage: studio-app (--bundle <absolute-path> | --dev <local-path>)")]
    ArgumentsInvalid,
    /// Path is not permitted for the selected mode.
    #[error("selected bundle path is invalid")]
    PathInvalid,
    /// No native Wayland endpoint is available.
    #[error("Studio requires a native Wayland session; X11 and XWayland are not supported")]
    WaylandUnavailable,
    /// Archive or manifest admission failed.
    #[error("plugin bundle validation failed: {0}")]
    BundleInvalid(String),
    /// Trust or signature admission failed.
    #[error("plugin integrity verification failed")]
    IntegrityInvalid,
    /// Operator publisher trust configuration is absent or unusable.
    #[error("publisher trust configuration rejected: {0}")]
    TrustConfigurationInvalid(TrustStoreError),
    /// Module policy or runtime startup failed.
    #[error("plugin guest startup failed: {0}")]
    GuestInvalid(String),
    /// Initial protocol mount admission failed.
    #[error("plugin UI mount failed: {0}")]
    UiInvalid(String),
    /// The selected signed bundle declares migrations but no lifecycle was provided.
    #[error("plugin application migration must complete before launch")]
    MigrationRequired,
    /// Migration failed safely; the application remains unavailable until recovery.
    #[error("plugin application migration failed: {0}")]
    MigrationInvalid(MigrationError),
}

impl LaunchError {
    /// Return the stable host-owned failure family.
    #[must_use]
    pub const fn code(&self) -> LaunchErrorCode {
        match self {
            Self::ArgumentsInvalid => LaunchErrorCode::ArgumentsInvalid,
            Self::PathInvalid => LaunchErrorCode::PathInvalid,
            Self::WaylandUnavailable => LaunchErrorCode::WaylandUnavailable,
            Self::BundleInvalid(_) => LaunchErrorCode::BundleInvalid,
            Self::IntegrityInvalid => LaunchErrorCode::IntegrityInvalid,
            Self::TrustConfigurationInvalid(_) => LaunchErrorCode::TrustConfigurationInvalid,
            Self::GuestInvalid(_) => LaunchErrorCode::GuestInvalid,
            Self::UiInvalid(_) => LaunchErrorCode::UiInvalid,
            Self::MigrationRequired => LaunchErrorCode::MigrationRequired,
            Self::MigrationInvalid(_) => LaunchErrorCode::MigrationInvalid,
        }
    }
}

/// Stateless secure startup orchestrator over one immutable host policy snapshot.
#[derive(Clone)]
pub struct StudioHost {
    config: HostConfig,
    wayland: WaylandAvailability,
    https_client: Option<Arc<dyn HttpsClient>>,
    credential_backend: Arc<dyn CredentialBackend>,
    provisioned_secrets: Arc<Mutex<Vec<(ProtectedSecretKey, SecretInput)>>>,
    browser: Arc<dyn BrowserHandoff>,
    callback_listener: Arc<dyn CallbackListener>,
    entropy: Arc<dyn EntropySource>,
}

impl std::fmt::Debug for StudioHost {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StudioHost")
            .field("config", &self.config)
            .field("wayland", &self.wayland)
            .field("https_client_installed", &self.https_client.is_some())
            .field("credential_backend_installed", &true)
            .field(
                "provisioned_secret_count",
                &self.provisioned_secrets.lock().len(),
            )
            .field("oauth_adapters_installed", &true)
            .finish()
    }
}

impl StudioHost {
    /// Create ordered compositor-shutdown ownership for one verified principal.
    #[must_use]
    pub fn shutdown_coordinator(principal: PluginPrincipal) -> crate::ShutdownCoordinator {
        crate::ShutdownCoordinator::new(principal)
    }
    /// Create host-owned recovery state for one verified principal.
    ///
    /// # Errors
    ///
    /// Returns an error when a fresh runtime identity cannot be generated.
    pub fn plugin_recovery(
        principal: PluginPrincipal,
    ) -> Result<crate::PluginRecovery, crate::RecoveryError> {
        crate::PluginRecovery::new(principal)
    }
    /// Create a host for one detected platform session.
    #[must_use]
    pub fn new(config: HostConfig, wayland: WaylandAvailability) -> Self {
        Self {
            config,
            wayland,
            https_client: None,
            credential_backend: Arc::new(OsCredentialBackend),
            provisioned_secrets: Arc::new(Mutex::new(Vec::new())),
            browser: Arc::new(SystemBrowser),
            callback_listener: Arc::new(TcpLoopbackListener),
            entropy: Arc::new(OsEntropy),
        }
    }

    /// Bind the platform HTTPS client used by provider OAuth and REST sessions.
    #[must_use]
    pub fn with_https_client(mut self, client: Arc<dyn HttpsClient>) -> Self {
        self.https_client = Some(client);
        self
    }

    /// Replace the system credential adapter, primarily for deterministic host tests.
    #[must_use]
    pub fn with_credential_backend(mut self, backend: Arc<dyn CredentialBackend>) -> Self {
        self.credential_backend = backend;
        self
    }

    /// Supply one host-captured secret for immediate protected-store provisioning.
    #[must_use]
    pub fn with_protected_secret(self, name: ProtectedSecretKey, value: SecretInput) -> Self {
        self.provisioned_secrets.lock().push((name, value));
        self
    }

    /// Override host OAuth adapters for deterministic integration tests.
    #[must_use]
    pub fn with_oauth_adapters(
        mut self,
        browser: Arc<dyn BrowserHandoff>,
        callback_listener: Arc<dyn CallbackListener>,
        entropy: Arc<dyn EntropySource>,
    ) -> Self {
        self.browser = browser;
        self.callback_listener = callback_listener;
        self.entropy = entropy;
        self
    }

    /// Construct the host-owned REST broker for one admitted package.
    ///
    /// Callers provide the package's already admitted route declarations and host-only resolver
    /// seams. The factory compiles every route atomically before returning, so a package cannot
    /// observe a broker with only a subset of its routes installed.
    ///
    /// # Errors
    ///
    /// Returns a stable broker admission error when a declaration or host limit is invalid.
    pub fn prepare_broker<'store>(
        &self,
        config: RestBrokerConfig<'store>,
    ) -> Result<std::sync::Arc<RestBroker<'store>>, BrokerError> {
        RestBroker::from_config(config)
    }

    /// Compose an instance-owned protected payment session from verified host identities.
    ///
    /// # Errors
    ///
    /// Returns a safe checkout construction failure.
    pub fn protected_payment_session(
        owner: InstanceId,
        principal: PluginPrincipal,
        checkout: Checkout,
    ) -> Result<crate::ProtectedPaymentSession, crate::ProtectedPaymentError> {
        crate::ProtectedPaymentSession::new(owner, principal, checkout)
    }

    /// Compose the complete native checkout shell for one verified plugin instance.
    ///
    /// # Errors
    ///
    /// Rejects invalid checkout, trusted-input, or navigation initialization.
    pub fn checkout_shell(
        owner: InstanceId,
        principal: PluginPrincipal,
        checkout: Checkout,
        reduced_motion: bool,
    ) -> Result<crate::NativeCheckoutShell, crate::NativeCheckoutError> {
        crate::NativeCheckoutShell::new(owner, principal, checkout, reduced_motion)
    }

    /// Admit, verify, instantiate, initialize, and atomically mount one selected bundle.
    ///
    /// # Errors
    ///
    /// Returns a host-owned [`LaunchError`] before exposing any partially prepared surface.
    pub fn prepare(&self, request: LaunchRequest) -> Result<PluginSurface, LaunchError> {
        self.prepare_internal(request, false, self.https_client.clone())
    }

    /// Run signed application migrations and launch only after the lifecycle commits.
    ///
    /// The runner receives a host-owned LocalStore and a host callback for the migration document;
    /// no database or guest capability crosses into migration code. Unsigned development bundles
    /// are rejected because migrations are an authenticated package authority.
    pub async fn prepare_with_migrations<S, F>(
        &self,
        request: LaunchRequest,
        store: &S,
        action: F,
    ) -> Result<PluginSurface, LaunchError>
    where
        S: LocalStore,
        F: FnMut(
                &studio_package::MigrationDeclaration,
                &[u8],
                &mut Value,
            ) -> Result<(), MigrationStepError>
            + Send,
    {
        if self.wayland == WaylandAvailability::Unavailable {
            return Err(LaunchError::WaylandUnavailable);
        }
        if request.mode() != LaunchMode::Production {
            return Err(LaunchError::MigrationRequired);
        }
        let path = request.path();
        if !path.is_absolute()
            || !path.metadata().is_ok_and(|metadata| {
                metadata.file_type().is_file()
                    && metadata.len() <= self.config.archive_policy.max_archive_bytes as u64
            })
        {
            return Err(LaunchError::PathInvalid);
        }
        let bytes = fs::read(path).map_err(|_| LaunchError::PathInvalid)?;
        let archive = inspect_archive(&bytes, self.config.archive_policy)
            .map_err(|error| LaunchError::BundleInvalid(error.to_string()))?;
        if self.config.trust_store.is_empty() {
            return Err(LaunchError::TrustConfigurationInvalid(
                TrustStoreError::NoActiveKeys,
            ));
        }
        let package = VerifiedMigrationBundle::admit(
            &archive,
            self.config.manifest_policy,
            &self.config.trust_store,
        )
        .map_err(|error| LaunchError::MigrationInvalid(MigrationError::Admission(error)))?;
        MigrationRunner::new(store)
            .run(&package, action)
            .await
            .map_err(LaunchError::MigrationInvalid)?;
        self.prepare_internal(request, true, self.https_client.clone())
    }

    fn prepare_internal(
        &self,
        request: LaunchRequest,
        migrations_complete: bool,
        https_client: Option<Arc<dyn HttpsClient>>,
    ) -> Result<PluginSurface, LaunchError> {
        if self.wayland == WaylandAvailability::Unavailable {
            return Err(LaunchError::WaylandUnavailable);
        }
        let (mode, path) = request.into_parts();
        if (mode == LaunchMode::Production && !path.is_absolute())
            || !path.metadata().is_ok_and(|metadata| {
                metadata.file_type().is_file()
                    && metadata.len() <= self.config.archive_policy.max_archive_bytes as u64
            })
        {
            return Err(LaunchError::PathInvalid);
        }
        let archive_bytes = fs::read(path).map_err(|_| LaunchError::PathInvalid)?;
        let archive = inspect_archive(&archive_bytes, self.config.archive_policy)
            .map_err(|error| LaunchError::BundleInvalid(error.to_string()))?;
        let manifest = parse_manifest(&archive.manifest, self.config.manifest_policy)
            .map_err(|error| LaunchError::BundleInvalid(error.to_string()))?;
        let manifest_value: Value = serde_json::from_slice(&archive.manifest)
            .map_err(|error| LaunchError::BundleInvalid(error.to_string()))?;
        let declared_assets = manifest.assets.clone();
        let archived_assets = archive.assets.keys().cloned().collect::<Vec<_>>();
        if declared_assets != archived_assets {
            return Err(LaunchError::BundleInvalid(
                "declared assets do not match archive assets".to_owned(),
            ));
        }
        let canonical_input = CanonicalBundleInput {
            manifest: manifest_value,
            module_path: manifest.entry.clone(),
            module: archive.module.clone(),
            assets: archive.assets,
        };
        let render_assets = canonical_input.assets.clone();
        if mode == LaunchMode::Production {
            if self.config.trust_store.is_empty() {
                return Err(LaunchError::TrustConfigurationInvalid(
                    TrustStoreError::NoActiveKeys,
                ));
            }
            verify_bundle_signature(
                &canonical_input,
                &archive.signature,
                &manifest.publisher.id,
                &manifest.publisher.key_id,
                &self.config.trust_store,
            )
            .map_err(|_| LaunchError::IntegrityInvalid)?;
        } else {
            canonical_bundle_document(&canonical_input)
                .map_err(|error| LaunchError::BundleInvalid(error.to_string()))?;
        }
        if !manifest.migrations.is_empty() && !migrations_complete {
            return Err(LaunchError::MigrationRequired);
        }

        let provider_plan = self
            .config
            .provider_registry
            .admit(&manifest, &studio_net::limits::BrokerLimits::default())
            .map_err(|error| LaunchError::BundleInvalid(error.to_string()))?;

        let github_services = prepare_github_services(
            &manifest,
            &archive_bytes,
            mode,
            &provider_plan,
            https_client,
            Arc::clone(&self.credential_backend),
            Arc::clone(&self.provisioned_secrets),
            Arc::clone(&self.browser),
            Arc::clone(&self.callback_listener),
            Arc::clone(&self.entropy),
        )?;

        let engine =
            SandboxEngine::new().map_err(|error| LaunchError::GuestInvalid(error.to_string()))?;
        let module_policy = ModulePolicy {
            max_memory_pages: u64::from(manifest.limits.memory_mib) * 16,
            ..ModulePolicy::default()
        };
        let validated = module_policy
            .validate(&engine, &canonical_input.module)
            .map_err(|error| LaunchError::GuestInvalid(error.to_string()))?;
        let budgets = RuntimeBudgets {
            max_memory_bytes: usize::from(manifest.limits.memory_mib) * 1024 * 1024,
            fuel_per_call: manifest.limits.event_fuel,
            ..RuntimeBudgets::default()
        };
        let mut instance = PluginInstance::instantiate(engine, validated, budgets)
            .map_err(|error| LaunchError::GuestInvalid(error.to_string()))?;
        let outcome = instance
            .invoke_init(0, 0)
            .map_err(|error| LaunchError::GuestInvalid(error.to_string()))?;
        let mount = decode_single_mount(&outcome.emissions, self.config.protocol_limits)?;

        let owner = InstanceId::new(manifest.id)
            .map_err(|error| LaunchError::UiInvalid(error.to_string()))?;
        let mut dispatcher = HostEventDispatcher::new(owner.clone());
        register_events(&mut dispatcher, &mount.root)?;
        let mut registry = UiRegistry::new(owner, self.config.protocol_limits);
        registry
            .mount(mount)
            .map_err(|error| LaunchError::UiInvalid(error.to_string()))?;
        let native_state = NativeStateStore::from_registry(dispatcher.owner(), &registry)
            .map_err(|error| LaunchError::UiInvalid(error.to_string()))?;
        Ok(PluginSurface::new(
            mode,
            registry,
            native_state,
            dispatcher,
            instance,
            render_assets,
            self.config.protocol_limits,
            provider_plan,
            github_services,
        ))
    }
}

#[derive(Clone)]
struct SharedCredentialBackend(Arc<dyn CredentialBackend>);

impl CredentialBackend for SharedCredentialBackend {
    fn set_secret(
        &self,
        locator: &CredentialLocator,
        secret: &[u8],
    ) -> Result<(), CredentialBackendError> {
        self.0.set_secret(locator, secret)
    }

    fn get_secret(
        &self,
        locator: &CredentialLocator,
    ) -> Result<CredentialBytes, CredentialBackendError> {
        self.0.get_secret(locator)
    }

    fn delete_secret(&self, locator: &CredentialLocator) -> Result<(), CredentialBackendError> {
        self.0.delete_secret(locator)
    }
}

fn prepare_github_services(
    manifest: &studio_package::ManifestV1,
    archive_bytes: &[u8],
    mode: LaunchMode,
    provider_plan: &studio_package::ProviderAdmissionPlan,
    https_client: Option<Arc<dyn HttpsClient>>,
    credential_backend: Arc<dyn CredentialBackend>,
    provisioned_secrets: Arc<Mutex<Vec<(ProtectedSecretKey, SecretInput)>>>,
    browser: Arc<dyn BrowserHandoff>,
    callback_listener: Arc<dyn CallbackListener>,
    entropy: Arc<dyn EntropySource>,
) -> Result<Option<(Arc<OAuthManager>, Arc<RestBroker<'static>>)>, LaunchError> {
    if !provider_plan
        .providers()
        .iter()
        .any(|provider| provider.id == "github")
    {
        return Ok(None);
    }
    let Some(https_client) = https_client else {
        return Ok(None);
    };
    let integration = manifest
        .integrations
        .iter()
        .find(|integration| integration.id == "github")
        .ok_or_else(|| {
            LaunchError::BundleInvalid(String::from("GitHub integration config missing"))
        })?;
    let config = integration.config.as_ref().ok_or_else(|| {
        LaunchError::BundleInvalid(String::from("GitHub integration config missing"))
    })?;
    let client_id = config
        .get("clientId")
        .and_then(Value::as_str)
        .ok_or_else(|| LaunchError::BundleInvalid(String::from("GitHub client id missing")))?;
    let secret_name = config
        .get("clientSecretName")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            LaunchError::BundleInvalid(String::from("GitHub secret reference missing"))
        })?;
    let secret_declaration = manifest
        .secrets
        .iter()
        .find(|secret| secret.name == secret_name)
        .ok_or_else(|| {
            LaunchError::BundleInvalid(String::from("GitHub secret declaration missing"))
        })?;
    let provider = provider_plan
        .providers()
        .iter()
        .find(|provider| provider.id == "github")
        .ok_or_else(|| {
            LaunchError::BundleInvalid(String::from("GitHub provider was not admitted"))
        })?;

    let mut instance_id = [0_u8; 16];
    getrandom::fill(&mut instance_id)
        .map_err(|_| LaunchError::GuestInvalid(String::from("runtime identity unavailable")))?;
    let bundle_digest: [u8; 32] = Sha256::digest(archive_bytes).into();
    let (principal, environment) = match mode {
        LaunchMode::Production => (
            PluginPrincipal::new_verified(
                manifest.publisher.id.clone(),
                manifest.publisher.key_id.clone(),
                manifest.id.clone(),
                bundle_digest,
                instance_id,
                TrustMode::Production,
            ),
            ApplicationEnvironment::Production,
        ),
        LaunchMode::Development => (
            PluginPrincipal::new(
                manifest.publisher.key_id.clone(),
                manifest.id.clone(),
                bundle_digest,
                instance_id,
                TrustMode::Development,
            ),
            ApplicationEnvironment::Development,
        ),
    };
    let principal = principal
        .map_err(|_| LaunchError::BundleInvalid(String::from("provider identity invalid")))?;
    let protected_store = ProtectedSecretStore::new(SharedCredentialBackend(credential_backend));
    let scope = protected_store
        .for_application(&principal, environment)
        .map_err(|_| LaunchError::BundleInvalid(String::from("protected store unavailable")))?;
    for (key, value) in std::mem::take(&mut *provisioned_secrets.lock()) {
        if manifest.secrets.iter().any(|declaration| {
            declaration.name == key.name() && declaration.purpose == key.purpose()
        }) {
            scope.configure(&key, value).map_err(|_| {
                LaunchError::BundleInvalid(String::from("protected secret configuration failed"))
            })?;
        }
    }
    drop(scope);
    let package = ProviderPackage::new("github", provider.version.clone(), client_id)
        .with_client_secret(ProtectedSecretReference {
            name: secret_name.to_owned(),
            purpose: secret_declaration.purpose.clone(),
        });
    let manager = Arc::new(OAuthManager::new(
        OAuthProviderRegistry::maintained(),
        [package],
        Arc::new(ProtectedOAuthTokenStore::new(
            protected_store,
            principal,
            environment,
        )),
        browser,
        callback_listener,
        entropy,
        Arc::new(HttpsOAuthTransport::new(Arc::clone(&https_client))),
        // OpenID Connect providers publish their signing keys; the host verifies the ID token
        // signature itself rather than trusting the token because it arrived over TLS.
        Arc::new(HttpsJwksProvider::new(Arc::clone(&https_client))),
    ));
    let transport = Arc::new(ProductionHttpTransport::new(
        https_client,
        TransportLimits::default(),
    ));
    let mut broker = RestBroker::new(transport, BrokerLimits::default());
    provider_plan
        .install_into(&mut broker)
        .map_err(|_| LaunchError::BundleInvalid(String::from("GitHub routes invalid")))?;
    let resolver: Arc<dyn studio_net::credential::OAuthSessionResolver> = manager.clone();
    broker.set_oauth_resolver(resolver);
    Ok(Some((manager, Arc::new(broker))))
}

fn decode_single_mount(
    emissions: &[Vec<u8>],
    limits: ProtocolLimits,
) -> Result<MountTree, LaunchError> {
    let [emission] = emissions else {
        return Err(LaunchError::UiInvalid(
            "initial call must emit exactly one mount".to_owned(),
        ));
    };
    match decode_guest_message(emission, limits)
        .map_err(|error| LaunchError::UiInvalid(error.to_string()))?
    {
        GuestMessage::Mount(mount) => Ok(mount),
        _ => Err(LaunchError::UiInvalid(
            "first guest message is not a mount".to_owned(),
        )),
    }
}

fn register_events(dispatcher: &mut HostEventDispatcher, root: &UiNode) -> Result<(), LaunchError> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        dispatcher
            .register(&node.id, node.kind)
            .map_err(|error| LaunchError::UiInvalid(error.to_string()))?;
        stack.extend(node.children.iter());
    }
    Ok(())
}
