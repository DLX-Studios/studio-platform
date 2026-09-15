#![allow(missing_docs)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use studio_security::{
    ApplicationEnvironment, BrokerCredentialError, BrokerCredentialSink, BrokerSecretInjector,
    CredentialBackend, CredentialBackendError, CredentialBytes, CredentialLocator,
    GuestSecretStatusApi, PluginPrincipal, ProtectedSecretErrorCode, ProtectedSecretKey,
    ProtectedSecretState, ProtectedSecretStore, SecretInput, TrustMode,
};

#[derive(Clone, Default)]
struct MemoryBackend {
    records: Arc<Mutex<HashMap<CredentialLocator, Vec<u8>>>>,
}

impl MemoryBackend {
    fn record_count(&self) -> usize {
        self.records.lock().unwrap().len()
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

fn principal(publisher: &str, key: &str, application: &str, generation: u8) -> PluginPrincipal {
    PluginPrincipal::new_verified(
        publisher,
        key,
        application,
        [generation; 32],
        [generation; 16],
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

#[test]
fn verified_publisher_application_and_environment_form_non_oracular_partitions() {
    let backend = MemoryBackend::default();
    let store = ProtectedSecretStore::new(backend.clone());
    let secret = declaration();
    let original = principal("publisher.example", "signing-key-1", "com.example.pos", 1);

    store
        .for_application(&original, ApplicationEnvironment::Production)
        .unwrap()
        .configure(
            &secret,
            SecretInput::new(b"rk_live_app_specific_1".to_vec()).unwrap(),
        )
        .unwrap();

    let updated_bundle = principal("publisher.example", "signing-key-2", "com.example.pos", 2);
    let stable_scope = store
        .for_application(&updated_bundle, ApplicationEnvironment::Production)
        .unwrap();
    assert_eq!(
        stable_scope.status(&secret).unwrap().state(),
        ProtectedSecretState::Configured
    );

    for (foreign, environment) in [
        (
            principal("publisher.example", "signing-key-1", "com.example.other", 1),
            ApplicationEnvironment::Production,
        ),
        (
            principal("publisher.other", "signing-key-1", "com.example.pos", 1),
            ApplicationEnvironment::Production,
        ),
        (original.clone(), ApplicationEnvironment::Staging),
    ] {
        let foreign_scope = store.for_application(&foreign, environment).unwrap();
        let guest = foreign_scope.guest_status_handle([secret.clone()]);
        assert_eq!(
            guest.secret_status(&secret).unwrap().state(),
            ProtectedSecretState::Missing
        );
        let injector = foreign_scope.broker_injection_handle([secret.clone()]);
        let mut sink = ExpectedSink::new(b"never injected");
        assert_eq!(
            injector
                .inject_at_send_time(&secret, &mut sink)
                .unwrap_err()
                .code(),
            ProtectedSecretErrorCode::SecretUnavailable
        );
        assert_eq!(sink.calls, 0);
    }

    assert_eq!(backend.record_count(), 1);
}

#[test]
fn guest_handle_exposes_status_while_broker_handle_injects_only_at_send_time() {
    let store = ProtectedSecretStore::new(MemoryBackend::default());
    let principal = principal("publisher.example", "signing-key-1", "com.example.pos", 1);
    let scope = store
        .for_application(&principal, ApplicationEnvironment::Production)
        .unwrap();
    let secret = declaration();
    let undeclared = ProtectedSecretKey::new("other.key", "Authenticate another service").unwrap();
    let guest = scope.guest_status_handle([secret.clone()]);

    assert_eq!(
        guest.secret_status(&secret).unwrap().state(),
        ProtectedSecretState::Missing
    );
    assert_eq!(
        guest.secret_status(&undeclared).unwrap_err().code(),
        ProtectedSecretErrorCode::RequestInvalid
    );

    let first = b"rk_live_first_app_value";
    let configured = scope
        .configure(&secret, SecretInput::new(first.to_vec()).unwrap())
        .unwrap();
    assert_eq!(configured.state(), ProtectedSecretState::Configured);
    assert_eq!(configured.revision(), Some(1));
    assert!(!format!("{configured:?}").contains(std::str::from_utf8(first).unwrap()));

    let second = b"rk_live_rotated_app_value";
    let rotated = scope
        .rotate(&secret, SecretInput::new(second.to_vec()).unwrap())
        .unwrap();
    assert_eq!(rotated.revision(), Some(2));

    let injector = scope.broker_injection_handle([secret.clone()]);
    let mut sink = ExpectedSink::new(second);
    injector.inject_at_send_time(&secret, &mut sink).unwrap();
    assert_eq!(sink.calls, 1);

    let revoked = scope.revoke(&secret).unwrap();
    assert_eq!(revoked.state(), ProtectedSecretState::Revoked);
    assert_eq!(revoked.revision(), Some(3));
    assert_eq!(
        guest.secret_status(&secret).unwrap().state(),
        ProtectedSecretState::Revoked
    );
    assert_eq!(
        injector
            .inject_at_send_time(&secret, &mut sink)
            .unwrap_err()
            .code(),
        ProtectedSecretErrorCode::SecretUnavailable
    );
    assert_eq!(sink.calls, 1);

    scope.purge(&secret).unwrap();
    assert_eq!(
        guest.secret_status(&secret).unwrap().state(),
        ProtectedSecretState::Missing
    );
}

#[test]
fn development_identity_cannot_address_staging_or_production_credentials() {
    let principal = PluginPrincipal::new_verified(
        "publisher.local",
        "developer-key",
        "com.example.local",
        [1; 32],
        [2; 16],
        TrustMode::Development,
    )
    .unwrap();
    let store = ProtectedSecretStore::new(MemoryBackend::default());

    assert!(
        store
            .for_application(&principal, ApplicationEnvironment::Development)
            .is_ok()
    );
    for environment in [
        ApplicationEnvironment::Staging,
        ApplicationEnvironment::Production,
    ] {
        assert_eq!(
            store
                .for_application(&principal, environment)
                .unwrap_err()
                .code(),
            ProtectedSecretErrorCode::RequestInvalid
        );
    }
}

struct ExpectedSink<'a> {
    expected: &'a [u8],
    calls: usize,
}

impl<'a> ExpectedSink<'a> {
    const fn new(expected: &'a [u8]) -> Self {
        Self { expected, calls: 0 }
    }
}

impl BrokerCredentialSink for ExpectedSink<'_> {
    fn inject(&mut self, secret: &[u8]) -> Result<(), BrokerCredentialError> {
        assert_eq!(secret, self.expected);
        self.calls += 1;
        Ok(())
    }
}
