//! Parsing of JSON-like scalar, array, and object values.
//!
//! Recursive containers are protected by the parser nesting limit. Entry order
//! and duplicate object keys are preserved for later semantic validation.

use crate::{
    ArrayValueAst, ObjectValueAst, ObjectValueEntryAst, SourceSpan, Spanned, TokenKind, ValueAst,
};

use super::Parser;

impl Parser<'_> {
    pub(super) fn parse_value(&mut self) -> Option<ValueAst> {
        let span = self.current_span();
        match self.peek_kind()? {
            TokenKind::StringLiteral(value) => {
                let value = value.clone();
                self.bump();
                Some(ValueAst::String(Spanned::new(value, span)))
            }
            TokenKind::IntegerLiteral(value) => {
                let value = *value;
                self.bump();
                Some(ValueAst::Integer(Spanned::new(value, span)))
            }
            TokenKind::NumberLiteral(value) => {
                let value = *value;
                self.bump();
                Some(ValueAst::Number(Spanned::new(value, span)))
            }
            TokenKind::True => {
                self.bump();
                Some(ValueAst::Boolean(Spanned::new(true, span)))
            }
            TokenKind::False => {
                self.bump();
                Some(ValueAst::Boolean(Spanned::new(false, span)))
            }
            TokenKind::Null => {
                self.bump();
                Some(ValueAst::Null(span))
            }
            TokenKind::LeftBracket => Some(ValueAst::Array(self.parse_array_value())),
            TokenKind::LeftBrace => Some(ValueAst::Object(self.parse_object_value())),
            _ => None,
        }
    }

    pub(super) fn parse_object_value(&mut self) -> ObjectValueAst {
        let Some(left_brace) = self.take(&TokenKind::LeftBrace) else {
            let span = self.current_span();
            self.expected("`{` before object value");
            return ObjectValueAst {
                entries: Vec::new(),
                span: SourceSpan::new(span.start, span.start),
            };
        };
        let start = left_brace.span.start;
        if !self.enter_nesting(left_brace.span) {
            self.skip_balanced_container();
            return ObjectValueAst {
                entries: Vec::new(),
                span: self.span_from(start),
            };
        }
        let entries = self.parse_object_value_entries();
        self.consume(&TokenKind::RightBrace, "`}` after object value");
        let value = ObjectValueAst {
            entries,
            span: self.span_from(start),
        };
        self.exit_nesting();
        value
    }

    pub(super) fn parse_object_value_entries(&mut self) -> Vec<ObjectValueEntryAst> {
        let mut entries = Vec::new();

        while !self.is_at_end()
            && !self.check(&TokenKind::RightBrace)
            && !self.is_container_boundary()
        {
            let Some(key) = self.object_key() else {
                self.expected("object key or `}`");
                self.discard_unexpected_terminal();
                self.synchronize_object_entry();
                continue;
            };

            self.consume(&TokenKind::Equal, "`=` after object key");
            let Some(value) = self.parse_value() else {
                self.expected("literal, array, or object value");
                self.synchronize_object_entry();
                continue;
            };

            let span = SourceSpan::new(key.span.start, value.span().end);
            entries.push(ObjectValueEntryAst { key, value, span });
            self.skip_optional_comma();
        }

        entries
    }

    fn parse_array_value(&mut self) -> ArrayValueAst {
        let Some(left_bracket) = self.take(&TokenKind::LeftBracket) else {
            let span = self.current_span();
            self.expected("`[` before array value");
            return ArrayValueAst {
                items: Vec::new(),
                span: SourceSpan::new(span.start, span.start),
            };
        };
        let start = left_bracket.span.start;
        if !self.enter_nesting(left_bracket.span) {
            self.skip_balanced_container();
            return ArrayValueAst {
                items: Vec::new(),
                span: self.span_from(start),
            };
        }
        let mut items = Vec::new();

        while !self.is_at_end()
            && !self.check(&TokenKind::RightBracket)
            && !self.is_container_boundary()
        {
            let Some(value) = self.parse_value() else {
                self.expected("array value or `]`");
                self.discard_unexpected_terminal();
                self.synchronize_array_item();
                continue;
            };
            items.push(value);

            if self.take(&TokenKind::Comma).is_some() {
                continue;
            }
            if !self.check(&TokenKind::RightBracket) {
                self.expected("`,` or `]` after array value");
                self.synchronize_array_item();
            }
        }

        self.consume(&TokenKind::RightBracket, "`]` after array value");
        let value = ArrayValueAst {
            items,
            span: self.span_from(start),
        };
        self.exit_nesting();
        value
    }

    fn synchronize_object_entry(&mut self) {
        while !self.is_at_end()
            && !self.is_container_boundary()
            && !self.check(&TokenKind::Comma)
            && !self.check(&TokenKind::RightBrace)
        {
            self.bump();
        }
        self.skip_optional_comma();
    }

    fn synchronize_array_item(&mut self) {
        while !self.is_at_end()
            && !self.is_container_boundary()
            && !self.check(&TokenKind::Comma)
            && !self.check(&TokenKind::RightBracket)
        {
            self.bump();
        }
        self.skip_optional_comma();
    }
}

#[cfg(test)]
mod tests {
    use crate::{ParserErrorKind, SourceSpan, Token, TokenKind};

    use super::Parser;

    #[test]
    fn array_parser_returns_an_empty_recovery_node_without_an_opening_bracket() {
        let tokens = [Token {
            kind: TokenKind::Identifier("not-an-array".to_owned()),
            span: SourceSpan::new(2, 14),
        }];
        let mut parser = Parser::new(&tokens);

        let array = parser.parse_array_value();

        assert!(array.items.is_empty());
        assert_eq!(array.span, SourceSpan::new(2, 2));
        assert!(matches!(
            parser.errors.as_slice(),
            [error] if matches!(error.kind, ParserErrorKind::UnexpectedToken { .. })
        ));
    }
}
