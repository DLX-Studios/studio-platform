//! Closed guest contract for host-owned application data.

use std::{collections::BTreeMap, error::Error, fmt, future::Future};

use serde_json::Value;

/// Current derivation version for application data namespaces.
pub const APPLICATION_DATA_NAMESPACE_VERSION: u16 = 1;

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
    /// Namespace derivation version used by this partition.
    #[must_use]
    pub const fn version(self) -> u16 {
        self.version
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
        let fields = fields.into_iter().collect::<BTreeMap<_, _>>();
        if version == 0
            || fields.is_empty()
            || fields.len() > 256
            || fields.keys().any(|name| !valid_identifier(name, 64))
        {
            return Err(ApplicationDataError::new(
                ApplicationDataErrorCode::RequestInvalid,
            ));
        }
        Ok(Self { version, fields })
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

/// Guest-visible application data interface implemented by a host-bound application handle.
pub trait ApplicationDataGuestApi: Send + Sync {
    /// Execute a typed collection helper or reject a forbidden operation with a stable safe code.
    fn execute(
        &self,
        request: GuestDataRequest,
    ) -> impl Future<Output = Result<CollectionResponse, ApplicationDataError>> + Send;
}

fn valid_identifier(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}
