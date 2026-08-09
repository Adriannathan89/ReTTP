//! Status, header, text, empty-body, and recursive JSON assertion evaluation.

use std::str;

use serde_json::{Map, Number, Value as JsonValue};
use utest_domain::{AssertionFailure, AssertionFailureKind, ExpectedType, ObjectMatchMode};
use utest_http::{HttpResponse, ResponseBody};

use crate::{
    AssertionConfig, AssertionReport, ResolvedBodyAssertion, ResolvedFieldAssertion,
    ResolvedHeaderAssertion, ResolvedObjectAssertion, ResolvedResponseExpectation,
    ResolvedTextAssertion,
};

const MAX_VALUE_PREVIEW_CHARS: usize = 256;
const MAX_EXACT_FLOAT_INTEGER: u64 = 9_007_199_254_740_991;

/// Evaluates fully resolved expectations against HTTP responses.
///
/// The engine is immutable and inexpensive to clone, so one configured value
/// can be shared by an executor across every test in a suite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AssertionEngine {
    config: AssertionConfig,
}

impl AssertionEngine {
    /// Creates an engine using previously validated resource limits.
    #[must_use]
    pub const fn new(config: AssertionConfig) -> Self {
        Self { config }
    }

    /// Returns the resource limits used by this engine.
    #[must_use]
    pub const fn config(self) -> AssertionConfig {
        self.config
    }

    /// Evaluates every requested response assertion in deterministic order.
    ///
    /// Status is evaluated first, followed by headers in declaration order and
    /// then the body. Evaluation stops after discovering a failure beyond the
    /// configured retention limit, and the returned report is marked as
    /// truncated.
    #[must_use]
    pub fn evaluate(
        &self,
        expected: &ResolvedResponseExpectation,
        actual: &HttpResponse,
    ) -> AssertionReport {
        let mut evaluation = Evaluation::new(self.config);

        evaluation.assert_status(expected.status, actual.status);
        if !evaluation.should_stop() {
            evaluation.assert_headers(expected, actual);
        }
        if !evaluation.should_stop() {
            evaluation.assert_body(expected.body.as_ref(), actual);
        }

        evaluation.finish()
    }
}

struct Evaluation {
    config: AssertionConfig,
    failures: Vec<AssertionFailure>,
    truncated: bool,
}

impl Evaluation {
    fn new(config: AssertionConfig) -> Self {
        Self {
            config,
            failures: Vec::with_capacity(config.max_failures().min(16)),
            truncated: false,
        }
    }

    fn finish(self) -> AssertionReport {
        AssertionReport::new(self.failures, self.truncated)
    }

    fn should_stop(&self) -> bool {
        self.truncated
    }

    fn push(&mut self, failure: AssertionFailure) {
        if self.failures.len() == self.config.max_failures() {
            self.truncated = true;
        } else {
            self.failures.push(failure);
        }
    }

    fn assert_status(&mut self, expected: Option<u16>, actual: u16) {
        let Some(expected) = expected else {
            return;
        };
        if expected != actual {
            self.push(AssertionFailure {
                path: "status".to_owned(),
                kind: AssertionFailureKind::StatusMismatch,
                expected: Some(expected.to_string()),
                actual: Some(actual.to_string()),
                message: format!("expected HTTP status {expected}, received {actual}"),
            });
        }
    }

    fn assert_headers(&mut self, expected: &ResolvedResponseExpectation, actual: &HttpResponse) {
        for (name, assertion) in &expected.headers {
            let values = actual.headers.get(name);
            let matches = match assertion {
                ResolvedHeaderAssertion::Exists => values.is_some_and(|values| !values.is_empty()),
                ResolvedHeaderAssertion::Exact(expected) => values.is_some_and(|values| {
                    values.len() == 1 && values[0].as_ref() == expected.as_bytes()
                }),
                ResolvedHeaderAssertion::Contains(expected) => values.is_some_and(|values| {
                    values.iter().any(|value| {
                        str::from_utf8(value).is_ok_and(|actual| actual.contains(expected.as_str()))
                    })
                }),
            };

            if !matches {
                self.push(header_failure(name, assertion, values));
                if self.should_stop() {
                    return;
                }
            }
        }
    }

    fn assert_body(&mut self, expected: Option<&ResolvedBodyAssertion>, actual: &HttpResponse) {
        let Some(expected) = expected else {
            return;
        };

        match expected {
            ResolvedBodyAssertion::Empty => {
                if !actual.raw_body().is_empty() {
                    self.push(AssertionFailure {
                        path: "$".to_owned(),
                        kind: AssertionFailureKind::ValueMismatch,
                        expected: Some("empty body".to_owned()),
                        actual: Some(format!("{} bytes", actual.raw_body().len())),
                        message: "expected an empty response body".to_owned(),
                    });
                }
            }
            ResolvedBodyAssertion::Text(assertion) => {
                let ResponseBody::Text(bytes) = &actual.body else {
                    self.push(invalid_body_failure(
                        "text",
                        response_body_kind(&actual.body),
                    ));
                    return;
                };
                let Ok(text) = str::from_utf8(bytes) else {
                    self.push(AssertionFailure {
                        path: "$".to_owned(),
                        kind: AssertionFailureKind::InvalidBody,
                        expected: Some("valid UTF-8 text".to_owned()),
                        actual: Some("non-UTF-8 text body".to_owned()),
                        message: "text response body is not valid UTF-8".to_owned(),
                    });
                    return;
                };
                self.assert_text(assertion, text);
            }
            ResolvedBodyAssertion::Json(assertion) => {
                let ResponseBody::Json { value, .. } = &actual.body else {
                    self.push(invalid_body_failure(
                        "JSON",
                        response_body_kind(&actual.body),
                    ));
                    return;
                };
                let Some(object) = value.as_object() else {
                    self.push(type_failure("$", "object", json_type_name(value)));
                    return;
                };
                self.assert_object(assertion, object, "$", 0);
            }
        }
    }

    fn assert_text(&mut self, assertion: &ResolvedTextAssertion, actual: &str) {
        let (expected, matches, comparison) = match assertion {
            ResolvedTextAssertion::Exact(expected) => {
                (expected, actual == expected, "equal exactly")
            }
            ResolvedTextAssertion::Contains(expected) => {
                (expected, actual.contains(expected), "contain")
            }
        };

        if !matches {
            self.push(AssertionFailure {
                path: "$".to_owned(),
                kind: AssertionFailureKind::ValueMismatch,
                expected: Some(preview_string(expected)),
                actual: Some(preview_string(actual)),
                message: format!("expected text response to {comparison} the expected value"),
            });
        }
    }

    fn assert_object(
        &mut self,
        assertion: &ResolvedObjectAssertion,
        actual: &Map<String, JsonValue>,
        path: &str,
        depth: usize,
    ) {
        if !self.enter_depth(path, depth) {
            return;
        }

        for (declared_name, field) in &assertion.fields {
            let field_path = child_path(path, declared_name);
            let Some(actual_value) = actual.get(declared_name) else {
                self.push(AssertionFailure {
                    path: field_path,
                    kind: AssertionFailureKind::MissingField,
                    expected: Some(field.expected_type.as_str().to_owned()),
                    actual: None,
                    message: format!("required field `{declared_name}` is missing"),
                });
                if self.should_stop() {
                    return;
                }
                continue;
            };

            self.assert_field(field, actual_value, &field_path, depth);
            if self.should_stop() {
                return;
            }
        }

        if assertion.mode == ObjectMatchMode::Exact {
            for actual_name in actual.keys() {
                if !assertion.fields.contains_key(actual_name) {
                    self.push(AssertionFailure {
                        path: child_path(path, actual_name),
                        kind: AssertionFailureKind::UnexpectedField,
                        expected: None,
                        actual: Some(preview_json(&actual[actual_name])),
                        message: format!(
                            "field `{actual_name}` is not permitted by the exact object assertion"
                        ),
                    });
                    if self.should_stop() {
                        return;
                    }
                }
            }
        }
    }

    fn assert_field(
        &mut self,
        assertion: &ResolvedFieldAssertion,
        actual: &JsonValue,
        path: &str,
        depth: usize,
    ) {
        if !matches_expected_type(actual, &assertion.expected_type) {
            self.push(type_failure(
                path,
                assertion.expected_type.as_str(),
                json_type_name(actual),
            ));
            return;
        }

        if let Some(expected) = &assertion.expected_value {
            let flexible_number = assertion.expected_type == ExpectedType::Number;
            self.compare_json(expected, actual, path, depth + 1, flexible_number);
            if self.should_stop() {
                return;
            }
        }

        if let Some(nested) = &assertion.nested {
            let Some(actual) = actual.as_object() else {
                return;
            };
            self.assert_object(nested, actual, path, depth + 1);
        }
    }

    fn compare_json(
        &mut self,
        expected: &JsonValue,
        actual: &JsonValue,
        path: &str,
        depth: usize,
        flexible_number: bool,
    ) {
        if self.should_stop() {
            return;
        }

        match expected {
            JsonValue::Object(expected) => {
                let Some(actual) = actual.as_object() else {
                    self.push(type_failure(path, "object", json_type_name(actual)));
                    return;
                };
                if !self.enter_depth(path, depth) {
                    return;
                }
                for (key, expected_value) in expected {
                    let child_path = child_path(path, key);
                    let Some(actual_value) = actual.get(key) else {
                        self.push(AssertionFailure {
                            path: child_path,
                            kind: AssertionFailureKind::MissingField,
                            expected: Some(preview_json(expected_value)),
                            actual: None,
                            message: format!("required compared field `{key}` is missing"),
                        });
                        if self.should_stop() {
                            return;
                        }
                        continue;
                    };
                    self.compare_json(
                        expected_value,
                        actual_value,
                        &child_path,
                        depth + 1,
                        expected_number_is_flexible(expected_value),
                    );
                    if self.should_stop() {
                        return;
                    }
                }
            }
            JsonValue::Array(expected) => {
                let Some(actual) = actual.as_array() else {
                    self.push(type_failure(path, "array", json_type_name(actual)));
                    return;
                };
                if !self.enter_depth(path, depth) {
                    return;
                }
                if expected.len() != actual.len() {
                    self.push(AssertionFailure {
                        path: path.to_owned(),
                        kind: AssertionFailureKind::ValueMismatch,
                        expected: Some(format!("array length {}", expected.len())),
                        actual: Some(format!("array length {}", actual.len())),
                        message: "array lengths differ".to_owned(),
                    });
                    if self.should_stop() {
                        return;
                    }
                }
                for (index, (expected_value, actual_value)) in
                    expected.iter().zip(actual).enumerate()
                {
                    let child_path = format!("{path}[{index}]");
                    self.compare_json(
                        expected_value,
                        actual_value,
                        &child_path,
                        depth + 1,
                        expected_number_is_flexible(expected_value),
                    );
                    if self.should_stop() {
                        return;
                    }
                }
            }
            JsonValue::Number(expected) => {
                let Some(actual) = actual.as_number() else {
                    self.push(type_failure(
                        path,
                        number_type_name(expected),
                        json_type_name(actual),
                    ));
                    return;
                };
                let type_matches =
                    flexible_number || !number_is_integer(expected) || number_is_integer(actual);
                if !type_matches {
                    self.push(type_failure(path, "integer", "number"));
                } else if !numbers_equal(expected, actual) {
                    self.push(value_failure(
                        path,
                        expected.to_string(),
                        actual.to_string(),
                    ));
                }
            }
            JsonValue::String(expected) => match actual.as_str() {
                Some(actual) if actual == expected => {}
                Some(actual) => self.push(value_failure(
                    path,
                    preview_string(expected),
                    preview_string(actual),
                )),
                None => self.push(type_failure(path, "string", json_type_name(actual))),
            },
            JsonValue::Bool(expected) => match actual.as_bool() {
                Some(actual) if actual == *expected => {}
                Some(actual) => {
                    self.push(value_failure(
                        path,
                        expected.to_string(),
                        actual.to_string(),
                    ));
                }
                None => self.push(type_failure(path, "boolean", json_type_name(actual))),
            },
            JsonValue::Null => {
                if !actual.is_null() {
                    self.push(type_failure(path, "null", json_type_name(actual)));
                }
            }
        }
    }

    fn enter_depth(&mut self, path: &str, depth: usize) -> bool {
        if depth <= self.config.max_json_depth() {
            return true;
        }
        self.push(AssertionFailure {
            path: path.to_owned(),
            kind: AssertionFailureKind::InvalidBody,
            expected: Some(format!(
                "JSON nesting at most {} levels",
                self.config.max_json_depth()
            )),
            actual: Some(format!("JSON nesting exceeds level {depth}")),
            message: "JSON assertion comparison depth limit exceeded".to_owned(),
        });
        false
    }
}

fn header_failure(
    name: &str,
    assertion: &ResolvedHeaderAssertion,
    actual: Option<&[bytes::Bytes]>,
) -> AssertionFailure {
    let expected = match assertion {
        ResolvedHeaderAssertion::Exists => "header to exist".to_owned(),
        ResolvedHeaderAssertion::Exact(value) => {
            format!("exactly one value equal to {}", preview_string(value))
        }
        ResolvedHeaderAssertion::Contains(value) => {
            format!("a value containing {}", preview_string(value))
        }
    };
    AssertionFailure {
        path: format!("headers[{name:?}]"),
        kind: AssertionFailureKind::HeaderMismatch,
        expected: Some(expected),
        actual: actual.map(preview_header_values),
        message: format!("response header `{name}` did not satisfy its assertion"),
    }
}

fn preview_header_values(values: &[bytes::Bytes]) -> String {
    if values.len() != 1 {
        return format!("{} values", values.len());
    }
    match str::from_utf8(&values[0]) {
        Ok(value) => preview_string(value),
        Err(_) => "non-UTF-8 value".to_owned(),
    }
}

fn invalid_body_failure(expected: &str, actual: &str) -> AssertionFailure {
    AssertionFailure {
        path: "$".to_owned(),
        kind: AssertionFailureKind::InvalidBody,
        expected: Some(expected.to_owned()),
        actual: Some(actual.to_owned()),
        message: format!("expected a {expected} response body, received {actual}"),
    }
}

fn type_failure(path: &str, expected: &str, actual: &str) -> AssertionFailure {
    AssertionFailure {
        path: path.to_owned(),
        kind: AssertionFailureKind::TypeMismatch,
        expected: Some(expected.to_owned()),
        actual: Some(actual.to_owned()),
        message: format!("expected type `{expected}`, received `{actual}`"),
    }
}

fn value_failure(path: &str, expected: String, actual: String) -> AssertionFailure {
    AssertionFailure {
        path: path.to_owned(),
        kind: AssertionFailureKind::ValueMismatch,
        expected: Some(expected),
        actual: Some(actual),
        message: "actual value does not equal the expected value".to_owned(),
    }
}

fn matches_expected_type(value: &JsonValue, expected: &ExpectedType) -> bool {
    match expected {
        ExpectedType::String => value.is_string(),
        ExpectedType::Boolean => value.is_boolean(),
        ExpectedType::Integer => value.as_number().is_some_and(number_is_integer),
        ExpectedType::Number => value.is_number(),
        ExpectedType::Object => value.is_object(),
        ExpectedType::Array => value.is_array(),
        ExpectedType::Null => value.is_null(),
    }
}

fn expected_number_is_flexible(value: &JsonValue) -> bool {
    value
        .as_number()
        .is_some_and(|number| !number_is_integer(number))
}

fn number_is_integer(number: &Number) -> bool {
    number.is_i64() || number.is_u64()
}

fn number_type_name(number: &Number) -> &'static str {
    if number_is_integer(number) {
        "integer"
    } else {
        "number"
    }
}

fn numbers_equal(expected: &Number, actual: &Number) -> bool {
    match (integer_value(expected), integer_value(actual)) {
        (Some(expected), Some(actual)) => expected == actual,
        (Some(integer), None) => safe_integer_as_f64(integer)
            .zip(actual.as_f64())
            .is_some_and(|(expected, actual)| expected == actual),
        (None, Some(integer)) => expected
            .as_f64()
            .zip(safe_integer_as_f64(integer))
            .is_some_and(|(expected, actual)| expected == actual),
        (None, None) => expected
            .as_f64()
            .zip(actual.as_f64())
            .is_some_and(|(expected, actual)| expected == actual),
    }
}

fn integer_value(number: &Number) -> Option<i128> {
    number
        .as_i64()
        .map(i128::from)
        .or_else(|| number.as_u64().map(i128::from))
}

fn safe_integer_as_f64(value: i128) -> Option<f64> {
    let limit = i128::from(MAX_EXACT_FLOAT_INTEGER);
    (value >= -limit && value <= limit).then_some(value as f64)
}

fn json_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(number) => number_type_name(number),
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

fn response_body_kind(body: &ResponseBody) -> &'static str {
    match body {
        ResponseBody::Empty => "empty",
        ResponseBody::Json { .. } => "JSON",
        ResponseBody::Text(_) => "text",
        ResponseBody::Binary(_) => "binary",
    }
}

fn child_path(parent: &str, key: &str) -> String {
    if is_identifier(key) {
        format!("{parent}.{key}")
    } else {
        let quoted = serde_json::to_string(key).unwrap_or_else(|_| format!("{key:?}"));
        format!("{parent}[{quoted}]")
    }
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn preview_json(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => "null".to_owned(),
        JsonValue::Bool(value) => value.to_string(),
        JsonValue::Number(value) => value.to_string(),
        JsonValue::String(value) => preview_string(value),
        JsonValue::Array(values) => format!("array with {} elements", values.len()),
        JsonValue::Object(values) => format!("object with {} fields", values.len()),
    }
}

fn preview_string(value: &str) -> String {
    let mut chars = value.chars();
    let preview: String = chars.by_ref().take(MAX_VALUE_PREVIEW_CHARS).collect();
    if chars.next().is_some() {
        format!("{preview:?}…")
    } else {
        format!("{preview:?}")
    }
}
