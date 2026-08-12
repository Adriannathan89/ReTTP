//! Resource limits applied during assertion evaluation.

use thiserror::Error;

/// Default maximum number of failures retained in one report.
pub const DEFAULT_MAX_FAILURES: usize = 100;

/// Default maximum recursive JSON comparison depth.
pub const DEFAULT_MAX_JSON_DEPTH: usize = 128;

/// Largest JSON comparison depth accepted from a caller.
pub const HARD_MAX_JSON_DEPTH: usize = 256;

/// Validated resource limits for an [`AssertionEngine`](crate::AssertionEngine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssertionConfig {
    max_failures: usize,
    max_json_depth: usize,
}

impl AssertionConfig {
    /// Creates validated assertion limits.
    ///
    /// # Errors
    ///
    /// Returns [`AssertionConfigError::ZeroMaxFailures`] when `max_failures`
    /// is zero, [`AssertionConfigError::ZeroMaxJsonDepth`] when
    /// `max_json_depth` is zero, or
    /// [`AssertionConfigError::JsonDepthExceedsHardLimit`] when the requested
    /// depth is greater than [`HARD_MAX_JSON_DEPTH`].
    pub const fn new(
        max_failures: usize,
        max_json_depth: usize,
    ) -> Result<Self, AssertionConfigError> {
        if max_failures == 0 {
            return Err(AssertionConfigError::ZeroMaxFailures);
        }
        if max_json_depth == 0 {
            return Err(AssertionConfigError::ZeroMaxJsonDepth);
        }
        if max_json_depth > HARD_MAX_JSON_DEPTH {
            return Err(AssertionConfigError::JsonDepthExceedsHardLimit {
                requested: max_json_depth,
                hard_limit: HARD_MAX_JSON_DEPTH,
            });
        }

        Ok(Self {
            max_failures,
            max_json_depth,
        })
    }

    /// Returns the maximum number of retained failures.
    #[must_use]
    pub const fn max_failures(self) -> usize {
        self.max_failures
    }

    /// Returns the maximum recursive JSON comparison depth.
    #[must_use]
    pub const fn max_json_depth(self) -> usize {
        self.max_json_depth
    }
}

impl Default for AssertionConfig {
    fn default() -> Self {
        Self {
            max_failures: DEFAULT_MAX_FAILURES,
            max_json_depth: DEFAULT_MAX_JSON_DEPTH,
        }
    }
}

/// Invalid assertion-engine resource configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AssertionConfigError {
    /// At least one failure slot is required.
    #[error("maximum assertion failures must be greater than zero")]
    ZeroMaxFailures,
    /// At least one JSON comparison level is required.
    #[error("maximum JSON depth must be greater than zero")]
    ZeroMaxJsonDepth,
    /// The requested recursion depth is above the library safety ceiling.
    #[error("maximum JSON depth {requested} exceeds hard limit {hard_limit}")]
    JsonDepthExceedsHardLimit {
        /// Requested depth.
        requested: usize,
        /// Compile-time safety ceiling.
        hard_limit: usize,
    },
}
