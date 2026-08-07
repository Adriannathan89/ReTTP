use serde::{Deserialize, Serialize};

use crate::TestCase;

/// An executable unit within a [`TestSuite`](crate::TestSuite).
///
/// Blocks allow a suite format to represent standalone tests, independent core
/// groups, and named pipelines without coupling the domain model to a scheduler.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SuiteBlock {
    Core(CoreBlock),
    Pipeline(PipelineBlock),
    Test(TestCase),
}

/// A group of tests that forms one core execution unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreBlock {
    pub tests: Vec<TestCase>,
}

impl CoreBlock {
    /// Creates a core block from its ordered tests.
    /// Returns whether the core contains no tests.
    #[must_use]
    pub fn new(tests: Vec<TestCase>) -> Self {
        Self { tests }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tests.is_empty()
    }
}

/// A named group of tests intended for pipeline-level execution or reporting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineBlock {
    pub name: String,
    pub tests: Vec<TestCase>,
}

impl PipelineBlock {
    /// Creates a named pipeline block from its ordered tests.
    /// Returns whether the pipeline contains no tests.
    #[must_use]
    pub fn new(name: impl Into<String>, tests: Vec<TestCase>) -> Self {
        Self {
            name: name.into(),
            tests,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tests.is_empty()
    }
}
