//! Asynchronous HTTP transport abstractions and the default `reqwest` adapter.
//!
//! This crate receives fully resolved requests from an execution runtime. It
//! deliberately does not perform `${variable}` interpolation, parse UTest DSL
//! source, execute suite ordering, or evaluate response assertions.
//!
//! [`HttpClient`] is the backend-neutral port. [`ReqwestHttpClient`] provides a
//! reusable production adapter with strict relative URL handling, disabled
//! redirects and system proxies, bounded response bodies, and deterministic
//! response classification.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod client;
mod config;
mod error;
mod model;
mod reqwest_client;

pub use client::HttpClient;
pub use config::{DEFAULT_MAX_RESPONSE_BODY_BYTES, DEFAULT_REQUEST_TIMEOUT, HttpClientConfig};
pub use error::HttpError;
pub use model::{
    HttpResponse, ResolvedHttpRequest, ResolvedRequestBody, ResolvedValue, ResponseBody,
    ResponseHeaders, ResponseHeadersIter,
};
pub use reqwest_client::ReqwestHttpClient;
