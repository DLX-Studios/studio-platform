//! Bounded closed JSON Schema subset used by signed route-group declarations.
//!
//! Declarations are authored at build time and signed into the package, so this validator keeps
//! the accepted surface deliberately small and auditable instead of linking a full JSON Schema
//! engine. Unsupported keywords fail declaration admission rather than being ignored, which
//! guarantees every schema that reaches runtime is fully understood by this module.
//!
//! Supported keywords: `type`, `properties`, `required`, `additionalProperties` (boolean only,
//! defaulting to closed), `items`, `enum`, `minLength`, `maxLength`, `minimum`, `maximum`,
//! `minItems`, and `maxItems`. Unknown properties inside declared objects are rejected, matching
//! the deny-unknown-fields convention used across Studio wire types.

use std::fmt;

use serde_json::Value;

/// Failure from constructing or applying a bounded declaration schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaError {
    path: String,
    reason: String,
}

impl SchemaError {
    /// Path of the offending location (`$`-rooted for values).
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Safe human-readable reason without payload content.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Display for SchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.reason)
    }
}

impl std::error::Error for SchemaError {}

const SCHEMA_KEYWORDS: &[&str] = &[
    "type",
    "properties",
    "required",
    "additionalProperties",
    "items",
    "enum",
    "minLength",
    "maxLength",
    "minimum",
    "maximum",
    "minItems",
    "maxItems",
];

const TYPE_NAMES: &[&str] = &[
    "object", "array", "string", "number", "integer", "boolean", "null",
];

/// A validated bounded schema ready to admit or reject guest-visible values.
#[derive(Clone, Debug)]
pub struct JsonSchema {
    root: Value,
}

impl JsonSchema {
    /// Construct and fully validate one bounded schema document.
    ///
    /// # Errors
    ///
    /// Returns [`SchemaError`] for non-object roots, unknown keywords, malformed constraints,
    /// or `required` entries without matching declared properties.
    pub fn new(root: Value) -> Result<Self, SchemaError> {
        check_schema_node(&root, "$")?;
        Ok(Self { root })
    }

    /// Validate one value against this schema.
    ///
    /// # Errors
    ///
    /// Returns [`SchemaError`] locating the first violation.
    pub fn validate(&self, value: &Value) -> Result<(), SchemaError> {
        validate_node(&self.root, value, "$")
    }
}

fn err(path: &str, reason: impl Into<String>) -> SchemaError {
    SchemaError {
        path: path.to_owned(),
        reason: reason.into(),
    }
}

fn check_schema_node(node: &Value, path: &str) -> Result<(), SchemaError> {
    let Some(object) = node.as_object() else {
        return Err(err(path, "schema must be a JSON object"));
    };
    for key in object.keys() {
        if !SCHEMA_KEYWORDS.contains(&key.as_str()) {
            return Err(err(path, format!("unsupported schema keyword `{key}`")));
        }
    }
    if let Some(kind) = object.get("type") {
        let Some(name) = kind.as_str() else {
            return Err(err(path, "`type` must be a string"));
        };
        if !TYPE_NAMES.contains(&name) {
            return Err(err(path, format!("unsupported type `{name}`")));
        }
    }
    if let Some(allowed) = object.get("enum") {
        let Some(values) = allowed.as_array() else {
            return Err(err(path, "`enum` must be an array"));
        };
        if values.is_empty()
            || !values
                .iter()
                .all(|value| !value.is_object() && !value.is_array() && !value.is_null())
        {
            return Err(err(path, "`enum` must contain scalar values"));
        }
    }
    for key in ["minLength", "maxLength", "minItems", "maxItems"] {
        if let Some(bound) = object.get(key)
            && bound.as_u64().is_none()
        {
            return Err(err(path, format!("`{key}` must be a non-negative integer")));
        }
    }
    for key in ["minimum", "maximum"] {
        if let Some(bound) = object.get(key)
            && bound.as_f64().is_none_or(f64::is_nan)
        {
            return Err(err(path, format!("`{key}` must be a finite number")));
        }
    }
    if let Some(required) = object.get("required") {
        let Some(names) = required.as_array() else {
            return Err(err(path, "`required` must be an array"));
        };
        let properties = object.get("properties").and_then(Value::as_object);
        for name in names {
            let Some(property) = name.as_str() else {
                return Err(err(path, "`required` entries must be strings"));
            };
            let declared = properties.is_some_and(|props| props.contains_key(property));
            if !declared {
                return Err(err(
                    path,
                    format!("`required` entry `{property}` has no declared property"),
                ));
            }
        }
    }
    if let Some(properties) = object.get("properties") {
        let Some(map) = properties.as_object() else {
            return Err(err(path, "`properties` must be an object"));
        };
        for (name, property) in map {
            check_schema_node(property, &format!("{path}.{name}"))?;
        }
    }
    if let Some(items) = object.get("items") {
        check_schema_node(items, &format!("{path}[]"))?;
    }
    if let Some(open) = object.get("additionalProperties")
        && !open.is_boolean()
    {
        return Err(err(path, "`additionalProperties` must be a boolean"));
    }
    Ok(())
}

fn validate_node(schema: &Value, value: &Value, path: &str) -> Result<(), SchemaError> {
    let Some(object) = schema.as_object() else {
        return Err(err(path, "internal schema corruption"));
    };
    if let Some(kind) = object.get("type").and_then(Value::as_str)
        && !type_matches(kind, value)
    {
        return Err(err(path, format!("expected `{kind}`")));
    }
    if let Some(allowed) = object.get("enum").and_then(Value::as_array)
        && !allowed.iter().any(|candidate| candidate == value)
    {
        return Err(err(path, "value not in declared enum"));
    }
    match value {
        Value::Object(fields) => validate_object(object, fields, path)?,
        Value::Array(items) => validate_array(object, items, path)?,
        Value::String(text) => validate_string(object, text.chars().count(), path)?,
        _ => {}
    }
    Ok(())
}

fn validate_object(
    schema: &serde_json::Map<String, Value>,
    fields: &serde_json::Map<String, Value>,
    path: &str,
) -> Result<(), SchemaError> {
    let open = schema
        .get("additionalProperties")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let properties = schema.get("properties").and_then(Value::as_object);
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for name in required.iter().filter_map(Value::as_str) {
            if !fields.contains_key(name) {
                return Err(err(&format!("{path}.{name}"), "missing required property"));
            }
        }
    }
    for (name, field) in fields {
        match properties.and_then(|props| props.get(name)) {
            Some(property) => validate_node(property, field, &format!("{path}.{name}"))?,
            None if open => {}
            None => return Err(err(&format!("{path}.{name}"), "undeclared property")),
        }
    }
    Ok(())
}

fn validate_array(
    schema: &serde_json::Map<String, Value>,
    items: &[Value],
    path: &str,
) -> Result<(), SchemaError> {
    apply_count_bound(schema, "minItems", items.len(), path, |bound, actual| {
        actual < bound
    })?;
    apply_count_bound(schema, "maxItems", items.len(), path, |bound, actual| {
        actual > bound
    })?;
    if let Some(item_schema) = schema.get("items") {
        for (index, item) in items.iter().enumerate() {
            validate_node(item_schema, item, &format!("{path}[{index}]"))?;
        }
    }
    Ok(())
}

fn validate_string(
    schema: &serde_json::Map<String, Value>,
    length: usize,
    path: &str,
) -> Result<(), SchemaError> {
    apply_count_bound(schema, "minLength", length, path, |bound, actual| {
        actual < bound
    })?;
    apply_count_bound(schema, "maxLength", length, path, |bound, actual| {
        actual > bound
    })?;
    Ok(())
}

fn apply_count_bound(
    schema: &serde_json::Map<String, Value>,
    keyword: &str,
    actual: usize,
    path: &str,
    violates: fn(u64, usize) -> bool,
) -> Result<(), SchemaError> {
    if let Some(bound) = schema.get(keyword).and_then(Value::as_u64)
        && violates(bound, actual)
    {
        return Err(err(
            path,
            format!("violates `{keyword}` bound of {bound}"),
        ));
    }
    Ok(())
}
