use thiserror::Error;

/// Validation errors returned while constructing domain values.
///
/// Parsers and suite authoring APIs can map these stable, backend-neutral errors
/// to their own diagnostics without depending on an HTTP implementation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainError {
    #[error("variable name cannot be empty")]
    EmptyVariableName,

    #[error("invalid variable name: {name}")]
    InvalidVariableName { name: String },

    #[error("test name cannot be empty")]
    EmptyTestName,

    #[error("pipeline name cannot be empty")]
    EmptyPipelineName,

    #[error("HTTP status code {status_code} is invalid")]
    InvalidHttpStatusCode { status_code: u16 },

    #[error("field name cannot be empty")]
    EmptyFieldName,

    #[error("request path cannot be empty")]
    EmptyRequestPath,
}
