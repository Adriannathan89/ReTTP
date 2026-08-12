//! Parsing of top-level `core` and `pipeline` blocks.
//!
//! This module also recovers within test containers while preserving valid
//! tests that follow malformed entries.

use crate::{CoreBlockAst, PipelineBlockAst, Spanned, TokenKind};

use super::{Parser, ParserErrorKind};

impl Parser<'_> {
    pub(super) fn parse_core_block(&mut self) -> CoreBlockAst {
        let start = self
            .consume(&TokenKind::Core, "`core`")
            .map_or(self.current_span().start, |token| token.span.start);
        self.consume(&TokenKind::LeftBrace, "`{` after `core`");

        let mut tests = Vec::new();
        while !self.is_at_end()
            && !self.check(&TokenKind::RightBrace)
            && !self.is_outer_block_boundary()
        {
            if self.check(&TokenKind::Test) {
                tests.push(self.parse_test());
            } else {
                self.expected("`test` or `}` in core block");
                self.bump();
                self.synchronize_test_container();
            }
        }

        self.consume(&TokenKind::RightBrace, "`}` after core block");
        CoreBlockAst {
            tests,
            span: self.span_from(start),
        }
    }

    pub(super) fn parse_pipeline_block(&mut self) -> PipelineBlockAst {
        let start = self
            .consume(&TokenKind::Pipeline, "`pipeline`")
            .map_or(self.current_span().start, |token| token.span.start);
        let name = self.string_literal("pipeline name string");
        self.consume(&TokenKind::LeftBrace, "`{` after pipeline name");

        let mut tests = Vec::new();
        while !self.is_at_end()
            && !self.check(&TokenKind::RightBrace)
            && !self.is_outer_block_boundary()
        {
            if self.check(&TokenKind::Test) {
                tests.push(self.parse_test());
            } else {
                self.expected("`test` or `}` in pipeline block");
                self.bump();
                self.synchronize_test_container();
            }
        }

        if tests.is_empty() {
            self.push_error(ParserErrorKind::EmptyPipeline, self.current_span());
        }

        self.consume(&TokenKind::RightBrace, "`}` after pipeline block");
        PipelineBlockAst {
            name: Spanned::new(name.value, name.span),
            tests,
            span: self.span_from(start),
        }
    }

    fn synchronize_test_container(&mut self) {
        while !self.is_at_end() && !self.is_suite_boundary() && !self.check(&TokenKind::RightBrace)
        {
            self.bump();
        }
    }
}
