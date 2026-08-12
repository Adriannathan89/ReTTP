//! Deterministic response assertion evaluation for Rettp.
//!
//! This crate compares a fully resolved [`ResolvedResponseExpectation`] with a
//! backend-neutral [`rettp_http::HttpResponse`]. It does not resolve variables,
//! commit captures, send HTTP requests, schedule suite blocks, or render
//! reports. Those responsibilities belong to later runtime layers.
//!
//! # Safety limits
//!
//! [`AssertionEngine`] bounds both recursive JSON traversal and retained
//! failures. This protects programmatic callers in addition to the response
//! byte and JSON-depth limits enforced by the HTTP adapter.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod config;
mod engine;
mod expectation;
mod report;

pub use config::{
    AssertionConfig, AssertionConfigError, DEFAULT_MAX_FAILURES, DEFAULT_MAX_JSON_DEPTH,
    HARD_MAX_JSON_DEPTH,
};
pub use engine::AssertionEngine;
pub use expectation::{
    ResolvedBodyAssertion, ResolvedFieldAssertion, ResolvedHeaderAssertion,
    ResolvedObjectAssertion, ResolvedResponseExpectation, ResolvedTextAssertion,
};
pub use report::AssertionReport;
