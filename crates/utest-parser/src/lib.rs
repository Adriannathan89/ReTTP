//! Lexical and syntactic front end for the UTest DSL.
//!
//! Source text is converted into spanned [`Token`] values by [`lex`]. The token
//! stream can then be converted into a syntax-preserving [`SuiteAst`] with
//! [`parse`]. Finally, [`validate_and_convert`] checks cross-node rules and
//! converts a valid tree into a domain [`utest_domain::TestSuite`]. HTTP I/O
//! and test execution remain separate phases.
//!
//! The usual flow is `SourceText` → [`lex`] → [`parse`] →
//! [`validate_and_convert`]:
//!
//! ```
//! use utest_parser::{SourceText, ValidationContext, lex, parse, validate_and_convert};
//!
//! let source = SourceText::new(
//!     "example.utest",
//!     r#"test "health" { request GET "/health" expect { status = 200 } }"#,
//! );
//! let lexed = lex(&source);
//! let parsed = parse(&lexed.tokens);
//! let validated = validate_and_convert(&parsed.ast, &ValidationContext::new());
//!
//! assert!(lexed.is_success());
//! assert!(parsed.is_success());
//! assert!(validated.is_success());
//! ```

/// Lexical analysis and token definitions for the UTest DSL.
pub mod lexer;
/// Source text, byte spans, and diagnostic locations.
pub mod source;

/// Recursive-descent parser and parser diagnostics.
pub mod parser;

/// Semantic validation and conversion from syntax trees to domain models.
pub mod semantic;

/// Syntax tree nodes produced by the parser.
pub mod ast;

pub use ast::*;
pub use lexer::*;
pub use parser::*;
pub use semantic::*;
pub use source::*;
