//! Closed Studio bundle manifest-v1 types and host-ceiling validation.

use std::collections::HashSet;

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use studio_net::declaration::RouteGroupDeclaration;

use crate::ManifestError;

/// Manifest resource and input ceilings fixed by the host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManifestPolicy {
    /// Supported manifest schema major.
    pub schema_version: u16,
    /// Supported host–guest protocol major.
    pub protocol_version: u16,
    /// Maximum requested linear memory in MiB.
    pub max_memory_mib: u16,
    /// Maximum requested fuel for one event.
    pub max_event_fuel: u64,
    /// Maximum encoded manifest bytes.
    pub max_manifest_bytes: usize,
}

impl Default for ManifestPolicy {
    fn default() -> Self {
        Self {
            schema_version: 1,
            protocol_version: 1,
            max_memory_mib: 16,
            max_event_fuel: 10_000_000,
            max_manifest_bytes: 64 * 1024,
        }
    }
}

/// Complete closed manifest-v1 wire shape.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManifestV1 {
    /// Exact schema major.
    pub schema_version: u16,
    /// Stable reverse-domain plugin identity.
    pub id: String,
    /// Safe host-visible display name.
    pub name: String,
    /// Semantic plugin release version.
    pub version: String,
    /// Publisher and provisioned signing-key identity.
    pub publisher: Publisher,
    /// Exact wasm archive entry.
    pub entry: String,
    /// SDK semantic version requirement.
    pub sdk_version: String,
    /// Exact host–guest protocol major.
    pub protocol_version: u16,
    /// Closed requested host capabilities.
    pub capabilities: Vec<Capability>,
    /// Requested guest resource ceilings.
    pub limits: BundleLimits,
    /// Ordered declared archive asset paths.
    pub assets: Vec<String>,
    /// Protected values required at runtime, declared without values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<SecretDeclaration>,
    /// Integration-plugin references enabled by this application package.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub integrations: Vec<IntegrationReference>,
    /// Signed host-mediated route groups contributed by enabled integrations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<RouteGroupDeclaration>,
}

/// A version-pinned integration enabled by an application package.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntegrationReference {
    /// Stable integration/plugin id, for example `github`.
    pub id: String,
    /// Descriptor version selected by the package.
    pub version: String,
    /// Public configuration values; secrets remain named declarations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Value>,
}

/// Signed publisher identity.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Publisher {
    /// Publisher identifier provisioned with the host.
    pub id: String,
    /// Provisioned public-key identifier.
    pub key_id: String,
}

/// Signed declaration of one out-of-band protected value.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SecretDeclaration {
    /// Stable lowercase identifier referenced by broker declarations.
    pub name: String,
    /// Safe host-visible explanation of why the value is required.
    pub purpose: String,
}

/// Closed capability catalog for milestone one.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum Capability {
    /// Deterministic host-owned payment simulator.
    #[serde(rename = "payment.simulate")]
    PaymentSimulate,
    /// Host-owned receipt preview simulator.
    #[serde(rename = "printer.simulate")]
    PrinterSimulate,
}

/// Guest-requested limits that cannot exceed host ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BundleLimits {
    /// Linear memory ceiling in MiB.
    #[serde(rename = "memoryMiB")]
    pub memory_mib: u16,
    /// Fuel restored for each event call.
    pub event_fuel: u64,
}

/// Decode and validate one untrusted manifest.
///
/// # Errors
///
/// Returns [`ManifestError`] for byte, JSON, identity, version, path, capability, or limit errors.
pub fn parse_manifest(bytes: &[u8], policy: ManifestPolicy) -> Result<ManifestV1, ManifestError> {
    if bytes.len() > policy.max_manifest_bytes {
        return Err(ManifestError::InvalidJson(
            "manifest byte limit exceeded".to_owned(),
        ));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| ManifestError::InvalidJson(error.to_string()))?;
    preflight_capabilities(&value)?;
    let manifest: ManifestV1 = serde_json::from_slice(bytes)
        .map_err(|error| ManifestError::InvalidJson(error.to_string()))?;
    validate_manifest(&manifest, policy)?;
    Ok(manifest)
}

fn preflight_capabilities(value: &Value) -> Result<(), ManifestError> {
    let Some(capabilities) = value.get("capabilities").and_then(Value::as_array) else {
        return Ok(());
    };
    let mut seen = HashSet::new();
    for capability in capabilities {
        let Some(capability) = capability.as_str() else {
            continue;
        };
        if !matches!(capability, "payment.simulate" | "printer.simulate")
            || !seen.insert(capability)
        {
            return Err(ManifestError::CapabilityInvalid);
        }
    }
    Ok(())
}

fn validate_manifest(manifest: &ManifestV1, policy: ManifestPolicy) -> Result<(), ManifestError> {
    if manifest.schema_version != policy.schema_version {
        return Err(ManifestError::VersionUnsupported {
            field: "schema",
            actual: manifest.schema_version,
        });
    }
    if manifest.protocol_version != policy.protocol_version {
        return Err(ManifestError::VersionUnsupported {
            field: "protocol",
            actual: manifest.protocol_version,
        });
    }
    validate_plugin_id(&manifest.id)?;
    validate_safe_text(&manifest.name, 128, "name")?;
    validate_safe_text(&manifest.publisher.id, 128, "publisher.id")?;
    validate_safe_text(&manifest.publisher.key_id, 128, "publisher.keyId")?;
    Version::parse(&manifest.version).map_err(|_| ManifestError::ManifestInvalid("version"))?;
    VersionReq::parse(&manifest.sdk_version)
        .map_err(|_| ManifestError::ManifestInvalid("sdkVersion"))?;
    if manifest.entry != "module.wasm" {
        return Err(ManifestError::PathInvalid(manifest.entry.clone()));
    }
    validate_assets(&manifest.assets)?;
    validate_secrets(&manifest.secrets)?;
    validate_integrations(&manifest.integrations)?;
    validate_routes(&manifest.routes)?;
    let mut capabilities = HashSet::new();
    if manifest
        .capabilities
        .iter()
        .any(|capability| !capabilities.insert(*capability))
    {
        return Err(ManifestError::CapabilityInvalid);
    }
    if manifest.limits.memory_mib == 0
        || manifest.limits.memory_mib > policy.max_memory_mib
        || manifest.limits.event_fuel == 0
        || manifest.limits.event_fuel > policy.max_event_fuel
    {
        return Err(ManifestError::LimitInvalid);
    }
    Ok(())
}

fn validate_integrations(integrations: &[IntegrationReference]) -> Result<(), ManifestError> {
    let mut ids = HashSet::new();
    for integration in integrations {
        if !valid_integration_id(&integration.id)
            || Version::parse(&integration.version).is_err()
            || !ids.insert(integration.id.as_str())
        {
            return Err(ManifestError::ManifestInvalid("integrations"));
        }
        if let Some(config) = &integration.config
            && serde_json::to_vec(config).map_or(true, |bytes| bytes.len() > 16 * 1024)
        {
            return Err(ManifestError::ManifestInvalid("integrations.config"));
        }
    }
    Ok(())
}

fn validate_routes(routes: &[studio_net::declaration::RouteGroupDeclaration]) -> Result<(), ManifestError> {
    let mut ids = HashSet::new();
    let ceilings = studio_net::limits::BrokerLimits::default();
    for route in routes {
        if !ids.insert(route.id.as_str()) || route.compile(&ceilings).is_err() {
            return Err(ManifestError::ManifestInvalid("routes"));
        }
    }
    Ok(())
}

fn validate_secrets(secrets: &[SecretDeclaration]) -> Result<(), ManifestError> {
    let mut names = HashSet::new();
    for secret in secrets {
        if !valid_secret_name(&secret.name)
            || validate_safe_text(&secret.purpose, 256, "secrets.purpose").is_err()
            || !names.insert(secret.name.as_str())
        {
            return Err(ManifestError::ManifestInvalid("secrets"));
        }
    }
    Ok(())
}

fn valid_secret_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.starts_with(|character: char| character.is_ascii_lowercase())
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        })
}

fn validate_plugin_id(id: &str) -> Result<(), ManifestError> {
    if !valid_plugin_id(id) {
        return Err(ManifestError::ManifestInvalid("id"));
    }
    Ok(())
}

fn valid_plugin_id(id: &str) -> bool {
    let segments: Vec<_> = id.split('.').collect();
    segments.len() >= 2
        && id.len() <= 128
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment.starts_with(char::is_alphanumeric)
                && segment.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
                })
        })
}

fn valid_integration_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.starts_with(|character: char| character.is_ascii_lowercase())
        && id.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-' | '_')
        })
}

fn validate_safe_text(
    value: &str,
    maximum_bytes: usize,
    field: &'static str,
) -> Result<(), ManifestError> {
    if value.is_empty() || value.len() > maximum_bytes || value.chars().any(char::is_control) {
        return Err(ManifestError::ManifestInvalid(field));
    }
    Ok(())
}

fn validate_assets(assets: &[String]) -> Result<(), ManifestError> {
    let mut exact = HashSet::new();
    let mut folded = HashSet::new();
    for asset in assets {
        let valid = asset.starts_with("assets/")
            && !asset.contains(['\\', '\0'])
            && !asset.chars().any(char::is_control)
            && asset
                .split('/')
                .all(|segment| !segment.is_empty() && segment != "." && segment != "..");
        if !valid || !exact.insert(asset.as_str()) || !folded.insert(asset.to_ascii_lowercase()) {
            return Err(ManifestError::PathInvalid(asset.clone()));
        }
    }
    Ok(())
}
