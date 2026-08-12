//! Transport port used by the suite runtime.

use async_trait::async_trait;

use crate::{HttpError, HttpResponse, ResolvedHttpRequest};

/// Asynchronous port for sending one fully resolved HTTP request.
///
/// Implementations must be safe to share between concurrently executing tasks.
/// The object-safe API allows runtimes to store `Arc<dyn HttpClient>` and tests
/// to replace network I/O with a deterministic fake.
#[async_trait]
pub trait HttpClient: Send + Sync {
    /// Sends a resolved request and returns its bounded, classified response.
    ///
    /// HTTP error statuses are successful transport results. The future only
    /// returns [`HttpError`] when request construction, transport, limits, or
    /// response decoding prevent assertion evaluation.
    ///
    /// # Errors
    ///
    /// Returns [`HttpError`] when the request is invalid, transport fails, the
    /// deadline or body limit is exceeded, or the declared response cannot be
    /// decoded.
    async fn execute(&self, request: &ResolvedHttpRequest) -> Result<HttpResponse, HttpError>;
}
