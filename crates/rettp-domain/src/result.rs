use serde::{Deserialize, Serialize};

/// Final state assigned by an executor to a test or aggregate block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Every required operation completed successfully.
    Passed,
    /// Execution completed but an assertion, transport, or runtime operation failed.
    Failed,
    /// Execution was not attempted because a dependency failed.
    Skipped,
    /// Execution stopped because core or an internal invariant failed.
    Aborted,
}

/// The recorded outcome of one [`TestCase`](crate::TestCase) execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestResult {
    /// Test name copied from its domain declaration.
    pub name: String,
    /// Final status of this test.
    pub status: ExecutionStatus,
    /// Saturating wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Deterministically ordered response assertion failures.
    pub failures: Vec<AssertionFailure>,
    /// Error that prevented normal assertion success, when present.
    pub error: Option<ExecutionErrorInfo>,
}

impl TestResult {
    /// Creates a successful result with no failures or execution error.
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

    /// Creates a skipped result whose reason is a failed dependency.
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

    /// Creates a failed result with assertion failures and an optional execution error.
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

    /// Creates a result aborted by an internal execution invariant.
    #[must_use]
    pub fn aborted(name: impl Into<String>, duration_ms: u64, reason: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: ExecutionStatus::Aborted,
            duration_ms,
            failures: Vec::new(),
            error: Some(ExecutionErrorInfo {
                kind: ExecutionErrorKind::Internal,
                message: reason.into(),
            }),
        }
    }
}

/// Aggregate outcome for the unique optional core block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreResult {
    /// Aggregate status derived from the core tests.
    pub status: ExecutionStatus,
    /// Saturating wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Core test outcomes in declaration order.
    pub tests: Vec<TestResult>,
}

/// Aggregate outcome for a named pipeline block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineResult {
    /// Pipeline name copied from its domain declaration.
    pub name: String,
    /// Aggregate status derived from the pipeline tests.
    pub status: ExecutionStatus,
    /// Saturating wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Pipeline test outcomes in declaration order.
    pub tests: Vec<TestResult>,
}

/// Outcome for any [`SuiteBlock`](crate::SuiteBlock) shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlockResult {
    /// Result of the unique optional core block.
    Core(CoreResult),
    /// Result of a named pipeline.
    Pipeline(PipelineResult),
    /// Result of one standalone test.
    Test(TestResult),
}

/// Complete source-ordered outcome of one [`TestSuite`](crate::TestSuite).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuiteResult {
    /// Optional suite name copied from the domain model.
    pub name: Option<String>,
    /// Final suite status.
    pub status: ExecutionStatus,
    /// Saturating wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Block outcomes in source declaration order.
    pub blocks: Vec<BlockResult>,
    /// Suite-level error used when execution cannot start normally.
    pub error: Option<ExecutionErrorInfo>,
}

/// A machine-readable explanation of one failed response assertion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssertionFailure {
    /// Response location such as `status`, a header, or a JSON path.
    pub path: String,
    /// Stable category of assertion mismatch.
    pub kind: AssertionFailureKind,
    /// Bounded expected-value preview, when relevant.
    pub expected: Option<String>,
    /// Bounded actual-value preview, when relevant.
    pub actual: Option<String>,
    /// Human-readable bounded failure explanation.
    pub message: String,
}

/// Category of response assertion failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AssertionFailureKind {
    /// A required JSON field was absent.
    MissingField,
    /// The actual value had an incompatible type.
    TypeMismatch,
    /// Compatible values were not equal under the selected comparison mode.
    ValueMismatch,
    /// Exact object matching found an undeclared field.
    UnexpectedField,
    /// The response status differed from the expected status.
    StatusMismatch,
    /// A response header assertion failed.
    HeaderMismatch,
    /// The response body could not satisfy the requested body classification.
    InvalidBody,
}

/// Details of an error that prevented normal test execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionErrorInfo {
    /// Stable execution-error category.
    pub kind: ExecutionErrorKind,
    /// Redacted human-readable diagnostic.
    pub message: String,
}

/// Category of execution error reported by a runner or backend adapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionErrorKind {
    /// A resolved request could not be represented safely on the wire.
    InvalidRequest,
    /// DNS, TCP, TLS, or another connection operation failed.
    Connection,
    /// The request deadline elapsed.
    Timeout,
    /// A response was malformed, unsupported, or exceeded a resource limit.
    InvalidResponse,
    /// Runtime interpolation or expectation resolution failed.
    VariableResolution,
    /// Execution was skipped because a required prior result failed.
    DependencyFailed,
    /// A validated-model or executor invariant failed.
    Internal,
}
