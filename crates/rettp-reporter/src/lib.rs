//! Safe terminal, JSON, and JUnit reporting for Rettp execution results.
//!
//! The reporter crate never renders [`rettp_domain::SuiteResult`] directly.
//! [`RunReport::from_suite_result`] first creates an owned, versioned report
//! whose value-bearing assertion previews and execution messages have been
//! strictly sanitized. Every renderer accepts only that publishable model.
//!
//! The crate performs no filesystem, environment, network, or process access.
//! Callers decide where rendered UTF-8 output is written.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod json;
mod junit;
mod model;
mod terminal;

pub use json::{JsonReportError, JsonReporter};
pub use junit::JunitReporter;
pub use model::{
    REPORT_SCHEMA_VERSION, ReportAssertionFailure, ReportBlock, ReportBlockKind, ReportErrorKind,
    ReportExecutionError, ReportFailureKind, ReportStatus, ReportSummary, ReportTest, RunReport,
};
pub use terminal::{ColorMode, TerminalReporter};
