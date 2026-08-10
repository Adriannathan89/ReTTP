//! Stable execution of the tracked fuzz seed and regression corpus.

use utest_application::check_source;
use utest_parser::{SourceSpan, SourceText, TokenKind, ValidationContext, lex, parse};

const FRONTEND_RECOVERY: &str = include_str!("../../../fuzz/corpus/parser_frontend/recovery.utest");
const CHECKER_PIPELINE: &str = include_str!("../../../fuzz/corpus/checker/pipeline.utest");

fn assert_valid_span(source: &SourceText, span: SourceSpan) {
    assert!(span.start <= span.end);
    assert!(span.end <= source.len());
    assert!(source.content().is_char_boundary(span.start));
    assert!(source.content().is_char_boundary(span.end));
    assert!(source.slice(span).is_some());
    let _ = source.location(span.start);
    let _ = source.location(span.end);
}

#[test]
fn frontend_recovery_corpus_completes_with_bounded_diagnostics() {
    let source = SourceText::new("recovery.utest", FRONTEND_RECOVERY);
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
    for diagnostic in &lexed.errors {
        assert_valid_span(&source, diagnostic.span);
    }
    let parsed = parse(&lexed.tokens);
    assert!(parsed.has_errors());
    assert_valid_span(&source, parsed.ast.span);
    for diagnostic in parsed.errors {
        assert_valid_span(&source, diagnostic.span);
    }
}

#[test]
fn complete_checker_corpus_preserves_all_or_nothing_conversion() {
    let source = SourceText::new("pipeline.utest", CHECKER_PIPELINE);
    let report = check_source(&source, &ValidationContext::new());
    assert_eq!(report.is_success(), report.suite.is_some());
    assert_eq!(report.has_errors(), report.suite.is_none());
    for diagnostic in report.diagnostics {
        assert_valid_span(&source, diagnostic.span);
        let _ = diagnostic.kind.to_string();
        let _ = diagnostic.phase().as_str();
    }
}
