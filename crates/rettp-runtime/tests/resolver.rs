use std::time::Duration;

use indexmap::IndexMap;
use rettp_assertion::{ResolvedBodyAssertion, ResolvedHeaderAssertion, ResolvedTextAssertion};
use rettp_domain::{
    BodyAssertion, Capture, ExpectedType, FieldAssertion, HeaderAssertion, HttpMethod,
    HttpRequestSpec, ObjectAssertion, RequestBody, ResponseExpectation, TextAssertion, Value,
    VariableName,
};
use rettp_http::{ResolvedRequestBody, ResolvedValue};
use rettp_runtime::{
    ResolutionLocation, RuntimeConfig, RuntimeError, RuntimeResolver, VariableStore, VariableValue,
};
use serde_json::json;

fn name(value: &str) -> VariableName {
    VariableName::new(value).expect("test variable name should be valid")
}

fn store(entries: impl IntoIterator<Item = (&'static str, VariableValue)>) -> VariableStore {
    let mut variables = VariableStore::new();
    for (raw_name, value) in entries {
        variables.insert_predefined(name(raw_name), value);
    }
    variables
}

#[test]
fn resolves_request_locations_in_declaration_order_without_consuming_variables() {
    let variables = store([
        ("ID", VariableValue::text("42")),
        ("TOKEN", VariableValue::json(json!("secret"))),
        ("COUNT", VariableValue::json(json!(7))),
        ("ACTIVE", VariableValue::json(json!(true))),
        ("NONE", VariableValue::json(json!(null))),
    ]);
    let mut request = HttpRequestSpec::new(HttpMethod::POST, "/users/${ID}/${ID}")
        .with_header("authorization", "Bearer ${TOKEN}")
        .with_query_param("count", "${COUNT}")
        .with_query_param("active", "${ACTIVE}")
        .with_query_param("none", "${NONE}")
        .with_body(RequestBody::Text("id=${ID}".into()));
    request.timeout_ms = Some(250);

    let resolver = RuntimeResolver::default();
    let first = resolver.resolve_request(&request, &variables).unwrap();
    let second = resolver.resolve_request(&request, &variables).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.path, "/users/42/42");
    assert_eq!(
        first.headers["authorization"],
        ResolvedValue::String("Bearer secret".into())
    );
    assert_eq!(first.query["count"], ResolvedValue::String("7".into()));
    assert_eq!(first.query["active"], ResolvedValue::String("true".into()));
    assert_eq!(first.query["none"], ResolvedValue::String("null".into()));
    assert_eq!(first.body, Some(ResolvedRequestBody::Text("id=42".into())));
    assert_eq!(first.timeout, Some(Duration::from_millis(250)));
    assert_eq!(variables.get(&name("ID")).unwrap().as_text(), Some("42"));
}

#[test]
fn exact_structured_placeholders_preserve_json_types_only_in_json_body() {
    let variables = store([
        (
            "OBJECT",
            VariableValue::json(json!({"id": u64::MAX, "nested": [true, null]})),
        ),
        ("ARRAY", VariableValue::json(json!([1, {"ok": false}]))),
    ]);
    let body = Value::Object(IndexMap::from([
        ("object".into(), Value::String("${OBJECT}".into())),
        ("array".into(), Value::String("${ARRAY}".into())),
    ]));
    let request =
        HttpRequestSpec::new(HttpMethod::POST, "/submit").with_body(RequestBody::Json(body));

    let resolver = RuntimeResolver::default();
    let resolved = resolver.resolve_request(&request, &variables).unwrap();
    assert_eq!(
        resolver.resolve_request(&request, &variables).unwrap(),
        resolved,
        "typed captures must remain reusable rather than being consumed"
    );
    let Some(ResolvedRequestBody::Json(ResolvedValue::Object(body))) = resolved.body else {
        panic!("request should resolve to an object JSON body");
    };
    assert_eq!(
        body["object"],
        ResolvedValue::Object(IndexMap::from([
            ("id".into(), ResolvedValue::UnsignedInteger(u64::MAX)),
            (
                "nested".into(),
                ResolvedValue::Array(vec![ResolvedValue::Boolean(true), ResolvedValue::Null])
            ),
        ]))
    );
    assert_eq!(
        body["array"],
        ResolvedValue::Array(vec![
            ResolvedValue::Integer(1),
            ResolvedValue::Object(IndexMap::from([(
                "ok".into(),
                ResolvedValue::Boolean(false)
            )])),
        ])
    );
}

#[test]
fn scalar_json_placeholders_stay_strings_and_mixed_structured_text_is_rejected() {
    let variables = store([
        ("NUMBER", VariableValue::json(json!(10))),
        ("OBJECT", VariableValue::json(json!({"id": 1}))),
    ]);
    let scalar = HttpRequestSpec::new(HttpMethod::POST, "/")
        .with_body(RequestBody::Json(Value::String("${NUMBER}".into())));
    let resolved = RuntimeResolver::default()
        .resolve_request(&scalar, &variables)
        .unwrap();
    assert_eq!(
        resolved.body,
        Some(ResolvedRequestBody::Json(ResolvedValue::String(
            "10".into()
        )))
    );

    let mixed = HttpRequestSpec::new(HttpMethod::POST, "/")
        .with_body(RequestBody::Json(Value::String("prefix-${OBJECT}".into())));
    assert_eq!(
        RuntimeResolver::default().resolve_request(&mixed, &variables),
        Err(RuntimeError::UnsupportedInterpolationType {
            name: name("OBJECT"),
            value_type: "object",
            location: ResolutionLocation::JsonRequestBody,
        })
    );
}

#[test]
fn structured_placeholders_are_rejected_in_every_non_json_request_location() {
    let variables = store([("DATA", VariableValue::json(json!([1, 2])))]);
    let cases = [
        (
            HttpRequestSpec::new(HttpMethod::GET, "/${DATA}"),
            ResolutionLocation::RequestPath,
        ),
        (
            HttpRequestSpec::new(HttpMethod::GET, "/").with_header("x-data", "${DATA}"),
            ResolutionLocation::RequestHeader,
        ),
        (
            HttpRequestSpec::new(HttpMethod::GET, "/").with_query_param("data", "${DATA}"),
            ResolutionLocation::QueryParameter,
        ),
        (
            HttpRequestSpec::new(HttpMethod::POST, "/")
                .with_body(RequestBody::Text("${DATA}".into())),
            ResolutionLocation::TextRequestBody,
        ),
        (
            HttpRequestSpec::new(HttpMethod::POST, "/").with_body(RequestBody::FormData(
                IndexMap::from([("data".into(), Value::String("${DATA}".into()))]),
            )),
            ResolutionLocation::FormField,
        ),
    ];

    for (request, location) in cases {
        assert_eq!(
            RuntimeResolver::default().resolve_request(&request, &variables),
            Err(RuntimeError::UnsupportedInterpolationType {
                name: name("DATA"),
                value_type: "array",
                location,
            })
        );
    }
}

#[test]
fn resolves_literal_recursive_request_values_and_binary_body() {
    let value = Value::Array(vec![
        Value::Integer(-1),
        Value::Number(1.25),
        Value::Boolean(true),
        Value::Null,
        Value::Object(IndexMap::from([("text".into(), Value::from("plain"))])),
    ]);
    let json_request =
        HttpRequestSpec::new(HttpMethod::POST, "/").with_body(RequestBody::Json(value));
    let resolved = RuntimeResolver::default()
        .resolve_request(&json_request, &VariableStore::new())
        .unwrap();
    assert_eq!(
        resolved.body,
        Some(ResolvedRequestBody::Json(ResolvedValue::Array(vec![
            ResolvedValue::Integer(-1),
            ResolvedValue::Number(1.25),
            ResolvedValue::Boolean(true),
            ResolvedValue::Null,
            ResolvedValue::Object(IndexMap::from([(
                "text".into(),
                ResolvedValue::String("plain".into())
            )])),
        ])))
    );

    let binary =
        HttpRequestSpec::new(HttpMethod::POST, "/").with_body(RequestBody::Binary(vec![0, 1, 255]));
    let first = RuntimeResolver::default()
        .resolve_request(&binary, &VariableStore::new())
        .unwrap();
    let second = RuntimeResolver::default()
        .resolve_request(&binary, &VariableStore::new())
        .unwrap();
    assert_eq!(first.body, second.body);
    assert_eq!(
        first.body,
        Some(ResolvedRequestBody::Binary(vec![0, 1, 255].into()))
    );
}

#[test]
fn request_resolution_reports_the_first_error_in_declaration_order() {
    let mut request = HttpRequestSpec::new(HttpMethod::GET, "/${MISSING_PATH}");
    request
        .headers
        .insert("first".into(), Value::from("${MISSING_HEADER}"));
    request
        .query
        .insert("later".into(), Value::from("${MISSING_QUERY}"));

    assert_eq!(
        RuntimeResolver::default().resolve_request(&request, &VariableStore::new()),
        Err(RuntimeError::UndefinedVariable {
            name: name("MISSING_PATH"),
            location: ResolutionLocation::RequestPath,
        })
    );

    request.path = "/ok".into();
    assert_eq!(
        RuntimeResolver::default().resolve_request(&request, &VariableStore::new()),
        Err(RuntimeError::UndefinedVariable {
            name: name("MISSING_HEADER"),
            location: ResolutionLocation::RequestHeader,
        })
    );
}

#[test]
fn rejects_non_finite_numbers_and_values_beyond_the_recursive_limit() {
    let non_finite = HttpRequestSpec::new(HttpMethod::POST, "/")
        .with_body(RequestBody::Json(Value::Number(f64::NAN)));
    assert_eq!(
        RuntimeResolver::default().resolve_request(&non_finite, &VariableStore::new()),
        Err(RuntimeError::NonFiniteNumber)
    );

    let config = RuntimeConfig::new(1024, 1).unwrap();
    let deeply_nested = HttpRequestSpec::new(HttpMethod::POST, "/").with_body(RequestBody::Json(
        Value::Array(vec![Value::Array(vec![Value::Null])]),
    ));
    assert_eq!(
        RuntimeResolver::new(config).resolve_request(&deeply_nested, &VariableStore::new()),
        Err(RuntimeError::NestingLimitExceeded { limit: 1 })
    );
}

#[test]
fn resolves_headers_text_and_empty_expectations() {
    let variables = store([("VALUE", VariableValue::text("resolved"))]);
    let expectation = ResponseExpectation {
        status: Some(201),
        headers: IndexMap::from([
            ("x-exists".into(), HeaderAssertion::Exists),
            ("x-exact".into(), HeaderAssertion::Exact("${VALUE}".into())),
            (
                "x-contains".into(),
                HeaderAssertion::Contains("prefix-${VALUE}".into()),
            ),
        ]),
        body: Some(BodyAssertion::Text(TextAssertion::Contains(
            "${VALUE}".into(),
        ))),
    };
    let resolved = RuntimeResolver::default()
        .resolve_expectation(&expectation, &variables)
        .unwrap();
    assert_eq!(resolved.status, Some(201));
    assert_eq!(
        resolved.headers["x-exists"],
        ResolvedHeaderAssertion::Exists
    );
    assert_eq!(
        resolved.headers["x-exact"],
        ResolvedHeaderAssertion::Exact("resolved".into())
    );
    assert_eq!(
        resolved.headers["x-contains"],
        ResolvedHeaderAssertion::Contains("prefix-resolved".into())
    );
    assert_eq!(
        resolved.body,
        Some(ResolvedBodyAssertion::Text(
            ResolvedTextAssertion::Contains("resolved".into())
        ))
    );

    for body in [
        BodyAssertion::Empty,
        BodyAssertion::Text(TextAssertion::Exact("direct".into())),
    ] {
        let resolved = RuntimeResolver::default()
            .resolve_expectation(
                &ResponseExpectation {
                    body: Some(body),
                    ..ResponseExpectation::default()
                },
                &variables,
            )
            .unwrap();
        assert!(matches!(
            resolved.body,
            Some(ResolvedBodyAssertion::Empty)
                | Some(ResolvedBodyAssertion::Text(ResolvedTextAssertion::Exact(_)))
        ));
    }
}

#[test]
fn resolves_recursive_expected_json_and_retains_capture_metadata() {
    let variables = store([(
        "EXPECTED",
        VariableValue::json(json!({
            "id": u64::MAX,
            "ratio": 1.5,
            "active": true,
            "missing": null,
            "tags": ["one", "two"]
        })),
    )]);
    let capture = Capture::new(name("SAVED"));
    let mut nested = ObjectAssertion::exact();
    nested.insert(FieldAssertion::type_and_value(
        "payload",
        ExpectedType::Object,
        Value::String("${EXPECTED}".into()),
    ));
    let mut root = ObjectAssertion::partial();
    root.insert(
        FieldAssertion::type_only("data", ExpectedType::Object)
            .with_nested(nested)
            .with_capture(capture.clone()),
    );
    let expectation = ResponseExpectation {
        body: Some(BodyAssertion::Json(root)),
        ..ResponseExpectation::default()
    };

    let resolved = RuntimeResolver::default()
        .resolve_expectation(&expectation, &variables)
        .unwrap();
    let Some(ResolvedBodyAssertion::Json(root)) = resolved.body else {
        panic!("expectation should resolve to JSON");
    };
    let data = &root.fields["data"];
    assert_eq!(data.capture, Some(capture));
    let expected = data.nested.as_ref().unwrap().fields["payload"]
        .expected_value
        .as_ref()
        .unwrap();
    assert_eq!(
        expected,
        &json!({
            "id": u64::MAX,
            "ratio": 1.5,
            "active": true,
            "missing": null,
            "tags": ["one", "two"]
        })
    );
}

#[test]
fn resolves_every_literal_expected_json_value_without_changing_types() {
    let expected = Value::Object(IndexMap::from([
        ("text".into(), Value::from("direct")),
        ("integer".into(), Value::Integer(-2)),
        ("number".into(), Value::Number(2.5)),
        ("boolean".into(), Value::Boolean(false)),
        ("null".into(), Value::Null),
        (
            "array".into(),
            Value::Array(vec![Value::Integer(1), Value::from("two")]),
        ),
    ]));
    let mut object = ObjectAssertion::partial();
    object.insert(FieldAssertion::type_and_value(
        "value",
        ExpectedType::Object,
        expected,
    ));
    let expectation = ResponseExpectation {
        body: Some(BodyAssertion::Json(object)),
        ..ResponseExpectation::default()
    };
    let resolved = RuntimeResolver::default()
        .resolve_expectation(&expectation, &VariableStore::new())
        .unwrap();
    let Some(ResolvedBodyAssertion::Json(object)) = resolved.body else {
        panic!("expectation should resolve to JSON");
    };
    assert_eq!(
        object.fields["value"].expected_value,
        Some(json!({
            "text": "direct",
            "integer": -2,
            "number": 2.5,
            "boolean": false,
            "null": null,
            "array": [1, "two"]
        }))
    );

    let mut invalid = ObjectAssertion::partial();
    invalid.insert(FieldAssertion::type_and_value(
        "bad",
        ExpectedType::Number,
        Value::Number(f64::INFINITY),
    ));
    assert_eq!(
        RuntimeResolver::default().resolve_expectation(
            &ResponseExpectation {
                body: Some(BodyAssertion::Json(invalid)),
                ..ResponseExpectation::default()
            },
            &VariableStore::new(),
        ),
        Err(RuntimeError::NonFiniteNumber)
    );
}

#[test]
fn structured_values_are_rejected_in_expected_text_and_headers() {
    let variables = store([("OBJECT", VariableValue::json(json!({"id": 1})))]);
    let cases = [
        ResponseExpectation {
            headers: IndexMap::from([(
                "x-value".into(),
                HeaderAssertion::Exact("${OBJECT}".into()),
            )]),
            ..ResponseExpectation::default()
        },
        ResponseExpectation {
            body: Some(BodyAssertion::Text(TextAssertion::Exact(
                "${OBJECT}".into(),
            ))),
            ..ResponseExpectation::default()
        },
    ];
    let expected_locations = [
        ResolutionLocation::ExpectedHeader,
        ResolutionLocation::ExpectedText,
    ];

    for (expectation, location) in cases.into_iter().zip(expected_locations) {
        assert_eq!(
            RuntimeResolver::default().resolve_expectation(&expectation, &variables),
            Err(RuntimeError::UnsupportedInterpolationType {
                name: name("OBJECT"),
                value_type: "object",
                location,
            })
        );
    }
}

#[test]
fn expectation_resolution_honors_depth_and_first_error_order() {
    let mut nested = ObjectAssertion::partial();
    nested.insert(FieldAssertion::type_and_value(
        "leaf",
        ExpectedType::String,
        Value::from("${MISSING_LEAF}"),
    ));
    let mut root = ObjectAssertion::partial();
    root.insert(FieldAssertion::type_only("nested", ExpectedType::Object).with_nested(nested));
    let expectation = ResponseExpectation {
        headers: IndexMap::from([(
            "x-first".into(),
            HeaderAssertion::Exact("${MISSING_HEADER}".into()),
        )]),
        body: Some(BodyAssertion::Json(root.clone())),
        ..ResponseExpectation::default()
    };
    assert_eq!(
        RuntimeResolver::default().resolve_expectation(&expectation, &VariableStore::new()),
        Err(RuntimeError::UndefinedVariable {
            name: name("MISSING_HEADER"),
            location: ResolutionLocation::ExpectedHeader,
        })
    );

    let depth_limited = ResponseExpectation {
        body: Some(BodyAssertion::Json(root)),
        ..ResponseExpectation::default()
    };
    assert_eq!(
        RuntimeResolver::new(RuntimeConfig::new(1024, 1).unwrap())
            .resolve_expectation(&depth_limited, &VariableStore::new()),
        Err(RuntimeError::NestingLimitExceeded { limit: 1 })
    );

    let variables = store([("DEEP", VariableValue::json(json!({"child": null})))]);
    let mut captured = ObjectAssertion::partial();
    captured.insert(FieldAssertion::type_and_value(
        "deep",
        ExpectedType::Object,
        Value::from("${DEEP}"),
    ));
    assert_eq!(
        RuntimeResolver::new(RuntimeConfig::new(1024, 1).unwrap()).resolve_expectation(
            &ResponseExpectation {
                body: Some(BodyAssertion::Json(captured)),
                ..ResponseExpectation::default()
            },
            &variables,
        ),
        Err(RuntimeError::NestingLimitExceeded { limit: 1 })
    );
}
