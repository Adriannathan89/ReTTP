use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

use rettp_application::{ExecutionEngine, check_source};
use rettp_domain::{
    BlockResult, BodyAssertion, Capture, CoreBlock, ExecutionErrorKind, ExecutionStatus,
    ExpectedType, FieldAssertion, HeaderAssertion, HttpMethod, HttpRequestSpec, ObjectAssertion,
    PipelineBlock, ResponseExpectation, SuiteBlock, TestCase, TestSuite, TextAssertion, Value,
    VariableName,
};
use rettp_http::{
    HttpClient, HttpError, HttpResponse, ResolvedHttpRequest, ResponseBody, ResponseHeaders,
};
use rettp_parser::{SourceText, ValidationContext};
use rettp_runtime::{RuntimeResolver, VariableStore, VariableValue};

type Reply = Result<HttpResponse, HttpError>;

#[derive(Clone, Default)]
struct FakeClient {
    replies: Arc<Mutex<VecDeque<Reply>>>,
    requests: Arc<Mutex<Vec<ResolvedHttpRequest>>>,
}

impl FakeClient {
    fn new(replies: impl IntoIterator<Item = Reply>) -> Self {
        Self {
            replies: Arc::new(Mutex::new(replies.into_iter().collect())),
            requests: Arc::default(),
        }
    }

    fn requests(&self) -> Vec<ResolvedHttpRequest> {
        self.requests.lock().expect("request lock poisoned").clone()
    }

    fn call_count(&self) -> usize {
        self.requests.lock().expect("request lock poisoned").len()
    }
}

impl HttpClient for FakeClient {
    fn execute<'life0, 'life1, 'async_trait>(
        &'life0 self,
        request: &'life1 ResolvedHttpRequest,
    ) -> Pin<Box<dyn Future<Output = Reply> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            self.requests
                .lock()
                .expect("request lock poisoned")
                .push(request.clone());
            self.replies
                .lock()
                .expect("reply lock poisoned")
                .pop_front()
                .expect("fake client received an unexpected request")
        })
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("execution unexpectedly yielded with an immediate fake client"),
    }
}

fn response(status: u16) -> HttpResponse {
    HttpResponse {
        status,
        headers: ResponseHeaders::new(),
        body: ResponseBody::Empty,
    }
}

fn text_response(status: u16, text: &'static str) -> HttpResponse {
    HttpResponse {
        status,
        headers: ResponseHeaders::new().with_header("x-result", "available"),
        body: ResponseBody::Text(text.as_bytes().to_vec().into()),
    }
}

fn json_response(status: u16, json: &str) -> HttpResponse {
    HttpResponse {
        status,
        headers: ResponseHeaders::new(),
        body: ResponseBody::Json {
            raw: json.as_bytes().to_vec().into(),
            value: json.parse().expect("valid JSON fixture"),
        },
    }
}

fn test_case(name: &str, path: &str, status: u16) -> TestCase {
    TestCase::new(
        name,
        HttpRequestSpec::new(HttpMethod::GET, path),
        ResponseExpectation {
            status: Some(status),
            ..ResponseExpectation::default()
        },
    )
}

fn engine() -> ExecutionEngine {
    ExecutionEngine::default()
}

fn execute(
    suite: &TestSuite,
    variables: &VariableStore,
    client: &FakeClient,
) -> rettp_domain::SuiteResult {
    block_on(engine().execute(suite, variables, client))
}

fn status_of(result: &BlockResult) -> ExecutionStatus {
    match result {
        BlockResult::Core(result) => result.status,
        BlockResult::Pipeline(result) => result.status,
        BlockResult::Test(result) => result.status,
    }
}

fn test_result(result: &BlockResult) -> &rettp_domain::TestResult {
    let BlockResult::Test(result) = result else {
        panic!("expected standalone result");
    };
    result
}

#[test]
fn public_configuration_accessors_and_default_are_stable() {
    let default = ExecutionEngine::default();
    let configured = ExecutionEngine::new(RuntimeResolver::default(), Default::default());

    assert_eq!(configured, default);
    assert_eq!(configured.resolver(), RuntimeResolver::default());
    assert_eq!(configured.assertions(), Default::default());
    assert_eq!(configured.clone(), configured);
    assert!(format!("{configured:?}").contains("ExecutionEngine"));
}

#[test]
fn preflight_rejects_every_invalid_shape_before_http() {
    let valid = test_case("unreached", "/", 200);
    let cases = [
        TestSuite::new(Vec::new()),
        TestSuite::new(vec![
            SuiteBlock::Core(CoreBlock::new(vec![valid.clone()])),
            SuiteBlock::Core(CoreBlock::new(vec![valid.clone()])),
        ]),
        TestSuite::new(vec![SuiteBlock::Pipeline(PipelineBlock::new(
            "empty",
            Vec::new(),
        ))]),
    ];

    for suite in cases {
        let client = FakeClient::default();
        let result = execute(&suite, &VariableStore::new(), &client);
        assert_eq!(result.status, ExecutionStatus::Aborted);
        assert_eq!(result.name, suite.name);
        assert_eq!(result.blocks.len(), suite.blocks.len());
        assert!(
            result
                .blocks
                .iter()
                .all(|block| status_of(block) == ExecutionStatus::Skipped)
        );
        assert_eq!(
            result.error.as_ref().expect("suite error").kind,
            ExecutionErrorKind::Internal
        );
        assert_eq!(client.call_count(), 0);
    }
}

#[test]
fn core_executes_first_while_results_stay_in_source_order_and_empty_core_is_valid() {
    let suite = TestSuite::named(
        "ordered",
        vec![
            SuiteBlock::Test(test_case("after", "/after", 201)),
            SuiteBlock::Core(CoreBlock::new(vec![test_case("setup", "/setup", 200)])),
            SuiteBlock::Pipeline(PipelineBlock::new(
                "flow",
                vec![test_case("pipeline", "/pipeline", 202)],
            )),
        ],
    );
    let client = FakeClient::new([Ok(response(200)), Ok(response(201)), Ok(response(202))]);

    let result = execute(&suite, &VariableStore::new(), &client);

    assert_eq!(result.status, ExecutionStatus::Passed);
    assert_eq!(result.name.as_deref(), Some("ordered"));
    assert!(matches!(
        &result.blocks[..],
        [
            BlockResult::Test(_),
            BlockResult::Core(_),
            BlockResult::Pipeline(_)
        ]
    ));
    assert_eq!(
        client
            .requests()
            .iter()
            .map(|request| request.path.as_str())
            .collect::<Vec<_>>(),
        ["/setup", "/after", "/pipeline"]
    );
    assert!(result.error.is_none());

    let empty_core_suite = TestSuite::new(vec![SuiteBlock::Core(CoreBlock::new(Vec::new()))]);
    let empty_result = execute(
        &empty_core_suite,
        &VariableStore::new(),
        &FakeClient::default(),
    );
    assert_eq!(empty_result.status, ExecutionStatus::Passed);
    let BlockResult::Core(core) = &empty_result.blocks[0] else {
        panic!("core result")
    };
    assert!(core.tests.is_empty());
}

#[test]
fn failed_core_aborts_suite_and_skips_every_remaining_test() {
    let suite = TestSuite::new(vec![
        SuiteBlock::Pipeline(PipelineBlock::new("later", vec![test_case("p", "/p", 200)])),
        SuiteBlock::Core(CoreBlock::new(vec![
            test_case("bad", "/bad", 200),
            test_case("never core", "/never-core", 200),
        ])),
        SuiteBlock::Test(test_case("never standalone", "/never", 200)),
    ]);
    let client = FakeClient::new([Ok(response(500))]);

    let result = execute(&suite, &VariableStore::new(), &client);

    assert_eq!(result.status, ExecutionStatus::Aborted);
    assert_eq!(client.call_count(), 1);
    assert_eq!(
        result.error.as_ref().expect("dependency error").kind,
        ExecutionErrorKind::DependencyFailed
    );
    assert_eq!(status_of(&result.blocks[0]), ExecutionStatus::Skipped);
    let BlockResult::Core(core) = &result.blocks[1] else {
        panic!("core result")
    };
    assert_eq!(core.status, ExecutionStatus::Failed);
    assert_eq!(
        core.tests
            .iter()
            .map(|test| test.status)
            .collect::<Vec<_>>(),
        [ExecutionStatus::Failed, ExecutionStatus::Skipped]
    );
    assert_eq!(status_of(&result.blocks[2]), ExecutionStatus::Skipped);
}

#[test]
fn pipeline_fails_fast_but_later_blocks_continue() {
    let suite = TestSuite::new(vec![
        SuiteBlock::Pipeline(PipelineBlock::new(
            "broken",
            vec![
                test_case("bad", "/bad", 200),
                test_case("skipped", "/skipped", 200),
            ],
        )),
        SuiteBlock::Test(test_case("continues", "/continues", 204)),
    ]);
    let client = FakeClient::new([Ok(response(500)), Ok(response(204))]);

    let result = execute(&suite, &VariableStore::new(), &client);

    assert_eq!(result.status, ExecutionStatus::Failed);
    let BlockResult::Pipeline(pipeline) = &result.blocks[0] else {
        panic!("pipeline result")
    };
    assert_eq!(pipeline.status, ExecutionStatus::Failed);
    assert_eq!(
        pipeline
            .tests
            .iter()
            .map(|test| test.status)
            .collect::<Vec<_>>(),
        [ExecutionStatus::Failed, ExecutionStatus::Skipped]
    );
    assert_eq!(
        test_result(&result.blocks[1]).status,
        ExecutionStatus::Passed
    );
    assert_eq!(
        client
            .requests()
            .iter()
            .map(|request| request.path.as_str())
            .collect::<Vec<_>>(),
        ["/bad", "/continues"]
    );
}

#[test]
fn standalone_failure_does_not_stop_later_standalone_tests() {
    let suite = TestSuite::new(vec![
        SuiteBlock::Test(test_case("first", "/first", 200)),
        SuiteBlock::Test(test_case("second", "/second", 201)),
    ]);
    let client = FakeClient::new([Ok(response(400)), Ok(response(201))]);
    let result = execute(&suite, &VariableStore::new(), &client);

    assert_eq!(result.status, ExecutionStatus::Failed);
    assert_eq!(
        test_result(&result.blocks[0]).status,
        ExecutionStatus::Failed
    );
    assert!(!test_result(&result.blocks[0]).failures.is_empty());
    assert_eq!(
        test_result(&result.blocks[1]).status,
        ExecutionStatus::Passed
    );
    assert_eq!(client.call_count(), 2);
}

fn capturing_test(name: &str, path: &str, variable: &str) -> TestCase {
    let mut object = ObjectAssertion::partial();
    object.insert(
        FieldAssertion::type_only("id", ExpectedType::Integer).with_capture(Capture::new(
            VariableName::new(variable).expect("valid fixture variable"),
        )),
    );
    TestCase::new(
        name,
        HttpRequestSpec::new(HttpMethod::GET, path),
        ResponseExpectation {
            status: Some(200),
            body: Some(BodyAssertion::Json(object)),
            ..ResponseExpectation::default()
        },
    )
}

#[test]
fn core_capture_is_reusable_by_all_blocks_and_initial_store_is_unchanged() {
    let suite = TestSuite::new(vec![
        SuiteBlock::Test(test_case("standalone", "/users/${ID}", 200)),
        SuiteBlock::Core(CoreBlock::new(vec![capturing_test(
            "capture", "/seed", "ID",
        )])),
        SuiteBlock::Pipeline(PipelineBlock::new(
            "flow",
            vec![test_case("pipeline", "/items/${ID}", 200)],
        )),
    ]);
    let initial = VariableStore::new();
    let client = FakeClient::new([
        Ok(json_response(200, r#"{"id":7}"#)),
        Ok(response(200)),
        Ok(response(200)),
    ]);

    let result = execute(&suite, &initial, &client);

    assert_eq!(result.status, ExecutionStatus::Passed);
    assert!(initial.is_empty());
    assert_eq!(
        client
            .requests()
            .iter()
            .map(|request| request.path.as_str())
            .collect::<Vec<_>>(),
        ["/seed", "/users/7", "/items/7"]
    );
}

#[test]
fn pipeline_captures_flow_forward_but_do_not_escape_and_standalone_scopes_are_isolated() {
    let suite = TestSuite::new(vec![
        SuiteBlock::Pipeline(PipelineBlock::new(
            "flow",
            vec![
                capturing_test("capture", "/seed", "LOCAL"),
                test_case("use", "/${LOCAL}", 200),
            ],
        )),
        SuiteBlock::Test(capturing_test("isolated capture", "/standalone", "SOLO")),
        SuiteBlock::Test(test_case("cannot see standalone", "/${SOLO}", 200)),
        SuiteBlock::Test(test_case("cannot see pipeline", "/${LOCAL}", 200)),
    ]);
    let client = FakeClient::new([
        Ok(json_response(200, r#"{"id":11}"#)),
        Ok(response(200)),
        Ok(json_response(200, r#"{"id":12}"#)),
    ]);

    let result = execute(&suite, &VariableStore::new(), &client);

    assert_eq!(result.status, ExecutionStatus::Failed);
    let BlockResult::Pipeline(pipeline) = &result.blocks[0] else {
        panic!("pipeline")
    };
    assert_eq!(pipeline.status, ExecutionStatus::Passed);
    assert_eq!(
        test_result(&result.blocks[1]).status,
        ExecutionStatus::Passed
    );
    for block in &result.blocks[2..] {
        let test = test_result(block);
        assert_eq!(test.status, ExecutionStatus::Failed);
        assert_eq!(
            test.error.as_ref().expect("resolution error").kind,
            ExecutionErrorKind::VariableResolution
        );
    }
    assert_eq!(
        client
            .requests()
            .iter()
            .map(|request| request.path.as_str())
            .collect::<Vec<_>>(),
        ["/seed", "/11", "/standalone"]
    );
}

#[test]
fn expectation_resolution_fails_before_http_and_messages_redact_values() {
    let mut expectation = ResponseExpectation::default();
    expectation.headers.insert(
        "x-result".to_owned(),
        HeaderAssertion::Exact("${MISSING}".into()),
    );
    let test = TestCase::new(
        "missing expected",
        HttpRequestSpec::new(HttpMethod::GET, "/safe"),
        expectation,
    );
    let client = FakeClient::new([Ok(response(200))]);
    let result = execute(
        &TestSuite::new(vec![SuiteBlock::Test(test)]),
        &VariableStore::new(),
        &client,
    );

    assert_eq!(client.call_count(), 0);
    let error = test_result(&result.blocks[0])
        .error
        .as_ref()
        .expect("resolution error");
    assert_eq!(error.kind, ExecutionErrorKind::VariableResolution);
    assert!(!error.message.contains("/safe"));

    let secret = "do-not-leak-this-value";
    let mut variables = VariableStore::new();
    variables.insert_predefined(
        VariableName::new("SECRET").unwrap(),
        VariableValue::text(secret),
    );
    let structured = TestCase::new(
        "structured in URL",
        HttpRequestSpec::new(HttpMethod::GET, "/${OBJECT}"),
        ResponseExpectation::default(),
    );
    variables.insert_predefined(
        VariableName::new("OBJECT").unwrap(),
        VariableValue::json(r#"{"secret":"do-not-leak-this-value"}"#.parse().unwrap()),
    );
    let result = execute(
        &TestSuite::new(vec![SuiteBlock::Test(structured)]),
        &variables,
        &FakeClient::default(),
    );
    let rendered = result.blocks[0].clone();
    assert!(!format!("{rendered:?}").contains(secret));
}

#[test]
fn all_http_errors_map_to_stable_execution_kinds_without_stopping_standalones() {
    let errors = [
        (
            HttpError::InvalidBaseUrl {
                reason: "configured base rejected".into(),
            },
            ExecutionErrorKind::Internal,
        ),
        (
            HttpError::InvalidRequest {
                reason: "bad request".into(),
            },
            ExecutionErrorKind::InvalidRequest,
        ),
        (
            HttpError::Connection {
                message: "offline".into(),
            },
            ExecutionErrorKind::Connection,
        ),
        (
            HttpError::Timeout {
                message: "deadline".into(),
            },
            ExecutionErrorKind::Timeout,
        ),
        (
            HttpError::InvalidResponse {
                reason: "bad response".into(),
            },
            ExecutionErrorKind::InvalidResponse,
        ),
        (
            HttpError::BodyTooLarge { limit_bytes: 16 },
            ExecutionErrorKind::InvalidResponse,
        ),
    ];
    let suite = TestSuite::new(
        (0..errors.len())
            .map(|index| SuiteBlock::Test(test_case(&format!("case {index}"), "/", 200)))
            .collect(),
    );
    let expected = errors
        .iter()
        .map(|(_, kind)| kind.clone())
        .collect::<Vec<_>>();
    let client = FakeClient::new(errors.into_iter().map(|(error, _)| Err(error)));

    let result = execute(&suite, &VariableStore::new(), &client);

    assert_eq!(result.status, ExecutionStatus::Failed);
    assert_eq!(client.call_count(), expected.len());
    assert_eq!(
        result
            .blocks
            .iter()
            .map(|block| test_result(block).error.as_ref().unwrap().kind.clone())
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn status_header_and_text_assertions_produce_structured_failures_or_pass() {
    let mut expected = ResponseExpectation {
        status: Some(200),
        body: Some(BodyAssertion::Text(TextAssertion::Contains(
            "needle".into(),
        ))),
        ..ResponseExpectation::default()
    };
    expected.headers.insert(
        "x-result".into(),
        HeaderAssertion::Exact("available".into()),
    );
    let passing = TestCase::new(
        "passes",
        HttpRequestSpec::new(HttpMethod::GET, "/pass"),
        expected.clone(),
    );
    let failing = TestCase::new(
        "fails",
        HttpRequestSpec::new(HttpMethod::GET, "/fail"),
        expected,
    );
    let client = FakeClient::new([
        Ok(text_response(200, "has needle")),
        Ok(text_response(500, "other")),
    ]);

    let result = execute(
        &TestSuite::new(vec![SuiteBlock::Test(passing), SuiteBlock::Test(failing)]),
        &VariableStore::new(),
        &client,
    );

    assert_eq!(
        test_result(&result.blocks[0]).status,
        ExecutionStatus::Passed
    );
    let failed = test_result(&result.blocks[1]);
    assert_eq!(failed.status, ExecutionStatus::Failed);
    assert!(failed.error.is_none());
    assert_eq!(failed.failures.len(), 2);
}

#[test]
fn capture_commit_conflict_aborts_test_and_applies_block_policy() {
    let capture = capturing_test("conflict", "/", "EXISTING");
    let mut variables = VariableStore::new();
    variables.insert_predefined(
        VariableName::new("EXISTING").unwrap(),
        VariableValue::text("hidden"),
    );

    let standalone = TestSuite::new(vec![
        SuiteBlock::Test(capture.clone()),
        SuiteBlock::Test(test_case("continues", "/next", 200)),
    ]);
    let client = FakeClient::new([Ok(json_response(200, r#"{"id":1}"#)), Ok(response(200))]);
    let result = execute(&standalone, &variables, &client);
    assert_eq!(result.status, ExecutionStatus::Failed);
    assert_eq!(
        test_result(&result.blocks[0]).status,
        ExecutionStatus::Aborted
    );
    assert_eq!(
        test_result(&result.blocks[0]).error.as_ref().unwrap().kind,
        ExecutionErrorKind::Internal
    );
    assert_eq!(
        test_result(&result.blocks[1]).status,
        ExecutionStatus::Passed
    );

    let pipeline = TestSuite::new(vec![SuiteBlock::Pipeline(PipelineBlock::new(
        "flow",
        vec![capture.clone(), test_case("skipped", "/skip", 200)],
    ))]);
    let result = execute(
        &pipeline,
        &variables,
        &FakeClient::new([Ok(json_response(200, r#"{"id":1}"#))]),
    );
    let BlockResult::Pipeline(pipeline) = &result.blocks[0] else {
        panic!("pipeline")
    };
    assert_eq!(pipeline.status, ExecutionStatus::Failed);
    assert_eq!(
        pipeline
            .tests
            .iter()
            .map(|test| test.status)
            .collect::<Vec<_>>(),
        [ExecutionStatus::Aborted, ExecutionStatus::Skipped]
    );

    let core = TestSuite::new(vec![SuiteBlock::Core(CoreBlock::new(vec![
        capture,
        test_case("skipped", "/skip", 200),
    ]))]);
    let result = execute(
        &core,
        &variables,
        &FakeClient::new([Ok(json_response(200, r#"{"id":1}"#))]),
    );
    assert_eq!(result.status, ExecutionStatus::Aborted);
    let BlockResult::Core(core) = &result.blocks[0] else {
        panic!("core")
    };
    assert_eq!(core.status, ExecutionStatus::Aborted);
    assert_eq!(
        core.tests
            .iter()
            .map(|test| test.status)
            .collect::<Vec<_>>(),
        [ExecutionStatus::Aborted, ExecutionStatus::Skipped]
    );
}

#[test]
fn duplicate_capture_transaction_aborts_without_leaking_response_values() {
    let variable = VariableName::new("DUPLICATE").expect("valid fixture variable");
    let mut object = ObjectAssertion::partial();
    object.insert(
        FieldAssertion::type_only("first", ExpectedType::String)
            .with_capture(Capture::new(variable.clone())),
    );
    object.insert(
        FieldAssertion::type_only("second", ExpectedType::String)
            .with_capture(Capture::new(variable)),
    );
    let test = TestCase::new(
        "duplicate capture",
        HttpRequestSpec::new(HttpMethod::GET, "/"),
        ResponseExpectation {
            body: Some(BodyAssertion::Json(object)),
            ..ResponseExpectation::default()
        },
    );
    let secret = "response-secret-must-not-leak";
    let client = FakeClient::new([Ok(json_response(
        200,
        &format!(r#"{{"first":"{secret}","second":"other"}}"#),
    ))]);

    let result = execute(
        &TestSuite::new(vec![SuiteBlock::Test(test)]),
        &VariableStore::new(),
        &client,
    );

    let result = test_result(&result.blocks[0]);
    assert_eq!(result.status, ExecutionStatus::Aborted);
    assert_eq!(
        result.error.as_ref().expect("capture error").kind,
        ExecutionErrorKind::Internal
    );
    assert!(!format!("{result:?}").contains(secret));
}

#[test]
fn durations_are_aggregate_nonnegative_and_skipped_durations_are_zero() {
    let suite = TestSuite::new(vec![SuiteBlock::Pipeline(PipelineBlock::new(
        "duration",
        vec![
            test_case("failed", "/", 200),
            test_case("skipped", "/", 200),
        ],
    ))]);
    let result = execute(
        &suite,
        &VariableStore::new(),
        &FakeClient::new([Ok(response(500))]),
    );
    let BlockResult::Pipeline(pipeline) = &result.blocks[0] else {
        panic!("pipeline")
    };
    assert!(result.duration_ms >= pipeline.duration_ms);
    assert!(pipeline.duration_ms >= pipeline.tests[0].duration_ms);
    assert_eq!(pipeline.tests[1].duration_ms, 0);
}

#[test]
fn checker_failure_produces_no_suite_and_cannot_reach_http_execution() {
    let source = SourceText::new(
        "invalid.rttp",
        r#"test "invalid" { request GET "/${UNDEFINED}" expect { status = 200 } }"#,
    );
    let report = check_source(&source, &ValidationContext::new());
    let client = FakeClient::default();

    assert!(report.has_errors());
    assert!(report.suite.is_none());
    assert_eq!(client.call_count(), 0);
}

#[test]
fn initial_variables_resolve_repeatedly_without_mutation() {
    let mut variables = VariableStore::new();
    variables.insert_predefined(VariableName::new("ID").unwrap(), VariableValue::text("42"));
    let suite = TestSuite::new(vec![
        SuiteBlock::Test(test_case("one", "/${ID}", 200)),
        SuiteBlock::Test(test_case("two", "/again/${ID}", 200)),
    ]);
    let client = FakeClient::new([Ok(response(200)), Ok(response(200))]);

    let result = execute(&suite, &variables, &client);

    assert_eq!(result.status, ExecutionStatus::Passed);
    assert_eq!(
        client
            .requests()
            .iter()
            .map(|request| request.path.as_str())
            .collect::<Vec<_>>(),
        ["/42", "/again/42"]
    );
    assert_eq!(
        variables
            .get(&VariableName::new("ID").unwrap())
            .and_then(VariableValue::as_text),
        Some("42")
    );
}

#[test]
fn request_values_are_resolved_before_transport() {
    let mut variables = VariableStore::new();
    variables.insert_predefined(
        VariableName::new("TOKEN").unwrap(),
        VariableValue::text("abc"),
    );
    let request = HttpRequestSpec::new(HttpMethod::POST, "/submit")
        .with_header("authorization", Value::from("Bearer ${TOKEN}"))
        .with_query_param("token", Value::from("${TOKEN}"))
        .with_body(rettp_domain::RequestBody::Text("payload ${TOKEN}".into()));
    let test = TestCase::new(
        "resolved",
        request,
        ResponseExpectation {
            status: Some(200),
            ..Default::default()
        },
    );
    let client = FakeClient::new([Ok(response(200))]);

    let result = execute(
        &TestSuite::new(vec![SuiteBlock::Test(test)]),
        &variables,
        &client,
    );

    assert_eq!(result.status, ExecutionStatus::Passed);
    let requests = client.requests();
    let sent = &requests[0];
    assert_eq!(sent.headers["authorization"], "Bearer abc".into());
    assert_eq!(sent.query["token"], "abc".into());
    assert!(
        matches!(&sent.body, Some(rettp_http::ResolvedRequestBody::Text(body)) if body == "payload abc")
    );
}
