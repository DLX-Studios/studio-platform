//! The closed Studio type system: one canonical number, serializable values,
//! and the deterministic language-to-host numeric mapping.
//!
//! The language has a single numeric type (`number`, JavaScript-like). Editing
//! `$state(1)` to `$state(1.5)` changes a value, never a type, so numeric
//! edits never break hot-swap compatibility. When a number crosses into the
//! host contract, the validator maps integral values within i64 range to
//! Integer and all other numbers to Decimal; exact-decimal rendering stays
//! with explicit formatting helpers, never float arithmetic.
use std::collections::BTreeMap;

/// Closed Studio types governing validation, swap compatibility, and lowering.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum StudioType {
    /// UTF-8 text.
    String,
    /// True or false.
    Boolean,
    /// The single canonical numeric type (lowered as f64 in both backends).
    Number,
    /// Homogeneous array.
    Array(Box<StudioType>),
    /// Record with named fields.
    Record(BTreeMap<String, StudioType>),
}

/// One numeric literal preserving its source form.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NumberLiteral {
    /// Source text as written.
    pub text: String,
    /// Numeric value.
    pub value: f64,
}

/// Closed literal values admitted by the portable subset.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum StudioValue {
    /// UTF-8 text.
    String(String),
    /// True or false.
    Boolean(bool),
    /// Canonical number.
    Number(NumberLiteral),
    /// Homogeneous array literal.
    Array(Vec<StudioValue>),
    /// Record literal with named fields.
    Record(std::collections::BTreeMap<String, StudioValue>),
}

impl StudioValue {
    /// The closed type of this value. Empty arrays admit no element type and
    /// empty records admit no fields, so both require a representative default.
    #[must_use]
    pub fn ty(&self) -> StudioType {
        match self {
            Self::String(_) => StudioType::String,
            Self::Boolean(_) => StudioType::Boolean,
            Self::Number(_) => StudioType::Number,
            Self::Array(elements) => StudioType::Array(Box::new(
                elements.first().map_or(StudioType::String, StudioValue::ty),
            )),
            Self::Record(fields) => StudioType::Record(
                fields
                    .iter()
                    .map(|(key, value)| (key.clone(), value.ty()))
                    .collect(),
            ),
        }
    }
}

/// Host-side numeric contract for one number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostNumericKind {
    /// Integral value within i64 range: maps to the host Integer contract.
    Integer,
    /// Everything else: maps to the host Decimal contract (exact decimal).
    Decimal,
}

/// Bounds of the host Integer contract as exact floats.
///
/// `I64_MIN_F64` is exactly `-2^63`. The upper bound is exclusive `2^63`
/// because `i64::MAX as f64` rounds up to `2^63`, which is not a valid `i64`.
const I64_MIN_F64: f64 = -9_223_372_036_854_775_808.0;
const I64_MAX_EXCLUSIVE_F64: f64 = 9_223_372_036_854_775_808.0;

/// Deterministically classify one number for the host contract.
#[must_use]
pub const fn host_numeric_kind(value: f64) -> HostNumericKind {
    if value.is_finite()
        && value.fract() == 0.0
        && value >= I64_MIN_F64
        && value < I64_MAX_EXCLUSIVE_F64
    {
        HostNumericKind::Integer
    } else {
        HostNumericKind::Decimal
    }
}

/// Swap compatibility: structural type equality. There is one numeric type, so
/// numeric edits never break compatibility; genuinely different types reset.
#[must_use]
pub fn compatible(previous: &StudioType, next: &StudioType) -> bool {
    match (previous, next) {
        (StudioType::String, StudioType::String)
        | (StudioType::Boolean, StudioType::Boolean)
        | (StudioType::Number, StudioType::Number) => true,
        (StudioType::Array(previous), StudioType::Array(next)) => compatible(previous, next),
        (StudioType::Record(previous), StudioType::Record(next)) => {
            previous.len() == next.len()
                && previous.iter().all(|(key, previous)| {
                    next.get(key).is_some_and(|next| compatible(previous, next))
                })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_one_canonical_type() {
        assert_eq!(
            StudioValue::Number(NumberLiteral {
                text: "1".to_owned(),
                value: 1.0
            })
            .ty(),
            StudioType::Number
        );
        assert_eq!(
            StudioValue::Number(NumberLiteral {
                text: "1.5".to_owned(),
                value: 1.5
            })
            .ty(),
            StudioType::Number
        );
        assert!(compatible(&StudioType::Number, &StudioType::Number));
        assert!(compatible(
            &StudioType::Array(Box::new(StudioType::Number)),
            &StudioType::Array(Box::new(StudioType::Number)),
        ));
        assert!(!compatible(
            &StudioType::Array(Box::new(StudioType::Number)),
            &StudioType::Array(Box::new(StudioType::String)),
        ));
    }

    #[test]
    fn host_numeric_mapping_is_deterministic() {
        assert_eq!(host_numeric_kind(1.0), HostNumericKind::Integer);
        assert_eq!(host_numeric_kind(-40.0), HostNumericKind::Integer);
        assert_eq!(
            host_numeric_kind(9_007_199_254_740_991.0),
            HostNumericKind::Integer
        );
        assert_eq!(
            host_numeric_kind(9_007_199_254_740_992.0),
            HostNumericKind::Integer
        );
        assert_eq!(host_numeric_kind(123_456_789.5), HostNumericKind::Decimal);
        assert_eq!(host_numeric_kind(1.5), HostNumericKind::Decimal);
        // The classic inexact sum is not integral, so it maps Decimal even
        // though its source text looks exact: the boundary is on the value.
        assert_eq!(host_numeric_kind(0.1 + 0.2 - 0.3), HostNumericKind::Decimal);
        assert_eq!(host_numeric_kind(0.0), HostNumericKind::Integer);
        assert_eq!(host_numeric_kind(f64::NAN), HostNumericKind::Decimal);
        assert_eq!(host_numeric_kind(f64::INFINITY), HostNumericKind::Decimal);
        assert_eq!(host_numeric_kind(1e30), HostNumericKind::Decimal);
    }

    #[test]
    fn genuine_type_changes_are_incompatible() {
        assert!(!compatible(&StudioType::Number, &StudioType::String));
        assert!(!compatible(&StudioType::String, &StudioType::Boolean));
        assert!(compatible(
            &StudioType::Record(
                [("name".to_owned(), StudioType::String)]
                    .into_iter()
                    .collect()
            ),
            &StudioType::Record(
                [("name".to_owned(), StudioType::String)]
                    .into_iter()
                    .collect()
            ),
        ));
        assert!(!compatible(
            &StudioType::Record(
                [("name".to_owned(), StudioType::String)]
                    .into_iter()
                    .collect()
            ),
            &StudioType::Record(
                [
                    ("name".to_owned(), StudioType::String),
                    ("sku".to_owned(), StudioType::String),
                ]
                .into_iter()
                .collect()
            ),
        ));
    }
}
