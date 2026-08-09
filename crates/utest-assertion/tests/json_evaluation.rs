use bytes::Bytes;
use serde_json::{Map, Value as JsonValue, json};
use utest_assertion::{
    AssertionConfig, AssertionEngine, DEFAULT_MAX_JSON_DEPTH, ResolvedBodyAssertion,
    ResolvedFieldAssertion, ResolvedObjectAssertion, ResolvedResponseExpectation,
};
use utest_domain::{AssertionFailureKind, ExpectedType};
use utest_http::{HttpResponse, ResponseBody, ResponseHeaders};

fn field(
    name: impl Into<String>,
    expected_type: ExpectedType,
    expected_value: JsonValue,
) -> ResolvedFieldAssertion {
    ResolvedFieldAssertion::type_and_value(name, expected_type, expected_value)
}

fn json_expectation(object: ResolvedObjectAssertion) -> ResolvedResponseExpectation {
    ResolvedResponseExpectation {
        status: None,
        headers: Default::default(),
        body: Some(ResolvedBodyAssertion::Json(object)),
    }
}

fn json_response(value: JsonValue) -> HttpResponse {
    HttpResponse {
        status: 200,
        headers: ResponseHeaders::new(),
        body: ResponseBody::Json {
            raw: Bytes::from_static(b"test JSON"),
            value,
        },
    }
}

fn evaluate(
    object: ResolvedObjectAssertion,
    actual: JsonValue,
) -> utest_assertion::AssertionReport {
    AssertionEngine::default().evaluate(&json_expectation(object), &json_response(actual))
}

fn nested_object(depth: usize, leaf: JsonValue) -> JsonValue {
    (0..depth).fold(leaf, |child, _| json!({ "next": child }))
}

#[test]
fn every_json_type_accepts_a_matching_value_without_requiring_exact_values() {
    let mut expected = ResolvedObjectAssertion::partial();
    for (name, expected_type) in [
        ("string", ExpectedType::String),
        ("boolean", ExpectedType::Boolean),
        ("integer", ExpectedType::Integer),
        ("number", ExpectedType::Number),
        ("object", ExpectedType::Object),
        ("array", ExpectedType::Array),
        ("null", ExpectedType::Null),
    ] {
        expected.insert(ResolvedFieldAssertion::type_only(name, expected_type));
    }

    let report = evaluate(
        expected,
        json!({
            "string": "value",
            "boolean": true,
            "integer": -7,
            "number": 2.5,
            "object": {"ignored": true},
            "array": [1, 2],
            "null": null,
            "undeclared": "allowed"
        }),
    );

    assert!(report.is_success());
}

#[test]
fn every_json_type_reports_one_non_cascading_type_mismatch() {
    let cases = [
        ("string", ExpectedType::String, json!(false), "boolean"),
        ("boolean", ExpectedType::Boolean, json!(0), "integer"),
        ("integer", ExpectedType::Integer, json!(1.5), "number"),
        ("number", ExpectedType::Number, json!("1"), "string"),
        ("object", ExpectedType::Object, json!([]), "array"),
        ("array", ExpectedType::Array, json!({}), "object"),
        ("null", ExpectedType::Null, json!(null), "boolean"),
    ];

    for (name, expected_type, wrong, actual_type) in cases {
        let actual = if name == "null" { json!(true) } else { wrong };
        let mut expected = ResolvedObjectAssertion::partial();
        expected.insert(
            field(name, expected_type.clone(), json!("value")).with_nested({
                let mut nested = ResolvedObjectAssertion::partial();
                nested.insert(ResolvedFieldAssertion::type_only(
                    "child",
                    ExpectedType::Null,
                ));
                nested
            }),
        );

        let report = evaluate(expected, json!({name: actual}));
        assert_eq!(report.len(), 1, "type mismatch must not cascade for {name}");
        let failure = &report.failures()[0];
        assert_eq!(failure.path, format!("$.{name}"));
        assert_eq!(failure.kind, AssertionFailureKind::TypeMismatch);
        assert_eq!(failure.expected.as_deref(), Some(expected_type.as_str()));
        assert_eq!(failure.actual.as_deref(), Some(actual_type));
    }
}

#[test]
fn scalar_values_have_positive_and_negative_comparisons() {
    let cases = [
        ("string", ExpectedType::String, json!("yes"), json!("no")),
        ("boolean", ExpectedType::Boolean, json!(true), json!(false)),
        ("integer", ExpectedType::Integer, json!(-2), json!(-3)),
        ("number", ExpectedType::Number, json!(2.5), json!(2.75)),
        ("null", ExpectedType::Null, json!(null), json!(false)),
    ];

    for (name, expected_type, expected_value, wrong_value) in cases {
        let mut expected = ResolvedObjectAssertion::partial();
        expected.insert(field(name, expected_type, expected_value.clone()));
        assert!(evaluate(expected.clone(), json!({name: expected_value})).is_success());

        let report = evaluate(expected, json!({name: wrong_value}));
        assert_eq!(report.len(), 1);
        assert_eq!(report.failures()[0].path, format!("$.{name}"));
        assert!(matches!(
            report.failures()[0].kind,
            AssertionFailureKind::ValueMismatch | AssertionFailureKind::TypeMismatch
        ));
    }
}

#[test]
fn integer_and_number_comparisons_preserve_representation_rules() {
    let mut numeric = ResolvedObjectAssertion::partial();
    numeric.insert(field("value", ExpectedType::Number, json!(1)));
    assert!(evaluate(numeric.clone(), json!({"value": 1.0})).is_success());

    numeric.insert(field("value", ExpectedType::Number, json!(1.0)));
    assert!(evaluate(numeric, json!({"value": 1})).is_success());

    let mut integer = ResolvedObjectAssertion::partial();
    integer.insert(field("value", ExpectedType::Integer, json!(1)));
    let report = evaluate(integer, json!({"value": 1.0}));
    assert_eq!(
        report.failures()[0].kind,
        AssertionFailureKind::TypeMismatch
    );
    assert_eq!(report.failures()[0].actual.as_deref(), Some("number"));

    let limit = 9_007_199_254_740_991_u64;
    let mut safe = ResolvedObjectAssertion::partial();
    safe.insert(field("value", ExpectedType::Number, json!(limit)));
    assert!(evaluate(safe, json!({"value": limit as f64})).is_success());

    let outside = limit + 1;
    let mut unsafe_rounding = ResolvedObjectAssertion::partial();
    unsafe_rounding.insert(field("value", ExpectedType::Number, json!(outside)));
    let report = evaluate(unsafe_rounding, json!({"value": outside as f64}));
    assert_eq!(
        report.failures()[0].kind,
        AssertionFailureKind::ValueMismatch
    );
}

#[test]
fn object_modes_are_partial_or_exact_only_at_the_declared_level() {
    let mut partial = ResolvedObjectAssertion::partial();
    partial.insert(field("id", ExpectedType::Integer, json!(1)));
    assert!(evaluate(partial, json!({"id": 1, "extra": true})).is_success());

    let mut exact = ResolvedObjectAssertion::exact();
    exact.insert(field("id", ExpectedType::Integer, json!(1)));
    let report = evaluate(exact, json!({"z-extra": [], "id": 1, "a-extra": {}}));
    assert_eq!(report.len(), 2);
    assert_eq!(
        report.failures()[0].kind,
        AssertionFailureKind::UnexpectedField
    );
    assert_eq!(report.failures()[0].path, "$[\"a-extra\"]");
    assert_eq!(
        report.failures()[0].actual.as_deref(),
        Some("object with 0 fields")
    );
    assert_eq!(report.failures()[1].path, "$[\"z-extra\"]");
    assert_eq!(
        report.failures()[1].actual.as_deref(),
        Some("array with 0 elements")
    );
}

#[test]
fn object_value_comparison_is_recursive_and_partial_through_the_deepest_leaf() {
    let mut expected = ResolvedObjectAssertion::partial();
    expected.insert(field(
        "result",
        ExpectedType::Object,
        json!({"user": {"profile": {"active": true}}}),
    ));
    assert!(
        evaluate(
            expected.clone(),
            json!({"result": {"user": {"profile": {"active": true, "extra": 1}, "role": "admin"}}})
        )
        .is_success()
    );

    let report = evaluate(
        expected,
        json!({"result": {"user": {"profile": {"active": false}}}}),
    );
    assert_eq!(report.failures()[0].path, "$.result.user.profile.active");
    assert_eq!(
        report.failures()[0].kind,
        AssertionFailureKind::ValueMismatch
    );
}

#[test]
fn nested_blocks_apply_their_own_mode_and_report_missing_fields() {
    let mut nested = ResolvedObjectAssertion::exact();
    nested.insert(ResolvedFieldAssertion::type_only(
        "required",
        ExpectedType::String,
    ));
    let mut root = ResolvedObjectAssertion::partial();
    root.insert(
        ResolvedFieldAssertion::type_only("payload", ExpectedType::Object).with_nested(nested),
    );

    let report = evaluate(root, json!({"payload": {"extra": 1}}));
    assert_eq!(report.len(), 2);
    assert_eq!(report.failures()[0].path, "$.payload.required");
    assert_eq!(
        report.failures()[0].kind,
        AssertionFailureKind::MissingField
    );
    assert_eq!(report.failures()[0].actual, None);
    assert_eq!(report.failures()[1].path, "$.payload.extra");
    assert_eq!(
        report.failures()[1].kind,
        AssertionFailureKind::UnexpectedField
    );
}

#[test]
fn arrays_require_exact_length_and_order_but_objects_inside_are_partial() {
    let mut expected = ResolvedObjectAssertion::partial();
    expected.insert(field(
        "items",
        ExpectedType::Array,
        json!([{"id": 1}, true]),
    ));
    assert!(
        evaluate(
            expected.clone(),
            json!({"items": [{"id": 1, "extra": "allowed"}, true]})
        )
        .is_success()
    );

    let report = evaluate(expected.clone(), json!({"items": [{"id": 2}, false]}));
    assert_eq!(report.len(), 2);
    assert_eq!(report.failures()[0].path, "$.items[0].id");
    assert_eq!(report.failures()[1].path, "$.items[1]");

    let report = evaluate(expected, json!({"items": [{"id": 1}]}));
    assert_eq!(report.failures()[0].path, "$.items");
    assert_eq!(
        report.failures()[0].expected.as_deref(),
        Some("array length 2")
    );
    assert_eq!(
        report.failures()[0].actual.as_deref(),
        Some("array length 1")
    );
}

#[test]
fn comparison_reports_nested_type_differences_and_missing_values() {
    let mut expected = ResolvedObjectAssertion::partial();
    expected.insert(field(
        "payload",
        ExpectedType::Object,
        json!({"object": {}, "array": [], "missing": "value"}),
    ));

    let report = evaluate(
        expected,
        json!({"payload": {"object": [], "array": {}, "extra": true}}),
    );
    assert_eq!(report.len(), 3);
    assert_eq!(report.failures()[0].path, "$.payload.array");
    assert_eq!(
        report.failures()[0].kind,
        AssertionFailureKind::TypeMismatch
    );
    assert_eq!(report.failures()[1].path, "$.payload.missing");
    assert_eq!(
        report.failures()[1].kind,
        AssertionFailureKind::MissingField
    );
    assert_eq!(report.failures()[1].expected.as_deref(), Some("\"value\""));
    assert_eq!(report.failures()[2].path, "$.payload.object");
    assert_eq!(
        report.failures()[2].kind,
        AssertionFailureKind::TypeMismatch
    );
}

#[test]
fn json_root_must_be_an_object_and_be_json_classified() {
    let expected = json_expectation(ResolvedObjectAssertion::partial());
    let engine = AssertionEngine::default();

    for (value, kind) in [
        (json!(null), "null"),
        (json!(true), "boolean"),
        (json!(1), "integer"),
        (json!(1.5), "number"),
        (json!("text"), "string"),
        (json!([]), "array"),
    ] {
        let report = engine.evaluate(&expected, &json_response(value));
        assert_eq!(report.failures()[0].path, "$");
        assert_eq!(
            report.failures()[0].kind,
            AssertionFailureKind::TypeMismatch
        );
        assert_eq!(report.failures()[0].actual.as_deref(), Some(kind));
    }

    for (body, kind) in [
        (ResponseBody::Empty, "empty"),
        (ResponseBody::Text(Bytes::from_static(b"{}")), "text"),
        (ResponseBody::Binary(Bytes::from_static(b"{}")), "binary"),
    ] {
        let actual = HttpResponse {
            status: 200,
            headers: ResponseHeaders::new(),
            body,
        };
        let report = engine.evaluate(&expected, &actual);
        assert_eq!(report.failures()[0].kind, AssertionFailureKind::InvalidBody);
        assert_eq!(report.failures()[0].actual.as_deref(), Some(kind));
    }
}

#[test]
fn paths_use_identifiers_json_quoting_and_array_indexes() {
    let unusual = "display-\"name\\line\n";
    let mut expected = ResolvedObjectAssertion::partial();
    expected.insert(field(
        "_profile1",
        ExpectedType::Array,
        json!([{unusual: "expected"}]),
    ));

    let report = evaluate(expected, json!({"_profile1": [{unusual: "actual"}]}));
    assert_eq!(
        report.failures()[0].path,
        "$._profile1[0][\"display-\\\"name\\\\line\\n\"]"
    );
}

#[test]
fn json_failures_stop_deterministically_after_one_beyond_the_cap() {
    let mut expected = ResolvedObjectAssertion::partial();
    expected.insert(ResolvedFieldAssertion::type_only(
        "first",
        ExpectedType::String,
    ));
    expected.insert(ResolvedFieldAssertion::type_only(
        "second",
        ExpectedType::String,
    ));
    expected.insert(ResolvedFieldAssertion::type_only(
        "third",
        ExpectedType::String,
    ));
    let engine = AssertionEngine::new(AssertionConfig::new(2, 8).expect("valid limits"));

    let report = engine.evaluate(&json_expectation(expected), &json_response(json!({})));
    assert_eq!(report.len(), 2);
    assert!(report.is_truncated());
    assert_eq!(report.failures()[0].path, "$.first");
    assert_eq!(report.failures()[1].path, "$.second");
}

#[test]
fn custom_depth_accepts_the_boundary_and_rejects_the_next_container() {
    let mut expected = ResolvedObjectAssertion::partial();
    expected.insert(field(
        "payload",
        ExpectedType::Object,
        nested_object(2, json!(true)),
    ));
    let actual = json!({"payload": nested_object(2, json!(true))});

    let boundary = AssertionEngine::new(AssertionConfig::new(10, 2).expect("valid limits"))
        .evaluate(
            &json_expectation(expected.clone()),
            &json_response(actual.clone()),
        );
    assert!(boundary.is_success());

    let exceeded = AssertionEngine::new(AssertionConfig::new(10, 1).expect("valid limits"))
        .evaluate(&json_expectation(expected), &json_response(actual));
    let failure = &exceeded.failures()[0];
    assert_eq!(failure.path, "$.payload.next");
    assert_eq!(failure.kind, AssertionFailureKind::InvalidBody);
    assert_eq!(
        failure.expected.as_deref(),
        Some("JSON nesting at most 1 levels")
    );
    assert_eq!(
        failure.actual.as_deref(),
        Some("JSON nesting exceeds level 2")
    );
}

#[test]
fn default_depth_accepts_its_boundary_and_rejects_one_more_container() {
    let boundary_value = nested_object(DEFAULT_MAX_JSON_DEPTH, json!(true));
    let mut boundary = ResolvedObjectAssertion::partial();
    boundary.insert(field(
        "payload",
        ExpectedType::Object,
        boundary_value.clone(),
    ));
    assert!(
        evaluate(boundary, json!({"payload": boundary_value})).is_success(),
        "the documented default depth itself must be accepted"
    );

    let exceeded_value = nested_object(DEFAULT_MAX_JSON_DEPTH + 1, json!(true));
    let mut exceeded = ResolvedObjectAssertion::partial();
    exceeded.insert(field(
        "payload",
        ExpectedType::Object,
        exceeded_value.clone(),
    ));
    let report = evaluate(exceeded, json!({"payload": exceeded_value}));
    assert_eq!(report.failures()[0].kind, AssertionFailureKind::InvalidBody);
}

#[test]
fn json_diagnostics_bound_strings_and_summarize_compound_values() {
    let long = "界".repeat(257);
    let mut expected = ResolvedObjectAssertion::exact();
    expected.insert(field("text", ExpectedType::String, json!(long)));
    expected.insert(field(
        "payload",
        ExpectedType::Object,
        json!({
            "missing-array": [1, 2, 3],
            "missing-object": {"secret": "not retained"}
        }),
    ));

    let report = evaluate(
        expected,
        json!({
            "text": "different",
            "payload": {},
            "extra-array": [1, 2],
            "extra-object": {"a": 1}
        }),
    );
    let text = report.failures()[0]
        .expected
        .as_deref()
        .expect("string preview is present");
    assert!(text.ends_with('…'));
    assert_eq!(text.matches('界').count(), 256);
    assert_eq!(
        report.failures()[1].expected.as_deref(),
        Some("array with 3 elements")
    );
    assert_eq!(
        report.failures()[2].expected.as_deref(),
        Some("object with 1 fields")
    );
    assert!(report.failures().iter().any(|failure| {
        failure.path == "$[\"extra-array\"]"
            && failure.actual.as_deref() == Some("array with 2 elements")
    }));
    assert!(report.failures().iter().any(|failure| {
        failure.path == "$[\"extra-object\"]"
            && failure.actual.as_deref() == Some("object with 1 fields")
    }));
}

#[test]
fn empty_and_unicode_keys_use_bracket_paths() {
    let mut expected = ResolvedObjectAssertion::partial();
    expected.insert(ResolvedFieldAssertion::type_only("", ExpectedType::String));
    expected.insert(ResolvedFieldAssertion::type_only(
        "naïve",
        ExpectedType::String,
    ));

    let report = evaluate(expected, JsonValue::Object(Map::new()));
    assert_eq!(report.failures()[0].path, "$[\"\"]");
    assert_eq!(report.failures()[1].path, "$[\"naïve\"]");
}
