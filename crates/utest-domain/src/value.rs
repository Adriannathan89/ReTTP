use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct InterpolatedString {
    raw: String,
}

impl InterpolatedString {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self { raw: value.into() }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }

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