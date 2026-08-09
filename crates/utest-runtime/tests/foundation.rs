use std::ffi::OsString;

use serde_json::json;
use utest_domain::{InterpolatedString, VariableName};
use utest_runtime::{
    DEFAULT_MAX_INTERPOLATED_BYTES, DEFAULT_MAX_RESOLUTION_DEPTH, HARD_MAX_INTERPOLATED_BYTES,
    HARD_MAX_RESOLUTION_DEPTH, Interpolator, ResolutionLocation, RuntimeConfig, RuntimeConfigError,
    RuntimeError, VariableAssignment, VariableAssignmentError, VariableStore, VariableValue,
};

fn name(value: &str) -> VariableName {
    VariableName::new(value).expect("test variable name should be valid")
}

fn assignment(value: &str) -> VariableAssignment {
    value
        .parse()
        .expect("test assignment should use valid NAME=VALUE syntax")
}

#[test]
fn runtime_config_defaults_and_accessors_are_stable() {
    let default = RuntimeConfig::default();
    assert_eq!(
        default.max_interpolated_bytes(),
        DEFAULT_MAX_INTERPOLATED_BYTES
    );
    assert_eq!(default.max_resolution_depth(), DEFAULT_MAX_RESOLUTION_DEPTH);

    let configured = RuntimeConfig::new(37, 11).expect("limits should be valid");
    assert_eq!(configured.max_interpolated_bytes(), 37);
    assert_eq!(configured.max_resolution_depth(), 11);
    assert_eq!(Interpolator::new(configured).config(), configured);
}

#[test]
fn runtime_config_rejects_every_invalid_boundary() {
    assert_eq!(
        RuntimeConfig::new(0, 1),
        Err(RuntimeConfigError::ZeroInterpolatedBytes)
    );
    assert_eq!(
        RuntimeConfig::new(HARD_MAX_INTERPOLATED_BYTES + 1, 1),
        Err(RuntimeConfigError::InterpolatedBytesTooLarge {
            requested: HARD_MAX_INTERPOLATED_BYTES + 1,
            maximum: HARD_MAX_INTERPOLATED_BYTES,
        })
    );
    assert_eq!(
        RuntimeConfig::new(1, 0),
        Err(RuntimeConfigError::ZeroResolutionDepth)
    );
    assert_eq!(
        RuntimeConfig::new(1, HARD_MAX_RESOLUTION_DEPTH + 1),
        Err(RuntimeConfigError::ResolutionDepthTooLarge {
            requested: HARD_MAX_RESOLUTION_DEPTH + 1,
            maximum: HARD_MAX_RESOLUTION_DEPTH,
        })
    );
    assert!(RuntimeConfig::new(HARD_MAX_INTERPOLATED_BYTES, HARD_MAX_RESOLUTION_DEPTH).is_ok());

    assert_eq!(
        RuntimeConfigError::ZeroInterpolatedBytes.to_string(),
        "maximum interpolated bytes must be greater than zero"
    );
    assert_eq!(
        RuntimeConfigError::InterpolatedBytesTooLarge {
            requested: 17,
            maximum: 16,
        }
        .to_string(),
        "maximum interpolated bytes 17 exceeds hard limit 16"
    );
    assert_eq!(
        RuntimeConfigError::ZeroResolutionDepth.to_string(),
        "maximum resolution depth must be greater than zero"
    );
    assert_eq!(
        RuntimeConfigError::ResolutionDepthTooLarge {
            requested: 9,
            maximum: 8,
        }
        .to_string(),
        "maximum resolution depth 9 exceeds hard limit 8"
    );
}

#[test]
fn assignments_split_once_and_accept_empty_or_equals_containing_values() {
    let empty = assignment("EMPTY=");
    assert_eq!(empty.name().as_str(), "EMPTY");
    assert_eq!(empty.value(), "");

    let token = assignment("TOKEN=left=right");
    assert_eq!(token.name().as_str(), "TOKEN");
    assert_eq!(token.value(), "left=right");

    let created = VariableAssignment::new(name("CREATED"), String::from("value"));
    assert_eq!(created.name().as_str(), "CREATED");
    assert_eq!(created.value(), "value");

    assert_eq!(
        "NO_SEPARATOR".parse::<VariableAssignment>(),
        Err(VariableAssignmentError::MissingEquals)
    );
    assert!(matches!(
        "9INVALID=value".parse::<VariableAssignment>(),
        Err(VariableAssignmentError::InvalidName(_))
    ));
}

#[test]
fn assignment_and_variable_debug_output_redacts_contents() {
    let secret = "do-not-print-this-secret";
    let assignment = VariableAssignment::new(name("TOKEN"), secret);
    let text = VariableValue::text(secret);
    let json = VariableValue::json(json!({"password": secret}));

    for rendered in [
        format!("{assignment:?}"),
        format!("{text:?}"),
        format!("{json:?}"),
    ] {
        assert!(rendered.contains("<redacted>"));
        assert!(!rendered.contains(secret));
    }
}

#[test]
fn variable_values_preserve_types_and_expose_only_matching_accessors() {
    let cases = [
        (VariableValue::json(json!(null)), "null"),
        (VariableValue::json(json!(true)), "boolean"),
        (VariableValue::json(json!(-1)), "integer"),
        (VariableValue::json(json!(u64::MAX)), "integer"),
        (VariableValue::json(json!(1.25)), "number"),
        (VariableValue::json(json!("captured")), "string"),
        (VariableValue::json(json!([1, 2])), "array"),
        (VariableValue::json(json!({"id": 1})), "object"),
    ];

    let text = VariableValue::text("provided");
    assert_eq!(text.type_name(), "string");
    assert_eq!(text.as_text(), Some("provided"));
    assert_eq!(text.as_json(), None);

    for (value, expected_type) in cases {
        assert_eq!(value.type_name(), expected_type);
        assert_eq!(value.as_text(), None);
        assert!(value.as_json().is_some());
    }
}

#[test]
fn environment_loading_skips_invalid_entries_and_cli_uses_last_value() {
    let mut store = VariableStore::new();
    assert!(store.is_empty());

    store.extend_environment([
        (OsString::from("FIRST"), OsString::from("environment")),
        (OsString::from("bad-name"), OsString::from("ignored")),
    ]);
    store.apply_cli([
        assignment("FIRST=cli"),
        assignment("SECOND=initial"),
        assignment("SECOND=final"),
    ]);

    assert_eq!(store.len(), 2);
    assert!(store.contains(&name("FIRST")));
    assert_eq!(
        store.get(&name("FIRST")).and_then(VariableValue::as_text),
        Some("cli")
    );
    assert_eq!(
        store.get(&name("SECOND")).and_then(VariableValue::as_text),
        Some("final")
    );
    assert_eq!(
        store.names().map(VariableName::as_str).collect::<Vec<_>>(),
        ["FIRST", "SECOND"]
    );

    let rendered = format!("{store:?}");
    assert!(rendered.contains("FIRST"));
    assert!(rendered.contains("SECOND"));
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("environment"));
    assert!(!rendered.contains("initial"));
    assert!(!rendered.contains("final"));
}

#[cfg(unix)]
#[test]
fn environment_loading_skips_non_unicode_names_and_values() {
    use std::os::unix::ffi::OsStringExt;

    let invalid_name = OsString::from_vec(vec![0xff]);
    let invalid_value = OsString::from_vec(vec![0xfe]);
    let mut store = VariableStore::new();
    store.extend_environment([
        (invalid_name, OsString::from("value")),
        (OsString::from("BAD_VALUE"), invalid_value),
        (OsString::from("VALID"), OsString::from("kept")),
    ]);

    assert_eq!(store.len(), 1);
    assert_eq!(
        store.get(&name("VALID")).and_then(VariableValue::as_text),
        Some("kept")
    );
}

#[test]
fn interpolation_supports_literals_repetition_and_all_scalar_cli_text() {
    let mut store = VariableStore::new();
    store.apply_cli([
        assignment("NAME=Ada"),
        assignment("EMPTY="),
        assignment("JSON_TEXT={\"id\":1}"),
    ]);
    let interpolator = Interpolator::default();

    assert_eq!(
        interpolator
            .interpolate(
                &InterpolatedString::new("hello ${NAME}/${NAME}${EMPTY}"),
                &store,
                ResolutionLocation::RequestPath,
            )
            .unwrap(),
        "hello Ada/Ada"
    );
    assert_eq!(
        interpolator
            .interpolate(
                &InterpolatedString::new("${JSON_TEXT}"),
                &store,
                ResolutionLocation::JsonRequestBody,
            )
            .unwrap(),
        "{\"id\":1}"
    );
    assert_eq!(
        interpolator
            .interpolate(
                &InterpolatedString::new("a literal closing brace } is retained"),
                &store,
                ResolutionLocation::RequestHeader,
            )
            .unwrap(),
        "a literal closing brace } is retained"
    );

    // Resolution borrows the scope and does not consume a value.
    assert_eq!(
        interpolator
            .interpolate(
                &InterpolatedString::new("${NAME}"),
                &store,
                ResolutionLocation::ExpectedText,
            )
            .unwrap(),
        "Ada"
    );
}

#[test]
fn interpolation_reports_malformed_and_missing_placeholders_without_values() {
    let mut store = VariableStore::new();
    store.apply_cli([assignment("SECRET=never-render-this")]);
    let interpolator = Interpolator::default();

    let cases = [
        (
            "${}",
            RuntimeError::EmptyPlaceholder {
                location: ResolutionLocation::QueryParameter,
            },
        ),
        (
            "${9bad}",
            RuntimeError::InvalidVariableName {
                name: "9bad".to_owned(),
                location: ResolutionLocation::QueryParameter,
            },
        ),
        (
            "${MISSING}",
            RuntimeError::UndefinedVariable {
                name: name("MISSING"),
                location: ResolutionLocation::QueryParameter,
            },
        ),
        (
            "prefix ${SECRET",
            RuntimeError::UnterminatedPlaceholder {
                location: ResolutionLocation::QueryParameter,
            },
        ),
    ];

    for (input, expected) in cases {
        let error = interpolator
            .interpolate(
                &InterpolatedString::new(input),
                &store,
                ResolutionLocation::QueryParameter,
            )
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!error.to_string().contains("never-render-this"));
    }
}

#[test]
fn interpolation_enforces_the_configured_utf8_byte_limit() {
    let config = RuntimeConfig::new(5, 1).expect("limits should be valid");
    let interpolator = Interpolator::new(config);
    let mut store = VariableStore::new();
    store.apply_cli([assignment("TWO=é")]);

    assert_eq!(
        interpolator
            .interpolate(
                &InterpolatedString::new("a${TWO}bc"),
                &store,
                ResolutionLocation::FormField,
            )
            .unwrap(),
        "aébc"
    );
    assert_eq!(
        interpolator.interpolate(
            &InterpolatedString::new("a${TWO}bcd"),
            &store,
            ResolutionLocation::FormField,
        ),
        Err(RuntimeError::InterpolatedValueTooLarge {
            location: ResolutionLocation::FormField,
            limit_bytes: 5,
        })
    );

    // An oversized literal follows the same bounded path without a placeholder.
    assert_eq!(
        interpolator.interpolate(
            &InterpolatedString::new("123456"),
            &store,
            ResolutionLocation::TextRequestBody,
        ),
        Err(RuntimeError::InterpolatedValueTooLarge {
            location: ResolutionLocation::TextRequestBody,
            limit_bytes: 5,
        })
    );

    // Oversized names are rejected before constructing an owned identifier.
    assert_eq!(
        interpolator.interpolate(
            &InterpolatedString::new("${TOO_LONG}"),
            &store,
            ResolutionLocation::FormField,
        ),
        Err(RuntimeError::InterpolatedValueTooLarge {
            location: ResolutionLocation::FormField,
            limit_bytes: 5,
        })
    );
}

#[test]
fn resolution_locations_have_stable_non_sensitive_names() {
    let cases = [
        (ResolutionLocation::RequestPath, "request path"),
        (ResolutionLocation::RequestHeader, "request header"),
        (ResolutionLocation::QueryParameter, "query parameter"),
        (ResolutionLocation::JsonRequestBody, "JSON request body"),
        (ResolutionLocation::TextRequestBody, "text request body"),
        (ResolutionLocation::FormField, "form field"),
        (
            ResolutionLocation::ExpectedHeader,
            "expected response header",
        ),
        (
            ResolutionLocation::ExpectedText,
            "expected text response body",
        ),
        (ResolutionLocation::ExpectedJson, "expected JSON value"),
    ];

    for (location, expected) in cases {
        assert_eq!(location.to_string(), expected);
    }
}

#[test]
fn runtime_error_messages_do_not_embed_variable_contents() {
    let errors = [
        RuntimeError::UnsupportedInterpolationType {
            name: name("OBJECT"),
            value_type: "object",
            location: ResolutionLocation::RequestHeader,
        },
        RuntimeError::NestingLimitExceeded { limit: 8 },
        RuntimeError::NonFiniteNumber,
        RuntimeError::InvalidCaptureBody,
        RuntimeError::MissingCaptureField {
            path: "$.id".to_owned(),
        },
        RuntimeError::InvalidNestedCaptureField {
            path: "$.user".to_owned(),
            actual_type: "array",
        },
        RuntimeError::DuplicateVariable {
            name: name("OBJECT"),
        },
    ];

    for error in errors {
        let rendered = error.to_string();
        assert!(!rendered.is_empty());
        assert!(!rendered.contains("do-not-print-this-secret"));
    }

    assert_eq!(
        VariableAssignmentError::MissingEquals.to_string(),
        "expected NAME=VALUE"
    );
}
