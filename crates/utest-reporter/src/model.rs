//! Versioned, strictly redacted data model shared by every reporter.

use serde::{Deserialize, Serialize};
use utest_domain::{
    AssertionFailure, AssertionFailureKind, BlockResult, ExecutionErrorInfo, ExecutionErrorKind,
    ExecutionStatus, SuiteResult, TestResult,
};

/// Schema version emitted by [`RunReport`].
pub const REPORT_SCHEMA_VERSION: u32 = 1;

const REDACTED: &str = "<redacted>";

/// Publishable result of one source-file execution.
///
/// This type is deliberately separate from [`SuiteResult`]. Its serialized
/// field names and enum spellings form the stable JSON reporter contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunReport {
    /// Version of this public report schema.
    pub schema_version: u32,
    /// Display path of the executed source file.
    pub source: String,
    /// Optional suite name supplied by the domain model.
    pub name: Option<String>,
    /// Final suite status.
    pub status: ReportStatus,
    /// Saturating suite duration in milliseconds.
    pub duration_ms: u64,
    /// Aggregate counts for all DSL tests.
    pub summary: ReportSummary,
    /// Source-ordered block reports.
    pub blocks: Vec<ReportBlock>,
    /// Sanitized suite-level error, when execution could not finish normally.
    pub error: Option<ReportExecutionError>,
}

impl RunReport {
    /// Converts an internal execution result into a publishable report.
    ///
    /// Assertion values and internal error messages are not copied verbatim.
    /// Renderers can therefore consume this value without access to variables,
    /// resolved requests, response bodies, cookies, tokens, or passwords.
    #[must_use]
    pub fn from_suite_result(source: impl Into<String>, result: &SuiteResult) -> Self {
        let blocks: Vec<_> = result.blocks.iter().map(ReportBlock::from).collect();
        let summary = ReportSummary::from_blocks(&blocks);
        Self {
            schema_version: REPORT_SCHEMA_VERSION,
            source: source.into(),
            name: result.name.clone(),
            status: result.status.into(),
            duration_ms: result.duration_ms,
            summary,
            blocks,
            error: result.error.as_ref().map(ReportExecutionError::from),
        }
    }
}

/// Stable status used by public reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    /// Every required operation completed successfully.
    Passed,
    /// Execution completed with at least one failed operation.
    Failed,
    /// Execution was not attempted because a dependency failed.
    Skipped,
    /// Execution stopped because a core or internal invariant failed.
    Aborted,
}

impl ReportStatus {
    /// Returns the stable uppercase terminal label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Passed => "PASS",
            Self::Failed => "FAIL",
            Self::Skipped => "SKIP",
            Self::Aborted => "ABORT",
        }
    }

    /// Returns the stable lowercase machine-readable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Aborted => "aborted",
        }
    }
}

impl From<ExecutionStatus> for ReportStatus {
    fn from(value: ExecutionStatus) -> Self {
        match value {
            ExecutionStatus::Passed => Self::Passed,
            ExecutionStatus::Failed => Self::Failed,
            ExecutionStatus::Skipped => Self::Skipped,
            ExecutionStatus::Aborted => Self::Aborted,
        }
    }
}

/// Aggregate test counts for a complete run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSummary {
    /// Number of all represented tests.
    pub total: u64,
    /// Number of passed tests.
    pub passed: u64,
    /// Number of failed tests.
    pub failed: u64,
    /// Number of skipped tests.
    pub skipped: u64,
    /// Number of aborted tests.
    pub aborted: u64,
}

impl ReportSummary {
    fn from_blocks(blocks: &[ReportBlock]) -> Self {
        let mut summary = Self::default();
        for test in blocks.iter().flat_map(|block| &block.tests) {
            summary.total = summary.total.saturating_add(1);
            match test.status {
                ReportStatus::Passed => summary.passed = summary.passed.saturating_add(1),
                ReportStatus::Failed => summary.failed = summary.failed.saturating_add(1),
                ReportStatus::Skipped => summary.skipped = summary.skipped.saturating_add(1),
                ReportStatus::Aborted => summary.aborted = summary.aborted.saturating_add(1),
            }
        }
        summary
    }
}

/// Kind of one source-level suite block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportBlockKind {
    /// The optional suite dependency block.
    Core,
    /// A named sequential pipeline.
    Pipeline,
    /// One standalone test block.
    Test,
}

impl ReportBlockKind {
    /// Returns the stable lowercase spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Pipeline => "pipeline",
            Self::Test => "test",
        }
    }
}

/// Publishable aggregate result for one source block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportBlock {
    /// Block category.
    pub kind: ReportBlockKind,
    /// Pipeline name, or standalone test name; core has no explicit name.
    pub name: Option<String>,
    /// Aggregate block status.
    pub status: ReportStatus,
    /// Saturating block duration in milliseconds.
    pub duration_ms: u64,
    /// Test results in declaration order.
    pub tests: Vec<ReportTest>,
}

impl From<&BlockResult> for ReportBlock {
    fn from(value: &BlockResult) -> Self {
        match value {
            BlockResult::Core(core) => Self {
                kind: ReportBlockKind::Core,
                name: None,
                status: core.status.into(),
                duration_ms: core.duration_ms,
                tests: core.tests.iter().map(ReportTest::from).collect(),
            },
            BlockResult::Pipeline(pipeline) => Self {
                kind: ReportBlockKind::Pipeline,
                name: Some(pipeline.name.clone()),
                status: pipeline.status.into(),
                duration_ms: pipeline.duration_ms,
                tests: pipeline.tests.iter().map(ReportTest::from).collect(),
            },
            BlockResult::Test(test) => Self {
                kind: ReportBlockKind::Test,
                name: Some(test.name.clone()),
                status: test.status.into(),
                duration_ms: test.duration_ms,
                tests: vec![ReportTest::from(test)],
            },
        }
    }
}

/// Publishable result of one DSL test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportTest {
    /// Authored test name.
    pub name: String,
    /// Final test status.
    pub status: ReportStatus,
    /// Saturating test duration in milliseconds.
    pub duration_ms: u64,
    /// Deterministically ordered sanitized assertion failures.
    pub failures: Vec<ReportAssertionFailure>,
    /// Sanitized execution error, when present.
    pub error: Option<ReportExecutionError>,
}

impl From<&TestResult> for ReportTest {
    fn from(value: &TestResult) -> Self {
        Self {
            name: value.name.clone(),
            status: value.status.into(),
            duration_ms: value.duration_ms,
            failures: value
                .failures
                .iter()
                .map(ReportAssertionFailure::from)
                .collect(),
            error: value.error.as_ref().map(ReportExecutionError::from),
        }
    }
}

/// Stable assertion-failure category used by public reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportFailureKind {
    /// A required JSON field was absent.
    MissingField,
    /// The actual value had an incompatible type.
    TypeMismatch,
    /// Compatible values were unequal.
    ValueMismatch,
    /// Exact object matching found an undeclared field.
    UnexpectedField,
    /// The response status differed from the expected status.
    StatusMismatch,
    /// A response header assertion failed.
    HeaderMismatch,
    /// The response body had an incompatible classification.
    InvalidBody,
}

impl ReportFailureKind {
    /// Returns the stable lowercase spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingField => "missing_field",
            Self::TypeMismatch => "type_mismatch",
            Self::ValueMismatch => "value_mismatch",
            Self::UnexpectedField => "unexpected_field",
            Self::StatusMismatch => "status_mismatch",
            Self::HeaderMismatch => "header_mismatch",
            Self::InvalidBody => "invalid_body",
        }
    }

    const fn safe_message(self) -> &'static str {
        match self {
            Self::MissingField => "a required field is missing",
            Self::TypeMismatch => "the actual value has an incompatible type",
            Self::ValueMismatch => "the actual value does not equal the expected value",
            Self::UnexpectedField => "an undeclared field was present during exact matching",
            Self::StatusMismatch => "the HTTP response status did not match",
            Self::HeaderMismatch => "a response header assertion failed",
            Self::InvalidBody => "the response body has an incompatible representation",
        }
    }
}

impl From<AssertionFailureKind> for ReportFailureKind {
    fn from(value: AssertionFailureKind) -> Self {
        match value {
            AssertionFailureKind::MissingField => Self::MissingField,
            AssertionFailureKind::TypeMismatch => Self::TypeMismatch,
            AssertionFailureKind::ValueMismatch => Self::ValueMismatch,
            AssertionFailureKind::UnexpectedField => Self::UnexpectedField,
            AssertionFailureKind::StatusMismatch => Self::StatusMismatch,
            AssertionFailureKind::HeaderMismatch => Self::HeaderMismatch,
            AssertionFailureKind::InvalidBody => Self::InvalidBody,
        }
    }
}

/// Sanitized details of one failed response assertion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportAssertionFailure {
    /// Response location such as `status`, a header, or a JSON path.
    pub path: String,
    /// Stable mismatch category.
    pub kind: ReportFailureKind,
    /// Safe structural preview or the literal `<redacted>`.
    pub expected: Option<String>,
    /// Safe structural preview or the literal `<redacted>`.
    pub actual: Option<String>,
    /// Generic value-free explanation generated from `kind`.
    pub message: String,
}

impl From<&AssertionFailure> for ReportAssertionFailure {
    fn from(value: &AssertionFailure) -> Self {
        let kind = ReportFailureKind::from(value.kind.clone());
        Self {
            path: value.path.clone(),
            kind,
            expected: sanitize_preview(value.expected.as_deref(), kind),
            actual: sanitize_preview(value.actual.as_deref(), kind),
            message: kind.safe_message().to_owned(),
        }
    }
}

fn sanitize_preview(value: Option<&str>, kind: ReportFailureKind) -> Option<String> {
    value.map(|value| {
        if preview_is_safe(value, kind) {
            value.to_owned()
        } else {
            REDACTED.to_owned()
        }
    })
}

fn preview_is_safe(value: &str, kind: ReportFailureKind) -> bool {
    match kind {
        ReportFailureKind::StatusMismatch => value
            .parse::<u16>()
            .is_ok_and(|status| (100..=599).contains(&status)),
        ReportFailureKind::TypeMismatch | ReportFailureKind::InvalidBody => matches!(
            value,
            "empty"
                | "JSON"
                | "text"
                | "binary"
                | "object"
                | "array"
                | "string"
                | "boolean"
                | "integer"
                | "number"
                | "null"
                | "valid UTF-8 text"
                | "non-UTF-8 text body"
        ),
        ReportFailureKind::MissingField
        | ReportFailureKind::ValueMismatch
        | ReportFailureKind::UnexpectedField
        | ReportFailureKind::HeaderMismatch => false,
    }
}

/// Stable execution-error category used by public reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportErrorKind {
    /// A resolved request could not be encoded safely.
    InvalidRequest,
    /// A network connection operation failed.
    Connection,
    /// The request deadline elapsed.
    Timeout,
    /// The response was malformed, unsupported, or oversized.
    InvalidResponse,
    /// Runtime interpolation or expected-value resolution failed.
    VariableResolution,
    /// Execution was skipped because a dependency failed.
    DependencyFailed,
    /// An internal invariant failed.
    Internal,
}

impl ReportErrorKind {
    /// Returns the stable lowercase spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::Connection => "connection",
            Self::Timeout => "timeout",
            Self::InvalidResponse => "invalid_response",
            Self::VariableResolution => "variable_resolution",
            Self::DependencyFailed => "dependency_failed",
            Self::Internal => "internal",
        }
    }

    const fn safe_message(self) -> &'static str {
        match self {
            Self::InvalidRequest => "the HTTP request is invalid",
            Self::Connection => "the HTTP connection failed",
            Self::Timeout => "the HTTP request timed out",
            Self::InvalidResponse => "the HTTP response is invalid",
            Self::VariableResolution => "runtime variable resolution failed",
            Self::DependencyFailed => "execution was skipped because a dependency failed",
            Self::Internal => "an internal execution invariant failed",
        }
    }
}

impl From<ExecutionErrorKind> for ReportErrorKind {
    fn from(value: ExecutionErrorKind) -> Self {
        match value {
            ExecutionErrorKind::InvalidRequest => Self::InvalidRequest,
            ExecutionErrorKind::Connection => Self::Connection,
            ExecutionErrorKind::Timeout => Self::Timeout,
            ExecutionErrorKind::InvalidResponse => Self::InvalidResponse,
            ExecutionErrorKind::VariableResolution => Self::VariableResolution,
            ExecutionErrorKind::DependencyFailed => Self::DependencyFailed,
            ExecutionErrorKind::Internal => Self::Internal,
        }
    }
}

/// Sanitized execution error safe for terminal and CI artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportExecutionError {
    /// Stable error category.
    pub kind: ReportErrorKind,
    /// Generic value-free explanation generated from `kind`.
    pub message: String,
}

impl From<&ExecutionErrorInfo> for ReportExecutionError {
    fn from(value: &ExecutionErrorInfo) -> Self {
        let kind = ReportErrorKind::from(value.kind.clone());
        Self {
            kind,
            message: kind.safe_message().to_owned(),
        }
    }
}
