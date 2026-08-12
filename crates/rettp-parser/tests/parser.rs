use rettp_parser::{
    AssertionTypeAst, BlockAst, BodyAssertionAst, ExpectationSectionAst, HttpMethodAst,
    ObjectMatchModeAst, Parser, ParserErrorKind, RequestSectionAst, ResponseHeaderAssertionAst,
    SourceSpan, SourceText, Token, TokenKind, ValueAst, lex, parse,
};

fn parse_source(input: &str) -> rettp_parser::ParseResult {
    let source = SourceText::new("parser-test.rttp", input);
    let lexed = lex(&source);
    assert!(
        lexed.is_success(),
        "unexpected lexer errors: {:?}",
        lexed.errors
    );
    parse(&lexed.tokens)
}

fn error_kinds(result: &rettp_parser::ParseResult) -> Vec<&ParserErrorKind> {
    result.errors.iter().map(|error| &error.kind).collect()
}

#[test]
fn rejects_empty_input_with_or_without_an_eof_token() {
    let lexed = parse_source("");
    assert!(!lexed.is_success());
    assert!(lexed.has_errors());
    assert_eq!(lexed.errors.len(), 1);
    assert_eq!(lexed.errors[0].kind, ParserErrorKind::EmptySuite);
    assert_eq!(lexed.errors[0].span, SourceSpan::new(0, 0));
    assert!(lexed.ast.blocks.is_empty());
    assert_eq!(lexed.ast.span, SourceSpan::new(0, 0));

    let no_tokens = Parser::new(&[]).parse();
    assert!(!no_tokens.is_success());
    assert_eq!(no_tokens.errors.len(), 1);
    assert_eq!(no_tokens.errors[0].kind, ParserErrorKind::EmptySuite);
    assert_eq!(no_tokens.errors[0].span, SourceSpan::new(0, 0));
    assert_eq!(no_tokens.ast.span, SourceSpan::new(0, 0));
}

#[test]
fn invalid_top_level_input_does_not_duplicate_the_error_with_empty_suite() {
    let result = parse_source("garbage");

    assert_eq!(result.errors.len(), 1);
    assert!(matches!(
        result.errors[0].kind,
        ParserErrorKind::UnexpectedToken { .. }
    ));
    assert!(result.ast.blocks.is_empty());
    assert!(!error_kinds(&result).contains(&&ParserErrorKind::EmptySuite));
}

#[test]
fn parses_complete_suite_and_preserves_block_and_declaration_order() {
    let input = r#"
        test "standalone" {
            request GET "/health"
            request POST "/health"
            expect { status = 200 }
            expect { body empty }
        }
        core {}
        pipeline "flow" {
            test "step" {
                request PUT "/items"
                expect { status = 204 }
            }
        }
    "#;
    let result = parse_source(input);
    assert!(result.is_success(), "{:?}", result.errors);
    assert_eq!(result.ast.blocks.len(), 3);
    assert!(matches!(result.ast.blocks[0], BlockAst::Test(_)));
    assert!(matches!(result.ast.blocks[1], BlockAst::Core(_)));
    assert!(matches!(result.ast.blocks[2], BlockAst::Pipeline(_)));
    assert_eq!(result.ast.span.start, input.find("test").unwrap());

    let BlockAst::Test(test) = &result.ast.blocks[0] else {
        panic!("first block must be a standalone test");
    };
    assert_eq!(test.requests.len(), 2);
    assert_eq!(test.expectations.len(), 2);
    assert_eq!(test.requests[0].method.value, HttpMethodAst::Get);
    assert_eq!(test.requests[1].method.value, HttpMethodAst::Post);
}

#[test]
fn parses_every_http_method() {
    let result = parse_source(
        r#"test "methods" {
            request GET "/"
            request POST "/"
            request PUT "/"
            request PATCH "/"
            request DELETE "/"
            request HEAD "/"
            request OPTIONS "/"
            expect { status = 200 }
        }"#,
    );
    assert!(result.is_success(), "{:?}", result.errors);
    let BlockAst::Test(test) = &result.ast.blocks[0] else {
        panic!("expected test");
    };
    assert_eq!(
        test.requests
            .iter()
            .map(|request| request.method.value)
            .collect::<Vec<_>>(),
        vec![
            HttpMethodAst::Get,
            HttpMethodAst::Post,
            HttpMethodAst::Put,
            HttpMethodAst::Patch,
            HttpMethodAst::Delete,
            HttpMethodAst::Head,
            HttpMethodAst::Options,
        ]
    );
}

#[test]
fn parses_request_sections_and_every_literal_value() {
    let result = parse_source(
        r#"test "request data" {
            request POST "/items" {
                headers {
                    "x-text" = "hello",
                    "x-number" = 1.5,
                    "x-bool" = true,
                    "x-null" = null
                }
                query { page = 2, enabled = false }
                body {
                    title = "item",
                    count = -2,
                    ratio = 3.5,
                    active = true,
                    missing = null,
                    tags = ["a", 1, 2.5, false, null, [], {}],
                    nested = { key = "value" }
                }
            }
            expect { status = 201 }
        }"#,
    );
    assert!(result.is_success(), "{:?}", result.errors);
    let BlockAst::Test(test) = &result.ast.blocks[0] else {
        panic!("expected test");
    };
    let sections = &test.requests[0].sections;
    assert_eq!(sections.len(), 3);
    assert!(matches!(sections[0], RequestSectionAst::Headers(_)));
    assert!(matches!(sections[1], RequestSectionAst::Query(_)));
    let RequestSectionAst::Body(body) = &sections[2] else {
        panic!("expected body");
    };
    assert_eq!(body.value.entries.len(), 7);
    let ValueAst::Array(tags) = &body.value.entries[5].value else {
        panic!("expected tags array");
    };
    assert_eq!(tags.items.len(), 7);
    assert!(matches!(tags.items[4], ValueAst::Null(_)));
    assert!(matches!(tags.items[5], ValueAst::Array(_)));
    assert!(matches!(tags.items[6], ValueAst::Object(_)));
}

#[test]
fn accepts_all_non_structural_keyword_tokens_as_object_keys() {
    let result = parse_source(
        r#"test "keys" {
            request POST "/" { body {
                status = null, exact = null,
                GET = null, POST = null, PUT = null, PATCH = null,
                DELETE = null, HEAD = null, OPTIONS = null,
                string = null, boolean = null, integer = null,
                number = null, object = null, array = null,
                true = null, false = null, null = null,
                "body" = null
            } }
            expect { status = 200 }
        }"#,
    );
    assert!(result.is_success(), "{:?}", result.errors);
    let BlockAst::Test(test) = &result.ast.blocks[0] else {
        panic!("expected test");
    };
    let RequestSectionAst::Body(body) = &test.requests[0].sections[0] else {
        panic!("expected body");
    };
    assert_eq!(body.value.entries.len(), 19);
    assert_eq!(body.value.entries[0].key.value, "status");
    assert_eq!(body.value.entries[2].key.value, "GET");
    assert_eq!(body.value.entries[18].key.value, "body");
}

#[test]
fn parses_response_headers_and_all_body_assertion_forms() {
    let result = parse_source(
        r#"test "expectations" {
            request GET "/"
            expect {
                headers {
                    "content-type": string,
                    "etag" = "abc",
                    "cache-control" contains "cache"
                }
                body empty
                body = "OK"
                body contains "created"
                body exact {
                    id: integer = 1,
                    score: number,
                    enabled: boolean = true,
                    tags: array = [],
                    nothing: null = null,
                    profile: object { name: string -> USER_NAME },
                    loose = { extra = false }
                }
            }
        }"#,
    );
    assert!(result.is_success(), "{:?}", result.errors);
    let BlockAst::Test(test) = &result.ast.blocks[0] else {
        panic!("expected test");
    };
    let sections = &test.expectations[0].sections;
    let ExpectationSectionAst::Headers(headers) = &sections[0] else {
        panic!("expected response headers");
    };
    assert!(matches!(
        headers.entries[0],
        ResponseHeaderAssertionAst::Exists { .. }
    ));
    assert!(matches!(
        headers.entries[1],
        ResponseHeaderAssertionAst::Exact { .. }
    ));
    assert!(matches!(
        headers.entries[2],
        ResponseHeaderAssertionAst::Contains { .. }
    ));
    assert!(matches!(
        sections[1],
        ExpectationSectionAst::Body(BodyAssertionAst::Empty { .. })
    ));
    assert!(matches!(
        sections[2],
        ExpectationSectionAst::Body(BodyAssertionAst::TextExact { .. })
    ));
    assert!(matches!(
        sections[3],
        ExpectationSectionAst::Body(BodyAssertionAst::TextContains { .. })
    ));
    let ExpectationSectionAst::Body(BodyAssertionAst::Object(object)) = &sections[4] else {
        panic!("expected object body assertion");
    };
    assert_eq!(object.mode, ObjectMatchModeAst::Exact);
    assert_eq!(object.fields.len(), 7);
    assert_eq!(
        object.fields[0].expected_type.as_ref().unwrap().value,
        AssertionTypeAst::Integer
    );
    assert_eq!(
        object.fields[1].expected_type.as_ref().unwrap().value,
        AssertionTypeAst::Number
    );
    assert_eq!(
        object.fields[2].expected_type.as_ref().unwrap().value,
        AssertionTypeAst::Boolean
    );
    assert_eq!(
        object.fields[3].expected_type.as_ref().unwrap().value,
        AssertionTypeAst::Array
    );
    assert_eq!(
        object.fields[4].expected_type.as_ref().unwrap().value,
        AssertionTypeAst::Null
    );
    assert_eq!(
        object.fields[5].expected_type.as_ref().unwrap().value,
        AssertionTypeAst::Object
    );
    let nested = object.fields[5].nested.as_ref().unwrap();
    assert_eq!(
        nested.fields[0].capture.as_ref().unwrap().value,
        "USER_NAME"
    );
    assert!(matches!(
        object.fields[6].expected_value,
        Some(ValueAst::Object(_))
    ));
}

#[test]
fn reports_suite_and_test_semantic_syntax_errors_without_dropping_nodes() {
    let result = parse_source(
        r#"
            garbage garbage
            core {}
            core {}
            pipeline "empty" {}
            test "missing both" {}
            test "wrong order" {
                expect { status = 200 }
                request GET "/"
            }
        "#,
    );
    let kinds = error_kinds(&result);
    assert!(matches!(kinds[0], ParserErrorKind::UnexpectedToken { .. }));
    assert!(kinds.contains(&&ParserErrorKind::DuplicateCore));
    assert!(kinds.contains(&&ParserErrorKind::EmptyPipeline));
    assert!(kinds.contains(&&ParserErrorKind::MissingRequest));
    assert!(kinds.contains(&&ParserErrorKind::MissingExpectation));
    assert!(kinds.contains(&&ParserErrorKind::RequestAfterExpectation));
    assert_eq!(result.ast.blocks.len(), 5);
}

#[test]
fn reports_malformed_request_and_recovers_to_later_sections() {
    let result = parse_source(
        r#"test "bad request" {
            request lower "/invalid-method"
            request GET "/with-options" {
                invalid tokens
                headers { "bad" = nope, "good" = "yes" }
                query nope
                body nope
            }
            expect { status = 200 }
        }"#,
    );
    assert!(result.has_errors());
    assert!(result.errors.iter().any(|error| {
        matches!(
            error.kind,
            ParserErrorKind::UnexpectedToken {
                expected: "uppercase HTTP method",
                ..
            }
        )
    }));
    let BlockAst::Test(test) = &result.ast.blocks[0] else {
        panic!("expected recovered test");
    };
    assert!(!test.requests.is_empty());
    assert!(!test.expectations.is_empty());
}

#[test]
fn reports_invalid_expectations_and_field_rules() {
    let result = parse_source(
        r#"test "bad expectation" {
            request GET "/"
            expect {
                nonsense
                status = "two hundred"
                body {
                    missing,
                    bad_type: nope,
                    comparison = status,
                    untyped_capture = 1 -> VALUE,
                    bad_capture: string -> "not-an-identifier"
                }
            }
        }"#,
    );
    assert!(result.has_errors());
    let kinds = error_kinds(&result);
    assert!(kinds.contains(&&ParserErrorKind::CaptureRequiresType));
    assert!(kinds.contains(&&ParserErrorKind::MissingFieldAssertion));
    assert!(
        kinds
            .iter()
            .filter(|kind| matches!(kind, ParserErrorKind::UnexpectedToken { .. }))
            .count()
            >= 5
    );
}

#[test]
fn reports_invalid_response_header_assertions() {
    let result = parse_source(
        r#"test "bad headers" {
            request GET "/"
            expect { headers {
                "wrong type": number,
                "missing operator" nope,
                "missing value" = nope
            } }
        }"#,
    );
    assert!(result.has_errors());
    assert!(
        result
            .errors
            .iter()
            .filter(|error| matches!(error.kind, ParserErrorKind::UnexpectedToken { .. }))
            .count()
            >= 3
    );
}

#[test]
fn recovers_from_invalid_object_entries_and_array_items() {
    let result = parse_source(
        r#"test "recovery" {
            request POST "/" {
                body {
                    = "bad",
                    missing_value = status,
                    array = [1 2, status, 3],
                    good = "kept"
                }
            }
            expect { status = 200 }
        }"#,
    );
    assert!(result.has_errors());
    let BlockAst::Test(test) = &result.ast.blocks[0] else {
        panic!("expected recovered test");
    };
    let RequestSectionAst::Body(body) = &test.requests[0].sections[0] else {
        panic!("expected recovered body");
    };
    assert!(
        body.value
            .entries
            .iter()
            .any(|entry| entry.key.value == "good")
    );
}

#[test]
fn missing_delimiters_at_eof_produce_eof_errors_and_partial_ast() {
    let result = parse_source(
        r#"pipeline "unfinished" { test "partial" { request GET "/" expect { body { value: string"#,
    );
    assert!(result.has_errors());
    assert!(
        result
            .errors
            .iter()
            .any(|error| matches!(error.kind, ParserErrorKind::UnexpectedEof { .. }))
    );
    assert_eq!(result.ast.blocks.len(), 1);
}

#[test]
fn parser_handles_a_token_slice_without_eof() {
    let tokens = [
        Token {
            kind: TokenKind::Identifier("unexpected".to_owned()),
            span: SourceSpan::new(4, 14),
        },
        Token {
            kind: TokenKind::IntegerLiteral(3),
            span: SourceSpan::new(15, 16),
        },
    ];
    let result = parse(&tokens);
    assert!(result.has_errors());
    assert!(result.ast.blocks.is_empty());
    assert_eq!(result.ast.span, SourceSpan::new(4, 4));
}

#[test]
fn nesting_limit_is_enforced_and_custom_limit_is_hard_capped() {
    let source = SourceText::new(
        "nesting.rttp",
        r#"test "nested" {
            request POST "/" { body { value = [[[1]]] } }
            expect { status = 200 }
        }"#,
    );
    let lexed = lex(&source);
    assert!(lexed.is_success());

    let limited = Parser::new(&lexed.tokens).with_max_nesting_depth(2).parse();
    assert!(limited.errors.iter().any(|error| {
        matches!(
            error.kind,
            ParserErrorKind::NestingLimitExceeded { limit: 2 }
        )
    }));

    let hard_capped = Parser::new(&lexed.tokens)
        .with_max_nesting_depth(usize::MAX)
        .parse();
    assert!(hard_capped.is_success(), "{:?}", hard_capped.errors);
}

#[test]
fn missing_request_brace_does_not_consume_following_expectation() {
    let result = parse_source(
        r#"test "boundary" {
            request GET "/" { body { key = "value" }
            expect { status = 200 }
        }"#,
    );
    assert!(result.has_errors());
    let BlockAst::Test(test) = &result.ast.blocks[0] else {
        panic!("expected test");
    };
    assert_eq!(test.requests.len(), 1);
    assert_eq!(test.expectations.len(), 1);
}

#[test]
fn parses_tests_inside_core_and_recovers_inside_test_containers() {
    let result = parse_source(
        r#"
        core {
            junk tokens
            test "setup" { request GET "/setup" expect { status = 200 } }
        }
        pipeline "flow" {
            junk tokens
            test "step" { request GET "/step" expect { status = 200 } }
        }
        "#,
    );
    assert!(result.has_errors());
    let BlockAst::Core(core) = &result.ast.blocks[0] else {
        panic!("expected core");
    };
    let BlockAst::Pipeline(pipeline) = &result.ast.blocks[1] else {
        panic!("expected pipeline");
    };
    assert_eq!(core.tests.len(), 1);
    assert_eq!(pipeline.tests.len(), 1);
}

#[test]
fn recovers_from_unknown_test_and_expectation_items() {
    let result = parse_source(
        r#"test "recovery" {
            unknown tokens before item
            request GET "/"
            expect {
                unknown tokens before section
                body { = 1, valid: string }
            }
        }"#,
    );
    assert!(result.has_errors());
    let BlockAst::Test(test) = &result.ast.blocks[0] else {
        panic!("expected test");
    };
    assert_eq!(test.requests.len(), 1);
    assert_eq!(test.expectations.len(), 1);
    let ExpectationSectionAst::Body(BodyAssertionAst::Object(object)) =
        &test.expectations[0].sections[0]
    else {
        panic!("expected recovered object assertion");
    };
    assert_eq!(object.fields.len(), 1);
}

#[test]
fn reports_missing_object_assertion_and_number_at_suite_level() {
    let result = parse_source(
        r#"1.5 test "body" {
            request GET "/"
            expect { body nope }
        }"#,
    );
    assert!(result.has_errors());
    assert!(result.errors.iter().any(|error| {
        matches!(
            &error.kind,
            ParserErrorKind::UnexpectedToken { found, .. } if found == "number `1.5`"
        )
    }));
    let BlockAst::Test(test) = &result.ast.blocks[0] else {
        panic!("expected recovered test");
    };
    assert!(matches!(
        test.expectations[0].sections[0],
        ExpectationSectionAst::Body(BodyAssertionAst::Object(_))
    ));
}

#[test]
fn zero_nesting_limit_skips_an_object_without_recursive_parsing() {
    let source = SourceText::new(
        "zero-depth.rttp",
        r#"test "limited" {
            request POST "/" { body { nested = { value = [1, { deep = true }] } } }
            expect { status = 200 }
        }"#,
    );
    let lexed = lex(&source);
    assert!(lexed.is_success());
    let result = Parser::new(&lexed.tokens).with_max_nesting_depth(0).parse();
    assert!(result.errors.iter().any(|error| {
        matches!(
            error.kind,
            ParserErrorKind::NestingLimitExceeded { limit: 0 }
        )
    }));

    let source = SourceText::new(
        "zero-depth-expectation.rttp",
        r#"test "limited" {
            request GET "/"
            expect { body { nested: object { value: string } } }
        }"#,
    );
    let lexed = lex(&source);
    let result = Parser::new(&lexed.tokens).with_max_nesting_depth(0).parse();
    assert!(result.errors.iter().any(|error| {
        matches!(
            error.kind,
            ParserErrorKind::NestingLimitExceeded { limit: 0 }
        )
    }));
}

#[test]
fn token_stream_without_eof_reports_missing_status_and_capture_values() {
    let source = SourceText::new(
        "no-eof.rttp",
        r#"test "partial" { request GET "/" expect { body { value: string ->"#,
    );
    let mut tokens = lex(&source).tokens;
    assert!(matches!(
        tokens.last().map(|token| &token.kind),
        Some(TokenKind::Eof)
    ));
    tokens.pop();
    let capture_result = parse(&tokens);
    assert!(
        capture_result
            .errors
            .iter()
            .any(|error| matches!(error.kind, ParserErrorKind::UnexpectedEof { .. }))
    );

    let source = SourceText::new(
        "no-eof-status.rttp",
        r#"test "partial" { request GET "/" expect { status ="#,
    );
    let mut tokens = lex(&source).tokens;
    tokens.pop();
    let status_result = parse(&tokens);
    assert!(
        status_result
            .errors
            .iter()
            .any(|error| matches!(error.kind, ParserErrorKind::UnexpectedEof { .. }))
    );
}
