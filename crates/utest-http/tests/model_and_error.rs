use std::time::Duration;

use bytes::Bytes;
use indexmap::IndexMap;
use serde_json::json;
use utest_domain::HttpMethod;
use utest_http::{
    HttpError, HttpResponse, ResolvedHttpRequest, ResolvedRequestBody, ResolvedValue, ResponseBody,
    ResponseHeaders,
};

#[test]
fn resolved_value_supports_public_scalar_conversions() {
    assert_eq!(
        ResolvedValue::from("text"),
        ResolvedValue::String("text".into())
    );
    assert_eq!(
        ResolvedValue::from(String::from("owned")),
        ResolvedValue::String("owned".into())
    );
    assert_eq!(ResolvedValue::from(-42_i64), ResolvedValue::Integer(-42));
    assert_eq!(ResolvedValue::from(3.5_f64), ResolvedValue::Number(3.5));
    assert_eq!(ResolvedValue::from(true), ResolvedValue::Boolean(true));
}

#[test]
fn resolved_value_represents_nested_collections_and_null() {
    let mut object = IndexMap::new();
    object.insert("missing".into(), ResolvedValue::Null);
    object.insert(
        "items".into(),
        ResolvedValue::Array(vec![ResolvedValue::Integer(1)]),
    );

    let value = ResolvedValue::Object(object.clone());

    assert_eq!(value.clone(), ResolvedValue::Object(object));
    assert!(format!("{value:?}").contains("missing"));
}

#[test]
fn resolved_request_new_uses_empty_defaults() {
    let request = ResolvedHttpRequest::new(HttpMethod::GET, "/health");

    assert_eq!(request.method, HttpMethod::GET);
    assert_eq!(request.path, "/health");
    assert!(request.headers.is_empty());
    assert!(request.query.is_empty());
    assert_eq!(request.body, None);
    assert_eq!(request.timeout, None);
}

#[test]
fn resolved_request_builders_add_and_replace_values() {
    let timeout = Duration::from_millis(750);
    let request = ResolvedHttpRequest::new(HttpMethod::POST, String::from("/items"))
        .with_header("x-request-id", "first")
        .with_header("accept", "application/json")
        .with_header("x-request-id", "replacement")
        .with_query_param("page", 1_i64)
        .with_query_param("enabled", true)
        .with_query_param("page", 2_i64)
        .with_body(ResolvedRequestBody::Text("payload".into()))
        .with_timeout(timeout);

    assert_eq!(
        request
            .headers
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["x-request-id", "accept"]
    );
    assert_eq!(
        request.headers["x-request-id"],
        ResolvedValue::String("replacement".into())
    );
    assert_eq!(
        request.query.keys().map(String::as_str).collect::<Vec<_>>(),
        ["page", "enabled"]
    );
    assert_eq!(request.query["page"], ResolvedValue::Integer(2));
    assert_eq!(
        request.body,
        Some(ResolvedRequestBody::Text("payload".into()))
    );
    assert_eq!(request.timeout, Some(timeout));
}

#[test]
fn resolved_request_body_supports_every_public_representation() {
    let mut fields = IndexMap::new();
    fields.insert("name".into(), ResolvedValue::from("Ada"));

    let bodies = [
        ResolvedRequestBody::Json(ResolvedValue::Null),
        ResolvedRequestBody::Text("hello".into()),
        ResolvedRequestBody::FormData(fields),
        ResolvedRequestBody::Binary(Bytes::from_static(&[0, 1, 2])),
    ];

    assert_eq!(bodies.clone(), bodies);
    assert!(format!("{:?}", bodies[0]).contains("Json"));
}

#[test]
fn empty_response_headers_expose_consistent_collection_behavior() {
    let headers = ResponseHeaders::new();

    assert!(headers.is_empty());
    assert_eq!(headers.len(), 0);
    assert_eq!(headers.get("Content-Type"), None);

    let mut iter = headers.iter();
    assert_eq!(iter.size_hint(), (0, Some(0)));
    assert_eq!(iter.len(), 0);
    assert_eq!(iter.next(), None);

    assert_eq!((&headers).into_iter().next(), None);
    assert_eq!(headers.clone(), ResponseHeaders::default());
}

#[test]
fn response_headers_normalize_names_and_preserve_repeated_raw_values() {
    let mut headers = ResponseHeaders::new().with_header("Content-Type", "application/json");
    headers.append("X-Binary", Bytes::from_static(&[0xff, 0x00]));
    headers.append(String::from("content-type"), "text/plain");

    assert_eq!(headers.len(), 2);
    assert_eq!(
        headers.get("CONTENT-TYPE"),
        Some(
            [
                Bytes::from_static(b"application/json"),
                Bytes::from_static(b"text/plain"),
            ]
            .as_slice()
        )
    );
    assert_eq!(
        headers.get("x-binary"),
        Some([Bytes::from_static(&[0xff, 0x00])].as_slice())
    );
    assert_eq!(
        headers.iter().map(|(name, _)| name).collect::<Vec<_>>(),
        ["content-type", "x-binary"]
    );
}

#[test]
fn response_body_accessors_cover_every_classification() {
    let empty = ResponseBody::Empty;
    assert_eq!(empty.raw_bytes(), b"");
    assert_eq!(empty.as_json(), None);
    assert_eq!(empty.as_text(), None);

    let json_raw = Bytes::from_static(br#"{"ok":true}"#);
    let json_value = json!({"ok": true});
    let json_body = ResponseBody::Json {
        raw: json_raw.clone(),
        value: json_value.clone(),
    };
    assert_eq!(json_body.raw_bytes(), json_raw.as_ref());
    assert_eq!(json_body.as_json(), Some(&json_value));
    assert_eq!(json_body.as_text(), None);

    let text = ResponseBody::Text(Bytes::from_static(b"hello"));
    assert_eq!(text.raw_bytes(), b"hello");
    assert_eq!(text.as_json(), None);
    assert_eq!(text.as_text(), Some("hello"));

    let invalid_text = ResponseBody::Text(Bytes::from_static(&[0xff]));
    assert_eq!(invalid_text.as_text(), None);

    let binary = ResponseBody::Binary(Bytes::from_static(&[0, 1, 2]));
    assert_eq!(binary.raw_bytes(), &[0, 1, 2]);
    assert_eq!(binary.as_json(), None);
    assert_eq!(binary.as_text(), None);
}

#[test]
fn http_response_keeps_status_headers_and_body() {
    let response = HttpResponse {
        status: 404,
        headers: ResponseHeaders::default(),
        body: ResponseBody::Binary(Bytes::from_static(b"not found")),
    };

    assert_eq!(response.status, 404);
    assert!(response.headers.is_empty());
    assert_eq!(response.body.raw_bytes(), b"not found");
    assert_eq!(response.raw_body(), b"not found");
    assert_eq!(response.clone(), response);
}

#[test]
fn http_errors_expose_stable_messages_and_public_payloads() {
    let cases = [
        (
            HttpError::InvalidBaseUrl {
                reason: "credentials are forbidden".into(),
            },
            "invalid HTTP base URL: credentials are forbidden",
        ),
        (
            HttpError::InvalidRequest {
                reason: "header is invalid".into(),
            },
            "invalid HTTP request: header is invalid",
        ),
        (
            HttpError::Connection {
                message: "connection refused".into(),
            },
            "HTTP connection failed: connection refused",
        ),
        (
            HttpError::Timeout {
                message: "deadline elapsed".into(),
            },
            "HTTP request timed out: deadline elapsed",
        ),
        (
            HttpError::InvalidResponse {
                reason: "malformed JSON".into(),
            },
            "invalid HTTP response: malformed JSON",
        ),
        (
            HttpError::BodyTooLarge { limit_bytes: 1024 },
            "HTTP response body exceeds the 1024-byte limit",
        ),
    ];

    for (error, expected) in cases {
        assert_eq!(error.to_string(), expected);
        assert_eq!(error.clone(), error);
        assert!(!format!("{error:?}").is_empty());
    }
}
