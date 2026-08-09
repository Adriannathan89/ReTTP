//! Validated configuration shared by HTTP adapter requests.

use std::time::Duration;

use url::Url;

use crate::HttpError;

/// Default request deadline used when a request has no override.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Default maximum number of response body bytes retained in memory.
pub const DEFAULT_MAX_RESPONSE_BODY_BYTES: usize = 10 * 1024 * 1024;

/// Validated configuration for an HTTP client adapter.
///
/// The base URL is restricted to HTTP(S), cannot contain credentials, a query,
/// or a fragment, and is normalized to end in `/`. Redirects are an adapter
/// policy and are always disabled rather than exposed through this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpClientConfig {
    base_url: Url,
    default_timeout: Duration,
    max_response_body_bytes: usize,
}

impl HttpClientConfig {
    /// Validates a base URL and creates configuration with secure defaults.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::InvalidBaseUrl`] when the URL is malformed, is not
    /// HTTP(S), lacks an explicit host, or contains credentials, query data, a
    /// fragment, surrounding whitespace, or control characters.
    pub fn new(base_url: impl AsRef<str>) -> Result<Self, HttpError> {
        let raw_base_url = base_url.as_ref();
        if raw_base_url != raw_base_url.trim() || raw_base_url.chars().any(char::is_control) {
            return Err(HttpError::invalid_base_url(
                "whitespace and control characters are not allowed in the base URL",
            ));
        }
        let mut base_url = Url::parse(raw_base_url)
            .map_err(|error| HttpError::invalid_base_url(error.to_string()))?;

        if !matches!(base_url.scheme(), "http" | "https") {
            return Err(HttpError::invalid_base_url(
                "only http and https schemes are supported",
            ));
        }
        let authority_input = raw_base_url
            .split_once(':')
            .map(|(_, remainder)| remainder)
            .and_then(|remainder| remainder.strip_prefix("//"));
        if authority_input
            .is_none_or(|authority| authority.is_empty() || authority.starts_with('/'))
        {
            return Err(HttpError::invalid_base_url(
                "an HTTP base URL must contain an explicit host",
            ));
        }
        if !base_url.username().is_empty() || base_url.password().is_some() {
            return Err(HttpError::invalid_base_url(
                "credentials are not allowed in the base URL",
            ));
        }
        if base_url.query().is_some() {
            return Err(HttpError::invalid_base_url(
                "a query is not allowed in the base URL",
            ));
        }
        if base_url.fragment().is_some() {
            return Err(HttpError::invalid_base_url(
                "a fragment is not allowed in the base URL",
            ));
        }

        if !base_url.path().ends_with('/') {
            base_url
                .path_segments_mut()
                .map_err(|()| HttpError::invalid_base_url("the URL cannot be used as a base"))?
                .push("");
        }

        Ok(Self {
            base_url,
            default_timeout: DEFAULT_REQUEST_TIMEOUT,
            max_response_body_bytes: DEFAULT_MAX_RESPONSE_BODY_BYTES,
        })
    }

    /// Sets the deadline used by requests without their own timeout.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::InvalidRequest`] when `timeout` is zero.
    pub fn with_default_timeout(mut self, timeout: Duration) -> Result<Self, HttpError> {
        if timeout.is_zero() {
            return Err(HttpError::invalid_request(
                "the default timeout must be greater than zero",
            ));
        }
        self.default_timeout = timeout;
        Ok(self)
    }

    /// Sets the maximum response body allocation.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::InvalidRequest`] when `limit` is zero.
    pub fn with_max_response_body_bytes(mut self, limit: usize) -> Result<Self, HttpError> {
        if limit == 0 {
            return Err(HttpError::invalid_request(
                "the response body limit must be greater than zero",
            ));
        }
        self.max_response_body_bytes = limit;
        Ok(self)
    }

    /// Returns the normalized base URL.
    #[must_use]
    pub const fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Returns the default request deadline.
    #[must_use]
    pub const fn default_timeout(&self) -> Duration {
        self.default_timeout
    }

    /// Returns the maximum accepted response body size.
    #[must_use]
    pub const fn max_response_body_bytes(&self) -> usize {
        self.max_response_body_bytes
    }

    pub(crate) fn resolve_url(&self, path: &str) -> Result<Url, HttpError> {
        if path.trim().is_empty() {
            return Err(HttpError::invalid_request(
                "the relative request path cannot be empty",
            ));
        }
        if path != path.trim() {
            return Err(HttpError::invalid_request(
                "leading or trailing whitespace is not allowed in the request path",
            ));
        }
        if path.starts_with("//") || path.contains('\\') || Url::parse(path).is_ok() {
            return Err(HttpError::invalid_request(
                "the request path must be relative to the configured base URL",
            ));
        }
        if path.contains('?') {
            return Err(HttpError::invalid_request(
                "request query parameters must use the query collection",
            ));
        }
        if path.contains('#') {
            return Err(HttpError::invalid_request(
                "a fragment is not allowed in the request path",
            ));
        }
        if path.chars().any(char::is_control) {
            return Err(HttpError::invalid_request(
                "control characters are not allowed in the request path",
            ));
        }

        let relative_path = path.trim_start_matches('/');
        let resolved = self
            .base_url
            .join(relative_path)
            .map_err(|error| HttpError::invalid_request(error.to_string()))?;

        if !resolved.path().starts_with(self.base_url.path()) {
            return Err(HttpError::invalid_request(
                "the request path cannot escape the configured base path",
            ));
        }

        Ok(resolved)
    }
}
