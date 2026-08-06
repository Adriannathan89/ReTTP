use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    Capture,
    Value,
};

/// A JSON type expected for an asserted field.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ExpectedType {
    String,
    Boolean,
    Integer,
    Number,
    Object,
    Array,
    Null,
}

impl ExpectedType {
    /// Returns the canonical lower-case JSON type name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Number => "number",
            Self::Object => "object",
            Self::Array => "array",
            Self::Null => "null",
        }
    }
}

/// Expectations for one named field in a JSON object.
///
/// A field can require only a type, require an exact value, recursively assert
/// a nested object, and/or capture its actual value for later interpolation.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct FieldAssertion {
    pub field_name: String,
    pub expected_type: ExpectedType,
    pub expected_value: Option<Value>,
    pub nested: Option<ObjectAssertion>,
    pub capture: Option<Capture>,
}

impl FieldAssertion {
    /// Creates an assertion that checks only the field type.
    /// Creates an assertion that checks both field type and exact value.
    #[must_use]
    pub fn type_only(
        field_name: impl Into<String>,
        expected_type: ExpectedType,
    ) -> Self {
        Self {
            field_name: field_name.into(),
            expected_type,
            expected_value: None,
            nested: None,
            capture: None,
        }
    }

    /// Configures the field's actual value to be captured after it matches.
    #[must_use]
    pub fn type_and_value(
        field_name: impl Into<String>,
        expected_type: ExpectedType,
        expected_value: Value,
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
    pub fn with_capture(mut self, capture: Capture) -> Self {
        self.capture = Some(capture);
        self
    }

    #[must_use]
    pub fn with_nested(mut self, nested: ObjectAssertion) -> Self {
        self.nested = Some(nested);
        self
    }
}

/// A collection of assertions for a JSON object response body or nested field.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ObjectAssertion {
    pub mode: ObjectMatchMode,
    pub fields: IndexMap<String, FieldAssertion>,
}

impl ObjectAssertion {
    /// Creates an assertion that permits response fields not listed in `fields`.
    /// Creates an assertion that rejects response fields not listed in `fields`.
    #[must_use]
    pub fn partial() -> Self {
        Self {
            mode: ObjectMatchMode::Partial,
            fields: IndexMap::new()
        }
    }

    #[must_use]
    pub fn exact() -> Self {
        Self {
            mode: ObjectMatchMode::Exact,
            fields: IndexMap::new(),
        }
    }

    /// Adds or replaces the assertion for its field name.
    pub fn insert(&mut self, assertion: FieldAssertion) {
        self.fields
            .insert(assertion.field_name.clone(), assertion);
    }
}

/// Policy for fields not explicitly declared in an [`ObjectAssertion`].
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ObjectMatchMode {
    Exact,
    Partial,
}
