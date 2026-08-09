//! Redacted runtime variable values, assignments, and scope storage.

use std::{borrow::Cow, ffi::OsString, fmt, str::FromStr, sync::Arc};

use indexmap::IndexMap;
use serde_json::Value as JsonValue;
use utest_domain::VariableName;

use crate::{PendingCaptures, RuntimeError, VariableAssignmentError};

/// A runtime value supplied as text or captured as typed JSON.
///
/// Its storage representation is private: captured parent/descendant values
/// can share immutable JSON without changing the accessor contract. Cloning a
/// capture therefore does not deep-clone its selected JSON subtree.
///
/// The custom [`Debug`] implementation reveals only the value's type. Use the
/// accessors deliberately when trusted application code needs the contents.
#[derive(Clone)]
pub struct VariableValue {
    inner: VariableValueInner,
}

#[derive(Clone)]
enum VariableValueInner {
    Text(String),
    Json(JsonValue),
    SharedJson(SharedJsonValue),
}

impl VariableValue {
    /// Creates a UTF-8 value supplied by an environment, CLI, or embedding host.
    #[must_use]
    pub fn text(value: impl Into<String>) -> Self {
        Self {
            inner: VariableValueInner::Text(value.into()),
        }
    }

    /// Creates an owned typed JSON value supplied by an embedding host.
    #[must_use]
    pub fn json(value: JsonValue) -> Self {
        Self {
            inner: VariableValueInner::Json(value),
        }
    }

    /// Returns the stable JSON-compatible type name of the stored value.
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match &self.inner {
            VariableValueInner::Text(_) => "string",
            VariableValueInner::Json(value) => json_type_name(value),
            VariableValueInner::SharedJson(value) => json_type_name(value.as_json()),
        }
    }

    /// Returns text supplied by an environment or CLI source.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match &self.inner {
            VariableValueInner::Text(value) => Some(value),
            VariableValueInner::Json(_) | VariableValueInner::SharedJson(_) => None,
        }
    }

    /// Returns the captured JSON value without cloning it.
    #[must_use]
    pub fn as_json(&self) -> Option<&JsonValue> {
        match &self.inner {
            VariableValueInner::Json(value) => Some(value),
            VariableValueInner::SharedJson(value) => Some(value.as_json()),
            VariableValueInner::Text(_) => None,
        }
    }

    pub(crate) fn scalar_text(&self) -> Option<Cow<'_, str>> {
        match &self.inner {
            VariableValueInner::Text(value) => Some(Cow::Borrowed(value)),
            VariableValueInner::Json(value) => scalar_json_text(value),
            VariableValueInner::SharedJson(value) => scalar_json_text(value.as_json()),
        }
    }

    pub(crate) fn is_structured(&self) -> bool {
        self.as_json()
            .is_some_and(|value| value.is_array() || value.is_object())
    }

    pub(crate) fn shared_json(root: Arc<JsonValue>, path: &[String]) -> Self {
        Self {
            inner: VariableValueInner::SharedJson(SharedJsonValue::at_path(root, path)),
        }
    }
}

impl From<String> for VariableValue {
    fn from(value: String) -> Self {
        Self::text(value)
    }
}

impl From<&str> for VariableValue {
    fn from(value: &str) -> Self {
        Self::text(value)
    }
}

impl From<JsonValue> for VariableValue {
    fn from(value: JsonValue) -> Self {
        Self::json(value)
    }
}

impl PartialEq for VariableValue {
    fn eq(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (VariableValueInner::Text(left), VariableValueInner::Text(right)) => left == right,
            (
                VariableValueInner::Json(_) | VariableValueInner::SharedJson(_),
                VariableValueInner::Json(_) | VariableValueInner::SharedJson(_),
            ) => self.as_json() == other.as_json(),
            _ => false,
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

#[derive(Clone)]
struct SharedJsonValue {
    root: Arc<JsonValue>,
    path: Arc<[String]>,
}

impl SharedJsonValue {
    fn as_json(&self) -> &JsonValue {
        let mut value = self.root.as_ref();
        for segment in self.path.iter() {
            value = value
                .get(segment)
                .expect("a shared capture path must remain valid");
        }
        value
    }

    fn at_path(root: Arc<JsonValue>, path: &[String]) -> Self {
        Self {
            root,
            path: Arc::from(path),
        }
    }
}

fn scalar_json_text(value: &JsonValue) -> Option<Cow<'_, str>> {
    match value {
        JsonValue::String(value) => Some(Cow::Borrowed(value)),
        JsonValue::Null => Some(Cow::Borrowed("null")),
        JsonValue::Bool(value) => Some(Cow::Owned(value.to_string())),
        JsonValue::Number(value) => Some(Cow::Owned(value.to_string())),
        JsonValue::Array(_) | JsonValue::Object(_) => None,
    }
}

fn json_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(number) if number.is_i64() || number.is_u64() => "integer",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
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
/// retaining the name's original position. Capture commit never replaces an
/// existing name and validates the entire transaction before mutation.
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
            self.insert_predefined(name, VariableValue::text(value));
        }
    }

    /// Applies assignments from left to right using last-assignment-wins.
    pub fn apply_cli(&mut self, assignments: impl IntoIterator<Item = VariableAssignment>) {
        for assignment in assignments {
            self.insert_predefined(assignment.name, VariableValue::text(assignment.value));
        }
    }

    /// Adds or replaces a predefined value supplied by an embedding host.
    ///
    /// Replacement follows the same policy as CLI precedence and preserves an
    /// existing name's insertion position. Capture transactions never call
    /// this method and therefore cannot overwrite visible variables.
    pub fn insert_predefined(&mut self, name: VariableName, value: VariableValue) {
        self.values.insert(name, value);
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

    /// Atomically commits captures from one successful test.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::DuplicateVariable`] when any captured name is
    /// already visible. The store is unchanged when an error is returned.
    pub fn commit(&mut self, pending: PendingCaptures) -> Result<(), RuntimeError> {
        if let Some(name) = pending.names().find(|name| self.contains(name)) {
            return Err(RuntimeError::DuplicateVariable { name: name.clone() });
        }
        self.values.extend(pending.into_values());
        Ok(())
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
