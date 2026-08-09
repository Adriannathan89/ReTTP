//! Recursive-descent parser for the UTest DSL.
//!
//! Parsing is syntax-oriented: declaration order and duplicates are preserved
//! in the AST so a later semantic-validation phase can produce precise
//! diagnostics. The parser performs bounded recovery and returns a partial AST
//! together with every recoverable [`ParserError`].

mod block_parser;
mod error;
mod expectation_parser;
mod request_parser;
mod suite_parser;
mod test_parser;
mod value_parser;

use std::mem::discriminant;

use crate::{SourceSpan, Spanned, SuiteAst, Token, TokenKind};

pub use error::{ParserError, ParserErrorKind};

/// Default maximum nesting depth for arrays, object values, and object assertions.
pub const DEFAULT_MAX_NESTING_DEPTH: usize = 128;
/// Absolute maximum nesting depth accepted by [`Parser::with_max_nesting_depth`].
///
/// Capping caller-provided limits prevents deeply nested input from causing
/// unbounded recursive parser calls.
pub const HARD_MAX_NESTING_DEPTH: usize = 256;

/// Result of parsing a token stream.
///
/// The AST may contain successfully recovered nodes even when `errors` is not
/// empty. Call [`ParseResult::is_success`] before treating it as valid syntax.
#[derive(Debug)]
pub struct ParseResult {
    /// Syntax-preserving suite AST, including nodes recovered after errors.
    pub ast: SuiteAst,
    /// Parser diagnostics in the order they were encountered.
    pub errors: Vec<ParserError>,
}

impl ParseResult {
    /// Returns `true` when parsing produced no diagnostics.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns `true` when one or more parser diagnostics were produced.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Stateful parser over a borrowed token slice.
///
/// Construct a parser with [`Parser::new`], optionally configure its nesting
/// limit, and consume it with [`Parser::parse`]. A trailing [`TokenKind::Eof`]
/// is supported but not required.
#[derive(Debug)]
pub struct Parser<'tokens> {
    tokens: &'tokens [Token],
    current: usize,
    nesting_depth: usize,
    max_nesting_depth: usize,
    errors: Vec<ParserError>,
}

impl<'tokens> Parser<'tokens> {
    /// Creates a parser using [`DEFAULT_MAX_NESTING_DEPTH`].
    #[must_use]
    pub const fn new(tokens: &'tokens [Token]) -> Self {
        Self {
            tokens,
            current: 0,
            nesting_depth: 0,
            max_nesting_depth: DEFAULT_MAX_NESTING_DEPTH,
            errors: Vec::new(),
        }
    }

    /// Sets the parser nesting limit.
    ///
    /// Values above [`HARD_MAX_NESTING_DEPTH`] are clamped to the hard limit.
    /// A value of zero rejects the first nested array, object value, or object
    /// assertion while still returning a recovered AST.
    #[must_use]
    pub const fn with_max_nesting_depth(mut self, limit: usize) -> Self {
        self.max_nesting_depth = if limit > HARD_MAX_NESTING_DEPTH {
            HARD_MAX_NESTING_DEPTH
        } else {
            limit
        };
        self
    }

    /// Parses the complete token stream into a suite AST and diagnostics.
    #[must_use]
    pub fn parse(self) -> ParseResult {
        self.parse_suite()
    }

    pub(super) fn is_at_end(&self) -> bool {
        self.peek_kind()
            .is_none_or(|kind| matches!(kind, TokenKind::Eof))
    }

    pub(super) fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.current)
    }

    pub(super) fn peek_kind(&self) -> Option<&TokenKind> {
        self.peek().map(|token| &token.kind)
    }

    pub(super) fn previous(&self) -> Option<&Token> {
        self.current
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
    }

    pub(super) fn advance(&mut self) -> Option<Token> {
        let token = self.peek()?.clone();
        self.bump();
        Some(token)
    }

    pub(super) fn bump(&mut self) -> bool {
        if self.is_at_end() {
            return false;
        }

        self.current += 1;
        true
    }

    pub(super) fn check(&self, expected: &TokenKind) -> bool {
        self.peek_kind()
            .is_some_and(|actual| discriminant(actual) == discriminant(expected))
    }

    pub(super) fn take(&mut self, expected: &TokenKind) -> Option<Token> {
        self.check(expected).then(|| self.advance()).flatten()
    }

    pub(super) fn consume(
        &mut self,
        expected_kind: &TokenKind,
        expected: &'static str,
    ) -> Option<Token> {
        if let Some(token) = self.take(expected_kind) {
            return Some(token);
        }

        self.expected(expected);
        None
    }

    pub(super) fn expected(&mut self, expected: &'static str) {
        let (kind, span) = match self.peek() {
            Some(token) if !matches!(token.kind, TokenKind::Eof) => (
                ParserErrorKind::UnexpectedToken {
                    expected,
                    found: token_description(&token.kind),
                },
                token.span,
            ),
            _ => (
                ParserErrorKind::UnexpectedEof { expected },
                self.current_span(),
            ),
        };

        self.push_error(kind, span);
    }

    pub(super) fn push_error(&mut self, kind: ParserErrorKind, span: SourceSpan) {
        self.errors.push(ParserError::new(kind, span));
    }

    pub(super) fn current_span(&self) -> SourceSpan {
        self.peek().map_or_else(
            || {
                let end = self.tokens.last().map_or(0, |token| token.span.end);
                SourceSpan::new(end, end)
            },
            |token| token.span,
        )
    }

    pub(super) fn span_from(&self, start: usize) -> SourceSpan {
        let end = self
            .previous()
            .map_or(start, |token| token.span.end.max(start));
        SourceSpan::new(start, end)
    }

    pub(super) fn skip_optional_comma(&mut self) {
        self.take(&TokenKind::Comma);
    }

    pub(super) fn is_contextual_identifier(&self, expected: &str) -> bool {
        matches!(
            self.peek_kind(),
            Some(TokenKind::Identifier(value)) if value == expected
        )
    }

    pub(super) fn take_contextual_identifier(&mut self, expected: &str) -> bool {
        if !self.is_contextual_identifier(expected) {
            return false;
        }

        self.bump();
        true
    }

    pub(super) fn string_literal(&mut self, expected: &'static str) -> Spanned<String> {
        match self.advance_if_string() {
            Some(value) => value,
            None => {
                let span = self.current_span();
                self.expected(expected);

                Spanned::new(String::new(), span)
            }
        }
    }

    pub(super) fn object_key(&mut self) -> Option<Spanned<String>> {
        let token = self.peek()?;
        let span = token.span;

        let value = match &token.kind {
            TokenKind::Identifier(value) | TokenKind::StringLiteral(value) => value.as_str(),
            TokenKind::Status => "status",
            TokenKind::Exact => "exact",
            TokenKind::Get => "GET",
            TokenKind::Post => "POST",
            TokenKind::Put => "PUT",
            TokenKind::Patch => "PATCH",
            TokenKind::Delete => "DELETE",
            TokenKind::Head => "HEAD",
            TokenKind::Options => "OPTIONS",
            TokenKind::TypeString => "string",
            TokenKind::TypeBoolean => "boolean",
            TokenKind::TypeInteger => "integer",
            TokenKind::TypeNumber => "number",
            TokenKind::TypeObject => "object",
            TokenKind::TypeArray => "array",
            TokenKind::True => "true",
            TokenKind::False => "false",
            TokenKind::Null => "null",
            _ => return None,
        }
        .to_owned();

        self.bump();
        Some(Spanned::new(value, span))
    }

    fn advance_if_string(&mut self) -> Option<Spanned<String>> {
        let token = self.peek()?;
        let span = token.span;
        let TokenKind::StringLiteral(value) = &token.kind else {
            return None;
        };
        let value = value.clone();
        self.bump();
        Some(Spanned::new(value, span))
    }

    pub(super) fn discard_unexpected_terminal(&mut self) {
        if !self.is_recovery_boundary() {
            self.bump();
        }
    }

    pub(super) fn is_suite_boundary(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(TokenKind::Core | TokenKind::Pipeline | TokenKind::Test)
        )
    }

    pub(super) fn is_outer_block_boundary(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(TokenKind::Core | TokenKind::Pipeline)
        )
    }

    pub(super) fn is_test_boundary(&self) -> bool {
        self.is_suite_boundary()
            || matches!(
                self.peek_kind(),
                Some(TokenKind::Request | TokenKind::Expect)
            )
    }

    pub(super) fn is_expectation_section_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(TokenKind::Status | TokenKind::Headers | TokenKind::Body)
        )
    }

    pub(super) fn is_container_boundary(&self) -> bool {
        self.is_test_boundary()
            || matches!(
                self.peek_kind(),
                Some(TokenKind::Headers | TokenKind::Query | TokenKind::Body)
            )
    }

    pub(super) fn enter_nesting(&mut self, span: SourceSpan) -> bool {
        if self.nesting_depth >= self.max_nesting_depth {
            self.push_error(
                ParserErrorKind::NestingLimitExceeded {
                    limit: self.max_nesting_depth,
                },
                span,
            );
            return false;
        }

        self.nesting_depth += 1;
        true
    }

    pub(super) fn exit_nesting(&mut self) {
        self.nesting_depth = self.nesting_depth.saturating_sub(1);
    }

    pub(super) fn skip_balanced_container(&mut self) {
        let mut depth = 1_usize;

        while !self.is_at_end() && depth > 0 {
            match self.peek_kind() {
                Some(TokenKind::LeftBrace | TokenKind::LeftBracket) => depth += 1,
                Some(TokenKind::RightBrace | TokenKind::RightBracket) => depth -= 1,
                _ => {}
            }
            self.bump();
        }
    }

    fn is_recovery_boundary(&self) -> bool {
        matches!(
            self.peek_kind(),
            None | Some(
                TokenKind::Eof
                    | TokenKind::Core
                    | TokenKind::Pipeline
                    | TokenKind::Test
                    | TokenKind::Request
                    | TokenKind::Expect
                    | TokenKind::Status
                    | TokenKind::Headers
                    | TokenKind::Query
                    | TokenKind::Body
                    | TokenKind::Exact
                    | TokenKind::LeftBrace
                    | TokenKind::RightBrace
                    | TokenKind::LeftBracket
                    | TokenKind::RightBracket
                    | TokenKind::Colon
                    | TokenKind::Equal
                    | TokenKind::Arrow
                    | TokenKind::Comma
            )
        )
    }
}

/// Parses a token slice using the default parser configuration.
///
/// This is the convenient entry point after lexical analysis with [`crate::lex`].
/// For a custom nesting limit, construct a [`Parser`] directly.
///
/// # Example
///
/// ```
/// use utest_parser::{BlockAst, SourceText, lex, parse};
///
/// let source = SourceText::new(
///     "health.utest",
///     r#"test "health" { request GET "/health" expect { status = 200 } }"#,
/// );
/// let lexed = lex(&source);
/// let parsed = parse(&lexed.tokens);
///
/// assert!(parsed.is_success());
/// assert!(matches!(parsed.ast.blocks.as_slice(), [BlockAst::Test(_)]));
/// ```
#[must_use]
pub fn parse(tokens: &[Token]) -> ParseResult {
    Parser::new(tokens).parse()
}

fn token_description(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Identifier(value) => format!("identifier `{value}`"),
        TokenKind::StringLiteral(_) => "string literal".to_owned(),
        TokenKind::IntegerLiteral(value) => format!("integer `{value}`"),
        TokenKind::NumberLiteral(value) => format!("number `{value}`"),
        other => format!("`{other:?}`"),
    }
}

#[cfg(test)]
mod tests {
    use super::Parser;

    #[test]
    fn bump_does_not_advance_an_empty_parser() {
        let mut parser = Parser::new(&[]);
        assert!(!parser.bump());
        assert_eq!(parser.current, 0);
    }
}
