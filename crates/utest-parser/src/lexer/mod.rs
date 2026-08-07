mod error;
mod scanner;
mod token;

use crate::source::SourceText;

pub use error::{LexerError, LexerErrorKind};

pub use scanner::Lexer;

pub use token::{Token, TokenKind, keyword_or_identifier};

#[derive(Debug)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub errors: Vec<LexerError>,
}

impl LexResult {
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }

    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[must_use]
pub fn lex(source: &SourceText) -> LexResult {
    Lexer::new(source).scan()
}
