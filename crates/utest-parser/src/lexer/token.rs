//! Token types and keyword recognition rules.

use crate::source::SourceSpan;

#[derive(Debug, Clone, PartialEq)]
/// A lexical token paired with its original source span.
pub struct Token {
    /// The token's category and decoded literal value, when applicable.
    pub kind: TokenKind,
    /// Half-open byte range occupied by this token in the source.
    pub span: SourceSpan,
}

impl Token {
    #[must_use]
    /// Creates a token from a kind and source span.
    pub const fn new(kind: TokenKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Every lexical category recognized by the UTest DSL lexer.
pub enum TokenKind {
    /// The `core` block keyword.
    Core,
    /// The `pipeline` block keyword.
    Pipeline,
    /// The `test` block keyword.
    Test,

    /// The `request` keyword.
    Request,
    /// The `expect` keyword.
    Expect,
    /// The `body` keyword.
    Body,
    /// The `headers` keyword.
    Headers,
    /// The `query` keyword.
    Query,
    /// The `status` keyword.
    Status,
    /// The `exact` keyword.
    Exact,

    /// The uppercase `GET` HTTP method.
    Get,
    /// The uppercase `POST` HTTP method.
    Post,
    /// The uppercase `PUT` HTTP method.
    Put,
    /// The uppercase `PATCH` HTTP method.
    Patch,
    /// The uppercase `DELETE` HTTP method.
    Delete,
    /// The uppercase `HEAD` HTTP method.
    Head,
    /// The uppercase `OPTIONS` HTTP method.
    Options,

    /// The `string` assertion type keyword.
    TypeString,
    /// The `boolean` assertion type keyword.
    TypeBoolean,
    /// The `integer` assertion type keyword.
    TypeInteger,
    /// The `number` assertion type keyword.
    TypeNumber,
    /// The `object` assertion type keyword.
    TypeObject,
    /// The `array` assertion type keyword.
    TypeArray,

    /// The `true` literal keyword.
    True,
    /// The `false` literal keyword.
    False,
    /// The `null` literal keyword.
    Null,

    /// An ASCII identifier that is not a reserved keyword.
    Identifier(String),
    /// A quoted string with its surrounding quotes removed and escapes decoded.
    StringLiteral(String),
    /// A signed base-10 integer represented as `i64`.
    IntegerLiteral(i64),
    /// A finite signed base-10 decimal represented as `f64`.
    NumberLiteral(f64),

    /// The `{` delimiter.
    LeftBrace,
    /// The `}` delimiter.
    RightBrace,
    /// The `[` delimiter.
    LeftBracket,
    /// The `]` delimiter.
    RightBracket,
    /// The `:` separator.
    Colon,
    /// The `=` assignment or equality separator.
    Equal,
    /// The `->` capture operator.
    Arrow,

    /// The optional `,` separator.
    Comma,

    /// The zero-length token emitted at the end of every source.
    Eof,
}

#[must_use]
/// Returns the reserved token for `value`, or an identifier token otherwise.
///
/// HTTP methods are intentionally case-sensitive: only uppercase method names
/// are recognized. For example, `GET` becomes [`TokenKind::Get`], while `get`
/// becomes [`TokenKind::Identifier`].
pub fn keyword_or_identifier(value: &str) -> TokenKind {
    match value {
        // blocks
        "core" => TokenKind::Core,
        "pipeline" => TokenKind::Pipeline,
        "test" => TokenKind::Test,

        // http definition
        "request" => TokenKind::Request,
        "expect" => TokenKind::Expect,
        "body" => TokenKind::Body,
        "headers" => TokenKind::Headers,
        "query" => TokenKind::Query,
        "status" => TokenKind::Status,
        "exact" => TokenKind::Exact,

        // http methods
        "GET" => TokenKind::Get,
        "POST" => TokenKind::Post,
        "PUT" => TokenKind::Put,
        "PATCH" => TokenKind::Patch,
        "DELETE" => TokenKind::Delete,
        "HEAD" => TokenKind::Head,
        "OPTIONS" => TokenKind::Options,

        // assertion types
        "string" => TokenKind::TypeString,
        "boolean" => TokenKind::TypeBoolean,
        "integer" => TokenKind::TypeInteger,
        "number" => TokenKind::TypeNumber,
        "object" => TokenKind::TypeObject,
        "array" => TokenKind::TypeArray,

        // literal / special value
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "null" => TokenKind::Null,

        // anything else is an identifier
        _ => TokenKind::Identifier(value.to_owned()),
    }
}
