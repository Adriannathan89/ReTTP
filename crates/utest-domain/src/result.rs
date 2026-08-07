use serde::{Deserialize, Serialize};

/// Final state assigned by an executor to a test or aggregate block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Passed,
    Failed,
    Skipped,
    Aborted,
}

/// The recorded outcome of one [`TestCase`](crate::TestCase) execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub status: ExecutionStatus,
    pub duration_ms: u64,
    pub failures: Vec<AssertionFailure>,
    pub error: Option<ExecutionErrorInfo>,
}

impl TestResult {
    /// Creates a successful result with no failures or execution error.
    /// Creates a skipped result whose reason is a failed dependency.
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

    /// Creates a failed result with assertion failures and an optional execution error.
    #[must_use]
    pub fn skipped(name: impl Into<String>, reason: impl Into<String>) -> Self {
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

    /// Creates an aborted result caused by an internal runner failure.
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
    pub fn aborted(name: impl Into<String>, reason: impl Into<String>) -> Self {
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

/// Aggregate outcome for a core block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreResult {
    pub status: ExecutionStatus,
    pub tests: Vec<TestResult>,
}

/// Aggregate outcome for a named pipeline block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineResult {
    pub name: String,
    pub status: ExecutionStatus,
    pub cores: Vec<CoreResult>,
}

/// Outcome for any [`SuiteBlock`](crate::SuiteBlock) shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlockResult {
    Core(CoreResult),
    Pipeline(PipelineResult),
    Test(TestResult),
}

/// A machine-readable explanation of one failed response assertion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssertionFailure {
    pub path: String,
    pub kind: AssertionFailureKind,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub message: String,
}

/// Category of response assertion failure.
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

/// Details of an error that prevented normal test execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionErrorInfo {
    pub kind: ExecutionErrorKind,
    pub message: String,
}

/// Category of execution error reported by a backend adapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionErrorKind {
    Connection,
    Timeout,
    InvalidResponse,
    VariableResolution,
    DependencyFailed,
    Internal,
}
