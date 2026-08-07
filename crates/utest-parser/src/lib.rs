//! Lexical front end for the UTest DSL.
//!
//! This crate turns named UTF-8 source text into a stream of [`Token`] values
//! with byte spans and collects recoverable lexical diagnostics. It intentionally
//! does not parse an AST, validate DSL semantics, resolve variables, perform I/O,
//! or execute HTTP requests.
//!
//! The usual entry point is [`lex`]:
//!
//! ```
//! use utest_parser::{lex, SourceText, TokenKind};
//!
//! let source = SourceText::new("example.utest", "test \"health\"");
//! let result = lex(&source);
//!
//! assert!(result.is_success());
//! assert_eq!(result.tokens[0].kind, TokenKind::Test);
//! ```

/// Lexical analysis and token definitions for the UTest DSL.
pub mod lexer;
/// Source text, byte spans, and diagnostic locations.
pub mod source;

pub use lexer::*;
pub use source::*;
