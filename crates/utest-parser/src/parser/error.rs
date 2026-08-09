//! Diagnostics produced while parsing UTest tokens.

use thiserror::Error;

use crate::SourceSpan;

/// Category and context of a parser diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParserErrorKind {
    /// A token did not match the grammar at its current position.
    #[error("expected {expected}, found {found}")]
    UnexpectedToken {
        /// Human-readable description of the expected syntax.
        expected: &'static str,
        /// Human-readable description of the token that was found.
        found: String,
    },

    /// The token stream ended before required syntax was found.
    #[error("expected {expected}, found end of input")]
    UnexpectedEof {
        /// Human-readable description of the expected syntax.
        expected: &'static str,
    },

    /// More than one top-level `core` block was declared.
    #[error("a suite may contain at most one core block")]
    DuplicateCore,

    /// A `pipeline` block contained no tests.
    #[error("a pipeline must contain at least one test")]
    EmptyPipeline,

    /// A test contained no request declaration.
    #[error("a test must contain at least one request")]
    MissingRequest,

    /// A test contained no expectation declaration.
    #[error("a test must contain at least one expectation")]
    MissingExpectation,

    /// A request was declared after the first expectation in a test.
    #[error("request declarations must appear before expectations")]
    RequestAfterExpectation,

    /// A field capture was declared without an explicit assertion type.
    #[error("a capture requires an explicit assertion type")]
    CaptureRequiresType,

    /// A field did not declare a type, comparison, or nested assertion.
    #[error("a field must contain a type assertion, value comparison, or nested object assertion")]
    MissingFieldAssertion,

    /// Recursive syntax exceeded the configured parser nesting limit.
    #[error("maximum parser nesting depth of {limit} exceeded")]
    NestingLimitExceeded {
        /// Effective nesting limit used by the parser.
        limit: usize,
    },
}

/// A parser diagnostic paired with its source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserError {
    /// Diagnostic category and associated context.
    pub kind: ParserErrorKind,
    /// Half-open UTF-8 byte span associated with the diagnostic.
    pub span: SourceSpan,
}

impl ParserError {
    /// Creates a parser diagnostic at the supplied source span.
    #[must_use]
    pub const fn new(kind: ParserErrorKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }
}
