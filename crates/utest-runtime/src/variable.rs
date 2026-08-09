//! Redacted runtime variable values, assignments, and scope storage.

use std::{borrow::Cow, ffi::OsString, fmt, str::FromStr};

use indexmap::IndexMap;
use serde_json::Value as JsonValue;
use utest_domain::VariableName;

use crate::VariableAssignmentError;

/// A runtime value supplied as text or captured as typed JSON.
///
/// The custom [`Debug`] implementation reveals only the value's type. Use the
/// accessors deliberately when trusted application code needs the contents.
#[derive(Clone, PartialEq)]
pub enum VariableValue {
    /// UTF-8 text loaded from an environment or CLI assignment.
    Text(String),
    /// An actual JSON response field retained with its original JSON type.
    Json(JsonValue),
}

impl VariableValue {
    /// Returns the stable JSON-compatible type name of the stored value.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Text(_) | Self::Json(JsonValue::String(_)) => "string",
            Self::Json(JsonValue::Null) => "null",
            Self::Json(JsonValue::Bool(_)) => "boolean",
            Self::Json(JsonValue::Number(number)) if number.is_i64() || number.is_u64() => {
                "integer"
            }
            Self::Json(JsonValue::Number(_)) => "number",
            Self::Json(JsonValue::Array(_)) => "array",
            Self::Json(JsonValue::Object(_)) => "object",
        }
    }

    /// Returns text supplied by an environment or CLI source.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            Self::Json(_) => None,
        }
    }

    /// Returns the captured JSON value without cloning it.
    #[must_use]
    pub const fn as_json(&self) -> Option<&JsonValue> {
        match self {
            Self::Json(value) => Some(value),
            Self::Text(_) => None,
        }
    }

    pub(crate) fn scalar_text(&self) -> Option<Cow<'_, str>> {
        match self {
            Self::Text(value) | Self::Json(JsonValue::String(value)) => Some(Cow::Borrowed(value)),
            Self::Json(JsonValue::Null) => Some(Cow::Borrowed("null")),
            Self::Json(JsonValue::Bool(value)) => Some(Cow::Owned(value.to_string())),
            Self::Json(JsonValue::Number(value)) => Some(Cow::Owned(value.to_string())),
            Self::Json(JsonValue::Array(_) | JsonValue::Object(_)) => None,
        }
    }
}

impl fmt::Debug for VariableValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VariableValue")
            .field("type", &self.type_name())
            .field("value", &"<redacted>")
            .finish()
    }
}

/// One validated CLI-compatible `NAME=VALUE` assignment.
#[derive(Clone, PartialEq, Eq)]
pub struct VariableAssignment {
    name: VariableName,
    value: String,
}

impl VariableAssignment {
    /// Creates an assignment from a validated name and arbitrary UTF-8 text.
    #[must_use]
    pub fn new(name: VariableName, value: impl Into<String>) -> Self {
        Self {
            name,
            value: value.into(),
        }
    }

    /// Returns the assignment's validated variable name.
    #[must_use]
    pub const fn name(&self) -> &VariableName {
        &self.name
    }

    /// Returns the assigned text. Callers must avoid logging this value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl FromStr for VariableAssignment {
    type Err = VariableAssignmentError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let (name, value) = raw
            .split_once('=')
            .ok_or(VariableAssignmentError::MissingEquals)?;
        Ok(Self::new(VariableName::new(name)?, value))
    }
}

impl fmt::Debug for VariableAssignment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VariableAssignment")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .finish()
    }
}

/// A cloneable variable scope with deterministic insertion order.
///
/// Applying CLI assignments replaces previous environment or CLI values while
/// retaining the name's original position. Capture commit is added by the
/// transactional capture stage.
#[derive(Clone, Default, PartialEq)]
pub struct VariableStore {
    values: IndexMap<VariableName, VariableValue>,
}

impl VariableStore {
    /// Creates an empty variable scope.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads valid Unicode values from the current process environment.
    ///
    /// Non-Unicode names or values and names that violate domain identifier
    /// rules are skipped. No environment value is included in diagnostics.
    #[must_use]
    pub fn from_environment() -> Self {
        let mut store = Self::new();
        store.extend_environment(std::env::vars_os());
        store
    }

    /// Loads environment-shaped entries, primarily for deterministic tests.
    ///
    /// Invalid or non-Unicode entries are skipped using the same policy as
    /// [`from_environment`](Self::from_environment).
    pub fn extend_environment(&mut self, entries: impl IntoIterator<Item = (OsString, OsString)>) {
        for (name, value) in entries {
            let (Ok(name), Ok(value)) = (name.into_string(), value.into_string()) else {
                continue;
            };
            let Ok(name) = VariableName::new(name) else {
                continue;
            };
            self.values.insert(name, VariableValue::Text(value));
        }
    }

    /// Applies assignments from left to right using last-assignment-wins.
    pub fn apply_cli(&mut self, assignments: impl IntoIterator<Item = VariableAssignment>) {
        for assignment in assignments {
            self.values
                .insert(assignment.name, VariableValue::Text(assignment.value));
        }
    }

    /// Returns a value without copying its potentially large contents.
    #[must_use]
    pub fn get(&self, name: &VariableName) -> Option<&VariableValue> {
        self.values.get(name)
    }

    /// Returns whether a name is defined in this scope.
    #[must_use]
    pub fn contains(&self, name: &VariableName) -> bool {
        self.values.contains_key(name)
    }

    /// Returns the number of variables in this scope.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns whether this scope contains no variables.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Iterates over variable names in deterministic insertion order.
    pub fn names(&self) -> impl ExactSizeIterator<Item = &VariableName> {
        self.values.keys()
    }
}

impl fmt::Debug for VariableStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VariableStore")
            .field("names", &self.values.keys().collect::<Vec<_>>())
            .field("values", &"<redacted>")
            .finish()
    }
}
