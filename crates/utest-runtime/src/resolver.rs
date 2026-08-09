//! Conversion of unresolved domain requests and expectations into runtime models.

use std::time::Duration;

use bytes::Bytes;
use indexmap::IndexMap;
use utest_assertion::{
    ResolvedBodyAssertion, ResolvedFieldAssertion, ResolvedHeaderAssertion,
    ResolvedObjectAssertion, ResolvedResponseExpectation, ResolvedTextAssertion,
};
use utest_domain::{
    BodyAssertion, FieldAssertion, HeaderAssertion, HttpRequestSpec, ObjectAssertion, RequestBody,
    ResponseExpectation, TextAssertion, Value,
};
use utest_http::{ResolvedHttpRequest, ResolvedRequestBody, ResolvedValue};

use crate::{Interpolator, ResolutionLocation, RuntimeConfig, RuntimeError, VariableStore};

/// Resolves domain requests and expectations against one variable scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimeResolver {
    interpolator: Interpolator,
}

impl RuntimeResolver {
    /// Creates a resolver using validated runtime resource limits.
    #[must_use]
    pub const fn new(config: RuntimeConfig) -> Self {
        Self {
            interpolator: Interpolator::new(config),
        }
    }

    /// Returns the resource limits applied by this resolver.
    #[must_use]
    pub const fn config(self) -> RuntimeConfig {
        self.interpolator.config()
    }

    /// Resolves every interpolated request location in declaration order.
    ///
    /// Exact object and array placeholders preserve their JSON type only in a
    /// JSON request body. Other locations accept scalar interpolation only.
    ///
    /// # Errors
    ///
    /// Returns the first deterministic [`RuntimeError`] encountered.
    pub fn resolve_request(
        &self,
        request: &HttpRequestSpec,
        variables: &VariableStore,
    ) -> Result<ResolvedHttpRequest, RuntimeError> {
        let path = self.interpolator.interpolate(
            &request.path,
            variables,
            ResolutionLocation::RequestPath,
        )?;
        let headers = request
            .headers
            .iter()
            .map(|(name, value)| {
                self.resolve_value(
                    value,
                    variables,
                    ResolutionLocation::RequestHeader,
                    false,
                    0,
                )
                .map(|value| (name.clone(), value))
            })
            .collect::<Result<IndexMap<_, _>, _>>()?;
        let query = request
            .query
            .iter()
            .map(|(name, value)| {
                self.resolve_value(
                    value,
                    variables,
                    ResolutionLocation::QueryParameter,
                    false,
                    0,
                )
                .map(|value| (name.clone(), value))
            })
            .collect::<Result<IndexMap<_, _>, _>>()?;
        let body = request
            .body
            .as_ref()
            .map(|body| self.resolve_request_body(body, variables))
            .transpose()?;

        Ok(ResolvedHttpRequest {
            method: request.method,
            path,
            headers,
            query,
            body,
            timeout: request.timeout_ms.map(Duration::from_millis),
        })
    }

    /// Resolves expected response strings and recursive JSON values.
    ///
    /// Capture declarations are copied unchanged for the later capture phase.
    ///
    /// # Errors
    ///
    /// Returns the first deterministic [`RuntimeError`] encountered.
    pub fn resolve_expectation(
        &self,
        expectation: &ResponseExpectation,
        variables: &VariableStore,
    ) -> Result<ResolvedResponseExpectation, RuntimeError> {
        let headers = expectation
            .headers
            .iter()
            .map(|(name, assertion)| {
                let assertion = match assertion {
                    HeaderAssertion::Exists => ResolvedHeaderAssertion::Exists,
                    HeaderAssertion::Exact(value) => {
                        ResolvedHeaderAssertion::Exact(self.interpolator.interpolate(
                            value,
                            variables,
                            ResolutionLocation::ExpectedHeader,
                        )?)
                    }
                    HeaderAssertion::Contains(value) => {
                        ResolvedHeaderAssertion::Contains(self.interpolator.interpolate(
                            value,
                            variables,
                            ResolutionLocation::ExpectedHeader,
                        )?)
                    }
                };
                Ok((name.clone(), assertion))
            })
            .collect::<Result<IndexMap<_, _>, RuntimeError>>()?;
        let body = expectation
            .body
            .as_ref()
            .map(|body| self.resolve_body_assertion(body, variables))
            .transpose()?;

        Ok(ResolvedResponseExpectation {
            status: expectation.status,
            headers,
            body,
        })
    }

    fn resolve_request_body(
        &self,
        body: &RequestBody,
        variables: &VariableStore,
    ) -> Result<ResolvedRequestBody, RuntimeError> {
        match body {
            RequestBody::Json(value) => self
                .resolve_value(
                    value,
                    variables,
                    ResolutionLocation::JsonRequestBody,
                    true,
                    0,
                )
                .map(ResolvedRequestBody::Json),
            RequestBody::Text(value) => self
                .interpolator
                .interpolate(value, variables, ResolutionLocation::TextRequestBody)
                .map(ResolvedRequestBody::Text),
            RequestBody::FormData(values) => values
                .iter()
                .map(|(name, value)| {
                    self.resolve_value(value, variables, ResolutionLocation::FormField, false, 0)
                        .map(|value| (name.clone(), value))
                })
                .collect::<Result<IndexMap<_, _>, _>>()
                .map(ResolvedRequestBody::FormData),
            RequestBody::Binary(value) => {
                Ok(ResolvedRequestBody::Binary(Bytes::from(value.clone())))
            }
        }
    }

    fn resolve_body_assertion(
        &self,
        body: &BodyAssertion,
        variables: &VariableStore,
    ) -> Result<ResolvedBodyAssertion, RuntimeError> {
        match body {
            BodyAssertion::Empty => Ok(ResolvedBodyAssertion::Empty),
            BodyAssertion::Text(TextAssertion::Exact(value)) => self
                .interpolator
                .interpolate(value, variables, ResolutionLocation::ExpectedText)
                .map(ResolvedTextAssertion::Exact)
                .map(ResolvedBodyAssertion::Text),
            BodyAssertion::Text(TextAssertion::Contains(value)) => self
                .interpolator
                .interpolate(value, variables, ResolutionLocation::ExpectedText)
                .map(ResolvedTextAssertion::Contains)
                .map(ResolvedBodyAssertion::Text),
            BodyAssertion::Json(assertion) => self
                .resolve_object_assertion(assertion, variables, 0)
                .map(ResolvedBodyAssertion::Json),
        }
    }

    fn resolve_object_assertion(
        &self,
        assertion: &ObjectAssertion,
        variables: &VariableStore,
        depth: usize,
    ) -> Result<ResolvedObjectAssertion, RuntimeError> {
        self.enter_depth(depth)?;
        let fields = assertion
            .fields
            .iter()
            .map(|(name, field)| {
                self.resolve_field_assertion(field, variables, depth + 1)
                    .map(|field| (name.clone(), field))
            })
            .collect::<Result<IndexMap<_, _>, _>>()?;
        Ok(ResolvedObjectAssertion {
            mode: assertion.mode.clone(),
            fields,
        })
    }

    fn resolve_field_assertion(
        &self,
        field: &FieldAssertion,
        variables: &VariableStore,
        depth: usize,
    ) -> Result<ResolvedFieldAssertion, RuntimeError> {
        self.enter_depth(depth)?;
        let expected_value = field
            .expected_value
            .as_ref()
            .map(|value| {
                self.interpolator.resolve_json_value(
                    value,
                    variables,
                    ResolutionLocation::ExpectedJson,
                    depth,
                )
            })
            .transpose()?;
        let nested = field
            .nested
            .as_ref()
            .map(|nested| self.resolve_object_assertion(nested, variables, depth))
            .transpose()?;

        Ok(ResolvedFieldAssertion {
            field_name: field.field_name.clone(),
            expected_type: field.expected_type.clone(),
            expected_value,
            nested,
            capture: field.capture.clone(),
        })
    }

    fn resolve_value(
        &self,
        value: &Value,
        variables: &VariableStore,
        location: ResolutionLocation,
        allow_structured_placeholder: bool,
        depth: usize,
    ) -> Result<ResolvedValue, RuntimeError> {
        self.interpolator.resolve_value(
            value,
            variables,
            location,
            allow_structured_placeholder,
            depth,
        )
    }

    fn enter_depth(&self, depth: usize) -> Result<(), RuntimeError> {
        if depth > self.config().max_resolution_depth() {
            return Err(RuntimeError::NestingLimitExceeded {
                limit: self.config().max_resolution_depth(),
            });
        }
        Ok(())
    }
}
