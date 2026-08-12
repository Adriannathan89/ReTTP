//! Runtime variable resolution and transactional response capture for Rettp.
//!
//! This crate bridges unresolved [`rettp_domain`] models with the resolved
//! request and expectation models consumed by later runtime stages. It does
//! not send requests or schedule suite blocks; those responsibilities belong
//! to the execution engine.
//!
//! Variable values are redacted from diagnostics and `Debug` output.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod capture;
mod config;
mod error;
mod interpolation;
mod resolver;
mod variable;

pub use capture::{CaptureEngine, CaptureEvaluation, PendingCaptures};
pub use config::{
    DEFAULT_MAX_INTERPOLATED_BYTES, DEFAULT_MAX_RESOLUTION_DEPTH, HARD_MAX_INTERPOLATED_BYTES,
    HARD_MAX_RESOLUTION_DEPTH, RuntimeConfig, RuntimeConfigError,
};
pub use error::{ResolutionLocation, RuntimeError, VariableAssignmentError};
pub use interpolation::Interpolator;
pub use resolver::RuntimeResolver;
pub use variable::{VariableAssignment, VariableStore, VariableValue};
