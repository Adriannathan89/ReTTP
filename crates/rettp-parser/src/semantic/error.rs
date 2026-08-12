//! Structured semantic diagnostics.

use std::fmt;

use thiserror::Error;

use crate::SourceSpan;

/// The declaration category involved in a duplicate or empty-name error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateKind {
    /// A second top-level `core` block.
    CoreBlock,
    /// A second request in one test.
    Request,
    /// A second expectation in one test.
    Expectation,
    /// A repeated request `headers` section.
    RequestHeaders,
    /// A repeated request `query` section.
    RequestQuery,
    /// A repeated request `body` section.
    RequestBody,
    /// A repeated response `status` section.
    ResponseStatus,
    /// A repeated response `headers` section.
    ResponseHeaders,
    /// A repeated response `body` section.
    ResponseBody,
    /// A request header with a duplicate case-insensitive name.
    RequestHeader,
    /// A response header assertion with a duplicate case-insensitive name.
    ResponseHeader,
    /// A request query parameter with a duplicate name.
    QueryParameter,
    /// A repeated key in an object value.
    ObjectKey,
    /// A repeated field in an object assertion.
    AssertionField,
}

impl fmt::Display for DuplicateKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let description = match self {
            Self::CoreBlock => "core block",
            Self::Request => "request declaration",
            Self::Expectation => "expectation declaration",
            Self::RequestHeaders => "request headers section",
            Self::RequestQuery => "request query section",
            Self::RequestBody => "request body section",
            Self::ResponseStatus => "response status section",
            Self::ResponseHeaders => "response headers section",
            Self::ResponseBody => "response body section",
            Self::RequestHeader => "request header",
            Self::ResponseHeader => "response header",
            Self::QueryParameter => "query parameter",
            Self::ObjectKey => "object key",
            Self::AssertionField => "assertion field",
        };
        formatter.write_str(description)
    }
}

/// A semantic rule violation independent of its source location.
///
/// Use [`ValidationError`] when the corresponding [`SourceSpan`] is also
/// required for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ValidationErrorKind {
    /// A singleton declaration or section appeared more than once.
    #[error("duplicate {kind}")]
    Duplicate {
        /// Category of the duplicated declaration.
        kind: DuplicateKind,
    },

    /// A named map entry or assertion field appeared more than once.
    #[error("duplicate {kind} `{name}`")]
    DuplicateNamed {
        /// Category of the duplicated entry.
        kind: DuplicateKind,
        /// Name exactly as written by the user.
        name: String,
    },

    /// A declaration that requires a name used an empty string.
    #[error("{kind} name cannot be empty")]
    EmptyName {
        /// Category of the unnamed declaration.
        kind: DuplicateKind,
    },

    /// A pipeline name was empty.
    #[error("pipeline name cannot be empty")]
    EmptyPipelineName,

    /// A test name was empty.
    #[error("test name cannot be empty")]
    EmptyTestName,

    /// A pipeline contained no tests.
    #[error("a pipeline must contain at least one test")]
    EmptyPipeline,

    /// A test did not contain exactly one request.
    #[error("a test must contain exactly one request")]
    InvalidRequestCount,

    /// A test did not contain exactly one expectation.
    #[error("a test must contain exactly one expectation")]
    InvalidExpectationCount,

    /// A request path was empty.
    #[error("request path cannot be empty")]
    EmptyRequestPath,

    /// A response status was outside the valid HTTP range.
    #[error("HTTP status code {status} is outside 100..=599")]
    InvalidHttpStatus {
        /// Invalid status value from the syntax tree.
        status: i64,
    },

    /// A method that cannot carry a body declared one.
    #[error("HTTP method {method} does not allow a request body")]
    RequestBodyNotAllowed {
        /// Uppercase HTTP method name.
        method: &'static str,
    },

    /// An assertion's explicit type was incompatible with its comparison.
    #[error("field `{field}` expects {expected}, but comparison value is {actual}")]
    TypeValueMismatch {
        /// Object field being asserted.
        field: String,
        /// Declared assertion type.
        expected: &'static str,
        /// Type inferred from the comparison value.
        actual: &'static str,
    },

    /// A field assertion supplied no constraint.
    #[error("field `{field}` has no type, comparison, or nested assertion")]
    MissingFieldAssertion {
        /// Object field being asserted.
        field: String,
    },

    /// A capture was declared without an explicit assertion type.
    #[error("capture `{name}` requires an explicit assertion type")]
    CaptureRequiresType {
        /// Capture variable name.
        name: String,
    },

    /// A nested assertion was attached to a non-object field.
    #[error("nested assertion for `{field}` requires the object type")]
    NestedAssertionRequiresObject {
        /// Object field containing the nested assertion.
        field: String,
    },

    /// A nested assertion requested exact rather than partial matching.
    #[error("nested assertion for `{field}` must use partial matching")]
    NestedAssertionMustBePartial {
        /// Object field containing the nested assertion.
        field: String,
    },

    /// A capture or interpolation used an invalid variable identifier.
    #[error("invalid variable name `{name}`")]
    InvalidVariableName {
        /// Invalid variable name.
        name: String,
    },

    /// An interpolation referenced a variable outside its visible scope.
    #[error("undefined variable `{name}`")]
    UndefinedVariable {
        /// Undefined variable name.
        name: String,
    },

    /// A capture attempted to redefine an already visible variable.
    #[error("variable `{name}` is already defined in this scope")]
    DuplicateVariable {
        /// Conflicting variable name.
        name: String,
    },

    /// An interpolation contained `${}`.
    #[error("empty interpolation placeholder")]
    EmptyInterpolation,

    /// An interpolation began with `${` but had no closing brace.
    #[error("unterminated interpolation placeholder")]
    UnterminatedInterpolation,

    /// A value or assertion exceeded the configured recursion limit.
    #[error("maximum semantic nesting depth of {limit} exceeded")]
    NestingLimitExceeded {
        /// Effective nesting limit from the validation context.
        limit: usize,
    },
}

/// A semantic diagnostic paired with its source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// The violated semantic rule.
    pub kind: ValidationErrorKind,
    /// Byte span of the source construct responsible for the error.
    pub span: SourceSpan,
}

impl ValidationError {
    /// Creates a semantic diagnostic from a rule violation and source span.
    #[must_use]
    pub const fn new(kind: ValidationErrorKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.kind, formatter)
    }
}

impl std::error::Error for ValidationError {}
