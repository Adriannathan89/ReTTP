use indexmap::IndexMap;
use rettp_domain::*;

fn request() -> HttpRequestSpec {
    HttpRequestSpec::new(HttpMethod::GET, "/health")
}

fn case(name: &str) -> TestCase {
    TestCase::new(name, request(), ResponseExpectation::default())
}

#[test]
fn interpolated_strings_and_values_report_their_contents_and_types() {
    let plain = InterpolatedString::new("hello");
    let interpolated = InterpolatedString::from("hello ${user}");

    assert_eq!(plain.as_str(), "hello");
    assert!(!plain.contains_interpolation());
    assert!(interpolated.contains_interpolation());
    assert_eq!(
        InterpolatedString::from(String::from("owned")).as_str(),
        "owned"
    );

    let values = [
        (Value::String(plain), "string"),
        (Value::Integer(1), "integer"),
        (Value::Number(1.5), "number"),
        (Value::Boolean(true), "boolean"),
        (Value::Null, "null"),
        (Value::Array(vec![]), "array"),
        (Value::Object(IndexMap::new()), "object"),
    ];
    for (value, expected_type) in values {
        assert_eq!(value.type_name(), expected_type);
    }
}

#[test]
fn values_convert_from_interpolated_and_plain_strings() {
    let direct = Value::from(InterpolatedString::new("direct-value"));
    let interpolation_only = Value::from(String::from("${interpolated_string}"));
    let mixed = Value::from("something ${interpolated_string}");

    assert_eq!(
        direct,
        Value::String(InterpolatedString::new("direct-value"))
    );
    assert_eq!(
        interpolation_only,
        Value::String(InterpolatedString::new("${interpolated_string}"))
    );
    assert_eq!(
        mixed,
        Value::String(InterpolatedString::new("something ${interpolated_string}"))
    );
}

#[test]
fn variable_names_validate_all_rules_and_display() {
    let valid = VariableName::new("_nam3").unwrap();
    assert_eq!(valid.as_str(), "_nam3");
    assert_eq!(valid.to_string(), "_nam3");
    assert_eq!(Capture::new(valid.clone()).variable, valid);
    assert_eq!(VariableName::new("name_1").unwrap().as_str(), "name_1");

    assert_eq!(VariableName::new(""), Err(DomainError::EmptyVariableName));
    assert_eq!(
        VariableName::new("3name"),
        Err(DomainError::InvalidVariableName {
            name: "3name".into()
        })
    );
    assert_eq!(
        VariableName::new("bad-name"),
        Err(DomainError::InvalidVariableName {
            name: "bad-name".into()
        })
    );
}

#[test]
fn request_methods_and_builder_preserve_every_setting() {
    let methods = [
        (HttpMethod::GET, "GET", false),
        (HttpMethod::POST, "POST", true),
        (HttpMethod::PUT, "PUT", true),
        (HttpMethod::PATCH, "PATCH", true),
        (HttpMethod::DELETE, "DELETE", false),
        (HttpMethod::HEAD, "HEAD", false),
        (HttpMethod::OPTIONS, "OPTIONS", false),
    ];
    for (method, name, allows_body) in methods {
        assert_eq!(method.as_str(), name);
        assert_eq!(method.allows_body(), allows_body);
    }

    let spec = HttpRequestSpec::new(HttpMethod::POST, "/users/${id}")
        .with_header("Accept", "application/json")
        .with_query_param("active", Value::Boolean(true))
        .with_body(RequestBody::Json(Value::Integer(42)));
    assert_eq!(spec.path.as_str(), "/users/${id}");
    assert_eq!(
        spec.headers["Accept"],
        Value::String(InterpolatedString::new("application/json"))
    );
    assert_eq!(spec.query["active"], Value::Boolean(true));
    assert_eq!(spec.body, Some(RequestBody::Json(Value::Integer(42))));
    assert_eq!(spec.timeout_ms, None);

    let bodies = [
        RequestBody::Text(InterpolatedString::from("text")),
        RequestBody::FormData(IndexMap::new()),
        RequestBody::Binary(vec![1, 2]),
    ];
    for body in bodies {
        assert_eq!(
            HttpRequestSpec::new(HttpMethod::POST, "/")
                .with_body(body.clone())
                .body,
            Some(body)
        );
    }
}

#[test]
fn request_headers_accept_values_and_preserve_declaration_order() {
    let spec = HttpRequestSpec::new(HttpMethod::GET, "/data/${id}")
        .with_header("X-Direct", "direct-value")
        .with_header("X-Variable", "${interpolated_string}")
        .with_header("X-Mixed", "something ${interpolated_string}")
        .with_header("X-Retry-Count", Value::Integer(3))
        .with_header("X-Direct", "replacement-value");

    assert_eq!(
        spec.headers.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["X-Direct", "X-Variable", "X-Mixed", "X-Retry-Count"]
    );
    assert_eq!(
        spec.headers["X-Direct"],
        Value::String(InterpolatedString::new("replacement-value"))
    );
    assert_eq!(
        spec.headers["X-Variable"],
        Value::String(InterpolatedString::new("${interpolated_string}"))
    );
    assert_eq!(
        spec.headers["X-Mixed"],
        Value::String(InterpolatedString::new("something ${interpolated_string}"))
    );
    assert_eq!(spec.headers["X-Retry-Count"], Value::Integer(3));
}

#[test]
fn assertions_build_and_replace_fields_by_name() {
    let capture = Capture::new(VariableName::new("token").unwrap());
    let nested = ObjectAssertion::exact();
    let assertion = FieldAssertion::type_and_value("id", ExpectedType::Integer, Value::Integer(7))
        .with_capture(capture.clone())
        .with_nested(nested.clone());
    assert_eq!(assertion.expected_value, Some(Value::Integer(7)));
    assert_eq!(assertion.capture, Some(capture));
    assert_eq!(assertion.nested, Some(nested));

    let types = [
        (ExpectedType::String, "string"),
        (ExpectedType::Boolean, "boolean"),
        (ExpectedType::Integer, "integer"),
        (ExpectedType::Number, "number"),
        (ExpectedType::Object, "object"),
        (ExpectedType::Array, "array"),
        (ExpectedType::Null, "null"),
    ];
    for (expected_type, name) in types {
        assert_eq!(expected_type.as_str(), name);
    }

    let mut partial = ObjectAssertion::partial();
    partial.insert(FieldAssertion::type_only("name", ExpectedType::String));
    partial.insert(FieldAssertion::type_and_value(
        "name",
        ExpectedType::String,
        Value::String("Ada".into()),
    ));
    assert_eq!(partial.mode, ObjectMatchMode::Partial);
    assert_eq!(partial.fields.len(), 1);
    assert_eq!(
        partial.fields["name"].expected_value,
        Some(Value::String("Ada".into()))
    );
    assert_eq!(ObjectAssertion::exact().mode, ObjectMatchMode::Exact);
}

#[test]
fn expectations_cases_blocks_and_suites_keep_domain_structure() {
    let expectation = ResponseExpectation {
        status: Some(201),
        headers: IndexMap::from([
            ("X-Request".into(), HeaderAssertion::Exists),
            (
                "Content-Type".into(),
                HeaderAssertion::Exact("application/json".into()),
            ),
            ("X-Trace".into(), HeaderAssertion::Contains("trace".into())),
        ]),
        body: Some(BodyAssertion::Json(ObjectAssertion::partial())),
    };
    let test = TestCase::new("creates user", request(), expectation.clone());
    assert_eq!(test.expectation, expectation);
    assert_eq!(
        BodyAssertion::Text(TextAssertion::Exact("ok".into())),
        BodyAssertion::Text(TextAssertion::Exact("ok".into()))
    );
    assert_eq!(
        BodyAssertion::Text(TextAssertion::Contains("ok".into())),
        BodyAssertion::Text(TextAssertion::Contains("ok".into()))
    );
    assert_eq!(BodyAssertion::Empty, BodyAssertion::Empty);

    let core = CoreBlock::new(vec![test.clone()]);
    assert!(!core.is_empty());
    assert!(CoreBlock::new(vec![]).is_empty());
    let pipeline = PipelineBlock::new("deploy", vec![test.clone()]);
    assert_eq!(pipeline.name, "deploy");
    assert!(!pipeline.is_empty());
    assert!(PipelineBlock::new("empty", vec![]).is_empty());

    let suite = TestSuite::new(vec![
        SuiteBlock::Core(core.clone()),
        SuiteBlock::Pipeline(pipeline.clone()),
        SuiteBlock::Test(test.clone()),
    ]);
    assert_eq!(suite.name, None);
    assert_eq!(suite.len(), 3);
    assert!(!suite.is_empty());
    let named = TestSuite::named("API", vec![]);
    assert_eq!(named.name.as_deref(), Some("API"));
    assert_eq!(named.len(), 0);
    assert!(named.is_empty());
    assert!(TestSuite::default().blocks.is_empty());
}

#[test]
fn results_and_errors_represent_all_outcomes() {
    let passed = TestResult::passed("ok", 12);
    assert_eq!(passed.status, ExecutionStatus::Passed);
    assert!(passed.failures.is_empty());
    assert_eq!(passed.error, None);
    let skipped = TestResult::skipped("later", "dependency failed");
    assert_eq!(skipped.status, ExecutionStatus::Skipped);
    assert_eq!(
        skipped.error.unwrap().kind,
        ExecutionErrorKind::DependencyFailed
    );
    let aborted = TestResult::aborted("stop", 7, "internal failure");
    assert_eq!(aborted.status, ExecutionStatus::Aborted);
    assert_eq!(aborted.duration_ms, 7);
    assert_eq!(aborted.error.unwrap().kind, ExecutionErrorKind::Internal);
    let failure = AssertionFailure {
        path: "$.id".into(),
        kind: AssertionFailureKind::ValueMismatch,
        expected: Some("1".into()),
        actual: Some("2".into()),
        message: "different".into(),
    };
    let failed = TestResult::failed("bad", 3, vec![failure.clone()], None);
    assert_eq!(failed.status, ExecutionStatus::Failed);
    assert_eq!(failed.failures, vec![failure]);

    let statuses = [
        ExecutionStatus::Passed,
        ExecutionStatus::Failed,
        ExecutionStatus::Skipped,
        ExecutionStatus::Aborted,
    ];
    assert_eq!(statuses.len(), 4);
    let failure_kinds = [
        AssertionFailureKind::MissingField,
        AssertionFailureKind::TypeMismatch,
        AssertionFailureKind::ValueMismatch,
        AssertionFailureKind::UnexpectedField,
        AssertionFailureKind::StatusMismatch,
        AssertionFailureKind::HeaderMismatch,
        AssertionFailureKind::InvalidBody,
    ];
    assert_eq!(failure_kinds.len(), 7);
    let error_kinds = [
        ExecutionErrorKind::InvalidRequest,
        ExecutionErrorKind::Connection,
        ExecutionErrorKind::Timeout,
        ExecutionErrorKind::InvalidResponse,
        ExecutionErrorKind::VariableResolution,
        ExecutionErrorKind::DependencyFailed,
        ExecutionErrorKind::Internal,
    ];
    assert_eq!(error_kinds.len(), 7);
    assert_eq!(
        BlockResult::Core(CoreResult {
            status: ExecutionStatus::Passed,
            duration_ms: 12,
            tests: vec![passed]
        }),
        BlockResult::Core(CoreResult {
            status: ExecutionStatus::Passed,
            duration_ms: 12,
            tests: vec![TestResult::passed("ok", 12)]
        })
    );
    assert_eq!(
        BlockResult::Pipeline(PipelineResult {
            name: "p".into(),
            status: ExecutionStatus::Skipped,
            duration_ms: 0,
            tests: vec![]
        }),
        BlockResult::Pipeline(PipelineResult {
            name: "p".into(),
            status: ExecutionStatus::Skipped,
            duration_ms: 0,
            tests: vec![]
        })
    );
    assert_eq!(BlockResult::Test(failed.clone()), BlockResult::Test(failed));

    assert_eq!(
        DomainError::EmptyVariableName.to_string(),
        "variable name cannot be empty"
    );
    assert_eq!(
        DomainError::InvalidVariableName { name: "x!".into() }.to_string(),
        "invalid variable name: x!"
    );
    assert_eq!(
        DomainError::EmptyTestName.to_string(),
        "test name cannot be empty"
    );
    assert_eq!(
        DomainError::EmptyPipelineName.to_string(),
        "pipeline name cannot be empty"
    );
    assert_eq!(
        DomainError::InvalidHttpStatusCode { status_code: 99 }.to_string(),
        "HTTP status code 99 is invalid"
    );
    assert_eq!(
        DomainError::EmptyFieldName.to_string(),
        "field name cannot be empty"
    );
    assert_eq!(
        DomainError::EmptyRequestPath.to_string(),
        "request path cannot be empty"
    );
}

#[test]
fn public_models_round_trip_through_json() {
    let suite = TestSuite::named("round trip", vec![SuiteBlock::Test(case("test"))]);
    let json = serde_json::to_string(&suite).unwrap();
    let decoded: TestSuite = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.name.as_deref(), Some("round trip"));
    assert_eq!(decoded.len(), 1);
}

#[test]
fn suite_results_round_trip_through_json_with_source_order_and_error_details() {
    let assertion_failure = AssertionFailure {
        path: "$.id".into(),
        kind: AssertionFailureKind::TypeMismatch,
        expected: Some("integer".into()),
        actual: Some("string".into()),
        message: "expected integer".into(),
    };
    let execution_error = ExecutionErrorInfo {
        kind: ExecutionErrorKind::InvalidRequest,
        message: "invalid request".into(),
    };
    let result = SuiteResult {
        name: Some("API".into()),
        status: ExecutionStatus::Failed,
        duration_ms: 31,
        blocks: vec![
            BlockResult::Core(CoreResult {
                status: ExecutionStatus::Passed,
                duration_ms: 10,
                tests: vec![TestResult::passed("bootstrap", 10)],
            }),
            BlockResult::Pipeline(PipelineResult {
                name: "flow".into(),
                status: ExecutionStatus::Failed,
                duration_ms: 20,
                tests: vec![TestResult::failed(
                    "step",
                    20,
                    vec![assertion_failure],
                    Some(execution_error.clone()),
                )],
            }),
            BlockResult::Test(TestResult::skipped("later", "dependency failed")),
        ],
        error: Some(execution_error),
    };

    let json = serde_json::to_string(&result).unwrap();
    let decoded: SuiteResult = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded, result);
    assert!(matches!(decoded.blocks[0], BlockResult::Core(_)));
    assert!(matches!(decoded.blocks[1], BlockResult::Pipeline(_)));
    assert!(matches!(decoded.blocks[2], BlockResult::Test(_)));
}
