//! Domain model for a language-agnostic HTTP unit-test runner.
//!
//! This crate contains the serializable contract shared by the suite author,
//! parser, HTTP executor, and reporter. It deliberately has no HTTP client or
//! backend-framework dependency: adapters for any backend language translate
//! their input into [`TestSuite`] and execute the resulting [`TestCase`] values.
//!
//! A suite is composed of [`SuiteBlock`] values. Each test describes an
//! [`HttpRequestSpec`] and a [`ResponseExpectation`]. Values can include
//! [`InterpolatedString`] placeholders and assertions can store response data
//! in named [`Capture`] variables for later tests. Execution layers return the
//! corresponding result types from [`result`].

/// Values and interpolation primitives used in requests and expectations.
pub mod value;
/// Validation errors emitted while constructing domain values.
pub mod error;
/// Execution outcomes and assertion diagnostics produced by a runner.
pub mod result;
/// HTTP request descriptions independent of any HTTP client implementation.
pub mod request;
/// Variable identifiers and response-value captures.
pub mod variable;
/// JSON object assertion descriptions.
pub mod assertion;
/// Expected HTTP response status, headers, and body.
pub mod expectation;
/// An individual HTTP test case.
pub mod test_case;
/// Suite grouping primitives for core and pipeline execution.
pub mod block;
/// The top-level test suite aggregate.
pub mod suite;

pub use value::*;
pub use error::*;
pub use result::*;
pub use request::*;
pub use variable::*;
pub use assertion::*;
pub use expectation::*;
pub use test_case::*;
pub use block::*;
pub use suite::*;
