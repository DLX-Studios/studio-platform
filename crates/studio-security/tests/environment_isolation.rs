#![allow(missing_docs)]

use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};

use studio_security::{
    ApplicationEnvironment, CredentialBackend, CredentialBackendError, CredentialBytes,
    CredentialLocator, EnvironmentDataStore, EnvironmentErrorCode, PluginPrincipal,
    PromotionDirection, PromotionPlan, ProtectedConfiguration, ProtectedSecretKey,
    ProtectedSecretState, ProtectedSecretStatus, ProtectedSecretStore, SecretInput, TrustMode,
    apply_promotion, resolve_active_environment,
};

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

    let backward =
        PromotionPlan::build::<ProtectedSecretStatus>(PromotionDirection::StagingToProduction, [])
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
