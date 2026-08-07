use crate::source::SourceSpan;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: SourceSpan,
}

impl Token {
    #[must_use]
    pub const fn new(kind: TokenKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Structure
    Core,
    Pipeline,
    Test,

    // HTTP Definition
    Request,
    Expect,
    Body,
    Headers,
    Query,
    Status,
    Exact,

    // HTTP methods
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,

    // Assertion Types
    TypeString,
    TypeBoolean,
    TypeInteger,
    TypeNumber,
    TypeObject,
    TypeArray,
    TypeAny,

    // Literal / Special Value
    True,
    False,
    Null,

    // Literal
    Identifier(String),
    StringLiteral(String),
    IntegerLiteral(i64),
    NumberLiteral(f64),

    // Symbols
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Colon,
    Equal,
    Arrow,

    // Optional separator
    Comma,

    Eof,
}

#[must_use]
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
        "any" => TokenKind::TypeAny,

        // literal / special value
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "null" => TokenKind::Null,

        // anything else is an identifier
        _ => TokenKind::Identifier(value.to_owned()),
    }
}
