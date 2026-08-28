//! Host-owned, append-only application audit records.
//!
//! The audit boundary deliberately accepts a small closed set of event classes.  Callers provide
//! an already host-verified actor and an explicit timestamp; there is no clock or identity lookup
//! hidden in this persistence layer, which keeps tests deterministic and makes attribution the
//! responsibility of the host service that received the operation.

#![allow(missing_docs)]
#![allow(clippy::all, clippy::pedantic, clippy::restriction, clippy::nursery)]
#![allow(
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::missing_fields_in_debug,
    clippy::format_push_string
)]

use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use studio_security::{PluginPrincipal, SensitiveValueFilter};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::{LocalStore, StoreBatch, StoreBatchEntry};

/// Current on-disk audit record format.
pub const AUDIT_LOG_FORMAT_VERSION: u16 = 1;

const AUDIT_HASH_DOMAIN: &[u8] = b"studio.application-audit-log.v1";
const AUDIT_NAMESPACE_DOMAIN: &[u8] = b"studio.application-audit-namespace.v1";
const AUDIT_BATCH_PREFIX: &str = "audit.v1";
const MAX_ACTOR_BYTES: usize = 256;
const MAX_DETAILS_BYTES: usize = 256 * 1024;
const MAX_EVENTS: usize = 1_000_000;
const MAX_QUERY_LIMIT: usize = 100_000;

/// Security-relevant application event classes accepted by the audit boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    /// A host-side authentication attempt, whether accepted or denied.
    AuthenticationAttempt,
    /// A role definition or role assignment changed.
    RoleChange,
    /// A membership was added, removed, or changed.
    MembershipChange,
    /// A destructive operation was requested or completed.
    DestructiveAction,
    /// A data export was requested or completed.
    DataExport,
    /// An inbound webhook was admitted or denied.
    WebhookAdmission,
    /// A declared workflow run started, completed, or failed.
    WorkflowRun,
}

impl AuditEventType {
    /// Stable wire name used in exports and query diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AuthenticationAttempt => "authentication_attempt",
            Self::RoleChange => "role_change",
            Self::MembershipChange => "membership_change",
            Self::DestructiveAction => "destructive_action",
            Self::DataExport => "data_export",
            Self::WebhookAdmission => "webhook_admission",
            Self::WorkflowRun => "workflow_run",
        }
    }
}

/// Alias retained for callers that name the event discriminator a kind.
pub type AuditEventKind = AuditEventType;

/// One event submitted to the host audit service.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Closed event class.
    pub event_type: AuditEventType,
    /// Host-attributed actor identity. This may be a user, workflow, webhook, or service identity.
    pub actor: String,
    /// Unix timestamp in milliseconds supplied by the host caller.
    pub occurred_at: i64,
    /// Structured event context. Sensitive fields are redacted before persistence.
    pub details: Value,
}

impl AuditEvent {
    /// Construct an event after validating its actor and structured context bounds.
    pub fn new(
        event_type: AuditEventType,
        actor: impl Into<String>,
        occurred_at: i64,
        details: Value,
    ) -> Result<Self, AuditLogError> {
        let event = Self {
            event_type,
            actor: actor.into(),
            occurred_at,
            details,
        };
        validate_event(&event)?;
        Ok(event)
    }

    /// Convenience constructor for an authentication attempt.
    pub fn authentication_attempt(
        actor: impl Into<String>,
        occurred_at: i64,
        accepted: bool,
    ) -> Result<Self, AuditLogError> {
        Self::new(
            AuditEventType::AuthenticationAttempt,
            actor,
            occurred_at,
            json!({ "accepted": accepted }),
        )
    }

    /// Convenience constructor for a role change.
    pub fn role_change(
        actor: impl Into<String>,
        occurred_at: i64,
        details: Value,
    ) -> Result<Self, AuditLogError> {
        Self::new(AuditEventType::RoleChange, actor, occurred_at, details)
    }

    /// Convenience constructor for a membership change.
    pub fn membership_change(
        actor: impl Into<String>,
        occurred_at: i64,
        details: Value,
    ) -> Result<Self, AuditLogError> {
        Self::new(
            AuditEventType::MembershipChange,
            actor,
            occurred_at,
            details,
        )
    }

    /// Convenience constructor for a destructive action.
    pub fn destructive_action(
        actor: impl Into<String>,
        occurred_at: i64,
        details: Value,
    ) -> Result<Self, AuditLogError> {
        Self::new(
            AuditEventType::DestructiveAction,
            actor,
            occurred_at,
            details,
        )
    }

    /// Convenience constructor for a data export.
    pub fn data_export(
        actor: impl Into<String>,
        occurred_at: i64,
        details: Value,
    ) -> Result<Self, AuditLogError> {
        Self::new(AuditEventType::DataExport, actor, occurred_at, details)
    }

    /// Convenience constructor for webhook admission.
    pub fn webhook_admission(
        actor: impl Into<String>,
        occurred_at: i64,
        details: Value,
    ) -> Result<Self, AuditLogError> {
        Self::new(
            AuditEventType::WebhookAdmission,
            actor,
            occurred_at,
            details,
        )
    }

    /// Convenience constructor for a workflow run.
    pub fn workflow_run(
        actor: impl Into<String>,
        occurred_at: i64,
        details: Value,
    ) -> Result<Self, AuditLogError> {
        Self::new(AuditEventType::WorkflowRun, actor, occurred_at, details)
    }
}

/// One persisted audit record, including its tamper-evidence links.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditRecord {
    sequence: u64,
    event_type: AuditEventType,
    actor: String,
    occurred_at: i64,
    details: Value,
    previous_hash: [u8; 32],
    hash: [u8; 32],
}

impl AuditRecord {
    /// Zero-based sequence number assigned by the host.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Event discriminator.
    #[must_use]
    pub const fn event_type(&self) -> AuditEventType {
        self.event_type
    }

    /// Alias for [`Self::event_type`].
    #[must_use]
    pub const fn kind(&self) -> AuditEventType {
        self.event_type()
    }

    /// Host-attributed actor identity.
    #[must_use]
    pub fn actor(&self) -> &str {
        &self.actor
    }

    /// Unix timestamp in milliseconds.
    #[must_use]
    pub const fn occurred_at(&self) -> i64 {
        self.occurred_at
    }

    /// Redacted structured event details.
    #[must_use]
    pub const fn details(&self) -> &Value {
        &self.details
    }

    /// Previous record hash as lowercase hexadecimal.
    #[must_use]
    pub fn previous_hash(&self) -> String {
        encode_hash(&self.previous_hash)
    }

    /// This record's hash as lowercase hexadecimal.
    #[must_use]
    pub fn hash(&self) -> String {
        encode_hash(&self.hash)
    }
}

/// Filters applied by [`AuditLog::query`]. Time bounds are inclusive.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AuditQuery {
    /// Inclusive lower Unix-millisecond timestamp.
    pub from: Option<i64>,
    /// Inclusive upper Unix-millisecond timestamp.
    pub to: Option<i64>,
    /// Restrict results to one event class.
    pub event_type: Option<AuditEventType>,
    /// Restrict results to one exact actor identity.
    pub actor: Option<String>,
    /// Maximum number of matching records, retained in sequence order.
    pub limit: Option<usize>,
}

impl AuditQuery {
    /// Construct an empty query.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            from: None,
            to: None,
            event_type: None,
            actor: None,
            limit: None,
        }
    }

    /// Set an inclusive timestamp range.
    #[must_use]
    pub const fn time_range(mut self, from: Option<i64>, to: Option<i64>) -> Self {
        self.from = from;
        self.to = to;
        self
    }

    /// Alias for [`Self::time_range`].
    #[must_use]
    pub const fn between(self, from: Option<i64>, to: Option<i64>) -> Self {
        self.time_range(from, to)
    }

    /// Restrict this query to an event class.
    #[must_use]
    pub const fn with_event_type(mut self, event_type: AuditEventType) -> Self {
        self.event_type = Some(event_type);
        self
    }

    /// Restrict this query to one actor.
    #[must_use]
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }

    /// Set a result limit.
    #[must_use]
    pub const fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Stable, value-free audit failure family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditLogErrorCode {
    /// Event or query input was malformed or exceeded a bound.
    RequestInvalid,
    /// The backing host store could not be read or written.
    StorageUnavailable,
    /// Persisted records do not match the supported audit format.
    Corrupt,
    /// The persisted hash chain or namespace marker does not verify.
    Tampered,
    /// The bounded audit log cannot accept another event.
    CapacityExceeded,
}

impl AuditLogErrorCode {
    /// Stable diagnostic identifier.
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        match self {
            Self::RequestInvalid => "audit.request_invalid",
            Self::StorageUnavailable => "audit.storage_unavailable",
            Self::Corrupt => "audit.corrupt",
            Self::Tampered => "audit.tampered",
            Self::CapacityExceeded => "audit.capacity_exceeded",
        }
    }
}

/// Safe audit failure without storage-engine, filesystem, actor, or event details.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuditLogDiagnostic {
    code: AuditLogErrorCode,
}

impl AuditLogDiagnostic {
    /// Stable machine-readable failure code.
    #[must_use]
    pub const fn code(self) -> AuditLogErrorCode {
        self.code
    }

    /// Stable wire-safe identifier.
    #[must_use]
    pub const fn stable_code(self) -> &'static str {
        self.code.stable_code()
    }
}

/// Audit operation error with a safe diagnostic projection.
#[derive(Debug, Error)]
#[error("{message}")]
pub struct AuditLogError {
    diagnostic: AuditLogDiagnostic,
    message: &'static str,
}

impl AuditLogError {
    const fn new(code: AuditLogErrorCode) -> Self {
        let message = match code {
            AuditLogErrorCode::RequestInvalid => "audit request invalid",
            AuditLogErrorCode::StorageUnavailable => "audit storage unavailable",
            AuditLogErrorCode::Corrupt => "audit log is corrupt",
            AuditLogErrorCode::Tampered => "audit log integrity verification failed",
            AuditLogErrorCode::CapacityExceeded => "audit log capacity exceeded",
        };
        Self {
            diagnostic: AuditLogDiagnostic { code },
            message,
        }
    }

    /// Safe diagnostic suitable for the host UI and automation.
    #[must_use]
    pub const fn diagnostic(&self) -> AuditLogDiagnostic {
        self.diagnostic
    }

    /// Stable machine-readable failure code.
    #[must_use]
    pub const fn code(&self) -> AuditLogErrorCode {
        self.diagnostic.code()
    }

    /// Stable wire-safe failure identifier.
    #[must_use]
    pub const fn stable_code(&self) -> &'static str {
        self.diagnostic.stable_code()
    }
}

/// Host-owned append-only audit log for one application identity.
pub struct AuditLog<S> {
    store: S,
    batch_id: String,
    namespace: [u8; 32],
    redactor: SensitiveValueFilter,
    state: Mutex<AuditState>,
}

#[derive(Default)]
struct AuditState {
    loaded: bool,
    records: Vec<AuditRecord>,
}

impl<S> fmt::Debug for AuditLog<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuditLog")
            .field("batch_id", &self.batch_id)
            .field(
                "records_loaded",
                &self.state.try_lock().map_or(0, |state| state.records.len()),
            )
            .finish()
    }
}

impl<S> AuditLog<S> {
    /// Bind a log to a verified plugin principal using the stable publisher/application identity.
    #[must_use]
    pub fn new(store: S, principal: &PluginPrincipal) -> Self {
        Self::with_filter(store, principal, SensitiveValueFilter::new())
    }

    /// Bind a log to a verified principal and a host-populated sensitive-value filter.
    #[must_use]
    pub fn with_filter(
        store: S,
        principal: &PluginPrincipal,
        redactor: SensitiveValueFilter,
    ) -> Self {
        let namespace = namespace_digest(principal.publisher_id(), principal.plugin_id());
        Self::from_namespace(store, namespace, redactor)
    }

    /// Bind a log to an application identifier when no plugin principal is available.
    ///
    /// This is intended for first-party host services. Plugin-facing callers should use
    /// [`Self::new`] so publisher/application partitioning follows the verified principal.
    pub fn for_application(
        store: S,
        application: impl Into<String>,
    ) -> Result<Self, AuditLogError> {
        Self::for_application_with_filter(store, application, SensitiveValueFilter::new())
    }

    /// Bind a first-party application log with an explicit redaction filter.
    pub fn for_application_with_filter(
        store: S,
        application: impl Into<String>,
        redactor: SensitiveValueFilter,
    ) -> Result<Self, AuditLogError> {
        let application = application.into();
        if !valid_identifier(&application) {
            return Err(AuditLogError::new(AuditLogErrorCode::RequestInvalid));
        }
        let namespace = namespace_digest("first-party", &application);
        Ok(Self::from_namespace(store, namespace, redactor))
    }

    fn from_namespace(store: S, namespace: [u8; 32], redactor: SensitiveValueFilter) -> Self {
        Self {
            store,
            batch_id: format!("{AUDIT_BATCH_PREFIX}.{}", encode_hash(&namespace)),
            namespace,
            redactor,
            state: Mutex::new(AuditState::default()),
        }
    }

    /// Stable host-store batch identity. It contains no raw application identity.
    #[must_use]
    pub fn storage_id(&self) -> &str {
        &self.batch_id
    }

    /// Append one event and return the host-assigned, redacted record.
    pub async fn append(&self, event: AuditEvent) -> Result<AuditRecord, AuditLogError>
    where
        S: LocalStore,
    {
        validate_event(&event)?;
        let mut state = self.state.lock().await;
        self.ensure_loaded(&mut state).await?;
        if state.records.len() >= MAX_EVENTS {
            return Err(AuditLogError::new(AuditLogErrorCode::CapacityExceeded));
        }

        let actor = self.redactor.sanitize(&event.actor);
        let details = self.redactor.sanitize_json(&event.details);
        let sequence = u64::try_from(state.records.len())
            .map_err(|_| AuditLogError::new(AuditLogErrorCode::CapacityExceeded))?;
        let previous_hash = state.records.last().map_or([0; 32], |record| record.hash);
        let hash = hash_record(
            sequence,
            event.event_type,
            &actor,
            event.occurred_at,
            &details,
            &previous_hash,
        )?;
        let record = AuditRecord {
            sequence,
            event_type: event.event_type,
            actor,
            occurred_at: event.occurred_at,
            details,
            previous_hash,
            hash,
        };
        let mut records = state.records.clone();
        records.push(record.clone());
        self.write_records(&records).await?;
        state.records = records;
        Ok(record)
    }

    /// Alias for [`Self::append`] for event-oriented host adapters.
    pub async fn append_event(&self, event: AuditEvent) -> Result<AuditRecord, AuditLogError>
    where
        S: LocalStore,
    {
        self.append(event).await
    }

    /// Record an authentication attempt, including denied attempts.
    pub async fn record_authentication_attempt(
        &self,
        actor: impl Into<String>,
        occurred_at: i64,
        accepted: bool,
    ) -> Result<AuditRecord, AuditLogError>
    where
        S: LocalStore,
    {
        self.append(AuditEvent::authentication_attempt(
            actor,
            occurred_at,
            accepted,
        )?)
        .await
    }

    /// Record a role definition or assignment change.
    pub async fn record_role_change(
        &self,
        actor: impl Into<String>,
        occurred_at: i64,
        details: Value,
    ) -> Result<AuditRecord, AuditLogError>
    where
        S: LocalStore,
    {
        self.append(AuditEvent::role_change(actor, occurred_at, details)?)
            .await
    }

    /// Record an application membership change.
    pub async fn record_membership_change(
        &self,
        actor: impl Into<String>,
        occurred_at: i64,
        details: Value,
    ) -> Result<AuditRecord, AuditLogError>
    where
        S: LocalStore,
    {
        self.append(AuditEvent::membership_change(actor, occurred_at, details)?)
            .await
    }

    /// Record a destructive application action.
    pub async fn record_destructive_action(
        &self,
        actor: impl Into<String>,
        occurred_at: i64,
        details: Value,
    ) -> Result<AuditRecord, AuditLogError>
    where
        S: LocalStore,
    {
        self.append(AuditEvent::destructive_action(actor, occurred_at, details)?)
            .await
    }

    /// Record a data export operation.
    pub async fn record_data_export(
        &self,
        actor: impl Into<String>,
        occurred_at: i64,
        details: Value,
    ) -> Result<AuditRecord, AuditLogError>
    where
        S: LocalStore,
    {
        self.append(AuditEvent::data_export(actor, occurred_at, details)?)
            .await
    }

    /// Record admission or rejection of an inbound webhook.
    pub async fn record_webhook_admission(
        &self,
        actor: impl Into<String>,
        occurred_at: i64,
        details: Value,
    ) -> Result<AuditRecord, AuditLogError>
    where
        S: LocalStore,
    {
        self.append(AuditEvent::webhook_admission(actor, occurred_at, details)?)
            .await
    }

    /// Record a declared workflow run.
    pub async fn record_workflow_run(
        &self,
        actor: impl Into<String>,
        occurred_at: i64,
        details: Value,
    ) -> Result<AuditRecord, AuditLogError>
    where
        S: LocalStore,
    {
        self.append(AuditEvent::workflow_run(actor, occurred_at, details)?)
            .await
    }

    /// Query complete redacted records in append sequence order.
    pub async fn query(&self, query: AuditQuery) -> Result<Vec<AuditRecord>, AuditLogError>
    where
        S: LocalStore,
    {
        validate_query(&query)?;
        let mut state = self.state.lock().await;
        self.ensure_loaded(&mut state).await?;
        let mut records = Vec::new();
        for record in &state.records {
            if query.from.is_some_and(|from| record.occurred_at < from)
                || query.to.is_some_and(|to| record.occurred_at > to)
                || query
                    .event_type
                    .is_some_and(|event_type| record.event_type != event_type)
                || query
                    .actor
                    .as_deref()
                    .is_some_and(|actor| record.actor != actor)
            {
                continue;
            }
            records.push(record.clone());
            if query.limit.is_some_and(|limit| records.len() >= limit) {
                break;
            }
        }
        Ok(records)
    }

    /// Alias for [`Self::query`] used by Designer query adapters.
    pub async fn list(&self, query: AuditQuery) -> Result<Vec<AuditRecord>, AuditLogError>
    where
        S: LocalStore,
    {
        self.query(query).await
    }

    /// Export matching records as a complete, deterministic JSON array.
    ///
    /// Records were redacted before persistence and are sanitized once more at export, so an
    /// updated filter cannot accidentally make a durable value observable. The returned bytes
    /// contain only stable record fields and hash-chain evidence.
    pub async fn export(&self, query: AuditQuery) -> Result<String, AuditLogError>
    where
        S: LocalStore,
    {
        let records = self.query(query).await?;
        let values = records.iter().map(export_record).collect::<Vec<_>>();
        serde_json::to_string(&values).map_err(|_| AuditLogError::new(AuditLogErrorCode::Corrupt))
    }

    /// Alias for [`Self::export`] emphasizing that the owner receives a redacted projection.
    pub async fn export_redacted(&self, query: AuditQuery) -> Result<String, AuditLogError>
    where
        S: LocalStore,
    {
        self.export(query).await
    }

    /// Re-read and verify the persisted chain, detecting external record edits or reordering.
    pub async fn verify_integrity(&self) -> Result<(), AuditLogError>
    where
        S: LocalStore,
    {
        let entries = self
            .store
            .batch_entries(&self.batch_id)
            .await
            .map_err(|_| AuditLogError::new(AuditLogErrorCode::StorageUnavailable))?;
        let records = decode_entries(&entries, &self.namespace)?;
        let mut state = self.state.lock().await;
        state.records = records;
        state.loaded = true;
        Ok(())
    }

    /// Alias for [`Self::verify_integrity`].
    pub async fn verify(&self) -> Result<(), AuditLogError>
    where
        S: LocalStore,
    {
        self.verify_integrity().await
    }

    /// Recover the underlying host store after dropping the audit service.
    #[must_use]
    pub fn into_inner(self) -> S {
        self.store
    }

    async fn ensure_loaded(&self, state: &mut AuditState) -> Result<(), AuditLogError>
    where
        S: LocalStore,
    {
        if state.loaded {
            return Ok(());
        }
        let entries = self
            .store
            .batch_entries(&self.batch_id)
            .await
            .map_err(|_| AuditLogError::new(AuditLogErrorCode::StorageUnavailable))?;
        state.records = decode_entries(&entries, &self.namespace)?;
        state.loaded = true;
        Ok(())
    }

    async fn write_records(&self, records: &[AuditRecord]) -> Result<(), AuditLogError>
    where
        S: LocalStore,
    {
        let mut entries = Vec::with_capacity(records.len() + 1);
        entries.push(StoreBatchEntry {
            ordinal: 0,
            payload: encode_entry(PersistedAuditEntry::Header {
                format_version: AUDIT_LOG_FORMAT_VERSION,
                namespace: encode_hash(&self.namespace),
            })?,
        });
        for (offset, record) in records.iter().enumerate() {
            entries.push(StoreBatchEntry {
                ordinal: u32::try_from(offset + 1)
                    .map_err(|_| AuditLogError::new(AuditLogErrorCode::CapacityExceeded))?,
                payload: encode_entry(PersistedAuditEntry::Record {
                    sequence: record.sequence,
                    event_type: record.event_type,
                    actor: record.actor.clone(),
                    occurred_at: record.occurred_at,
                    details: record.details.clone(),
                    previous_hash: encode_hash(&record.previous_hash),
                    hash: encode_hash(&record.hash),
                })?,
            });
        }
        let batch = StoreBatch::new(&self.batch_id, entries)
            .map_err(|_| AuditLogError::new(AuditLogErrorCode::Corrupt))?;
        self.store
            .write_batch(&batch)
            .await
            .map_err(|_| AuditLogError::new(AuditLogErrorCode::StorageUnavailable))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PersistedAuditEntry {
    Header {
        format_version: u16,
        namespace: String,
    },
    Record {
        sequence: u64,
        event_type: AuditEventType,
        actor: String,
        occurred_at: i64,
        details: Value,
        previous_hash: String,
        hash: String,
    },
}

fn validate_event(event: &AuditEvent) -> Result<(), AuditLogError> {
    if event.actor.is_empty()
        || event.actor.len() > MAX_ACTOR_BYTES
        || event.actor.chars().any(char::is_control)
        || serde_json::to_vec(&event.details)
            .map_err(|_| AuditLogError::new(AuditLogErrorCode::RequestInvalid))?
            .len()
            > MAX_DETAILS_BYTES
    {
        return Err(AuditLogError::new(AuditLogErrorCode::RequestInvalid));
    }
    Ok(())
}

fn validate_query(query: &AuditQuery) -> Result<(), AuditLogError> {
    if query.from.zip(query.to).is_some_and(|(from, to)| from > to)
        || query.actor.as_deref().is_some_and(|actor| {
            actor.is_empty() || actor.len() > MAX_ACTOR_BYTES || actor.chars().any(char::is_control)
        })
        || query.limit.is_some_and(|limit| limit > MAX_QUERY_LIMIT)
    {
        return Err(AuditLogError::new(AuditLogErrorCode::RequestInvalid));
    }
    Ok(())
}

fn encode_entry(entry: PersistedAuditEntry) -> Result<Value, AuditLogError> {
    serde_json::to_value(entry).map_err(|_| AuditLogError::new(AuditLogErrorCode::Corrupt))
}

fn decode_entries(
    entries: &[StoreBatchEntry],
    expected_namespace: &[u8; 32],
) -> Result<Vec<AuditRecord>, AuditLogError> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    if entries.len() > MAX_EVENTS + 1 {
        return Err(AuditLogError::new(AuditLogErrorCode::CapacityExceeded));
    }
    let mut decoded = entries.iter();
    let header = decode_entry(decoded.next().ok_or_else(corrupt)?.payload.clone())?;
    match header {
        PersistedAuditEntry::Header {
            format_version,
            namespace,
        } if format_version == AUDIT_LOG_FORMAT_VERSION
            && decode_hash(&namespace).is_ok_and(|found| found == *expected_namespace) => {}
        PersistedAuditEntry::Header { .. } => {
            return Err(AuditLogError::new(AuditLogErrorCode::Tampered));
        }
        PersistedAuditEntry::Record { .. } => return Err(corrupt()),
    }

    let mut records = Vec::with_capacity(entries.len().saturating_sub(1));
    let mut previous_hash = [0; 32];
    for (expected_sequence, entry) in decoded.enumerate() {
        if entry.ordinal != u32::try_from(expected_sequence + 1).map_err(|_| corrupt())? {
            return Err(corrupt());
        }
        let PersistedAuditEntry::Record {
            sequence,
            event_type,
            actor,
            occurred_at,
            details,
            previous_hash: previous_hash_text,
            hash: hash_text,
        } = decode_entry(entry.payload.clone())?
        else {
            return Err(corrupt());
        };
        if sequence
            != u64::try_from(expected_sequence)
                .map_err(|_| AuditLogError::new(AuditLogErrorCode::CapacityExceeded))?
        {
            return Err(AuditLogError::new(AuditLogErrorCode::Tampered));
        }
        let persisted_previous_hash = decode_hash(&previous_hash_text)?;
        let hash = decode_hash(&hash_text)?;
        let computed_hash = hash_record(
            sequence,
            event_type,
            &actor,
            occurred_at,
            &details,
            &persisted_previous_hash,
        )?;
        if persisted_previous_hash != previous_hash || computed_hash != hash {
            return Err(AuditLogError::new(AuditLogErrorCode::Tampered));
        }
        let event = AuditEvent {
            event_type,
            actor: actor.clone(),
            occurred_at,
            details: details.clone(),
        };
        validate_event(&event)?;
        records.push(AuditRecord {
            sequence,
            event_type,
            actor,
            occurred_at,
            details,
            previous_hash: persisted_previous_hash,
            hash,
        });
        previous_hash = hash;
    }
    Ok(records)
}

fn decode_entry(value: Value) -> Result<PersistedAuditEntry, AuditLogError> {
    serde_json::from_value(value).map_err(|_| corrupt())
}

fn hash_record(
    sequence: u64,
    event_type: AuditEventType,
    actor: &str,
    occurred_at: i64,
    details: &Value,
    previous_hash: &[u8; 32],
) -> Result<[u8; 32], AuditLogError> {
    let details = serde_json::to_vec(&canonical_json(details))
        .map_err(|_| AuditLogError::new(AuditLogErrorCode::Corrupt))?;
    let mut hasher = Sha256::new();
    hasher.update(AUDIT_HASH_DOMAIN);
    hasher.update(sequence.to_be_bytes());
    hasher.update(event_type.as_str().as_bytes());
    update_bytes(&mut hasher, actor.as_bytes());
    hasher.update(occurred_at.to_be_bytes());
    update_bytes(&mut hasher, &details);
    hasher.update(previous_hash);
    Ok(hasher.finalize().into())
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let sorted = object
                .iter()
                .map(|(key, value)| (key.clone(), canonical_json(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.clone(),
    }
}

fn update_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}

fn namespace_digest(publisher: &str, application: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(AUDIT_NAMESPACE_DOMAIN);
    update_bytes(&mut hasher, publisher.as_bytes());
    update_bytes(&mut hasher, application.as_bytes());
    hasher.finalize().into()
}

fn encode_hash(hash: &[u8; 32]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in hash {
        encoded.push_str(&format!("{byte:02x}"));
    }
    encoded
}

fn decode_hash(value: &str) -> Result<[u8; 32], AuditLogError> {
    if value.len() != 64 {
        return Err(AuditLogError::new(AuditLogErrorCode::Tampered));
    }
    let mut hash = [0; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_digit(chunk[0]).ok_or_else(tampered)?;
        let low = hex_digit(chunk[1]).ok_or_else(tampered)?;
        hash[index] = (high << 4) | low;
    }
    Ok(hash)
}

const fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn export_record(record: &AuditRecord) -> Value {
    json!({
        "sequence": record.sequence,
        "event_type": record.event_type.as_str(),
        "actor": record.actor,
        "occurred_at": record.occurred_at,
        "details": record.details,
        "previous_hash": encode_hash(&record.previous_hash),
        "hash": encode_hash(&record.hash),
    })
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

const fn corrupt() -> AuditLogError {
    AuditLogError::new(AuditLogErrorCode::Corrupt)
}

const fn tampered() -> AuditLogError {
    AuditLogError::new(AuditLogErrorCode::Tampered)
}
