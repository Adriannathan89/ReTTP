//! Response assertion engine configuration.

use crate::AssertionConfig;

/// Evaluates fully resolved expectations against HTTP responses.
///
/// The engine is immutable and inexpensive to clone, so one configured value
/// can be shared by an executor across every test in a suite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AssertionEngine {
    config: AssertionConfig,
}

impl AssertionEngine {
    /// Creates an engine using previously validated resource limits.
    #[must_use]
    pub const fn new(config: AssertionConfig) -> Self {
        Self { config }
    }

    /// Returns the resource limits used by this engine.
    #[must_use]
    pub const fn config(self) -> AssertionConfig {
        self.config
    }
}
