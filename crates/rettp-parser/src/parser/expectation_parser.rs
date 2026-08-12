//! Parsing of response expectations and field assertions.
//!
//! Supported assertions cover status codes, response headers, empty or textual
//! bodies, partial and exact top-level objects, typed fields, comparisons,
//! nested partial objects, and typed captures.

use crate::{
    AssertionTypeAst, BodyAssertionAst, ExpectationAst, ExpectationSectionAst, FieldAssertionAst,
    ObjectAssertionAst, ObjectMatchModeAst, ResponseHeaderAssertionAst, ResponseHeadersAst,
    SourceSpan, Spanned, StatusAssertionAst, TokenKind,
};

use super::{Parser, ParserErrorKind};

impl Parser<'_> {
    pub(super) fn parse_expectation(&mut self) -> ExpectationAst {
        let start = self
            .consume(&TokenKind::Expect, "`expect`")
            .map_or(self.current_span().start, |token| token.span.start);
        self.consume(&TokenKind::LeftBrace, "`{` after `expect`");
        let mut sections = Vec::new();

        while !self.is_at_end() && !self.check(&TokenKind::RightBrace) && !self.is_test_boundary() {
            match self.peek_kind() {
                Some(TokenKind::Status) => {
                    sections.push(ExpectationSectionAst::Status(self.parse_status_assertion()))
                }
                Some(TokenKind::Headers) => sections.push(ExpectationSectionAst::Headers(
                    self.parse_response_headers(),
                )),
                Some(TokenKind::Body) => {
                    sections.push(ExpectationSectionAst::Body(self.parse_body_assertion()))
                }
                Some(_) => {
                    self.expected("`status`, `headers`, `body`, or `}` in expectation");
                    self.bump();
                    self.synchronize_expectation_section();
                }
                None => break,
            }
        }

        self.consume(&TokenKind::RightBrace, "`}` after expectation");
        ExpectationAst {
            sections,
            span: self.span_from(start),
        }
    }

    fn parse_status_assertion(&mut self) -> StatusAssertionAst {
        let start = self
            .consume(&TokenKind::Status, "`status`")
            .map_or(self.current_span().start, |token| token.span.start);
        self.consume(&TokenKind::Equal, "`=` after `status`");

        let value_span = self.current_span();
        let expected = match self.peek_kind() {
            Some(TokenKind::IntegerLiteral(value)) => {
                let value = *value;
                self.bump();
                Spanned::new(value, value_span)
            }
            Some(_) => {
                self.expected("integer HTTP status");
                self.discard_unexpected_terminal();
                Spanned::new(0, value_span)
            }
            None => {
                self.expected("integer HTTP status");
                Spanned::new(0, value_span)
            }
        };

        self.skip_optional_comma();
        StatusAssertionAst {
            expected,
            span: self.span_from(start),
        }
    }

    fn parse_response_headers(&mut self) -> ResponseHeadersAst {
        let start = self
            .consume(&TokenKind::Headers, "`headers`")
            .map_or(self.current_span().start, |token| token.span.start);
        self.consume(&TokenKind::LeftBrace, "`{` after `headers`");
        let mut entries = Vec::new();

        while !self.is_at_end()
            && !self.check(&TokenKind::RightBrace)
            && !self.is_expectation_section_start()
            && !self.is_test_boundary()
        {
            let name = self.string_literal("quoted response header name");
            let entry_start = name.span.start;

            let entry = if self.take(&TokenKind::Colon).is_some() {
                let type_token =
                    self.consume(&TokenKind::TypeString, "`string` after response header `:`");
                let type_span = type_token
                    .as_ref()
                    .map_or(self.current_span(), |token| token.span);
                ResponseHeaderAssertionAst::Exists {
                    name,
                    type_span,
                    span: self.span_from(entry_start),
                }
            } else if self.take(&TokenKind::Equal).is_some() {
                let expected = self.string_literal("response header comparison string");
                ResponseHeaderAssertionAst::Exact {
                    name,
                    expected,
                    span: self.span_from(entry_start),
                }
            } else if self.take_contextual_identifier("contains") {
                let expected = self.string_literal("response header substring");
                ResponseHeaderAssertionAst::Contains {
                    name,
                    expected,
                    span: self.span_from(entry_start),
                }
            } else {
                self.expected("`: string`, `=`, or `contains` after response header name");
                self.synchronize_expectation_entry();
                continue;
            };

            entries.push(entry);
            self.skip_optional_comma();
        }

        self.consume(&TokenKind::RightBrace, "`}` after response headers");
        ResponseHeadersAst {
            entries,
            span: self.span_from(start),
        }
    }

    fn parse_body_assertion(&mut self) -> BodyAssertionAst {
        let start = self
            .consume(&TokenKind::Body, "`body`")
            .map_or(self.current_span().start, |token| token.span.start);

        if self.take_contextual_identifier("empty") {
            self.skip_optional_comma();
            return BodyAssertionAst::Empty {
                span: self.span_from(start),
            };
        }

        if self.take(&TokenKind::Equal).is_some() {
            let expected = self.string_literal("text body comparison string");
            self.skip_optional_comma();
            return BodyAssertionAst::TextExact {
                expected,
                span: self.span_from(start),
            };
        }

        if self.take_contextual_identifier("contains") {
            let expected = self.string_literal("text body substring");
            self.skip_optional_comma();
            return BodyAssertionAst::TextContains {
                expected,
                span: self.span_from(start),
            };
        }

        let mode = if self.take(&TokenKind::Exact).is_some() {
            ObjectMatchModeAst::Exact
        } else {
            ObjectMatchModeAst::Partial
        };
        let mut assertion = self.parse_object_assertion(mode);
        assertion.span = SourceSpan::new(start, assertion.span.end.max(start));
        BodyAssertionAst::Object(assertion)
    }

    fn parse_object_assertion(&mut self, mode: ObjectMatchModeAst) -> ObjectAssertionAst {
        let Some(left_brace) = self.take(&TokenKind::LeftBrace) else {
            let span = self.current_span();
            self.expected("`{` before object assertion");
            return ObjectAssertionAst {
                mode,
                fields: Vec::new(),
                span: SourceSpan::new(span.start, span.start),
            };
        };
        let start = left_brace.span.start;
        if !self.enter_nesting(left_brace.span) {
            self.skip_balanced_container();
            return ObjectAssertionAst {
                mode,
                fields: Vec::new(),
                span: self.span_from(start),
            };
        }
        let mut fields = Vec::new();

        while !self.is_at_end()
            && !self.check(&TokenKind::RightBrace)
            && !self.is_container_boundary()
        {
            if let Some(field) = self.parse_field_assertion() {
                fields.push(field);
                self.skip_optional_comma();
            } else {
                self.synchronize_expectation_entry();
            }
        }

        self.consume(&TokenKind::RightBrace, "`}` after object assertion");
        let assertion = ObjectAssertionAst {
            mode,
            fields,
            span: self.span_from(start),
        };
        self.exit_nesting();
        assertion
    }

    fn parse_field_assertion(&mut self) -> Option<FieldAssertionAst> {
        let Some(name) = self.object_key() else {
            self.expected("assertion field name or `}`");
            self.discard_unexpected_terminal();
            return None;
        };
        let start = name.span.start;
        let mut expected_type = None;
        let mut expected_value = None;
        let mut nested = None;

        if self.take(&TokenKind::Colon).is_some() {
            expected_type = self.parse_assertion_type();
        }

        if self.take(&TokenKind::Equal).is_some() {
            expected_value = self.parse_value();
            if expected_value.is_none() {
                self.expected("comparison value after `=`");
            }
        } else if expected_type
            .as_ref()
            .is_some_and(|kind| kind.value == AssertionTypeAst::Object)
            && self.check(&TokenKind::LeftBrace)
        {
            nested = Some(self.parse_object_assertion(ObjectMatchModeAst::Partial));
        }

        let capture = if self.take(&TokenKind::Arrow).is_some() {
            if expected_type.is_none() {
                self.push_error(ParserErrorKind::CaptureRequiresType, self.current_span());
            }
            Some(self.parse_capture_name())
        } else {
            None
        };

        if expected_type.is_none() && expected_value.is_none() && nested.is_none() {
            self.push_error(ParserErrorKind::MissingFieldAssertion, self.current_span());
        }

        Some(FieldAssertionAst {
            name,
            expected_type,
            expected_value,
            nested,
            capture,
            span: self.span_from(start),
        })
    }

    fn parse_assertion_type(&mut self) -> Option<Spanned<AssertionTypeAst>> {
        let span = self.current_span();
        let value = match self.peek_kind()? {
            TokenKind::TypeString => AssertionTypeAst::String,
            TokenKind::TypeBoolean => AssertionTypeAst::Boolean,
            TokenKind::TypeInteger => AssertionTypeAst::Integer,
            TokenKind::TypeNumber => AssertionTypeAst::Number,
            TokenKind::TypeObject => AssertionTypeAst::Object,
            TokenKind::TypeArray => AssertionTypeAst::Array,
            TokenKind::Null => AssertionTypeAst::Null,
            _ => {
                self.expected("assertion type");
                self.discard_unexpected_terminal();
                return None;
            }
        };
        self.bump();
        Some(Spanned::new(value, span))
    }

    fn parse_capture_name(&mut self) -> Spanned<String> {
        let span = self.current_span();
        match self.peek_kind() {
            Some(TokenKind::Identifier(value)) => {
                let value = value.clone();
                self.bump();
                Spanned::new(value, span)
            }
            Some(_) => {
                self.expected("capture variable identifier");
                self.discard_unexpected_terminal();
                Spanned::new(String::new(), span)
            }
            None => {
                self.expected("capture variable identifier");
                Spanned::new(String::new(), span)
            }
        }
    }

    fn synchronize_expectation_section(&mut self) {
        while !self.is_at_end()
            && !self.is_test_boundary()
            && !matches!(
                self.peek_kind(),
                Some(
                    TokenKind::Status
                        | TokenKind::Headers
                        | TokenKind::Body
                        | TokenKind::RightBrace
                )
            )
        {
            self.bump();
        }
    }

    fn synchronize_expectation_entry(&mut self) {
        while !self.is_at_end()
            && !self.is_test_boundary()
            && !self.check(&TokenKind::Comma)
            && !self.check(&TokenKind::RightBrace)
        {
            self.bump();
        }
        self.skip_optional_comma();
    }
}
