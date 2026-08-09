//! Semantic rule checking for parsed suites.

use std::collections::HashSet;

use utest_domain::VariableName;

use crate::{
    AssertionTypeAst, BlockAst, BodyAssertionAst, ExpectationAst, ExpectationSectionAst,
    FieldAssertionAst, HttpMethodAst, ObjectAssertionAst, ObjectMatchModeAst, ObjectValueAst,
    RequestAst, RequestSectionAst, SourceSpan, Spanned, SuiteAst, TestAst, ValueAst,
};

use super::{
    DuplicateKind, ValidationContext, ValidationError, ValidationErrorKind,
    interpolation::{InterpolationError, variable_names},
};

pub(super) fn validate(ast: &SuiteAst, context: &ValidationContext) -> Vec<ValidationError> {
    Validator {
        errors: Vec::new(),
        max_depth: context.max_depth(),
    }
    .validate_suite(ast, context)
}

struct Validator {
    errors: Vec<ValidationError>,
    max_depth: usize,
}

impl Validator {
    fn validate_suite(
        mut self,
        ast: &SuiteAst,
        context: &ValidationContext,
    ) -> Vec<ValidationError> {
        let mut first_core = None;
        for (index, block) in ast.blocks.iter().enumerate() {
            if let BlockAst::Core(core) = block {
                if first_core.is_some() {
                    self.duplicate(DuplicateKind::CoreBlock, None, core.span);
                } else {
                    first_core = Some(index);
                }
            }
        }

        let global = context.predefined_variables().clone();
        let mut global_with_core = global.clone();
        if let Some(index) = first_core
            && let BlockAst::Core(core) = &ast.blocks[index]
        {
            for test in &core.tests {
                self.validate_test(test, &mut global_with_core);
            }
        }

        for (index, block) in ast.blocks.iter().enumerate() {
            match block {
                BlockAst::Core(core) if Some(index) != first_core => {
                    let mut isolated = global.clone();
                    for test in &core.tests {
                        self.validate_test(test, &mut isolated);
                    }
                }
                BlockAst::Core(_) => {}
                BlockAst::Pipeline(pipeline) => {
                    if pipeline.name.value.is_empty() {
                        self.error(ValidationErrorKind::EmptyPipelineName, pipeline.name.span);
                    }
                    if pipeline.tests.is_empty() {
                        self.error(ValidationErrorKind::EmptyPipeline, pipeline.span);
                    }
                    let mut pipeline_scope = global_with_core.clone();
                    for test in &pipeline.tests {
                        self.validate_test(test, &mut pipeline_scope);
                    }
                }
                BlockAst::Test(test) => {
                    let mut standalone_scope = global_with_core.clone();
                    self.validate_test(test, &mut standalone_scope);
                }
            }
        }

        self.errors
    }

    fn validate_test(&mut self, test: &TestAst, scope: &mut HashSet<VariableName>) {
        if test.name.value.is_empty() {
            self.error(ValidationErrorKind::EmptyTestName, test.name.span);
        }
        if test.requests.len() != 1 {
            self.error(ValidationErrorKind::InvalidRequestCount, test.span);
        }
        for request in test.requests.iter().skip(1) {
            self.duplicate(DuplicateKind::Request, None, request.span);
        }
        if test.expectations.len() != 1 {
            self.error(ValidationErrorKind::InvalidExpectationCount, test.span);
        }
        for expectation in test.expectations.iter().skip(1) {
            self.duplicate(DuplicateKind::Expectation, None, expectation.span);
        }

        for request in &test.requests {
            self.validate_request(request, scope);
        }

        let mut captures = HashSet::new();
        for expectation in &test.expectations {
            self.validate_expectation(expectation, scope, &mut captures);
        }
        scope.extend(captures);
    }

    fn validate_request(&mut self, request: &RequestAst, scope: &HashSet<VariableName>) {
        if request.path.value.is_empty() {
            self.error(ValidationErrorKind::EmptyRequestPath, request.path.span);
        }
        self.validate_interpolation(&request.path, scope);

        let mut saw_headers = false;
        let mut saw_query = false;
        let mut saw_body = false;
        let mut header_names = HashSet::new();
        let mut query_names = HashSet::new();

        for section in &request.sections {
            match section {
                RequestSectionAst::Headers(headers) => {
                    if saw_headers {
                        self.duplicate(DuplicateKind::RequestHeaders, None, headers.span);
                    }
                    saw_headers = true;
                    for entry in &headers.entries {
                        self.validate_header_name(
                            &entry.name,
                            DuplicateKind::RequestHeader,
                            &mut header_names,
                        );
                        self.validate_value(&entry.value, scope, 0);
                    }
                }
                RequestSectionAst::Query(query) => {
                    if saw_query {
                        self.duplicate(DuplicateKind::RequestQuery, None, query.span);
                    }
                    saw_query = true;
                    self.validate_object_entries(
                        &query.entries,
                        scope,
                        0,
                        DuplicateKind::QueryParameter,
                        &mut query_names,
                    );
                }
                RequestSectionAst::Body(body) => {
                    if saw_body {
                        self.duplicate(DuplicateKind::RequestBody, None, body.span);
                    }
                    saw_body = true;
                    if !method_allows_body(request.method.value) {
                        self.error(
                            ValidationErrorKind::RequestBodyNotAllowed {
                                method: method_name(request.method.value),
                            },
                            body.span,
                        );
                    }
                    self.validate_object(&body.value, scope, 0);
                }
            }
        }
    }

    fn validate_expectation(
        &mut self,
        expectation: &ExpectationAst,
        scope: &HashSet<VariableName>,
        captures: &mut HashSet<VariableName>,
    ) {
        let mut saw_status = false;
        let mut saw_headers = false;
        let mut saw_body = false;
        let mut header_names = HashSet::new();

        for section in &expectation.sections {
            match section {
                ExpectationSectionAst::Status(status) => {
                    if saw_status {
                        self.duplicate(DuplicateKind::ResponseStatus, None, status.span);
                    }
                    saw_status = true;
                    if !(100..=599).contains(&status.expected.value) {
                        self.error(
                            ValidationErrorKind::InvalidHttpStatus {
                                status: status.expected.value,
                            },
                            status.expected.span,
                        );
                    }
                }
                ExpectationSectionAst::Headers(headers) => {
                    if saw_headers {
                        self.duplicate(DuplicateKind::ResponseHeaders, None, headers.span);
                    }
                    saw_headers = true;
                    for entry in &headers.entries {
                        let (name, value) = match entry {
                            crate::ResponseHeaderAssertionAst::Exists { name, .. } => (name, None),
                            crate::ResponseHeaderAssertionAst::Exact { name, expected, .. }
                            | crate::ResponseHeaderAssertionAst::Contains {
                                name, expected, ..
                            } => (name, Some(expected)),
                        };
                        self.validate_header_name(
                            name,
                            DuplicateKind::ResponseHeader,
                            &mut header_names,
                        );
                        if let Some(value) = value {
                            self.validate_interpolation(value, scope);
                        }
                    }
                }
                ExpectationSectionAst::Body(body) => {
                    if saw_body {
                        self.duplicate(DuplicateKind::ResponseBody, None, body.span());
                    }
                    saw_body = true;
                    match body {
                        BodyAssertionAst::Empty { .. } => {}
                        BodyAssertionAst::TextExact { expected, .. }
                        | BodyAssertionAst::TextContains { expected, .. } => {
                            self.validate_interpolation(expected, scope);
                        }
                        BodyAssertionAst::Object(assertion) => {
                            self.validate_assertion(assertion, scope, captures, 0);
                        }
                    }
                }
            }
        }
    }

    fn validate_assertion(
        &mut self,
        assertion: &ObjectAssertionAst,
        scope: &HashSet<VariableName>,
        captures: &mut HashSet<VariableName>,
        depth: usize,
    ) {
        if !self.enter_depth(depth, assertion.span) {
            return;
        }
        let mut fields = HashSet::new();
        for field in &assertion.fields {
            self.validate_name(&field.name, DuplicateKind::AssertionField, &mut fields);
            self.validate_field(field, scope, captures, depth + 1);
        }
    }

    fn validate_field(
        &mut self,
        field: &FieldAssertionAst,
        scope: &HashSet<VariableName>,
        captures: &mut HashSet<VariableName>,
        depth: usize,
    ) {
        if field.expected_type.is_none() && field.expected_value.is_none() && field.nested.is_none()
        {
            self.error(
                ValidationErrorKind::MissingFieldAssertion {
                    field: field.name.value.clone(),
                },
                field.span,
            );
        }

        if let (Some(expected), Some(value)) = (&field.expected_type, &field.expected_value)
            && !type_accepts_value(expected.value, value)
        {
            self.error(
                ValidationErrorKind::TypeValueMismatch {
                    field: field.name.value.clone(),
                    expected: assertion_type_name(expected.value),
                    actual: value_type_name(value),
                },
                value.span(),
            );
        }
        if let Some(value) = &field.expected_value {
            self.validate_value(value, scope, depth);
        }

        if let Some(nested) = &field.nested {
            if !matches!(
                field.expected_type.as_ref().map(|kind| kind.value),
                Some(AssertionTypeAst::Object)
            ) {
                self.error(
                    ValidationErrorKind::NestedAssertionRequiresObject {
                        field: field.name.value.clone(),
                    },
                    nested.span,
                );
            }
            if nested.mode != ObjectMatchModeAst::Partial {
                self.error(
                    ValidationErrorKind::NestedAssertionMustBePartial {
                        field: field.name.value.clone(),
                    },
                    nested.span,
                );
            }
            self.validate_assertion(nested, scope, captures, depth);
        }

        if let Some(capture) = &field.capture {
            if field.expected_type.is_none() {
                self.error(
                    ValidationErrorKind::CaptureRequiresType {
                        name: capture.value.clone(),
                    },
                    capture.span,
                );
            }
            match VariableName::new(capture.value.clone()) {
                Ok(variable) => {
                    if scope.contains(&variable) || !captures.insert(variable) {
                        self.error(
                            ValidationErrorKind::DuplicateVariable {
                                name: capture.value.clone(),
                            },
                            capture.span,
                        );
                    }
                }
                Err(_) => self.error(
                    ValidationErrorKind::InvalidVariableName {
                        name: capture.value.clone(),
                    },
                    capture.span,
                ),
            }
        }
    }

    fn validate_object(
        &mut self,
        object: &ObjectValueAst,
        scope: &HashSet<VariableName>,
        depth: usize,
    ) {
        if !self.enter_depth(depth, object.span) {
            return;
        }
        let mut names = HashSet::new();
        self.validate_object_entries(
            &object.entries,
            scope,
            depth + 1,
            DuplicateKind::ObjectKey,
            &mut names,
        );
    }

    fn validate_object_entries(
        &mut self,
        entries: &[crate::ObjectValueEntryAst],
        scope: &HashSet<VariableName>,
        depth: usize,
        kind: DuplicateKind,
        names: &mut HashSet<String>,
    ) {
        for entry in entries {
            self.validate_name(&entry.key, kind, names);
            self.validate_value(&entry.value, scope, depth);
        }
    }

    fn validate_value(&mut self, value: &ValueAst, scope: &HashSet<VariableName>, depth: usize) {
        match value {
            ValueAst::String(value) => self.validate_interpolation(value, scope),
            ValueAst::Array(array) => {
                if !self.enter_depth(depth, array.span) {
                    return;
                }
                for item in &array.items {
                    self.validate_value(item, scope, depth + 1);
                }
            }
            ValueAst::Object(object) => self.validate_object(object, scope, depth),
            ValueAst::Integer(_)
            | ValueAst::Number(_)
            | ValueAst::Boolean(_)
            | ValueAst::Null(_) => {}
        }
    }

    fn validate_interpolation(&mut self, value: &Spanned<String>, scope: &HashSet<VariableName>) {
        let names = match variable_names(&value.value) {
            Ok(names) => names,
            Err(InterpolationError::Empty) => {
                self.error(ValidationErrorKind::EmptyInterpolation, value.span);
                return;
            }
            Err(InterpolationError::Unterminated) => {
                self.error(ValidationErrorKind::UnterminatedInterpolation, value.span);
                return;
            }
        };

        for name in names {
            match VariableName::new(name) {
                Ok(variable) if !scope.contains(&variable) => self.error(
                    ValidationErrorKind::UndefinedVariable {
                        name: name.to_owned(),
                    },
                    value.span,
                ),
                Ok(_) => {}
                Err(_) => self.error(
                    ValidationErrorKind::InvalidVariableName {
                        name: name.to_owned(),
                    },
                    value.span,
                ),
            }
        }
    }

    fn validate_name(
        &mut self,
        name: &Spanned<String>,
        kind: DuplicateKind,
        names: &mut HashSet<String>,
    ) {
        if name.value.is_empty() {
            self.error(ValidationErrorKind::EmptyName { kind }, name.span);
        }
        if !names.insert(name.value.clone()) {
            self.duplicate(kind, Some(name.value.clone()), name.span);
        }
    }

    fn validate_header_name(
        &mut self,
        name: &Spanned<String>,
        kind: DuplicateKind,
        names: &mut HashSet<String>,
    ) {
        if name.value.is_empty() {
            self.error(ValidationErrorKind::EmptyName { kind }, name.span);
        }
        if !names.insert(name.value.to_ascii_lowercase()) {
            self.duplicate(kind, Some(name.value.clone()), name.span);
        }
    }

    fn enter_depth(&mut self, depth: usize, span: SourceSpan) -> bool {
        if depth >= self.max_depth {
            self.error(
                ValidationErrorKind::NestingLimitExceeded {
                    limit: self.max_depth,
                },
                span,
            );
            return false;
        }
        true
    }

    fn duplicate(&mut self, kind: DuplicateKind, name: Option<String>, span: SourceSpan) {
        let kind = match name {
            Some(name) => ValidationErrorKind::DuplicateNamed { kind, name },
            None => ValidationErrorKind::Duplicate { kind },
        };
        self.error(kind, span);
    }

    fn error(&mut self, kind: ValidationErrorKind, span: SourceSpan) {
        self.errors.push(ValidationError::new(kind, span));
    }
}

const fn method_allows_body(method: HttpMethodAst) -> bool {
    matches!(
        method,
        HttpMethodAst::Post | HttpMethodAst::Put | HttpMethodAst::Patch
    )
}

const fn method_name(method: HttpMethodAst) -> &'static str {
    match method {
        HttpMethodAst::Get => "GET",
        HttpMethodAst::Post => "POST",
        HttpMethodAst::Put => "PUT",
        HttpMethodAst::Patch => "PATCH",
        HttpMethodAst::Delete => "DELETE",
        HttpMethodAst::Head => "HEAD",
        HttpMethodAst::Options => "OPTIONS",
    }
}

const fn assertion_type_name(kind: AssertionTypeAst) -> &'static str {
    match kind {
        AssertionTypeAst::String => "string",
        AssertionTypeAst::Integer => "integer",
        AssertionTypeAst::Number => "number",
        AssertionTypeAst::Object => "object",
        AssertionTypeAst::Array => "array",
        AssertionTypeAst::Boolean => "boolean",
        AssertionTypeAst::Null => "null",
    }
}

const fn value_type_name(value: &ValueAst) -> &'static str {
    match value {
        ValueAst::String(_) => "string",
        ValueAst::Integer(_) => "integer",
        ValueAst::Number(_) => "number",
        ValueAst::Boolean(_) => "boolean",
        ValueAst::Null(_) => "null",
        ValueAst::Array(_) => "array",
        ValueAst::Object(_) => "object",
    }
}

const fn type_accepts_value(expected: AssertionTypeAst, value: &ValueAst) -> bool {
    match expected {
        AssertionTypeAst::Number => matches!(value, ValueAst::Integer(_) | ValueAst::Number(_)),
        AssertionTypeAst::String => matches!(value, ValueAst::String(_)),
        AssertionTypeAst::Integer => matches!(value, ValueAst::Integer(_)),
        AssertionTypeAst::Object => matches!(value, ValueAst::Object(_)),
        AssertionTypeAst::Array => matches!(value, ValueAst::Array(_)),
        AssertionTypeAst::Boolean => matches!(value, ValueAst::Boolean(_)),
        AssertionTypeAst::Null => matches!(value, ValueAst::Null(_)),
    }
}
