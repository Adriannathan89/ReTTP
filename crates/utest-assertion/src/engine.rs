//! Status, header, text, and empty-body assertion evaluation.

use std::str;

use utest_domain::{AssertionFailure, AssertionFailureKind};
use utest_http::{HttpResponse, ResponseBody};

use crate::{
    AssertionConfig, AssertionReport, ResolvedBodyAssertion, ResolvedHeaderAssertion,
    ResolvedResponseExpectation, ResolvedTextAssertion,
};

const MAX_VALUE_PREVIEW_CHARS: usize = 256;

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
            ResolvedBodyAssertion::Json(_) => {
                if !matches!(actual.body, ResponseBody::Json { .. }) {
                    self.push(invalid_body_failure(
                        "JSON",
                        response_body_kind(&actual.body),
                    ));
                }
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

fn response_body_kind(body: &ResponseBody) -> &'static str {
    match body {
        ResponseBody::Empty => "empty",
        ResponseBody::Json { .. } => "JSON",
        ResponseBody::Text(_) => "text",
        ResponseBody::Binary(_) => "binary",
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
