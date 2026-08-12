//! `reqwest` implementation of the HTTP client port.

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use indexmap::IndexMap;
use reqwest::{
    Client, Method, Response, Url,
    header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
    redirect::Policy,
};
use rettp_domain::HttpMethod;
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};

use crate::{
    HttpClient, HttpClientConfig, HttpError, HttpResponse, ResolvedHttpRequest,
    ResolvedRequestBody, ResolvedValue, ResponseBody, ResponseHeaders,
};

const MAX_JSON_DEPTH: usize = 256;

/// A reusable [`HttpClient`] backed by `reqwest` and rustls.
///
/// Clones share reqwest's internal connection pool. Redirects and automatic
/// system proxies are disabled, and every response body is bounded by
/// [`HttpClientConfig`].
#[derive(Debug, Clone)]
pub struct ReqwestHttpClient {
    client: Client,
    config: HttpClientConfig,
}

impl ReqwestHttpClient {
    /// Builds an adapter from validated client configuration.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError`] if the rustls-backed reqwest client cannot be
    /// initialized from the supplied configuration.
    pub fn new(config: HttpClientConfig) -> Result<Self, HttpError> {
        let client = Client::builder()
            .tls_backend_rustls()
            .redirect(Policy::none())
            .no_proxy()
            .timeout(config.default_timeout())
            .build()
            .map_err(HttpError::from_reqwest)?;

        Ok(Self { client, config })
    }

    /// Returns the immutable configuration used by this adapter.
    #[must_use]
    pub const fn config(&self) -> &HttpClientConfig {
        &self.config
    }
}

#[async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn execute(&self, request: &ResolvedHttpRequest) -> Result<HttpResponse, HttpError> {
        if request.timeout.is_some_and(|timeout| timeout.is_zero()) {
            return Err(HttpError::invalid_request(
                "the request timeout must be greater than zero",
            ));
        }

        let mut url = self.config.resolve_url(&request.path)?;
        append_query(&mut url, &request.query)?;

        let method = reqwest_method(request.method);
        let mut headers = encode_headers(&request.headers)?;
        let encoded_body = request.body.as_ref().map(encode_body).transpose()?;

        if let Some(body) = &encoded_body
            && !headers.contains_key(CONTENT_TYPE)
        {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static(body.content_type));
        }

        let mut builder = self.client.request(method, url).headers(headers);
        if let Some(timeout) = request.timeout {
            builder = builder.timeout(timeout);
        }
        if let Some(body) = encoded_body {
            builder = builder.body(body.bytes);
        }

        let response = builder.send().await.map_err(HttpError::from_reqwest)?;
        decode_response(response, self.config.max_response_body_bytes()).await
    }
}

const fn reqwest_method(method: HttpMethod) -> Method {
    match method {
        HttpMethod::GET => Method::GET,
        HttpMethod::POST => Method::POST,
        HttpMethod::PUT => Method::PUT,
        HttpMethod::PATCH => Method::PATCH,
        HttpMethod::DELETE => Method::DELETE,
        HttpMethod::HEAD => Method::HEAD,
        HttpMethod::OPTIONS => Method::OPTIONS,
    }
}

struct EncodedBody {
    bytes: Bytes,
    content_type: &'static str,
}

fn encode_headers(values: &IndexMap<String, ResolvedValue>) -> Result<HeaderMap, HttpError> {
    let mut headers = HeaderMap::with_capacity(values.len());

    for (raw_name, value) in values {
        let name = HeaderName::from_bytes(raw_name.as_bytes())
            .map_err(|error| HttpError::invalid_request(error.to_string()))?;
        if headers.contains_key(&name) {
            return Err(HttpError::invalid_request(format!(
                "duplicate case-insensitive request header `{raw_name}`"
            )));
        }

        let value = wire_scalar(value, false, "header")?;
        let value = HeaderValue::from_str(&value)
            .map_err(|error| HttpError::invalid_request(error.to_string()))?;
        headers.insert(name, value);
    }

    Ok(headers)
}

fn append_query(url: &mut Url, values: &IndexMap<String, ResolvedValue>) -> Result<(), HttpError> {
    if values.is_empty() {
        return Ok(());
    }

    let mut query = url.query_pairs_mut();
    for (name, value) in values {
        let value = wire_scalar(value, true, "query parameter")?;
        query.append_pair(name, &value);
    }
    drop(query);
    Ok(())
}

fn encode_body(body: &ResolvedRequestBody) -> Result<EncodedBody, HttpError> {
    match body {
        ResolvedRequestBody::Json(value) => {
            let value = to_json(value, 0)?;
            let bytes = serde_json::to_vec(&value)
                .map_err(|error| HttpError::invalid_request(error.to_string()))?;
            Ok(EncodedBody {
                bytes: Bytes::from(bytes),
                content_type: "application/json",
            })
        }
        ResolvedRequestBody::Text(value) => Ok(EncodedBody {
            bytes: Bytes::copy_from_slice(value.as_bytes()),
            content_type: "text/plain; charset=utf-8",
        }),
        ResolvedRequestBody::FormData(values) => {
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            for (name, value) in values {
                let value = wire_scalar(value, true, "form field")?;
                serializer.append_pair(name, &value);
            }
            Ok(EncodedBody {
                bytes: Bytes::from(serializer.finish()),
                content_type: "application/x-www-form-urlencoded",
            })
        }
        ResolvedRequestBody::Binary(value) => Ok(EncodedBody {
            bytes: value.clone(),
            content_type: "application/octet-stream",
        }),
    }
}

fn wire_scalar(
    value: &ResolvedValue,
    allow_null: bool,
    location: &str,
) -> Result<String, HttpError> {
    match value {
        ResolvedValue::String(value) => Ok(value.clone()),
        ResolvedValue::Integer(value) => Ok(value.to_string()),
        ResolvedValue::UnsignedInteger(value) => Ok(value.to_string()),
        ResolvedValue::Number(value) if value.is_finite() => Ok(value.to_string()),
        ResolvedValue::Number(_) => Err(HttpError::invalid_request(format!(
            "a {location} number must be finite"
        ))),
        ResolvedValue::Boolean(value) => Ok(value.to_string()),
        ResolvedValue::Null if allow_null => Ok("null".to_owned()),
        ResolvedValue::Null => Err(HttpError::invalid_request(format!(
            "null is not supported in a {location}"
        ))),
        ResolvedValue::Array(_) | ResolvedValue::Object(_) => Err(HttpError::invalid_request(
            format!("complex values are not supported in a {location}"),
        )),
    }
}

fn to_json(value: &ResolvedValue, depth: usize) -> Result<JsonValue, HttpError> {
    match value {
        ResolvedValue::String(value) => Ok(JsonValue::String(value.clone())),
        ResolvedValue::Integer(value) => Ok(JsonValue::Number((*value).into())),
        ResolvedValue::UnsignedInteger(value) => Ok(JsonValue::Number((*value).into())),
        ResolvedValue::Number(value) => JsonNumber::from_f64(*value)
            .map(JsonValue::Number)
            .ok_or_else(|| HttpError::invalid_request("a JSON number must be finite")),
        ResolvedValue::Boolean(value) => Ok(JsonValue::Bool(*value)),
        ResolvedValue::Null => Ok(JsonValue::Null),
        ResolvedValue::Array(values) => {
            ensure_json_depth(depth, values.is_empty())?;
            values
                .iter()
                .map(|value| to_json(value, depth + 1))
                .collect::<Result<Vec<_>, _>>()
                .map(JsonValue::Array)
        }
        ResolvedValue::Object(values) => {
            ensure_json_depth(depth, values.is_empty())?;
            values
                .iter()
                .map(|(name, value)| Ok((name.clone(), to_json(value, depth + 1)?)))
                .collect::<Result<JsonMap<_, _>, HttpError>>()
                .map(JsonValue::Object)
        }
    }
}

fn ensure_json_depth(depth: usize, is_empty: bool) -> Result<(), HttpError> {
    if !is_empty && depth >= MAX_JSON_DEPTH {
        Err(HttpError::invalid_request(format!(
            "JSON nesting exceeds the {MAX_JSON_DEPTH}-level limit"
        )))
    } else {
        Ok(())
    }
}

async fn decode_response(
    response: Response,
    max_body_bytes: usize,
) -> Result<HttpResponse, HttpError> {
    let status = response.status().as_u16();
    let headers = response_headers(response.headers());
    let content_type = response.headers().get(CONTENT_TYPE).cloned();
    let body = read_body(response, max_body_bytes).await?;
    let body = classify_body(content_type.as_ref(), body)?;

    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

fn response_headers(headers: &HeaderMap) -> ResponseHeaders {
    let mut response_headers = ResponseHeaders::default();
    for (name, value) in headers {
        response_headers.append(name.as_str(), Bytes::copy_from_slice(value.as_bytes()));
    }
    response_headers
}

async fn read_body(mut response: Response, limit: usize) -> Result<Bytes, HttpError> {
    let limit_u64 = u64::try_from(limit).unwrap_or(u64::MAX);
    if response
        .content_length()
        .is_some_and(|content_length| content_length > limit_u64)
    {
        return Err(HttpError::BodyTooLarge { limit_bytes: limit });
    }

    let initial_capacity = response
        .content_length()
        .and_then(|length| usize::try_from(length).ok())
        .map_or(0, |length| length.min(limit));
    let mut body = BytesMut::with_capacity(initial_capacity);

    while let Some(chunk) = response.chunk().await.map_err(HttpError::from_reqwest)? {
        if chunk.len() > limit.saturating_sub(body.len()) {
            return Err(HttpError::BodyTooLarge { limit_bytes: limit });
        }
        body.extend_from_slice(&chunk);
    }

    Ok(body.freeze())
}

fn classify_body(
    content_type: Option<&HeaderValue>,
    body: Bytes,
) -> Result<ResponseBody, HttpError> {
    if body.is_empty() {
        return Ok(ResponseBody::Empty);
    }

    let Some(content_type) = content_type else {
        return Ok(ResponseBody::Binary(body));
    };
    let content_type = content_type
        .to_str()
        .map_err(|error| HttpError::invalid_response(error.to_string()))?
        .parse::<mime::Mime>()
        .map_err(|error| HttpError::invalid_response(error.to_string()))?;

    let is_json = content_type.type_() == mime::APPLICATION
        && (content_type.subtype() == mime::JSON || content_type.suffix() == Some(mime::JSON));
    if is_json {
        let value = serde_json::from_slice(&body)
            .map_err(|error| HttpError::invalid_response(error.to_string()))?;
        return Ok(ResponseBody::Json { raw: body, value });
    }

    if content_type.type_() == mime::TEXT {
        std::str::from_utf8(&body)
            .map_err(|error| HttpError::invalid_response(error.to_string()))?;
        return Ok(ResponseBody::Text(body));
    }

    Ok(ResponseBody::Binary(body))
}
