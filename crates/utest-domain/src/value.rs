use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// A typed value used in request query/body data and expected JSON values.
///
/// This is the runner's backend-neutral value representation. It maps directly
/// to common JSON data types while retaining [`InterpolatedString`] for values
/// that must be resolved from captured variables at execution time.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum Value {
    String(InterpolatedString),
    Integer(i64),
    Number(f64),
    Boolean(bool),
    Null,
    Array(Vec<Value>),
    Object(IndexMap<String, Value>),
}

impl Value {
    /// Returns the canonical type name used by the assertion layer.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Value::String(_) => "string",
            Value::Integer(_) => "integer",
            Value::Number(_) => "number",
            Value::Boolean(_) => "boolean",
            Value::Null => "null",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }
}

/// Text that may contain `${variable}` placeholders.
///
/// Interpolation is intentionally not evaluated in this crate. An execution
/// adapter resolves it against its variable store immediately before sending a
/// request or evaluating an expectation.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct InterpolatedString {
    raw: String,
}

impl InterpolatedString {
    /// Creates text that can later be interpolated by an execution adapter.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self { raw: value.into() }
    }

    /// Returns the original, unresolved text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Returns whether the text contains the interpolation opening marker.
    #[must_use]
    pub fn contains_interpolation(&self) -> bool {
        self.raw.contains("${")
    }
}

impl From<String> for InterpolatedString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for InterpolatedString {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}
