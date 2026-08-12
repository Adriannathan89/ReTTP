//! Parsing and recovery at the suite root.
//!
//! The suite parser recognizes core, pipeline, and standalone test blocks,
//! preserves their source order, and reports empty suites and duplicate core
//! declarations.

use crate::{BlockAst, SourceSpan, SuiteAst, TokenKind};

use super::{ParseResult, Parser, ParserErrorKind};

impl Parser<'_> {
    pub(super) fn parse_suite(mut self) -> ParseResult {
        let start = self.peek().map_or(0, |token| token.span.start);
        let mut blocks = Vec::new();
        let mut has_core = false;

        while !self.is_at_end() {
            match self.peek_kind() {
                Some(TokenKind::Core) => {
                    let keyword_span = self.current_span();
                    if has_core {
                        self.push_error(ParserErrorKind::DuplicateCore, keyword_span);
                    }
                    has_core = true;
                    blocks.push(BlockAst::Core(self.parse_core_block()));
                }
                Some(TokenKind::Pipeline) => {
                    blocks.push(BlockAst::Pipeline(self.parse_pipeline_block()));
                }
                Some(TokenKind::Test) => {
                    blocks.push(BlockAst::Test(self.parse_test()));
                }
                Some(_) => {
                    self.expected("`core`, `pipeline`, `test`, or end of input");
                    self.bump();
                    self.synchronize_suite();
                }
                None => break,
            }
        }

        let span = if let Some(last) = blocks.last() {
            SourceSpan::new(start, last.span().end)
        } else {
            SourceSpan::new(start, start)
        };
        if blocks.is_empty() && self.errors.is_empty() {
            self.push_error(ParserErrorKind::EmptySuite, span);
        }

        ParseResult {
            ast: SuiteAst { blocks, span },
            errors: self.errors,
        }
    }

    fn synchronize_suite(&mut self) {
        while !self.is_at_end()
            && !matches!(
                self.peek_kind(),
                Some(TokenKind::Core | TokenKind::Pipeline | TokenKind::Test)
            )
        {
            self.bump();
        }
    }
}
