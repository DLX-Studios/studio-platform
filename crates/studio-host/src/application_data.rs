//! Closed guest contract for host-owned application data.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt::{self, Write as _},
    future::Future,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use studio_security::PluginPrincipal;
use tokio::sync::Mutex;

use crate::{
    LocalStore, LocalStoreDiagnosticCode, StoreBatch, StoreBatchEntry, SurrealQueryStore,
};

/// Current derivation version for application data namespaces.
pub const APPLICATION_DATA_NAMESPACE_VERSION: u16 = 1;
const NAMESPACE_DOMAIN: &[u8] = b"studio.application-data.namespace.v1";
const COLLECTION_FORMAT_VERSION: u16 = 1;
const MAX_COLLECTIONS: usize = 128;
const MAX_RECORDS_PER_COLLECTION: usize = 10_000;
const MAX_RECORD_BYTES: usize = 256 * 1024;
const MAX_ARRAY_ITEMS: usize = 4096;
const MAX_SCHEMA_DEPTH: usize = 16;
const MAX_PATCH_OPERATIONS: usize = 256;
const HOST_MAX_QUERY_BYTES: usize = 64 * 1024;
const HOST_MAX_RESULT_BYTES: usize = 4 * 1024 * 1024;
const HOST_MAX_QUERY_DURATION: Duration = Duration::from_secs(10);

/// Opaque host-derived partition for one verified publisher/application pair.
///
/// The digest is deliberately not exposed. Guest requests are bound to a namespace by the host
/// after package verification and contain no namespace or database selector.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct ApplicationDataNamespace {
    version: u16,
    digest: [u8; 32],
}

impl ApplicationDataNamespace {
    /// Derive a stable partition from a host-verified publisher and application identity.
    ///
    /// Publisher signing-key rotation, bundle updates, and runtime restarts do not move data.
    /// The derivation is length-delimited and domain/version separated.
    #[must_use]
    pub fn derive(principal: &PluginPrincipal) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(NAMESPACE_DOMAIN);
        hasher.update(APPLICATION_DATA_NAMESPACE_VERSION.to_be_bytes());
        update_identity(&mut hasher, principal.publisher_id());
        update_identity(&mut hasher, principal.plugin_id());
        Self {
            version: APPLICATION_DATA_NAMESPACE_VERSION,
            digest: hasher.finalize().into(),
        }
    }

    /// Namespace derivation version used by this partition.
    #[must_use]
    pub const fn version(self) -> u16 {
        self.version
    }

    fn storage_prefix(self) -> String {
        let mut encoded = String::with_capacity(64);
        for byte in self.digest {
            let _ = write!(encoded, "{byte:02x}");
        }
        format!("appdata.v{}.{}", self.version, encoded)
    }
}

impl fmt::Debug for ApplicationDataNamespace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApplicationDataNamespace")
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

/// Stable, validated application record identifier.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RecordId(String);

impl RecordId {
    /// Validate a guest-selected record identifier.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationDataErrorCode::RequestInvalid`] for an empty, oversized, or unsafe
    /// identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ApplicationDataError> {
        let value = value.into();
        if !valid_identifier(&value, 128) {
            return Err(ApplicationDataError::new(
                ApplicationDataErrorCode::RequestInvalid,
            ));
        }
        Ok(Self(value))
    }

    /// Validated identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Closed set of value shapes accepted by declared application record schemas.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldType {
    /// JSON string.
    String,
    /// JSON boolean.
    Boolean,
    /// Integral JSON number representable by `i64` or `u64`.
    Integer,
    /// Any JSON number.
    Number,
    /// JSON object validated recursively against the nested record schema.
    Object(Box<RecordSchema>),
    /// JSON array whose entries share one declared type.
    Array(Box<FieldType>),
}

/// One field in a declared record schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDeclaration {
    value_type: FieldType,
    required: bool,
}

impl FieldDeclaration {
    /// Declare a required field.
    #[must_use]
    pub const fn required(value_type: FieldType) -> Self {
        Self {
            value_type,
            required: true,
        }
    }

    /// Declare an optional field.
    #[must_use]
    pub const fn optional(value_type: FieldType) -> Self {
        Self {
            value_type,
            required: false,
        }
    }

    /// Declared JSON value type.
    #[must_use]
    pub const fn value_type(&self) -> &FieldType {
        &self.value_type
    }

    /// Whether the field must be present.
    #[must_use]
    pub const fn is_required(&self) -> bool {
        self.required
    }
}

/// Closed, versioned schema for one collection record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordSchema {
    version: u32,
    fields: BTreeMap<String, FieldDeclaration>,
}

impl RecordSchema {
    /// Construct a schema from host-verified package declarations.
    ///
    /// Unknown record fields are always rejected.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationDataErrorCode::RequestInvalid`] for version zero, an empty schema,
    /// unsafe field names, or excessive field counts.
    pub fn new(
        version: u32,
        fields: impl IntoIterator<Item = (String, FieldDeclaration)>,
    ) -> Result<Self, ApplicationDataError> {
        if version == 0 {
            return Err(ApplicationDataError::new(
                ApplicationDataErrorCode::RequestInvalid,
            ));
        }
        let mut collected = BTreeMap::new();
        for (name, declaration) in fields {
            if collected.len() >= 256
                || !valid_identifier(&name, 64)
                || collected.insert(name, declaration).is_some()
            {
                return Err(ApplicationDataError::new(
                    ApplicationDataErrorCode::RequestInvalid,
                ));
            }
        }
        if collected.is_empty() {
            return Err(ApplicationDataError::new(
                ApplicationDataErrorCode::RequestInvalid,
            ));
        }
        Ok(Self {
            version,
            fields: collected,
        })
    }

    /// Package-declared schema version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Closed field declarations.
    #[must_use]
    pub const fn fields(&self) -> &BTreeMap<String, FieldDeclaration> {
        &self.fields
    }
}

/// One package-declared collection and its record schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionDeclaration {
    name: String,
    schema: RecordSchema,
}

impl CollectionDeclaration {
    /// Validate a collection declaration from a verified package.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationDataErrorCode::RequestInvalid`] for an unsafe collection name.
    pub fn new(
        name: impl Into<String>,
        schema: RecordSchema,
    ) -> Result<Self, ApplicationDataError> {
        let name = name.into();
        if !valid_identifier(&name, 64) {
            return Err(ApplicationDataError::new(
                ApplicationDataErrorCode::RequestInvalid,
            ));
        }
        Ok(Self { name, schema })
    }

    /// Stable declared collection name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Declared record schema.
    #[must_use]
    pub const fn schema(&self) -> &RecordSchema {
        &self.schema
    }
}

/// Bounded, top-level patch operation over declared record fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatchOperation {
    /// Add or replace one declared field.
    Set {
        /// Declared field name.
        field: String,
        /// Replacement JSON value.
        value: Value,
    },
    /// Remove one optional declared field.
    Remove {
        /// Declared field name.
        field: String,
    },
}

/// Typed collection helper request. No variant accepts a database handle or query string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CollectionRequest {
    /// Select one record by identifier.
    Select {
        /// Declared collection name.
        collection: String,
        /// Record identifier.
        id: RecordId,
    },
    /// List all records in stable identifier order.
    List {
        /// Declared collection name.
        collection: String,
    },
    /// Create a record that does not already exist.
    Create {
        /// Declared collection name.
        collection: String,
        /// Record identifier.
        id: RecordId,
        /// Complete record object.
        record: Value,
    },
    /// Shallow-merge declared fields into an existing record.
    UpdateMerge {
        /// Declared collection name.
        collection: String,
        /// Record identifier.
        id: RecordId,
        /// Partial record object.
        fields: Value,
    },
    /// Apply bounded field patch operations to an existing record.
    UpdatePatch {
        /// Declared collection name.
        collection: String,
        /// Record identifier.
        id: RecordId,
        /// Ordered patch operations.
        operations: Vec<PatchOperation>,
    },
    /// Delete one record.
    Delete {
        /// Declared collection name.
        collection: String,
        /// Record identifier.
        id: RecordId,
    },
}

/// Guest operation kinds that are intentionally outside the collection-helper contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForbiddenDataOperation {
    /// Attempt to submit raw query language.
    RawQuery,
    /// Attempt to select or switch a namespace.
    NamespaceSwitch,
    /// Attempt to select or switch a database.
    DatabaseSwitch,
}

/// Closed guest entry point for application data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuestDataRequest {
    /// Supported typed collection operation.
    Collection(CollectionRequest),
    /// Unsupported operation marker. No query text or target name crosses the boundary.
    Forbidden(ForbiddenDataOperation),
}

/// One stored application record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredRecord {
    /// Stable record identifier.
    pub id: RecordId,
    /// Schema-validated record object.
    pub value: Value,
}

/// Result variants returned by typed collection helpers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CollectionResponse {
    /// Result of selecting one record.
    Selected(Option<StoredRecord>),
    /// Stable ordered collection listing.
    Listed(Vec<StoredRecord>),
    /// Created or updated record.
    Written(StoredRecord),
    /// Whether a record was removed.
    Deleted(bool),
}

/// Host ceilings for one opt-in SurrealQL declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurrealQueryLimits {
    /// Maximum UTF-8 bytes in one query source.
    pub max_query_bytes: usize,
    /// Maximum JSON-encoded bytes in the returned value.
    pub max_result_bytes: usize,
    /// Maximum wall-clock time spent executing one query.
    pub max_duration: Duration,
}

impl SurrealQueryLimits {
    /// Construct limits, rejecting zero values and durations longer than the host ceiling.
    pub fn new(
        max_query_bytes: usize,
        max_result_bytes: usize,
        max_duration: Duration,
    ) -> Result<Self, SurrealQueryError> {
        if max_query_bytes == 0
            || max_result_bytes == 0
            || max_duration.is_zero()
            || max_query_bytes > HOST_MAX_QUERY_BYTES
            || max_result_bytes > HOST_MAX_RESULT_BYTES
            || max_duration > HOST_MAX_QUERY_DURATION
        {
            return Err(SurrealQueryError::new(
                SurrealQueryErrorCode::DeclarationInvalid,
                QuerySource::unknown(),
            ));
        }
        Ok(Self {
            max_query_bytes,
            max_result_bytes,
            max_duration,
        })
    }

    const fn is_valid(self) -> bool {
        self.max_query_bytes > 0
            && self.max_result_bytes > 0
            && !self.max_duration.is_zero()
            && self.max_query_bytes <= HOST_MAX_QUERY_BYTES
            && self.max_result_bytes <= HOST_MAX_RESULT_BYTES
            && self.max_duration <= HOST_MAX_QUERY_DURATION
    }
}

impl Default for SurrealQueryLimits {
    fn default() -> Self {
        Self {
            max_query_bytes: 16 * 1024,
            max_result_bytes: 1024 * 1024,
            max_duration: Duration::from_secs(1),
        }
    }
}

/// Signed-manifest declaration for the `data.surreal.query` capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurrealQueryDeclaration {
    limits: SurrealQueryLimits,
}

impl SurrealQueryDeclaration {
    /// Declare bounded SurrealQL with explicit per-application limits.
    #[must_use]
    pub const fn new(limits: SurrealQueryLimits) -> Self {
        Self { limits }
    }

    /// Limits admitted for this declaration.
    #[must_use]
    pub const fn limits(self) -> SurrealQueryLimits {
        self.limits
    }
}

/// Source location carried with a query so a rejection can be linked safely by the host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuerySource {
    /// One-based source line.
    pub line: u32,
    /// One-based source column.
    pub column: u32,
}

impl QuerySource {
    /// A stable fallback when an adapter has no source location.
    #[must_use]
    pub const fn unknown() -> Self {
        Self { line: 1, column: 1 }
    }
}

/// A query request with values kept structurally separate from query text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurrealQueryRequest {
    query: String,
    parameters: BTreeMap<String, Value>,
    source: QuerySource,
}

impl SurrealQueryRequest {
    /// Construct a request. Parameters are bound by the host and are never interpolated.
    #[must_use]
    pub fn new(query: impl Into<String>, parameters: BTreeMap<String, Value>) -> Self {
        Self {
            query: query.into(),
            parameters,
            source: QuerySource::unknown(),
        }
    }

    /// Attach a source location for diagnostics.
    #[must_use]
    pub const fn with_source(mut self, source: QuerySource) -> Self {
        self.source = QuerySource {
            line: source.line.max(1),
            column: source.column.max(1),
        };
        self
    }

    /// Query text supplied to the host validator.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Host-bound parameters, kept separate from query text.
    #[must_use]
    pub const fn parameters(&self) -> &BTreeMap<String, Value> {
        &self.parameters
    }

    /// Source location used by the resulting diagnostic.
    #[must_use]
    pub const fn source(&self) -> QuerySource {
        self.source
    }
}

/// JSON result returned by an authorized bounded query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurrealQueryResponse {
    /// First statement's JSON result.
    pub value: Value,
}

/// Compatibility alias for adapters that call the capability simply `QueryLimits`.
pub type QueryLimits = SurrealQueryLimits;
/// Compatibility alias for adapters that call the capability simply `QueryDeclaration`.
pub type QueryDeclaration = SurrealQueryDeclaration;
/// Compatibility alias for adapters that call the request simply `QueryRequest`.
pub type QueryRequest = SurrealQueryRequest;
/// Compatibility alias for adapters that call the response simply `QueryResponse`.
pub type QueryResponse = SurrealQueryResponse;

/// Safe query rejection family. No SurrealDB parser or storage details cross this boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurrealQueryErrorCode {
    /// The signed declaration did not admit this capability.
    CapabilityDenied,
    /// The declaration or request shape is invalid.
    DeclarationInvalid,
    /// Query source exceeded the admitted byte limit.
    QueryTooLarge,
    /// Query syntax or statement count is outside the bounded subset.
    QueryInvalid,
    /// Namespace/database/system access or an unsafe function was requested.
    Forbidden,
    /// A referenced collection was not declared by this application.
    CollectionUndeclared,
    /// A returned value exceeded the admitted byte limit.
    ResultTooLarge,
    /// Query execution exceeded the admitted wall-clock limit.
    TimedOut,
    /// The host storage operation failed.
    ExecutionFailed,
}

/// Source-linked, safe query diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SurrealQueryError {
    code: SurrealQueryErrorCode,
    source: QuerySource,
}

impl SurrealQueryError {
    const fn new(code: SurrealQueryErrorCode, source: QuerySource) -> Self {
        Self { code, source }
    }

    /// Stable machine-readable rejection code.
    #[must_use]
    pub const fn code(self) -> SurrealQueryErrorCode {
        self.code
    }

    /// Source location supplied by the guest adapter.
    #[must_use]
    pub const fn source(self) -> QuerySource {
        self.source
    }
}

impl fmt::Display for SurrealQueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            SurrealQueryErrorCode::CapabilityDenied => "surreal query capability denied",
            SurrealQueryErrorCode::DeclarationInvalid => "surreal query declaration invalid",
            SurrealQueryErrorCode::QueryTooLarge => "surreal query exceeds its host limit",
            SurrealQueryErrorCode::QueryInvalid => "surreal query is invalid",
            SurrealQueryErrorCode::Forbidden => "surreal query operation denied",
            SurrealQueryErrorCode::CollectionUndeclared => "query collection not declared",
            SurrealQueryErrorCode::ResultTooLarge => "surreal query result exceeds its host limit",
            SurrealQueryErrorCode::TimedOut => "surreal query timed out",
            SurrealQueryErrorCode::ExecutionFailed => "surreal query failed",
        })
    }
}

impl Error for SurrealQueryError {}

/// Stable, context-free application data failure family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationDataErrorCode {
    /// Malformed or unbounded guest request.
    RequestInvalid,
    /// Collection was not declared by the verified package.
    CollectionUndeclared,
    /// Record input did not satisfy the declared schema.
    SchemaViolation,
    /// Create targeted an existing record.
    RecordAlreadyExists,
    /// Update targeted a missing record.
    RecordNotFound,
    /// Guest attempted to address another application partition.
    CrossNamespaceDenied,
    /// Raw query entry points are not available to collection-helper guests.
    RawQueryDenied,
    /// Guest attempted to select or switch a namespace.
    NamespaceSwitchDenied,
    /// Guest attempted to select or switch a database.
    DatabaseSwitchDenied,
    /// Host-owned persistence was unavailable.
    StorageUnavailable,
    /// Persisted application data was malformed.
    StoredDataInvalid,
    /// Persisted collection schema differs from the declared package schema.
    SchemaVersionMismatch,
    /// The authenticated application session lacks the requested role or row grant.
    AuthorizationDenied,
}

/// Safe application data error with no record, namespace, or storage-engine context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApplicationDataError {
    code: ApplicationDataErrorCode,
}

impl ApplicationDataError {
    const fn new(code: ApplicationDataErrorCode) -> Self {
        Self { code }
    }

    pub(crate) const fn authorization_denied() -> Self {
        Self::new(ApplicationDataErrorCode::AuthorizationDenied)
    }

    /// Stable guest-safe code.
    #[must_use]
    pub const fn code(self) -> ApplicationDataErrorCode {
        self.code
    }
}

impl fmt::Display for ApplicationDataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self.code {
            ApplicationDataErrorCode::RequestInvalid => "application data request invalid",
            ApplicationDataErrorCode::CollectionUndeclared => "collection not declared",
            ApplicationDataErrorCode::SchemaViolation => "record schema validation failed",
            ApplicationDataErrorCode::RecordAlreadyExists => "record already exists",
            ApplicationDataErrorCode::RecordNotFound => "record not found",
            ApplicationDataErrorCode::CrossNamespaceDenied => "application namespace denied",
            ApplicationDataErrorCode::RawQueryDenied => "raw application data query denied",
            ApplicationDataErrorCode::NamespaceSwitchDenied => "namespace switch denied",
            ApplicationDataErrorCode::DatabaseSwitchDenied => "database switch denied",
            ApplicationDataErrorCode::StorageUnavailable => "application data unavailable",
            ApplicationDataErrorCode::StoredDataInvalid => "stored application data invalid",
            ApplicationDataErrorCode::SchemaVersionMismatch => {
                "application data schema migration required"
            }
            ApplicationDataErrorCode::AuthorizationDenied => "application data authorization denied",
        })
    }
}

impl Error for ApplicationDataError {}

/// Host-owned application data service over one private [`LocalStore`].
///
/// The mutex serializes collection read/modify/write cycles. It is owned by the host service,
/// shared by every application handle, and never crosses a guest boundary.
pub struct ApplicationDataHost<S> {
    store: S,
    operation_lock: Mutex<()>,
}

impl<S> ApplicationDataHost<S> {
    /// Wrap a LocalStore in the application-data mediation layer.
    #[must_use]
    pub const fn new(store: S) -> Self {
        Self {
            store,
            operation_lock: Mutex::const_new(()),
        }
    }

    /// Recover the underlying store after all bound handles have been dropped.
    #[must_use]
    pub fn into_inner(self) -> S {
        self.store
    }
}

impl<S: LocalStore> ApplicationDataHost<S> {
    /// Bind a guest interface to a verified principal and its package-declared collections.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationDataErrorCode::RequestInvalid`] for duplicate or excessive
    /// declarations. The returned handle contains no database selector, engine handle, or query
    /// surface.
    pub fn bind<'a>(
        &'a self,
        principal: &PluginPrincipal,
        declarations: impl IntoIterator<Item = CollectionDeclaration>,
    ) -> Result<ApplicationDataHandle<'a, S>, ApplicationDataError> {
        let mut collections = BTreeMap::new();
        for declaration in declarations {
            if collections.len() >= MAX_COLLECTIONS
                || collections
                    .insert(declaration.name.clone(), declaration.schema)
                    .is_some()
            {
                return Err(ApplicationDataError::new(
                    ApplicationDataErrorCode::RequestInvalid,
                ));
            }
        }
        if collections.is_empty() {
            return Err(ApplicationDataError::new(
                ApplicationDataErrorCode::RequestInvalid,
            ));
        }
        Ok(ApplicationDataHandle {
            host: self,
            namespace: ApplicationDataNamespace::derive(principal),
            collections,
            query: None,
        })
    }

    /// Bind a guest interface with the signed `data.surreal.query` declaration.
    ///
    /// This opt-in path requires a host query-capable store. Existing [`Self::bind`] callers
    /// remain collection-helper-only and cannot acquire a query handle accidentally.
    pub fn bind_with_query<'a>(
        &'a self,
        principal: &PluginPrincipal,
        declarations: impl IntoIterator<Item = CollectionDeclaration>,
        query: SurrealQueryDeclaration,
    ) -> Result<ApplicationDataHandle<'a, S>, ApplicationDataError>
    where
        S: SurrealQueryStore,
    {
        if !query.limits.is_valid() {
            return Err(ApplicationDataError::new(
                ApplicationDataErrorCode::RequestInvalid,
            ));
        }
        let mut handle = self.bind(principal, declarations)?;
        handle.query = Some(query.limits);
        Ok(handle)
    }

    /// Alias for [`Self::bind_with_query`] with the capability name made explicit.
    pub fn bind_with_surreal_query<'a>(
        &'a self,
        principal: &PluginPrincipal,
        declarations: impl IntoIterator<Item = CollectionDeclaration>,
        query: SurrealQueryDeclaration,
    ) -> Result<ApplicationDataHandle<'a, S>, ApplicationDataError>
    where
        S: SurrealQueryStore,
    {
        self.bind_with_query(principal, declarations, query)
    }
}

/// Guest data interface permanently bound to one application namespace and declaration set.
pub struct ApplicationDataHandle<'a, S> {
    host: &'a ApplicationDataHost<S>,
    namespace: ApplicationDataNamespace,
    collections: BTreeMap<String, RecordSchema>,
    query: Option<SurrealQueryLimits>,
}

impl<S> ApplicationDataHandle<'_, S> {
    /// Opaque bound namespace for host authorization and diagnostics correlation.
    #[must_use]
    pub const fn namespace(&self) -> ApplicationDataNamespace {
        self.namespace
    }

    /// Host-dispatch authorization guard for an already derived namespace.
    ///
    /// Guest collection requests never carry this value. Host adapters that multiplex requests
    /// use this check before dispatching an internally attributed request.
    ///
    /// # Errors
    ///
    /// Returns [`ApplicationDataErrorCode::CrossNamespaceDenied`] for every mismatch.
    pub fn authorize_namespace(
        &self,
        requested: ApplicationDataNamespace,
    ) -> Result<(), ApplicationDataError> {
        if requested != self.namespace {
            return Err(ApplicationDataError::new(
                ApplicationDataErrorCode::CrossNamespaceDenied,
            ));
        }
        Ok(())
    }
}

impl<S: LocalStore> ApplicationDataHandle<'_, S> {
    /// Read one host-private batch in this application's namespace.
    ///
    /// This is restricted to sibling host services such as RBAC and audit persistence. It is
    /// never exported through the guest API and still uses the same serialized store boundary.
    pub(crate) async fn internal_batch_entries(
        &self,
        suffix: &str,
    ) -> Result<Vec<StoreBatchEntry>, crate::LocalStoreError> {
        let _guard = self.host.operation_lock.lock().await;
        self.host
            .store
            .batch_entries(&format!("{}.{}", self.namespace.storage_prefix(), suffix))
            .await
    }

    /// Atomically replace one host-private batch in this application's namespace.
    pub(crate) async fn internal_write_batch(
        &self,
        suffix: &str,
        entries: impl IntoIterator<Item = StoreBatchEntry>,
    ) -> Result<(), crate::LocalStoreError> {
        let batch = StoreBatch::new(
            format!("{}.{}", self.namespace.storage_prefix(), suffix),
            entries,
        )?;
        let _guard = self.host.operation_lock.lock().await;
        self.host.store.write_batch(&batch).await
    }

    /// Select one record through the typed helper.
    ///
    /// # Errors
    ///
    /// Returns a stable safe code for undeclared collections, invalid persisted data, schema
    /// mismatch, or unavailable storage.
    pub async fn select(
        &self,
        collection: impl Into<String>,
        id: RecordId,
    ) -> Result<Option<StoredRecord>, ApplicationDataError> {
        match self
            .execute_collection(CollectionRequest::Select {
                collection: collection.into(),
                id,
            })
            .await?
        {
            CollectionResponse::Selected(record) => Ok(record),
            _ => Err(ApplicationDataError::new(
                ApplicationDataErrorCode::StoredDataInvalid,
            )),
        }
    }

    /// List a collection in stable record-identifier order.
    ///
    /// # Errors
    ///
    /// Returns a stable safe code for undeclared collections, invalid persisted data, schema
    /// mismatch, or unavailable storage.
    pub async fn list(
        &self,
        collection: impl Into<String>,
    ) -> Result<Vec<StoredRecord>, ApplicationDataError> {
        match self
            .execute_collection(CollectionRequest::List {
                collection: collection.into(),
            })
            .await?
        {
            CollectionResponse::Listed(records) => Ok(records),
            _ => Err(ApplicationDataError::new(
                ApplicationDataErrorCode::StoredDataInvalid,
            )),
        }
    }

    /// Create a schema-valid record.
    ///
    /// # Errors
    ///
    /// Returns a stable safe code when the collection is undeclared, the ID already exists, the
    /// record violates its schema or bounds, persisted data is invalid, or storage is unavailable.
    pub async fn create(
        &self,
        collection: impl Into<String>,
        id: RecordId,
        record: Value,
    ) -> Result<StoredRecord, ApplicationDataError> {
        self.write_helper(CollectionRequest::Create {
            collection: collection.into(),
            id,
            record,
        })
        .await
    }

    /// Shallow-merge declared fields into an existing record.
    ///
    /// # Errors
    ///
    /// Returns a stable safe code when the collection or record is absent, the merged record
    /// violates its schema, persisted data is invalid, or storage is unavailable.
    pub async fn update_merge(
        &self,
        collection: impl Into<String>,
        id: RecordId,
        fields: Value,
    ) -> Result<StoredRecord, ApplicationDataError> {
        self.write_helper(CollectionRequest::UpdateMerge {
            collection: collection.into(),
            id,
            fields,
        })
        .await
    }

    /// Apply bounded top-level field patch operations to an existing record.
    ///
    /// # Errors
    ///
    /// Returns a stable safe code when the collection or record is absent, a patch is invalid,
    /// the patched record violates its schema, persisted data is invalid, or storage is
    /// unavailable.
    pub async fn update_patch(
        &self,
        collection: impl Into<String>,
        id: RecordId,
        operations: Vec<PatchOperation>,
    ) -> Result<StoredRecord, ApplicationDataError> {
        self.write_helper(CollectionRequest::UpdatePatch {
            collection: collection.into(),
            id,
            operations,
        })
        .await
    }

    /// Delete one record, returning whether it existed.
    ///
    /// # Errors
    ///
    /// Returns a stable safe code for undeclared collections, invalid persisted data, schema
    /// mismatch, or unavailable storage.
    pub async fn delete(
        &self,
        collection: impl Into<String>,
        id: RecordId,
    ) -> Result<bool, ApplicationDataError> {
        match self
            .execute_collection(CollectionRequest::Delete {
                collection: collection.into(),
                id,
            })
            .await?
        {
            CollectionResponse::Deleted(deleted) => Ok(deleted),
            _ => Err(ApplicationDataError::new(
                ApplicationDataErrorCode::StoredDataInvalid,
            )),
        }
    }

    async fn write_helper(
        &self,
        request: CollectionRequest,
    ) -> Result<StoredRecord, ApplicationDataError> {
        match self.execute_collection(request).await? {
            CollectionResponse::Written(record) => Ok(record),
            _ => Err(ApplicationDataError::new(
                ApplicationDataErrorCode::StoredDataInvalid,
            )),
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn execute_collection(
        &self,
        request: CollectionRequest,
    ) -> Result<CollectionResponse, ApplicationDataError> {
        let _guard = self.host.operation_lock.lock().await;
        let collection_name = request.collection_name();
        let schema = self.collections.get(collection_name).ok_or_else(|| {
            ApplicationDataError::new(ApplicationDataErrorCode::CollectionUndeclared)
        })?;
        let batch_id = collection_batch_id(self.namespace, collection_name);
        let mut records = load_collection(&self.host.store, &batch_id, schema).await?;

        match request {
            CollectionRequest::Select { id, .. } => Ok(CollectionResponse::Selected(
                records.remove(&id).map(|value| StoredRecord { id, value }),
            )),
            CollectionRequest::List { .. } => Ok(CollectionResponse::Listed(
                records
                    .into_iter()
                    .map(|(id, value)| StoredRecord { id, value })
                    .collect(),
            )),
            CollectionRequest::Create { id, record, .. } => {
                validate_record(schema, &record)?;
                if records.contains_key(&id) {
                    return Err(ApplicationDataError::new(
                        ApplicationDataErrorCode::RecordAlreadyExists,
                    ));
                }
                if records.len() >= MAX_RECORDS_PER_COLLECTION {
                    return Err(ApplicationDataError::new(
                        ApplicationDataErrorCode::RequestInvalid,
                    ));
                }
                records.insert(id.clone(), record.clone());
                write_collection(&self.host.store, &batch_id, schema, &records).await?;
                Ok(CollectionResponse::Written(StoredRecord {
                    id,
                    value: record,
                }))
            }
            CollectionRequest::UpdateMerge { id, fields, .. } => {
                let fields = fields.as_object().ok_or(schema_violation())?;
                if fields.is_empty() {
                    return Err(ApplicationDataError::new(
                        ApplicationDataErrorCode::RequestInvalid,
                    ));
                }
                let record = records.get_mut(&id).ok_or(record_not_found())?;
                let object = record.as_object_mut().ok_or(stored_data_invalid())?;
                for (field, value) in fields {
                    object.insert(field.clone(), value.clone());
                }
                validate_record(schema, record)?;
                let written = record.clone();
                write_collection(&self.host.store, &batch_id, schema, &records).await?;
                Ok(CollectionResponse::Written(StoredRecord {
                    id,
                    value: written,
                }))
            }
            CollectionRequest::UpdatePatch { id, operations, .. } => {
                if operations.is_empty() || operations.len() > MAX_PATCH_OPERATIONS {
                    return Err(ApplicationDataError::new(
                        ApplicationDataErrorCode::RequestInvalid,
                    ));
                }
                let record = records.get_mut(&id).ok_or(record_not_found())?;
                let object = record.as_object_mut().ok_or(stored_data_invalid())?;
                for operation in operations {
                    match operation {
                        PatchOperation::Set { field, value } => {
                            if !valid_identifier(&field, 64) {
                                return Err(schema_violation());
                            }
                            object.insert(field, value);
                        }
                        PatchOperation::Remove { field } => {
                            let declaration =
                                schema.fields().get(&field).ok_or(schema_violation())?;
                            if declaration.is_required() {
                                return Err(schema_violation());
                            }
                            object.remove(&field);
                        }
                    }
                }
                validate_record(schema, record)?;
                let written = record.clone();
                write_collection(&self.host.store, &batch_id, schema, &records).await?;
                Ok(CollectionResponse::Written(StoredRecord {
                    id,
                    value: written,
                }))
            }
            CollectionRequest::Delete { id, .. } => {
                let deleted = records.remove(&id).is_some();
                if deleted {
                    write_collection(&self.host.store, &batch_id, schema, &records).await?;
                }
                Ok(CollectionResponse::Deleted(deleted))
            }
        }
    }
}

impl<S: SurrealQueryStore> ApplicationDataHandle<'_, S> {
    /// Execute one bounded, host-scoped SurrealQL request.
    ///
    /// The query declaration is the only way to obtain this method. Query variables are passed
    /// to SurrealDB through its native binding API; values are never formatted into query text.
    /// Every table target is checked against the package declaration and rewritten to an opaque
    /// namespace-local table name before reaching the private engine.
    pub async fn query(
        &self,
        request: SurrealQueryRequest,
    ) -> Result<SurrealQueryResponse, SurrealQueryError> {
        let source = request.source;
        let Some(limits) = self.query else {
            return Err(SurrealQueryError::new(
                SurrealQueryErrorCode::CapabilityDenied,
                source,
            ));
        };
        if request.query.is_empty() || request.query.len() > limits.max_query_bytes {
            return Err(SurrealQueryError::new(
                SurrealQueryErrorCode::QueryTooLarge,
                source,
            ));
        }
        validate_parameters(&request.parameters, source)?;
        let scoped = scope_query(&request.query, self.namespace, &self.collections, &request.parameters)
            .map_err(|code| SurrealQueryError::new(code, source))?;
        let value = self
            .host
            .store
            .execute_surreal_query(&scoped, request.parameters, limits.max_duration)
            .await
            .map_err(|error| {
                let code = if error.diagnostic().code() == LocalStoreDiagnosticCode::QueryTimedOut
                {
                    SurrealQueryErrorCode::TimedOut
                } else {
                    SurrealQueryErrorCode::ExecutionFailed
                };
                SurrealQueryError::new(code, source)
            })?;
        let bytes = serde_json::to_vec(&value).map_err(|_| {
            SurrealQueryError::new(SurrealQueryErrorCode::ExecutionFailed, source)
        })?;
        if bytes.len() > limits.max_result_bytes {
            return Err(SurrealQueryError::new(
                SurrealQueryErrorCode::ResultTooLarge,
                source,
            ));
        }
        Ok(SurrealQueryResponse { value })
    }
}

/// Guest-facing query interface implemented only by a query-capable application binding.
pub trait ApplicationDataQueryGuestApi: Send + Sync {
    /// Execute one host-validated, namespace-scoped query.
    fn query(
        &self,
        request: SurrealQueryRequest,
    ) -> impl Future<Output = Result<SurrealQueryResponse, SurrealQueryError>> + Send;
}

impl<S: SurrealQueryStore> ApplicationDataQueryGuestApi for ApplicationDataHandle<'_, S> {
    fn query(
        &self,
        request: SurrealQueryRequest,
    ) -> impl Future<Output = Result<SurrealQueryResponse, SurrealQueryError>> + Send {
        ApplicationDataHandle::query(self, request)
    }
}

impl CollectionRequest {
    fn collection_name(&self) -> &str {
        match self {
            Self::Select { collection, .. }
            | Self::List { collection }
            | Self::Create { collection, .. }
            | Self::UpdateMerge { collection, .. }
            | Self::UpdatePatch { collection, .. }
            | Self::Delete { collection, .. } => collection,
        }
    }
}

/// Guest-visible application data interface implemented by a host-bound application handle.
pub trait ApplicationDataGuestApi: Send + Sync {
    /// Execute a typed collection helper or reject a forbidden operation with a stable safe code.
    ///
    /// # Errors
    ///
    /// Returns only stable safe application-data codes; storage, namespace, record, and schema
    /// details are not included.
    fn execute(
        &self,
        request: GuestDataRequest,
    ) -> impl Future<Output = Result<CollectionResponse, ApplicationDataError>> + Send;
}

impl<S: LocalStore> ApplicationDataGuestApi for ApplicationDataHandle<'_, S> {
    fn execute(
        &self,
        request: GuestDataRequest,
    ) -> impl Future<Output = Result<CollectionResponse, ApplicationDataError>> + Send {
        async move {
            match request {
                GuestDataRequest::Collection(request) => self.execute_collection(request).await,
                GuestDataRequest::Forbidden(operation) => {
                    Err(ApplicationDataError::new(match operation {
                        ForbiddenDataOperation::RawQuery => {
                            ApplicationDataErrorCode::RawQueryDenied
                        }
                        ForbiddenDataOperation::NamespaceSwitch => {
                            ApplicationDataErrorCode::NamespaceSwitchDenied
                        }
                        ForbiddenDataOperation::DatabaseSwitch => {
                            ApplicationDataErrorCode::DatabaseSwitchDenied
                        }
                    }))
                }
            }
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PersistedCollectionEntry {
    Header {
        format_version: u16,
        schema_version: u32,
    },
    Record {
        id: String,
        value: Value,
    },
}

async fn load_collection<S: LocalStore>(
    store: &S,
    batch_id: &str,
    schema: &RecordSchema,
) -> Result<BTreeMap<RecordId, Value>, ApplicationDataError> {
    let entries = store
        .batch_entries(batch_id)
        .await
        .map_err(|_| storage_unavailable())?;
    if entries.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut entries = entries.into_iter();
    let header = decode_entry(entries.next().ok_or(stored_data_invalid())?.payload)?;
    match header {
        PersistedCollectionEntry::Header {
            format_version,
            schema_version,
        } if format_version == COLLECTION_FORMAT_VERSION && schema_version == schema.version() => {}
        PersistedCollectionEntry::Header { .. } => {
            return Err(ApplicationDataError::new(
                ApplicationDataErrorCode::SchemaVersionMismatch,
            ));
        }
        PersistedCollectionEntry::Record { .. } => return Err(stored_data_invalid()),
    }

    let mut records = BTreeMap::new();
    for entry in entries {
        let PersistedCollectionEntry::Record { id, value } = decode_entry(entry.payload)? else {
            return Err(stored_data_invalid());
        };
        let id = RecordId::new(id).map_err(|_| stored_data_invalid())?;
        validate_record(schema, &value).map_err(|_| stored_data_invalid())?;
        if records.insert(id, value).is_some() || records.len() > MAX_RECORDS_PER_COLLECTION {
            return Err(stored_data_invalid());
        }
    }
    Ok(records)
}

async fn write_collection<S: LocalStore>(
    store: &S,
    batch_id: &str,
    schema: &RecordSchema,
    records: &BTreeMap<RecordId, Value>,
) -> Result<(), ApplicationDataError> {
    let mut entries = Vec::with_capacity(records.len() + 1);
    entries.push(StoreBatchEntry {
        ordinal: 0,
        payload: encode_entry(PersistedCollectionEntry::Header {
            format_version: COLLECTION_FORMAT_VERSION,
            schema_version: schema.version(),
        })?,
    });
    for (offset, (id, value)) in records.iter().enumerate() {
        entries.push(StoreBatchEntry {
            ordinal: u32::try_from(offset + 1).map_err(|_| request_invalid())?,
            payload: encode_entry(PersistedCollectionEntry::Record {
                id: id.as_str().to_owned(),
                value: value.clone(),
            })?,
        });
    }
    let batch = StoreBatch::new(batch_id, entries).map_err(|_| storage_unavailable())?;
    store
        .write_batch(&batch)
        .await
        .map_err(|_| storage_unavailable())
}

fn encode_entry(entry: PersistedCollectionEntry) -> Result<Value, ApplicationDataError> {
    serde_json::to_value(entry).map_err(|_| stored_data_invalid())
}

fn decode_entry(value: Value) -> Result<PersistedCollectionEntry, ApplicationDataError> {
    serde_json::from_value(value).map_err(|_| stored_data_invalid())
}

fn validate_record(schema: &RecordSchema, value: &Value) -> Result<(), ApplicationDataError> {
    if serde_json::to_vec(value)
        .map_err(|_| schema_violation())?
        .len()
        > MAX_RECORD_BYTES
    {
        return Err(schema_violation());
    }
    validate_object(schema, value, 0)
}

fn validate_object(
    schema: &RecordSchema,
    value: &Value,
    depth: usize,
) -> Result<(), ApplicationDataError> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(schema_violation());
    }
    let object = value.as_object().ok_or(schema_violation())?;
    if object
        .keys()
        .any(|field| !schema.fields().contains_key(field))
    {
        return Err(schema_violation());
    }
    for (field, declaration) in schema.fields() {
        match object.get(field) {
            Some(value) => validate_field_type(declaration.value_type(), value, depth + 1)?,
            None if declaration.is_required() => return Err(schema_violation()),
            None => {}
        }
    }
    Ok(())
}

fn validate_field_type(
    value_type: &FieldType,
    value: &Value,
    depth: usize,
) -> Result<(), ApplicationDataError> {
    let valid = match value_type {
        FieldType::String => value.is_string(),
        FieldType::Boolean => value.is_boolean(),
        FieldType::Integer => value.as_i64().is_some() || value.as_u64().is_some(),
        FieldType::Number => value.is_number(),
        FieldType::Object(schema) => return validate_object(schema, value, depth),
        FieldType::Array(item_type) => {
            let Some(items) = value.as_array() else {
                return Err(schema_violation());
            };
            if items.len() > MAX_ARRAY_ITEMS || depth > MAX_SCHEMA_DEPTH {
                return Err(schema_violation());
            }
            for item in items {
                validate_field_type(item_type, item, depth + 1)?;
            }
            true
        }
    };
    if valid {
        Ok(())
    } else {
        Err(schema_violation())
    }
}

fn collection_batch_id(namespace: ApplicationDataNamespace, collection: &str) -> String {
    format!("{}.{}", namespace.storage_prefix(), collection)
}

fn update_identity(hasher: &mut Sha256, identity: &str) {
    hasher.update(
        u64::try_from(identity.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    hasher.update(identity.as_bytes());
}

const fn request_invalid() -> ApplicationDataError {
    ApplicationDataError::new(ApplicationDataErrorCode::RequestInvalid)
}

const fn schema_violation() -> ApplicationDataError {
    ApplicationDataError::new(ApplicationDataErrorCode::SchemaViolation)
}

const fn record_not_found() -> ApplicationDataError {
    ApplicationDataError::new(ApplicationDataErrorCode::RecordNotFound)
}

const fn storage_unavailable() -> ApplicationDataError {
    ApplicationDataError::new(ApplicationDataErrorCode::StorageUnavailable)
}

const fn stored_data_invalid() -> ApplicationDataError {
    ApplicationDataError::new(ApplicationDataErrorCode::StoredDataInvalid)
}

fn valid_identifier(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

#[derive(Clone, Copy)]
struct QueryToken<'a> {
    text: &'a str,
    start: usize,
    end: usize,
    quoted: bool,
}

fn validate_parameters(
    parameters: &BTreeMap<String, Value>,
    source: QuerySource,
) -> Result<(), SurrealQueryError> {
    if parameters.len() > 256 {
        return Err(SurrealQueryError::new(
            SurrealQueryErrorCode::QueryInvalid,
            source,
        ));
    }
    let encoded = serde_json::to_vec(parameters).map_err(|_| {
        SurrealQueryError::new(SurrealQueryErrorCode::QueryInvalid, source)
    })?;
    if encoded.len() > HOST_MAX_RESULT_BYTES {
        return Err(SurrealQueryError::new(
            SurrealQueryErrorCode::QueryInvalid,
            source,
        ));
    }
    if parameters
        .keys()
        .any(|name| !valid_identifier(name, 64))
    {
        return Err(SurrealQueryError::new(
            SurrealQueryErrorCode::QueryInvalid,
            source,
        ));
    }
    Ok(())
}

fn scope_query(
    query: &str,
    namespace: ApplicationDataNamespace,
    collections: &BTreeMap<String, RecordSchema>,
    parameters: &BTreeMap<String, Value>,
) -> Result<String, SurrealQueryErrorCode> {
    let tokens = lex_query(query).ok_or(SurrealQueryErrorCode::QueryInvalid)?;
    if tokens.is_empty() || tokens.iter().any(|token| token.text == ";") {
        return Err(SurrealQueryErrorCode::QueryInvalid);
    }
    let first = tokens[0].text.to_ascii_lowercase();
    if !matches!(first.as_str(), "select" | "create" | "update" | "upsert" | "delete") {
        return Err(SurrealQueryErrorCode::Forbidden);
    }
    if first == "delete" && !tokens.iter().any(|token| token.text.eq_ignore_ascii_case("from")) {
        return Err(SurrealQueryErrorCode::QueryInvalid);
    }
    let forbidden = [
        "use", "info", "define", "remove", "option", "live", "kill", "sleep", "system",
        "information_schema", "meta", "auth", "session", "file", "http", "https", "script",
        "javascript", "js", "function", "fn", "os", "process", "filesystem",
    ];
    for (index, token) in tokens.iter().enumerate() {
        let lower = token.text.to_ascii_lowercase();
        if token.text == ":"
            && tokens
                .get(index + 1)
                .is_some_and(|next| next.text == ":")
        {
            return Err(SurrealQueryErrorCode::Forbidden);
        }
        if !token.quoted && forbidden.contains(&lower.as_str()) {
            return Err(SurrealQueryErrorCode::Forbidden);
        }
        if !token.quoted && token.text.starts_with('$') {
            let name = &token.text[1..];
            if name.is_empty() || !parameters.contains_key(name) {
                return Err(SurrealQueryErrorCode::QueryInvalid);
            }
        }
    }

    let mut replacements = Vec::new();
    let mut expect_table = false;
    let mut table_list = false;
    for token in &tokens {
        let lower = token.text.to_ascii_lowercase();
        if expect_table {
            if token.quoted || !collections.contains_key(token.text) {
                return Err(SurrealQueryErrorCode::CollectionUndeclared);
            }
            replacements.push((token.start, token.end, scoped_table_name(namespace, token.text)));
            expect_table = false;
            table_list = true;
        } else if table_list && token.text == "," {
            expect_table = true;
        } else if table_list
            && matches!(
                lower.as_str(),
                "where"
                    | "split"
                    | "group"
                    | "order"
                    | "limit"
                    | "start"
                    | "fetch"
                    | "set"
                    | "content"
                    | "return"
                    | "timeout"
            )
        {
            table_list = false;
        }
        if matches!(
            lower.as_str(),
            "from" | "into" | "update" | "create" | "upsert" | "join"
        ) {
            expect_table = true;
        }
    }
    if expect_table {
        return Err(SurrealQueryErrorCode::QueryInvalid);
    }
    let mut scoped = query.to_owned();
    for (start, end, replacement) in replacements.into_iter().rev() {
        scoped.replace_range(start..end, &replacement);
    }
    Ok(scoped)
}

fn scoped_table_name(namespace: ApplicationDataNamespace, collection: &str) -> String {
    format!("appdata_v{}_{}_{}", namespace.version, hex_digest(namespace.digest), collection)
}

fn hex_digest(digest: [u8; 32]) -> String {
    let mut value = String::with_capacity(64);
    for byte in digest {
        let _ = write!(value, "{byte:02x}");
    }
    value
}

fn lex_query(query: &str) -> Option<Vec<QueryToken<'_>>> {
    let bytes = query.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index += 2;
            let start = index;
            while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/') {
                index += 1;
            }
            if index + 1 >= bytes.len() {
                return None;
            }
            index += 2;
            if start == index {
                return None;
            }
            continue;
        }
        let start = index;
        let quoted = matches!(bytes[index], b'\'' | b'"' | b'`');
        if quoted {
            let quote = bytes[index];
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index = index.saturating_add(2);
                } else if bytes[index] == quote {
                    index += 1;
                    break;
                } else {
                    index += 1;
                }
            }
            if index > bytes.len() || bytes.get(index.saturating_sub(1)) != Some(&quote) {
                return None;
            }
        } else if bytes[index].is_ascii_alphanumeric()
            || matches!(bytes[index], b'_' | b'$' | b'.' | b'-')
        {
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric()
                    || matches!(bytes[index], b'_' | b'$' | b'.' | b'-'))
            {
                index += 1;
            }
        } else {
            index += 1;
        }
        tokens.push(QueryToken {
            text: &query[start..index],
            start,
            end: index,
            quoted,
        });
    }
    Some(tokens)
}
