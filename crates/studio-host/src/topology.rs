//! Host-owned center/station topology and offline operation replay.
//!
//! A center is the authority for shared application records. Stations retain a
//! bounded materialized cache and a transient outbound queue while disconnected;
//! they never become an authority for shared records. The in-process center is
//! deliberately transport-neutral so an on-premises hub and a Studio Cloud
//! namespace can use the same protocol. [`PersistentCenter`] binds the center
//! state to the host-owned [`LocalStore`] durability boundary.

use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{LocalStore, StoreBatch, StoreBatchEntry};

const TOKEN_BYTES: usize = 32;
const TOKEN_TTL: u64 = 100;
const STATE_VERSION: u32 = 1;
const STATE_BATCH_PREFIX: &str = "studio-center-topology-v1:";
const MAX_VALUE_BYTES: usize = 256 * 1024;

/// Stable identity of one center authority.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct CenterId(String);

impl CenterId {
    /// Create a validated center identity.
    pub fn new(value: impl Into<String>) -> Result<Self, TopologyError> {
        let value = value.into();
        valid_identifier(&value, 128).then_some(Self(value)).ok_or_else(|| {
            TopologyError::new(TopologyErrorCode::InvalidIdentifier)
        })
    }

    /// Return the stable center identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable identity of one enrolled station.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct StationId(String);

impl StationId {
    /// Create a validated station identity.
    pub fn new(value: impl Into<String>) -> Result<Self, TopologyError> {
        let value = value.into();
        valid_identifier(&value, 128).then_some(Self(value)).ok_or_else(|| {
            TopologyError::new(TopologyErrorCode::InvalidIdentifier)
        })
    }

    /// Return the stable station identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Globally idempotent identity of one station write intent.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct OperationId(String);

impl OperationId {
    /// Create a validated operation identity.
    pub fn new(value: impl Into<String>) -> Result<Self, TopologyError> {
        let value = value.into();
        valid_identifier(&value, 192).then_some(Self(value)).ok_or_else(|| {
            TopologyError::new(TopologyErrorCode::InvalidIdentifier)
        })
    }

    /// Return the operation identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable identity of an explicit unresolved conflict.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ConflictId(String);

impl ConflictId {
    fn generated(revision: u64, operation: &OperationId) -> Self {
        Self(format!("conflict-{revision}-{}", operation.as_str()))
    }

    /// Return the conflict identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Supported center deployment shapes. Both use the same typed center protocol.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CenterTopology {
    /// A host-owned center reachable on the local/on-premises network.
    SelfHosted { endpoint: String },
    /// A Studio Cloud namespace selected by the authenticated host.
    StudioCloud { namespace: String },
}

impl CenterTopology {
    /// Validate a deployment shape without attempting network access.
    pub fn validate(&self) -> Result<(), TopologyError> {
        let value = match self {
            Self::SelfHosted { endpoint } => endpoint,
            Self::StudioCloud { namespace } => namespace,
        };
        if valid_identifier(value, 512) {
            Ok(())
        } else {
            Err(TopologyError::new(TopologyErrorCode::InvalidTopology))
        }
    }
}

/// Opaque one-time enrollment token. The center stores only a digest.
#[derive(Clone, Eq, PartialEq)]
pub struct PairingToken(String);

impl PairingToken {
    fn generated(center: &CenterId, sequence: u64) -> Self {
        Self(hex_digest(b"studio.center.pairing.v1", center.as_str(), sequence))
    }

    /// Parse a token received from a host-owned pairing flow.
    pub fn from_str(value: impl Into<String>) -> Result<Self, TopologyError> {
        let value = value.into();
        if value.len() == TOKEN_BYTES * 2
            && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            Ok(Self(value))
        } else {
            Err(TopologyError::new(TopologyErrorCode::PairingTokenInvalid))
        }
    }

    /// Return the token for the enrollment exchange.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PairingToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PairingToken(REDACTED)")
    }
}

/// Host-owned station settings. No operational record is stored here.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StationSettings {
    display_name: String,
    topology: CenterTopology,
}

impl StationSettings {
    /// Create settings for one station presentation and center endpoint.
    pub fn new(
        display_name: impl Into<String>,
        topology: CenterTopology,
    ) -> Result<Self, TopologyError> {
        let display_name = display_name.into();
        if !valid_identifier(&display_name, 128) {
            return Err(TopologyError::new(TopologyErrorCode::InvalidIdentifier));
        }
        topology.validate()?;
        Ok(Self {
            display_name,
            topology,
        })
    }

    /// Human-facing station name.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Configured center topology.
    #[must_use]
    pub const fn topology(&self) -> &CenterTopology {
        &self.topology
    }
}

/// An immutable operation applied to one center record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum WriteIntent {
    /// Replace or create the record value.
    Set(Value),
    /// Delete the record while retaining a tombstone for conflict detection.
    Delete,
}

impl WriteIntent {
    fn validate(&self) -> Result<(), TopologyError> {
        if let Self::Set(value) = self {
            let bytes = serde_json::to_vec(value)
                .map_err(|_| TopologyError::new(TopologyErrorCode::OperationInvalid))?;
            if bytes.len() > MAX_VALUE_BYTES {
                return Err(TopologyError::new(TopologyErrorCode::OperationInvalid));
            }
        }
        Ok(())
    }

    fn value(&self) -> Option<Value> {
        match self {
            Self::Set(value) => Some(value.clone()),
            Self::Delete => None,
        }
    }
}

/// A station-authenticated write request.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WriteOperation {
    operation_id: OperationId,
    table: String,
    key: String,
    base_revision: u64,
    intent: WriteIntent,
}

impl WriteOperation {
    /// Construct a write against a center table/key and observed revision.
    pub fn new(
        operation_id: OperationId,
        table: impl Into<String>,
        key: impl Into<String>,
        base_revision: u64,
        intent: WriteIntent,
    ) -> Result<Self, TopologyError> {
        let table = table.into();
        let key = key.into();
        if !valid_identifier(&table, 128) || !valid_identifier(&key, 256) {
            return Err(TopologyError::new(TopologyErrorCode::OperationInvalid));
        }
        intent.validate()?;
        Ok(Self {
            operation_id,
            table,
            key,
            base_revision,
            intent,
        })
    }

    /// Convenience constructor for a set operation.
    pub fn set(
        operation_id: OperationId,
        table: impl Into<String>,
        key: impl Into<String>,
        base_revision: u64,
        value: Value,
    ) -> Result<Self, TopologyError> {
        Self::new(operation_id, table, key, base_revision, WriteIntent::Set(value))
    }

    /// Convenience constructor for a delete operation.
    pub fn delete(
        operation_id: OperationId,
        table: impl Into<String>,
        key: impl Into<String>,
        base_revision: u64,
    ) -> Result<Self, TopologyError> {
        Self::new(operation_id, table, key, base_revision, WriteIntent::Delete)
    }

    /// Operation identity used for exactly-once logical replay.
    #[must_use]
    pub const fn operation_id(&self) -> &OperationId {
        &self.operation_id
    }

    /// Center table targeted by this operation.
    #[must_use]
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Center record key targeted by this operation.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Center revision observed by the station when it authored this operation.
    #[must_use]
    pub const fn base_revision(&self) -> u64 {
        self.base_revision
    }

    /// Requested intent.
    #[must_use]
    pub const fn intent(&self) -> &WriteIntent {
        &self.intent
    }
}

/// One authoritative record (including deleted tombstones).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SharedRecord {
    table: String,
    key: String,
    value: Option<Value>,
    revision: u64,
    last_operation: OperationId,
    last_station: StationId,
}

impl SharedRecord {
    /// Center table.
    #[must_use]
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Record key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Current value, or `None` for a tombstone.
    #[must_use]
    pub const fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }

    /// Revision of the operation that last changed this record.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Operation that last changed this record.
    #[must_use]
    pub const fn last_operation(&self) -> &OperationId {
        &self.last_operation
    }

    /// Station whose accepted operation last changed this record.
    #[must_use]
    pub const fn last_station(&self) -> &StationId {
        &self.last_station
    }
}

/// Open or resolved state of a conflict.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ConflictState {
    /// Both intents remain available for an explicit resolution.
    Open,
    /// A host-authorized resolution was applied.
    Resolved {
        resolution: ConflictResolution,
        revision: u64,
    },
}

/// Explicit choice for resolving a conflict; no implicit last-writer-wins path exists.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ConflictResolution {
    /// Keep the current authoritative value.
    KeepAuthoritative,
    /// Apply the incoming intent preserved by the conflict.
    ApplyIncoming,
    /// Apply a new explicitly selected intent.
    Set(WriteIntent),
}

/// Both sides of one stale-write conflict.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CenterConflict {
    id: ConflictId,
    table: String,
    key: String,
    base_revision: u64,
    revision: u64,
    authoritative: Option<SharedRecord>,
    incoming_station: StationId,
    incoming_operation: OperationId,
    incoming_intent: WriteIntent,
    state: ConflictState,
}

impl CenterConflict {
    /// Conflict identity.
    #[must_use]
    pub const fn id(&self) -> &ConflictId {
        &self.id
    }

    /// Record table and key.
    #[must_use]
    pub fn table(&self) -> &str {
        &self.table
    }

    /// Record key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Revision observed by the incoming station.
    #[must_use]
    pub const fn base_revision(&self) -> u64 {
        self.base_revision
    }

    /// Center event revision at which the conflict was recorded.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Authoritative side preserved at conflict time.
    #[must_use]
    pub const fn authoritative(&self) -> Option<&SharedRecord> {
        self.authoritative.as_ref()
    }

    /// Station carrying the incoming intent.
    #[must_use]
    pub const fn incoming_station(&self) -> &StationId {
        &self.incoming_station
    }

    /// Incoming operation identity.
    #[must_use]
    pub const fn incoming_operation(&self) -> &OperationId {
        &self.incoming_operation
    }

    /// Incoming intent preserved for a resolver.
    #[must_use]
    pub const fn incoming_intent(&self) -> &WriteIntent {
        &self.incoming_intent
    }

    /// Current conflict state.
    #[must_use]
    pub const fn state(&self) -> &ConflictState {
        &self.state
    }
}

/// Receipt returned after a center accepts, replays, or records an operation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OperationReceipt {
    operation_id: OperationId,
    revision: u64,
    outcome: OperationOutcome,
}

impl OperationReceipt {
    /// Operation identity.
    #[must_use]
    pub const fn operation_id(&self) -> &OperationId {
        &self.operation_id
    }

    /// Center event revision associated with this receipt.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Applied or conflict outcome.
    #[must_use]
    pub const fn outcome(&self) -> &OperationOutcome {
        &self.outcome
    }
}

/// Logical outcome recorded in the center's idempotency log.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OperationOutcome {
    /// The operation changed authoritative state.
    Applied,
    /// The operation was preserved as an explicit conflict.
    Conflict { conflict_id: ConflictId },
}

/// Result of submitting one operation to the center.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplyResult {
    /// First application of an operation that changed state.
    Applied(OperationReceipt),
    /// A duplicate submission of an already acknowledged operation.
    Replayed(OperationReceipt),
    /// First application of an operation preserved as a conflict.
    Conflict {
        receipt: OperationReceipt,
        conflict: CenterConflict,
    },
}

/// Complete materialized center view delivered to stations as a cache snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CenterSnapshot {
    center_id: CenterId,
    topology: CenterTopology,
    revision: u64,
    records: Vec<SharedRecord>,
    conflicts: Vec<CenterConflict>,
}

impl CenterSnapshot {
    /// Center identity.
    #[must_use]
    pub const fn center_id(&self) -> &CenterId {
        &self.center_id
    }

    /// Deployment topology.
    #[must_use]
    pub const fn topology(&self) -> &CenterTopology {
        &self.topology
    }

    /// Latest center event revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Deterministically ordered authoritative records.
    #[must_use]
    pub fn records(&self) -> &[SharedRecord] {
        &self.records
    }

    /// Explicit conflict records, including resolved history.
    #[must_use]
    pub fn conflicts(&self) -> &[CenterConflict] {
        &self.conflicts
    }
}

/// Station-local materialized data. It contains settings and a cache only.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StationLocalState {
    settings: StationSettings,
    cache: StationCache,
}

impl StationLocalState {
    /// Station settings.
    #[must_use]
    pub const fn settings(&self) -> &StationSettings {
        &self.settings
    }

    /// Cached center state; this is never accepted as an authority.
    #[must_use]
    pub const fn cache(&self) -> &StationCache {
        &self.cache
    }
}

/// Station cache metadata and last center snapshot.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StationCache {
    snapshot: Option<CenterSnapshot>,
}

impl StationCache {
    /// Last center revision observed by this station.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.snapshot.as_ref().map_or(0, CenterSnapshot::revision)
    }

    /// Last materialized center snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> Option<&CenterSnapshot> {
        self.snapshot.as_ref()
    }

    /// Look up a cached record without exposing a station-side authority API.
    #[must_use]
    pub fn record(&self, table: &str, key: &str) -> Option<&SharedRecord> {
        self.snapshot
            .as_ref()?
            .records
            .iter()
            .find(|record| record.table == table && record.key == key)
    }
}

/// Enrollment proof retained by a station after one pairing exchange.
#[derive(Clone, Eq, PartialEq)]
pub struct Enrollment {
    center_id: CenterId,
    station_id: StationId,
    credential: String,
}

impl Enrollment {
    /// Center to which this proof is scoped.
    #[must_use]
    pub const fn center_id(&self) -> &CenterId {
        &self.center_id
    }

    /// Enrolled station identity.
    #[must_use]
    pub const fn station_id(&self) -> &StationId {
        &self.station_id
    }
}

impl fmt::Debug for Enrollment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Enrollment")
            .field("center_id", &self.center_id)
            .field("station_id", &self.station_id)
            .field("credential", &"REDACTED")
            .finish()
    }
}

/// A center authority with deterministic in-process state and idempotency.
#[derive(Clone)]
pub struct CenterServer {
    state: Arc<Mutex<CenterState>>,
}

impl fmt::Debug for CenterServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CenterServer(host-owned state)")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CenterState {
    state_version: u32,
    center_id: CenterId,
    topology: CenterTopology,
    revision: u64,
    logical_clock: u64,
    next_station: u64,
    next_token: u64,
    records: BTreeMap<RecordKey, SharedRecord>,
    operations: BTreeMap<OperationId, StoredOperation>,
    conflicts: BTreeMap<ConflictId, CenterConflict>,
    pairings: BTreeMap<String, PendingPairing>,
    stations: BTreeMap<StationId, StoredStation>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
struct RecordKey {
    table: String,
    key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredOperation {
    fingerprint: String,
    receipt: OperationReceipt,
    conflict: Option<ConflictId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PendingPairing {
    expires_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoredStation {
    credential_hash: String,
}

impl CenterServer {
    /// Create an empty center. No network or cloud service is contacted.
    pub fn new(center_id: CenterId, topology: CenterTopology) -> Result<Self, TopologyError> {
        topology.validate()?;
        Ok(Self {
            state: Arc::new(Mutex::new(CenterState {
                state_version: STATE_VERSION,
                center_id,
                topology,
                revision: 0,
                logical_clock: 0,
                next_station: 0,
                next_token: 0,
                records: BTreeMap::new(),
                operations: BTreeMap::new(),
                conflicts: BTreeMap::new(),
                pairings: BTreeMap::new(),
                stations: BTreeMap::new(),
            })),
        })
    }

    /// Issue a deterministic, single-use pairing token with logical expiry.
    pub fn issue_pairing_token(&self) -> Result<PairingToken, TopologyError> {
        let mut state = lock(&self.state)?;
        state.logical_clock = state.logical_clock.saturating_add(1);
        state.next_token = state.next_token.saturating_add(1);
        let token = PairingToken::generated(&state.center_id, state.next_token);
        state.pairings.insert(
            digest_token(token.as_str()),
            PendingPairing {
                expires_at: state.logical_clock.saturating_add(TOKEN_TTL),
            },
        );
        Ok(token)
    }

    /// Advance the deterministic pairing clock and expire old enrollment tokens.
    pub fn advance_pairing_clock(&self, ticks: u64) -> Result<(), TopologyError> {
        let mut state = lock(&self.state)?;
        state.logical_clock = state.logical_clock.saturating_add(ticks);
        let now = state.logical_clock;
        state.pairings.retain(|_, pairing| pairing.expires_at >= now);
        Ok(())
    }

    /// Consume a pairing token and return a scoped station enrollment proof.
    pub fn pair(
        &self,
        token: &PairingToken,
        display_name: impl Into<String>,
    ) -> Result<Enrollment, TopologyError> {
        let mut state = lock(&self.state)?;
        let display_name = display_name.into();
        if !valid_identifier(&display_name, 128) {
            return Err(TopologyError::new(TopologyErrorCode::InvalidIdentifier));
        }
        let token_hash = digest_token(token.as_str());
        let pending = state
            .pairings
            .get(&token_hash)
            .cloned()
            .ok_or_else(|| TopologyError::new(TopologyErrorCode::PairingTokenUnknown))?;
        if pending.expires_at < state.logical_clock {
            state.pairings.remove(&token_hash);
            return Err(TopologyError::new(TopologyErrorCode::PairingTokenExpired));
        }
        state
            .pairings
            .remove(&token_hash)
            .ok_or_else(|| TopologyError::new(TopologyErrorCode::PairingTokenUnknown))?;
        state.next_station = state.next_station.saturating_add(1);
        let station_id = StationId(format!("station-{}", state.next_station));
        let credential = hex_digest(
            b"studio.center.station.v1",
            &format!("{}:{}:{display_name}", state.center_id.as_str(), station_id.as_str()),
            state.next_station,
        );
        state.stations.insert(
            station_id.clone(),
            StoredStation {
                credential_hash: digest_token(&credential),
            },
        );
        Ok(Enrollment {
            center_id: state.center_id.clone(),
            station_id,
            credential,
        })
    }

    /// Center identity.
    pub fn center_id(&self) -> Result<CenterId, TopologyError> {
        Ok(lock(&self.state)?.center_id.clone())
    }

    /// Configured center deployment shape.
    pub fn topology(&self) -> Result<CenterTopology, TopologyError> {
        Ok(lock(&self.state)?.topology.clone())
    }

    /// Latest authoritative snapshot. This is the only source stations may sync.
    pub fn snapshot(&self) -> Result<CenterSnapshot, TopologyError> {
        let state = lock(&self.state)?;
        Ok(snapshot_of(&state))
    }

    /// Apply or logically replay one enrolled station operation.
    pub fn apply(
        &self,
        enrollment: &Enrollment,
        operation: &WriteOperation,
    ) -> Result<ApplyResult, TopologyError> {
        let mut state = lock(&self.state)?;
        apply_locked(&mut state, enrollment, operation)
    }

    /// Resolve one conflict through an explicit host-authorized choice.
    pub fn resolve_conflict(
        &self,
        enrollment: &Enrollment,
        conflict_id: &ConflictId,
        operation_id: OperationId,
        resolution: ConflictResolution,
    ) -> Result<ApplyResult, TopologyError> {
        let mut state = lock(&self.state)?;
        authorize(&state, enrollment)?;
        let conflict = state
            .conflicts
            .get(conflict_id)
            .cloned()
            .ok_or_else(|| TopologyError::new(TopologyErrorCode::ConflictUnknown))?;
        if !matches!(conflict.state, ConflictState::Open) {
            return Err(TopologyError::new(TopologyErrorCode::ConflictAlreadyResolved));
        }
        let intent = match &resolution {
            ConflictResolution::KeepAuthoritative => conflict
                .authoritative
                .as_ref()
                .map_or(WriteIntent::Delete, |record| {
                    record.value.clone().map_or(WriteIntent::Delete, WriteIntent::Set)
                }),
            ConflictResolution::ApplyIncoming => conflict.incoming_intent.clone(),
            ConflictResolution::Set(intent) => intent.clone(),
        };
        intent.validate()?;
        let operation = WriteOperation::new(
            operation_id,
            conflict.table.clone(),
            conflict.key.clone(),
            state.revision,
            intent,
        )?;
        let fingerprint = fingerprint(enrollment.station_id(), &operation)?;
        if let Some(existing) = state.operations.get(operation.operation_id()) {
            if existing.fingerprint != fingerprint {
                return Err(TopologyError::new(TopologyErrorCode::OperationIdConflict));
            }
            return Ok(ApplyResult::Replayed(existing.receipt.clone()));
        }
        state.revision = state.revision.saturating_add(1);
        let revision = state.revision;
        let key = RecordKey {
            table: operation.table.clone(),
            key: operation.key.clone(),
        };
        let record = SharedRecord {
            table: operation.table.clone(),
            key: operation.key.clone(),
            value: operation.intent.value(),
            revision,
            last_operation: operation.operation_id.clone(),
            last_station: enrollment.station_id.clone(),
        };
        state.records.insert(key, record);
        if let Some(stored) = state.conflicts.get_mut(conflict_id) {
            stored.state = ConflictState::Resolved { resolution, revision };
        }
        let receipt = OperationReceipt {
            operation_id: operation.operation_id.clone(),
            revision,
            outcome: OperationOutcome::Applied,
        };
        state.operations.insert(
            operation.operation_id.clone(),
            StoredOperation {
                fingerprint,
                receipt: receipt.clone(),
                conflict: None,
            },
        );
        Ok(ApplyResult::Applied(receipt))
    }

    fn from_state(state: CenterState) -> Result<Self, TopologyError> {
        if state.state_version != STATE_VERSION {
            return Err(TopologyError::new(TopologyErrorCode::PersistedStateInvalid));
        }
        state.topology.validate()?;
        Ok(Self {
            state: Arc::new(Mutex::new(state)),
        })
    }

    fn state_copy(&self) -> Result<CenterState, TopologyError> {
        Ok(lock(&self.state)?.clone())
    }
}

/// Station connection with settings/cache and a transient offline outbox.
pub struct Station {
    center: CenterServer,
    enrollment: Enrollment,
    local: StationLocalState,
    pending: VecDeque<WriteOperation>,
    next_operation: u64,
    connected: bool,
}

impl fmt::Debug for Station {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Station")
            .field("station_id", &self.enrollment.station_id)
            .field("connected", &self.connected)
            .field("pending", &self.pending.len())
            .finish()
    }
}

impl Station {
    /// Enroll a station and seed its cache from authoritative center state.
    pub fn enroll(
        center: &CenterServer,
        token: PairingToken,
        settings: StationSettings,
    ) -> Result<Self, TopologyError> {
        if settings.topology != center.topology()? {
            return Err(TopologyError::new(TopologyErrorCode::InvalidTopology));
        }
        let enrollment = center.pair(&token, settings.display_name.clone())?;
        if enrollment.center_id != center.center_id()? {
            return Err(TopologyError::new(TopologyErrorCode::Unauthorized));
        }
        let cache = StationCache {
            snapshot: Some(center.snapshot()?),
        };
        Ok(Self {
            center: center.clone(),
            enrollment,
            local: StationLocalState { settings, cache },
            pending: VecDeque::new(),
            next_operation: 0,
            connected: true,
        })
    }

    /// Station identity assigned by the center.
    #[must_use]
    pub const fn station_id(&self) -> &StationId {
        &self.enrollment.station_id
    }

    /// Enrollment proof for host-mediated calls; the credential itself has no accessor.
    #[must_use]
    pub const fn enrollment(&self) -> &Enrollment {
        &self.enrollment
    }

    /// Read-only station settings/cache; no operational source of truth is exposed.
    #[must_use]
    pub const fn local_state(&self) -> &StationLocalState {
        &self.local
    }

    /// Mark the connection unavailable; subsequent writes enter the transient outbox.
    pub const fn disconnect(&mut self) {
        self.connected = false;
    }

    /// Mark the connection available and replay queued operations exactly once logically.
    pub fn reconnect(&mut self) -> Result<Vec<StationWriteResult>, TopologyError> {
        self.connected = true;
        self.flush()
    }

    /// Whether the station currently has a center connection.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.connected
    }

    /// Number of transient outbound operations awaiting replay.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Pull the authoritative center snapshot into the station cache.
    pub fn sync(&mut self) -> Result<(), TopologyError> {
        if !self.connected {
            return Err(TopologyError::new(TopologyErrorCode::Disconnected));
        }
        self.local.cache.snapshot = Some(self.center.snapshot()?);
        Ok(())
    }

    /// Set one shared record, applying immediately or queueing while offline.
    pub fn set(
        &mut self,
        table: impl Into<String>,
        key: impl Into<String>,
        value: Value,
    ) -> Result<StationWriteResult, TopologyError> {
        self.submit(table, key, WriteIntent::Set(value))
    }

    /// Delete one shared record, applying immediately or queueing while offline.
    pub fn delete(
        &mut self,
        table: impl Into<String>,
        key: impl Into<String>,
    ) -> Result<StationWriteResult, TopologyError> {
        self.submit(table, key, WriteIntent::Delete)
    }

    /// Resolve a cached conflict through the authoritative center.
    pub fn resolve_conflict(
        &mut self,
        conflict_id: &ConflictId,
        operation_id: OperationId,
        resolution: ConflictResolution,
    ) -> Result<StationWriteResult, TopologyError> {
        if !self.connected {
            return Err(TopologyError::new(TopologyErrorCode::Disconnected));
        }
        let result = station_result(self.center.resolve_conflict(
            &self.enrollment,
            conflict_id,
            operation_id,
            resolution,
        )?);
        self.sync()?;
        Ok(result)
    }

    /// Replay all queued operations and refresh the cache.
    pub fn flush(&mut self) -> Result<Vec<StationWriteResult>, TopologyError> {
        if !self.connected {
            return Err(TopologyError::new(TopologyErrorCode::Disconnected));
        }
        let mut results = Vec::new();
        while let Some(operation) = self.pending.front().cloned() {
            let result = station_result(self.center.apply(&self.enrollment, &operation)?);
            self.pending.pop_front();
            results.push(result);
        }
        self.sync()?;
        Ok(results)
    }

    fn submit(
        &mut self,
        table: impl Into<String>,
        key: impl Into<String>,
        intent: WriteIntent,
    ) -> Result<StationWriteResult, TopologyError> {
        self.next_operation = self.next_operation.saturating_add(1);
        let operation_id = OperationId::new(format!(
            "{}:{}",
            self.enrollment.station_id.as_str(),
            self.next_operation
        ))?;
        let operation = WriteOperation::new(
            operation_id,
            table,
            key,
            self.local.cache.revision(),
            intent,
        )?;
        if !self.connected {
            self.pending.push_back(operation.clone());
            return Ok(StationWriteResult::Queued(operation));
        }
        let result = station_result(self.center.apply(&self.enrollment, &operation)?);
        self.sync()?;
        Ok(result)
    }
}

/// Station-facing result, distinguishing queued work, acknowledgements, replays, and conflicts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StationWriteResult {
    /// Operation retained in the transient offline outbox.
    Queued(WriteOperation),
    /// Operation was accepted and changed center state.
    Applied(OperationReceipt),
    /// Center recognized a duplicate logical operation.
    Replayed(OperationReceipt),
    /// Operation was accepted into explicit conflict preservation.
    Conflict { receipt: OperationReceipt, conflict: CenterConflict },
}

/// Durable center wrapper backed by the host-owned LocalStore interface.
pub struct PersistentCenter<S: LocalStore> {
    center: CenterServer,
    store: Arc<S>,
    batch_id: String,
    mutation_lock: Arc<tokio::sync::Mutex<()>>,
}

impl<S: LocalStore> fmt::Debug for PersistentCenter<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PersistentCenter(host LocalStore)")
    }
}

impl<S: LocalStore> PersistentCenter<S> {
    /// Open a center from its durable host-owned state, or initialize it fresh.
    pub async fn open(
        center_id: CenterId,
        topology: CenterTopology,
        store: Arc<S>,
    ) -> Result<Self, TopologyError> {
        let batch_id = state_batch_id(&center_id);
        let entries = store
            .batch_entries(&batch_id)
            .await
            .map_err(|_| TopologyError::new(TopologyErrorCode::PersistenceUnavailable))?;
        let center = if entries.is_empty() {
            CenterServer::new(center_id, topology)?
        } else if entries.len() == 1 {
            let state: CenterState = serde_json::from_value(entries[0].payload.clone())
                .map_err(|_| TopologyError::new(TopologyErrorCode::PersistedStateInvalid))?;
            if state.center_id != center_id || state.topology != topology {
                return Err(TopologyError::new(TopologyErrorCode::PersistedStateInvalid));
            }
            CenterServer::from_state(state)?
        } else {
            return Err(TopologyError::new(TopologyErrorCode::PersistedStateInvalid));
        };
        Ok(Self {
            center,
            store,
            batch_id,
            mutation_lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    /// Authoritative center snapshot.
    pub fn snapshot(&self) -> Result<CenterSnapshot, TopologyError> {
        self.center.snapshot()
    }

    /// Issue and durably retain a pairing token.
    pub async fn issue_pairing_token(&self) -> Result<PairingToken, TopologyError> {
        self.mutate(|center| center.issue_pairing_token()).await
    }

    /// Pair and durably retain a station enrollment.
    pub async fn pair(
        &self,
        token: &PairingToken,
        display_name: impl Into<String>,
    ) -> Result<Enrollment, TopologyError> {
        let display_name = display_name.into();
        self.mutate(|center| center.pair(token, display_name)).await
    }

    /// Apply and durably acknowledge one operation.
    pub async fn apply(
        &self,
        enrollment: &Enrollment,
        operation: &WriteOperation,
    ) -> Result<ApplyResult, TopologyError> {
        self.mutate(|center| center.apply(enrollment, operation)).await
    }

    /// Resolve and durably acknowledge one conflict.
    pub async fn resolve_conflict(
        &self,
        enrollment: &Enrollment,
        conflict_id: &ConflictId,
        operation_id: OperationId,
        resolution: ConflictResolution,
    ) -> Result<ApplyResult, TopologyError> {
        self.mutate(|center| {
            center.resolve_conflict(enrollment, conflict_id, operation_id, resolution)
        })
        .await
    }

    async fn mutate<T: 'static, F>(&self, operation: F) -> Result<T, TopologyError>
    where
        F: FnOnce(&CenterServer) -> Result<T, TopologyError>,
    {
        let _guard = self.mutation_lock.lock().await;
        let before = self.center.state_copy()?;
        let result = operation(&self.center)?;
        let state = self.center.state_copy()?;
        if let Err(error) = self.persist(&state).await {
            self.center.restore_state(before)?;
            return Err(error);
        }
        Ok(result)
    }

    async fn persist(&self, state: &CenterState) -> Result<(), TopologyError> {
        let payload = serde_json::to_value(state)
            .map_err(|_| TopologyError::new(TopologyErrorCode::PersistenceUnavailable))?;
        let batch = StoreBatch::new(
            self.batch_id.clone(),
            [StoreBatchEntry { ordinal: 0, payload }],
        )
        .map_err(|_| TopologyError::new(TopologyErrorCode::PersistenceUnavailable))?;
        self.store
            .write_batch(&batch)
            .await
            .map_err(|_| TopologyError::new(TopologyErrorCode::PersistenceUnavailable))
    }
}

impl CenterServer {
    fn restore_state(&self, state: CenterState) -> Result<(), TopologyError> {
        *lock(&self.state)? = state;
        Ok(())
    }
}

/// Stable topology failure family with no storage, credential, or record leakage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyErrorCode {
    /// Center/station/record identity was empty or unsafe.
    InvalidIdentifier,
    /// Deployment topology was malformed.
    InvalidTopology,
    /// Pairing token syntax was malformed.
    PairingTokenInvalid,
    /// Pairing token no longer exists.
    PairingTokenUnknown,
    /// Pairing token passed its logical expiry.
    PairingTokenExpired,
    /// Enrollment proof does not belong to this center/station.
    Unauthorized,
    /// Write shape or value bound was invalid.
    OperationInvalid,
    /// Same operation identity was reused for a different intent.
    OperationIdConflict,
    /// Station attempted an operation while disconnected.
    Disconnected,
    /// Requested conflict was absent.
    ConflictUnknown,
    /// Requested conflict already has a resolution.
    ConflictAlreadyResolved,
    /// Durable state could not be read or written.
    PersistenceUnavailable,
    /// Durable state was not a supported topology snapshot.
    PersistedStateInvalid,
}

/// Topology failure with a stable safe code.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[error("center topology operation failed: {code:?}")]
pub struct TopologyError {
    code: TopologyErrorCode,
}

impl TopologyError {
    const fn new(code: TopologyErrorCode) -> Self {
        Self { code }
    }

    /// Stable machine-readable failure code.
    #[must_use]
    pub const fn code(self) -> TopologyErrorCode {
        self.code
    }
}

fn apply_locked(
    state: &mut CenterState,
    enrollment: &Enrollment,
    operation: &WriteOperation,
) -> Result<ApplyResult, TopologyError> {
    authorize(state, enrollment)?;
    if operation.base_revision > state.revision {
        return Err(TopologyError::new(TopologyErrorCode::OperationInvalid));
    }
    let operation_fingerprint = fingerprint(enrollment.station_id(), operation)?;
    if let Some(existing) = state.operations.get(operation.operation_id()) {
        if existing.fingerprint != operation_fingerprint {
            return Err(TopologyError::new(TopologyErrorCode::OperationIdConflict));
        }
        if let Some(conflict_id) = &existing.conflict {
            let conflict = state
                .conflicts
                .get(conflict_id)
                .cloned()
                .ok_or_else(|| TopologyError::new(TopologyErrorCode::PersistedStateInvalid))?;
            return Ok(ApplyResult::Conflict {
                receipt: existing.receipt.clone(),
                conflict,
            });
        }
        return Ok(ApplyResult::Replayed(existing.receipt.clone()));
    }
    let key = RecordKey {
        table: operation.table.clone(),
        key: operation.key.clone(),
    };
    let current = state.records.get(&key).cloned();
    let has_newer_open_conflict = state.conflicts.values().any(|conflict| {
        conflict.table == operation.table
            && conflict.key == operation.key
            && matches!(conflict.state, ConflictState::Open)
            && conflict.revision > operation.base_revision
    });
    if current.as_ref().is_some_and(|record| record.revision > operation.base_revision)
        || has_newer_open_conflict
    {
        state.revision = state.revision.saturating_add(1);
        let revision = state.revision;
        let conflict_id = ConflictId::generated(revision, operation.operation_id());
        let conflict = CenterConflict {
            id: conflict_id.clone(),
            table: operation.table.clone(),
            key: operation.key.clone(),
            base_revision: operation.base_revision,
            revision,
            authoritative: current,
            incoming_station: enrollment.station_id.clone(),
            incoming_operation: operation.operation_id.clone(),
            incoming_intent: operation.intent.clone(),
            state: ConflictState::Open,
        };
        let receipt = OperationReceipt {
            operation_id: operation.operation_id.clone(),
            revision,
            outcome: OperationOutcome::Conflict {
                conflict_id: conflict_id.clone(),
            },
        };
        state.conflicts.insert(conflict_id.clone(), conflict.clone());
        state.operations.insert(
            operation.operation_id.clone(),
            StoredOperation {
                fingerprint: operation_fingerprint,
                receipt: receipt.clone(),
                conflict: Some(conflict_id),
            },
        );
        return Ok(ApplyResult::Conflict { receipt, conflict });
    }
    state.revision = state.revision.saturating_add(1);
    let revision = state.revision;
    state.records.insert(
        key,
        SharedRecord {
            table: operation.table.clone(),
            key: operation.key.clone(),
            value: operation.intent.value(),
            revision,
            last_operation: operation.operation_id.clone(),
            last_station: enrollment.station_id.clone(),
        },
    );
    let receipt = OperationReceipt {
        operation_id: operation.operation_id.clone(),
        revision,
        outcome: OperationOutcome::Applied,
    };
    state.operations.insert(
        operation.operation_id.clone(),
        StoredOperation {
            fingerprint: operation_fingerprint,
            receipt: receipt.clone(),
            conflict: None,
        },
    );
    Ok(ApplyResult::Applied(receipt))
}

fn authorize(state: &CenterState, enrollment: &Enrollment) -> Result<(), TopologyError> {
    if enrollment.center_id != state.center_id {
        return Err(TopologyError::new(TopologyErrorCode::Unauthorized));
    }
    let station = state
        .stations
        .get(&enrollment.station_id)
        .ok_or_else(|| TopologyError::new(TopologyErrorCode::Unauthorized))?;
    if station.credential_hash != digest_token(&enrollment.credential) {
        return Err(TopologyError::new(TopologyErrorCode::Unauthorized));
    }
    Ok(())
}

fn snapshot_of(state: &CenterState) -> CenterSnapshot {
    CenterSnapshot {
        center_id: state.center_id.clone(),
        topology: state.topology.clone(),
        revision: state.revision,
        records: state.records.values().cloned().collect(),
        conflicts: state.conflicts.values().cloned().collect(),
    }
}

fn station_result(result: ApplyResult) -> StationWriteResult {
    match result {
        ApplyResult::Applied(receipt) => StationWriteResult::Applied(receipt),
        ApplyResult::Replayed(receipt) => StationWriteResult::Replayed(receipt),
        ApplyResult::Conflict { receipt, conflict } => {
            StationWriteResult::Conflict { receipt, conflict }
        }
    }
}

fn fingerprint(station: &StationId, operation: &WriteOperation) -> Result<String, TopologyError> {
    let encoded = serde_json::to_vec(&(station, operation))
        .map_err(|_| TopologyError::new(TopologyErrorCode::OperationInvalid))?;
    Ok(hex_bytes(&Sha256::digest(encoded)))
}

fn digest_token(token: &str) -> String {
    hex_bytes(&Sha256::digest(token.as_bytes()))
}

fn hex_digest(domain: &[u8], identity: &str, sequence: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((identity.len() as u64).to_be_bytes());
    hasher.update(identity.as_bytes());
    hasher.update(sequence.to_be_bytes());
    hex_bytes(&hasher.finalize())
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push_str(&format!("{byte:02x}"));
    }
    result
}

fn state_batch_id(center_id: &CenterId) -> String {
    format!("{STATE_BATCH_PREFIX}{}", center_id.as_str())
}

fn valid_identifier(value: &str, max_len: usize) -> bool {
    !value.is_empty() && value.len() <= max_len && !value.chars().any(char::is_control)
}

fn lock<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, TopologyError> {
    mutex
        .lock()
        .map_err(|_| TopologyError::new(TopologyErrorCode::PersistenceUnavailable))
}
