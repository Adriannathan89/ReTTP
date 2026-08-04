use serde::{Serialize, Deserialize};

/*
 * Abstraction Representation for a test suite block, which can be either a core test or a pipeline.
 * Represents the structure of a test suite block, including its name and the type of block it is (core or pipeline).
 * Used for serialization and deserialization of test suite blocks in the context of a testing framework.
 */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Passed,
    Failed,
    Skipped,
    Aborted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub status: ExecutionStatus,
    pub duration_ms: u64,
    pub failures: Vec<AssertionFailure>,
    pub error: Option<ExecutionErrorInfo>,
}

/*
 * Test Result constructors for different execution statuses (passed, skipped, failed, aborted).
 * Each constructor initializes a TestResult instance with the appropriate status, duration, failures, and error information.
 * These constructors provide a convenient way to create TestResult instances based on the outcome of test
 */
impl TestResult {
    #[must_use]
    pub fn passed(name: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            name: name.into(),
            status: ExecutionStatus::Passed,
            duration_ms,
            failures: Vec::new(),
            error: None,
        }
    }

    #[must_use]
    pub fn skipped(
        name: impl Into<String>, 
        reason: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: ExecutionStatus::Skipped,
            duration_ms: 0,
            failures: Vec::new(),
            error: Some(ExecutionErrorInfo {
                kind: ExecutionErrorKind::DependencyFailed,
                message: reason.into(),
            }),
        }
    }
    
    #[must_use]
    pub fn failed(
        name: impl Into<String>,
        duration_ms: u64,
        failures: Vec<AssertionFailure>,
        error: Option<ExecutionErrorInfo>,
    ) -> Self {
        Self {
            name: name.into(),
            status: ExecutionStatus::Failed,
            duration_ms,
            failures,
            error,
        }
    }

    #[must_use]
    pub fn aborted(
        name: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: ExecutionStatus::Aborted,
            duration_ms: 0,
            failures: Vec::new(),
            error: Some(ExecutionErrorInfo {
                kind: ExecutionErrorKind::Internal,
                message: reason.into(),
            }),
        }
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreResult {
    pub status: ExecutionStatus,
    pub tests: Vec<TestResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineResult {
    pub name: String,
    pub status: ExecutionStatus,
    pub cores: Vec<CoreResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlockResult {
    Core(CoreResult),
    Pipeline(PipelineResult),
    Test(TestResult),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssertionFailure {
    pub path: String,
    pub kind: AssertionFailureKind,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AssertionFailureKind {
    MissingField,
    TypeMismatch,
    ValueMismatch,
    UnexpectedField,
    StatusMismatch,
    HeaderMismatch,
    InvalidBody,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionErrorInfo {
    pub kind: ExecutionErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionErrorKind {
    Connection,
    Timeout,
    InvalidResponse,
    VariableResolution,
    DependencyFailed,
    Internal,
}