use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    InterpolatedString,
    Value,
};

/*
 * HTTP methods for request specification.
 * Abstracts the HTTP method to allow for easy serialization and deserialization.
 * Used in the request specification to define the method of the HTTP request.
 */

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

    #[must_use]
    pub const fn allows_body(&self) -> bool {
        matches!(self, Self::POST | Self::PUT | Self::PATCH)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct HttpRequestSpec {
    pub method: HttpMethod,
    pub path: InterpolatedString,
    pub headers: IndexMap<String, InterpolatedString>,
    pub query: IndexMap<String, Value>,
    pub body: Option<RequestBody>,
    pub timeout_ms: Option<u64>,
}

/*
 * HTTP request specification for defining the details of an HTTP request.
 * Includes the HTTP method, path, headers, query parameters, body, and timeout.
 * Used in the request specification to define the details of the HTTP request.
 */
impl HttpRequestSpec {
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RequestBody {
    Json(Value),
    Text(InterpolatedString),
    FormData(IndexMap<String, Value>),
    Binary(Vec<u8>),
}