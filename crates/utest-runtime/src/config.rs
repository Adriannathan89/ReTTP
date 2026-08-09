//! Validated resource limits for runtime resolution.

use thiserror::Error;

/// Default maximum byte length of one resolved string.
pub const DEFAULT_MAX_INTERPOLATED_BYTES: usize = 1024 * 1024;
/// Largest configurable byte length of one resolved string.
pub const HARD_MAX_INTERPOLATED_BYTES: usize = 16 * 1024 * 1024;
/// Default maximum nesting depth for values and assertions.
pub const DEFAULT_MAX_RESOLUTION_DEPTH: usize = 128;
/// Largest configurable nesting depth for values and assertions.
pub const HARD_MAX_RESOLUTION_DEPTH: usize = 256;

/// Resource limits applied while resolving variables and traversing models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeConfig {
    max_interpolated_bytes: usize,
    max_resolution_depth: usize,
}

impl RuntimeConfig {
    /// Validates and creates runtime resource limits.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeConfigError`] when either limit is zero or exceeds its
    /// documented hard maximum.
    pub const fn new(
        max_interpolated_bytes: usize,
        max_resolution_depth: usize,
    ) -> Result<Self, RuntimeConfigError> {
        if max_interpolated_bytes == 0 {
            return Err(RuntimeConfigError::ZeroInterpolatedBytes);
        }
        if max_interpolated_bytes > HARD_MAX_INTERPOLATED_BYTES {
            return Err(RuntimeConfigError::InterpolatedBytesTooLarge {
                requested: max_interpolated_bytes,
                maximum: HARD_MAX_INTERPOLATED_BYTES,
            });
        }
        if max_resolution_depth == 0 {
            return Err(RuntimeConfigError::ZeroResolutionDepth);
        }
        if max_resolution_depth > HARD_MAX_RESOLUTION_DEPTH {
            return Err(RuntimeConfigError::ResolutionDepthTooLarge {
                requested: max_resolution_depth,
                maximum: HARD_MAX_RESOLUTION_DEPTH,
            });
        }
        Ok(Self {
            max_interpolated_bytes,
            max_resolution_depth,
        })
    }

    /// Returns the maximum byte length of one resolved string.
    #[must_use]
    pub const fn max_interpolated_bytes(self) -> usize {
        self.max_interpolated_bytes
    }

    /// Returns the maximum permitted recursive traversal depth.
    #[must_use]
    pub const fn max_resolution_depth(self) -> usize {
        self.max_resolution_depth
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_interpolated_bytes: DEFAULT_MAX_INTERPOLATED_BYTES,
            max_resolution_depth: DEFAULT_MAX_RESOLUTION_DEPTH,
        }
    }
}

/// Invalid resource-limit configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RuntimeConfigError {
    /// A resolved string cannot have a zero-byte limit.
    #[error("maximum interpolated bytes must be greater than zero")]
    ZeroInterpolatedBytes,
    /// The requested resolved-string limit exceeds the hard maximum.
    #[error("maximum interpolated bytes {requested} exceeds hard limit {maximum}")]
    InterpolatedBytesTooLarge {
        /// Rejected byte limit.
        requested: usize,
        /// Largest supported byte limit.
        maximum: usize,
    },
    /// Recursive traversal cannot have a zero-depth limit.
    #[error("maximum resolution depth must be greater than zero")]
    ZeroResolutionDepth,
    /// The requested recursive traversal limit exceeds the hard maximum.
    #[error("maximum resolution depth {requested} exceeds hard limit {maximum}")]
    ResolutionDepthTooLarge {
        /// Rejected depth limit.
        requested: usize,
        /// Largest supported depth limit.
        maximum: usize,
    },
}
