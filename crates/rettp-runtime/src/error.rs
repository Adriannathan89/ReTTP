//! Redacted errors produced during assignment parsing, resolution, and capture.

use rettp_domain::{DomainError, VariableName};
use thiserror::Error;

/// Location whose interpolation rules rejected a variable value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionLocation {
    /// Relative HTTP request path.
    RequestPath,
    /// Outgoing request header.
    RequestHeader,
    /// URL query parameter.
    QueryParameter,
    /// JSON request-body value.
    JsonRequestBody,
    /// Plain-text request body.
    TextRequestBody,
    /// URL-encoded form field.
    FormField,
    /// Expected response-header value.
    ExpectedHeader,
    /// Expected text response body.
    ExpectedText,
    /// Expected JSON value.
    ExpectedJson,
}

impl std::fmt::Display for ResolutionLocation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::RequestPath => "request path",
            Self::RequestHeader => "request header",
            Self::QueryParameter => "query parameter",
            Self::JsonRequestBody => "JSON request body",
            Self::TextRequestBody => "text request body",
            Self::FormField => "form field",
            Self::ExpectedHeader => "expected response header",
            Self::ExpectedText => "expected text response body",
            Self::ExpectedJson => "expected JSON value",
        })
    }
}

/// A failure while resolving variables or staging and committing captures.
///
/// Variants intentionally omit variable contents and resolved request data.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum RuntimeError {
    /// An interpolation contained `${}`.
    #[error("empty interpolation placeholder in {location}")]
    EmptyPlaceholder {
        /// Context containing the malformed placeholder.
        location: ResolutionLocation,
    },
    /// An interpolation opening marker had no closing brace.
    #[error("unterminated interpolation placeholder in {location}")]
    UnterminatedPlaceholder {
        /// Context containing the malformed placeholder.
        location: ResolutionLocation,
    },
    /// A placeholder name did not satisfy [`rettp_domain::VariableName`].
    #[error("invalid variable name `{name}` in {location}")]
    InvalidVariableName {
        /// Invalid placeholder text.
        name: String,
        /// Context containing the placeholder.
        location: ResolutionLocation,
    },
    /// A valid variable name was absent from the active runtime scope.
    #[error("undefined variable `{name}` in {location}")]
    UndefinedVariable {
        /// Missing variable name.
        name: VariableName,
        /// Context containing the placeholder.
        location: ResolutionLocation,
    },
    /// An object or array was used outside an exact JSON-value placeholder.
    #[error("variable `{name}` of type {value_type} cannot be used in {location}")]
    UnsupportedInterpolationType {
        /// Variable whose type is unsupported at the location.
        name: VariableName,
        /// Stable JSON type name.
        value_type: &'static str,
        /// Rejected interpolation context.
        location: ResolutionLocation,
    },
    /// A resolved string would exceed its configured allocation boundary.
    #[error("resolved string in {location} exceeds the {limit_bytes}-byte limit")]
    InterpolatedValueTooLarge {
        /// Rejected interpolation context.
        location: ResolutionLocation,
        /// Configured maximum byte length.
        limit_bytes: usize,
    },
    /// Programmatically constructed input exceeded the recursive safety limit.
    #[error("resolution nesting exceeds the {limit}-level limit")]
    NestingLimitExceeded {
        /// Configured maximum traversal depth.
        limit: usize,
    },
    /// A floating-point domain value could not be represented as JSON.
    #[error("a resolved number must be finite")]
    NonFiniteNumber,
    /// Successful assertions unexpectedly lacked a JSON object capture body.
    #[error("successful capture evaluation requires a JSON object response body")]
    InvalidCaptureBody,
    /// Successful assertions unexpectedly lacked a declared capture field.
    #[error("successful capture evaluation could not find field at `{path}`")]
    MissingCaptureField {
        /// JSON path of the absent actual field.
        path: String,
    },
    /// Successful assertions unexpectedly produced a non-object nested field.
    #[error("successful capture evaluation found {actual_type} instead of object at `{path}`")]
    InvalidNestedCaptureField {
        /// JSON path of the incompatible actual field.
        path: String,
        /// Stable JSON type name of the actual value.
        actual_type: &'static str,
    },
    /// A capture tried to replace a value already visible in the scope.
    #[error("variable `{name}` is already defined in this scope")]
    DuplicateVariable {
        /// Conflicting variable name.
        name: VariableName,
    },
}

/// Invalid `NAME=VALUE` input supplied through a CLI-compatible source.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VariableAssignmentError {
    /// The assignment contained no equals separator.
    #[error("expected NAME=VALUE")]
    MissingEquals,
    /// The name portion violated domain identifier rules.
    #[error(transparent)]
    InvalidName(#[from] DomainError),
}
