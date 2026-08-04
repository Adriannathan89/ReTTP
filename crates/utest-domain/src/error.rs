use thiserror::Error;

/*
 * Domain errors for handling various error conditions in the application.
 * Abstracts common error scenarios into a single enum for easier error handling and reporting.
 */

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