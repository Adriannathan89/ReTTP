use serde::{Deserialize, Serialize};

use crate::SuiteBlock;

/// The top-level, serializable specification of an HTTP test suite.
///
/// A suite has an optional display name and ordered blocks. Execution policy is
/// deliberately outside this type so each backend runner can schedule blocks
/// according to its own capabilities.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TestSuite {
    pub name: Option<String>,
    pub blocks: Vec<SuiteBlock>,
}

impl TestSuite {
    /// Creates an unnamed suite from ordered blocks.
    /// Creates a named suite from ordered blocks.
    #[must_use]
    pub fn new(blocks: Vec<SuiteBlock>) -> Self {
        Self { name: None, blocks }
    }

    /// Returns the number of blocks in the suite.
    #[must_use]
    pub fn named(name: impl Into<String>, blocks: Vec<SuiteBlock>) -> Self {
        Self {
            name: Some(name.into()),
            blocks,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Returns whether the suite has no blocks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}
