//! Artifact fingerprints: versioned cache keys binding build outputs to
//! every toolchain input. Identical fingerprints yield byte-identical
//! artifacts; any input drift changes the key.
use sha2::{Digest, Sha256};

/// One named toolchain input version.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolchainInput {
    /// Input name (for example `studio-ir-version`).
    pub name: String,
    /// Input version or revision.
    pub version: String,
}

/// Versioned fingerprint of one buildable artifact.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtifactFingerprint {
    /// Lowercase hex SHA-256 over the canonical input record.
    pub key: String,
    /// Fingerprint record version.
    pub record_version: u16,
}

/// Fingerprint record version.
pub const FINGERPRINT_RECORD_VERSION: u16 = 1;

/// Compute the fingerprint for one module: source bytes, sorted dependency
/// fingerprints, and every toolchain input version.
///
/// # Errors
///
/// This function cannot fail; it returns the key directly.
#[must_use]
pub fn fingerprint_module(
    source: &[u8],
    dependencies: &[(&str, &str)],
    inputs: &[ToolchainInput],
) -> ArtifactFingerprint {
    let mut record = String::new();
    record.push_str("studio-artifact-fingerprint-v1\n");
    let mut dependencies: Vec<(&str, &str)> = dependencies.to_vec();
    dependencies.sort_unstable();
    for (id, key) in dependencies {
        record.push_str("dep:");
        record.push_str(id);
        record.push('=');
        record.push_str(key);
        record.push('\n');
    }
    let mut inputs: Vec<(&str, &str)> = inputs
        .iter()
        .map(|input| (input.name.as_str(), input.version.as_str()))
        .collect();
    inputs.sort_unstable();
    for (name, version) in inputs {
        record.push_str("tool:");
        record.push_str(name);
        record.push('=');
        record.push_str(version);
        record.push('\n');
    }
    record.push_str("source-sha256:");
    record.push_str(&hex_sha256(source));
    record.push('\n');
    ArtifactFingerprint {
        key: hex_sha256(record.as_bytes()),
        record_version: FINGERPRINT_RECORD_VERSION,
    }
}

/// Standard toolchain inputs for a `.studio` module build.
#[must_use]
pub fn studio_toolchain_inputs(
    adapter: &crate::rsvelte_adapter::AdapterFingerprint,
    asc_version: &str,
    asc_options: &str,
    target_abi: &str,
) -> Vec<ToolchainInput> {
    vec![
        ToolchainInput {
            name: "studio-ir-version".to_owned(),
            version: crate::ir::STUDIO_IR_VERSION.to_string(),
        },
        ToolchainInput {
            name: "studio-script-version".to_owned(),
            version: crate::STUDIO_SCRIPT_VERSION.to_string(),
        },
        ToolchainInput {
            name: "protocol-version".to_owned(),
            version: studio_protocol::PROTOCOL_VERSION.to_string(),
        },
        ToolchainInput {
            name: "rsvelte-facade".to_owned(),
            version: adapter.facade_version.clone(),
        },
        ToolchainInput {
            name: "rsvelte-compiler".to_owned(),
            version: adapter.compiler_version.clone(),
        },
        ToolchainInput {
            name: "rsvelte-svelte".to_owned(),
            version: adapter.svelte_version.clone(),
        },
        ToolchainInput {
            name: "asc-version".to_owned(),
            version: asc_version.to_owned(),
        },
        ToolchainInput {
            name: "asc-options".to_owned(),
            version: asc_options.to_owned(),
        },
        ToolchainInput {
            name: "target-abi".to_owned(),
            version: target_abi.to_owned(),
        },
    ]
}

fn hex_sha256(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let digest = Sha256::digest(bytes);
    digest
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> Vec<ToolchainInput> {
        vec![
            ToolchainInput {
                name: "b".to_owned(),
                version: "2".to_owned(),
            },
            ToolchainInput {
                name: "a".to_owned(),
                version: "1".to_owned(),
            },
        ]
    }

    #[test]
    fn identical_inputs_produce_identical_keys() {
        let first = fingerprint_module(b"source", &[("dep", "key")], &inputs());
        let second = fingerprint_module(b"source", &[("dep", "key")], &inputs());
        assert_eq!(first.key, second.key);
        assert_eq!(first.key.len(), 64);
        assert_eq!(first.record_version, FINGERPRINT_RECORD_VERSION);
    }

    #[test]
    fn any_input_drift_changes_the_key() {
        let base = fingerprint_module(b"source", &[], &inputs()).key;
        assert_ne!(
            fingerprint_module(b"source!", &[], &inputs()).key,
            base,
            "source drift"
        );
        assert_ne!(
            fingerprint_module(b"source", &[("dep", "other")], &inputs()).key,
            base,
            "dependency drift"
        );
        assert_ne!(
            fingerprint_module(
                b"source",
                &[],
                &[ToolchainInput {
                    name: "a".to_owned(),
                    version: "9".to_owned()
                }]
            )
            .key,
            base,
            "toolchain drift"
        );
    }
}
