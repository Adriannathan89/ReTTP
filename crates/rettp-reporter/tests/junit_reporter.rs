use rettp_reporter::{
    JunitReporter, REPORT_SCHEMA_VERSION, ReportAssertionFailure, ReportBlock, ReportBlockKind,
    ReportErrorKind, ReportExecutionError, ReportFailureKind, ReportStatus, ReportSummary,
    ReportTest, RunReport,
};

use rettp_domain::{
    AssertionFailure, AssertionFailureKind, BlockResult, ExecutionErrorInfo, ExecutionErrorKind,
    ExecutionStatus, SuiteResult, TestResult,
};

fn test(
    name: &str,
    status: ReportStatus,
    duration_ms: u64,
    failures: Vec<ReportAssertionFailure>,
    error: Option<ReportExecutionError>,
) -> ReportTest {
    ReportTest {
        name: name.into(),
        status,
        duration_ms,
        failures,
        error,
    }
}

fn error(kind: ReportErrorKind, message: &str) -> ReportExecutionError {
    ReportExecutionError {
        kind,
        message: message.into(),
    }
}

#[test]
fn renders_all_status_semantics_and_consistent_aggregate_counts() {
    let assertion = ReportAssertionFailure {
        path: "$.a<&".into(),
        kind: ReportFailureKind::ValueMismatch,
        expected: Some("<redacted>".into()),
        actual: None,
        message: "values differ > safely".into(),
    };
    let report = RunReport {
        schema_version: REPORT_SCHEMA_VERSION,
        source: "ignored.rttp".into(),
        name: Some("API & <suite> \"one\"".into()),
        status: ReportStatus::Aborted,
        duration_ms: 3_004,
        summary: ReportSummary {
            total: 7,
            passed: 1,
            failed: 3,
            skipped: 1,
            aborted: 2,
        },
        blocks: vec![
            ReportBlock {
                kind: ReportBlockKind::Core,
                name: None,
                status: ReportStatus::Passed,
                duration_ms: 1,
                tests: vec![test("pass & go", ReportStatus::Passed, 1, vec![], None)],
            },
            ReportBlock {
                kind: ReportBlockKind::Pipeline,
                name: Some("pipe<'\"&".into()),
                status: ReportStatus::Failed,
                duration_ms: 2_003,
                tests: vec![
                    test("assert", ReportStatus::Failed, 1_001, vec![assertion], None),
                    test(
                        "runtime",
                        ReportStatus::Failed,
                        2,
                        vec![],
                        Some(error(ReportErrorKind::Timeout, "request <timed & out>")),
                    ),
                    test(
                        "skip",
                        ReportStatus::Skipped,
                        0,
                        vec![],
                        Some(error(
                            ReportErrorKind::DependencyFailed,
                            "dependency \"failed\" & stopped",
                        )),
                    ),
                ],
            },
            ReportBlock {
                kind: ReportBlockKind::Pipeline,
                name: None,
                status: ReportStatus::Failed,
                duration_ms: 0,
                tests: vec![test(
                    "implicit-error",
                    ReportStatus::Failed,
                    0,
                    vec![],
                    None,
                )],
            },
            ReportBlock {
                kind: ReportBlockKind::Test,
                name: None,
                status: ReportStatus::Aborted,
                duration_ms: 1_000,
                tests: vec![
                    test("aborted", ReportStatus::Aborted, 1_000, vec![], None),
                    test("default-skip", ReportStatus::Skipped, 0, vec![], None),
                ],
            },
        ],
        error: Some(error(
            ReportErrorKind::Internal,
            "suite <invariant> & failed",
        )),
    };

    let output = JunitReporter.render(&report);

    assert!(output.ends_with('\n'));
    assert!(!output.ends_with("\n\n"));
    assert!(output.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
    assert!(output.contains(concat!(
        "<testsuites name=\"API &amp; &lt;suite&gt; &quot;one&quot;\" ",
        "tests=\"8\" failures=\"1\" errors=\"4\" skipped=\"2\" time=\"3.004\">"
    )));
    assert!(output.contains(concat!(
        "<testsuite name=\"pipeline:pipe&lt;&apos;&quot;&amp;\" tests=\"3\" ",
        "failures=\"1\" errors=\"1\" skipped=\"1\" time=\"2.003\">"
    )));
    assert!(output.contains(
        "<failure type=\"assertion\" message=\"1 assertion failure(s)\">\n$.a&lt;&amp; [value_mismatch]: values differ &gt; safely\nexpected: &lt;redacted&gt;\n"
    ));
    assert!(!output.contains("actual:"));
    assert!(
        output.contains("<error type=\"timeout\" message=\"request &lt;timed &amp; out&gt;\" />")
    );
    assert!(output.contains("<skipped message=\"dependency &quot;failed&quot; &amp; stopped\" />"));
    assert!(output.contains("<skipped message=\"dependency failed\" />"));
    assert_eq!(
        output
            .matches("<error type=\"internal\" message=\"test execution failed\" />")
            .count(),
        2
    );
    assert!(output.contains(
        "<testsuite name=\"rettp\" tests=\"1\" failures=\"0\" errors=\"1\" skipped=\"0\" time=\"0.000\">"
    ));
    assert!(
        output.contains(
            "<error type=\"internal\" message=\"suite &lt;invariant&gt; &amp; failed\" />"
        )
    );
    assert!(output.ends_with("</testsuites>\n"));
}

#[test]
fn escapes_xml_attributes_text_and_replaces_forbidden_xml_characters() {
    let forbidden = '\u{1}';
    let report = RunReport {
        schema_version: REPORT_SCHEMA_VERSION,
        source: format!("source<&\"'{forbidden}\u{9}\u{a}\u{d}\u{10000}"),
        name: None,
        status: ReportStatus::Failed,
        duration_ms: u64::MAX,
        summary: ReportSummary::default(),
        blocks: vec![ReportBlock {
            kind: ReportBlockKind::Test,
            name: Some(format!("name{forbidden}")),
            status: ReportStatus::Failed,
            duration_ms: u64::MAX,
            tests: vec![test(
                &format!("case{forbidden}"),
                ReportStatus::Failed,
                u64::MAX,
                vec![ReportAssertionFailure {
                    path: format!("path{forbidden}&<>\"'"),
                    kind: ReportFailureKind::TypeMismatch,
                    expected: None,
                    actual: Some(format!("actual{forbidden}&<>\"'")),
                    message: format!("message{forbidden}&<>\"'"),
                }],
                None,
            )],
        }],
        error: None,
    };

    let output = JunitReporter.render(&report);

    assert!(!output.contains(forbidden));
    assert!(output.contains('\u{fffd}'));
    assert!(output.contains("name=\"source&lt;&amp;&quot;&apos;�\t\n\r\u{10000}\""));
    assert!(output.contains("name=\"test:name�\""));
    assert!(output.contains("name=\"case�\""));
    assert!(output.contains("path�&amp;&lt;&gt;\"' [type_mismatch]"));
    assert!(output.contains("message�&amp;&lt;&gt;\"'"));
    assert!(output.contains("actual: actual�&amp;&lt;&gt;\"'"));
    assert!(output.contains(&format!(
        "time=\"{}.{:03}\"",
        u64::MAX / 1_000,
        u64::MAX % 1_000
    )));
}

#[test]
fn empty_report_uses_source_name_and_zero_counts() {
    let report = RunReport {
        schema_version: REPORT_SCHEMA_VERSION,
        source: "empty.rttp".into(),
        name: None,
        status: ReportStatus::Passed,
        duration_ms: 0,
        summary: ReportSummary::default(),
        blocks: Vec::new(),
        error: None,
    };

    assert_eq!(
        JunitReporter.render(&report),
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<testsuites name=\"empty.rttp\" tests=\"0\" failures=\"0\" errors=\"0\" skipped=\"0\" time=\"0.000\">\n",
            "</testsuites>\n",
        )
    );
}

#[test]
fn reporter_has_copy_value_semantics() {
    let reporter = JunitReporter;
    let copied = reporter;

    assert_eq!(reporter, copied);
    assert!(format!("{reporter:?}").contains("JunitReporter"));
}

#[test]
fn converted_domain_secrets_never_reach_junit_output() {
    let result = SuiteResult {
        name: Some("safe suite".into()),
        status: ExecutionStatus::Aborted,
        duration_ms: 1,
        blocks: vec![BlockResult::Test(TestResult::failed(
            "assertion",
            1,
            vec![AssertionFailure {
                path: "$.password".into(),
                kind: AssertionFailureKind::ValueMismatch,
                expected: Some("expected=hunter2".into()),
                actual: Some("actual=abc123".into()),
                message: "raw assertion token=top-secret".into(),
            }],
            None,
        ))],
        error: Some(ExecutionErrorInfo {
            kind: ExecutionErrorKind::Internal,
            message: "raw suite password=hunter2".into(),
        }),
    };

    let output = JunitReporter.render(&RunReport::from_suite_result("safe.rttp", &result));

    assert!(output.contains("expected: &lt;redacted&gt;"));
    assert!(output.contains("actual: &lt;redacted&gt;"));
    assert!(output.contains("the actual value does not equal the expected value"));
    assert!(output.contains("an internal execution invariant failed"));
    for secret in ["hunter2", "abc123", "top-secret"] {
        assert!(!output.contains(secret));
    }
}
