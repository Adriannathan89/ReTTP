use bytes::Bytes;
use rettp_assertion::{
    AssertionEngine, ResolvedBodyAssertion, ResolvedFieldAssertion, ResolvedObjectAssertion,
    ResolvedResponseExpectation, ResolvedTextAssertion,
};
use rettp_domain::{Capture, ExpectedType, InterpolatedString, VariableName};
use rettp_http::{HttpResponse, ResponseBody, ResponseHeaders};
use rettp_runtime::{
    CaptureEngine, Interpolator, ResolutionLocation, RuntimeError, VariableStore, VariableValue,
};
use serde_json::{Value as JsonValue, json};

fn name(value: &str) -> VariableName {
    VariableName::new(value).expect("test variable name should be valid")
}

fn captured_field(
    field_name: &str,
    expected_type: ExpectedType,
    variable: &str,
) -> ResolvedFieldAssertion {
    ResolvedFieldAssertion::type_only(field_name, expected_type)
        .with_capture(Capture::new(name(variable)))
}

fn expectation(object: ResolvedObjectAssertion) -> ResolvedResponseExpectation {
    ResolvedResponseExpectation {
        status: Some(200),
        headers: Default::default(),
        body: Some(ResolvedBodyAssertion::Json(object)),
    }
}

fn response(status: u16, value: JsonValue) -> HttpResponse {
    HttpResponse {
        status,
        headers: ResponseHeaders::new(),
        body: ResponseBody::Json {
            raw: Bytes::from_static(b"redacted test response"),
            value,
        },
    }
}

fn evaluate(
    expected: &ResolvedResponseExpectation,
    actual: &HttpResponse,
) -> Result<rettp_runtime::CaptureEvaluation, RuntimeError> {
    CaptureEngine.evaluate(&AssertionEngine::default(), expected, actual)
}

#[test]
fn assertion_failure_never_stages_captures_and_report_apis_remain_available() {
    let mut object = ResolvedObjectAssertion::partial();
    object.insert(captured_field("secret", ExpectedType::String, "SAVED"));

    let evaluation = evaluate(&expectation(object), &response(503, json!({"secret": 7})))
        .expect("assertion failures are reports rather than runtime errors");

    assert!(!evaluation.report().is_success());
    assert!(!evaluation.report().is_empty());
    assert_eq!(evaluation.report().len(), 2);
    assert!(!evaluation.report().is_truncated());
    assert!(evaluation.pending().is_none());

    let (report, pending) = evaluation.into_parts();
    assert!(pending.is_none());
    assert_eq!(report.into_failures().len(), 2);
}

#[test]
fn successful_capture_preserves_every_json_type_u64_and_deterministic_nested_order() {
    let mut nested = ResolvedObjectAssertion::partial();
    nested.insert(captured_field("child-key", ExpectedType::String, "CHILD"));

    let mut object = ResolvedObjectAssertion::partial();
    object.insert(captured_field("parent", ExpectedType::Object, "PARENT").with_nested(nested));
    for (field_name, expected_type, variable) in [
        ("null", ExpectedType::Null, "NULL"),
        ("boolean", ExpectedType::Boolean, "BOOLEAN"),
        ("signed", ExpectedType::Integer, "SIGNED"),
        ("unsigned", ExpectedType::Integer, "UNSIGNED"),
        ("number", ExpectedType::Number, "NUMBER"),
        ("string", ExpectedType::String, "STRING"),
        ("array", ExpectedType::Array, "ARRAY"),
    ] {
        object.insert(captured_field(field_name, expected_type, variable));
    }

    let actual = json!({
        "parent": {"child-key": "deep secret", "ignored": true},
        "null": null,
        "boolean": true,
        "signed": -9,
        "unsigned": u64::MAX,
        "number": 1.25,
        "string": "top secret",
        "array": [1, {"nested": false}]
    });
    let evaluation = evaluate(&expectation(object), &response(200, actual))
        .expect("matching response should stage captures");
    assert!(evaluation.report().is_success());

    let pending = evaluation
        .pending()
        .expect("successful evaluation stages a transaction");
    assert_eq!(pending.len(), 9);
    assert!(!pending.is_empty());
    assert_eq!(
        pending
            .names()
            .map(VariableName::as_str)
            .collect::<Vec<_>>(),
        [
            "PARENT", "CHILD", "NULL", "BOOLEAN", "SIGNED", "UNSIGNED", "NUMBER", "STRING", "ARRAY"
        ]
    );

    let rendered = format!("{pending:?}");
    assert!(rendered.contains("PARENT"));
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("deep secret"));
    assert!(!rendered.contains("top secret"));
    let evaluation_debug = format!("{evaluation:?}");
    assert!(evaluation_debug.contains("<redacted>"));
    assert!(!evaluation_debug.contains("deep secret"));
    assert!(!evaluation_debug.contains("top secret"));

    let (_, pending) = evaluation.into_parts();
    let mut store = VariableStore::new();
    store
        .commit(pending.expect("transaction should remain available"))
        .expect("fresh store accepts every capture");

    let expected_values = [
        (
            "PARENT",
            json!({"child-key": "deep secret", "ignored": true}),
        ),
        ("CHILD", json!("deep secret")),
        ("NULL", json!(null)),
        ("BOOLEAN", json!(true)),
        ("SIGNED", json!(-9)),
        ("UNSIGNED", json!(u64::MAX)),
        ("NUMBER", json!(1.25)),
        ("STRING", json!("top secret")),
        ("ARRAY", json!([1, {"nested": false}])),
    ];
    for (variable, expected) in expected_values {
        let stored = store
            .get(&name(variable))
            .and_then(VariableValue::as_json)
            .expect("captured value should remain typed JSON");
        assert_eq!(stored, &expected);
        assert_eq!(
            store.get(&name(variable)).and_then(VariableValue::as_json),
            Some(&expected),
            "reading a capture repeatedly must not consume it"
        );
    }

    let cloned_scope = store.clone();
    assert_eq!(cloned_scope, store);
    for variable in ["PARENT", "CHILD", "NULL", "ARRAY"] {
        let original = store
            .get(&name(variable))
            .and_then(VariableValue::as_json)
            .expect("capture should be JSON");
        let cloned = cloned_scope
            .get(&name(variable))
            .and_then(VariableValue::as_json)
            .expect("cloned capture should be JSON");
        assert!(
            std::ptr::eq(original, cloned),
            "scope clone should retain shared storage for {variable}"
        );
    }
    let store_debug = format!("{store:?}");
    assert!(store_debug.contains("<redacted>"));
    assert!(!store_debug.contains("deep secret"));
    assert!(!store_debug.contains("top secret"));
}

#[test]
fn variable_value_storage_is_private_equatable_interpolatable_and_redacted() {
    let secret = "shared secret must stay redacted";
    let value = VariableValue::json(json!({"token": secret, "nested": [1, true]}));
    let equivalent = VariableValue::from(json!({"token": secret, "nested": [1, true]}));
    assert_eq!(value, equivalent);
    assert_eq!(value.type_name(), "object");
    assert_eq!(value.as_json(), equivalent.as_json());
    assert_eq!(value.as_text(), None);

    for rendered in [format!("{value:?}"), format!("{equivalent:?}")] {
        assert!(rendered.contains("object"));
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains(secret));
    }

    let mut store = VariableStore::new();
    store.insert_predefined(name("COUNT"), VariableValue::json(json!(u64::MAX)));
    assert_eq!(
        Interpolator::default()
            .interpolate(
                &InterpolatedString::from("value=${COUNT}"),
                &store,
                ResolutionLocation::RequestHeader,
            )
            .expect("shared scalar should interpolate exactly like owned JSON"),
        format!("value={}", u64::MAX)
    );
}

#[test]
fn captures_resolve_deep_relative_paths_and_independent_topmost_subtrees() {
    let mut below_captured_parent = ResolvedObjectAssertion::partial();
    below_captured_parent.insert(captured_field(
        "grand-child",
        ExpectedType::String,
        "DESCENDANT",
    ));
    let mut parent_nested = ResolvedObjectAssertion::partial();
    parent_nested.insert(
        ResolvedFieldAssertion::type_only("bridge", ExpectedType::Object)
            .with_nested(below_captured_parent),
    );

    let mut independent_nested = ResolvedObjectAssertion::partial();
    independent_nested.insert(captured_field(
        "independent-leaf",
        ExpectedType::Array,
        "INDEPENDENT",
    ));

    let mut object = ResolvedObjectAssertion::partial();
    object.insert(
        captured_field("parent", ExpectedType::Object, "PARENT").with_nested(parent_nested),
    );
    object.insert(
        ResolvedFieldAssertion::type_only("uncaptured-root", ExpectedType::Object)
            .with_nested(independent_nested),
    );

    let evaluation = evaluate(
        &expectation(object),
        &response(
            200,
            json!({
                "parent": {
                    "bridge": {"grand-child": "deep value", "sibling": "retained by parent"}
                },
                "uncaptured-root": {
                    "independent-leaf": [1, 2],
                    "unrelated": "must not affect the selected capture"
                },
                "unasserted-response-field": vec![0; 32]
            }),
        ),
    )
    .expect("deep and independent captures should stage successfully");
    let (_, pending) = evaluation.into_parts();
    let mut store = VariableStore::new();
    store
        .commit(pending.expect("successful evaluation should contain a transaction"))
        .expect("fresh scope accepts captures");

    assert_eq!(
        store.get(&name("PARENT")).and_then(VariableValue::as_json),
        Some(&json!({
            "bridge": {"grand-child": "deep value", "sibling": "retained by parent"}
        }))
    );
    assert_eq!(
        store
            .get(&name("DESCENDANT"))
            .and_then(VariableValue::as_json),
        Some(&json!("deep value"))
    );
    assert_eq!(
        store
            .get(&name("INDEPENDENT"))
            .and_then(VariableValue::as_json),
        Some(&json!([1, 2]))
    );
}

#[test]
fn successful_evaluation_without_capture_returns_an_empty_transaction() {
    let mut object = ResolvedObjectAssertion::partial();
    object.insert(ResolvedFieldAssertion::type_only(
        "id",
        ExpectedType::Integer,
    ));

    let evaluation = evaluate(&expectation(object), &response(200, json!({"id": 1})))
        .expect("matching response should succeed");
    let pending = evaluation
        .pending()
        .expect("success has a commit-ready transaction");
    assert_eq!(pending.len(), 0);
    assert!(pending.is_empty());
    assert_eq!(pending.names().len(), 0);

    let (_, pending) = evaluation.into_parts();
    let mut store = VariableStore::new();
    store
        .commit(pending.expect("empty transaction should be present"))
        .expect("empty transaction should commit");
    assert!(store.is_empty());

    let text_expectation = ResolvedResponseExpectation {
        status: Some(200),
        headers: Default::default(),
        body: Some(ResolvedBodyAssertion::Text(ResolvedTextAssertion::Exact(
            "plain".to_owned(),
        ))),
    };
    let text_response = HttpResponse {
        status: 200,
        headers: ResponseHeaders::new(),
        body: ResponseBody::Text(Bytes::from_static(b"plain")),
    };
    let text_evaluation = evaluate(&text_expectation, &text_response)
        .expect("successful non-JSON assertions should produce an empty transaction");
    assert!(text_evaluation.report().is_success());
    assert!(
        text_evaluation
            .pending()
            .expect("success should carry a transaction")
            .is_empty()
    );
}

#[test]
fn duplicate_commit_is_atomic_even_when_non_conflicting_capture_precedes_collision() {
    let mut object = ResolvedObjectAssertion::partial();
    object.insert(captured_field("fresh", ExpectedType::String, "FRESH"));
    object.insert(captured_field("taken", ExpectedType::String, "TAKEN"));
    let evaluation = evaluate(
        &expectation(object),
        &response(
            200,
            json!({"fresh": "must not leak", "taken": "new secret"}),
        ),
    )
    .expect("matching response should stage transaction");
    let (_, pending) = evaluation.into_parts();

    let mut store = VariableStore::new();
    store.insert_predefined(name("TAKEN"), VariableValue::text("original secret"));
    let before = store.clone();
    let error = store
        .commit(pending.expect("successful evaluation should stage captures"))
        .expect_err("visible variable must reject the whole transaction");

    assert_eq!(
        error,
        RuntimeError::DuplicateVariable {
            name: name("TAKEN")
        }
    );
    assert_eq!(store, before, "failed commit must not partially add FRESH");
    assert!(!store.contains(&name("FRESH")));
    assert_eq!(
        store.get(&name("TAKEN")).and_then(VariableValue::as_text),
        Some("original secret")
    );
    let rendered = format!("{error:?} {error}");
    assert!(rendered.contains("TAKEN"));
    assert!(!rendered.contains("original secret"));
    assert!(!rendered.contains("new secret"));
    assert!(!rendered.contains("must not leak"));
}

#[test]
fn duplicate_names_within_programmatic_capture_model_are_rejected_before_commit() {
    let mut object = ResolvedObjectAssertion::partial();
    object.insert(captured_field("first", ExpectedType::String, "SAME"));
    object.insert(captured_field("second", ExpectedType::String, "SAME"));

    let error = evaluate(
        &expectation(object),
        &response(200, json!({"first": "secret one", "second": "secret two"})),
    )
    .expect_err("duplicate staged names violate the resolved-model invariant");
    assert_eq!(
        error,
        RuntimeError::DuplicateVariable { name: name("SAME") }
    );
    let rendered = format!("{error:?} {error}");
    assert!(!rendered.contains("secret one"));
    assert!(!rendered.contains("secret two"));
}

#[test]
fn inconsistent_public_field_key_reports_missing_capture_path_without_response_value() {
    let mut object = ResolvedObjectAssertion::partial();
    object.fields.insert(
        "asserted-key".to_owned(),
        captured_field("different-key", ExpectedType::String, "SAVED"),
    );

    let error = evaluate(
        &expectation(object),
        &response(200, json!({"asserted-key": "secret response value"})),
    )
    .expect_err("inconsistent public resolved model should fail capture extraction");
    assert_eq!(
        error,
        RuntimeError::MissingCaptureField {
            path: "$[\"different-key\"]".to_owned(),
        }
    );
    let rendered = format!("{error:?} {error}");
    assert!(rendered.contains("different-key"));
    assert!(!rendered.contains("secret response value"));
}

#[test]
fn invalid_nested_capture_reports_types_and_escaped_path_without_values() {
    let cases = [
        (ExpectedType::Null, json!(null), "null"),
        (ExpectedType::Boolean, json!(true), "boolean"),
        (ExpectedType::Integer, json!(7), "integer"),
        (ExpectedType::Number, json!(1.25), "number"),
        (ExpectedType::String, json!("secret payload"), "string"),
        (ExpectedType::Array, json!(["secret payload"]), "array"),
    ];

    for (expected_type, actual, actual_type) in cases {
        let mut nested = ResolvedObjectAssertion::partial();
        nested.insert(captured_field("child", ExpectedType::String, "CHILD"));
        let mut object = ResolvedObjectAssertion::partial();
        object.insert(
            ResolvedFieldAssertion::type_only("not an identifier", expected_type)
                .with_nested(nested),
        );

        let error = evaluate(
            &expectation(object),
            &response(200, json!({"not an identifier": actual})),
        )
        .expect_err("nested capture requires an actual object");
        assert_eq!(
            error,
            RuntimeError::InvalidNestedCaptureField {
                path: "$[\"not an identifier\"]".to_owned(),
                actual_type,
            }
        );
        let rendered = format!("{error:?} {error}");
        assert!(rendered.contains("not an identifier"));
        assert!(!rendered.contains("secret payload"));
    }
}
