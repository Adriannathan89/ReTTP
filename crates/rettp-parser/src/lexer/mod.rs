//! Tokenization API for Rettp DSL source text.
//!
//! Use [`lex`] for the normal one-shot interface. The lexer records independent
//! diagnostics and always emits [`TokenKind::Eof`], allowing callers to report
//! every lexical issue found in a source file.

mod error;
mod scanner;
mod token;

use crate::source::SourceText;

pub use error::{LexerError, LexerErrorKind};

pub use scanner::Lexer;

pub use token::{Token, TokenKind, keyword_or_identifier};

#[derive(Debug)]
/// The complete output of one lexer run.
pub struct LexResult {
    /// Tokens in source order, always ending with [`TokenKind::Eof`].
    pub tokens: Vec<Token>,
    /// Recoverable lexical diagnostics in source order.
    pub errors: Vec<LexerError>,
}

impl LexResult {
    #[must_use]
    /// Returns `true` when no lexer errors were collected.
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }

    #[must_use]
    /// Returns `true` when at least one lexer error was collected.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[must_use]
/// Lexes `source` into tokens and recoverable diagnostics.
///
/// The result always contains an EOF token, even if malformed input generated
/// diagnostics. This function performs no syntax or semantic validation.
pub fn lex(source: &SourceText) -> LexResult {
    Lexer::new(source).scan()
}
