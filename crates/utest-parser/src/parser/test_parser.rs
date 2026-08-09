//! Parsing of named test blocks.
//!
//! Tests retain duplicate request and expectation declarations while reporting
//! missing declarations and requests that appear after expectations.

use crate::{TestAst, TokenKind};

use super::{Parser, ParserErrorKind};

impl Parser<'_> {
    pub(super) fn parse_test(&mut self) -> TestAst {
        let start = self
            .consume(&TokenKind::Test, "`test`")
            .map_or(self.current_span().start, |token| token.span.start);
        let name = self.string_literal("test name string");
        self.consume(&TokenKind::LeftBrace, "`{` after test name");

        let mut requests = Vec::new();
        let mut expectations = Vec::new();
        let mut saw_expectation = false;

        while !self.is_at_end() && !self.check(&TokenKind::RightBrace) && !self.is_suite_boundary()
        {
            match self.peek_kind() {
                Some(TokenKind::Request) => {
                    if saw_expectation {
                        self.push_error(
                            ParserErrorKind::RequestAfterExpectation,
                            self.current_span(),
                        );
                    }
                    requests.push(self.parse_request());
                }
                Some(TokenKind::Expect) => {
                    saw_expectation = true;
                    expectations.push(self.parse_expectation());
                }
                Some(_) => {
                    self.expected("`request`, `expect`, or `}` in test block");
                    self.bump();
                    self.synchronize_test_item();
                }
                None => break,
            }
        }

        if requests.is_empty() {
            self.push_error(ParserErrorKind::MissingRequest, self.current_span());
        }
        if expectations.is_empty() {
            self.push_error(ParserErrorKind::MissingExpectation, self.current_span());
        }

        self.consume(&TokenKind::RightBrace, "`}` after test block");
        TestAst {
            name,
            requests,
            expectations,
            span: self.span_from(start),
        }
    }

    fn synchronize_test_item(&mut self) {
        while !self.is_at_end() && !self.is_test_boundary() && !self.check(&TokenKind::RightBrace) {
            self.bump();
        }
    }
}
