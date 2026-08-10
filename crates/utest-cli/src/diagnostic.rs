//! Compiler-style source diagnostics shared by `check` and `run`.

use std::io::{self, Write};

use utest_application::CheckDiagnostic;
use utest_parser::SourceText;

/// Writes one value-free compiler diagnostic to standard error.
pub(crate) fn render(source: &SourceText, diagnostic: &CheckDiagnostic) -> io::Result<()> {
    let location = source.location(diagnostic.span.start);
    writeln!(
        io::stderr().lock(),
        "{}:{}:{}: error[{}]: {}",
        source.name(),
        location.line,
        location.column,
        diagnostic.phase().as_str(),
        diagnostic.kind,
    )
}
