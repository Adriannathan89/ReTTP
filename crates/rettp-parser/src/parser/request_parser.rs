//! Parsing of HTTP requests and their optional sections.
//!
//! Request sections preserve source order and duplicates for later semantic
//! validation. Headers, query values, and JSON-like object bodies share the
//! common value parser where applicable.

use crate::{
    HeaderValueEntryAst, HttpMethodAst, RequestAst, RequestBodyAst, RequestHeadersAst,
    RequestQueryAst, RequestSectionAst, SourceSpan, Spanned, TokenKind,
};

use super::Parser;

impl Parser<'_> {
    pub(super) fn parse_request(&mut self) -> RequestAst {
        let start = self
            .consume(&TokenKind::Request, "`request`")
            .map_or(self.current_span().start, |token| token.span.start);
        let method = self.parse_http_method();
        let path = self.string_literal("request path string");
        let mut sections = Vec::new();

        if self.take(&TokenKind::LeftBrace).is_some() {
            while !self.is_at_end()
                && !self.check(&TokenKind::RightBrace)
                && !self.is_test_boundary()
            {
                match self.peek_kind() {
                    Some(TokenKind::Headers) => {
                        sections.push(RequestSectionAst::Headers(self.parse_request_headers()))
                    }
                    Some(TokenKind::Query) => {
                        sections.push(RequestSectionAst::Query(self.parse_request_query()));
                    }
                    Some(TokenKind::Body) => {
                        sections.push(RequestSectionAst::Body(self.parse_request_body()));
                    }
                    Some(_) => {
                        self.expected("`headers`, `query`, `body`, or `}` in request");
                        self.bump();
                        self.synchronize_request_section();
                    }
                    None => break,
                }
            }
            self.consume(&TokenKind::RightBrace, "`}` after request options");
        }

        RequestAst {
            method,
            path,
            sections,
            span: self.span_from(start),
        }
    }

    fn parse_http_method(&mut self) -> Spanned<HttpMethodAst> {
        let span = self.current_span();
        let method = match self.peek_kind() {
            Some(TokenKind::Get) => HttpMethodAst::Get,
            Some(TokenKind::Post) => HttpMethodAst::Post,
            Some(TokenKind::Put) => HttpMethodAst::Put,
            Some(TokenKind::Patch) => HttpMethodAst::Patch,
            Some(TokenKind::Delete) => HttpMethodAst::Delete,
            Some(TokenKind::Head) => HttpMethodAst::Head,
            Some(TokenKind::Options) => HttpMethodAst::Options,
            _ => {
                self.expected("uppercase HTTP method");
                if !matches!(self.peek_kind(), Some(TokenKind::StringLiteral(_))) {
                    self.discard_unexpected_terminal();
                }
                return Spanned::new(HttpMethodAst::Get, span);
            }
        };
        self.bump();
        Spanned::new(method, span)
    }

    fn parse_request_headers(&mut self) -> RequestHeadersAst {
        let start = self
            .consume(&TokenKind::Headers, "`headers`")
            .map_or(self.current_span().start, |token| token.span.start);
        self.consume(&TokenKind::LeftBrace, "`{` after `headers`");
        let mut entries = Vec::new();

        while !self.is_at_end()
            && !self.check(&TokenKind::RightBrace)
            && !self.is_container_boundary()
        {
            let name = self.string_literal("quoted request header name");
            self.consume(&TokenKind::Equal, "`=` after request header name");
            let Some(value) = self.parse_value() else {
                self.expected("request header value");
                self.synchronize_request_entry();
                continue;
            };
            let span = SourceSpan::new(name.span.start, value.span().end);
            entries.push(HeaderValueEntryAst { name, value, span });
            self.skip_optional_comma();
        }

        self.consume(&TokenKind::RightBrace, "`}` after request headers");
        RequestHeadersAst {
            entries,
            span: self.span_from(start),
        }
    }

    fn parse_request_query(&mut self) -> RequestQueryAst {
        let start = self
            .consume(&TokenKind::Query, "`query`")
            .map_or(self.current_span().start, |token| token.span.start);
        let object = self.parse_object_value();
        RequestQueryAst {
            entries: object.entries,
            span: SourceSpan::new(start, object.span.end),
        }
    }

    fn parse_request_body(&mut self) -> RequestBodyAst {
        let start = self
            .consume(&TokenKind::Body, "`body`")
            .map_or(self.current_span().start, |token| token.span.start);
        let value = self.parse_object_value();
        RequestBodyAst {
            span: SourceSpan::new(start, value.span.end),
            value,
        }
    }

    fn synchronize_request_section(&mut self) {
        while !self.is_at_end()
            && !self.is_test_boundary()
            && !matches!(
                self.peek_kind(),
                Some(
                    TokenKind::Headers | TokenKind::Query | TokenKind::Body | TokenKind::RightBrace
                )
            )
        {
            self.bump();
        }
    }

    fn synchronize_request_entry(&mut self) {
        while !self.is_at_end()
            && !self.is_container_boundary()
            && !self.check(&TokenKind::Comma)
            && !self.check(&TokenKind::RightBrace)
        {
            self.bump();
        }
        self.skip_optional_comma();
    }
}
