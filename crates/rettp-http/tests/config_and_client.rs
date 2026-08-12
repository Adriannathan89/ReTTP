use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use rettp_domain::HttpMethod;
use rettp_http::{
    DEFAULT_MAX_RESPONSE_BODY_BYTES, DEFAULT_REQUEST_TIMEOUT, HttpClient, HttpClientConfig,
    HttpError, HttpResponse, ResolvedHttpRequest, ResponseBody, ResponseHeaders,
};

fn assert_invalid_base_url(input: &str) {
    assert!(
        matches!(
            HttpClientConfig::new(input),
            Err(HttpError::InvalidBaseUrl { .. })
        ),
        "expected `{input}` to be rejected as an invalid base URL"
    );
}

#[test]
fn config_uses_documented_defaults_and_normalizes_base_path() {
    let config = HttpClientConfig::new("https://example.com/api").unwrap();

    assert_eq!(config.base_url().as_str(), "https://example.com/api/");
    assert_eq!(config.default_timeout(), DEFAULT_REQUEST_TIMEOUT);
    assert_eq!(config.default_timeout(), Duration::from_secs(30));
    assert_eq!(
        config.max_response_body_bytes(),
        DEFAULT_MAX_RESPONSE_BODY_BYTES
    );
    assert_eq!(
        config.max_response_body_bytes(),
        10 * 1024 * 1024,
        "the default limit is ten mebibytes"
    );
}

#[test]
fn config_preserves_an_existing_trailing_slash() {
    let config = HttpClientConfig::new("http://localhost:8080/nested/").unwrap();

    assert_eq!(config.base_url().as_str(), "http://localhost:8080/nested/");
}

#[test]
fn config_accepts_positive_timeout_and_body_limit_overrides() {
    let timeout = Duration::from_millis(250);
    let config = HttpClientConfig::new("https://example.com")
        .unwrap()
        .with_default_timeout(timeout)
        .unwrap()
        .with_max_response_body_bytes(4096)
        .unwrap();

    assert_eq!(config.default_timeout(), timeout);
    assert_eq!(config.max_response_body_bytes(), 4096);
}

#[test]
fn config_rejects_zero_timeout_and_body_limit() {
    let zero_timeout = HttpClientConfig::new("https://example.com")
        .unwrap()
        .with_default_timeout(Duration::ZERO);
    let zero_limit = HttpClientConfig::new("https://example.com")
        .unwrap()
        .with_max_response_body_bytes(0);

    assert!(matches!(
        zero_timeout,
        Err(HttpError::InvalidRequest { .. })
    ));
    assert!(matches!(zero_limit, Err(HttpError::InvalidRequest { .. })));
}

#[test]
fn config_rejects_malformed_or_unsupported_base_urls() {
    for input in [
        "not a URL",
        "ftp://example.com/api",
        "mailto:user@example.com",
        "http:///api",
        " https://example.com",
        "https://example.com\n",
    ] {
        assert_invalid_base_url(input);
    }
}

#[test]
fn config_rejects_credentials_query_and_fragment() {
    for input in [
        "https://user@example.com/api",
        "https://user:password@example.com/api",
        "https://example.com/api?debug=true",
        "https://example.com/api#section",
    ] {
        assert_invalid_base_url(input);
    }
}

#[derive(Debug)]
struct CapturingClient {
    received: Mutex<Vec<ResolvedHttpRequest>>,
    response: HttpResponse,
}

#[async_trait]
impl HttpClient for CapturingClient {
    async fn execute(&self, request: &ResolvedHttpRequest) -> Result<HttpResponse, HttpError> {
        self.received.lock().unwrap().push(request.clone());
        Ok(self.response.clone())
    }
}

fn assert_object_safe(_: &dyn HttpClient) {}

#[tokio::test]
async fn http_client_supports_fake_dynamic_dispatch() {
    let fake = Arc::new(CapturingClient {
        received: Mutex::new(Vec::new()),
        response: HttpResponse {
            status: 204,
            headers: ResponseHeaders::default(),
            body: ResponseBody::Empty,
        },
    });
    assert_object_safe(fake.as_ref());

    let client: Arc<dyn HttpClient> = fake.clone();
    let request = ResolvedHttpRequest::new(HttpMethod::GET, "/health")
        .with_header("x-trace-id", "trace-1")
        .with_query_param("verbose", true)
        .with_timeout(Duration::from_secs(1));

    let response = client.execute(&request).await.unwrap();

    assert_eq!(response.status, 204);
    assert!(response.headers.is_empty());
    assert_eq!(response.body, ResponseBody::Empty);
    assert_eq!(fake.received.lock().unwrap().as_slice(), &[request]);
}
