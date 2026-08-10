use utest_domain::{
    AssertionFailure, AssertionFailureKind, BlockResult, CoreResult, ExecutionErrorInfo,
    ExecutionErrorKind, ExecutionStatus, PipelineResult, SuiteResult, TestResult,
};
use utest_reporter::{
    REPORT_SCHEMA_VERSION, ReportBlockKind, ReportErrorKind, ReportFailureKind, ReportStatus,
    RunReport,
};

fn assertion(
    kind: AssertionFailureKind,
    expected: Option<&str>,
    actual: Option<&str>,
) -> AssertionFailure {
    AssertionFailure {
        path: "$.credential".into(),
        kind,
        expected: expected.map(str::to_owned),
        actual: actual.map(str::to_owned),
        message: "raw secret message: password=hunter2".into(),
    }
}

fn error(kind: ExecutionErrorKind) -> ExecutionErrorInfo {
    ExecutionErrorInfo {
        kind,
        message: "raw secret error: token=abc123".into(),
    }
}

#[test]
fn converts_every_block_and_status_in_source_order() {
    let result = SuiteResult {
        name: Some("API suite".into()),
        status: ExecutionStatus::Aborted,
        duration_ms: u64::MAX,
        blocks: vec![
            BlockResult::Core(CoreResult {
                status: ExecutionStatus::Passed,
                duration_ms: 1,
                tests: vec![TestResult::passed("bootstrap", 1)],
            }),
            BlockResult::Pipeline(PipelineResult {
                name: "checkout".into(),
                status: ExecutionStatus::Failed,
                duration_ms: 2,
                tests: vec![
                    TestResult::failed("charge", 2, Vec::new(), None),
                    TestResult::skipped("receipt", "raw dependency secret"),
                ],
            }),
            BlockResult::Test(TestResult::aborted("standalone", 3, "raw internal secret")),
        ],
        error: Some(error(ExecutionErrorKind::Internal)),
    };

    let report = RunReport::from_suite_result("tests/api.utest", &result);

    assert_eq!(report.schema_version, REPORT_SCHEMA_VERSION);
    assert_eq!(report.source, "tests/api.utest");
    assert_eq!(report.name.as_deref(), Some("API suite"));
    assert_eq!(report.status, ReportStatus::Aborted);
    assert_eq!(report.duration_ms, u64::MAX);
    assert_eq!(report.summary.total, 4);
    assert_eq!(report.summary.passed, 1);
    assert_eq!(report.summary.failed, 1);
    assert_eq!(report.summary.skipped, 1);
    assert_eq!(report.summary.aborted, 1);

    assert_eq!(report.blocks[0].kind, ReportBlockKind::Core);
    assert_eq!(report.blocks[0].name, None);
    assert_eq!(report.blocks[0].status, ReportStatus::Passed);
    assert_eq!(report.blocks[0].tests[0].name, "bootstrap");

    assert_eq!(report.blocks[1].kind, ReportBlockKind::Pipeline);
    assert_eq!(report.blocks[1].name.as_deref(), Some("checkout"));
    assert_eq!(report.blocks[1].status, ReportStatus::Failed);
    assert_eq!(report.blocks[1].tests[1].status, ReportStatus::Skipped);
    assert_eq!(
        report.blocks[1].tests[1]
            .error
            .as_ref()
            .map(|value| value.kind),
        Some(ReportErrorKind::DependencyFailed)
    );

    assert_eq!(report.blocks[2].kind, ReportBlockKind::Test);
    assert_eq!(report.blocks[2].name.as_deref(), Some("standalone"));
    assert_eq!(report.blocks[2].status, ReportStatus::Aborted);
    assert_eq!(report.blocks[2].tests[0].status, ReportStatus::Aborted);
    assert_eq!(
        report.error.as_ref().map(|value| value.message.as_str()),
        Some("an internal execution invariant failed")
    );

    let debug = format!("{report:?}");
    assert!(!debug.contains("raw dependency secret"));
    assert!(!debug.contains("raw internal secret"));
    assert!(!debug.contains("token=abc123"));
}

#[test]
fn maps_all_failure_kinds_and_redacts_value_bearing_previews() {
    let failures = vec![
        assertion(AssertionFailureKind::MissingField, None, Some("secret-a")),
        assertion(
            AssertionFailureKind::TypeMismatch,
            Some("integer"),
            Some("secret-b"),
        ),
        assertion(
            AssertionFailureKind::ValueMismatch,
            Some("secret-c"),
            Some("secret-d"),
        ),
        assertion(
            AssertionFailureKind::UnexpectedField,
            Some("secret-e"),
            Some("secret-f"),
        ),
        assertion(
            AssertionFailureKind::StatusMismatch,
            Some("200"),
            Some("secret-g"),
        ),
        assertion(
            AssertionFailureKind::HeaderMismatch,
            Some("secret-h"),
            Some("secret-i"),
        ),
        assertion(
            AssertionFailureKind::InvalidBody,
            Some("JSON"),
            Some("secret-j"),
        ),
    ];
    let result = SuiteResult {
        name: None,
        status: ExecutionStatus::Failed,
        duration_ms: 0,
        blocks: vec![BlockResult::Test(TestResult::failed(
            "redaction",
            0,
            failures,
            None,
        ))],
        error: None,
    };

    let report = RunReport::from_suite_result("safe.utest", &result);
    let failures = &report.blocks[0].tests[0].failures;
    assert_eq!(
        failures
            .iter()
            .map(|failure| failure.kind)
            .collect::<Vec<_>>(),
        vec![
            ReportFailureKind::MissingField,
            ReportFailureKind::TypeMismatch,
            ReportFailureKind::ValueMismatch,
            ReportFailureKind::UnexpectedField,
            ReportFailureKind::StatusMismatch,
            ReportFailureKind::HeaderMismatch,
            ReportFailureKind::InvalidBody,
        ]
    );
    assert_eq!(failures[0].expected, None);
    assert_eq!(failures[0].actual.as_deref(), Some("<redacted>"));
    assert_eq!(failures[1].expected.as_deref(), Some("integer"));
    assert_eq!(failures[1].actual.as_deref(), Some("<redacted>"));
    assert_eq!(failures[4].expected.as_deref(), Some("200"));
    assert_eq!(failures[4].actual.as_deref(), Some("<redacted>"));
    assert_eq!(failures[6].expected.as_deref(), Some("JSON"));
    assert_eq!(failures[6].actual.as_deref(), Some("<redacted>"));
    assert!(
        failures
            .iter()
            .all(|failure| !failure.message.contains("hunter2"))
    );
    assert!(
        failures
            .iter()
            .flat_map(|failure| [failure.expected.as_deref(), failure.actual.as_deref()])
            .flatten()
            .all(|preview| !preview.starts_with("secret-"))
    );
}

#[test]
fn permits_only_documented_structural_previews() {
    const SAFE_PREVIEWS: &[&str] = &[
        "empty",
        "JSON",
        "text",
        "binary",
        "object",
        "array",
        "string",
        "boolean",
        "integer",
        "number",
        "null",
        "valid UTF-8 text",
        "non-UTF-8 text body",
    ];

    let failures = SAFE_PREVIEWS
        .iter()
        .map(|preview| {
            assertion(
                AssertionFailureKind::TypeMismatch,
                Some(preview),
                Some(preview),
            )
        })
        .chain([
            assertion(
                AssertionFailureKind::TypeMismatch,
                Some("Integer"),
                Some("JSON "),
            ),
            assertion(
                AssertionFailureKind::StatusMismatch,
                Some("0"),
                Some("65535"),
            ),
            assertion(
                AssertionFailureKind::StatusMismatch,
                Some("65536"),
                Some("200 OK"),
            ),
        ])
        .collect();
    let result = SuiteResult {
        name: None,
        status: ExecutionStatus::Failed,
        duration_ms: 0,
        blocks: vec![BlockResult::Test(TestResult::failed(
            "preview", 0, failures, None,
        ))],
        error: None,
    };

    let report = RunReport::from_suite_result("preview.utest", &result);
    let failures = &report.blocks[0].tests[0].failures;
    for (failure, preview) in failures.iter().zip(SAFE_PREVIEWS) {
        assert_eq!(failure.expected.as_deref(), Some(*preview));
        assert_eq!(failure.actual.as_deref(), Some(*preview));
    }
    assert_eq!(
        failures[SAFE_PREVIEWS.len()].expected.as_deref(),
        Some("<redacted>")
    );
    assert_eq!(
        failures[SAFE_PREVIEWS.len()].actual.as_deref(),
        Some("<redacted>")
    );
    assert_eq!(
        failures[SAFE_PREVIEWS.len() + 1].expected.as_deref(),
        Some("<redacted>")
    );
    assert_eq!(
        failures[SAFE_PREVIEWS.len() + 1].actual.as_deref(),
        Some("<redacted>")
    );
    assert_eq!(
        failures[SAFE_PREVIEWS.len() + 2].expected.as_deref(),
        Some("<redacted>")
    );
    assert_eq!(
        failures[SAFE_PREVIEWS.len() + 2].actual.as_deref(),
        Some("<redacted>")
    );
}

#[test]
fn maps_all_execution_error_kinds_to_value_free_messages() {
    let kinds = [
        ExecutionErrorKind::InvalidRequest,
        ExecutionErrorKind::Connection,
        ExecutionErrorKind::Timeout,
        ExecutionErrorKind::InvalidResponse,
        ExecutionErrorKind::VariableResolution,
        ExecutionErrorKind::DependencyFailed,
        ExecutionErrorKind::Internal,
    ];
    let expected = [
        (
            ReportErrorKind::InvalidRequest,
            "the HTTP request is invalid",
        ),
        (ReportErrorKind::Connection, "the HTTP connection failed"),
        (ReportErrorKind::Timeout, "the HTTP request timed out"),
        (
            ReportErrorKind::InvalidResponse,
            "the HTTP response is invalid",
        ),
        (
            ReportErrorKind::VariableResolution,
            "runtime variable resolution failed",
        ),
        (
            ReportErrorKind::DependencyFailed,
            "execution was skipped because a dependency failed",
        ),
        (
            ReportErrorKind::Internal,
            "an internal execution invariant failed",
        ),
    ];

    for (domain_kind, (report_kind, safe_message)) in kinds.into_iter().zip(expected) {
        let result = SuiteResult {
            name: None,
            status: ExecutionStatus::Failed,
            duration_ms: 0,
            blocks: Vec::new(),
            error: Some(error(domain_kind)),
        };
        let report = RunReport::from_suite_result("error.utest", &result);
        let report_error = report.error.expect("suite error should be represented");
        assert_eq!(report_error.kind, report_kind);
        assert_eq!(report_error.message, safe_message);
        assert!(!report_error.message.contains("abc123"));
    }
}

#[test]
fn stable_public_enum_spellings_are_exhaustive() {
    let statuses = [
        (ReportStatus::Passed, "PASS", "passed"),
        (ReportStatus::Failed, "FAIL", "failed"),
        (ReportStatus::Skipped, "SKIP", "skipped"),
        (ReportStatus::Aborted, "ABORT", "aborted"),
    ];
    for (status, label, spelling) in statuses {
        assert_eq!(status.label(), label);
        assert_eq!(status.as_str(), spelling);
        assert_eq!(
            serde_json::to_string(&status).unwrap(),
            format!("\"{spelling}\"")
        );
    }

    for (kind, spelling) in [
        (ReportBlockKind::Core, "core"),
        (ReportBlockKind::Pipeline, "pipeline"),
        (ReportBlockKind::Test, "test"),
    ] {
        assert_eq!(kind.as_str(), spelling);
        assert_eq!(
            serde_json::to_string(&kind).unwrap(),
            format!("\"{spelling}\"")
        );
    }

    for (kind, spelling) in [
        (ReportFailureKind::MissingField, "missing_field"),
        (ReportFailureKind::TypeMismatch, "type_mismatch"),
        (ReportFailureKind::ValueMismatch, "value_mismatch"),
        (ReportFailureKind::UnexpectedField, "unexpected_field"),
        (ReportFailureKind::StatusMismatch, "status_mismatch"),
        (ReportFailureKind::HeaderMismatch, "header_mismatch"),
        (ReportFailureKind::InvalidBody, "invalid_body"),
    ] {
        assert_eq!(kind.as_str(), spelling);
        assert_eq!(
            serde_json::to_string(&kind).unwrap(),
            format!("\"{spelling}\"")
        );
    }

    for (kind, spelling) in [
        (ReportErrorKind::InvalidRequest, "invalid_request"),
        (ReportErrorKind::Connection, "connection"),
        (ReportErrorKind::Timeout, "timeout"),
        (ReportErrorKind::InvalidResponse, "invalid_response"),
        (ReportErrorKind::VariableResolution, "variable_resolution"),
        (ReportErrorKind::DependencyFailed, "dependency_failed"),
        (ReportErrorKind::Internal, "internal"),
    ] {
        assert_eq!(kind.as_str(), spelling);
        assert_eq!(
            serde_json::to_string(&kind).unwrap(),
            format!("\"{spelling}\"")
        );
    }
}

#[test]
fn report_round_trips_through_the_versioned_schema() {
    let result = SuiteResult {
        name: None,
        status: ExecutionStatus::Passed,
        duration_ms: 0,
        blocks: Vec::new(),
        error: None,
    };
    let report = RunReport::from_suite_result(String::from("empty.utest"), &result);
    let encoded = serde_json::to_string(&report).unwrap();
    let decoded: RunReport = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded, report);
    assert_eq!(decoded.summary.total, 0);
    assert_eq!(decoded.summary.passed, 0);
    assert!(decoded.blocks.is_empty());
}
