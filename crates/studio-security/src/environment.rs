//! Active-environment resolution, per-environment application data isolation, and a
//! promotion path that cannot move credential material.
//!
//! # UNVERIFIED
//! - Authored against ticket 28 acceptance criteria; serialized runner has not executed yet.
//! - Boundaries deliberately leave row-scope extension points clean for ticket 24 (RBAC):
//!   [`EnvironmentDataScope`] is the single place a future row-scope axis would attach.

use std::{collections::BTreeMap, error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::protected::{ApplicationEnvironment, ProtectedSecretState, ProtectedSecretStatus};
use crate::{PluginPrincipal, TrustMode};

const DATA_PARTITION_DOMAIN: &[u8] = b"studio.environment.data-partition.v1";
const ACTIVE_ENVIRONMENT_SETTING: &str = "environment.active";

const ENVIRONMENT_LABELS: [(&str, ApplicationEnvironment); 3] = [
    ("development", ApplicationEnvironment::Development),
    ("staging", ApplicationEnvironment::Staging),
    ("production", ApplicationEnvironment::Production),
];

/// Stable, value-free environment-layer failure family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentErrorCode {
    /// Malformed application identifier, logical key, or principal binding.
    RequestInvalid,
    /// No active environment was configured; Studio never assumes one.
    ConfigMissing,
    /// The configured active-environment value was unrecognized.
    ConfigInvalid,
    /// Conflicting active-environment entries were supplied together.
    ConfigAmbiguous,
    /// An attempt crossed an environment boundary and was refused.
    CrossEnvironmentDenied,
}

impl EnvironmentErrorCode {
    /// Stable wire-safe identifier suitable for guest action results.
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::RequestInvalid => "environment.request_invalid",
            Self::ConfigMissing => "environment.config_missing",
            Self::ConfigInvalid => "environment.config_invalid",
            Self::ConfigAmbiguous => "environment.config_ambiguous",
            Self::CrossEnvironmentDenied => "environment.cross_environment_denied",
        }
    }
}

/// Safe environment-layer error without provider or configuration-value context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvironmentError {
    code: EnvironmentErrorCode,
}

impl EnvironmentError {
    const fn new(code: EnvironmentErrorCode) -> Self {
        Self { code }
    }

    /// Stable failure family code.
    #[must_use]
    pub const fn code(self) -> EnvironmentErrorCode {
        self.code
    }

    /// Stable wire-safe identifier.
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        self.code.stable_code()
    }
}

impl fmt::Display for EnvironmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            EnvironmentErrorCode::RequestInvalid => "environment request invalid",
            EnvironmentErrorCode::ConfigMissing => {
                "active environment is not configured; select one explicitly"
            }
            EnvironmentErrorCode::ConfigInvalid => "configured active environment is not valid",
            EnvironmentErrorCode::ConfigAmbiguous => {
                "conflicting active environment configuration entries"
            }
            EnvironmentErrorCode::CrossEnvironmentDenied => {
                "cross-environment access denied"
            }
        })
    }
}

impl Error for EnvironmentError {}

/// Host-protected configuration entries read for active-environment resolution.
///
/// Values arrive from the same trust boundary as the protected store: the host, never guests.
/// Duplicate keys are retained so conflicting selections can be diagnosed instead of silently
/// resolved by last-write-wins.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProtectedConfiguration {
    entries: Vec<(String, String)>,
}

impl ProtectedConfiguration {
    /// Build a configuration from host-read key/value pairs.
    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    /// Every value recorded under one key.
    fn values_for(&self, key: &str) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(entry_key, _)| entry_key == key)
            .map(|(_, value)| value.as_str())
            .collect()
    }
}

/// The resolved active deployment environment plus its provenance-safe origin note.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveEnvironment {
    environment: ApplicationEnvironment,
}

impl ActiveEnvironment {
    /// Selected deployment environment.
    #[must_use]
    pub const fn environment(self) -> ApplicationEnvironment {
        self.environment
    }

    /// Safe diagnostic description that never echoes raw configuration values.
    #[must_use]
    pub const fn safe_description(self) -> &'static str {
        match self.environment {
            ApplicationEnvironment::Development => "active environment: development",
            ApplicationEnvironment::Staging => "active environment: staging",
            ApplicationEnvironment::Production => "active environment: production",
        }
    }
}

/// Resolve the active environment from protected host configuration.
///
/// There is intentionally **no fallback**: a missing, invalid, or ambiguous selection fails with
/// a stable safe code instead of silently pointing at production.
///
/// # Errors
///
/// Returns [`EnvironmentErrorCode::ConfigMissing`] when nothing selects an environment,
/// [`EnvironmentErrorCode::ConfigInvalid`] for an unrecognized value, and
/// [`EnvironmentErrorCode::ConfigAmbiguous`] when multiple conflicting entries exist.
pub fn resolve_active_environment(
    configuration: &ProtectedConfiguration,
) -> Result<ActiveEnvironment, EnvironmentError> {
    let selections = configuration.values_for(ACTIVE_ENVIRONMENT_SETTING);
    let distinct: Vec<&str> = {
        let mut seen: Vec<&str> = Vec::new();
        for selection in &selections {
            if !seen
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(selection))
            {
                seen.push(selection);
            }
        }
        seen
    };
    match distinct.as_slice() {
        [] => Err(EnvironmentError::new(EnvironmentErrorCode::ConfigMissing)),
        [only] => ENVIRONMENT_LABELS
            .iter()
            .find(|(label, _)| only.eq_ignore_ascii_case(label))
            .map(|(_, environment)| ActiveEnvironment {
                environment: *environment,
            })
            .ok_or(EnvironmentError::new(EnvironmentErrorCode::ConfigInvalid)),
        [..] => Err(EnvironmentError::new(EnvironmentErrorCode::ConfigAmbiguous)),
    }
}

/// Host-owned application data layer partitioned per environment.
///
/// Each environment resolves to an independent partition digest, mirroring how
/// [`crate::ProtectedSecretStore`] partitions credentials, so three environments coexist on one
/// machine without sharing application data.
#[derive(Clone)]
pub struct EnvironmentDataStore {
    application: String,
}

impl EnvironmentDataStore {
    /// Bind the data layer to one application identifier.
    ///
    /// # Errors
    ///
    /// Returns [`EnvironmentErrorCode::RequestInvalid`] for empty or oversized identifiers.
    pub fn new(application: impl Into<String>) -> Result<Self, EnvironmentError> {
        let application = application.into();
        if application.is_empty() || application.len() > 256 {
            return Err(EnvironmentError::new(EnvironmentErrorCode::RequestInvalid));
        }
        Ok(Self { application })
    }

    /// Resolve the isolated scope for one environment.
    #[must_use]
    pub fn scope(&self, environment: ApplicationEnvironment) -> EnvironmentDataScope<'_> {
        EnvironmentDataScope {
            store: self,
            environment,
            partition: derive_data_partition(&self.application, environment),
        }
    }

    /// Resolve the scope for the currently active environment.
    ///
    /// # Errors
    ///
    /// Propagates the closed resolution diagnostics from
    /// [`resolve_active_environment`]; there is no default environment.
    pub fn active_scope(
        &self,
        configuration: &ProtectedConfiguration,
    ) -> Result<EnvironmentDataScope<'_>, EnvironmentError> {
        Ok(self.scope(resolve_active_environment(configuration)?.environment()))
    }

    /// Bind a guest-derived principal to its permitted scope.
    ///
    /// Development principals may only address the development environment, matching the
    /// protected-secret admission rule.
    ///
    /// # Errors
    ///
    /// Returns [`EnvironmentErrorCode::CrossEnvironmentDenied`] when a development principal
    /// attempts to address staging or production data.
    pub fn scope_for_principal(
        &self,
        principal: &PluginPrincipal,
        requested: ApplicationEnvironment,
    ) -> Result<EnvironmentDataScope<'_>, EnvironmentError> {
        if principal.trust_mode() == TrustMode::Development
            && requested != ApplicationEnvironment::Development
        {
            return Err(EnvironmentError::new(
                EnvironmentErrorCode::CrossEnvironmentDenied,
            ));
        }
        Ok(self.scope(requested))
    }

    fn application(&self) -> &str {
        &self.application
    }
}

impl fmt::Debug for EnvironmentDataStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EnvironmentDataStore")
            .field("application", &self.application)
            .finish_non_exhaustive()
    }
}

/// One environment's isolated application-data namespace.
///
/// All addressing flows through this scope, so a holder cannot form a key belonging to another
/// environment: keys minted elsewhere are refused with
/// [`EnvironmentErrorCode::CrossEnvironmentDenied`].
#[derive(Clone, Copy)]
pub struct EnvironmentDataScope<'a> {
    store: &'a EnvironmentDataStore,
    environment: ApplicationEnvironment,
    partition: [u8; 32],
}

impl<'a> EnvironmentDataScope<'a> {
    /// This scope's environment.
    #[must_use]
    pub const fn environment(self) -> ApplicationEnvironment {
        self.environment
    }

    /// Mint a partition-bound storage key for a logical name.
    ///
    /// # Errors
    ///
    /// Returns [`EnvironmentErrorCode::RequestInvalid`] for malformed logical keys and
    /// [`EnvironmentErrorCode::CrossEnvironmentDenied`] when the key was minted by a different
    /// environment's scope.
    pub fn key(&self, logical: &str) -> Result<EnvironmentDataKey, EnvironmentError> {
        validate_logical_key(logical)?;
        Ok(EnvironmentDataKey {
            environment: self.environment,
            encoded: encode_hex(&self.partition),
            logical: logical.to_string(),
        })
    }

    /// Accept a key only if it was minted by this exact environment scope.
    ///
    /// # Errors
    ///
    /// Returns [`EnvironmentErrorCode::CrossEnvironmentDenied`] on any environment mismatch.
    pub fn admit(&self, key: &EnvironmentDataKey) -> Result<(), EnvironmentError> {
        let expected = encode_hex(&self.partition);
        if key.environment == self.environment && key.encoded == expected {
            return Ok(());
        }
        Err(EnvironmentError::new(
            EnvironmentErrorCode::CrossEnvironmentDenied,
        ))
    }

    /// Application identifier shared by every environment scope of this store.
    #[must_use]
    pub fn application(self) -> &'a str {
        self.store.application()
    }
}

impl fmt::Debug for EnvironmentDataScope<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EnvironmentDataScope")
            .field("application", &self.store.application())
            .field("environment", &self.environment.label())
            .finish_non_exhaustive()
    }
}

/// Storage key bound to the environment that minted it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentDataKey {
    environment: ApplicationEnvironment,
    encoded: String,
    logical: String,
}

impl EnvironmentDataKey {
    /// Logical name within the minting environment.
    #[must_use]
    pub fn logical(&self) -> &str {
        &self.logical
    }
}

mod sealed {
    /// Marker admitting only value-free metadata into promotion plans.
    pub trait SecretFree {}
}

/// Value-free promotion inputs.
///
/// Sealed and implemented only for [`ProtectedSecretStatus`], whose fields are a validated name,
/// a lifecycle-state enum, and an optional revision counter: no byte buffer reachable from guest
/// code can satisfy this bound, which is the type-system half of the zero-secret-material proof.
pub trait SecretFreeMetadata: sealed::SecretFree {
    /// View of the admitted metadata.
    fn describe(&self) -> PromotionEntry;
}

impl sealed::SecretFree for ProtectedSecretStatus {}
impl SecretFreeMetadata for ProtectedSecretStatus {
    fn describe(&self) -> PromotionEntry {
        PromotionEntry {
            name: self.key().name().to_string(),
            purpose: self.key().purpose().to_string(),
            state: self.state(),
            revision: self.revision(),
        }
    }
}

/// Value-free descriptor copied into a promotion plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionEntry {
    /// Package-declared secret name.
    pub name: String,
    /// Package-declared safe purpose.
    pub purpose: String,
    /// Source-partition lifecycle state.
    pub state: ProtectedSecretState,
    /// Source-partition revision, absent while missing.
    pub revision: Option<u64>,
}

/// Forward-only promotion direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromotionDirection {
    /// Development to staging.
    DevelopmentToStaging,
    /// Staging to production.
    StagingToProduction,
}

impl PromotionDirection {
    /// Source environment of the promotion.
    #[must_use]
    pub const fn source(self) -> ApplicationEnvironment {
        match self {
            Self::DevelopmentToStaging => ApplicationEnvironment::Development,
            Self::StagingToProduction => ApplicationEnvironment::Staging,
        }
    }

    /// Target environment of the promotion.
    #[must_use]
    pub const fn target(self) -> ApplicationEnvironment {
        match self {
            Self::DevelopmentToStaging => ApplicationEnvironment::Staging,
            Self::StagingToProduction => ApplicationEnvironment::Production,
        }
    }
}

/// A reviewed promotion between two environments.
///
/// Construction admits only [`SecretFreeMetadata`] items, and execution touches no credential
/// backend at all: there is no code path from a [`PromotionPlan`] to secret bytes, which is the
/// structural half of the zero-secret-material proof.
#[derive(Clone, Debug)]
pub struct PromotionPlan {
    direction: PromotionDirection,
    entries: Vec<PromotionEntry>,
}

impl PromotionPlan {
    /// Plan a promotion from value-free status metadata only.
    ///
    /// # Errors
    ///
    /// Returns [`EnvironmentErrorCode::RequestInvalid`] for a backward or lateral direction.
    pub fn build<S: SecretFreeMetadata>(
        direction: PromotionDirection,
        statuses: impl IntoIterator<Item = S>,
    ) -> Result<Self, EnvironmentError> {
        let entries: Vec<PromotionEntry> = statuses
            .into_iter()
            .map(|status| status.describe())
            .collect();
        match direction {
            PromotionDirection::DevelopmentToStaging
            | PromotionDirection::StagingToProduction => Ok(Self {
                direction,
                entries,
            }),
        }
    }

    /// Planned direction.
    #[must_use]
    pub const fn direction(&self) -> PromotionDirection {
        self.direction
    }

    /// Value-free entries the operator must act on after promotion.
    #[must_use]
    pub fn entries(&self) -> &[PromotionEntry] {
        self.entries.as_slice()
    }

    /// Names that must receive fresh credentials in the target environment because promotion
    /// carried none.
    #[must_use]
    pub fn requiring_configuration_in_target(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.state == ProtectedSecretState::Configured)
            .map(|entry| entry.name.clone())
            .collect()
    }
}

/// Receipt of a completed promotion; carries names and counts, never values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromotionReceipt {
    /// Direction actually applied.
    pub direction: PromotionDirection,
    /// Number of application-data records copied.
    pub data_records_copied: usize,
    /// Secret declarations that require fresh configuration in the target environment.
    pub secrets_requiring_configuration: Vec<String>,
}

/// Apply a promotion: copy application-data records from the source environment partition to
/// the target environment partition and report which secrets need fresh target-side
/// configuration.
///
/// Credential material is untouched by construction: the plan holds only value-free metadata,
/// and this function receives no credential backend handle, so there is no code path from a
/// [`PromotionPlan`] to secret bytes.
///
/// # Errors
///
/// Returns [`EnvironmentErrorCode::CrossEnvironmentDenied`] if the caller-supplied scopes do not
/// match the plan's direction, or [`EnvironmentErrorCode::RequestInvalid`] for malformed record
/// names.
pub fn apply_promotion(
    plan: &PromotionPlan,
    source: &EnvironmentDataScope<'_>,
    target: &EnvironmentDataScope<'_>,
    source_records: &BTreeMap<String, Vec<u8>>,
) -> Result<(PromotionReceipt, BTreeMap<String, Vec<u8>>), EnvironmentError> {
    if source.environment() != plan.direction().source()
        || target.environment() != plan.direction().target()
    {
        return Err(EnvironmentError::new(
            EnvironmentErrorCode::CrossEnvironmentDenied,
        ));
    }
    let mut copied = 0usize;
    for logical in source_records.keys() {
        let minted = source.key(logical)?;
        target.admit(&minted)?;
        copied += 1;
    }
    Ok((
        PromotionReceipt {
            direction: plan.direction(),
            data_records_copied: copied,
            secrets_requiring_configuration: plan.requiring_configuration_in_target(),
        },
        source_records.clone(),
    ))
}

fn derive_data_partition(application: &str, environment: ApplicationEnvironment) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(DATA_PARTITION_DOMAIN);
    hash_field(&mut hasher, application.as_bytes());
    hash_field(&mut hasher, environment.label().as_bytes());
    hasher.finalize().into()
}

fn validate_logical_key(logical: &str) -> Result<(), EnvironmentError> {
    let valid = !logical.is_empty()
        && logical.len() <= 256
        && logical
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_'));
    if valid {
        Ok(())
    } else {
        Err(EnvironmentError::new(EnvironmentErrorCode::RequestInvalid))
    }
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing into a String cannot fail");
            output
        },
    )
}
