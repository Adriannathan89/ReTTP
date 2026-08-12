//! Expectation models whose strings and JSON values are fully resolved.

use indexmap::IndexMap;
use rettp_domain::{Capture, ExpectedType, ObjectMatchMode};
use serde_json::Value as JsonValue;

/// Expected properties of one HTTP response after variable interpolation.
///
/// Unlike [`rettp_domain::ResponseExpectation`], this type cannot carry an
/// [`rettp_domain::InterpolatedString`]. Week 8 runtime code converts the
/// domain expectation into this representation before evaluation.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResolvedResponseExpectation {
    /// Optional exact HTTP status requirement.
    pub status: Option<u16>,
    /// Case-insensitive header assertions in declaration order.
    pub headers: IndexMap<String, ResolvedHeaderAssertion>,
    /// Optional response-body assertion.
    pub body: Option<ResolvedBodyAssertion>,
}

/// A comparison strategy for one response header after interpolation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedHeaderAssertion {
    /// Require at least one value for the named header.
    Exists,
    /// Require exactly one response value equal to the expected text.
    Exact(String),
    /// Require at least one response value containing the expected text.
    Contains(String),
}

/// A response-body assertion after interpolation.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedBodyAssertion {
    /// Assert a JSON object using field declarations.
    Json(ResolvedObjectAssertion),
    /// Assert a strictly text-classified response.
    Text(ResolvedTextAssertion),
    /// Require a response body containing zero bytes.
    Empty,
}

/// A comparison strategy for a text response body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedTextAssertion {
    /// Require complete string equality.
    Exact(String),
    /// Require the response text to contain a substring.
    Contains(String),
}

/// Assertions for a JSON object response body or nested object field.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedObjectAssertion {
    /// Policy for actual fields not declared by this assertion.
    pub mode: ObjectMatchMode,
    /// Field assertions keyed by their declared field name.
    pub fields: IndexMap<String, ResolvedFieldAssertion>,
}

impl ResolvedObjectAssertion {
    /// Creates an object assertion that permits undeclared actual fields.
    #[must_use]
    pub fn partial() -> Self {
        Self {
            mode: ObjectMatchMode::Partial,
            fields: IndexMap::new(),
        }
    }

    /// Creates an object assertion that rejects undeclared actual fields.
    #[must_use]
    pub fn exact() -> Self {
        Self {
            mode: ObjectMatchMode::Exact,
            fields: IndexMap::new(),
        }
    }

    /// Adds or replaces a field assertion using its own field name as the key.
    pub fn insert(&mut self, assertion: ResolvedFieldAssertion) {
        self.fields.insert(assertion.field_name.clone(), assertion);
    }
}

/// Fully resolved requirements for one named JSON object field.
///
/// `expected_value` is compared only after the field matches `expected_type`.
/// Object values use recursive partial comparison, while arrays retain exact
/// length and order. `capture` is metadata for Week 8; the Week 7 assertion
/// engine never mutates a variable store.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedFieldAssertion {
    /// Object key to locate in the actual response.
    pub field_name: String,
    /// Required JSON type.
    pub expected_type: ExpectedType,
    /// Optional resolved JSON value comparison.
    pub expected_value: Option<JsonValue>,
    /// Optional recursive object-field assertions.
    pub nested: Option<ResolvedObjectAssertion>,
    /// Optional future capture declaration retained for the runtime.
    pub capture: Option<Capture>,
}

impl ResolvedFieldAssertion {
    /// Creates an assertion that checks only a field's JSON type.
    #[must_use]
    pub fn type_only(field_name: impl Into<String>, expected_type: ExpectedType) -> Self {
        Self {
            field_name: field_name.into(),
            expected_type,
            expected_value: None,
            nested: None,
            capture: None,
        }
    }

    /// Creates an assertion that checks a field's type and resolved value.
    #[must_use]
    pub fn type_and_value(
        field_name: impl Into<String>,
        expected_type: ExpectedType,
        expected_value: JsonValue,
    ) -> Self {
        Self {
            field_name: field_name.into(),
            expected_type,
            expected_value: Some(expected_value),
            nested: None,
            capture: None,
        }
    }

    /// Adds recursive assertions for an object-valued field.
    #[must_use]
    pub fn with_nested(mut self, nested: ResolvedObjectAssertion) -> Self {
        self.nested = Some(nested);
        self
    }

    /// Retains a capture declaration for the later capture runtime.
    #[must_use]
    pub fn with_capture(mut self, capture: Capture) -> Self {
        self.capture = Some(capture);
        self
    }
}
