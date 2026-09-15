#![allow(missing_docs)]

use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};

use ed25519_dalek::SigningKey;
use serde_json::json;
use studio_package::{
    ArchivePolicy, CanonicalBundleInput, PackInput, PackMode, TrustStore, TrustedPublisherKey,
    inspect_archive, pack_bundle, verify_bundle_signature,
};
use studio_security::{
    ApplicationEnvironment, BrokerCredentialError, BrokerCredentialSink, BrokerSecretInjector,
    CredentialBackend, CredentialBackendError, CredentialBytes, CredentialLocator, PluginPrincipal,
    ProtectedSecretKey, ProtectedSecretStore, SecretInput, TrustMode,
};

const FIRST_VALUE: &[u8] = b"rk_live_first_out_of_band_value";
const ROTATED_VALUE: &[u8] = b"rk_live_rotated_out_of_band_value";

#[derive(Clone, Default)]
struct MemoryBackend(Arc<Mutex<HashMap<CredentialLocator, Vec<u8>>>>);

impl CredentialBackend for MemoryBackend {
    fn set_secret(
        &self,
        locator: &CredentialLocator,
        secret: &[u8],
    ) -> Result<(), CredentialBackendError> {
        self.0
            .lock()
            .unwrap()
            .insert(locator.clone(), secret.to_vec());
        Ok(())
    }

    fn get_secret(
        &self,
        locator: &CredentialLocator,
    ) -> Result<CredentialBytes, CredentialBackendError> {
        self.0
            .lock()
            .unwrap()
            .get(locator)
            .cloned()
            .map(CredentialBytes::new)
            .ok_or(CredentialBackendError::NotFound)
    }

    fn delete_secret(&self, locator: &CredentialLocator) -> Result<(), CredentialBackendError> {
        self.0
            .lock()
            .unwrap()
            .remove(locator)
            .map(|_| ())
            .ok_or(CredentialBackendError::NotFound)
    }
}

#[test]
fn declared_package_stays_signed_and_value_free_across_out_of_band_rotation() {
    let signing_seed = [7; 32];
    let manifest = serde_json::to_vec(&json!({
        "schemaVersion": 1,
        "id": "com.example.pos",
        "name": "Example POS",
        "version": "0.1.0",
        "publisher": {"id": "example", "keyId": "key-1"},
        "entry": "module.wasm",
        "sdkVersion": "^0.1.0",
        "protocolVersion": 1,
        "capabilities": [],
        "limits": {"memoryMiB": 16, "eventFuel": 1_000_000},
        "assets": [],
        "secrets": [{
            "name": "payments.restricted_key",
            "purpose": "Authenticate bounded payment API requests"
        }]
    }))
    .unwrap();
    let module = b"\0asm\x01\0\0\0".to_vec();
    let package = pack_bundle(PackInput {
        manifest,
        module: module.clone(),
        assets: BTreeMap::new(),
        mode: PackMode::Signed(signing_seed),
    })
    .unwrap();
    let inspected = inspect_archive(&package, ArchivePolicy::default()).unwrap();
    let canonical_input = CanonicalBundleInput {
        manifest: serde_json::from_slice(&inspected.manifest).unwrap(),
        module_path: "module.wasm".to_owned(),
        module,
        assets: BTreeMap::new(),
    };
    let signing_key = SigningKey::from_bytes(&signing_seed);
    let trust_store = TrustStore::from_keys([TrustedPublisherKey {
        publisher_id: "example".to_owned(),
        key_id: "key-1".to_owned(),
        verifying_key: signing_key.verifying_key().to_bytes(),
        enabled: true,
    }])
    .unwrap();
    verify_bundle_signature(
        &canonical_input,
        &inspected.signature,
        "example",
        "key-1",
        &trust_store,
    )
    .unwrap();

    let protected_store = ProtectedSecretStore::new(MemoryBackend::default());
    let principal = PluginPrincipal::new_verified(
        "example",
        "key-1",
        "com.example.pos",
        [3; 32],
        [4; 16],
        TrustMode::Production,
    )
    .unwrap();
    let scope = protected_store
        .for_application(&principal, ApplicationEnvironment::Production)
        .unwrap();
    let declaration = ProtectedSecretKey::new(
        "payments.restricted_key",
        "Authenticate bounded payment API requests",
    )
    .unwrap();
    scope
        .configure(
            &declaration,
            SecretInput::new(FIRST_VALUE.to_vec()).unwrap(),
        )
        .unwrap();
    scope
        .rotate(
            &declaration,
            SecretInput::new(ROTATED_VALUE.to_vec()).unwrap(),
        )
        .unwrap();

    verify_bundle_signature(
        &canonical_input,
        &inspected.signature,
        "example",
        "key-1",
        &trust_store,
    )
    .unwrap();
    let injector = scope.broker_injection_handle([declaration.clone()]);
    let mut sink = ExactSink(ROTATED_VALUE);
    injector
        .inject_at_send_time(&declaration, &mut sink)
        .unwrap();

    for value in [FIRST_VALUE, ROTATED_VALUE] {
        assert!(!contains(&package, value));
        assert!(!contains(&inspected.manifest, value));
    }
}

struct ExactSink(&'static [u8]);

impl BrokerCredentialSink for ExactSink {
    fn inject(&mut self, secret: &[u8]) -> Result<(), BrokerCredentialError> {
        assert_eq!(secret, self.0);
        Ok(())
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
