use std::fmt;

use crate::DomainError;
use serde::{Deserialize, Serialize};

/// A validated identifier for a value shared between test cases.
///
/// Names must be non-empty, start with an alphabetic character or `_`, and
/// then contain only alphanumeric characters or `_`. This keeps capture and
/// interpolation syntax portable across backend-language adapters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct VariableName(String);

impl VariableName {
    /// Validates and creates a variable name.
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();

        if value.is_empty() {
            return Err(DomainError::EmptyVariableName);
        }

        let mut characters = value.chars();

        let first = characters
            .next()
            .expect("a non-empty string must have a first character");

        if !first.is_alphabetic() && first != '_' {
            return Err(DomainError::InvalidVariableName { name: value });
        }

        if !characters.all(|c| c.is_alphanumeric() || c == '_') {
            return Err(DomainError::InvalidVariableName { name: value });
        }

        Ok(Self(value))
    }

    /// Returns the validated identifier without allocating.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VariableName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A request to save an extracted response value under a variable name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capture {
    pub variable: VariableName,
}

impl Capture {
    /// Creates a capture target for a validated variable name.
    #[must_use]
    pub const fn new(variable: VariableName) -> Self {
        Self { variable }
    }
}
