//! Syntax tree nodes produced by the UTest parser.
//!
//! The AST is intentionally syntax-oriented. Collections retain declaration
//! order and duplicates so that a later semantic-validation phase can report
//! precise errors without losing source information. Every parsed construct
//! carries a [`SourceSpan`] using half-open UTF-8 byte offsets.

use crate::SourceSpan;

/// Associates a parsed value with the source range that produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned<T> {
    /// Parsed value.
    pub value: T,
    /// Half-open UTF-8 byte range occupied by `value`.
    pub span: SourceSpan,
}

impl<T> Spanned<T> {
    /// Creates a value paired with its source span.
    #[must_use]
    pub const fn new(value: T, span: SourceSpan) -> Self {
        Self { value, span }
    }
}

/// Root node for one UTest source file.
///
/// Blocks remain in source order. In particular, a [`CoreBlockAst`] may occur
/// anywhere syntactically even though execution treats it as a dependency.
#[derive(Debug, Clone, PartialEq)]
pub struct SuiteAst {
    /// Top-level core, pipeline, and standalone test blocks.
    pub blocks: Vec<BlockAst>,
    /// Span covering the complete parsed suite.
    pub span: SourceSpan,
}

/// A top-level block in a UTest suite.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockAst {
    /// Shared setup tests declared by a `core` block.
    Core(CoreBlockAst),
    /// An ordered group of tests declared by a `pipeline` block.
    Pipeline(PipelineBlockAst),
    /// A standalone `test` block.
    Test(TestAst),
}

impl BlockAst {
    /// Returns the source span of the contained block.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Core(block) => block.span,
            Self::Pipeline(block) => block.span,
            Self::Test(block) => block.span,
        }
    }
}

/// Shared tests that must run before pipeline and standalone tests.
///
/// A core block may be empty. The AST does not enforce the suite-level rule
/// that at most one core block may be present.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreBlockAst {
    /// Tests declared in the core block, in source order.
    pub tests: Vec<TestAst>,
    /// Span covering the complete `core` block.
    pub span: SourceSpan,
}

/// A named, ordered group of tests.
///
/// Semantic validation requires at least one test, while this syntax node
/// preserves an empty list so the validator can issue that diagnostic.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineBlockAst {
    /// Pipeline name and its source span.
    pub name: Spanned<String>,
    /// Tests declared in the pipeline, in source order.
    pub tests: Vec<TestAst>,
    /// Span covering the complete `pipeline` block.
    pub span: SourceSpan,
}

/// A named HTTP test with request and expectation declarations.
///
/// Requests and expectations are stored separately in declaration order.
/// Duplicate or missing declarations remain representable for later semantic
/// validation.
#[derive(Debug, Clone, PartialEq)]
pub struct TestAst {
    /// Test name and its source span.
    pub name: Spanned<String>,
    /// Request declarations associated with the test.
    pub requests: Vec<RequestAst>,
    /// Expectation declarations associated with the test.
    pub expectations: Vec<ExpectationAst>,
    /// Span covering the complete `test` block.
    pub span: SourceSpan,
}

/// HTTP method used by a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethodAst {
    /// `GET` method.
    Get,
    /// `POST` method.
    Post,
    /// `PUT` method.
    Put,
    /// `PATCH` method.
    Patch,
    /// `DELETE` method.
    Delete,
    /// `HEAD` method.
    Head,
    /// `OPTIONS` method.
    Options,
}

/// One HTTP request declaration inside a test.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestAst {
    /// HTTP method and its source span.
    pub method: Spanned<HttpMethodAst>,
    /// Request path and its source span.
    pub path: Spanned<String>,
    /// Optional request sections in declaration order.
    pub sections: Vec<RequestSectionAst>,
    /// Span covering the complete request declaration.
    pub span: SourceSpan,
}

/// A section that contributes data to an HTTP request.
#[derive(Debug, Clone, PartialEq)]
pub enum RequestSectionAst {
    /// Request header entries.
    Headers(RequestHeadersAst),
    /// URL query entries.
    Query(RequestQueryAst),
    /// JSON object request body.
    Body(RequestBodyAst),
}

impl RequestSectionAst {
    /// Returns the source span of the contained request section.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Headers(section) => section.span,
            Self::Query(section) => section.span,
            Self::Body(section) => section.span,
        }
    }
}

/// Header entries attached to a request.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestHeadersAst {
    /// Header entries in declaration order, including duplicates.
    pub entries: Vec<HeaderValueEntryAst>,
    /// Span covering the complete `headers` section.
    pub span: SourceSpan,
}

/// A request header name and its value.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderValueEntryAst {
    /// Header name and its source span.
    pub name: Spanned<String>,
    /// Header value as parsed by the common value grammar.
    pub value: ValueAst,
    /// Span covering the complete header entry.
    pub span: SourceSpan,
}

/// Query parameters attached to a request.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestQueryAst {
    /// Query entries in declaration order, including duplicates.
    pub entries: Vec<ObjectValueEntryAst>,
    /// Span covering the complete `query` section.
    pub span: SourceSpan,
}

/// JSON object body attached to a request.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestBodyAst {
    /// Parsed JSON-like object value.
    pub value: ObjectValueAst,
    /// Span covering the `body` keyword and its object value.
    pub span: SourceSpan,
}

/// Assertions declared by one `expect` block.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectationAst {
    /// Expectation sections in declaration order, including duplicates.
    pub sections: Vec<ExpectationSectionAst>,
    /// Span covering the complete `expect` block.
    pub span: SourceSpan,
}

/// A response property asserted by an expectation block.
#[derive(Debug, Clone, PartialEq)]
pub enum ExpectationSectionAst {
    /// Expected HTTP response status.
    Status(StatusAssertionAst),
    /// Assertions about response headers.
    Headers(ResponseHeadersAst),
    /// Assertion about the response body.
    Body(BodyAssertionAst),
}

impl ExpectationSectionAst {
    /// Returns the source span of the contained expectation section.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Status(assertion) => assertion.span,
            Self::Headers(assertions) => assertions.span,
            Self::Body(assertion) => assertion.span(),
        }
    }
}

/// A collection of response-header assertions.
#[derive(Debug, Clone, PartialEq)]
pub struct ResponseHeadersAst {
    /// Header assertions in declaration order, including duplicates.
    pub entries: Vec<ResponseHeaderAssertionAst>,
    /// Span covering the complete response `headers` section.
    pub span: SourceSpan,
}

/// Expected HTTP response status code.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusAssertionAst {
    /// Expected numeric status code and its source span.
    pub expected: Spanned<i64>,
    /// Span covering the complete status assertion.
    pub span: SourceSpan,
}

/// An assertion applied to one response header.
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseHeaderAssertionAst {
    /// Requires a header to exist and satisfy the header type syntax.
    Exists {
        /// Header name and its source span.
        name: Spanned<String>,
        /// Span of the type marker used by the existence assertion.
        type_span: SourceSpan,
        /// Span covering the complete assertion.
        span: SourceSpan,
    },

    /// Requires a header value to equal the expected string.
    Exact {
        /// Header name and its source span.
        name: Spanned<String>,
        /// Expected complete header value and its source span.
        expected: Spanned<String>,
        /// Span covering the complete assertion.
        span: SourceSpan,
    },

    /// Requires a header value to contain the expected substring.
    Contains {
        /// Header name and its source span.
        name: Spanned<String>,
        /// Expected substring and its source span.
        expected: Spanned<String>,
        /// Span covering the complete assertion.
        span: SourceSpan,
    },
}

impl ResponseHeaderAssertionAst {
    /// Returns the source span of this header assertion.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Exists { span, .. } | Self::Exact { span, .. } | Self::Contains { span, .. } => {
                *span
            }
        }
    }
}

/// Assertion applied to an HTTP response body.
#[derive(Debug, Clone, PartialEq)]
pub enum BodyAssertionAst {
    /// Requires the response body to be empty.
    Empty {
        /// Span covering the complete empty-body assertion.
        span: SourceSpan,
    },

    /// Requires the response body text to equal a string.
    TextExact {
        /// Expected complete response text and its source span.
        expected: Spanned<String>,
        /// Span covering the complete assertion.
        span: SourceSpan,
    },

    /// Requires the response body text to contain a substring.
    TextContains {
        /// Expected substring and its source span.
        expected: Spanned<String>,
        /// Span covering the complete assertion.
        span: SourceSpan,
    },

    /// Applies field assertions to a JSON object response body.
    Object(ObjectAssertionAst),
}

impl BodyAssertionAst {
    /// Returns the source span of this body assertion.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Empty { span }
            | Self::TextContains { span, .. }
            | Self::TextExact { span, .. } => *span,

            Self::Object(assertion) => assertion.span,
        }
    }
}

/// Matching behavior for an object assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectMatchModeAst {
    /// Validates declared fields and permits additional actual fields.
    Partial,
    /// Validates declared fields and rejects additional actual fields.
    Exact,
}

/// Assertions applied to fields of a JSON object.
///
/// The parser uses [`ObjectMatchModeAst::Exact`] only for a top-level
/// `body exact` assertion. Nested object assertions use partial matching.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectAssertionAst {
    /// Whether unmentioned actual fields are permitted.
    pub mode: ObjectMatchModeAst,
    /// Field assertions in declaration order, including duplicates.
    pub fields: Vec<FieldAssertionAst>,
    /// Span covering the complete object assertion, including `body` and
    /// `exact` syntax when present.
    pub span: SourceSpan,
}

/// Validation and optional capture rules for one object field.
///
/// The optional components allow type-only, comparison-only, combined
/// type-and-comparison, and nested-object assertions. A capture is represented
/// independently, but semantic validation requires an explicit
/// [`expected_type`](Self::expected_type) whenever `capture` is present.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldAssertionAst {
    /// Object field name and its source span.
    pub name: Spanned<String>,
    /// Optional required runtime type.
    pub expected_type: Option<Spanned<AssertionTypeAst>>,
    /// Optional expected value used for comparison.
    ///
    /// Object comparisons are partial: fields present in the expected value
    /// must match, while additional actual fields are accepted.
    pub expected_value: Option<ValueAst>,
    /// Optional recursively nested object assertion.
    pub nested: Option<ObjectAssertionAst>,
    /// Optional capture variable name and its source span.
    pub capture: Option<Spanned<String>>,
    /// Span covering the complete field assertion.
    pub span: SourceSpan,
}

/// Runtime type accepted by a field assertion.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum AssertionTypeAst {
    /// UTF-8 string value.
    String,
    /// Signed integer value.
    Integer,
    /// Floating-point numeric value.
    Number,
    /// Object value.
    Object,
    /// Array value.
    Array,
    /// Boolean value.
    Boolean,
    /// Null value.
    Null,
}

/// JSON-like literal value represented in the UTest syntax tree.
///
/// Each variant owns its source span directly or through its contained node,
/// avoiding a second, potentially contradictory span around the enum.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueAst {
    /// String literal.
    String(Spanned<String>),
    /// Signed integer literal.
    Integer(Spanned<i64>),
    /// Floating-point number literal.
    Number(Spanned<f64>),
    /// Boolean literal.
    Boolean(Spanned<bool>),
    /// Null literal, represented only by its source span.
    Null(SourceSpan),
    /// Array literal.
    Array(ArrayValueAst),
    /// Object literal.
    Object(ObjectValueAst),
}

impl ValueAst {
    /// Returns the source span of this value.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::String(value) => value.span,
            Self::Integer(value) => value.span,
            Self::Number(value) => value.span,
            Self::Boolean(value) => value.span,
            Self::Null(span) => *span,
            Self::Array(value) => value.span,
            Self::Object(value) => value.span,
        }
    }
}

/// JSON-like array literal.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayValueAst {
    /// Array elements in source order.
    pub items: Vec<ValueAst>,
    /// Span covering the complete array, including brackets.
    pub span: SourceSpan,
}

/// JSON-like object literal.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectValueAst {
    /// Object entries in source order, including duplicate keys.
    pub entries: Vec<ObjectValueEntryAst>,
    /// Span covering the complete object, including braces.
    pub span: SourceSpan,
}

/// A key-value entry in a JSON-like object literal.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectValueEntryAst {
    /// Entry key and its source span.
    pub key: Spanned<String>,
    /// Parsed entry value.
    pub value: ValueAst,
    /// Span covering the key, separator, and value.
    pub span: SourceSpan,
}
