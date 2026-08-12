use bytes::Bytes;
use indexmap::IndexMap;
use rettp_assertion::{
    AssertionConfig, AssertionEngine, ResolvedBodyAssertion, ResolvedHeaderAssertion,
    ResolvedObjectAssertion, ResolvedResponseExpectation, ResolvedTextAssertion,
};
use rettp_domain::AssertionFailureKind;
use rettp_http::{HttpResponse, ResponseBody, ResponseHeaders};
use serde_json::json;

fn response(status: u16, headers: ResponseHeaders, body: ResponseBody) -> HttpResponse {
    HttpResponse {
        status,
        headers,
        body,
    }
}

fn expectation(
    status: Option<u16>,
    headers: IndexMap<String, ResolvedHeaderAssertion>,
    body: Option<ResolvedBodyAssertion>,
) -> ResolvedResponseExpectation {
    ResolvedResponseExpectation {
        status,
        headers,
        body,
    }
}

#[test]
fn absent_expectations_produce_an_empty_success_report() {
    let actual = response(
        503,
        ResponseHeaders::new().with_header("x-ignored", "value"),
        ResponseBody::Binary(Bytes::from_static(b"ignored")),
    );

    let report =
        AssertionEngine::default().evaluate(&ResolvedResponseExpectation::default(), &actual);

    assert!(report.is_success());
    assert!(report.is_empty());
    assert_eq!(report.len(), 0);
    assert!(!report.is_truncated());
    assert!(report.into_failures().is_empty());
}

#[test]
fn status_assertion_reports_stable_structured_details() {
    let actual = response(404, ResponseHeaders::new(), ResponseBody::Empty);

    let passed = AssertionEngine::default()
        .evaluate(&expectation(Some(404), IndexMap::new(), None), &actual);
    assert!(passed.is_success());

    let failed = AssertionEngine::default()
        .evaluate(&expectation(Some(200), IndexMap::new(), None), &actual);
    assert_eq!(failed.len(), 1);
    let failure = &failed.failures()[0];
    assert_eq!(failure.path, "status");
    assert_eq!(failure.kind, AssertionFailureKind::StatusMismatch);
    assert_eq!(failure.expected.as_deref(), Some("200"));
    assert_eq!(failure.actual.as_deref(), Some("404"));
    assert_eq!(failure.message, "expected HTTP status 200, received 404");
}

#[test]
fn header_assertions_support_exists_exact_and_contains() {
    let headers = ResponseHeaders::new()
        .with_header("X-Present", "")
        .with_header("Content-Type", "application/json; charset=utf-8")
        .with_header("X-Trace", "first")
        .with_header("x-trace", "prefix-target-suffix");
    let expected_headers = IndexMap::from([
        ("x-present".to_owned(), ResolvedHeaderAssertion::Exists),
        (
            "CONTENT-TYPE".to_owned(),
            ResolvedHeaderAssertion::Exact("application/json; charset=utf-8".to_owned()),
        ),
        (
            "X-TRACE".to_owned(),
            ResolvedHeaderAssertion::Contains("target".to_owned()),
        ),
    ]);
    let actual = response(200, headers, ResponseBody::Empty);

    let report =
        AssertionEngine::default().evaluate(&expectation(None, expected_headers, None), &actual);

    assert!(report.is_success());
}

#[test]
fn exact_header_rejects_missing_wrong_and_repeated_values() {
    let headers = ResponseHeaders::new()
        .with_header("x-wrong", "actual")
        .with_header("x-repeated", "expected")
        .with_header("X-Repeated", "extra");
    let expected_headers = IndexMap::from([
        (
            "x-missing".to_owned(),
            ResolvedHeaderAssertion::Exact("expected".to_owned()),
        ),
        (
            "x-wrong".to_owned(),
            ResolvedHeaderAssertion::Exact("expected".to_owned()),
        ),
        (
            "x-repeated".to_owned(),
            ResolvedHeaderAssertion::Exact("expected".to_owned()),
        ),
    ]);

    let report = AssertionEngine::default().evaluate(
        &expectation(None, expected_headers, None),
        &response(200, headers, ResponseBody::Empty),
    );

    assert_eq!(report.len(), 3);
    assert_eq!(report.failures()[0].path, "headers[\"x-missing\"]");
    assert_eq!(report.failures()[0].actual, None);
    assert_eq!(report.failures()[1].actual.as_deref(), Some("\"actual\""));
    assert_eq!(report.failures()[2].actual.as_deref(), Some("2 values"));
    for failure in report.failures() {
        assert_eq!(failure.kind, AssertionFailureKind::HeaderMismatch);
        assert_eq!(
            failure.expected.as_deref(),
            Some("exactly one value equal to \"expected\"")
        );
    }
}

#[test]
fn exists_and_contains_failures_handle_missing_and_non_utf8_values() {
    let headers = ResponseHeaders::new().with_header("x-binary", Bytes::from_static(&[0xff]));
    let expected_headers = IndexMap::from([
        ("x-missing".to_owned(), ResolvedHeaderAssertion::Exists),
        (
            "x-binary".to_owned(),
            ResolvedHeaderAssertion::Contains("needle".to_owned()),
        ),
    ]);

    let report = AssertionEngine::default().evaluate(
        &expectation(None, expected_headers, None),
        &response(200, headers, ResponseBody::Empty),
    );

    assert_eq!(report.len(), 2);
    assert_eq!(
        report.failures()[0].expected.as_deref(),
        Some("header to exist")
    );
    assert_eq!(report.failures()[0].actual, None);
    assert_eq!(
        report.failures()[1].expected.as_deref(),
        Some("a value containing \"needle\"")
    );
    assert_eq!(
        report.failures()[1].actual.as_deref(),
        Some("non-UTF-8 value")
    );
}

#[test]
fn empty_body_means_zero_raw_bytes_regardless_of_classification() {
    let expected = expectation(None, IndexMap::new(), Some(ResolvedBodyAssertion::Empty));
    let engine = AssertionEngine::default();

    for body in [
        ResponseBody::Empty,
        ResponseBody::Json {
            raw: Bytes::new(),
            value: json!({}),
        },
        ResponseBody::Text(Bytes::new()),
        ResponseBody::Binary(Bytes::new()),
    ] {
        assert!(
            engine
                .evaluate(&expected, &response(200, ResponseHeaders::new(), body))
                .is_success()
        );
    }

    let report = engine.evaluate(
        &expected,
        &response(
            200,
            ResponseHeaders::new(),
            ResponseBody::Text(Bytes::from_static(b" ")),
        ),
    );
    let failure = &report.failures()[0];
    assert_eq!(failure.path, "$");
    assert_eq!(failure.kind, AssertionFailureKind::ValueMismatch);
    assert_eq!(failure.expected.as_deref(), Some("empty body"));
    assert_eq!(failure.actual.as_deref(), Some("1 bytes"));
    assert_eq!(failure.message, "expected an empty response body");
}

#[test]
fn text_assertions_are_strictly_classified_and_support_both_comparisons() {
    let actual = response(
        200,
        ResponseHeaders::new(),
        ResponseBody::Text(Bytes::from_static(b"service ready")),
    );
    let engine = AssertionEngine::default();

    for assertion in [
        ResolvedTextAssertion::Exact("service ready".to_owned()),
        ResolvedTextAssertion::Contains("ready".to_owned()),
    ] {
        let expected = expectation(
            None,
            IndexMap::new(),
            Some(ResolvedBodyAssertion::Text(assertion)),
        );
        assert!(engine.evaluate(&expected, &actual).is_success());
    }

    for (assertion, comparison) in [
        (
            ResolvedTextAssertion::Exact("ready".to_owned()),
            "equal exactly",
        ),
        (
            ResolvedTextAssertion::Contains("missing".to_owned()),
            "contain",
        ),
    ] {
        let expected = expectation(
            None,
            IndexMap::new(),
            Some(ResolvedBodyAssertion::Text(assertion)),
        );
        let report = engine.evaluate(&expected, &actual);
        let failure = &report.failures()[0];
        assert_eq!(failure.kind, AssertionFailureKind::ValueMismatch);
        assert_eq!(failure.actual.as_deref(), Some("\"service ready\""));
        assert_eq!(
            failure.message,
            format!("expected text response to {comparison} the expected value")
        );
    }
}

#[test]
fn text_assertion_rejects_invalid_utf8_and_every_other_body_classification() {
    let expected = expectation(
        None,
        IndexMap::new(),
        Some(ResolvedBodyAssertion::Text(ResolvedTextAssertion::Exact(
            "text".to_owned(),
        ))),
    );
    let engine = AssertionEngine::default();

    let invalid_utf8 = engine.evaluate(
        &expected,
        &response(
            200,
            ResponseHeaders::new(),
            ResponseBody::Text(Bytes::from_static(&[0xff])),
        ),
    );
    let failure = &invalid_utf8.failures()[0];
    assert_eq!(failure.kind, AssertionFailureKind::InvalidBody);
    assert_eq!(failure.expected.as_deref(), Some("valid UTF-8 text"));
    assert_eq!(failure.actual.as_deref(), Some("non-UTF-8 text body"));
    assert_eq!(failure.message, "text response body is not valid UTF-8");

    let cases = [
        (ResponseBody::Empty, "empty"),
        (
            ResponseBody::Json {
                raw: Bytes::from_static(b"{}"),
                value: json!({}),
            },
            "JSON",
        ),
        (ResponseBody::Binary(Bytes::from_static(b"text")), "binary"),
    ];
    for (body, kind) in cases {
        let report = engine.evaluate(&expected, &response(200, ResponseHeaders::new(), body));
        let failure = &report.failures()[0];
        assert_eq!(failure.kind, AssertionFailureKind::InvalidBody);
        assert_eq!(failure.expected.as_deref(), Some("text"));
        assert_eq!(failure.actual.as_deref(), Some(kind));
        assert_eq!(
            failure.message,
            format!("expected a text response body, received {kind}")
        );
    }
}

#[test]
fn json_assertion_requires_json_classification_in_this_batch() {
    let expected = expectation(
        None,
        IndexMap::new(),
        Some(ResolvedBodyAssertion::Json(
            ResolvedObjectAssertion::partial(),
        )),
    );
    let engine = AssertionEngine::default();
    let json_body = ResponseBody::Json {
        raw: Bytes::from_static(b"{}"),
        value: json!({}),
    };

    assert!(
        engine
            .evaluate(&expected, &response(200, ResponseHeaders::new(), json_body))
            .is_success()
    );

    let report = engine.evaluate(
        &expected,
        &response(
            200,
            ResponseHeaders::new(),
            ResponseBody::Text(Bytes::from_static(b"{}")),
        ),
    );
    let failure = &report.failures()[0];
    assert_eq!(failure.kind, AssertionFailureKind::InvalidBody);
    assert_eq!(failure.expected.as_deref(), Some("JSON"));
    assert_eq!(failure.actual.as_deref(), Some("text"));
}

#[test]
fn failure_order_is_status_then_declared_headers_then_body() {
    let expected_headers = IndexMap::from([
        ("x-first".to_owned(), ResolvedHeaderAssertion::Exists),
        ("x-second".to_owned(), ResolvedHeaderAssertion::Exists),
    ]);
    let expected = expectation(
        Some(200),
        expected_headers,
        Some(ResolvedBodyAssertion::Empty),
    );

    let report = AssertionEngine::default().evaluate(
        &expected,
        &response(
            500,
            ResponseHeaders::new(),
            ResponseBody::Binary(Bytes::from_static(b"body")),
        ),
    );

    assert_eq!(report.len(), 4);
    assert_eq!(
        report
            .failures()
            .iter()
            .map(|failure| failure.path.as_str())
            .collect::<Vec<_>>(),
        [
            "status",
            "headers[\"x-first\"]",
            "headers[\"x-second\"]",
            "$"
        ]
    );
    assert!(!report.is_truncated());
}

#[test]
fn report_truncates_only_after_observing_one_failure_beyond_the_limit() {
    let engine = AssertionEngine::new(
        AssertionConfig::new(2, 1).expect("test configuration should be valid"),
    );
    let two_failures = expectation(
        Some(200),
        IndexMap::from([("x-first".to_owned(), ResolvedHeaderAssertion::Exists)]),
        None,
    );
    let actual = response(500, ResponseHeaders::new(), ResponseBody::Empty);

    let exact_limit = engine.evaluate(&two_failures, &actual);
    assert_eq!(exact_limit.len(), 2);
    assert!(!exact_limit.is_truncated());

    let beyond_limit = expectation(
        Some(200),
        IndexMap::from([
            ("x-first".to_owned(), ResolvedHeaderAssertion::Exists),
            ("x-second".to_owned(), ResolvedHeaderAssertion::Exists),
        ]),
        Some(ResolvedBodyAssertion::Empty),
    );
    let report = engine.evaluate(&beyond_limit, &actual);
    assert_eq!(report.len(), 2);
    assert!(report.is_truncated());
    assert_eq!(report.failures()[0].path, "status");
    assert_eq!(report.failures()[1].path, "headers[\"x-first\"]");
}

#[test]
fn diagnostic_previews_are_unicode_safe_and_bounded_by_characters() {
    let long_value = "🦀".repeat(257);
    let expected = expectation(
        None,
        IndexMap::new(),
        Some(ResolvedBodyAssertion::Text(ResolvedTextAssertion::Exact(
            long_value,
        ))),
    );
    let actual = response(
        200,
        ResponseHeaders::new(),
        ResponseBody::Text(Bytes::from_static(b"different")),
    );

    let report = AssertionEngine::default().evaluate(&expected, &actual);
    let preview = report.failures()[0]
        .expected
        .as_deref()
        .expect("expected preview should be present");

    assert!(preview.ends_with('…'));
    assert_eq!(preview.matches('🦀').count(), 256);
}
