//! Production admission checks shared by full and minimal runtime builds.

use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use studio_package::{
    ArchiveFiles, ArchivePolicy, CanonicalBundleInput, TrustStore, build_archive,
    canonical_bundle_document,
};
use studio_runtime::{
    StudioHost,
    cli::LaunchRequest,
    host::{HostConfig, LaunchErrorCode, NativeDisplayAvailability},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Bundle(PathBuf);
impl Drop for Bundle {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn manifest() -> Value {
    json!({
        "schemaVersion": 1, "id": "com.example.core", "name": "Core fixture",
        "version": "0.1.0", "publisher": {"id": "example", "keyId": "key-1"},
        "entry": "module.wasm", "sdkVersion": "^0.1.0", "protocolVersion": 1,
        "capabilities": [], "limits": {"memoryMiB": 16, "eventFuel": 10_000_000}, "assets": []
    })
}

#[allow(clippy::format_collect)]
fn fixture(manifest: Value, valid_signature: bool) -> (StudioHost, Bundle) {
    // Serialize the public protocol types so the fixture follows the real wire representation.
    let mount = studio_protocol::GuestMessage::Mount(studio_protocol::MountTree {
        protocol_version: 1,
        route: "/".into(),
        root: studio_protocol::UiNode {
            id: "root".into(),
            kind: studio_protocol::NodeKind::Column,
            props: BTreeMap::new(),
            children: vec![],
        },
    });
    let mount = serde_json::to_string(&mount).unwrap();
    let encoded = serde_json::to_string(&mount).unwrap();
    let module = wat::parse_str(format!(
        r#"(module
        (import "studio_host" "emit" (func $emit (param i32 i32) (result i32)))
        (memory (export "memory") 1 1) (table 1 1 funcref)
        (data (i32.const 0) {encoded})
        (func (export "studio_alloc") (param i32) (result i32) i32.const 4096)
        (func (export "studio_dealloc") (param i32 i32))
        (func (export "studio_init") (param i32 i32) (result i32)
            i32.const 0 i32.const {} call $emit)
        (func (export "studio_event") (param i32 i32) (result i32) i32.const 0))"#,
        mount.len()
    ))
    .unwrap();
    let key = SigningKey::from_bytes(&[7; 32]);
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    let input = CanonicalBundleInput {
        manifest,
        module_path: "module.wasm".into(),
        module: module.clone(),
        assets: BTreeMap::new(),
    };
    let mut signature = key
        .sign(&canonical_bundle_document(&input).unwrap())
        .to_bytes();
    if !valid_signature {
        signature[0] ^= 1;
    }
    let archive = build_archive(
        &ArchiveFiles {
            manifest: manifest_bytes,
            module,
            signature: signature.to_vec(),
            assets: BTreeMap::new(),
        },
        ArchivePolicy::default(),
    )
    .unwrap();
    let path = std::env::temp_dir().join(format!(
        "studio-core-{}-{}.studio",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, archive).unwrap();
    let public: String = key
        .verifying_key()
        .to_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let trust = TrustStore::from_json_at(&serde_json::to_vec(&json!({
        "schemaVersion":1, "snapshotId":"core-test", "version":1, "validFrom":100, "expiresAt":300,
        "keys":[{"publisherId":"example", "keyId":"key-1", "publicKey":public, "validFrom":100, "expiresAt":300}], "revocations":[]
    })).unwrap(), 200).unwrap();
    (
        StudioHost::new(HostConfig::new(trust), NativeDisplayAvailability::Available),
        Bundle(path),
    )
}

fn request(bundle: &Bundle) -> LaunchRequest {
    LaunchRequest::parse_runtime_from([
        "studio-app".into(),
        "--bundle".into(),
        bundle.0.clone().into_os_string(),
    ])
    .unwrap()
}

#[test]
fn signed_bundle_mounts_in_core_and_full_hosts() {
    let (host, bundle) = fixture(manifest(), true);
    let surface = host.prepare(request(&bundle)).unwrap();
    assert_eq!(surface.registry().root_id(), Some("root"));
    assert!(surface.warning().is_none());
}

#[test]
fn core_and_full_hosts_reject_invalid_signatures() {
    let (host, bundle) = fixture(manifest(), false);
    assert_eq!(
        host.prepare(request(&bundle)).unwrap_err().code(),
        LaunchErrorCode::IntegrityInvalid
    );
}

#[cfg(not(feature = "services"))]
#[test]
fn core_host_rejects_service_requirements_before_running_guest() {
    let mut manifest = manifest();
    manifest["secrets"] = json!([{"name":"service.token", "purpose":"Fixture service credential"}]);
    let (host, bundle) = fixture(manifest, true);
    assert_eq!(
        host.prepare(request(&bundle)).unwrap_err().code(),
        LaunchErrorCode::ProviderConfigurationInvalid
    );
}
