//! Closed guest contract for host-owned application data.

use std::{
    collections::BTreeMap,
    error::Error,
    fmt::{self, Write as _},
    future::Future,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use studio_security::PluginPrincipal;
use tokio::sync::Mutex;

use crate::{LocalStore, StoreBatch, StoreBatchEntry};

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
        })
    }
}

/// Guest data interface permanently bound to one application namespace and declaration set.
pub struct ApplicationDataHandle<'a, S> {
    host: &'a ApplicationDataHost<S>,
    namespace: ApplicationDataNamespace,
    collections: BTreeMap<String, RecordSchema>,
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
