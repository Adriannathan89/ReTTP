//! Conversion from a validated syntax tree into domain models.
//!
//! All functions in this module assume that the validator has already checked
//! the tree. Keeping conversion private ensures callers cannot bypass those
//! invariants through the public parser API.

use utest_domain::{
    BodyAssertion, Capture, CoreBlock, ExpectedType, FieldAssertion, HeaderAssertion, HttpMethod,
    HttpRequestSpec, InterpolatedString, ObjectAssertion, ObjectMatchMode, PipelineBlock,
    RequestBody, ResponseExpectation, SuiteBlock, TestCase, TestSuite, TextAssertion, Value,
    VariableName,
};

use crate::{
    AssertionTypeAst, BlockAst, BodyAssertionAst, ExpectationAst, ExpectationSectionAst,
    FieldAssertionAst, HttpMethodAst, ObjectAssertionAst, ObjectMatchModeAst, ObjectValueAst,
    RequestAst, RequestSectionAst, ResponseHeaderAssertionAst, SuiteAst, TestAst, ValueAst,
};

pub(super) fn convert(ast: &SuiteAst) -> TestSuite {
    TestSuite::new(ast.blocks.iter().map(convert_block).collect())
}

fn convert_block(block: &BlockAst) -> SuiteBlock {
    match block {
        BlockAst::Core(core) => SuiteBlock::Core(CoreBlock::new(
            core.tests.iter().map(convert_test).collect(),
        )),
        BlockAst::Pipeline(pipeline) => SuiteBlock::Pipeline(PipelineBlock::new(
            pipeline.name.value.clone(),
            pipeline.tests.iter().map(convert_test).collect(),
        )),
        BlockAst::Test(test) => SuiteBlock::Test(convert_test(test)),
    }
}

fn convert_test(test: &TestAst) -> TestCase {
    let request = test
        .requests
        .first()
        .expect("semantic validation guarantees exactly one request");
    let expectation = test
        .expectations
        .first()
        .expect("semantic validation guarantees exactly one expectation");
    TestCase::new(
        test.name.value.clone(),
        convert_request(request),
        convert_expectation(expectation),
    )
}

fn convert_request(request: &RequestAst) -> HttpRequestSpec {
    let mut converted = HttpRequestSpec::new(
        convert_method(request.method.value),
        InterpolatedString::new(request.path.value.clone()),
    );

    for section in &request.sections {
        match section {
            RequestSectionAst::Headers(headers) => {
                for entry in &headers.entries {
                    converted
                        .headers
                        .insert(entry.name.value.clone(), convert_value(&entry.value));
                }
            }
            RequestSectionAst::Query(query) => {
                for entry in &query.entries {
                    converted
                        .query
                        .insert(entry.key.value.clone(), convert_value(&entry.value));
                }
            }
            RequestSectionAst::Body(body) => {
                converted.body = Some(RequestBody::Json(Value::Object(convert_object(
                    &body.value,
                ))));
            }
        }
    }

    converted
}

fn convert_expectation(expectation: &ExpectationAst) -> ResponseExpectation {
    let mut converted = ResponseExpectation::default();

    for section in &expectation.sections {
        match section {
            ExpectationSectionAst::Status(status) => {
                converted.status = Some(
                    u16::try_from(status.expected.value)
                        .expect("validated HTTP status must fit in u16"),
                );
            }
            ExpectationSectionAst::Headers(headers) => {
                for entry in &headers.entries {
                    let (name, assertion) = match entry {
                        ResponseHeaderAssertionAst::Exists { name, .. } => {
                            (&name.value, HeaderAssertion::Exists)
                        }
                        ResponseHeaderAssertionAst::Exact { name, expected, .. } => (
                            &name.value,
                            HeaderAssertion::Exact(expected.value.clone().into()),
                        ),
                        ResponseHeaderAssertionAst::Contains { name, expected, .. } => (
                            &name.value,
                            HeaderAssertion::Contains(expected.value.clone().into()),
                        ),
                    };
                    converted.headers.insert(name.clone(), assertion);
                }
            }
            ExpectationSectionAst::Body(body) => {
                converted.body = Some(convert_body_assertion(body));
            }
        }
    }

    converted
}

fn convert_body_assertion(assertion: &BodyAssertionAst) -> BodyAssertion {
    match assertion {
        BodyAssertionAst::Empty { .. } => BodyAssertion::Empty,
        BodyAssertionAst::TextExact { expected, .. } => {
            BodyAssertion::Text(TextAssertion::Exact(expected.value.clone().into()))
        }
        BodyAssertionAst::TextContains { expected, .. } => {
            BodyAssertion::Text(TextAssertion::Contains(expected.value.clone().into()))
        }
        BodyAssertionAst::Object(object) => BodyAssertion::Json(convert_object_assertion(object)),
    }
}

fn convert_object_assertion(assertion: &ObjectAssertionAst) -> ObjectAssertion {
    let mut converted = ObjectAssertion {
        mode: match assertion.mode {
            ObjectMatchModeAst::Partial => ObjectMatchMode::Partial,
            ObjectMatchModeAst::Exact => ObjectMatchMode::Exact,
        },
        fields: Default::default(),
    };
    for field in &assertion.fields {
        let field = convert_field_assertion(field);
        converted.fields.insert(field.field_name.clone(), field);
    }
    converted
}

fn convert_field_assertion(field: &FieldAssertionAst) -> FieldAssertion {
    let expected_type = field
        .expected_type
        .as_ref()
        .map(|kind| convert_assertion_type(kind.value))
        .or_else(|| field.expected_value.as_ref().map(infer_expected_type))
        .or_else(|| field.nested.as_ref().map(|_| ExpectedType::Object))
        .expect("validated field assertions always have an expected type");

    FieldAssertion {
        field_name: field.name.value.clone(),
        expected_type,
        expected_value: field.expected_value.as_ref().map(convert_value),
        nested: field.nested.as_ref().map(convert_object_assertion),
        capture: field.capture.as_ref().map(|capture| {
            Capture::new(
                VariableName::new(capture.value.clone())
                    .expect("validated capture names must be valid variables"),
            )
        }),
    }
}

fn convert_object(object: &ObjectValueAst) -> indexmap::IndexMap<String, Value> {
    object
        .entries
        .iter()
        .map(|entry| (entry.key.value.clone(), convert_value(&entry.value)))
        .collect()
}

fn convert_value(value: &ValueAst) -> Value {
    match value {
        ValueAst::String(value) => Value::String(value.value.clone().into()),
        ValueAst::Integer(value) => Value::Integer(value.value),
        ValueAst::Number(value) => Value::Number(value.value),
        ValueAst::Boolean(value) => Value::Boolean(value.value),
        ValueAst::Null(_) => Value::Null,
        ValueAst::Array(array) => Value::Array(array.items.iter().map(convert_value).collect()),
        ValueAst::Object(object) => Value::Object(convert_object(object)),
    }
}

const fn convert_method(method: HttpMethodAst) -> HttpMethod {
    match method {
        HttpMethodAst::Get => HttpMethod::GET,
        HttpMethodAst::Post => HttpMethod::POST,
        HttpMethodAst::Put => HttpMethod::PUT,
        HttpMethodAst::Patch => HttpMethod::PATCH,
        HttpMethodAst::Delete => HttpMethod::DELETE,
        HttpMethodAst::Head => HttpMethod::HEAD,
        HttpMethodAst::Options => HttpMethod::OPTIONS,
    }
}

const fn convert_assertion_type(kind: AssertionTypeAst) -> ExpectedType {
    match kind {
        AssertionTypeAst::String => ExpectedType::String,
        AssertionTypeAst::Integer => ExpectedType::Integer,
        AssertionTypeAst::Number => ExpectedType::Number,
        AssertionTypeAst::Object => ExpectedType::Object,
        AssertionTypeAst::Array => ExpectedType::Array,
        AssertionTypeAst::Boolean => ExpectedType::Boolean,
        AssertionTypeAst::Null => ExpectedType::Null,
    }
}

const fn infer_expected_type(value: &ValueAst) -> ExpectedType {
    match value {
        ValueAst::String(_) => ExpectedType::String,
        ValueAst::Integer(_) => ExpectedType::Integer,
        ValueAst::Number(_) => ExpectedType::Number,
        ValueAst::Boolean(_) => ExpectedType::Boolean,
        ValueAst::Null(_) => ExpectedType::Null,
        ValueAst::Array(_) => ExpectedType::Array,
        ValueAst::Object(_) => ExpectedType::Object,
    }
}
