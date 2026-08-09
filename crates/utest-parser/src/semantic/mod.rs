//! Semantic validation and domain-model conversion.
//!
//! Parsing deliberately preserves syntactically valid input even when it
//! violates rules that require context, such as variable visibility, unique
//! sections, HTTP status ranges, or compatible assertion values. This module
//! performs those checks and converts a fully valid [`crate::SuiteAst`] into a
//! [`utest_domain::TestSuite`].
//!
//! Conversion is atomic: when any validation error is found, the result has no
//! suite. This prevents callers from accidentally executing a partially valid
//! definition.
//!
//! # Example
//!
//! ```
//! use utest_parser::{SourceText, ValidationContext, lex, parse, validate_and_convert};
//!
//! let source = SourceText::new(
//!     "health.utest",
//!     r#"test "health" { request GET "/health" expect { status = 200 } }"#,
//! );
//! let lexed = lex(&source);
//! let parsed = parse(&lexed.tokens);
//! let result = validate_and_convert(&parsed.ast, &ValidationContext::new());
//!
//! assert!(result.is_success());
//! assert!(result.suite.is_some());
//! ```

mod context;
mod converter;
mod error;
mod interpolation;
mod validator;

use utest_domain::TestSuite;

use crate::SuiteAst;

pub use context::{DEFAULT_MAX_SEMANTIC_DEPTH, HARD_MAX_SEMANTIC_DEPTH, ValidationContext};
pub use error::{DuplicateKind, ValidationError, ValidationErrorKind};

/// The outcome of semantic validation and conversion.
///
/// A successful result contains a domain suite and no errors. A failed result
/// contains one or more errors and never contains a partial suite.
#[derive(Debug)]
pub struct ValidationResult {
    /// The converted suite, present only when validation succeeds.
    pub suite: Option<TestSuite>,
    /// All semantic errors found during the validation pass.
    ///
    /// Errors are reported in deterministic traversal order.
    pub errors: Vec<ValidationError>,
}

impl ValidationResult {
    /// Returns `true` when validation produced no errors.
    ///
    /// A successful result also contains a value in [`Self::suite`].
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns `true` when at least one semantic error was reported.
    ///
    /// When this returns `true`, [`Self::suite`] is `None`.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Validates a parsed suite and converts it into the domain representation.
///
/// `context` supplies variables that exist before the suite begins and the
/// maximum allowed nesting depth. The validator also applies capture scope,
/// duplicate declaration, interpolation, HTTP, and assertion compatibility
/// rules.
///
/// The function collects all errors that can safely be diagnosed in one pass.
/// If any error is found, conversion is skipped and
/// [`ValidationResult::suite`] is `None`.
///
/// # Examples
///
/// ```
/// use utest_domain::VariableName;
/// use utest_parser::{SourceText, ValidationContext, lex, parse, validate_and_convert};
///
/// let source = SourceText::new(
///     "users.utest",
///     r#"test "user" { request GET "/users/${USER_ID}" expect { status = 200 } }"#,
/// );
/// let tokens = lex(&source).tokens;
/// let ast = parse(&tokens).ast;
/// let context = ValidationContext::new().with_predefined_variable(
///     VariableName::new("USER_ID").expect("valid variable name"),
/// );
///
/// assert!(validate_and_convert(&ast, &context).is_success());
/// ```
#[must_use]
pub fn validate_and_convert(ast: &SuiteAst, context: &ValidationContext) -> ValidationResult {
    let errors = validator::validate(ast, context);
    if !errors.is_empty() {
        return ValidationResult {
            suite: None,
            errors,
        };
    }

    ValidationResult {
        suite: Some(converter::convert(ast)),
        errors,
    }
}
