use utest_parser::{ParserError, ParserErrorKind, SourceSpan};

#[test]
fn parser_error_new_preserves_kind_and_span() {
    let span = SourceSpan::new(4, 9);
    let error = ParserError::new(
        ParserErrorKind::UnexpectedToken {
            expected: "a test block",
            found: "pipeline".to_owned(),
        },
        span,
    );

    assert_eq!(error.span, span);
    assert_eq!(
        error.kind.to_string(),
        "expected a test block, found pipeline"
    );
}

#[test]
fn every_parser_error_kind_has_a_human_readable_message() {
    let cases = [
        (
            ParserErrorKind::UnexpectedEof { expected: "`}`" },
            "expected `}`, found end of input".to_owned(),
        ),
        (
            ParserErrorKind::DuplicateCore,
            "a suite may contain at most one core block".to_owned(),
        ),
        (
            ParserErrorKind::EmptyPipeline,
            "a pipeline must contain at least one test".to_owned(),
        ),
        (
            ParserErrorKind::MissingRequest,
            "a test must contain at least one request".to_owned(),
        ),
        (
            ParserErrorKind::MissingExpectation,
            "a test must contain at least one expectation".to_owned(),
        ),
        (
            ParserErrorKind::RequestAfterExpectation,
            "request declarations must appear before expectations".to_owned(),
        ),
        (
            ParserErrorKind::CaptureRequiresType,
            "a capture requires an explicit assertion type".to_owned(),
        ),
        (
            ParserErrorKind::MissingFieldAssertion,
            "a field must contain a type assertion, value comparison, or nested object assertion"
                .to_owned(),
        ),
        (
            ParserErrorKind::NestingLimitExceeded { limit: 128 },
            "maximum parser nesting depth of 128 exceeded".to_owned(),
        ),
    ];

    for (kind, expected) in cases {
        assert_eq!(kind.to_string(), expected);
    }
}
