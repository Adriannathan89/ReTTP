#![no_main]

//! Exercises lexer and parser span invariants with bounded arbitrary UTF-8.

use libfuzzer_sys::fuzz_target;
use utest_parser::{SourceSpan, SourceText, TokenKind, lex, parse};

const MAX_FUZZ_INPUT_BYTES: usize = 64 * 1024;

fn assert_valid_span(source: &SourceText, span: SourceSpan) {
    assert!(span.start <= span.end);
    assert!(span.end <= source.len());
    assert!(source.content().is_char_boundary(span.start));
    assert!(source.content().is_char_boundary(span.end));
    let _ = source.slice(span);
    let _ = source.location(span.start);
    let _ = source.location(span.end);
}

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_FUZZ_INPUT_BYTES {
        return;
    }

    let content = String::from_utf8_lossy(data);
    let source = SourceText::new("fuzz.utest", content.as_ref());
    let lexed = lex(&source);

    assert!(
        lexed
            .tokens
            .last()
            .is_some_and(|token| matches!(token.kind, TokenKind::Eof))
    );
    for token in &lexed.tokens {
        assert_valid_span(&source, token.span);
    }
    for error in &lexed.errors {
        assert_valid_span(&source, error.span);
        let _ = format!("{error:?}");
    }

    let parsed = parse(&lexed.tokens);
    assert_valid_span(&source, parsed.ast.span);
    for error in &parsed.errors {
        assert_valid_span(&source, error.span);
        let _ = format!("{error:?}");
    }
});
