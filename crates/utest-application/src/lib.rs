//! Application use cases for UTest suites.
//!
//! This crate coordinates lower-level domain and parser capabilities without
//! introducing filesystem, terminal, or process concerns. Its source-checking
//! use case runs the lexical, syntax, and semantic phases in order and exposes
//! one diagnostic model to delivery layers such as the CLI.
//!
//! # Example
//!
//! ```
//! use utest_application::check_source;
//! use utest_parser::{SourceText, ValidationContext};
//!
//! let source = SourceText::new(
//!     "health.utest",
//!     r#"test "health" { request GET "/health" expect { status = 200 } }"#,
//! );
//! let report = check_source(&source, &ValidationContext::new());
//!
//! assert!(report.is_success());
//! assert!(report.suite.is_some());
//! ```

/// Source validation without filesystem, terminal, or process concerns.
pub mod check;

pub use check::*;
