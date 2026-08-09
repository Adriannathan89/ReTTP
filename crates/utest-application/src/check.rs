//! Source-checking orchestration and phase-neutral diagnostics.
//!
//! [`check_source`] stops at the first unsuccessful compiler phase. This keeps
//! later diagnostics trustworthy: syntax is not attempted after lexical
//! failure, and semantic validation is not attempted after syntax failure.

use std::fmt;

use utest_domain::TestSuite;
use utest_parser::{
    LexerErrorKind, ParserErrorKind, SourceSpan, SourceText, ValidationContext,
    ValidationErrorKind, lex, parse, validate_and_convert,
};

/// A compiler phase that can emit a source-check diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckPhase {
    /// Tokenization of source text.
    Lexical,
    /// Parsing tokens into a syntax tree.
    Syntax,
    /// Contextual validation and domain conversion.
    Semantic,
}

impl CheckPhase {
    /// Returns the stable lowercase name used in user-facing diagnostics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lexical => "lexical",
            Self::Syntax => "syntax",
            Self::Semantic => "semantic",
        }
    }
}

/// The underlying error category reported by a source-check phase.
///
/// The variant preserves each parser-layer error type so clients may inspect
/// structured diagnostics instead of parsing display strings.
#[derive(Debug, Clone, PartialEq)]
pub enum CheckDiagnosticKind {
    /// A lexer error.
    Lexical(LexerErrorKind),
    /// A parser error.
    Syntax(ParserErrorKind),
    /// A semantic validator error.
    Semantic(ValidationErrorKind),
}

impl CheckDiagnosticKind {
    /// Returns the phase that produced this diagnostic kind.
    #[must_use]
    pub const fn phase(&self) -> CheckPhase {
        match self {
            Self::Lexical(_) => CheckPhase::Lexical,
            Self::Syntax(_) => CheckPhase::Syntax,
            Self::Semantic(_) => CheckPhase::Semantic,
        }
    }
}

impl fmt::Display for CheckDiagnosticKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lexical(kind) => fmt::Display::fmt(kind, formatter),
            Self::Syntax(kind) => fmt::Display::fmt(kind, formatter),
            Self::Semantic(kind) => fmt::Display::fmt(kind, formatter),
        }
    }
}

/// A source-check error and the byte span that caused it.
///
/// Use [`Self::phase`] to select a diagnostic label and use the span together
/// with [`SourceText::location`] to calculate line and column information.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckDiagnostic {
    /// Structured error emitted by the failing phase.
    pub kind: CheckDiagnosticKind,
    /// Half-open byte range in the checked source.
    pub span: SourceSpan,
}

impl CheckDiagnostic {
    /// Returns the compiler phase that emitted this diagnostic.
    #[must_use]
    pub const fn phase(&self) -> CheckPhase {
        self.kind.phase()
    }
}

/// Result of checking one source file.
///
/// Success contains a converted suite and no diagnostics. Failure contains
/// diagnostics from exactly one compiler phase and no partial suite.
#[derive(Debug)]
pub struct CheckReport {
    /// Converted domain suite, present only after all phases succeed.
    pub suite: Option<TestSuite>,
    /// Diagnostics from the first phase that failed.
    pub diagnostics: Vec<CheckDiagnostic>,
}

impl CheckReport {
    /// Returns `true` when the source passed every checking phase.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Returns `true` when the report contains one or more diagnostics.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }
}

/// Lexes, parses, validates, and converts a source definition.
///
/// The pipeline short-circuits after the first phase with errors:
///
/// 1. lexical errors prevent parsing;
/// 2. syntax errors prevent semantic validation;
/// 3. semantic errors prevent domain conversion.
///
/// This function performs no filesystem access and writes no output, making it
/// suitable for command-line tools, language servers, and editor integrations.
/// Predefined variables and nesting limits come from `context`.
///
/// # Examples
///
/// ```
/// use utest_application::{CheckPhase, check_source};
/// use utest_parser::{SourceText, ValidationContext};
///
/// let source = SourceText::new(
///     "invalid.utest",
///     r#"test "health" { request GET "/health" expect { status = 700 } }"#,
/// );
/// let report = check_source(&source, &ValidationContext::new());
///
/// assert!(report.has_errors());
/// assert_eq!(report.diagnostics[0].phase(), CheckPhase::Semantic);
/// assert!(report.suite.is_none());
/// ```
#[must_use]
pub fn check_source(source: &SourceText, context: &ValidationContext) -> CheckReport {
    let lexed = lex(source);
    if !lexed.errors.is_empty() {
        return CheckReport {
            suite: None,
            diagnostics: lexed
                .errors
                .into_iter()
                .map(|error| CheckDiagnostic {
                    kind: CheckDiagnosticKind::Lexical(error.kind),
                    span: error.span,
                })
                .collect(),
        };
    }

    let parsed = parse(&lexed.tokens);
    if !parsed.errors.is_empty() {
        return CheckReport {
            suite: None,
            diagnostics: parsed
                .errors
                .into_iter()
                .map(|error| CheckDiagnostic {
                    kind: CheckDiagnosticKind::Syntax(error.kind),
                    span: error.span,
                })
                .collect(),
        };
    }

    let validated = validate_and_convert(&parsed.ast, context);
    CheckReport {
        suite: validated.suite,
        diagnostics: validated
            .errors
            .into_iter()
            .map(|error| CheckDiagnostic {
                kind: CheckDiagnosticKind::Semantic(error.kind),
                span: error.span,
            })
            .collect(),
    }
}
