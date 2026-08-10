//! Application use cases for UTest suites.
//!
//! This crate coordinates lower-level compiler, runtime, HTTP-port, and
//! assertion capabilities without introducing filesystem, terminal, concrete
//! HTTP-adapter, or process concerns.
//!
//! [`check_source`] compiles source through lexical, syntax, and semantic
//! phases. [`ExecutionEngine`] executes an already validated domain suite
//! through a caller-supplied [`utest_http::HttpClient`].
//!
//! # Source checking
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

#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// Source validation without filesystem, terminal, or process concerns.
pub mod check;
/// Sequential suite execution over runtime and HTTP ports.
pub mod execution;

pub use check::*;
pub use execution::ExecutionEngine;
