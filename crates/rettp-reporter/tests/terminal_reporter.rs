use rettp_domain::{
    AssertionFailure, AssertionFailureKind, BlockResult, ExecutionErrorInfo, ExecutionErrorKind,
    ExecutionStatus, PipelineResult, SuiteResult, TestResult,
};
use rettp_reporter::{ColorMode, RunReport, TerminalReporter};

fn report_with_every_status() -> RunReport {
    let assertion = AssertionFailure {
        path: "$.token".into(),
        kind: AssertionFailureKind::ValueMismatch,
        expected: Some("expected-secret".into()),
        actual: Some("actual-secret".into()),
        message: "raw assertion secret".into(),
    };
    let result = SuiteResult {
        name: Some("API\nSuite".into()),
        status: ExecutionStatus::Aborted,
        duration_ms: 42,
        blocks: vec![
            BlockResult::Pipeline(PipelineResult {
                name: "checkout\u{1b}[31m".into(),
                status: ExecutionStatus::Failed,
                duration_ms: 30,
                tests: vec![
                    TestResult::passed("health\ncheck", 1),
                    TestResult::failed("assert", 2, vec![assertion], None),
                    TestResult::skipped("later", "raw dependency secret"),
                ],
            }),
            BlockResult::Test(TestResult::aborted("standalone", 9, "raw internal secret")),
        ],
        error: Some(ExecutionErrorInfo {
            kind: ExecutionErrorKind::Internal,
            message: "raw suite secret".into(),
        }),
    };
    RunReport::from_suite_result("tests/api\nrettp", &result)
}

#[test]
fn renders_plain_report_deterministically_and_redacts_domain_values() {
    let output = TerminalReporter::default().render(&report_with_every_status());

    assert_eq!(
        output,
        concat!(
            "Rettp: \"tests/api\\nrettp\"\n",
            "[ABORT] \"API\\nSuite\" (42 ms)\n",
            "  [FAIL] pipeline \"checkout\\u{1b}[31m\" (3 tests, 30 ms)\n",
            "    [PASS] \"health\\ncheck\" (1 ms)\n",
            "    [FAIL] \"assert\" (2 ms)\n",
            "      - $.token [value_mismatch]: the actual value does not equal the expected value\n",
            "        expected: <redacted>\n",
            "        actual: <redacted>\n",
            "    [SKIP] \"later\" (0 ms)\n",
            "      error [dependency_failed]: execution was skipped because a dependency failed\n",
            "  [ABORT] test \"standalone\" (1 tests, 9 ms)\n",
            "    [ABORT] \"standalone\" (9 ms)\n",
            "      error [internal]: an internal execution invariant failed\n",
            "Summary: 4 total, 1 passed, 1 failed, 1 skipped, 1 aborted\n",
            "Suite error [internal]: an internal execution invariant failed\n",
        )
    );
    assert!(!output.contains('\u{1b}'));
    for secret in [
        "expected-secret",
        "actual-secret",
        "raw assertion secret",
        "raw dependency secret",
        "raw internal secret",
        "raw suite secret",
    ] {
        assert!(!output.contains(secret));
    }
}

#[test]
fn ansi_mode_colors_each_status_and_resets_every_label() {
    let output = TerminalReporter::new(ColorMode::Ansi).render(&report_with_every_status());

    assert!(output.contains("\u{1b}[32m[PASS]\u{1b}[0m"));
    assert!(output.contains("\u{1b}[31m[FAIL]\u{1b}[0m"));
    assert!(output.contains("\u{1b}[90m[SKIP]\u{1b}[0m"));
    assert!(output.contains("\u{1b}[31m[ABORT]\u{1b}[0m"));
    assert_eq!(output.matches("\u{1b}[0m").count(), 7);
}

#[test]
fn renders_unnamed_empty_blocks_and_optional_failure_fields() {
    let mut report = report_with_every_status();
    report.name = None;
    report.error = None;
    report.blocks.clear();
    report.blocks.push(rettp_reporter::ReportBlock {
        kind: rettp_reporter::ReportBlockKind::Core,
        name: None,
        status: rettp_reporter::ReportStatus::Passed,
        duration_ms: 0,
        tests: vec![rettp_reporter::ReportTest {
            name: "minimal".into(),
            status: rettp_reporter::ReportStatus::Failed,
            duration_ms: 0,
            failures: vec![rettp_reporter::ReportAssertionFailure {
                path: "body".into(),
                kind: rettp_reporter::ReportFailureKind::MissingField,
                expected: None,
                actual: None,
                message: "a required field is missing".into(),
            }],
            error: None,
        }],
    });
    report.blocks.push(rettp_reporter::ReportBlock {
        kind: rettp_reporter::ReportBlockKind::Pipeline,
        name: None,
        status: rettp_reporter::ReportStatus::Passed,
        duration_ms: 0,
        tests: Vec::new(),
    });

    let output = TerminalReporter::default().render(&report);

    assert!(output.contains("[ABORT] \"unnamed suite\""));
    assert!(output.contains("  [PASS] core (1 tests, 0 ms)"));
    assert!(output.contains("  [PASS] pipeline (0 tests, 0 ms)"));
    assert!(output.contains("      - body [missing_field]: a required field is missing"));
    assert!(!output.contains("expected:"));
    assert!(!output.contains("actual:"));
    assert!(!output.contains("Suite error"));
}

#[test]
fn reporter_value_semantics_expose_the_selected_color_mode() {
    let plain = TerminalReporter::default();
    let ansi = TerminalReporter::new(ColorMode::Ansi);

    assert_eq!(plain.color_mode(), ColorMode::Plain);
    assert_eq!(ansi.color_mode(), ColorMode::Ansi);
    assert_ne!(plain, ansi);
    assert!(format!("{ansi:?}").contains("Ansi"));
}

#[test]
fn plain_mode_escapes_controls_from_every_free_form_report_field() {
    let mut report = report_with_every_status();
    report.source = "source\u{1b}\n".into();
    report.name = Some("suite\r\t".into());
    report.blocks[0].name = Some("block\u{7}".into());
    let test = &mut report.blocks[0].tests[1];
    test.name = "test\u{8}".into();
    test.failures[0].path = "path\u{1b}[2J\nnext".into();
    test.failures[0].message = "message\rrewritten".into();
    test.failures[0].expected = Some("expected\u{0}".into());
    test.failures[0].actual = Some("actual\tvalue".into());
    test.error = Some(rettp_reporter::ReportExecutionError {
        kind: rettp_reporter::ReportErrorKind::Internal,
        message: "test-error\u{1b}[31m".into(),
    });
    report.error = Some(rettp_reporter::ReportExecutionError {
        kind: rettp_reporter::ReportErrorKind::Internal,
        message: "suite-error\u{7}\n".into(),
    });

    let output = TerminalReporter::default().render(&report);

    assert!(output.contains("\"source\\u{1b}\\n\""));
    assert!(output.contains("\"suite\\r\\t\""));
    assert!(output.contains("pipeline \"block\\u{7}\""));
    assert!(output.contains("\"test\\u{8}\""));
    assert!(output.contains("path\\u{1b}[2J\\nnext"));
    assert!(output.contains("message\\rrewritten"));
    assert!(output.contains("expected\\u{0}"));
    assert!(output.contains("actual\\tvalue"));
    assert!(output.contains("test-error\\u{1b}[31m"));
    assert!(output.contains("suite-error\\u{7}\\n"));
    assert!(
        output
            .chars()
            .filter(|character| character.is_control())
            .all(|character| character == '\n')
    );
}
