//! Bounded `${VARIABLE}` interpolation for scalar runtime values.

use utest_domain::{InterpolatedString, VariableName};

use crate::{ResolutionLocation, RuntimeConfig, RuntimeError, VariableStore};

/// Resolves placeholders using a validated allocation limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Interpolator {
    config: RuntimeConfig,
}

impl Interpolator {
    /// Creates an interpolator with validated runtime limits.
    #[must_use]
    pub const fn new(config: RuntimeConfig) -> Self {
        Self { config }
    }

    /// Returns the resource limits used by this interpolator.
    #[must_use]
    pub const fn config(self) -> RuntimeConfig {
        self.config
    }

    /// Resolves a string using scalar variables only.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError`] for malformed or missing placeholders,
    /// object/array variables, or output exceeding the configured byte limit.
    pub fn interpolate(
        &self,
        input: &InterpolatedString,
        variables: &VariableStore,
        location: ResolutionLocation,
    ) -> Result<String, RuntimeError> {
        let raw = input.as_str();
        let mut output = String::with_capacity(raw.len().min(self.config.max_interpolated_bytes()));
        let mut cursor = 0;

        while let Some(relative_start) = raw[cursor..].find("${") {
            let start = cursor + relative_start;
            self.push_bounded(&mut output, &raw[cursor..start], location)?;
            let name_start = start + 2;
            let Some(relative_end) = raw[name_start..].find('}') else {
                return Err(RuntimeError::UnterminatedPlaceholder { location });
            };
            let name_end = name_start + relative_end;
            let raw_name = &raw[name_start..name_end];
            if raw_name.len() > self.config.max_interpolated_bytes() {
                return Err(RuntimeError::InterpolatedValueTooLarge {
                    location,
                    limit_bytes: self.config.max_interpolated_bytes(),
                });
            }
            let (name, value) = lookup(raw_name, variables, location)?;
            let Some(value) = value.scalar_text() else {
                return Err(RuntimeError::UnsupportedInterpolationType {
                    name,
                    value_type: value.type_name(),
                    location,
                });
            };
            self.push_bounded(&mut output, &value, location)?;
            cursor = name_end + 1;
        }

        self.push_bounded(&mut output, &raw[cursor..], location)?;
        Ok(output)
    }

    fn push_bounded(
        &self,
        output: &mut String,
        fragment: &str,
        location: ResolutionLocation,
    ) -> Result<(), RuntimeError> {
        let limit = self.config.max_interpolated_bytes();
        if output
            .len()
            .checked_add(fragment.len())
            .is_none_or(|length| length > limit)
        {
            return Err(RuntimeError::InterpolatedValueTooLarge {
                location,
                limit_bytes: limit,
            });
        }
        output.push_str(fragment);
        Ok(())
    }
}

fn lookup<'a>(
    raw_name: &str,
    variables: &'a VariableStore,
    location: ResolutionLocation,
) -> Result<(VariableName, &'a crate::VariableValue), RuntimeError> {
    if raw_name.is_empty() {
        return Err(RuntimeError::EmptyPlaceholder { location });
    }
    let name = VariableName::new(raw_name).map_err(|_| RuntimeError::InvalidVariableName {
        name: raw_name.to_owned(),
        location,
    })?;
    let Some(value) = variables.get(&name) else {
        return Err(RuntimeError::UndefinedVariable { name, location });
    };
    Ok((name, value))
}
