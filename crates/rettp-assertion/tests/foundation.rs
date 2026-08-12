use indexmap::IndexMap;
use rettp_assertion::{
    AssertionConfig, AssertionConfigError, AssertionEngine, DEFAULT_MAX_FAILURES,
    DEFAULT_MAX_JSON_DEPTH, HARD_MAX_JSON_DEPTH, ResolvedBodyAssertion, ResolvedFieldAssertion,
    ResolvedHeaderAssertion, ResolvedObjectAssertion, ResolvedResponseExpectation,
    ResolvedTextAssertion,
};
use rettp_domain::{Capture, ExpectedType, ObjectMatchMode, VariableName};
use serde_json::json;

#[test]
fn default_config_uses_public_resource_limits() {
    let config = AssertionConfig::default();

    assert_eq!(config.max_failures(), DEFAULT_MAX_FAILURES);
    assert_eq!(config.max_json_depth(), DEFAULT_MAX_JSON_DEPTH);
}

#[test]
fn config_accepts_boundary_values() {
    let minimum = AssertionConfig::new(1, 1).expect("minimum limits should be valid");
    assert_eq!(minimum.max_failures(), 1);
    assert_eq!(minimum.max_json_depth(), 1);

    let maximum = AssertionConfig::new(usize::MAX, HARD_MAX_JSON_DEPTH)
        .expect("failure retention is not recursion and may use usize::MAX");
    assert_eq!(maximum.max_failures(), usize::MAX);
    assert_eq!(maximum.max_json_depth(), HARD_MAX_JSON_DEPTH);
}

#[test]
fn config_rejects_every_invalid_limit() {
    assert_eq!(
        AssertionConfig::new(0, 1),
        Err(AssertionConfigError::ZeroMaxFailures)
    );
    assert_eq!(
        AssertionConfig::new(1, 0),
        Err(AssertionConfigError::ZeroMaxJsonDepth)
    );

    let requested = HARD_MAX_JSON_DEPTH + 1;
    assert_eq!(
        AssertionConfig::new(1, requested),
        Err(AssertionConfigError::JsonDepthExceedsHardLimit {
            requested,
            hard_limit: HARD_MAX_JSON_DEPTH,
        })
    );
}

#[test]
fn config_errors_have_actionable_messages() {
    assert_eq!(
        AssertionConfigError::ZeroMaxFailures.to_string(),
        "maximum assertion failures must be greater than zero"
    );
    assert_eq!(
        AssertionConfigError::ZeroMaxJsonDepth.to_string(),
        "maximum JSON depth must be greater than zero"
    );
    assert_eq!(
        AssertionConfigError::JsonDepthExceedsHardLimit {
            requested: 257,
            hard_limit: 256,
        }
        .to_string(),
        "maximum JSON depth 257 exceeds hard limit 256"
    );
}

#[test]
fn engine_preserves_explicit_and_default_configs() {
    let config = AssertionConfig::new(7, 9).expect("test limits should be valid");
    let engine = AssertionEngine::new(config);
    assert_eq!(engine.config(), config);

    let default_engine = AssertionEngine::default();
    assert_eq!(default_engine.config(), AssertionConfig::default());
    assert_eq!(default_engine, default_engine.clone());
}

#[test]
fn response_expectation_defaults_to_no_requirements() {
    let expectation = ResolvedResponseExpectation::default();

    assert_eq!(expectation.status, None);
    assert!(expectation.headers.is_empty());
    assert_eq!(expectation.body, None);
}

#[test]
fn resolved_variants_retain_owned_values() {
    let headers = IndexMap::from([
        ("x-exists".to_owned(), ResolvedHeaderAssertion::Exists),
        (
            "x-exact".to_owned(),
            ResolvedHeaderAssertion::Exact("application/json".to_owned()),
        ),
        (
            "x-contains".to_owned(),
            ResolvedHeaderAssertion::Contains("json".to_owned()),
        ),
    ]);
    let text = ResolvedBodyAssertion::Text(ResolvedTextAssertion::Contains("ready".to_owned()));
    let expectation = ResolvedResponseExpectation {
        status: Some(200),
        headers,
        body: Some(text),
    };

    assert_eq!(expectation.status, Some(200));
    assert_eq!(expectation.headers.len(), 3);
    assert_eq!(
        expectation.body,
        Some(ResolvedBodyAssertion::Text(
            ResolvedTextAssertion::Contains("ready".to_owned())
        ))
    );

    assert_eq!(
        ResolvedTextAssertion::Exact("done".to_owned()),
        ResolvedTextAssertion::Exact("done".to_owned())
    );
    assert_eq!(ResolvedBodyAssertion::Empty, ResolvedBodyAssertion::Empty);
}

#[test]
fn object_factories_select_match_mode_and_start_empty() {
    let partial = ResolvedObjectAssertion::partial();
    assert_eq!(partial.mode, ObjectMatchMode::Partial);
    assert!(partial.fields.is_empty());

    let exact = ResolvedObjectAssertion::exact();
    assert_eq!(exact.mode, ObjectMatchMode::Exact);
    assert!(exact.fields.is_empty());
}

#[test]
fn object_insert_uses_field_name_and_replaces_existing_assertion() {
    let mut object = ResolvedObjectAssertion::partial();
    object.insert(ResolvedFieldAssertion::type_only(
        "id",
        ExpectedType::Integer,
    ));
    object.insert(ResolvedFieldAssertion::type_and_value(
        "id",
        ExpectedType::Integer,
        json!(42),
    ));

    assert_eq!(object.fields.len(), 1);
    let assertion = object.fields.get("id").expect("id should be indexed");
    assert_eq!(assertion.field_name, "id");
    assert_eq!(assertion.expected_type, ExpectedType::Integer);
    assert_eq!(assertion.expected_value, Some(json!(42)));
}

#[test]
fn field_builders_compose_value_nested_assertions_and_capture() {
    let capture = Capture::new(VariableName::new("saved_id").expect("valid variable name"));
    let mut nested = ResolvedObjectAssertion::exact();
    nested.insert(ResolvedFieldAssertion::type_only(
        "active",
        ExpectedType::Boolean,
    ));

    let assertion = ResolvedFieldAssertion::type_and_value(
        "profile",
        ExpectedType::Object,
        json!({"active": true}),
    )
    .with_nested(nested.clone())
    .with_capture(capture.clone());

    assert_eq!(assertion.field_name, "profile");
    assert_eq!(assertion.expected_type, ExpectedType::Object);
    assert_eq!(assertion.expected_value, Some(json!({"active": true})));
    assert_eq!(assertion.nested, Some(nested));
    assert_eq!(assertion.capture, Some(capture));
}

#[test]
fn type_only_field_leaves_optional_requirements_unset() {
    let assertion = ResolvedFieldAssertion::type_only("payload", ExpectedType::Null);

    assert_eq!(assertion.field_name, "payload");
    assert_eq!(assertion.expected_type, ExpectedType::Null);
    assert_eq!(assertion.expected_value, None);
    assert_eq!(assertion.nested, None);
    assert_eq!(assertion.capture, None);
}

#[test]
fn json_body_variant_retains_object_assertion() {
    let mut object = ResolvedObjectAssertion::partial();
    object.insert(ResolvedFieldAssertion::type_only(
        "items",
        ExpectedType::Array,
    ));

    let body = ResolvedBodyAssertion::Json(object.clone());
    assert_eq!(body, ResolvedBodyAssertion::Json(object));
}
