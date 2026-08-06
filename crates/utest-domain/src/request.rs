use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    InterpolatedString,
    Value,
};

/// HTTP methods supported by the portable request model.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    PATCH,
    DELETE,
    HEAD,
    OPTIONS,
}

impl HttpMethod {
    /// Returns the conventional uppercase HTTP method token.
    /// Returns whether this model permits a request body for the method.
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

    /// Adds or replaces a request header.
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct HttpRequestSpec {
    pub method: HttpMethod,
    pub path: InterpolatedString,
    pub headers: IndexMap<String, InterpolatedString>,
    pub query: IndexMap<String, Value>,
    pub body: Option<RequestBody>,
    pub timeout_ms: Option<u64>,
}

impl HttpRequestSpec {
    /// Creates a request with empty headers/query parameters and no body or timeout.
    /// Adds or replaces a query parameter.
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

    /// Sets the request body.
    #[must_use]
    pub fn with_header(
        mut self, name: impl Into<String>, 
        value: impl Into<InterpolatedString>
    ) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    #[must_use]
    pub fn with_query_param(
        mut self, 
        name: impl Into<String>,
        value: impl Into<Value>
    ) -> Self {
        self.query.insert(name.into(), value.into());
        self
    }

    #[must_use]
    pub fn with_body(
        mut self, 
        body: RequestBody
    ) -> Self {
        self.body = Some(body);
        self
    }

}

/// Encoded content to be sent with an HTTP request.
///
/// The executor chooses the concrete wire encoding and corresponding headers.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RequestBody {
    Json(Value),
    Text(InterpolatedString),
    FormData(IndexMap<String, Value>),
    Binary(Vec<u8>),
}
