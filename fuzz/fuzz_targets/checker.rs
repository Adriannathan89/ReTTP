#![no_main]

//! Exercises the complete checker and its all-or-nothing conversion contract.

use libfuzzer_sys::fuzz_target;
use rettp_application::check_source;
use rettp_parser::{SourceSpan, SourceText, ValidationContext};

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
    let source = SourceText::new("fuzz.rttp", content.as_ref());
    let report = check_source(&source, &ValidationContext::new());

    assert_eq!(report.is_success(), report.suite.is_some());
    assert_eq!(report.has_errors(), report.suite.is_none());
    for diagnostic in &report.diagnostics {
        assert_valid_span(&source, diagnostic.span);
        let _ = diagnostic.kind.to_string();
        let _ = diagnostic.phase().as_str();
    }
});
