#![allow(missing_docs)]

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use ed25519_dalek::{Signer, SigningKey};
use serde_json::json;
use studio_app::{
    cli::{LaunchMode, LaunchRequest},
    host::{HostConfig, LaunchErrorCode, StudioHost, WaylandAvailability},
    plugin_surface::DEVELOPMENT_WARNING,
};
use studio_components::InputAction;
use studio_package::{
    ArchiveFiles, ArchivePolicy, CanonicalBundleInput, TrustStore, TrustedPublisherKey,
    build_archive, canonical_bundle_document,
};
use studio_protocol::{GuestMessage, HostEvent, MountTree, NodeKind, UiNode};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct FixtureDirectory(PathBuf);

impl FixtureDirectory {
    fn new() -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "studio-plugin-launch-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for FixtureDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn mount_message() -> Vec<u8> {
    let node = |id: &str, kind: NodeKind, label: Option<&str>| UiNode {
        id: id.to_owned(),
        kind,
        props: label
            .map(|label| BTreeMap::from([("label".to_owned(), json!(label))]))
            .unwrap_or_default(),
        children: Vec::new(),
    };
    serde_json::to_vec(&GuestMessage::Mount(MountTree {
        protocol_version: 1,
        route: "/catalog".to_owned(),
        root: UiNode {
            id: "root".to_owned(),
            kind: NodeKind::Column,
            props: BTreeMap::new(),
            children: vec![node("checkout", NodeKind::Button, Some("Checkout"))],
        },
    }))
    .unwrap()
}

fn plugin_module() -> Vec<u8> {
    let mount = String::from_utf8(mount_message()).unwrap();
    let encoded_mount = serde_json::to_string(&mount).unwrap();
    wat::parse_str(format!(
        r#"(module
          (import "studio_host" "emit" (func $emit (param i32 i32) (result i32)))
          (memory (export "memory") 1 1)
          (table 1 1 funcref)
          (data (i32.const 0) {encoded_mount})
          (func (export "studio_alloc") (param i32) (result i32) i32.const 4096)
          (func (export "studio_dealloc") (param i32 i32))
          (func (export "studio_init") (param i32 i32) (result i32)
            i32.const 0 i32.const {mount_length} call $emit)
          (func (export "studio_event") (param i32 i32) (result i32) i32.const 0))"#,
        mount_length = mount.len(),
    ))
    .unwrap()
}

fn manifest() -> serde_json::Value {
    json!({
        "schemaVersion": 1,
        "id": "com.example.pos",
        "name": "Example POS",
        "version": "0.1.0",
        "publisher": {"id": "example", "keyId": "key-1"},
        "entry": "module.wasm",
        "sdkVersion": "^0.1.0",
        "protocolVersion": 1,
        "capabilities": [],
        "limits": {"memoryMiB": 16, "eventFuel": 10_000_000},
        "assets": []
    })
}

fn signed_bundle(module: Vec<u8>, signing_key: &SigningKey, valid_signature: bool) -> Vec<u8> {
    let manifest = manifest();
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    let input = CanonicalBundleInput {
        manifest,
        module_path: "module.wasm".to_owned(),
        module: module.clone(),
        assets: BTreeMap::new(),
    };
    let mut signature = signing_key
        .sign(&canonical_bundle_document(&input).unwrap())
        .to_bytes();
    if !valid_signature {
        signature[0] ^= 1;
    }
    build_archive(
        &ArchiveFiles {
            manifest: manifest_bytes,
            module,
            signature: signature.to_vec(),
            assets: BTreeMap::new(),
        },
        ArchivePolicy::default(),
    )
    .unwrap()
}

fn trust(signing_key: &SigningKey) -> TrustStore {
    TrustStore::from_keys([TrustedPublisherKey {
        publisher_id: "example".to_owned(),
        key_id: "key-1".to_owned(),
        verifying_key: signing_key.verifying_key().to_bytes(),
        enabled: true,
    }])
    .unwrap()
}

fn provisioned_trust(signing_key: &SigningKey) -> TrustStore {
    let public_key = signing_key
        .verifying_key()
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let snapshot = json!({
        "schemaVersion": 1,
        "snapshotId": "test-launch-1",
        "version": 1,
        "validFrom": 100,
        "expiresAt": 300,
        "keys": [{
            "publisherId": "example",
            "keyId": "key-1",
            "publicKey": public_key,
            "validFrom": 100,
            "expiresAt": 300
        }],
        "revocations": []
    });
    TrustStore::from_json_at(&serde_json::to_vec(&snapshot).unwrap(), 200).unwrap()
}

fn production_request(path: &Path) -> LaunchRequest {
    LaunchRequest::parse_from(["studio", "--bundle", path.to_str().unwrap()]).unwrap()
}

#[test]
fn production_requires_an_absolute_regular_file_selection() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let host = StudioHost::new(
        HostConfig::new(trust(&signing_key)),
        WaylandAvailability::Available,
    );
    let relative = LaunchRequest::parse_from(["studio", "--bundle", "relative.studio"]).unwrap();
    assert_eq!(
        host.prepare(relative).unwrap_err().code(),
        LaunchErrorCode::PathInvalid
    );

    let fixtures = FixtureDirectory::new();
    let directory = production_request(&fixtures.0);
    assert_eq!(
        host.prepare(directory).unwrap_err().code(),
        LaunchErrorCode::PathInvalid
    );
}

#[test]
fn valid_signed_bundle_mounts_before_exposing_the_surface() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "valid.studio",
        &signed_bundle(plugin_module(), &signing_key, true),
    );
    let host = StudioHost::new(
        HostConfig::new(provisioned_trust(&signing_key)),
        WaylandAvailability::Available,
    );
    let surface = host.prepare(production_request(&path)).unwrap();

    assert_eq!(surface.mode(), LaunchMode::Production);
    assert_eq!(surface.registry().root_id(), Some("root"));
    assert!(surface.warning().is_none());
}

#[test]
fn production_rejects_empty_default_trust_before_guest_execution() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "valid.studio",
        &signed_bundle(plugin_module(), &signing_key, true),
    );
    let host = StudioHost::new(
        HostConfig::new(TrustStore::default()),
        WaylandAvailability::Available,
    );
    assert_eq!(
        host.prepare(production_request(&path)).unwrap_err().code(),
        LaunchErrorCode::TrustConfigurationInvalid
    );
}

#[test]
fn signature_mutation_is_rejected_before_guest_execution() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "mutated.studio",
        &signed_bundle(plugin_module(), &signing_key, false),
    );
    let host = StudioHost::new(
        HostConfig::new(trust(&signing_key)),
        WaylandAvailability::Available,
    );
    assert_eq!(
        host.prepare(production_request(&path)).unwrap_err().code(),
        LaunchErrorCode::IntegrityInvalid
    );
}

#[test]
fn unsigned_development_launch_requires_explicit_dev_and_keeps_warning() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "unsigned.studio",
        &signed_bundle(plugin_module(), &signing_key, false),
    );
    let request = LaunchRequest::parse_from(["studio", "--dev", path.to_str().unwrap()]).unwrap();
    let host = StudioHost::new(
        HostConfig::new(TrustStore::default()),
        WaylandAvailability::Available,
    );
    let surface = host.prepare(request).unwrap();
    assert_eq!(surface.mode(), LaunchMode::Development);
    assert_eq!(surface.warning(), Some(DEVELOPMENT_WARNING));
}

#[test]
fn no_wayland_session_rejects_launch_without_x11_fallback() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "valid.studio",
        &signed_bundle(plugin_module(), &signing_key, true),
    );
    let host = StudioHost::new(
        HostConfig::new(trust(&signing_key)),
        WaylandAvailability::Unavailable,
    );
    assert_eq!(
        host.prepare(production_request(&path)).unwrap_err().code(),
        LaunchErrorCode::WaylandUnavailable
    );
}

#[test]
fn mounted_catalog_accepts_pointer_and_keyboard_activation() {
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let fixtures = FixtureDirectory::new();
    let path = fixtures.write(
        "valid.studio",
        &signed_bundle(plugin_module(), &signing_key, true),
    );
    let host = StudioHost::new(
        HostConfig::new(trust(&signing_key)),
        WaylandAvailability::Available,
    );
    let surface = host.prepare(production_request(&path)).unwrap();
    for action in [InputAction::PointerClick, InputAction::KeyboardActivate] {
        let HostEvent::Ui(event) = surface.dispatch_input("checkout", action).unwrap() else {
            panic!("expected UI event");
        };
        assert_eq!(event.event, "pressed");
    }
}
