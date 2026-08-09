//! Lexical and syntactic front end for the UTest DSL.
//!
//! Source text is converted into spanned [`Token`] values by [`lex`]. The token
//! stream can then be converted into a syntax-preserving [`SuiteAst`] with
//! [`parse`]. Semantic validation, variable resolution, HTTP I/O, and test
//! execution remain separate phases.
//!
//! The usual flow is `SourceText` → [`lex`] → [`parse`]:
//!
//! ```
//! use utest_parser::{BlockAst, SourceText, lex, parse};
//!
//! let source = SourceText::new(
//!     "example.utest",
//!     r#"test "health" { request GET "/health" expect { status = 200 } }"#,
//! );
//! let lexed = lex(&source);
//! let parsed = parse(&lexed.tokens);
//!
//! assert!(lexed.is_success());
//! assert!(parsed.is_success());
//! assert!(matches!(parsed.ast.blocks.as_slice(), [BlockAst::Test(_)]));
//! ```

/// Lexical analysis and token definitions for the UTest DSL.
pub mod lexer;
/// Source text, byte spans, and diagnostic locations.
pub mod source;

/// Recursive-descent parser and parser diagnostics.
pub mod parser;

/// Syntax tree nodes produced by the parser.
pub mod ast;

pub use ast::*;
pub use lexer::*;
pub use parser::*;
pub use source::*;
