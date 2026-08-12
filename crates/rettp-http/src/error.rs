//! Errors produced while configuring or using an HTTP adapter.

use thiserror::Error;

/// A failure that prevented an HTTP request from producing a usable response.
///
/// HTTP error status codes are deliberately absent from this enum. Statuses in
/// the `4xx` and `5xx` ranges are valid [`HttpResponse`](crate::HttpResponse)
/// values and must be evaluated by the assertion layer.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum HttpError {
    /// The configured base URL is malformed or violates adapter policy.
    #[error("invalid HTTP base URL: {reason}")]
    InvalidBaseUrl {
        /// A stable explanation that does not include credentials.
        reason: String,
    },

    /// The resolved request cannot be represented safely on the HTTP wire.
    #[error("invalid HTTP request: {reason}")]
    InvalidRequest {
        /// The request validation or encoding failure.
        reason: String,
    },

    /// DNS, TCP, TLS, or another connection operation failed.
    #[error("HTTP connection failed: {message}")]
    Connection {
        /// A backend diagnostic with request URLs removed.
        message: String,
    },

    /// The configured request deadline elapsed.
    #[error("HTTP request timed out: {message}")]
    Timeout {
        /// A backend diagnostic with request URLs removed.
        message: String,
    },

    /// The server returned data that contradicts its declared representation.
    #[error("invalid HTTP response: {reason}")]
    InvalidResponse {
        /// The response decoding failure.
        reason: String,
    },

    /// The response body exceeded the configured allocation boundary.
    #[error("HTTP response body exceeds the {limit_bytes}-byte limit")]
    BodyTooLarge {
        /// Maximum accepted response body size.
        limit_bytes: usize,
    },
}

impl HttpError {
    pub(crate) fn invalid_base_url(reason: impl Into<String>) -> Self {
        Self::InvalidBaseUrl {
            reason: reason.into(),
        }
    }

    pub(crate) fn invalid_request(reason: impl Into<String>) -> Self {
        Self::InvalidRequest {
            reason: reason.into(),
        }
    }

    pub(crate) fn invalid_response(reason: impl Into<String>) -> Self {
        Self::InvalidResponse {
            reason: reason.into(),
        }
    }

    pub(crate) fn from_reqwest(error: reqwest::Error) -> Self {
        let is_timeout = error.is_timeout();
        let is_builder = error.is_builder();
        let is_decode = error.is_decode();
        let message = error.without_url().to_string();

        if is_timeout {
            Self::Timeout { message }
        } else if is_builder {
            Self::InvalidRequest { reason: message }
        } else if is_decode {
            Self::InvalidResponse { reason: message }
        } else {
            Self::Connection { message }
        }
    }
}
