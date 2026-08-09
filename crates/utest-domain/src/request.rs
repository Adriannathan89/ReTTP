//! Transport-independent HTTP request domain types.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{InterpolatedString, Value};

/// HTTP methods supported by the portable request model.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    /// Retrieves a resource without a request body.
    GET,
    /// Creates or submits a resource and may include a request body.
    POST,
    /// Replaces a resource and may include a request body.
    PUT,
    /// Partially updates a resource and may include a request body.
    PATCH,
    /// Deletes a resource without a request body in the UTest DSL.
    DELETE,
    /// Retrieves response metadata without a response body.
    HEAD,
    /// Retrieves communication options for a resource.
    OPTIONS,
}

impl HttpMethod {
    /// Returns the conventional uppercase HTTP method token.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::GET => "GET",
            Self::POST => "POST",
            Self::PUT => "PUT",
            Self::PATCH => "PATCH",
            Self::DELETE => "DELETE",
            Self::HEAD => "HEAD",
            Self::OPTIONS => "OPTIONS",
        }
    }

    /// Returns whether the UTest DSL permits a request body for this method.
    #[must_use]
    pub const fn allows_body(&self) -> bool {
        matches!(self, Self::POST | Self::PUT | Self::PATCH)
    }
}

/// A complete, transport-independent description of one HTTP request.
///
/// Backend adapters are responsible for resolving interpolation, encoding the
/// selected [`RequestBody`], applying timeout behavior, and sending this
/// specification through their native HTTP stack. `IndexMap` preserves the
/// declaration order when a suite is serialized or displayed.
///
/// String header values can be direct, interpolation-only, or mixed text such
/// as `"something ${variable}"`. Non-string [`Value`] variants remain available
/// for adapters that support typed header serialization.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct HttpRequestSpec {
    /// HTTP method used by the request.
    pub method: HttpMethod,
    /// Unresolved path, which may contain interpolation placeholders.
    pub path: InterpolatedString,
    /// Insertion-ordered request headers and their typed values.
    pub headers: IndexMap<String, Value>,
    /// Insertion-ordered URL query parameters.
    pub query: IndexMap<String, Value>,
    /// Optional encoded request body.
    pub body: Option<RequestBody>,
    /// Optional request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
}

impl HttpRequestSpec {
    /// Creates a request with empty headers/query parameters and no body or timeout.
    #[must_use]
    pub fn new(method: HttpMethod, path: impl Into<InterpolatedString>) -> Self {
        Self {
            method,
            path: path.into(),
            headers: IndexMap::new(),
            query: IndexMap::new(),
            body: None,
            timeout_ms: None,
        }
    }

    /// Adds or replaces a typed request header.
    ///
    /// Replacing a header preserves its existing `IndexMap` position.
    /// Adds or replaces a typed query parameter.
    #[must_use]
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    /// Sets or replaces the request body.
    #[must_use]
    pub fn with_query_param(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.query.insert(name.into(), value.into());
        self
    }

    #[must_use]
    pub fn with_body(mut self, body: RequestBody) -> Self {
        self.body = Some(body);
        self
    }
}

/// Encoded content to be sent with an HTTP request.
///
/// The executor chooses the concrete wire encoding and corresponding headers.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RequestBody {
    /// JSON-compatible typed value.
    Json(Value),
    /// Plain or interpolated UTF-8 text.
    Text(InterpolatedString),
    /// Insertion-ordered form fields.
    FormData(IndexMap<String, Value>),
    /// Uninterpreted binary bytes.
    Binary(Vec<u8>),
}
