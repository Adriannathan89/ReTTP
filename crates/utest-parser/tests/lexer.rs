use utest_parser::{LexerErrorKind, SourceLocation, SourceSpan, SourceText, TokenKind, lex};

fn lex_ok(input: &str) -> Vec<TokenKind> {
    let source = SourceText::new("test.utest", input);
    let result = lex(&source);
    assert!(
        result.is_success(),
        "unexpected errors: {:?}",
        result.errors
    );
    result.tokens.into_iter().map(|token| token.kind).collect()
}

#[test]
fn lexes_every_keyword_and_uppercase_http_method() {
    assert_eq!(
        lex_ok(
            "core pipeline test request expect body headers query status exact GET POST PUT PATCH DELETE HEAD OPTIONS string boolean integer number object array any true false null"
        ),
        vec![
            TokenKind::Core,
            TokenKind::Pipeline,
            TokenKind::Test,
            TokenKind::Request,
            TokenKind::Expect,
            TokenKind::Body,
            TokenKind::Headers,
            TokenKind::Query,
            TokenKind::Status,
            TokenKind::Exact,
            TokenKind::Get,
            TokenKind::Post,
            TokenKind::Put,
            TokenKind::Patch,
            TokenKind::Delete,
            TokenKind::Head,
            TokenKind::Options,
            TokenKind::TypeString,
            TokenKind::TypeBoolean,
            TokenKind::TypeInteger,
            TokenKind::TypeNumber,
            TokenKind::TypeObject,
            TokenKind::TypeArray,
            TokenKind::TypeAny,
            TokenKind::True,
            TokenKind::False,
            TokenKind::Null,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_empty_source_as_eof() {
    assert_eq!(lex_ok(""), vec![TokenKind::Eof]);
}

#[test]
fn preserves_case_sensitive_methods_and_identifiers_with_digits() {
    assert_eq!(
        lex_ok("get POST2 value123 _private"),
        vec![
            TokenKind::Identifier("get".into()),
            TokenKind::Identifier("POST2".into()),
            TokenKind::Identifier("value123".into()),
            TokenKind::Identifier("_private".into()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_symbols_numbers_and_assertion_forms() {
    assert_eq!(
        lex_ok("{ } [ ] : = -> , field: number = -10.5"),
        vec![
            TokenKind::LeftBrace,
            TokenKind::RightBrace,
            TokenKind::LeftBracket,
            TokenKind::RightBracket,
            TokenKind::Colon,
            TokenKind::Equal,
            TokenKind::Arrow,
            TokenKind::Comma,
            TokenKind::Identifier("field".into()),
            TokenKind::Colon,
            TokenKind::TypeNumber,
            TokenKind::Equal,
            TokenKind::NumberLiteral(-10.5),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_positive_integer_and_number_literals() {
    assert_eq!(
        lex_ok("0 200 10.5"),
        vec![
            TokenKind::IntegerLiteral(0),
            TokenKind::IntegerLiteral(200),
            TokenKind::NumberLiteral(10.5),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_integer_limits_and_reports_overflow_without_stopping() {
    let source = SourceText::new(
        "numbers.utest",
        "-9223372036854775808 9223372036854775808 core",
    );
    let result = lex(&source);
    assert_eq!(result.tokens[0].kind, TokenKind::IntegerLiteral(i64::MIN));
    assert!(
        matches!(result.errors.as_slice(), [error] if matches!(error.kind, LexerErrorKind::InvalidInteger { .. }))
    );
    assert!(
        result
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Core)
    );
    assert!(matches!(
        result.tokens.last().map(|token| &token.kind),
        Some(TokenKind::Eof)
    ));
}

#[test]
fn reports_float_overflow_without_stopping() {
    let source = SourceText::new("numbers.utest", format!("{}.0 core", "9".repeat(400)));
    let result = lex(&source);
    assert!(matches!(
        result.errors.as_slice(),
        [error] if matches!(error.kind, LexerErrorKind::InvalidNumber { .. })
    ));
    assert!(
        result
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Core)
    );
}

#[test]
fn lexes_escaped_and_interpolated_strings() {
    assert_eq!(
        lex_ok(r#""Bearer ${ACCESS_TOKEN}: \"x\"\\\n\r\t""#),
        vec![
            TokenKind::StringLiteral("Bearer ${ACCESS_TOKEN}: \"x\"\\\n\r\t".into()),
            TokenKind::Eof
        ]
    );
}

#[test]
fn lexes_unicode_string_literal() {
    assert_eq!(
        lex_ok("\"halo 你好 👋\""),
        vec![
            TokenKind::StringLiteral("halo 你好 👋".into()),
            TokenKind::Eof
        ]
    );
}

#[test]
fn ignores_all_whitespace_and_line_comment_styles() {
    assert_eq!(
        lex_ok("\n\tcore # comment\r\n// another\ntest\r"),
        vec![TokenKind::Core, TokenKind::Test, TokenKind::Eof]
    );
}

#[test]
fn ignores_comments_at_end_of_source() {
    assert_eq!(
        lex_ok("core # comment\n// final comment"),
        vec![TokenKind::Core, TokenKind::Eof]
    );
}

#[test]
fn lexes_a_representative_pipeline_source() {
    let input = r#"
        pipeline "authentication flow" {
            test "login" {
                request POST "/login"
                expect { status: integer = 200 body { access_token: string -> ACCESS_TOKEN } }
            }
        }
    "#;
    let source = SourceText::new("pipeline.utest", input);
    let result = lex(&source);
    assert!(result.is_success(), "{:?}", result.errors);
    assert!(
        result
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Pipeline)
    );
    assert!(
        result
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Arrow)
    );
}

#[test]
fn records_byte_spans_and_eof_for_unicode_source() {
    let source = SourceText::new("unicode.utest", "test \"你好\"");
    let result = lex(&source);
    assert!(result.is_success());
    assert_eq!(result.tokens[0].span, SourceSpan::new(0, 4));
    assert_eq!(result.tokens[1].span, SourceSpan::new(5, 13));
    assert_eq!(
        result.tokens[1].kind,
        TokenKind::StringLiteral("你好".into())
    );
    assert_eq!(result.tokens.last().unwrap().span, SourceSpan::new(13, 13));
}

#[test]
fn source_text_slice_and_location_are_utf8_boundary_safe() {
    let source = SourceText::new("unicode.utest", "你a\nxy");
    assert_eq!(source.name(), "unicode.utest");
    assert_eq!(source.content(), "你a\nxy");
    assert_eq!(source.len(), 7);
    assert!(!source.is_empty());
    assert_eq!(source.slice(SourceSpan::new(0, 3)), Some("你"));
    assert_eq!(source.slice(SourceSpan::new(1, 3)), None);
    assert_eq!(source.slice(SourceSpan::new(0, 99)), None);
    assert_eq!(source.location(1), SourceLocation { line: 1, column: 1 });
    assert_eq!(source.location(3), SourceLocation { line: 1, column: 2 });
    assert_eq!(source.location(4), SourceLocation { line: 1, column: 3 });
    assert_eq!(
        source.location(usize::MAX),
        SourceLocation { line: 2, column: 3 }
    );
}

#[test]
fn empty_source_text_reports_empty_and_first_location() {
    let source = SourceText::new("empty.utest", "");
    assert!(source.is_empty());
    assert_eq!(source.location(42), SourceLocation { line: 1, column: 1 });
}

#[test]
fn source_span_helpers_handle_reversed_ranges() {
    let span = SourceSpan::new(5, 3);
    assert_eq!(span.len(), 0);
    assert!(!span.is_empty());
    assert!(SourceSpan::new(3, 3).is_empty());
}

#[test]
fn collects_unexpected_characters_and_keeps_eof() {
    let source = SourceText::new("invalid.utest", "@ ? $ - /");
    let result = lex(&source);
    assert_eq!(result.errors.len(), 5);
    assert!(result.has_errors());
    assert!(matches!(
        result.errors[0].kind,
        LexerErrorKind::UnexpectedCharacter { character: '@' }
    ));
    assert!(matches!(
        result.tokens.last().map(|token| &token.kind),
        Some(TokenKind::Eof)
    ));
}

#[test]
fn reports_and_recovers_from_string_errors() {
    let source = SourceText::new(
        "invalid-string.utest",
        "\"bad\\q ignored\" core \"newline\n test",
    );
    let result = lex(&source);
    assert_eq!(result.errors.len(), 2);
    assert!(matches!(
        result.errors[0].kind,
        LexerErrorKind::InvalidEscapeSequence { character: 'q' }
    ));
    assert!(matches!(
        result.errors[1].kind,
        LexerErrorKind::UnterminatedString
    ));
    assert!(
        result
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Core)
    );
    assert!(
        result
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Test)
    );
}

#[test]
fn recovers_from_invalid_escape_at_newline() {
    let source = SourceText::new("invalid-string.utest", "\"bad\\q\ncore");
    let result = lex(&source);
    assert!(matches!(
        result.errors.as_slice(),
        [error] if matches!(error.kind, LexerErrorKind::InvalidEscapeSequence { character: 'q' })
    ));
    assert!(
        result
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Core)
    );
}

#[test]
fn reports_unterminated_string_at_eof() {
    let source = SourceText::new("invalid-string.utest", "\"unfinished");
    let result = lex(&source);
    assert!(
        matches!(result.errors.as_slice(), [error] if matches!(error.kind, LexerErrorKind::UnterminatedString))
    );
    assert_eq!(result.errors[0].span, SourceSpan::new(0, 11));
}

#[test]
fn reports_unterminated_string_when_escape_reaches_eof() {
    let source = SourceText::new("invalid-string.utest", "\"unfinished\\");
    let result = lex(&source);
    assert!(matches!(
        result.errors.as_slice(),
        [error] if matches!(error.kind, LexerErrorKind::UnterminatedString)
    ));
    assert!(matches!(
        result.tokens.last().map(|token| &token.kind),
        Some(TokenKind::Eof)
    ));
}

#[test]
fn lexer_error_display_messages_are_human_readable() {
    assert_eq!(
        LexerErrorKind::UnexpectedCharacter { character: '@' }.to_string(),
        "unexpected character `@`"
    );
    assert_eq!(
        LexerErrorKind::InvalidEscapeSequence { character: 'q' }.to_string(),
        "invalid escape sequence `\\q`"
    );
    assert_eq!(
        LexerErrorKind::InvalidInteger { value: "x".into() }.to_string(),
        "invalid integer literal `x`"
    );
    assert_eq!(
        LexerErrorKind::InvalidNumber { value: "x".into() }.to_string(),
        "invalid number literal `x`"
    );
}
