//! Secure bundle-to-policy-to-instance-to-mount startup orchestration.

use std::{ffi::OsStr, fs};

use serde_json::Value;
use studio_actions::Checkout;
use studio_components::{HostEventDispatcher, NativeStateStore};
use studio_host::{LocalStore, MigrationError, MigrationRunner, MigrationStepError};
use studio_package::{
    ArchivePolicy, CanonicalBundleInput, ManifestPolicy, TrustStore, canonical_bundle_document,
    TrustStoreError, VerifiedMigrationBundle, inspect_archive, parse_manifest,
    verify_bundle_signature,
};
use studio_net::{BrokerError, RestBroker, RestBrokerConfig};
use studio_protocol::{GuestMessage, MountTree, ProtocolLimits, UiNode, decode_guest_message};
use studio_security::PluginPrincipal;
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
#[derive(Clone, Debug)]
pub struct StudioHost {
    config: HostConfig,
    wayland: WaylandAvailability,
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
    pub const fn new(config: HostConfig, wayland: WaylandAvailability) -> Self {
        Self { config, wayland }
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
        self.prepare_internal(request, false)
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
        F: FnMut(&studio_package::MigrationDeclaration, &[u8], &mut Value)
                -> Result<(), MigrationStepError>
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
        self.prepare_internal(request, true)
    }

    fn prepare_internal(
        &self,
        request: LaunchRequest,
        migrations_complete: bool,
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
        ))
    }
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
