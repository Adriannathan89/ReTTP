//! Success-only response capture staging and atomic transaction data.

use std::{fmt, sync::Arc};

use indexmap::IndexMap;
use rettp_assertion::{
    AssertionEngine, AssertionReport, ResolvedBodyAssertion, ResolvedObjectAssertion,
    ResolvedResponseExpectation,
};
use rettp_domain::VariableName;
use rettp_http::{HttpResponse, ResponseBody};
use serde_json::{Map as JsonMap, Value as JsonValue};

use crate::{RuntimeError, VariableValue};

/// Evaluates assertions and stages captures only for a completely valid response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CaptureEngine;

impl CaptureEngine {
    /// Evaluates one response and prepares a capture transaction on success.
    ///
    /// A failed assertion report always carries no pending captures. Successful
    /// evaluation walks capture declarations in deterministic field order and
    /// retains the actual JSON values.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError`] only when programmatically constructed resolved
    /// models violate an invariant that a successful assertion normally proves.
    pub fn evaluate(
        &self,
        assertions: &AssertionEngine,
        expected: &ResolvedResponseExpectation,
        actual: &HttpResponse,
    ) -> Result<CaptureEvaluation, RuntimeError> {
        let report = assertions.evaluate(expected, actual);
        if !report.is_success() {
            return Ok(CaptureEvaluation {
                report,
                pending: None,
            });
        }

        let mut pending = PendingCaptures::new();
        let Some(ResolvedBodyAssertion::Json(expected_object)) = &expected.body else {
            return Ok(CaptureEvaluation {
                report,
                pending: Some(pending),
            });
        };
        if !contains_capture(expected_object) {
            return Ok(CaptureEvaluation {
                report,
                pending: Some(pending),
            });
        }
        let ResponseBody::Json {
            value: JsonValue::Object(actual_object),
            ..
        } = &actual.body
        else {
            return Err(RuntimeError::InvalidCaptureBody);
        };

        extract_object(
            expected_object,
            actual_object,
            None,
            &mut Vec::new(),
            "$",
            &mut pending,
        )?;
        Ok(CaptureEvaluation {
            report,
            pending: Some(pending),
        })
    }
}

/// Assertion result together with captures staged from the same response.
#[derive(Debug, PartialEq)]
pub struct CaptureEvaluation {
    report: AssertionReport,
    pending: Option<PendingCaptures>,
}

impl CaptureEvaluation {
    /// Returns the assertion result that governed capture staging.
    #[must_use]
    pub const fn report(&self) -> &AssertionReport {
        &self.report
    }

    /// Returns staged captures, or `None` when any assertion failed.
    #[must_use]
    pub const fn pending(&self) -> Option<&PendingCaptures> {
        self.pending.as_ref()
    }

    /// Consumes the evaluation into its report and optional transaction.
    #[must_use]
    pub fn into_parts(self) -> (AssertionReport, Option<PendingCaptures>) {
        (self.report, self.pending)
    }
}

/// A move-only transaction containing captures from one successful test.
///
/// Values are deliberately inaccessible and redacted from [`Debug`]. The only
/// public mutation path is consuming the transaction through
/// [`VariableStore::commit`](crate::VariableStore::commit).
#[derive(Default, PartialEq)]
pub struct PendingCaptures {
    values: IndexMap<VariableName, VariableValue>,
}

impl PendingCaptures {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn insert(
        &mut self,
        name: VariableName,
        value: VariableValue,
    ) -> Result<(), RuntimeError> {
        if self.values.contains_key(&name) {
            return Err(RuntimeError::DuplicateVariable { name });
        }
        self.values.insert(name, value);
        Ok(())
    }

    /// Returns the number of staged variables.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns whether no captures were declared by the successful test.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Iterates over staged names without exposing their values.
    pub fn names(&self) -> impl ExactSizeIterator<Item = &VariableName> {
        self.values.keys()
    }

    pub(crate) fn into_values(self) -> IndexMap<VariableName, VariableValue> {
        self.values
    }
}

impl fmt::Debug for PendingCaptures {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingCaptures")
            .field("names", &self.values.keys().collect::<Vec<_>>())
            .field("values", &"<redacted>")
            .finish()
    }
}

fn contains_capture(assertion: &ResolvedObjectAssertion) -> bool {
    assertion
        .fields
        .values()
        .any(|field| field.capture.is_some() || field.nested.as_ref().is_some_and(contains_capture))
}

fn extract_object(
    expected: &ResolvedObjectAssertion,
    actual: &JsonMap<String, JsonValue>,
    shared_ancestor: Option<(Arc<JsonValue>, usize)>,
    capture_path: &mut Vec<String>,
    path: &str,
    pending: &mut PendingCaptures,
) -> Result<(), RuntimeError> {
    for field in expected.fields.values() {
        let field_path = child_path(path, &field.field_name);
        let actual_value =
            actual
                .get(&field.field_name)
                .ok_or_else(|| RuntimeError::MissingCaptureField {
                    path: field_path.clone(),
                })?;
        capture_path.push(field.field_name.clone());

        let mut descendant_ancestor = shared_ancestor.clone();
        if let Some(capture) = &field.capture {
            let captured = if let Some((root, base_depth)) = &shared_ancestor {
                VariableValue::shared_json(Arc::clone(root), &capture_path[*base_depth..])
            } else {
                let root = Arc::new(actual_value.clone());
                descendant_ancestor = Some((Arc::clone(&root), capture_path.len()));
                VariableValue::shared_json(root, &[])
            };
            pending.insert(capture.variable.clone(), captured)?;
        }
        if let Some(nested) = &field.nested {
            let JsonValue::Object(actual_nested) = actual_value else {
                return Err(RuntimeError::InvalidNestedCaptureField {
                    path: field_path,
                    actual_type: json_type_name(actual_value),
                });
            };
            extract_object(
                nested,
                actual_nested,
                descendant_ancestor,
                capture_path,
                &field_path,
                pending,
            )?;
        }
        capture_path.pop();
    }
    Ok(())
}

fn child_path(parent: &str, field: &str) -> String {
    if is_identifier(field) {
        format!("{parent}.{field}")
    } else {
        format!(
            "{parent}[{}]",
            serde_json::to_string(field).expect("string serialization is infallible")
        )
    }
}

fn is_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character.is_alphabetic() || character == '_')
        && characters.all(|character| character.is_alphanumeric() || character == '_')
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
