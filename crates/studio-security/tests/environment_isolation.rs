#![allow(missing_docs)]

use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use studio_security::{
    ApplicationEnvironment, ApplicationSecretStore, CredentialBackend, CredentialBackendError,
    CredentialBytes, CredentialLocator, EnvironmentDataStore, EnvironmentErrorCode,
    OsCredentialBackend, PluginPrincipal, PromotionDirection, PromotionPlan,
    ProtectedConfiguration, ProtectedSecretError, ProtectedSecretErrorCode, ProtectedSecretKey,
    ProtectedSecretState, ProtectedSecretStatus, ProtectedSecretStore, SecretInput, TrustMode,
    apply_promotion, resolve_active_environment,
};

static NEXT_OS_TEST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Default)]
struct MemoryBackend {
    records: Arc<Mutex<HashMap<CredentialLocator, Vec<u8>>>>,
}

impl MemoryBackend {
    fn snapshot(&self) -> HashMap<CredentialLocator, Vec<u8>> {
        self.records.lock().unwrap().clone()
    }
}

impl CredentialBackend for MemoryBackend {
    fn set_secret(
        &self,
        locator: &CredentialLocator,
        secret: &[u8],
    ) -> Result<(), CredentialBackendError> {
        self.records
            .lock()
            .unwrap()
            .insert(locator.clone(), secret.to_vec());
        Ok(())
    }

    fn get_secret(
        &self,
        locator: &CredentialLocator,
    ) -> Result<CredentialBytes, CredentialBackendError> {
        self.records
            .lock()
            .unwrap()
            .get(locator)
            .cloned()
            .map(CredentialBytes::new)
            .ok_or(CredentialBackendError::NotFound)
    }

    fn delete_secret(&self, locator: &CredentialLocator) -> Result<(), CredentialBackendError> {
        self.records
            .lock()
            .unwrap()
            .remove(locator)
            .map(|_| ())
            .ok_or(CredentialBackendError::NotFound)
    }
}

struct OsVaultCleanup<'guard, 'store> {
    staging: &'guard ApplicationSecretStore<'store, OsCredentialBackend>,
    production: &'guard ApplicationSecretStore<'store, OsCredentialBackend>,
    key: &'guard ProtectedSecretKey,
    finished: bool,
}

impl<'guard, 'store> OsVaultCleanup<'guard, 'store> {
    fn new(
        staging: &'guard ApplicationSecretStore<'store, OsCredentialBackend>,
        production: &'guard ApplicationSecretStore<'store, OsCredentialBackend>,
        key: &'guard ProtectedSecretKey,
    ) -> Self {
        Self {
            staging,
            production,
            key,
            finished: false,
        }
    }

    fn cleanup(&mut self) -> Result<(), ProtectedSecretError> {
        let mut first_error = None;
        for (environment, scope) in [("staging", self.staging), ("production", self.production)] {
            if let Err(error) = scope.purge(self.key) {
                eprintln!(
                    "external gap: failed to purge OS-vault {environment} evidence ({error:?})"
                );
                first_error.get_or_insert(error);
            }
        }
        self.finished = true;
        first_error.map_or(Ok(()), Err)
    }
}

impl Drop for OsVaultCleanup<'_, '_> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.cleanup();
        }
    }
}

fn principal(publisher: &str, key: &str, application: &str) -> PluginPrincipal {
    PluginPrincipal::new_verified(
        publisher,
        key,
        application,
        [1; 32],
        [2; 16],
        TrustMode::Production,
    )
    .unwrap()
}

fn declaration() -> ProtectedSecretKey {
    ProtectedSecretKey::new(
        "payments.restricted_key",
        "Authenticate bounded payment API requests",
    )
    .unwrap()
}

fn config(environment: &str) -> ProtectedConfiguration {
    ProtectedConfiguration::new([("environment.active".to_string(), environment.to_string())])
}

#[test]
fn three_environments_coexist_with_independent_data_stores() {
    let store = EnvironmentDataStore::new("com.example.pos").unwrap();
    let development = store.scope(ApplicationEnvironment::Development);
    let staging = store.scope(ApplicationEnvironment::Staging);
    let production = store.scope(ApplicationEnvironment::Production);

    let development_key = development.key("orders.counter").unwrap();
    let staging_key = staging.key("orders.counter").unwrap();
    let production_key = production.key("orders.counter").unwrap();

    assert_ne!(development_key, staging_key);
    assert_ne!(staging_key, production_key);
    assert_ne!(development_key, production_key);

    // Same-logical-name keys are only valid inside their minting environment.
    assert_eq!(
        staging.admit(&development_key).unwrap_err().code(),
        EnvironmentErrorCode::CrossEnvironmentDenied
    );
    assert_eq!(
        production.admit(&staging_key).unwrap_err().code(),
        EnvironmentErrorCode::CrossEnvironmentDenied
    );
    assert_eq!(
        development.admit(&production_key).unwrap_err().code(),
        EnvironmentErrorCode::CrossEnvironmentDenied
    );

    assert!(development.admit(&development_key).is_ok());
    assert!(staging.admit(&staging_key).is_ok());
    assert!(production.admit(&production_key).is_ok());

    assert_eq!(
        development.environment(),
        ApplicationEnvironment::Development
    );
    assert_eq!(staging.application(), "com.example.pos");
}

#[test]
fn active_environment_resolution_follows_protected_configuration_without_defaults() {
    assert_eq!(
        resolve_active_environment(&config("development"))
            .unwrap()
            .environment(),
        ApplicationEnvironment::Development
    );
    assert_eq!(
        resolve_active_environment(&config("Staging"))
            .unwrap()
            .environment(),
        ApplicationEnvironment::Staging
    );
    assert_eq!(
        resolve_active_environment(&config("PRODUCTION"))
            .unwrap()
            .environment(),
        ApplicationEnvironment::Production
    );

    let missing = ProtectedConfiguration::default();
    assert_eq!(
        resolve_active_environment(&missing).unwrap_err().code(),
        EnvironmentErrorCode::ConfigMissing
    );

    let invalid = config("prod");
    let invalid_error = resolve_active_environment(&invalid).unwrap_err();
    assert_eq!(invalid_error.code(), EnvironmentErrorCode::ConfigInvalid);
    assert_eq!(invalid_error.stable_code(), "environment.config_invalid");
    assert!(!format!("{invalid_error}").contains("prod"));

    let ambiguous = ProtectedConfiguration::new([
        ("environment.active".to_string(), "staging".to_string()),
        ("environment.active".to_string(), "production".to_string()),
    ]);
    assert_eq!(
        resolve_active_environment(&ambiguous).unwrap_err().code(),
        EnvironmentErrorCode::ConfigAmbiguous
    );
}

#[test]
fn secret_resolution_follows_the_active_environment_not_the_package() {
    let backend = MemoryBackend::default();
    let store = ProtectedSecretStore::new(backend.clone());
    let principal = principal("publisher.example", "signing-key-1", "com.example.pos");
    let secret = declaration();

    let staging = store
        .for_application(&principal, ApplicationEnvironment::Staging)
        .unwrap();
    staging
        .configure(
            &secret,
            SecretInput::new(b"rk_staging_app_value_1".to_vec()).unwrap(),
        )
        .unwrap();

    for (environment, expected_state) in [
        (
            ApplicationEnvironment::Development,
            ProtectedSecretState::Missing,
        ),
        (
            ApplicationEnvironment::Staging,
            ProtectedSecretState::Configured,
        ),
        (
            ApplicationEnvironment::Production,
            ProtectedSecretState::Missing,
        ),
    ] {
        let scope = store.for_application(&principal, environment).unwrap();
        assert_eq!(
            scope.status(&secret).unwrap().state(),
            expected_state,
            "secret state must follow the environment partition, not the package"
        );
    }

    let active = match resolve_active_environment(&config("staging"))
        .unwrap()
        .environment()
    {
        ApplicationEnvironment::Staging => Some(&staging),
        _ => None,
    };
    assert_eq!(
        active.unwrap().status(&secret).unwrap().state(),
        ProtectedSecretState::Configured
    );
    assert_eq!(backend.snapshot().len(), 1);
}

#[test]
fn promotion_copies_zero_secret_material() {
    let backend = MemoryBackend::default();
    let store = ProtectedSecretStore::new(backend.clone());
    let principal = principal("publisher.example", "signing-key-1", "com.example.pos");
    let secret = declaration();

    let staging_store = store
        .for_application(&principal, ApplicationEnvironment::Staging)
        .unwrap();
    staging_store
        .configure(
            &secret,
            SecretInput::new(b"rk_staging_live_credential_material".to_vec()).unwrap(),
        )
        .unwrap();
    let production_store = store
        .for_application(&principal, ApplicationEnvironment::Production)
        .unwrap();

    let before = backend.snapshot();

    let secret_status = staging_store.status(&secret).unwrap();
    let undeclared_key =
        ProtectedSecretKey::new("analytics.token", "Authorize analytics uploads").unwrap();
    let undeclared_status = staging_store.status(&undeclared_key).unwrap();

    let plan = PromotionPlan::build(
        PromotionDirection::StagingToProduction,
        "com.example.pos",
        [secret_status, undeclared_status],
    )
    .unwrap();
    assert_eq!(plan.entries().len(), 2);
    assert_eq!(
        plan.requiring_configuration_in_target(),
        vec!["payments.restricted_key".to_string()]
    );
    let rendered_plan = format!("{plan:?}");
    assert!(!rendered_plan.contains("rk_staging_live_credential_material"));

    let data_store = EnvironmentDataStore::new("com.example.pos").unwrap();
    let mut source_records = BTreeMap::new();
    source_records.insert("orders.counter".to_string(), b"row-count-42".to_vec());
    let (receipt, promoted_records) = apply_promotion(
        &plan,
        &data_store.scope(ApplicationEnvironment::Staging),
        &data_store.scope(ApplicationEnvironment::Production),
        &source_records,
    )
    .unwrap();
    assert_eq!(receipt.data_records_copied, 1);
    assert_eq!(
        promoted_records.get("orders.counter"),
        Some(&b"row-count-42".to_vec())
    );
    assert_eq!(
        receipt.secrets_requiring_configuration,
        vec!["payments.restricted_key".to_string()]
    );

    let after = backend.snapshot();
    assert_eq!(
        before, after,
        "promotion must not add, remove, or alter any credential record"
    );
    assert_eq!(
        production_store.status(&secret).unwrap().state(),
        ProtectedSecretState::Missing
    );
}

#[test]
fn wrong_environment_denial_matrix_produces_stable_safe_codes() {
    let store = EnvironmentDataStore::new("com.example.pos").unwrap();
    let environments = [
        ApplicationEnvironment::Development,
        ApplicationEnvironment::Staging,
        ApplicationEnvironment::Production,
    ];

    for from in environments {
        for to in environments {
            if from == to {
                continue;
            }
            let source_scope = store.scope(from);
            let target_scope = store.scope(to);
            let key = source_scope.key("orders.counter").unwrap();
            assert_eq!(
                target_scope.admit(&key).unwrap_err().stable_code(),
                "environment.cross_environment_denied",
                "{from:?} -> {to:?}"
            );
        }
    }

    let developer = PluginPrincipal::new_verified(
        "publisher.local",
        "developer-key",
        "com.example.local",
        [1; 32],
        [2; 16],
        TrustMode::Development,
    )
    .unwrap();
    for requested in [
        ApplicationEnvironment::Staging,
        ApplicationEnvironment::Production,
    ] {
        assert_eq!(
            store
                .scope_for_principal(&developer, requested)
                .unwrap_err()
                .code(),
            EnvironmentErrorCode::CrossEnvironmentDenied
        );
    }
    assert!(
        store
            .scope_for_principal(&developer, ApplicationEnvironment::Development)
            .is_ok()
    );

    let backward = PromotionPlan::build::<ProtectedSecretStatus>(
        PromotionDirection::StagingToProduction,
        "com.example.pos",
        [],
    )
    .unwrap();
    let mismatch = apply_promotion(
        &backward,
        &store.scope(ApplicationEnvironment::Production),
        &store.scope(ApplicationEnvironment::Staging),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert_eq!(
        mismatch.code(),
        EnvironmentErrorCode::CrossEnvironmentDenied
    );
    assert_eq!(
        mismatch.stable_code(),
        "environment.cross_environment_denied"
    );
}

#[test]
fn malformed_inputs_return_stable_safe_codes() {
    let store = EnvironmentDataStore::new("com.example.pos").unwrap();
    assert_eq!(
        EnvironmentDataStore::new("").unwrap_err().stable_code(),
        "environment.request_invalid"
    );
    assert_eq!(
        store
            .scope(ApplicationEnvironment::Staging)
            .key("../escape")
            .unwrap_err()
            .code(),
        EnvironmentErrorCode::RequestInvalid
    );
}

#[test]
fn localstore_and_secret_namespaces_are_separate_and_independent() {
    // Same application and environment produce independent namespaces for data
    // and for protected secrets because the partition digests use distinct
    // domain separators (data: studio.environment.data-partition.v1,
    // credential: studio.protected-secret.partition.v1). Operating on one
    // must never alias or move state belonging to the other.
    let backend = MemoryBackend::default();
    let secret_store = ProtectedSecretStore::new(backend.clone());
    let data_store = EnvironmentDataStore::new("com.example.pos").unwrap();
    let principal = principal("publisher.example", "signing-key-1", "com.example.pos");
    let secret_key =
        ProtectedSecretKey::new("payments.restricted_key", "Authenticate payments").unwrap();

    let staging_data = data_store.scope(ApplicationEnvironment::Staging);
    let staging_secrets = secret_store
        .for_application(&principal, ApplicationEnvironment::Staging)
        .unwrap();

    // Data keys and secret locators for the same logical name must be
    // independent: minting a data key does not affect secret state and
    // configuring a secret does not affect data key admission.
    let data_key = staging_data.key("payments.restricted_key").unwrap();
    assert!(staging_data.admit(&data_key).is_ok());
    assert_eq!(
        staging_secrets.status(&secret_key).unwrap().state(),
        ProtectedSecretState::Missing
    );

    staging_secrets
        .configure(
            &secret_key,
            SecretInput::new(b"rk_staging_secret_material".to_vec()).unwrap(),
        )
        .unwrap();
    assert_eq!(
        staging_secrets.status(&secret_key).unwrap().state(),
        ProtectedSecretState::Configured
    );
    // Data key remains valid in its own namespace after secret configuration.
    assert!(staging_data.admit(&data_key).is_ok());
    assert_eq!(backend.snapshot().len(), 1);

    // Promotion of data records must leave credential material untouched,
    // proving the two namespaces are separate promotion domains.
    let plan = PromotionPlan::build(
        PromotionDirection::StagingToProduction,
        "com.example.pos",
        [staging_secrets.status(&secret_key).unwrap()],
    )
    .unwrap();
    let before = backend.snapshot();
    let mut source_records = BTreeMap::new();
    source_records.insert(
        "payments.restricted_key".to_string(),
        b"data-payload".to_vec(),
    );
    let production_data = data_store.scope(ApplicationEnvironment::Production);
    let (receipt, promoted) =
        apply_promotion(&plan, &staging_data, &production_data, &source_records).unwrap();
    assert_eq!(receipt.data_records_copied, 1);
    assert_eq!(
        promoted.get("payments.restricted_key"),
        Some(&b"data-payload".to_vec())
    );
    assert_eq!(before, backend.snapshot());
    let production_secrets = secret_store
        .for_application(&principal, ApplicationEnvironment::Production)
        .unwrap();
    assert_eq!(
        production_secrets.status(&secret_key).unwrap().state(),
        ProtectedSecretState::Missing
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn promotion_refuses_invalid_source_target_identities() {
    let primary = EnvironmentDataStore::new("com.example.pos").unwrap();
    let other_app = EnvironmentDataStore::new("com.example.other").unwrap();

    // Mismatched application identities must be refused even when the
    // environment direction is correct, including a plan with real entries.
    let cross_backend = MemoryBackend::default();
    let cross_secret_store = ProtectedSecretStore::new(cross_backend);
    let cross_principal = principal("publisher.example", "signing-key-1", "com.example.pos");
    let cross_secret_store = cross_secret_store
        .for_application(&cross_principal, ApplicationEnvironment::Staging)
        .unwrap();
    let cross_secret = declaration();
    cross_secret_store
        .configure(
            &cross_secret,
            SecretInput::new(b"cross-app-provenance-value".to_vec()).unwrap(),
        )
        .unwrap();
    let plan = PromotionPlan::build(
        PromotionDirection::StagingToProduction,
        "com.example.pos",
        [cross_secret_store.status(&cross_secret).unwrap()],
    )
    .unwrap();
    let cross_app = apply_promotion(
        &plan,
        &other_app.scope(ApplicationEnvironment::Staging),
        &other_app.scope(ApplicationEnvironment::Production),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert_eq!(
        cross_app.code(),
        EnvironmentErrorCode::CrossEnvironmentDenied
    );
    assert_eq!(
        cross_app.stable_code(),
        "environment.cross_environment_denied"
    );

    // Same-environment promotion is invalid (direction requires distinct
    // environments).
    let same_env = apply_promotion(
        &plan,
        &primary.scope(ApplicationEnvironment::Staging),
        &primary.scope(ApplicationEnvironment::Staging),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert_eq!(
        same_env.code(),
        EnvironmentErrorCode::CrossEnvironmentDenied
    );

    // Swapped environments relative to the plan direction must be refused.
    let swapped = apply_promotion(
        &plan,
        &primary.scope(ApplicationEnvironment::Production),
        &primary.scope(ApplicationEnvironment::Staging),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert_eq!(swapped.code(), EnvironmentErrorCode::CrossEnvironmentDenied);

    // Development → Production direct jump is not a valid plan direction;
    // the scopes would mismatch either allowed direction.
    let dev_to_prod_plan = PromotionPlan::build::<ProtectedSecretStatus>(
        PromotionDirection::DevelopmentToStaging,
        "com.example.pos",
        [],
    )
    .unwrap();
    let dev_to_prod = apply_promotion(
        &dev_to_prod_plan,
        &primary.scope(ApplicationEnvironment::Development),
        &primary.scope(ApplicationEnvironment::Production),
        &BTreeMap::new(),
    )
    .unwrap_err();
    assert_eq!(
        dev_to_prod.code(),
        EnvironmentErrorCode::CrossEnvironmentDenied
    );

    // Malformed logical record names in the promotion payload must be
    // rejected with RequestInvalid, not silently copied.
    let backend = MemoryBackend::default();
    let secret_store = ProtectedSecretStore::new(backend);
    let principal = principal("publisher.example", "signing-key-1", "com.example.pos");
    let staging_secrets = secret_store
        .for_application(&principal, ApplicationEnvironment::Staging)
        .unwrap();
    let secret =
        ProtectedSecretKey::new("payments.restricted_key", "Authenticate payments").unwrap();
    staging_secrets
        .configure(
            &secret,
            SecretInput::new(b"rk_staging_value".to_vec()).unwrap(),
        )
        .unwrap();
    let realistic_plan = PromotionPlan::build(
        PromotionDirection::StagingToProduction,
        "com.example.pos",
        [staging_secrets.status(&secret).unwrap()],
    )
    .unwrap();
    for bad_logical in ["", "../escape", "bad/key", "a".repeat(257).as_str()] {
        let mut records = BTreeMap::new();
        records.insert(bad_logical.to_string(), b"value".to_vec());
        let err = apply_promotion(
            &realistic_plan,
            &primary.scope(ApplicationEnvironment::Staging),
            &primary.scope(ApplicationEnvironment::Production),
            &records,
        )
        .unwrap_err();
        assert_eq!(
            err.code(),
            EnvironmentErrorCode::RequestInvalid,
            "bad logical {bad_logical:?} should be RequestInvalid"
        );
        assert_eq!(err.stable_code(), "environment.request_invalid");
        if !bad_logical.is_empty() {
            assert!(!format!("{err}").contains(bad_logical));
        }
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn os_credential_backend_evidence_with_explicit_external_gate() {
    // This test exercises the most production-real backend safely available
    // in deterministic CI: the OS credential facility. It must not fake
    // claims about vault availability — when the facility is unavailable
    // (e.g., no Secret Service on headless Linux), the test records the
    // external gap and still passes via the deterministic MemoryBackend
    // proof above.
    let facility = OsCredentialBackend::shipped_facility();
    // On mobile targets no facility is shipped; that is an explicit gap.
    if facility.is_none() {
        eprintln!(
            "external gap: no shipped credential facility on this target — OsCredentialBackend::shipped_facility() is None"
        );
        return;
    }
    let facility = facility.unwrap();
    // Adapter initialization check; D-Bus/Secret Service may still be locked.
    if !OsCredentialBackend::is_available() {
        eprintln!(
            "external gap: OsCredentialBackend unavailable (facility {facility} not initialized) — deterministic MemoryBackend still proves isolation"
        );
        return;
    }

    // Use a process-and-counter identity per test invocation. Unlike a truncated
    // hash, the atomic counter retains its full entropy for concurrent runs.
    let unique = format!(
        "{:x}-{:x}",
        std::process::id(),
        NEXT_OS_TEST_ID.fetch_add(1, Ordering::Relaxed)
    );
    let publisher = format!("publisher.example.os-gate-{unique}");
    let application = format!("com.example.os-gate-{unique}");
    let declaration_name = format!("payments.os_gate_{unique}");
    // ProtectedSecretKey names must be lowercase alphanumeric plus ._- ; hex satisfies.
    let secret_key = ProtectedSecretKey::new(
        declaration_name.clone(),
        "OS gate evidence: staging-only credential",
    )
    .unwrap();

    let backend = OsCredentialBackend;
    let store = ProtectedSecretStore::new(backend);
    let principal = PluginPrincipal::new_verified(
        publisher.clone(),
        format!("signing-key-os-{unique}"),
        application.clone(),
        [9; 32],
        [10; 16],
        TrustMode::Production,
    )
    .unwrap();

    // If the vault is locked or D-Bus unavailable, individual operations
    // will fail with BackendUnavailable — treat that as the external gate
    // rather than a test failure.
    let staging = match store.for_application(&principal, ApplicationEnvironment::Staging) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("external gap: for_application failed with {e:?} — vault unavailable");
            return;
        }
    };
    let production = store
        .for_application(&principal, ApplicationEnvironment::Production)
        .unwrap();
    let mut cleanup = OsVaultCleanup::new(&staging, &production, &secret_key);

    // Purge any leftover from a prior interrupted run before asserting.
    for (environment, result) in [
        ("staging", staging.purge(&secret_key)),
        ("production", production.purge(&secret_key)),
    ] {
        match result {
            Ok(()) => {}
            Err(e) if e.code() == ProtectedSecretErrorCode::BackendUnavailable => {
                eprintln!(
                    "external gap: OsCredentialBackend {environment} purge failed with {e:?} — gate retained"
                );
                return;
            }
            Err(e) => {
                panic!("unexpected error from OsCredentialBackend {environment} purge: {e:?}")
            }
        }
    }

    let staging_value = format!("rk_os_staging_{unique}").into_bytes();
    match staging.configure(
        &secret_key,
        SecretInput::new(staging_value.clone()).unwrap(),
    ) {
        Ok(_) => {}
        Err(e) if e.code() == ProtectedSecretErrorCode::BackendUnavailable => {
            eprintln!(
                "external gap: OsCredentialBackend configure failed with {e:?} (facility {facility} unavailable/locked) — gate retained"
            );
            return;
        }
        Err(e) => panic!("unexpected error from OsCredentialBackend configure: {e:?}"),
    }

    // Secret resolution must follow the active environment, not the package:
    // staging is Configured, production remains Missing when using the real
    // OS facility.
    let staging_status = match staging.status(&secret_key) {
        Ok(s) => s,
        Err(e) if e.code() == ProtectedSecretErrorCode::BackendUnavailable => {
            eprintln!("external gap: status failed with {e:?} — gate retained");
            return;
        }
        Err(e) => panic!("staging status failed: {e:?}"),
    };
    assert_eq!(staging_status.state(), ProtectedSecretState::Configured);
    let production_status = match production.status(&secret_key) {
        Ok(status) => status,
        Err(e) if e.code() == ProtectedSecretErrorCode::BackendUnavailable => {
            eprintln!("external gap: production status failed with {e:?} — gate retained");
            return;
        }
        Err(e) => panic!("production status failed: {e:?}"),
    };
    assert_eq!(production_status.state(), ProtectedSecretState::Missing);

    // Promotion must still copy zero secret bytes even with the real backend:
    // build a plan from value-free status only and prove the production
    // partition stays Missing after promotion.
    let plan = PromotionPlan::build(
        PromotionDirection::StagingToProduction,
        application.clone(),
        [staging_status],
    )
    .unwrap();
    let rendered = format!("{plan:?}");
    assert!(!rendered.contains(&*String::from_utf8_lossy(&staging_value)));

    let data_store = EnvironmentDataStore::new(application.clone()).unwrap();
    let mut source_records = BTreeMap::new();
    source_records.insert("orders.counter".to_string(), b"42".to_vec());
    let (receipt, _) = apply_promotion(
        &plan,
        &data_store.scope(ApplicationEnvironment::Staging),
        &data_store.scope(ApplicationEnvironment::Production),
        &source_records,
    )
    .unwrap();
    assert_eq!(receipt.data_records_copied, 1);
    assert_eq!(
        receipt.secrets_requiring_configuration,
        vec![declaration_name.clone()]
    );

    // Verify the OS backend still shows production Missing (promotion did not
    // move the credential).
    let after = match production.status(&secret_key) {
        Ok(status) => status,
        Err(e) if e.code() == ProtectedSecretErrorCode::BackendUnavailable => {
            eprintln!("external gap: production verification failed with {e:?} — gate retained");
            return;
        }
        Err(e) => panic!("production verification failed: {e:?}"),
    };
    assert_eq!(after.state(), ProtectedSecretState::Missing);

    // Clean up so the developer vault does not retain the test credential.
    match cleanup.cleanup() {
        Ok(()) => eprintln!(
            "production-real evidence: OsCredentialBackend ({facility}) proved isolated staging secret and secret-free promotion"
        ),
        Err(e) => eprintln!(
            "external gap: OsCredentialBackend cleanup failed with {e:?}; vault evidence may remain"
        ),
    }
}
