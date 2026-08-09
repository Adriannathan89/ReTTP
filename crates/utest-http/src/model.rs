//! Fully resolved request values and backend-neutral HTTP responses.

use std::{str, time::Duration};

use bytes::Bytes;
use indexmap::{IndexMap, map};
use serde_json::Value as JsonValue;
use utest_domain::HttpMethod;

/// A fully resolved value that cannot contain interpolation placeholders.
///
/// Runtime code converts domain [`utest_domain::Value`] values into this type
/// after substituting every variable. HTTP wire locations apply additional
/// restrictions; for example, objects are valid JSON values but invalid header
/// values.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedValue {
    /// Resolved UTF-8 text.
    String(String),
    /// Signed 64-bit integer.
    Integer(i64),
    /// Double-precision number. The adapter rejects non-finite values.
    Number(f64),
    /// Boolean value.
    Boolean(bool),
    /// Explicit null value.
    Null,
    /// Ordered collection of resolved values.
    Array(Vec<ResolvedValue>),
    /// Insertion-ordered object of resolved values.
    Object(IndexMap<String, ResolvedValue>),
}

impl From<String> for ResolvedValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for ResolvedValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<i64> for ResolvedValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<f64> for ResolvedValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<bool> for ResolvedValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

/// An HTTP request whose interpolation has already been resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedHttpRequest {
    /// HTTP method sent on the wire.
    pub method: HttpMethod,
    /// Relative path beneath the adapter's configured base URL.
    pub path: String,
    /// Insertion-ordered request headers.
    pub headers: IndexMap<String, ResolvedValue>,
    /// Insertion-ordered query parameters.
    pub query: IndexMap<String, ResolvedValue>,
    /// Optional request body.
    pub body: Option<ResolvedRequestBody>,
    /// Optional non-zero timeout overriding the client default.
    pub timeout: Option<Duration>,
}

impl ResolvedHttpRequest {
    /// Creates a request without headers, query parameters, body, or timeout.
    #[must_use]
    pub fn new(method: HttpMethod, path: impl Into<String>) -> Self {
        Self {
            method,
            path: path.into(),
            headers: IndexMap::new(),
            query: IndexMap::new(),
            body: None,
            timeout: None,
        }
    }

    /// Adds or replaces a resolved request header.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<ResolvedValue>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    /// Adds or replaces a resolved query parameter.
    #[must_use]
    pub fn with_query_param(
        mut self,
        name: impl Into<String>,
        value: impl Into<ResolvedValue>,
    ) -> Self {
        self.query.insert(name.into(), value.into());
        self
    }

    /// Sets or replaces the request body.
    #[must_use]
    pub fn with_body(mut self, body: ResolvedRequestBody) -> Self {
        self.body = Some(body);
        self
    }

    /// Sets a per-request timeout.
    ///
    /// A zero duration is retained here for ergonomic construction but is
    /// rejected by [`HttpClient::execute`](crate::HttpClient::execute).
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

/// Fully resolved content sent as an HTTP request body.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedRequestBody {
    /// JSON-compatible resolved data.
    Json(ResolvedValue),
    /// UTF-8 plain text.
    Text(String),
    /// Fields encoded as `application/x-www-form-urlencoded`.
    FormData(IndexMap<String, ResolvedValue>),
    /// Uninterpreted bytes.
    Binary(Bytes),
}

/// Case-insensitive response headers retaining every value for repeated names.
///
/// Header values remain bytes because valid HTTP header values are not
/// required to be UTF-8. Names are stored in canonical lowercase form.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResponseHeaders {
    entries: IndexMap<String, Vec<Bytes>>,
}

impl ResponseHeaders {
    /// Returns the number of unique header names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether no response headers were recorded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns all values for a header name using ASCII-insensitive matching.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&[Bytes]> {
        let name = name.to_ascii_lowercase();
        self.entries.get(name.as_str()).map(Vec::as_slice)
    }

    /// Iterates over canonical names and all values in insertion order.
    pub fn iter(&self) -> ResponseHeadersIter<'_> {
        ResponseHeadersIter {
            inner: self.entries.iter(),
        }
    }

    pub(crate) fn append(&mut self, name: &str, value: Bytes) {
        self.entries.entry(name.to_owned()).or_default().push(value);
    }
}

/// An iterator over unique response header names and their values.
pub struct ResponseHeadersIter<'a> {
    inner: map::Iter<'a, String, Vec<Bytes>>,
}

impl<'a> Iterator for ResponseHeadersIter<'a> {
    type Item = (&'a str, &'a [Bytes]);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|(name, values)| (name.as_str(), values.as_slice()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for ResponseHeadersIter<'_> {}

impl<'a> IntoIterator for &'a ResponseHeaders {
    type Item = (&'a str, &'a [Bytes]);
    type IntoIter = ResponseHeadersIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// A bounded response body and its content-type-driven classification.
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseBody {
    /// No response body bytes were received.
    Empty,
    /// A JSON body and the immutable raw bytes from which it was decoded.
    Json {
        /// Original bytes received from the server.
        raw: Bytes,
        /// Parsed JSON value.
        value: JsonValue,
    },
    /// A UTF-8 body declared with a `text/*` content type.
    Text(Bytes),
    /// A body with a non-text content type or no content type.
    Binary(Bytes),
}

impl ResponseBody {
    /// Returns the original body bytes for every classification.
    #[must_use]
    pub fn raw_bytes(&self) -> &[u8] {
        match self {
            Self::Empty => &[],
            Self::Json { raw, .. } | Self::Text(raw) | Self::Binary(raw) => raw,
        }
    }

    /// Returns the parsed JSON value when this is a declared JSON body.
    #[must_use]
    pub const fn as_json(&self) -> Option<&JsonValue> {
        match self {
            Self::Json { value, .. } => Some(value),
            _ => None,
        }
    }

    /// Returns declared text without allocating.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(raw) => str::from_utf8(raw).ok(),
            _ => None,
        }
    }
}

/// A complete response returned by an [`HttpClient`](crate::HttpClient).
#[derive(Debug, Clone, PartialEq)]
pub struct HttpResponse {
    /// Numeric HTTP response status.
    pub status: u16,
    /// Case-insensitive, multi-value response headers.
    pub headers: ResponseHeaders,
    /// Bounded and classified response body.
    pub body: ResponseBody,
}

impl HttpResponse {
    /// Returns the unmodified response body bytes.
    #[must_use]
    pub fn raw_body(&self) -> &[u8] {
        self.body.raw_bytes()
    }
}
