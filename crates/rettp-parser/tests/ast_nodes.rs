use rettp_parser::{
    ArrayValueAst, AssertionTypeAst, BlockAst, BodyAssertionAst, CoreBlockAst, ExpectationAst,
    ExpectationSectionAst, FieldAssertionAst, HeaderValueEntryAst, HttpMethodAst,
    ObjectAssertionAst, ObjectMatchModeAst, ObjectValueAst, ObjectValueEntryAst, PipelineBlockAst,
    RequestAst, RequestBodyAst, RequestHeadersAst, RequestQueryAst, RequestSectionAst,
    ResponseHeaderAssertionAst, ResponseHeadersAst, SourceSpan, Spanned, StatusAssertionAst,
    SuiteAst, TestAst, ValueAst,
};

const fn span(start: usize, end: usize) -> SourceSpan {
    SourceSpan::new(start, end)
}

fn text(value: &str, source_span: SourceSpan) -> Spanned<String> {
    Spanned::new(value.to_owned(), source_span)
}

fn integer(value: i64, source_span: SourceSpan) -> ValueAst {
    ValueAst::Integer(Spanned::new(value, source_span))
}

fn empty_test(name: &str, source_span: SourceSpan) -> TestAst {
    TestAst {
        name: text(name, source_span),
        requests: Vec::new(),
        expectations: Vec::new(),
        span: source_span,
    }
}

#[test]
fn spanned_new_stores_owned_and_copy_values() {
    let number = Spanned::new(42, span(1, 3));
    assert_eq!(number.value, 42);
    assert_eq!(number.span, span(1, 3));

    let original = text("name", span(4, 10));
    let mut cloned = original.clone();
    cloned.value.push_str(" changed");
    assert_eq!(original.value, "name");
    assert_ne!(original, cloned);
}

#[test]
fn block_spans_cover_every_block_kind() {
    let core_span = span(0, 4);
    let pipeline_span = span(5, 20);
    let test_span = span(21, 30);

    let blocks = [
        BlockAst::Core(CoreBlockAst {
            tests: Vec::new(),
            span: core_span,
        }),
        BlockAst::Pipeline(PipelineBlockAst {
            name: text("flow", span(14, 18)),
            tests: Vec::new(),
            span: pipeline_span,
        }),
        BlockAst::Test(empty_test("standalone", test_span)),
    ];

    assert_eq!(
        blocks.map(|block| block.span()),
        [core_span, pipeline_span, test_span]
    );
}

#[test]
fn request_section_spans_cover_every_section_kind() {
    let headers_span = span(10, 20);
    let query_span = span(21, 30);
    let body_span = span(31, 40);
    let sections = [
        RequestSectionAst::Headers(RequestHeadersAst {
            entries: Vec::new(),
            span: headers_span,
        }),
        RequestSectionAst::Query(RequestQueryAst {
            entries: Vec::new(),
            span: query_span,
        }),
        RequestSectionAst::Body(RequestBodyAst {
            value: ObjectValueAst {
                entries: Vec::new(),
                span: span(36, 40),
            },
            span: body_span,
        }),
    ];

    assert_eq!(
        sections.map(|section| section.span()),
        [headers_span, query_span, body_span]
    );
}

#[test]
fn expectation_and_response_header_spans_cover_every_kind() {
    let status_span = span(0, 12);
    let headers_span = span(13, 50);
    let body_span = span(51, 61);
    let exists_span = span(20, 25);
    let exact_span = span(26, 35);
    let contains_span = span(36, 49);

    let response_headers = ResponseHeadersAst {
        entries: vec![
            ResponseHeaderAssertionAst::Exists {
                name: text("etag", span(20, 24)),
                type_span: span(24, 25),
                span: exists_span,
            },
            ResponseHeaderAssertionAst::Exact {
                name: text("content-type", span(26, 28)),
                expected: text("application/json", span(29, 35)),
                span: exact_span,
            },
            ResponseHeaderAssertionAst::Contains {
                name: text("cache-control", span(36, 38)),
                expected: text("no-cache", span(39, 49)),
                span: contains_span,
            },
        ],
        span: headers_span,
    };

    assert_eq!(response_headers.entries[0].span(), exists_span);
    assert_eq!(response_headers.entries[1].span(), exact_span);
    assert_eq!(response_headers.entries[2].span(), contains_span);

    let sections = [
        ExpectationSectionAst::Status(StatusAssertionAst {
            expected: Spanned::new(200, span(9, 12)),
            span: status_span,
        }),
        ExpectationSectionAst::Headers(response_headers),
        ExpectationSectionAst::Body(BodyAssertionAst::Empty { span: body_span }),
    ];
    assert_eq!(
        sections.map(|section| section.span()),
        [status_span, headers_span, body_span]
    );
}

#[test]
fn body_assertion_spans_cover_every_assertion_kind() {
    let spans = [span(0, 1), span(2, 3), span(4, 5), span(6, 7)];
    let assertions = [
        BodyAssertionAst::Empty { span: spans[0] },
        BodyAssertionAst::TextExact {
            expected: text("OK", spans[1]),
            span: spans[1],
        },
        BodyAssertionAst::TextContains {
            expected: text("created", spans[2]),
            span: spans[2],
        },
        BodyAssertionAst::Object(ObjectAssertionAst {
            mode: ObjectMatchModeAst::Partial,
            fields: Vec::new(),
            span: spans[3],
        }),
    ];

    assert_eq!(assertions.map(|assertion| assertion.span()), spans);
}

#[test]
fn value_spans_cover_every_value_kind_and_recursive_values() {
    let scalar_spans = [span(0, 1), span(2, 3), span(4, 5), span(6, 7), span(8, 9)];
    let scalars = [
        ValueAst::String(text("value", scalar_spans[0])),
        integer(7, scalar_spans[1]),
        ValueAst::Number(Spanned::new(2.5, scalar_spans[2])),
        ValueAst::Boolean(Spanned::new(true, scalar_spans[3])),
        ValueAst::Null(scalar_spans[4]),
    ];

    for (value, expected_span) in scalars.iter().zip(scalar_spans) {
        assert_eq!(value.span(), expected_span);
    }

    let nested_array_span = span(10, 30);
    let nested_object_span = span(31, 50);
    let array = ValueAst::Array(ArrayValueAst {
        items: vec![
            scalars[0].clone(),
            scalars[1].clone(),
            scalars[2].clone(),
            scalars[3].clone(),
            scalars[4].clone(),
            ValueAst::Array(ArrayValueAst {
                items: Vec::new(),
                span: span(20, 22),
            }),
            ValueAst::Object(ObjectValueAst {
                entries: Vec::new(),
                span: span(23, 25),
            }),
        ],
        span: nested_array_span,
    });
    let object = ValueAst::Object(ObjectValueAst {
        entries: vec![ObjectValueEntryAst {
            key: text("items", span(32, 37)),
            value: array.clone(),
            span: span(32, 49),
        }],
        span: nested_object_span,
    });

    assert_eq!(array.span(), nested_array_span);
    assert_eq!(object.span(), nested_object_span);
    assert_ne!(array.span(), scalars[0].span());
}

#[test]
fn field_assertions_represent_all_supported_combinations() {
    let string_type = Spanned::new(AssertionTypeAst::String, span(10, 16));
    let capture = text("ACCESS_TOKEN", span(80, 92));
    let nested = ObjectAssertionAst {
        mode: ObjectMatchModeAst::Partial,
        fields: Vec::new(),
        span: span(50, 60),
    };

    let fields = [
        FieldAssertionAst {
            name: text("typed", span(0, 5)),
            expected_type: Some(string_type.clone()),
            expected_value: None,
            nested: None,
            capture: None,
            span: span(0, 16),
        },
        FieldAssertionAst {
            name: text("compared", span(17, 25)),
            expected_type: None,
            expected_value: Some(integer(1, span(28, 29))),
            nested: None,
            capture: None,
            span: span(17, 29),
        },
        FieldAssertionAst {
            name: text("both", span(30, 34)),
            expected_type: Some(Spanned::new(AssertionTypeAst::Integer, span(36, 43))),
            expected_value: Some(integer(2, span(46, 47))),
            nested: None,
            capture: None,
            span: span(30, 47),
        },
        FieldAssertionAst {
            name: text("nested", span(48, 54)),
            expected_type: None,
            expected_value: None,
            nested: Some(nested.clone()),
            capture: None,
            span: span(48, 60),
        },
        FieldAssertionAst {
            name: text("token", span(61, 66)),
            expected_type: Some(string_type.clone()),
            expected_value: None,
            nested: None,
            capture: Some(capture.clone()),
            span: span(61, 92),
        },
        FieldAssertionAst {
            name: text("combined", span(93, 101)),
            expected_type: Some(string_type),
            expected_value: Some(ValueAst::String(text("ok", span(104, 108)))),
            nested: Some(nested),
            capture: Some(capture),
            span: span(93, 120),
        },
    ];

    assert!(fields[0].expected_type.is_some());
    assert!(fields[1].expected_value.is_some());
    assert!(fields[2].expected_type.is_some() && fields[2].expected_value.is_some());
    assert!(fields[3].nested.is_some());
    assert!(fields[4].capture.is_some());
    assert!(fields[5].expected_type.is_some());
    assert!(fields[5].expected_value.is_some());
    assert!(fields[5].nested.is_some());
    assert!(fields[5].capture.is_some());
}

#[test]
fn enum_variants_are_copyable_comparable_and_debuggable() {
    let methods = [
        HttpMethodAst::Get,
        HttpMethodAst::Post,
        HttpMethodAst::Put,
        HttpMethodAst::Patch,
        HttpMethodAst::Delete,
        HttpMethodAst::Head,
        HttpMethodAst::Options,
    ];
    let types = [
        AssertionTypeAst::String,
        AssertionTypeAst::Integer,
        AssertionTypeAst::Number,
        AssertionTypeAst::Object,
        AssertionTypeAst::Array,
        AssertionTypeAst::Boolean,
        AssertionTypeAst::Null,
    ];
    let modes = [ObjectMatchModeAst::Partial, ObjectMatchModeAst::Exact];

    assert_eq!(methods, methods.clone());
    assert_ne!(methods[0], methods[1]);
    assert_eq!(types, types.clone());
    assert_ne!(types[0], types[1]);
    assert_eq!(modes, modes.clone());
    assert_ne!(modes[0], modes[1]);
    assert_eq!(format!("{:?}", methods[6]), "Options");
    assert_eq!(format!("{:?}", types[6]), "Null");
    assert_eq!(format!("{:?}", modes[1]), "Exact");
}

#[test]
fn complete_suite_preserves_order_duplicates_and_clone_independence() {
    let duplicate_entry = HeaderValueEntryAst {
        name: text("x-id", span(30, 34)),
        value: integer(1, span(35, 36)),
        span: span(30, 36),
    };
    let request = RequestAst {
        method: Spanned::new(HttpMethodAst::Post, span(20, 24)),
        path: text("/items", span(25, 31)),
        sections: vec![
            RequestSectionAst::Headers(RequestHeadersAst {
                entries: vec![duplicate_entry.clone(), duplicate_entry],
                span: span(30, 40),
            }),
            RequestSectionAst::Query(RequestQueryAst {
                entries: vec![ObjectValueEntryAst {
                    key: text("page", span(41, 45)),
                    value: integer(1, span(46, 47)),
                    span: span(41, 47),
                }],
                span: span(41, 48),
            }),
            RequestSectionAst::Body(RequestBodyAst {
                value: ObjectValueAst {
                    entries: vec![ObjectValueEntryAst {
                        key: text("name", span(49, 53)),
                        value: ValueAst::String(text("first", span(54, 61))),
                        span: span(49, 61),
                    }],
                    span: span(48, 62),
                },
                span: span(48, 62),
            }),
        ],
        span: span(20, 62),
    };
    let expectation = ExpectationAst {
        sections: vec![ExpectationSectionAst::Status(StatusAssertionAst {
            expected: Spanned::new(200, span(70, 73)),
            span: span(63, 73),
        })],
        span: span(63, 74),
    };
    let test = TestAst {
        name: text("create", span(10, 16)),
        requests: vec![request.clone(), request],
        expectations: vec![expectation.clone(), expectation],
        span: span(10, 74),
    };
    let suite = SuiteAst {
        blocks: vec![
            BlockAst::Test(test.clone()),
            BlockAst::Core(CoreBlockAst {
                tests: Vec::new(),
                span: span(75, 81),
            }),
            BlockAst::Pipeline(PipelineBlockAst {
                name: text("flow", span(82, 86)),
                tests: vec![test.clone()],
                span: span(82, 100),
            }),
            BlockAst::Test(test),
        ],
        span: span(0, 100),
    };

    let mut cloned = suite.clone();
    let BlockAst::Test(first_test) = &mut cloned.blocks[0] else {
        panic!("first block must remain a test");
    };
    first_test.name.value.push_str(" changed");

    assert_ne!(suite, cloned);
    let BlockAst::Test(original_test) = &suite.blocks[0] else {
        panic!("first block must be a test");
    };
    assert_eq!(original_test.name.value, "create");
    assert_eq!(original_test.requests.len(), 2);
    assert_eq!(original_test.expectations.len(), 2);
    let RequestSectionAst::Headers(headers) = &original_test.requests[0].sections[0] else {
        panic!("first request section must be headers");
    };
    assert_eq!(headers.entries.len(), 2);
    assert!(matches!(suite.blocks[1], BlockAst::Core(_)));
    assert!(matches!(suite.blocks[2], BlockAst::Pipeline(_)));
    assert!(matches!(suite.blocks[3], BlockAst::Test(_)));
}
