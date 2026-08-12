use rettp_domain::{
    BodyAssertion, ExpectedType, HeaderAssertion, HttpMethod, ObjectMatchMode, RequestBody,
    SuiteBlock, TextAssertion, Value, VariableName,
};
use rettp_parser::{
    ArrayValueAst, AssertionTypeAst, BlockAst, BodyAssertionAst, DEFAULT_MAX_SEMANTIC_DEPTH,
    ExpectationAst, FieldAssertionAst, HARD_MAX_SEMANTIC_DEPTH, HttpMethodAst, ObjectAssertionAst,
    ObjectMatchModeAst, ObjectValueAst, ObjectValueEntryAst, RequestAst, SourceSpan, SourceText,
    Spanned, SuiteAst, TestAst, ValidationContext, ValidationErrorKind, ValueAst, lex, parse,
    validate_and_convert,
};

fn ast(input: &str) -> SuiteAst {
    let source = SourceText::new("semantic-test.rttp", input);
    let lexed = lex(&source);
    assert!(lexed.is_success(), "lexer errors: {:?}", lexed.errors);
    parse(&lexed.tokens).ast
}

fn validate(input: &str) -> rettp_parser::ValidationResult {
    validate_and_convert(&ast(input), &ValidationContext::new())
}

fn kinds(result: &rettp_parser::ValidationResult) -> Vec<&ValidationErrorKind> {
    result.errors.iter().map(|error| &error.kind).collect()
}

fn has_kind(
    result: &rettp_parser::ValidationResult,
    predicate: impl Fn(&ValidationErrorKind) -> bool,
) -> bool {
    result.errors.iter().any(|error| predicate(&error.kind))
}

fn valid_test(name: &str, method: HttpMethodAst) -> TestAst {
    let span = SourceSpan::new(0, 1);
    TestAst {
        name: Spanned::new(name.to_owned(), span),
        requests: vec![RequestAst {
            method: Spanned::new(method, span),
            path: Spanned::new("/".to_owned(), span),
            sections: Vec::new(),
            span,
        }],
        expectations: vec![ExpectationAst {
            sections: Vec::new(),
            span,
        }],
        span,
    }
}

#[test]
fn converts_a_complete_suite_and_all_domain_variants() {
    let source = r#"
        core {
            test "bootstrap" {
                request POST "/session" {
                    headers {
                        "X-Direct" = "direct-value",
                        "X-Variable" = "${SEED}",
                        "X-Mixed" = "before ${SEED} after",
                        "X-Integer" = 1,
                        "X-Number" = 1.5,
                        "X-Boolean" = true,
                        "X-Null" = null,
                        "X-Array" = ["x", 2, 2.5, false, null],
                        "X-Object" = { nested = "value" }
                    }
                    query {
                        text = "${SEED}", integer = -2, number = 3.5,
                        boolean = false, nothing = null,
                        array = [1, "two"], object = { ok = true }
                    }
                    body {
                        text = "payload", integer = 3, number = 4.5,
                        boolean = true, nothing = null,
                        array = [1, 2.5], object = { key = "value" }
                    }
                }
                expect {
                    status = 201
                    headers {
                        "X-Exists": string,
                        "X-Exact" = "${SEED}",
                        "X-Contains" contains "prefix ${SEED}"
                    }
                    body exact {
                        string_value: string = "ok",
                        integer_value: integer = 1,
                        number_integer: number = 2,
                        number_float: number = 2.5,
                        boolean_value: boolean = true,
                        null_value: null = null,
                        array_value: array = [1],
                        object_value: object = { inside = true },
                        nested: object { child: string -> CORE_VALUE }
                    }
                }
            }
        }
        pipeline "flow" {
            test "put" {
                request PUT "/${CORE_VALUE}" { body { value = 1 } }
                expect { body = "exact ${SEED}" }
            }
            test "patch" {
                request PATCH "/patch"
                expect { body contains "contains ${SEED}" }
            }
            test "delete" { request DELETE "/delete" expect { body empty } }
        }
        test "get" { request GET "/get" expect { status = 100 } }
        test "head" { request HEAD "/head" expect { status = 599 } }
        test "options" { request OPTIONS "/options" expect {} }
    "#;
    let context = ValidationContext::new()
        .with_predefined_variable(VariableName::new("SEED").expect("test variable must be valid"));
    let result = validate_and_convert(&ast(source), &context);
    assert!(result.is_success(), "{:?}", result.errors);
    assert!(!result.has_errors());

    let suite = result
        .suite
        .expect("successful validation converts the AST");
    assert_eq!(suite.blocks.len(), 5);
    let SuiteBlock::Core(core) = &suite.blocks[0] else {
        panic!("expected core block");
    };
    let request = &core.tests[0].request;
    assert_eq!(request.method, HttpMethod::POST);
    assert_eq!(request.path.as_str(), "/session");
    assert_eq!(request.headers.len(), 9);
    assert!(matches!(request.headers["X-Integer"], Value::Integer(1)));
    assert!(matches!(request.headers["X-Number"], Value::Number(1.5)));
    assert!(matches!(request.headers["X-Boolean"], Value::Boolean(true)));
    assert!(matches!(request.headers["X-Null"], Value::Null));
    assert!(matches!(request.headers["X-Array"], Value::Array(_)));
    assert!(matches!(request.headers["X-Object"], Value::Object(_)));
    let Value::String(mixed) = &request.headers["X-Mixed"] else {
        panic!("mixed header must remain an interpolated string");
    };
    assert_eq!(mixed.as_str(), "before ${SEED} after");
    assert_eq!(request.query.len(), 7);
    let Some(RequestBody::Json(Value::Object(body))) = &request.body else {
        panic!("expected JSON object request body");
    };
    assert_eq!(body.len(), 7);

    let expectation = &core.tests[0].expectation;
    assert_eq!(expectation.status, Some(201));
    assert!(matches!(
        expectation.headers["X-Exists"],
        HeaderAssertion::Exists
    ));
    let HeaderAssertion::Exact(value) = &expectation.headers["X-Exact"] else {
        panic!("expected exact header assertion");
    };
    assert_eq!(value.as_str(), "${SEED}");
    assert!(matches!(
        expectation.headers["X-Contains"],
        HeaderAssertion::Contains(_)
    ));
    let Some(BodyAssertion::Json(object)) = &expectation.body else {
        panic!("expected JSON body assertion");
    };
    assert_eq!(object.mode, ObjectMatchMode::Exact);
    assert_eq!(
        object.fields["string_value"].expected_type,
        ExpectedType::String
    );
    assert_eq!(
        object.fields["integer_value"].expected_type,
        ExpectedType::Integer
    );
    assert_eq!(
        object.fields["number_integer"].expected_type,
        ExpectedType::Number
    );
    assert_eq!(
        object.fields["boolean_value"].expected_type,
        ExpectedType::Boolean
    );
    assert_eq!(
        object.fields["null_value"].expected_type,
        ExpectedType::Null
    );
    assert_eq!(
        object.fields["array_value"].expected_type,
        ExpectedType::Array
    );
    assert_eq!(
        object.fields["object_value"].expected_type,
        ExpectedType::Object
    );
    let nested = object.fields["nested"]
        .nested
        .as_ref()
        .expect("nested object must be converted");
    assert_eq!(nested.mode, ObjectMatchMode::Partial);
    assert_eq!(
        nested.fields["child"]
            .capture
            .as_ref()
            .expect("capture must be converted")
            .variable
            .as_str(),
        "CORE_VALUE"
    );

    let SuiteBlock::Pipeline(pipeline) = &suite.blocks[1] else {
        panic!("expected pipeline");
    };
    assert_eq!(pipeline.name, "flow");
    assert_eq!(pipeline.tests[0].request.method, HttpMethod::PUT);
    assert_eq!(pipeline.tests[1].request.method, HttpMethod::PATCH);
    assert_eq!(pipeline.tests[2].request.method, HttpMethod::DELETE);
    assert!(matches!(
        pipeline.tests[0].expectation.body,
        Some(BodyAssertion::Text(TextAssertion::Exact(_)))
    ));
    assert!(matches!(
        pipeline.tests[1].expectation.body,
        Some(BodyAssertion::Text(TextAssertion::Contains(_)))
    ));
    assert!(matches!(
        pipeline.tests[2].expectation.body,
        Some(BodyAssertion::Empty)
    ));
    let SuiteBlock::Test(get) = &suite.blocks[2] else {
        panic!("expected GET")
    };
    let SuiteBlock::Test(head) = &suite.blocks[3] else {
        panic!("expected HEAD")
    };
    let SuiteBlock::Test(options) = &suite.blocks[4] else {
        panic!("expected OPTIONS")
    };
    assert_eq!(get.request.method, HttpMethod::GET);
    assert_eq!(head.request.method, HttpMethod::HEAD);
    assert_eq!(options.request.method, HttpMethod::OPTIONS);
}

#[test]
fn infers_comparison_only_types_and_number_accepts_both_numeric_forms() {
    let result = validate(
        r#"test "inference" {
            request GET "/"
            expect { body {
                string = "x", integer = 1, number = 1.5,
                boolean = false, null_value = null,
                array = [], object = {},
                integer_as_number: number = 2,
                float_as_number: number = 2.5
            } }
        }"#,
    );
    assert!(result.is_success(), "{:?}", result.errors);
    let suite = result.suite.unwrap();
    let SuiteBlock::Test(test) = &suite.blocks[0] else {
        panic!("expected test")
    };
    let Some(BodyAssertion::Json(assertion)) = &test.expectation.body else {
        panic!("expected object assertion");
    };
    let expected = [
        ("string", ExpectedType::String),
        ("integer", ExpectedType::Integer),
        ("number", ExpectedType::Number),
        ("boolean", ExpectedType::Boolean),
        ("null_value", ExpectedType::Null),
        ("array", ExpectedType::Array),
        ("object", ExpectedType::Object),
        ("integer_as_number", ExpectedType::Number),
        ("float_as_number", ExpectedType::Number),
    ];
    for (name, kind) in expected {
        assert_eq!(assertion.fields[name].expected_type, kind);
    }
}

#[test]
fn reports_every_explicit_type_mismatch_without_partial_conversion() {
    let result = validate(
        r#"test "mismatch" {
            request GET "/"
            expect { body {
                string: string = 1,
                integer: integer = 1.5,
                number: number = "not-a-number",
                object: object = [],
                array: array = {},
                boolean: boolean = null,
                null_value: null = false
            } }
        }"#,
    );
    assert!(result.has_errors());
    assert!(!result.is_success());
    assert!(result.suite.is_none());
    assert_eq!(
        result
            .errors
            .iter()
            .filter(|error| matches!(error.kind, ValidationErrorKind::TypeValueMismatch { .. }))
            .count(),
        7
    );
}

#[test]
fn reports_duplicate_blocks_declarations_sections_and_named_entries() {
    let result = validate(
        r#"
        core {}
        core {}
        test "duplicates" {
            request POST "/" {
                headers { "X-Key" = "a", "x-key" = "b" }
                headers {}
                query { key = 1, key = 2 }
                query {}
                body { key = 1, key = 2 }
                body {}
            }
            request GET "/"
            expect {
                status = 200
                status = 201
                headers { "X-Key": string, "x-key" = "v" }
                headers {}
                body { key: string, key: string }
                body empty
            }
            expect {}
        }
        "#,
    );
    assert!(result.suite.is_none());
    use rettp_parser::DuplicateKind as D;
    for duplicate in [
        D::CoreBlock,
        D::Request,
        D::Expectation,
        D::RequestHeaders,
        D::RequestQuery,
        D::RequestBody,
        D::ResponseStatus,
        D::ResponseHeaders,
        D::ResponseBody,
    ] {
        assert!(
            has_kind(&result, |kind| matches!(kind,
                ValidationErrorKind::Duplicate { kind } if *kind == duplicate
            )),
            "missing duplicate {duplicate:?}: {:?}",
            result.errors
        );
    }
    for duplicate in [
        D::RequestHeader,
        D::ResponseHeader,
        D::QueryParameter,
        D::ObjectKey,
        D::AssertionField,
    ] {
        assert!(
            has_kind(&result, |kind| matches!(kind,
                ValidationErrorKind::DuplicateNamed { kind, .. } if *kind == duplicate
            )),
            "missing named duplicate {duplicate:?}: {:?}",
            result.errors
        );
    }
}

#[test]
fn validates_status_boundaries_body_methods_and_empty_textual_names() {
    let valid = validate(
        r#"
        test "lower" { request GET "/" expect { status = 100 } }
        test "upper" { request GET "/" expect { status = 599 } }
        test "post" { request POST "/" { body {} } expect {} }
        test "put" { request PUT "/" { body {} } expect {} }
        test "patch" { request PATCH "/" { body {} } expect {} }
        "#,
    );
    assert!(valid.is_success(), "{:?}", valid.errors);

    let invalid = validate(
        r#"
        pipeline "" {}
        test "" {
            request GET "" { body {} }
            expect { status = 99 status = 600 }
        }
        test "delete" { request DELETE "/" { body {} } expect {} }
        test "head" { request HEAD "/" { body {} } expect {} }
        test "options" { request OPTIONS "/" { body {} } expect {} }
        "#,
    );
    assert!(has_kind(&invalid, |kind| matches!(
        kind,
        ValidationErrorKind::EmptyPipelineName
    )));
    assert!(has_kind(&invalid, |kind| matches!(
        kind,
        ValidationErrorKind::EmptyPipeline
    )));
    assert!(has_kind(&invalid, |kind| matches!(
        kind,
        ValidationErrorKind::EmptyTestName
    )));
    assert!(has_kind(&invalid, |kind| matches!(
        kind,
        ValidationErrorKind::EmptyRequestPath
    )));
    assert_eq!(
        invalid
            .errors
            .iter()
            .filter(|error| matches!(error.kind, ValidationErrorKind::InvalidHttpStatus { .. }))
            .count(),
        2
    );
    assert_eq!(
        invalid
            .errors
            .iter()
            .filter(|error| matches!(
                error.kind,
                ValidationErrorKind::RequestBodyNotAllowed { .. }
            ))
            .count(),
        4
    );
}

#[test]
fn reports_missing_request_expectation_and_empty_map_names() {
    let span = SourceSpan::new(4, 8);
    let mut missing_both = valid_test("missing", HttpMethodAst::Get);
    missing_both.requests.clear();
    missing_both.expectations.clear();
    let source = r#"test "empty names" {
        request POST "/" {
            headers { "" = "x" }
            query { "" = 1 }
            body { "" = 1 }
        }
        expect { headers { "": string } body { "": string } }
    }"#;
    let mut parsed = ast(source);
    parsed.blocks.push(BlockAst::Test(missing_both));
    parsed.span = span;
    let result = validate_and_convert(&parsed, &ValidationContext::new());
    assert!(has_kind(&result, |kind| matches!(
        kind,
        ValidationErrorKind::InvalidRequestCount
    )));
    assert!(has_kind(&result, |kind| matches!(
        kind,
        ValidationErrorKind::InvalidExpectationCount
    )));
    assert_eq!(
        result
            .errors
            .iter()
            .filter(|error| matches!(error.kind, ValidationErrorKind::EmptyName { .. }))
            .count(),
        5
    );
}

#[test]
fn validates_capture_requirements_names_and_duplicates() {
    let result = validate(
        r#"test "captures" {
            request GET "/"
            expect { body {
                valid: string -> VALUE,
                duplicate: integer -> VALUE,
                no_type = 1 -> UNTYPED,
                invalid: string -> "bad-name",
                also_invalid: string -> ""
            } }
        }"#,
    );
    assert!(has_kind(&result, |kind| matches!(kind,
        ValidationErrorKind::DuplicateVariable { name } if name == "VALUE"
    )));
    assert!(has_kind(&result, |kind| matches!(kind,
        ValidationErrorKind::CaptureRequiresType { name } if name == "UNTYPED"
    )));
    assert_eq!(
        result
            .errors
            .iter()
            .filter(|error| matches!(error.kind, ValidationErrorKind::InvalidVariableName { .. }))
            .count(),
        2
    );

    let context = ValidationContext::new()
        .with_predefined_variable(VariableName::new("TAKEN").expect("test variable must be valid"));
    let predefined_duplicate = validate_and_convert(
        &ast(r#"test "capture" { request GET "/" expect { body { x: string -> TAKEN } } }"#),
        &context,
    );
    assert!(has_kind(&predefined_duplicate, |kind| matches!(kind,
        ValidationErrorKind::DuplicateVariable { name } if name == "TAKEN"
    )));
}

#[test]
fn accepts_direct_and_all_supported_interpolation_forms() {
    let context = ValidationContext::new().with_predefined_variables([
        VariableName::new("id").unwrap(),
        VariableName::new("first").unwrap(),
        VariableName::new("second").unwrap(),
    ]);
    let result = validate_and_convert(
        &ast(r#"test "interpolation" {
                request GET "/data/${id}/${first}-${second}" {
                    headers {
                        "direct" = "direct-value",
                        "only" = "${first}",
                        "mixed" = "something ${first}",
                        "multiple" = "${first}/${second}/${first}"
                    }
                    query { value = "prefix ${second}" }
                }
                expect {
                    headers {
                        "exact" = "${first}",
                        "contains" contains "x ${second}"
                    }
                    body = "${first} and ${second}"
                }
            }"#),
        &context,
    );
    assert!(result.is_success(), "{:?}", result.errors);
}

#[test]
fn rejects_malformed_invalid_and_undefined_interpolation() {
    let result = validate(
        r#"test "bad interpolation" {
            request GET "/${}" {
                headers {
                    "unterminated" = "${unterminated",
                    "invalid" = "${bad-name}",
                    "undefined" = "${missing}",
                    "nested" = "${outer ${inner}}"
                }
            }
            expect { body contains "${unknown}" }
        }"#,
    );
    assert!(has_kind(&result, |kind| matches!(
        kind,
        ValidationErrorKind::EmptyInterpolation
    )));
    assert!(has_kind(&result, |kind| matches!(
        kind,
        ValidationErrorKind::UnterminatedInterpolation
    )));
    assert!(has_kind(&result, |kind| matches!(kind,
        ValidationErrorKind::InvalidVariableName { name } if name == "bad-name"
    )));
    assert!(has_kind(&result, |kind| matches!(kind,
        ValidationErrorKind::InvalidVariableName { name } if name == "outer ${inner"
    )));
    assert!(has_kind(&result, |kind| matches!(kind,
        ValidationErrorKind::UndefinedVariable { name } if name == "missing"
    )));
    assert!(has_kind(&result, |kind| matches!(kind,
        ValidationErrorKind::UndefinedVariable { name } if name == "unknown"
    )));
}

#[test]
fn applies_core_pipeline_and_standalone_variable_scopes() {
    let source = r#"
        pipeline "before-core" {
            test "first" {
                request GET "/${CORE}"
                expect { body { id: integer -> STEP } }
            }
            test "second" { request GET "/${STEP}" expect {} }
        }
        test "standalone-core" { request GET "/${CORE}" expect {} }
        core {
            test "setup" {
                request GET "/"
                expect { body { token: string -> CORE } }
            }
        }
    "#;
    let valid = validate(source);
    assert!(valid.is_success(), "{:?}", valid.errors);

    let invalid = validate(
        r#"
        pipeline "one" {
            test "use before declaration" { request GET "/${LATER}" expect {} }
            test "declare" { request GET "/" expect { body { x: string -> LATER } } }
        }
        pipeline "two" { test "isolated" { request GET "/${LATER}" expect {} } }
        test "standalone one" {
            request GET "/"
            expect { body { x: string -> LOCAL } }
        }
        test "standalone two" { request GET "/${LOCAL}" expect {} }
        "#,
    );
    assert_eq!(
        invalid
            .errors
            .iter()
            .filter(|error| matches!(error.kind, ValidationErrorKind::UndefinedVariable { .. }))
            .count(),
        3
    );
}

#[test]
fn validates_nested_object_rules_and_missing_field_assertions() {
    let span = SourceSpan::new(0, 1);
    let field = |name: &str| FieldAssertionAst {
        name: Spanned::new(name.to_owned(), span),
        expected_type: None,
        expected_value: None,
        nested: None,
        capture: None,
        span,
    };
    let mut test = valid_test("defensive", HttpMethodAst::Get);
    test.expectations[0].sections = vec![rettp_parser::ExpectationSectionAst::Body(
        BodyAssertionAst::Object(ObjectAssertionAst {
            mode: ObjectMatchModeAst::Partial,
            fields: vec![
                field("missing"),
                FieldAssertionAst {
                    name: Spanned::new("wrong_type".to_owned(), span),
                    expected_type: Some(Spanned::new(AssertionTypeAst::String, span)),
                    expected_value: None,
                    nested: Some(ObjectAssertionAst {
                        mode: ObjectMatchModeAst::Partial,
                        fields: Vec::new(),
                        span,
                    }),
                    capture: None,
                    span,
                },
                FieldAssertionAst {
                    name: Spanned::new("exact_nested".to_owned(), span),
                    expected_type: Some(Spanned::new(AssertionTypeAst::Object, span)),
                    expected_value: None,
                    nested: Some(ObjectAssertionAst {
                        mode: ObjectMatchModeAst::Exact,
                        fields: Vec::new(),
                        span,
                    }),
                    capture: None,
                    span,
                },
            ],
            span,
        }),
    )];
    let result = validate_and_convert(
        &SuiteAst {
            blocks: vec![BlockAst::Test(test)],
            span,
        },
        &ValidationContext::new(),
    );
    assert!(has_kind(&result, |kind| matches!(kind,
        ValidationErrorKind::MissingFieldAssertion { field } if field == "missing"
    )));
    assert!(has_kind(&result, |kind| matches!(kind,
        ValidationErrorKind::NestedAssertionRequiresObject { field } if field == "wrong_type"
    )));
    assert!(has_kind(&result, |kind| matches!(kind,
        ValidationErrorKind::NestedAssertionMustBePartial { field } if field == "exact_nested"
    )));
}

fn nested_value(depth: usize, span: SourceSpan) -> ValueAst {
    let mut value = ValueAst::Null(span);
    for level in 0..depth {
        value = if level % 2 == 0 {
            ValueAst::Array(ArrayValueAst {
                items: vec![value],
                span,
            })
        } else {
            ValueAst::Object(ObjectValueAst {
                entries: vec![ObjectValueEntryAst {
                    key: Spanned::new(format!("level_{level}"), span),
                    value,
                    span,
                }],
                span,
            })
        };
    }
    value
}

fn suite_with_nested_header(depth: usize) -> SuiteAst {
    let span = SourceSpan::new(0, 1);
    let mut test = valid_test("depth", HttpMethodAst::Get);
    test.requests[0].sections = vec![rettp_parser::RequestSectionAst::Headers(
        rettp_parser::RequestHeadersAst {
            entries: vec![rettp_parser::HeaderValueEntryAst {
                name: Spanned::new("X-Value".to_owned(), span),
                value: nested_value(depth, span),
                span,
            }],
            span,
        },
    )];
    SuiteAst {
        blocks: vec![BlockAst::Test(test)],
        span,
    }
}

#[test]
fn enforces_default_custom_and_hard_semantic_depth_limits() {
    assert_eq!(
        ValidationContext::default().max_depth(),
        DEFAULT_MAX_SEMANTIC_DEPTH
    );
    assert_eq!(
        ValidationContext::new()
            .with_max_depth(usize::MAX)
            .max_depth(),
        HARD_MAX_SEMANTIC_DEPTH
    );

    let shallow = validate_and_convert(
        &suite_with_nested_header(2),
        &ValidationContext::new().with_max_depth(4),
    );
    assert!(shallow.is_success(), "{:?}", shallow.errors);

    let custom = validate_and_convert(
        &suite_with_nested_header(5),
        &ValidationContext::new().with_max_depth(3),
    );
    assert!(has_kind(&custom, |kind| matches!(
        kind,
        ValidationErrorKind::NestingLimitExceeded { limit: 3 }
    )));
    assert!(custom.suite.is_none());

    let assertion_root = validate_and_convert(
        &ast(r#"test "depth" { request GET "/" expect { body { value: string } } }"#),
        &ValidationContext::new().with_max_depth(0),
    );
    assert!(has_kind(&assertion_root, |kind| matches!(
        kind,
        ValidationErrorKind::NestingLimitExceeded { limit: 0 }
    )));

    let default = validate_and_convert(
        &suite_with_nested_header(DEFAULT_MAX_SEMANTIC_DEPTH + 1),
        &ValidationContext::new(),
    );
    assert!(has_kind(&default, |kind| matches!(kind,
        ValidationErrorKind::NestingLimitExceeded { limit } if *limit == DEFAULT_MAX_SEMANTIC_DEPTH
    )));
}

#[test]
fn validation_errors_expose_spans_display_and_error_traits() {
    let result = validate(r#"test "bad" { request GET "/${missing}" expect {} }"#);
    let error = result.errors.first().expect("expected validation error");
    assert!(error.span.end > error.span.start);
    assert_eq!(error.to_string(), "undefined variable `missing`");
    let cloned = error.clone();
    assert_eq!(cloned, *error);
    let as_error: &dyn std::error::Error = error;
    assert!(as_error.source().is_none());

    let labels = [
        rettp_parser::DuplicateKind::CoreBlock,
        rettp_parser::DuplicateKind::Request,
        rettp_parser::DuplicateKind::Expectation,
        rettp_parser::DuplicateKind::RequestHeaders,
        rettp_parser::DuplicateKind::RequestQuery,
        rettp_parser::DuplicateKind::RequestBody,
        rettp_parser::DuplicateKind::ResponseStatus,
        rettp_parser::DuplicateKind::ResponseHeaders,
        rettp_parser::DuplicateKind::ResponseBody,
        rettp_parser::DuplicateKind::RequestHeader,
        rettp_parser::DuplicateKind::ResponseHeader,
        rettp_parser::DuplicateKind::QueryParameter,
        rettp_parser::DuplicateKind::ObjectKey,
        rettp_parser::DuplicateKind::AssertionField,
    ];
    for label in labels {
        assert!(!label.to_string().is_empty());
    }
    assert!(!kinds(&result).is_empty());
}

#[test]
fn duplicate_core_is_validated_in_an_isolated_scope() {
    let result = validate(
        r#"
        core { test "first" { request GET "/" expect { body { x: string -> SHARED } } } }
        core { test "second" { request GET "/${SHARED}" expect {} } }
        "#,
    );
    assert!(has_kind(&result, |kind| matches!(
        kind,
        ValidationErrorKind::Duplicate {
            kind: rettp_parser::DuplicateKind::CoreBlock
        }
    )));
    assert!(has_kind(&result, |kind| matches!(kind,
        ValidationErrorKind::UndefinedVariable { name } if name == "SHARED"
    )));
}
