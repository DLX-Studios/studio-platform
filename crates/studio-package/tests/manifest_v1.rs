#![allow(missing_docs)]

use serde_json::{Value, json};
use studio_package::{Capability, ManifestErrorCode, ManifestPolicy, parse_manifest};

fn valid_manifest() -> Value {
    json!({
        "schemaVersion": 1,
        "id": "com.example.pos",
        "name": "Example POS",
        "version": "0.1.0",
        "publisher": {"id": "example", "keyId": "dev-example-1"},
        "entry": "module.wasm",
        "sdkVersion": "^0.1.0",
        "protocolVersion": 1,
        "capabilities": ["payment.simulate", "printer.simulate"],
        "limits": {"memoryMiB": 16, "eventFuel": 10_000_000},
        "assets": ["assets/catalog.json"]
    })
}

fn decode(value: &Value) -> Result<studio_package::ManifestV1, studio_package::ManifestError> {
    parse_manifest(
        &serde_json::to_vec(value).unwrap(),
        ManifestPolicy::default(),
    )
}

#[test]
fn accepts_the_complete_closed_manifest_and_capability_catalog() {
    let manifest = decode(&valid_manifest()).unwrap();
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.id.as_str(), "com.example.pos");
    assert_eq!(manifest.publisher.id.as_str(), "example");
    assert_eq!(manifest.entry.as_str(), "module.wasm");
    assert_eq!(
        manifest.capabilities,
        [Capability::PaymentSimulate, Capability::PrinterSimulate]
    );
    assert_eq!(manifest.assets, ["assets/catalog.json"]);
}

#[test]
fn rejects_missing_unknown_duplicate_and_noncanonical_input_fields() {
    let mut missing = valid_manifest();
    missing.as_object_mut().unwrap().remove("publisher");
    assert_eq!(
        decode(&missing).unwrap_err().code(),
        ManifestErrorCode::InvalidJson
    );

    for (pointer, key) in [
        ("", "unknown"),
        ("/publisher", "unknown"),
        ("/limits", "unknown"),
    ] {
        let mut value = valid_manifest();
        value
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.to_owned(), json!(true));
        assert_eq!(
            decode(&value).unwrap_err().code(),
            ManifestErrorCode::InvalidJson
        );
    }

    let duplicate = br#"{
      "schemaVersion":1,"schemaVersion":1,"id":"com.example.pos","name":"Example POS",
      "version":"0.1.0","publisher":{"id":"example","keyId":"key"},"entry":"module.wasm",
      "sdkVersion":"^0.1.0","protocolVersion":1,"capabilities":[],
      "limits":{"memoryMiB":16,"eventFuel":10000000},"assets":[]
    }"#;
    assert_eq!(
        parse_manifest(duplicate, ManifestPolicy::default())
            .unwrap_err()
            .code(),
        ManifestErrorCode::InvalidJson
    );

    let mut fractional_integer = valid_manifest();
    fractional_integer["limits"]["eventFuel"] = json!(10_000_000.5);
    assert_eq!(
        decode(&fractional_integer).unwrap_err().code(),
        ManifestErrorCode::InvalidJson
    );
}

#[test]
fn rejects_invalid_identity_versions_and_display_text() {
    for (pointer, invalid) in [
        ("/id", json!("example")),
        ("/id", json!("com..example")),
        ("/name", json!("")),
        ("/name", json!("bad\nname")),
        ("/version", json!("not-semver")),
        ("/publisher/id", json!("")),
        ("/publisher/keyId", json!("")),
        ("/sdkVersion", json!("not-a-requirement")),
    ] {
        let mut value = valid_manifest();
        *value.pointer_mut(pointer).unwrap() = invalid;
        assert_eq!(
            decode(&value).unwrap_err().code(),
            ManifestErrorCode::ManifestInvalid
        );
    }

    for (field, version) in [("schemaVersion", 2), ("protocolVersion", 2)] {
        let mut value = valid_manifest();
        value[field] = json!(version);
        assert_eq!(
            decode(&value).unwrap_err().code(),
            ManifestErrorCode::VersionUnsupported
        );
    }
}

#[test]
fn rejects_unknown_duplicate_capabilities_and_values_above_host_ceilings() {
    for capabilities in [
        json!(["network.raw"]),
        json!(["payment.simulate", "payment.simulate"]),
    ] {
        let mut value = valid_manifest();
        value["capabilities"] = capabilities;
        assert_eq!(
            decode(&value).unwrap_err().code(),
            ManifestErrorCode::CapabilityInvalid
        );
    }

    for (field, invalid) in [("memoryMiB", json!(17)), ("eventFuel", json!(10_000_001))] {
        let mut value = valid_manifest();
        value["limits"][field] = invalid;
        assert_eq!(
            decode(&value).unwrap_err().code(),
            ManifestErrorCode::LimitInvalid
        );
    }
}

#[test]
fn rejects_inconsistent_unsafe_and_case_colliding_entry_asset_paths() {
    for entry in [
        "other.wasm",
        "/module.wasm",
        "../module.wasm",
        "module\\wasm",
    ] {
        let mut value = valid_manifest();
        value["entry"] = json!(entry);
        assert_eq!(
            decode(&value).unwrap_err().code(),
            ManifestErrorCode::PathInvalid
        );
    }

    for assets in [
        json!(["catalog.json"]),
        json!(["assets/../catalog.json"]),
        json!(["assets/catalog.json", "assets/catalog.json"]),
        json!(["assets/Catalog.json", "assets/catalog.json"]),
        json!(["module.wasm"]),
    ] {
        let mut value = valid_manifest();
        value["assets"] = assets;
        assert_eq!(
            decode(&value).unwrap_err().code(),
            ManifestErrorCode::PathInvalid
        );
    }
}
