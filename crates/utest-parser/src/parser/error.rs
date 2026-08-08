use thiserror::Error;

use crate::SourceSpan;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParserErrorKind {
    #[error("expected {expected}, found {found}")]
    UnexpectedToken {
        expected: &'static str,
        found: String,
    },

    #[error("expected {expected}, found end of input")]
    UnexpectedEof { expected: &'static str },

    #[error("a suite may contain at most one core block")]
    DuplicateCore,

    #[error("a pipeline must contain at least one test")]
    EmptyPipeline,

    #[error("a test must contain at least one request")]
    MissingRequest,

    #[error("a test must contain at least one expectation")]
    MissingExpectation,

    #[error("request declaration must appear before expectation")]
    RequestAfterExpectation,

    #[error("a capture requires an explicit assertion type")]
    CaptureRequireType,

    #[error("a field must contain a type assertion, value comparison, or nested object assertion")]
    MissingFieldAssertion,

    #[error("maximum parser nesting depth of {limit} exceeded")]
    NestingLimitExceeded { limit: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserError {
    pub kind: ParserErrorKind,
    pub span: SourceSpan,
}

impl ParserError {
    pub const fn new(kind: ParserErrorKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }
}
