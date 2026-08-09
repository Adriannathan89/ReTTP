use std::time::Duration;

use bytes::Bytes;
use indexmap::IndexMap;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
    time::sleep,
};
use utest_domain::HttpMethod;
use utest_http::{
    HttpClient, HttpClientConfig, HttpError, ReqwestHttpClient, ResolvedHttpRequest,
    ResolvedRequestBody, ResolvedValue, ResponseBody,
};

struct TestServer {
    origin: String,
    request: JoinHandle<Vec<u8>>,
}

async fn serve(response: impl Into<Vec<u8>>, delay: Duration) -> TestServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let response = response.into();
    let request = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(5), async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            let mut expected_len = None;

            loop {
                let read = stream.read(&mut buffer).await.unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);

                if expected_len.is_none()
                    && let Some(header_end) = find_bytes(&request, b"\r\n\r\n")
                {
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    expected_len = Some(header_end + 4 + content_length);
                }

                if expected_len.is_some_and(|length| request.len() >= length) {
                    break;
                }
            }

            sleep(delay).await;
            stream.write_all(&response).await.unwrap();
            stream.shutdown().await.unwrap();
            request
        })
        .await
        .expect("local test server timed out")
    });

    TestServer {
        origin: format!("http://{address}"),
        request,
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn ok_response(body: &[u8]) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes()
    .into_iter()
    .chain(body.iter().copied())
    .collect()
}

fn client(base_url: &str) -> ReqwestHttpClient {
    ReqwestHttpClient::new(HttpClientConfig::new(base_url).unwrap()).unwrap()
}

fn request_text(request: &[u8]) -> &str {
    std::str::from_utf8(request).unwrap()
}

#[tokio::test]
async fn exposes_config_and_rejects_paths_outside_the_configured_base() {
    let config = HttpClientConfig::new("http://127.0.0.1:9/api")
        .unwrap()
        .with_default_timeout(Duration::from_millis(250))
        .unwrap()
        .with_max_response_body_bytes(512)
        .unwrap();
    let client = ReqwestHttpClient::new(config.clone()).unwrap();
    assert_eq!(client.config(), &config);

    for path in [
        "",
        "   ",
        " resource",
        "resource ",
        "//example.com/resource",
        r"resource\child",
        "https://example.com/resource",
        "resource?query=true",
        "resource#fragment",
        "resource\nnext",
        "../outside",
    ] {
        let error = client
            .execute(&ResolvedHttpRequest::new(HttpMethod::GET, path))
            .await
            .unwrap_err();
        assert!(
            matches!(error, HttpError::InvalidRequest { .. }),
            "path `{path:?}` produced {error:?}"
        );
    }
}

#[tokio::test]
async fn sends_all_methods_beneath_base_path_and_encodes_scalar_metadata() {
    let methods = [
        HttpMethod::GET,
        HttpMethod::POST,
        HttpMethod::PUT,
        HttpMethod::PATCH,
        HttpMethod::DELETE,
        HttpMethod::HEAD,
        HttpMethod::OPTIONS,
    ];

    for method in methods {
        let server = serve(ok_response(b""), Duration::ZERO).await;
        let client = client(&format!("{}/api", server.origin));
        let request = ResolvedHttpRequest::new(method, "/users/alice")
            .with_header("x-string", "direct value")
            .with_header("x-integer", -12_i64)
            .with_header("x-unsigned", u64::MAX)
            .with_header("x-number", 1.25_f64)
            .with_header("x-boolean", true)
            .with_query_param("text", "a value")
            .with_query_param("integer", -4_i64)
            .with_query_param("unsigned", u64::MAX)
            .with_query_param("number", 2.5_f64)
            .with_query_param("boolean", false)
            .with_query_param("nothing", ResolvedValue::Null);

        let response = client.execute(&request).await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, ResponseBody::Empty);

        let received = server.request.await.unwrap();
        let received = request_text(&received);
        assert!(received.starts_with(&format!(
            "{} /api/users/alice?text=a+value&integer=-4&unsigned={}&number=2.5&boolean=false&nothing=null HTTP/1.1\r\n",
            method.as_str(),
            u64::MAX,
        )));
        assert!(received.contains("x-string: direct value\r\n"));
        assert!(received.contains("x-integer: -12\r\n"));
        assert!(received.contains(&format!("x-unsigned: {}\r\n", u64::MAX)));
        assert!(received.contains("x-number: 1.25\r\n"));
        assert!(received.contains("x-boolean: true\r\n"));
    }
}

#[tokio::test]
async fn rejects_invalid_header_query_form_and_timeout_before_network_io() {
    let client = client("http://127.0.0.1:9/api");
    let complex = [
        ResolvedValue::Array(vec![]),
        ResolvedValue::Object(IndexMap::new()),
    ];

    for value in [
        ResolvedValue::Null,
        ResolvedValue::Number(f64::NAN),
        complex[0].clone(),
        complex[1].clone(),
    ] {
        let mut request = ResolvedHttpRequest::new(HttpMethod::GET, "resource");
        request.headers.insert("x-value".into(), value);
        assert!(matches!(
            client.execute(&request).await,
            Err(HttpError::InvalidRequest { .. })
        ));
    }

    for value in [
        ResolvedValue::Number(f64::INFINITY),
        complex[0].clone(),
        complex[1].clone(),
    ] {
        let mut request = ResolvedHttpRequest::new(HttpMethod::GET, "resource");
        request.query.insert("value".into(), value);
        assert!(matches!(
            client.execute(&request).await,
            Err(HttpError::InvalidRequest { .. })
        ));
    }

    for value in complex {
        let mut fields = IndexMap::new();
        fields.insert("value".into(), value);
        let request = ResolvedHttpRequest::new(HttpMethod::POST, "resource")
            .with_body(ResolvedRequestBody::FormData(fields));
        assert!(matches!(
            client.execute(&request).await,
            Err(HttpError::InvalidRequest { .. })
        ));
    }

    for (name, value) in [("bad header", "ok"), ("x-value", "bad\r\nvalue")] {
        let request =
            ResolvedHttpRequest::new(HttpMethod::GET, "resource").with_header(name, value);
        assert!(matches!(
            client.execute(&request).await,
            Err(HttpError::InvalidRequest { .. })
        ));
    }

    let mut duplicate = ResolvedHttpRequest::new(HttpMethod::GET, "resource");
    duplicate.headers.insert("X-Trace".into(), "one".into());
    duplicate.headers.insert("x-trace".into(), "two".into());
    assert!(matches!(
        client.execute(&duplicate).await,
        Err(HttpError::InvalidRequest { .. })
    ));

    let zero_timeout =
        ResolvedHttpRequest::new(HttpMethod::GET, "resource").with_timeout(Duration::ZERO);
    assert!(matches!(
        client.execute(&zero_timeout).await,
        Err(HttpError::InvalidRequest { .. })
    ));
}

#[tokio::test]
async fn encodes_all_request_bodies_and_respects_content_type_override() {
    let mut object = IndexMap::new();
    object.insert("string".into(), "value".into());
    object.insert("integer".into(), (-2_i64).into());
    object.insert("unsigned".into(), u64::MAX.into());
    object.insert("number".into(), 3.5_f64.into());
    object.insert("boolean".into(), true.into());
    object.insert("null".into(), ResolvedValue::Null);
    object.insert(
        "array".into(),
        ResolvedValue::Array(vec![ResolvedValue::Integer(1)]),
    );

    let mut form = IndexMap::new();
    form.insert("name".into(), "Ada Lovelace".into());
    form.insert("count".into(), 2_i64.into());
    form.insert("unsigned".into(), u64::MAX.into());
    form.insert("ratio".into(), 1.5_f64.into());
    form.insert("active".into(), false.into());
    form.insert("empty".into(), ResolvedValue::Null);

    let cases = [
        (
            ResolvedRequestBody::Json(ResolvedValue::Object(object)),
            "application/json",
            br#"{"string":"value","integer":-2,"unsigned":18446744073709551615,"number":3.5,"boolean":true,"null":null,"array":[1]}"#.as_slice(),
            None,
        ),
        (
            ResolvedRequestBody::Text("hello dunia".into()),
            "text/plain; charset=utf-8",
            b"hello dunia".as_slice(),
            None,
        ),
        (
            ResolvedRequestBody::FormData(form),
            "application/x-www-form-urlencoded",
            b"name=Ada+Lovelace&count=2&unsigned=18446744073709551615&ratio=1.5&active=false&empty=null".as_slice(),
            None,
        ),
        (
            ResolvedRequestBody::Binary(Bytes::from_static(&[0, 1, 255])),
            "application/octet-stream",
            &[0, 1, 255],
            Some("application/vnd.custom"),
        ),
    ];

    for (body, default_content_type, expected_body, override_content_type) in cases {
        let server = serve(ok_response(b""), Duration::ZERO).await;
        let mut request = ResolvedHttpRequest::new(HttpMethod::POST, "submit").with_body(body);
        if let Some(content_type) = override_content_type {
            request = request.with_header("content-type", content_type);
        }
        client(&server.origin).execute(&request).await.unwrap();

        let received = server.request.await.unwrap();
        let header_end = find_bytes(&received, b"\r\n\r\n").unwrap();
        let headers = request_text(&received[..header_end + 4]);
        let expected_content_type = override_content_type.unwrap_or(default_content_type);
        assert!(headers.contains(&format!("content-type: {expected_content_type}\r\n")));
        if matches!(request.body, Some(ResolvedRequestBody::Json(_))) {
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&received[header_end + 4..]).unwrap(),
                serde_json::from_slice::<serde_json::Value>(expected_body).unwrap()
            );
        } else {
            assert_eq!(&received[header_end + 4..], expected_body);
        }
    }
}

#[tokio::test]
async fn preserves_status_and_headers_and_classifies_response_bodies() {
    let cases: Vec<(Vec<u8>, u16, ResponseBody)> = vec![
        (
            b"HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: 11\r\nX-Repeat: one\r\nx-repeat: two\r\nX-Raw: \xff\r\nConnection: close\r\n\r\n{\"ok\":true}"
                .to_vec(),
            201,
            ResponseBody::Json {
                raw: Bytes::from_static(br#"{"ok":true}"#),
                value: serde_json::json!({"ok": true}),
            },
        ),
        (
            b"HTTP/1.1 404 Not Found\r\nContent-Type: application/problem+json; charset=utf-8\r\nContent-Length: 14\r\nConnection: close\r\n\r\n{\"error\":true}"
                .to_vec(),
            404,
            ResponseBody::Json {
                raw: Bytes::from_static(br#"{"error":true}"#),
                value: serde_json::json!({"error": true}),
            },
        ),
        (
            b"HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nContent-Length: 4\r\nConnection: close\r\n\r\noops"
                .to_vec(),
            500,
            ResponseBody::Text(Bytes::from_static(b"oops")),
        ),
        (
            b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 3\r\nConnection: close\r\n\r\n\0\x01\xff"
                .to_vec(),
            200,
            ResponseBody::Binary(Bytes::from_static(&[0, 1, 255])),
        ),
        (
            b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nraw".to_vec(),
            200,
            ResponseBody::Binary(Bytes::from_static(b"raw")),
        ),
        (
            b"HTTP/1.1 204 No Content\r\nContent-Type: application/json\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_vec(),
            204,
            ResponseBody::Empty,
        ),
    ];

    for (response, expected_status, expected_body) in cases {
        let server = serve(response, Duration::ZERO).await;
        let response = client(&server.origin)
            .execute(&ResolvedHttpRequest::new(HttpMethod::GET, "response"))
            .await
            .unwrap();

        assert_eq!(response.status, expected_status);
        assert_eq!(response.body, expected_body);
        if expected_status == 201 {
            assert_eq!(
                response.headers.get("X-REPEAT").unwrap(),
                [Bytes::from_static(b"one"), Bytes::from_static(b"two")]
            );
            assert!(
                response
                    .headers
                    .iter()
                    .any(|(name, values)| name == "x-repeat" && values.len() == 2)
            );
            assert_eq!(
                response.headers.get("x-raw").unwrap(),
                [Bytes::from_static(&[255])]
            );
        }
        server.request.await.unwrap();
    }
}

#[tokio::test]
async fn rejects_invalid_declared_response_representations() {
    let responses = [
        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 8\r\nConnection: close\r\n\r\nnot-json"
            .as_slice(),
        b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 1\r\nConnection: close\r\n\r\n\xff"
            .as_slice(),
        b"HTTP/1.1 200 OK\r\nContent-Type: \xff\r\nContent-Length: 1\r\nConnection: close\r\n\r\nx"
            .as_slice(),
        b"HTTP/1.1 200 OK\r\nContent-Type: not a mime\r\nContent-Length: 1\r\nConnection: close\r\n\r\nx"
            .as_slice(),
    ];

    for raw_response in responses {
        let server = serve(raw_response, Duration::ZERO).await;
        let error = client(&server.origin)
            .execute(&ResolvedHttpRequest::new(HttpMethod::GET, "invalid"))
            .await
            .unwrap_err();
        assert!(
            matches!(error, HttpError::InvalidResponse { .. }),
            "{error:?}"
        );
        server.request.await.unwrap();
    }
}

#[tokio::test]
async fn enforces_declared_and_streamed_response_limits() {
    let declared = serve(
        b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabcdef",
        Duration::ZERO,
    )
    .await;
    let config = HttpClientConfig::new(&declared.origin)
        .unwrap()
        .with_max_response_body_bytes(5)
        .unwrap();
    let error = ReqwestHttpClient::new(config)
        .unwrap()
        .execute(&ResolvedHttpRequest::new(HttpMethod::GET, "large"))
        .await
        .unwrap_err();
    assert_eq!(error, HttpError::BodyTooLarge { limit_bytes: 5 });
    declared.request.await.unwrap();

    let streamed = serve(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n3\r\ndef\r\n0\r\n\r\n",
        Duration::ZERO,
    )
    .await;
    let config = HttpClientConfig::new(&streamed.origin)
        .unwrap()
        .with_max_response_body_bytes(5)
        .unwrap();
    let error = ReqwestHttpClient::new(config)
        .unwrap()
        .execute(&ResolvedHttpRequest::new(HttpMethod::GET, "large"))
        .await
        .unwrap_err();
    assert_eq!(error, HttpError::BodyTooLarge { limit_bytes: 5 });
    streamed.request.await.unwrap();
}

#[tokio::test]
async fn maps_timeout_connection_and_truncated_response_errors() {
    let slow = serve(ok_response(b"late"), Duration::from_millis(200)).await;
    let request =
        ResolvedHttpRequest::new(HttpMethod::GET, "slow").with_timeout(Duration::from_millis(20));
    let error = client(&slow.origin).execute(&request).await.unwrap_err();
    assert!(matches!(error, HttpError::Timeout { .. }));
    slow.request.abort();

    let unused = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", unused.local_addr().unwrap());
    drop(unused);
    let error = client(&origin)
        .execute(&ResolvedHttpRequest::new(HttpMethod::GET, "missing"))
        .await
        .unwrap_err();
    assert!(matches!(error, HttpError::Connection { .. }), "{error:?}");
    assert!(!error.to_string().contains(&origin));

    let truncated = serve(
        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nab",
        Duration::ZERO,
    )
    .await;
    let error = client(&truncated.origin)
        .execute(&ResolvedHttpRequest::new(HttpMethod::GET, "truncated"))
        .await
        .unwrap_err();
    assert!(
        matches!(error, HttpError::InvalidResponse { .. }),
        "{error:?}"
    );
    truncated.request.await.unwrap();
}

#[tokio::test]
async fn does_not_follow_redirects() {
    let server = serve(
        b"HTTP/1.1 302 Found\r\nLocation: /destination\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        Duration::ZERO,
    )
    .await;
    let response = client(&server.origin)
        .execute(&ResolvedHttpRequest::new(HttpMethod::GET, "redirect"))
        .await
        .unwrap();

    assert_eq!(response.status, 302);
    assert_eq!(response.body, ResponseBody::Empty);
    assert!(request_text(&server.request.await.unwrap()).starts_with("GET /redirect "));
}

fn nested_array(levels: usize) -> ResolvedValue {
    (0..levels).fold(ResolvedValue::Null, |value, _| {
        ResolvedValue::Array(vec![value])
    })
}

fn nested_object(levels: usize) -> ResolvedValue {
    (0..levels).fold(ResolvedValue::Null, |value, _| {
        ResolvedValue::Object(IndexMap::from([("nested".to_owned(), value)]))
    })
}

#[tokio::test]
async fn accepts_json_at_depth_limit_and_rejects_deeper_json_before_io() {
    let server = serve(ok_response(b""), Duration::ZERO).await;
    let request = ResolvedHttpRequest::new(HttpMethod::POST, "json")
        .with_body(ResolvedRequestBody::Json(nested_array(256)));
    client(&server.origin).execute(&request).await.unwrap();
    server.request.await.unwrap();

    let request = ResolvedHttpRequest::new(HttpMethod::POST, "json")
        .with_body(ResolvedRequestBody::Json(nested_array(257)));
    let error = client("http://127.0.0.1:9")
        .execute(&request)
        .await
        .unwrap_err();
    assert!(matches!(error, HttpError::InvalidRequest { .. }));

    for invalid_json in [ResolvedValue::Number(f64::NAN), nested_object(257)] {
        let request = ResolvedHttpRequest::new(HttpMethod::POST, "json")
            .with_body(ResolvedRequestBody::Json(invalid_json));
        let error = client("http://127.0.0.1:9")
            .execute(&request)
            .await
            .unwrap_err();
        assert!(matches!(error, HttpError::InvalidRequest { .. }));
    }
}
