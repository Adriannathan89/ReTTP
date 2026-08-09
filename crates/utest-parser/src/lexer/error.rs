//! Lexer diagnostic types.

use thiserror::Error;

use crate::source::SourceSpan;

#[derive(Debug, Clone, PartialEq, Error)]
/// The category of a recoverable lexical error.
pub enum LexerErrorKind {
    /// A character has no meaning in the current lexical context.
    #[error("unexpected character `{character}`")]
    UnexpectedCharacter { character: char },

    /// A quoted string ended at a line feed or end of input without a closing quote.
    #[error("unterminated string literal")]
    UnterminatedString,

    /// A string escape is not one of `\"`, `\\`, `\n`, `\r`, or `\t`.
    #[error("invalid escape sequence `\\{character}`")]
    InvalidEscapeSequence { character: char },

    /// An integer lexeme cannot be represented as an `i64`.
    #[error("invalid integer literal `{value}`")]
    InvalidInteger { value: String },

    /// A decimal lexeme cannot be represented as a finite `f64`.
    #[error("invalid number literal `{value}`")]
    InvalidNumber { value: String },
}

#[derive(Debug, Clone, PartialEq)]
/// A recoverable lexer diagnostic with the source range that caused it.
pub struct LexerError {
    /// The diagnostic category.
    pub kind: LexerErrorKind,
    /// Byte span of the malformed source lexeme.
    pub span: SourceSpan,
}

impl LexerError {
    #[must_use]
    /// Creates a lexer error with its diagnostic kind and byte span.
    pub const fn new(kind: LexerErrorKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }
}
