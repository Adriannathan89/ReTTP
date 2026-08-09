//! Bounded `${VARIABLE}` interpolation for scalar runtime values.

use indexmap::IndexMap;
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use utest_domain::{InterpolatedString, Value, VariableName};
use utest_http::ResolvedValue;

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
            let (name, value) = self.lookup(raw_name, variables, location)?;
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

    pub(crate) fn resolve_value(
        &self,
        value: &Value,
        variables: &VariableStore,
        location: ResolutionLocation,
        allow_structured_placeholder: bool,
        depth: usize,
    ) -> Result<ResolvedValue, RuntimeError> {
        self.enter_depth(depth)?;
        match value {
            Value::String(input) => {
                if allow_structured_placeholder
                    && let Some((_name, stored)) =
                        self.exact_placeholder(input, variables, location)?
                    && stored.is_structured()
                {
                    return self.json_to_resolved(
                        stored
                            .as_json()
                            .expect("only captured JSON can be structured"),
                        depth,
                    );
                }
                self.interpolate(input, variables, location)
                    .map(ResolvedValue::String)
            }
            Value::Integer(value) => Ok(ResolvedValue::Integer(*value)),
            Value::Number(value) if value.is_finite() => Ok(ResolvedValue::Number(*value)),
            Value::Number(_) => Err(RuntimeError::NonFiniteNumber),
            Value::Boolean(value) => Ok(ResolvedValue::Boolean(*value)),
            Value::Null => Ok(ResolvedValue::Null),
            Value::Array(values) => values
                .iter()
                .map(|value| {
                    self.resolve_value(
                        value,
                        variables,
                        location,
                        allow_structured_placeholder,
                        depth + 1,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .map(ResolvedValue::Array),
            Value::Object(values) => values
                .iter()
                .map(|(name, value)| {
                    self.resolve_value(
                        value,
                        variables,
                        location,
                        allow_structured_placeholder,
                        depth + 1,
                    )
                    .map(|value| (name.clone(), value))
                })
                .collect::<Result<IndexMap<_, _>, _>>()
                .map(ResolvedValue::Object),
        }
    }

    pub(crate) fn resolve_json_value(
        &self,
        value: &Value,
        variables: &VariableStore,
        location: ResolutionLocation,
        depth: usize,
    ) -> Result<JsonValue, RuntimeError> {
        self.enter_depth(depth)?;
        match value {
            Value::String(input) => {
                if let Some((_name, stored)) = self.exact_placeholder(input, variables, location)?
                    && stored.is_structured()
                {
                    return self.clone_json_bounded(
                        stored
                            .as_json()
                            .expect("only captured JSON can be structured"),
                        depth,
                    );
                }
                self.interpolate(input, variables, location)
                    .map(JsonValue::String)
            }
            Value::Integer(value) => Ok(JsonValue::Number((*value).into())),
            Value::Number(value) => JsonNumber::from_f64(*value)
                .map(JsonValue::Number)
                .ok_or(RuntimeError::NonFiniteNumber),
            Value::Boolean(value) => Ok(JsonValue::Bool(*value)),
            Value::Null => Ok(JsonValue::Null),
            Value::Array(values) => values
                .iter()
                .map(|value| self.resolve_json_value(value, variables, location, depth + 1))
                .collect::<Result<Vec<_>, _>>()
                .map(JsonValue::Array),
            Value::Object(values) => values
                .iter()
                .map(|(name, value)| {
                    self.resolve_json_value(value, variables, location, depth + 1)
                        .map(|value| (name.clone(), value))
                })
                .collect::<Result<JsonMap<_, _>, _>>()
                .map(JsonValue::Object),
        }
    }

    fn exact_placeholder<'a>(
        &self,
        input: &InterpolatedString,
        variables: &'a VariableStore,
        location: ResolutionLocation,
    ) -> Result<Option<(VariableName, &'a crate::VariableValue)>, RuntimeError> {
        let raw = input.as_str();
        if !raw.starts_with("${") || !raw.ends_with('}') || raw[2..raw.len() - 1].contains('}') {
            return Ok(None);
        }
        let name = &raw[2..raw.len() - 1];
        self.lookup(name, variables, location).map(Some)
    }

    fn lookup<'a>(
        &self,
        raw_name: &str,
        variables: &'a VariableStore,
        location: ResolutionLocation,
    ) -> Result<(VariableName, &'a crate::VariableValue), RuntimeError> {
        if raw_name.is_empty() {
            return Err(RuntimeError::EmptyPlaceholder { location });
        }
        if raw_name.len() > self.config.max_interpolated_bytes() {
            return Err(RuntimeError::InterpolatedValueTooLarge {
                location,
                limit_bytes: self.config.max_interpolated_bytes(),
            });
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

    fn json_to_resolved(
        &self,
        value: &JsonValue,
        depth: usize,
    ) -> Result<ResolvedValue, RuntimeError> {
        self.enter_depth(depth)?;
        match value {
            JsonValue::Null => Ok(ResolvedValue::Null),
            JsonValue::Bool(value) => Ok(ResolvedValue::Boolean(*value)),
            JsonValue::Number(value) => {
                if let Some(value) = value.as_i64() {
                    Ok(ResolvedValue::Integer(value))
                } else if let Some(value) = value.as_u64() {
                    Ok(ResolvedValue::UnsignedInteger(value))
                } else {
                    Ok(ResolvedValue::Number(
                        value.as_f64().expect("serde_json numbers are finite"),
                    ))
                }
            }
            JsonValue::String(value) => Ok(ResolvedValue::String(value.clone())),
            JsonValue::Array(values) => values
                .iter()
                .map(|value| self.json_to_resolved(value, depth + 1))
                .collect::<Result<Vec<_>, _>>()
                .map(ResolvedValue::Array),
            JsonValue::Object(values) => values
                .iter()
                .map(|(name, value)| {
                    self.json_to_resolved(value, depth + 1)
                        .map(|value| (name.clone(), value))
                })
                .collect::<Result<IndexMap<_, _>, _>>()
                .map(ResolvedValue::Object),
        }
    }

    fn clone_json_bounded(
        &self,
        value: &JsonValue,
        depth: usize,
    ) -> Result<JsonValue, RuntimeError> {
        self.enter_depth(depth)?;
        match value {
            JsonValue::Null => Ok(JsonValue::Null),
            JsonValue::Bool(value) => Ok(JsonValue::Bool(*value)),
            JsonValue::Number(value) => Ok(JsonValue::Number(value.clone())),
            JsonValue::String(value) => Ok(JsonValue::String(value.clone())),
            JsonValue::Array(values) => values
                .iter()
                .map(|value| self.clone_json_bounded(value, depth + 1))
                .collect::<Result<Vec<_>, _>>()
                .map(JsonValue::Array),
            JsonValue::Object(values) => values
                .iter()
                .map(|(name, value)| {
                    self.clone_json_bounded(value, depth + 1)
                        .map(|value| (name.clone(), value))
                })
                .collect::<Result<JsonMap<_, _>, _>>()
                .map(JsonValue::Object),
        }
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

    fn enter_depth(&self, depth: usize) -> Result<(), RuntimeError> {
        if depth > self.config.max_resolution_depth() {
            return Err(RuntimeError::NestingLimitExceeded {
                limit: self.config.max_resolution_depth(),
            });
        }
        Ok(())
    }
}
