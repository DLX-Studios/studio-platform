//! Schema-aware content collections, typed bindings, fixture previews, and declarative form validation.
//!
//! This module is host-independent and rendering/UI agnostic. It owns the
//! deterministic domain logic that the command engine, diagnostics, and any
//! renderer/preview caller share. UI/GPUI and Library asset integration are
//! intentionally narrow: callers pass owned snapshots and receive owned values.
//!
//! ## Seams and gaps (ticket 48)
//!
//! Ticket 38 command families are landed in the shared command registry; content
//! collection and binding variants below execute through the same
//! `DesignerSession::submit` seam and retain their preview/validation/
//! diagnostics contracts.
//! - **Ticket 48 (Library asset APIs)** is not yet landed. `LibraryAssetId` field kind
//!   (`ContentFieldKind::Asset`) is reserved for Library collections, but this crate does
//!   not depend on any 48 Library admission or asset-resolution API. Content collections
//!   are isolated on `StudioDesign.collections`; a future Library-backed collection can
//!   be projected into this shape or share the same `CollectionId` namespace. Rendering
//!   of `PropertyValue::Asset` via Library remains a narrow UI integration behind
//!   `resolve_binding_value`/`preview_collection` (callers decide how to fetch assets).
//! - **Rendering/UI integration** is deliberately narrow: `preview_collection` and
//!   `resolve_binding_value` are pure functions over `StudioDesignSnapshot`. GPUI or any
//!   renderer can stamp repeated nodes deterministically without coupling to this crate.

use std::collections::BTreeMap;

use regex::Regex;

use crate::model::{
    CollectionId, ContentBinding, ContentCollection, ContentCollectionSchema, ContentFieldKind,
    ContentFixture, ContentRecord, DesignerDiagnostic, DiagnosticSeverity, FixtureKind,
    FormDefinition, FormValidationResult, PropertyValue, RecordId, STUDIO_DESIGN_SCHEMA_VERSION,
    StudioDesign,
};

/// Validate that a field name is `1..=128` bytes without control characters.
fn valid_field_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

/// Whether a [`PropertyValue`] matches a [`ContentFieldKind`] for typed bindings.
#[must_use]
pub fn property_value_matches_kind(value: &PropertyValue, kind: ContentFieldKind) -> bool {
    matches!(
        (value, kind),
        (PropertyValue::String(_), ContentFieldKind::String)
            | (PropertyValue::Integer(_), ContentFieldKind::Integer)
            | (PropertyValue::Decimal(_), ContentFieldKind::Decimal)
            | (PropertyValue::Boolean(_), ContentFieldKind::Boolean)
            | (PropertyValue::Color(_), ContentFieldKind::Color)
            | (PropertyValue::Length(_), ContentFieldKind::Length)
            | (PropertyValue::Asset(_), ContentFieldKind::Asset)
    )
}

/// Whether a [`ContentFieldKind`] is compatible with a [`PropertyValue`] used as a fallback.
///
/// Fallbacks are authored as `PropertyValue`s, so they must match the binding's
/// expected kind to be considered valid.
#[must_use]
pub fn fallback_matches_kind(fallback: &PropertyValue, kind: ContentFieldKind) -> bool {
    property_value_matches_kind(fallback, kind)
}

/// Validate a collection schema.
#[must_use]
pub fn validate_collection_schema(schema: &ContentCollectionSchema) -> Vec<DesignerDiagnostic> {
    let mut diagnostics = Vec::new();
    if schema.schema_version != STUDIO_DESIGN_SCHEMA_VERSION {
        diagnostics.push(DesignerDiagnostic {
            code: "CONTENT_SCHEMA_VERSION_INVALID".to_owned(),
            severity: DiagnosticSeverity::Error,
            message: "a collection schema has an unsupported schema version".to_owned(),
            node_id: None,
            interaction_id: None,
            collection_id: None,
            binding_id: None,
            form_id: None,
            record_id: None,
        });
    }
    if schema.fields.is_empty() {
        diagnostics.push(DesignerDiagnostic {
            code: "CONTENT_SCHEMA_EMPTY".to_owned(),
            severity: DiagnosticSeverity::Error,
            message: "a collection schema must declare at least one field".to_owned(),
            node_id: None,
            interaction_id: None,
            collection_id: None,
            binding_id: None,
            form_id: None,
            record_id: None,
        });
    }
    for (name, field) in &schema.fields {
        if !valid_field_name(name) {
            diagnostics.push(DesignerDiagnostic {
                code: "CONTENT_FIELD_NAME_INVALID".to_owned(),
                severity: DiagnosticSeverity::Error,
                message: format!("field name '{name}' must be 1..=128 safe bytes"),
                node_id: None,
                interaction_id: None,
                collection_id: None,
                binding_id: None,
                form_id: None,
                record_id: None,
            });
        }
        // Kind is closed; no extra validation needed beyond schema version.
        let _ = field;
    }
    diagnostics
}

/// Validate that a record conforms to its collection schema.
#[must_use]
pub fn validate_record(
    collection: &ContentCollection,
    record: &ContentRecord,
) -> Vec<DesignerDiagnostic> {
    let mut diagnostics = Vec::new();
    if record.schema_version != STUDIO_DESIGN_SCHEMA_VERSION {
        diagnostics.push(record_diagnostic(
            &collection.id,
            &record.id,
            "CONTENT_RECORD_SCHEMA_INVALID",
            "a record has an unsupported schema version",
        ));
    }
    for (field_name, field_schema) in &collection.schema.fields {
        let value = record.values.get(field_name);
        match (field_schema.required, value) {
            (true, None) => diagnostics.push(record_diagnostic(
                &collection.id,
                &record.id,
                "CONTENT_RECORD_REQUIRED_MISSING",
                format!("required field '{field_name}' is missing"),
            )),
            (false, None) => {}
            (_, Some(value)) => {
                if !property_value_matches_kind(value, field_schema.kind) {
                    diagnostics.push(record_diagnostic(
                        &collection.id,
                        &record.id,
                        "CONTENT_RECORD_TYPE_MISMATCH",
                        format!(
                            "field '{field_name}' expected {:?} but found {}",
                            field_schema.kind,
                            value_type_name(value)
                        ),
                    ));
                }
            }
        }
    }
    for field_name in record.values.keys() {
        if !collection.schema.fields.contains_key(field_name) {
            diagnostics.push(record_diagnostic(
                &collection.id,
                &record.id,
                "CONTENT_RECORD_UNKNOWN_FIELD",
                format!("field '{field_name}' is not declared in the collection schema"),
            ));
        }
    }
    diagnostics
}

fn record_diagnostic(
    collection_id: &CollectionId,
    record_id: &RecordId,
    code: &str,
    message: impl Into<String>,
) -> DesignerDiagnostic {
    DesignerDiagnostic {
        code: code.to_owned(),
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        node_id: None,
        interaction_id: None,
        collection_id: Some(collection_id.clone()),
        binding_id: None,
        form_id: None,
        record_id: Some(record_id.clone()),
    }
}

fn value_type_name(value: &PropertyValue) -> &'static str {
    match value {
        PropertyValue::String(_) => "String",
        PropertyValue::Boolean(_) => "Boolean",
        PropertyValue::Integer(_) => "Integer",
        PropertyValue::Decimal(_) => "Decimal",
        PropertyValue::Length(_) => "Length",
        PropertyValue::Color(_) => "Color",
        PropertyValue::Token(_) => "Token",
        PropertyValue::Binding(_) => "Binding",
        PropertyValue::Node(_) => "Node",
        PropertyValue::Asset(_) => "Asset",
        PropertyValue::List(_) => "List",
    }
}

/// Validate a fixture declaration.
#[must_use]
pub fn validate_fixture(fixture: &ContentFixture) -> Vec<DesignerDiagnostic> {
    let mut diagnostics = Vec::new();
    if fixture.schema_version != STUDIO_DESIGN_SCHEMA_VERSION {
        diagnostics.push(DesignerDiagnostic {
            code: "CONTENT_FIXTURE_SCHEMA_INVALID".to_owned(),
            severity: DiagnosticSeverity::Error,
            message: "a fixture has an unsupported schema version".to_owned(),
            node_id: None,
            interaction_id: None,
            collection_id: None,
            binding_id: None,
            form_id: None,
            record_id: None,
        });
    }
    if fixture.kind == FixtureKind::Edge && fixture.edge_records.is_empty() {
        diagnostics.push(DesignerDiagnostic {
            code: "CONTENT_FIXTURE_EDGE_EMPTY".to_owned(),
            severity: DiagnosticSeverity::Error,
            message: "an edge fixture must provide at least one edge record".to_owned(),
            node_id: None,
            interaction_id: None,
            collection_id: None,
            binding_id: None,
            form_id: None,
            record_id: None,
        });
    }
    if fixture.kind != FixtureKind::Error && fixture.error_message.is_some() {
        diagnostics.push(DesignerDiagnostic {
            code: "CONTENT_FIXTURE_ERROR_MESSAGE_MISPLACED".to_owned(),
            severity: DiagnosticSeverity::Error,
            message: "only an error fixture may carry an error message".to_owned(),
            node_id: None,
            interaction_id: None,
            collection_id: None,
            binding_id: None,
            form_id: None,
            record_id: None,
        });
    }
    if fixture.kind == FixtureKind::Error {
        let msg = fixture.error_message.as_deref().unwrap_or("").trim();
        if msg.is_empty() {
            diagnostics.push(DesignerDiagnostic {
                code: "CONTENT_FIXTURE_ERROR_EMPTY".to_owned(),
                severity: DiagnosticSeverity::Error,
                message: "an error fixture must provide a non-empty error message".to_owned(),
                node_id: None,
                interaction_id: None,
                collection_id: None,
                binding_id: None,
                form_id: None,
                record_id: None,
            });
        }
    }
    diagnostics
}

/// Validate a binding declaration (schema version + fallback type + repeated flag).
#[must_use]
pub fn validate_binding_shape(binding: &ContentBinding) -> Vec<DesignerDiagnostic> {
    let mut diagnostics = Vec::new();
    if binding.schema_version != STUDIO_DESIGN_SCHEMA_VERSION {
        diagnostics.push(binding_diagnostic(
            binding,
            "CONTENT_BINDING_SCHEMA_INVALID",
            "a binding has an unsupported schema version",
            DiagnosticSeverity::Error,
        ));
    }
    if !valid_field_name(&binding.property) {
        diagnostics.push(binding_diagnostic(
            binding,
            "CONTENT_BINDING_PROPERTY_INVALID",
            "a binding property name must be 1..=128 safe bytes",
            DiagnosticSeverity::Error,
        ));
    }
    if !valid_field_name(&binding.source.field) {
        diagnostics.push(binding_diagnostic(
            binding,
            "CONTENT_BINDING_FIELD_INVALID",
            "a binding source field must be 1..=128 safe bytes",
            DiagnosticSeverity::Error,
        ));
    }
    if let Some(fallback) = &binding.fallback
        && !fallback_matches_kind(fallback, binding.expected_kind)
    {
        diagnostics.push(binding_diagnostic(
            binding,
            "CONTENT_BINDING_FALLBACK_MISMATCH",
            format!(
                "fallback type {} does not match expected {:?}",
                value_type_name(fallback),
                binding.expected_kind
            ),
            DiagnosticSeverity::Error,
        ));
    }
    diagnostics
}

fn binding_diagnostic(
    binding: &ContentBinding,
    code: &str,
    message: impl Into<String>,
    severity: DiagnosticSeverity,
) -> DesignerDiagnostic {
    DesignerDiagnostic {
        code: code.to_owned(),
        severity,
        message: message.into(),
        node_id: Some(binding.node_id.clone()),
        interaction_id: None,
        collection_id: Some(binding.source.collection_id.clone()),
        binding_id: Some(binding.id.clone()),
        form_id: None,
        record_id: None,
    }
}

/// Validate that all bindings resolve against the current design.
///
/// - Unknown collection → error.
/// - Unknown field → error.
/// - Field kind != expected kind → error unless a valid fallback is declared.
/// - Missing node/property → warning (node/property may not yet exist).
#[must_use]
pub fn binding_diagnostics(design: &StudioDesign) -> Vec<DesignerDiagnostic> {
    let mut diagnostics = Vec::new();
    for binding in design.bindings.values() {
        diagnostics.extend(validate_binding_shape(binding));
        if !design.nodes.contains_key(&binding.node_id) {
            diagnostics.push(binding_diagnostic(
                binding,
                "CONTENT_BINDING_NODE_MISSING",
                format!("binding target node '{}' does not exist", binding.node_id),
                DiagnosticSeverity::Warning,
            ));
        }
        let Some(collection) = design.collections.get(&binding.source.collection_id) else {
            diagnostics.push(binding_diagnostic(
                binding,
                "CONTENT_BINDING_COLLECTION_MISSING",
                format!(
                    "binding source collection '{}' does not exist",
                    binding.source.collection_id
                ),
                DiagnosticSeverity::Error,
            ));
            continue;
        };
        let Some(field_schema) = collection.schema.fields.get(&binding.source.field) else {
            diagnostics.push(binding_diagnostic(
                binding,
                "CONTENT_BINDING_FIELD_MISSING",
                format!(
                    "field '{}' is not declared in collection '{}'",
                    binding.source.field, collection.id
                ),
                DiagnosticSeverity::Error,
            ));
            continue;
        };
        if field_schema.kind != binding.expected_kind {
            if binding.fallback.is_some() {
                diagnostics.push(binding_diagnostic(
                    binding,
                    "CONTENT_BINDING_TYPE_MISMATCH_WITH_FALLBACK",
                    format!(
                        "field '{}' in collection '{}' is {:?} but binding expects {:?}; fallback will be used",
                        binding.source.field, collection.id, field_schema.kind, binding.expected_kind
                    ),
                    DiagnosticSeverity::Warning,
                ));
            } else {
                diagnostics.push(binding_diagnostic(
                    binding,
                    "CONTENT_BINDING_TYPE_MISMATCH",
                    format!(
                        "field '{}' in collection '{}' is {:?} but binding expects {:?} and no fallback is declared",
                        binding.source.field, collection.id, field_schema.kind, binding.expected_kind
                    ),
                    DiagnosticSeverity::Error,
                ));
            }
        }
    }
    diagnostics
}

/// Validate form shape (field names, length bounds, pattern, numeric bounds).
#[must_use]
pub fn validate_form_shape(form: &FormDefinition) -> Vec<DesignerDiagnostic> {
    let mut diagnostics = Vec::new();
    if form.schema_version != STUDIO_DESIGN_SCHEMA_VERSION {
        diagnostics.push(form_diagnostic(
            form,
            "FORM_SCHEMA_INVALID",
            "a form has an unsupported schema version",
        ));
    }
    if form.name.trim().is_empty()
        || form.name.len() > 256
        || form.name.chars().any(char::is_control)
    {
        diagnostics.push(form_diagnostic(
            form,
            "FORM_NAME_INVALID",
            "a form name must be 1..=256 safe bytes",
        ));
    }
    if form.fields.is_empty() {
        diagnostics.push(form_diagnostic(
            form,
            "FORM_FIELDS_EMPTY",
            "a form must declare at least one field",
        ));
    }
    for (name, field) in &form.fields {
        if !valid_field_name(name) {
            diagnostics.push(form_diagnostic(
                form,
                "FORM_FIELD_NAME_INVALID",
                format!("field name '{name}' must be 1..=128 safe bytes"),
            ));
        }
        if let (Some(min), Some(max)) = (field.minimum_length, field.maximum_length)
            && min > max
        {
            diagnostics.push(form_diagnostic(
                form,
                "FORM_FIELD_LENGTH_BOUNDS_INVALID",
                format!("field '{name}' has minimum_length > maximum_length"),
            ));
        }
        if let Some(pattern) = &field.pattern
            && Regex::new(pattern).is_err()
        {
            diagnostics.push(form_diagnostic(
                form,
                "FORM_FIELD_PATTERN_INVALID",
                format!("field '{name}' has an invalid regex pattern"),
            ));
        }
        if let (Some(min), Some(max)) = (&field.minimum_value, &field.maximum_value)
            && min > max
        {
            diagnostics.push(form_diagnostic(
                form,
                "FORM_FIELD_VALUE_BOUNDS_INVALID",
                format!("field '{name}' has minimum_value > maximum_value"),
            ));
        }
    }
    diagnostics
}

fn form_diagnostic(
    form: &FormDefinition,
    code: &str,
    message: impl Into<String>,
) -> DesignerDiagnostic {
    DesignerDiagnostic {
        code: code.to_owned(),
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        node_id: None,
        interaction_id: None,
        collection_id: form.target_collection_id.clone(),
        binding_id: None,
        form_id: Some(form.id.clone()),
        record_id: None,
    }
}

/// Deterministic preview of a list bound to a collection, parameterized by fixture.
///
/// The renderer contract is narrow and pure:
/// - The same collection + fixture always yields the same preview (no I/O, no clock).
/// - Callers iterate `preview.records` to stamp repeated children; non-repeated
///   bindings read the first record when present or their fallback otherwise.
/// - Fixture switching never changes the collection's durable records; it only
///   affects the preview view (empty/loading/error/populated/edge).
#[must_use]
pub fn preview_collection(
    collection: &ContentCollection,
    override_kind: Option<FixtureKind>,
) -> crate::model::CollectionPreview {
    let kind = override_kind.unwrap_or(collection.fixture.kind);
    match kind {
        FixtureKind::Empty => crate::model::CollectionPreview {
            collection_id: collection.id.clone(),
            fixture: FixtureKind::Empty,
            records: Vec::new(),
            is_loading: false,
            is_error: false,
            error_message: None,
        },
        FixtureKind::Loading => crate::model::CollectionPreview {
            collection_id: collection.id.clone(),
            fixture: FixtureKind::Loading,
            records: Vec::new(),
            is_loading: true,
            is_error: false,
            error_message: None,
        },
        FixtureKind::Error => crate::model::CollectionPreview {
            collection_id: collection.id.clone(),
            fixture: FixtureKind::Error,
            records: Vec::new(),
            is_loading: false,
            is_error: true,
            error_message: collection
                .fixture
                .error_message
                .clone()
                .or_else(|| Some("collection failed to load".to_owned())),
        },
        FixtureKind::Populated => {
            let mut records = collection.records.values().cloned().collect::<Vec<_>>();
            records.sort_by(|a, b| a.id.cmp(&b.id));
            crate::model::CollectionPreview {
                collection_id: collection.id.clone(),
                fixture: FixtureKind::Populated,
                records,
                is_loading: false,
                is_error: false,
                error_message: None,
            }
        }
        FixtureKind::Edge => {
            let mut records = if collection.fixture.edge_records.is_empty() {
                // Deterministic edge case: surface 0/1/many boundaries with the real data.
                let mut all = collection.records.values().cloned().collect::<Vec<_>>();
                all.sort_by(|a, b| a.id.cmp(&b.id));
                all
            } else {
                let mut edge = collection.fixture.edge_records.clone();
                edge.sort_by(|a, b| a.id.cmp(&b.id));
                edge
            };
            records.sort_by(|a, b| a.id.cmp(&b.id));
            crate::model::CollectionPreview {
                collection_id: collection.id.clone(),
                fixture: FixtureKind::Edge,
                records,
                is_loading: false,
                is_error: false,
                error_message: None,
            }
        }
    }
}

/// Resolve one binding for a given record (or `None` for fixture states with no record).
///
/// Returns the property value to render, honoring fallback when the source is
/// missing or has the wrong type. This keeps repeated lists rendering
/// identically across all fixture states: the caller always gets a deterministic
/// `Some(value)` when a valid fallback exists, or `None` when it does not.
#[must_use]
pub fn resolve_binding_value(
    binding: &ContentBinding,
    _collection: &ContentCollection,
    record: Option<&ContentRecord>,
) -> Option<PropertyValue> {
    if let Some(record) = record {
        if let Some(value) = record.values.get(&binding.source.field) {
            if property_value_matches_kind(value, binding.expected_kind) {
                return Some(value.clone());
            }
            // Type mismatch at record level → fallback if available.
            return binding.fallback.clone();
        }
        return binding.fallback.clone();
    }
    // No record (empty/loading/error fixtures): use fallback.
    binding.fallback.clone()
}

/// Whether a binding type mismatch is a *build error*.
///
/// Contract: mismatch is a build error unless a valid typed fallback is declared.
#[must_use]
pub fn is_binding_build_error(binding: &ContentBinding, design: &StudioDesign) -> bool {
    let Some(collection) = design.collections.get(&binding.source.collection_id) else {
        return true;
    };
    let Some(field) = collection.schema.fields.get(&binding.source.field) else {
        return true;
    };
    if field.kind == binding.expected_kind {
        return false;
    }
    // Mismatched kind: build error unless fallback matches expected kind.
    binding
        .fallback
        .as_ref()
        .is_none_or(|fallback| !fallback_matches_kind(fallback, binding.expected_kind))
}

/// Collect build-blocking binding errors for `cargo build`-style gating.
#[must_use]
pub fn build_blocking_binding_errors(design: &StudioDesign) -> Vec<DesignerDiagnostic> {
    design
        .bindings
        .values()
        .filter(|binding| is_binding_build_error(binding, design))
        .map(|binding| {
            let reason = design
                .collections
                .get(&binding.source.collection_id)
                .and_then(|c| c.schema.fields.get(&binding.source.field))
                .map_or_else(
                    || "collection or field is missing".to_owned(),
                    |f| format!("field is {:?}", f.kind),
                );
            DesignerDiagnostic {
                code: "CONTENT_BUILD_BLOCKED".to_owned(),
                severity: DiagnosticSeverity::Error,
                message: format!(
                    "binding '{}' on node '{}' cannot build: {} but binding expects {:?}{}",
                    binding.id,
                    binding.node_id,
                    reason,
                    binding.expected_kind,
                    if binding.fallback.is_some() {
                        " (fallback exists but has wrong type)"
                    } else {
                        " and no fallback is declared"
                    }
                ),
                node_id: Some(binding.node_id.clone()),
                interaction_id: None,
                collection_id: Some(binding.source.collection_id.clone()),
                binding_id: Some(binding.id.clone()),
                form_id: None,
                record_id: None,
            }
        })
        .collect()
}

/// Declarative form validation that runs in prototype mode (no imperative code).
///
/// Prototype mode means: validation is a pure function of the form schema and
/// the submitted values. It is deterministic and does not perform I/O.
#[must_use]
pub fn validate_form_values(
    form: &FormDefinition,
    values: &BTreeMap<String, PropertyValue>,
) -> FormValidationResult {
    let mut field_errors = BTreeMap::new();
    for (name, field) in &form.fields {
        let value = values.get(name);
        if field.required && value.is_none() {
            field_errors.insert(name.clone(), "this field is required".to_owned());
            continue;
        }
        let Some(value) = value else {
            continue;
        };
        if !property_value_matches_kind(value, field.kind) {
            field_errors.insert(
                name.clone(),
                format!(
                    "expected {:?} but found {}",
                    field.kind,
                    value_type_name(value)
                ),
            );
            continue;
        }
        // Length constraints for strings.
        if let PropertyValue::String(text) = value {
            if let Some(min) = field.minimum_length
                && text.len() < min
            {
                field_errors.insert(name.clone(), format!("must be at least {min} characters"));
                continue;
            }
            if let Some(max) = field.maximum_length
                && text.len() > max
            {
                field_errors.insert(name.clone(), format!("must be at most {max} characters"));
                continue;
            }
            if let Some(pattern) = &field.pattern
                && let Ok(re) = Regex::new(pattern)
                && !re.is_match(text)
            {
                field_errors.insert(name.clone(), "does not match required pattern".to_owned());
                continue;
            }
        }
        // Numeric bounds are compared lexicographically for decimals stored as strings;
        // for integers we parse. This keeps validation deterministic without floating errors.
        if let PropertyValue::Integer(int_val) = value {
            if let Some(min_str) = &field.minimum_value
                && let Ok(min_int) = min_str.parse::<i64>()
                && *int_val < min_int
            {
                field_errors.insert(name.clone(), format!("must be at least {min_int}"));
                continue;
            }
            if let Some(max_str) = &field.maximum_value
                && let Ok(max_int) = max_str.parse::<i64>()
                && *int_val > max_int
            {
                field_errors.insert(name.clone(), format!("must be at most {max_int}"));
            }
        }
    }
    // Unknown fields are not an error; they are ignored (forward compatibility).
    let valid = field_errors.is_empty();
    FormValidationResult {
        valid,
        field_errors,
    }
}

/// Surface affected bindings when a collection schema changes.
///
/// Call this after a schema edit to produce diagnostics that point at every
/// binding whose field was removed or whose kind changed. The session's
/// `reference_diagnostics` equivalent merges these with other diagnostics.
#[must_use]
pub fn schema_change_binding_diagnostics(
    design: &StudioDesign,
    collection_id: &CollectionId,
    old_schema: &ContentCollectionSchema,
    new_schema: &ContentCollectionSchema,
) -> Vec<DesignerDiagnostic> {
    let mut diagnostics = Vec::new();
    let changed_fields = old_schema
        .fields
        .iter()
        .filter_map(|(name, old_field)| {
            new_schema
                .fields
                .get(name)
                .map_or(Some((name.clone(), None)), |new_field| {
                    if new_field.kind == old_field.kind {
                        None
                    } else {
                        Some((name.clone(), Some(new_field.kind)))
                    }
                })
        })
        .collect::<BTreeMap<_, _>>();
    let removed_fields = old_schema
        .fields
        .keys()
        .filter(|name| !new_schema.fields.contains_key(*name))
        .cloned()
        .collect::<Vec<_>>();

    for binding in design
        .bindings
        .values()
        .filter(|b| &b.source.collection_id == collection_id)
    {
        if removed_fields.contains(&binding.source.field) {
            diagnostics.push(DesignerDiagnostic {
                code: "CONTENT_SCHEMA_FIELD_REMOVED".to_owned(),
                severity: DiagnosticSeverity::Error,
                message: format!(
                    "field '{}' was removed from collection '{}'; binding '{}' is now dangling",
                    binding.source.field, collection_id, binding.id
                ),
                node_id: Some(binding.node_id.clone()),
                interaction_id: None,
                collection_id: Some(collection_id.clone()),
                binding_id: Some(binding.id.clone()),
                form_id: None,
                record_id: None,
            });
        } else if let Some(new_kind) = changed_fields.get(&binding.source.field).copied().flatten()
        {
            diagnostics.push(DesignerDiagnostic {
                code: "CONTENT_SCHEMA_FIELD_KIND_CHANGED".to_owned(),
                severity: DiagnosticSeverity::Error,
                message: format!(
                    "field '{}' in collection '{}' changed kind; binding '{}' expects {:?} but schema is now {:?}",
                    binding.source.field, collection_id, binding.id, binding.expected_kind, new_kind
                ),
                node_id: Some(binding.node_id.clone()),
                interaction_id: None,
                collection_id: Some(collection_id.clone()),
                binding_id: Some(binding.id.clone()),
                form_id: None,
                record_id: None,
            });
        }
    }
    diagnostics
}
