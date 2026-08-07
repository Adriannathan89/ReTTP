use thiserror::Error;

use crate::source::SourceSpan;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum LexerErrorKind {
    #[error("unexpected character `{character}`")]
    UnexpectedCharacter { character: char },

    #[error("unterminated string literal")]
    UnterminatedString,

    #[error("invalid escape sequence `\\{character}`")]
    InvalidEscapeSequence { character: char },

    #[error("invalid integer literal `{value}`")]
    InvalidInteger { value: String },

    #[error("invalid number literal `{value}`")]
    InvalidNumber { value: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexerError {
    pub kind: LexerErrorKind,
    pub span: SourceSpan,
}

impl LexerError {
    #[must_use]
    pub const fn new(kind: LexerErrorKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }
}
