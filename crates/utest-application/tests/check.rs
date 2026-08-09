use utest_application::{
    CheckDiagnostic, CheckDiagnosticKind, CheckPhase, CheckReport, check_source,
};
use utest_domain::{HttpMethod, SuiteBlock, TestSuite, Value, VariableName};
use utest_parser::{
    LexerErrorKind, ParserErrorKind, SourceSpan, SourceText, ValidationContext, ValidationErrorKind,
};

fn check(input: &str) -> CheckReport {
    check_source(
        &SourceText::new("check-test.utest", input),
        &ValidationContext::new(),
    )
}

#[test]
fn successful_source_is_converted_without_diagnostics() {
    let source = SourceText::new(
        "valid.utest",
        r#"
            test "fetch user" {
                request GET "/users/${USER_ID}" {
                    headers { "X-Trace" = "trace ${USER_ID}" }
                }
                expect { status = 200 }
            }
        "#,
    );
    let context = ValidationContext::new().with_predefined_variable(
        VariableName::new("USER_ID").expect("the fixture variable name is valid"),
    );

    let report = check_source(&source, &context);

    assert!(report.is_success());
    assert!(!report.has_errors());
    assert!(report.diagnostics.is_empty());
    let suite = report.suite.expect("valid input must produce a suite");
    assert_eq!(suite.len(), 1);
    let SuiteBlock::Test(test) = &suite.blocks[0] else {
        panic!("the fixture must convert to a standalone test");
    };
    assert_eq!(test.name, "fetch user");
    assert_eq!(test.request.method, HttpMethod::GET);
    assert_eq!(test.request.path.as_str(), "/users/${USER_ID}");
    let Value::String(trace_header) = &test.request.headers["X-Trace"] else {
        panic!("header must retain its interpolated string representation");
    };
    assert_eq!(trace_header.as_str(), "trace ${USER_ID}");
    assert_eq!(test.expectation.status, Some(200));
}

#[test]
fn lexical_errors_are_all_reported_and_stop_later_phases() {
    let report = check("@ ? $ test \"otherwise valid\" { request GET \"/\" expect {} }");

    assert!(report.has_errors());
    assert!(!report.is_success());
    assert!(report.suite.is_none());
    assert_eq!(report.diagnostics.len(), 3);
    assert_eq!(
        report
            .diagnostics
            .iter()
            .map(CheckDiagnostic::phase)
            .collect::<Vec<_>>(),
        vec![
            CheckPhase::Lexical,
            CheckPhase::Lexical,
            CheckPhase::Lexical,
        ]
    );
    assert_eq!(report.diagnostics[0].span, SourceSpan::new(0, 1));
    assert_eq!(report.diagnostics[1].span, SourceSpan::new(2, 3));
    assert_eq!(report.diagnostics[2].span, SourceSpan::new(4, 5));
    assert!(matches!(
        report.diagnostics[0].kind,
        CheckDiagnosticKind::Lexical(LexerErrorKind::UnexpectedCharacter { character: '@' })
    ));
    assert!(report.diagnostics.iter().all(|diagnostic| !matches!(
        diagnostic.kind,
        CheckDiagnosticKind::Syntax(_) | CheckDiagnosticKind::Semantic(_)
    )));
}

#[test]
fn syntax_errors_are_all_reported_and_stop_semantic_validation() {
    let source = r#"
        test "first" {}
        test "second" {}
    "#;
    let report = check(source);

    assert!(report.has_errors());
    assert!(report.suite.is_none());
    assert_eq!(report.diagnostics.len(), 4);
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.phase() == CheckPhase::Syntax)
    );
    assert_eq!(
        report
            .diagnostics
            .iter()
            .filter(|diagnostic| matches!(
                diagnostic.kind,
                CheckDiagnosticKind::Syntax(ParserErrorKind::MissingRequest)
            ))
            .count(),
        2
    );
    assert_eq!(
        report
            .diagnostics
            .iter()
            .filter(|diagnostic| matches!(
                diagnostic.kind,
                CheckDiagnosticKind::Syntax(ParserErrorKind::MissingExpectation)
            ))
            .count(),
        2
    );
    assert!(report.diagnostics.iter().all(|diagnostic| {
        diagnostic.span.start < diagnostic.span.end && diagnostic.span.end <= source.len()
    }));
}

#[test]
fn semantic_errors_have_spans_and_never_return_a_partial_suite() {
    let source = r#"test "invalid" {
        request GET "/users/${MISSING}"
        expect { status = 99 }
    }"#;
    let report = check(source);

    assert!(report.has_errors());
    assert!(report.suite.is_none());
    assert_eq!(report.diagnostics.len(), 2);
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.phase() == CheckPhase::Semantic)
    );

    let undefined = report
        .diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.kind,
                CheckDiagnosticKind::Semantic(ValidationErrorKind::UndefinedVariable { ref name })
                    if name == "MISSING"
            )
        })
        .expect("undefined interpolation must be diagnosed");
    assert!(source[undefined.span.start..undefined.span.end].contains("MISSING"));

    let invalid_status = report
        .diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.kind,
                CheckDiagnosticKind::Semantic(ValidationErrorKind::InvalidHttpStatus {
                    status: 99
                })
            )
        })
        .expect("invalid HTTP status must be diagnosed");
    assert_eq!(
        &source[invalid_status.span.start..invalid_status.span.end],
        "99"
    );
}

#[test]
fn phases_have_stable_names_and_value_semantics() {
    let cases = [
        (CheckPhase::Lexical, "lexical", "Lexical"),
        (CheckPhase::Syntax, "syntax", "Syntax"),
        (CheckPhase::Semantic, "semantic", "Semantic"),
    ];

    for (phase, name, debug_name) in cases {
        assert_eq!(phase.as_str(), name);
        assert_eq!(phase.clone(), phase);
        assert_eq!(format!("{phase:?}"), debug_name);
    }
}

#[test]
fn diagnostic_kinds_delegate_phase_display_and_value_semantics() {
    let cases = [
        (
            CheckDiagnosticKind::Lexical(LexerErrorKind::UnexpectedCharacter { character: '@' }),
            CheckPhase::Lexical,
            "unexpected character `@`",
        ),
        (
            CheckDiagnosticKind::Syntax(ParserErrorKind::UnexpectedEof { expected: "a test" }),
            CheckPhase::Syntax,
            "expected a test, found end of input",
        ),
        (
            CheckDiagnosticKind::Semantic(ValidationErrorKind::InvalidHttpStatus { status: 42 }),
            CheckPhase::Semantic,
            "HTTP status code 42 is outside 100..=599",
        ),
    ];

    for (kind, phase, message) in cases {
        assert_eq!(kind.phase(), phase);
        assert_eq!(kind.to_string(), message);
        assert_eq!(kind.clone(), kind);
        assert!(!format!("{kind:?}").is_empty());
    }

    assert_ne!(
        CheckDiagnosticKind::Lexical(LexerErrorKind::UnterminatedString),
        CheckDiagnosticKind::Lexical(LexerErrorKind::UnexpectedCharacter { character: '@' })
    );
}

#[test]
fn diagnostics_expose_fields_phase_and_derived_traits() {
    let diagnostic = CheckDiagnostic {
        kind: CheckDiagnosticKind::Semantic(ValidationErrorKind::EmptyRequestPath),
        span: SourceSpan::new(7, 12),
    };

    assert_eq!(diagnostic.phase(), CheckPhase::Semantic);
    assert_eq!(diagnostic.span.start, 7);
    assert_eq!(diagnostic.span.end, 12);
    assert_eq!(diagnostic.clone(), diagnostic);
    assert!(format!("{diagnostic:?}").contains("EmptyRequestPath"));
}

#[test]
fn manually_constructed_reports_reflect_diagnostic_presence() {
    let success = CheckReport {
        suite: Some(TestSuite::new(Vec::new())),
        diagnostics: Vec::new(),
    };
    assert!(success.is_success());
    assert!(!success.has_errors());
    assert!(success.suite.is_some());
    assert!(format!("{success:?}").contains("diagnostics: []"));

    let failure = CheckReport {
        suite: None,
        diagnostics: vec![CheckDiagnostic {
            kind: CheckDiagnosticKind::Syntax(ParserErrorKind::MissingRequest),
            span: SourceSpan::new(1, 2),
        }],
    };
    assert!(!failure.is_success());
    assert!(failure.has_errors());
    assert!(failure.suite.is_none());
}
