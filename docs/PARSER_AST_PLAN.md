# UTest Parser AST — Specification and Implementation Plan

## 1. Purpose

This document defines the proposed AST and recursive-descent parser for the
UTest DSL parser phase. It is based on the current `utest-domain` model, the
implemented lexer, the example suite in `a.utest`, and the grammar decisions
confirmed for this phase.

This is a review document. The proposed Rust files are stored as complete file
contents under `docs/generated/parser/`. No parser implementation is applied to
`crates/utest-parser/src` until the proposal is accepted.

## 2. Scope

The parser phase converts a lexer token stream into a syntax-preserving AST:

```text
SourceText
    -> lexer
Vec<Token> + lexer diagnostics
    -> parser
SuiteAst + parser diagnostics
```

The parser is responsible for:

- recognizing suite, block, test, request, expectation, assertion, and value syntax;
- preserving source spans on AST nodes and important values;
- collecting multiple parser diagnostics;
- recovering at deterministic grammar boundaries;
- requiring every test to contain request and expectation syntax;
- requiring every pipeline to contain at least one test;
- allowing one optional, possibly empty, `core` block;
- rejecting a second `core` block;
- accepting duplicate request and expectation declarations for later semantic validation.

The parser does not:

- convert AST nodes into `utest-domain` values;
- validate duplicate requests, expectations, or request sections;
- validate HTTP status ranges;
- validate value/type compatibility;
- resolve interpolation or captures;
- enforce variable scope;
- execute HTTP requests or test blocks.

## 3. Confirmed language decisions

### Suite blocks

- A suite contains `core`, `pipeline`, and independent `test` blocks.
- `core` is optional, may occur anywhere at the suite level, and may be empty.
- At most one `core` is permitted syntactically.
- Runtime will execute `core` as a dependency before pipelines and independent tests regardless of source order.
- A pipeline must contain at least one test.
- Every test must contain both request and expectation syntax.
- Requests must appear before expectations within a test.
- Duplicate requests and expectations remain in the AST and are rejected later by semantic validation.

### Status assertions

HTTP status uses direct comparison syntax:

```utest
status = 200
```

The parser accepts any `IntegerLiteral`. The semantic phase validates the HTTP
status range.

### Object assertions

Field assertions support:

```utest
result: object
result = { status = "ok" }
result: object = { status = "ok" }
result: object {
    status: string
}
```

These forms mean type-only, comparison-only, type plus comparison, and nested
partial assertion respectively.

Object comparison is partial: only members written in the expected object are
compared. Additional response members do not fail the assertion. Nested exact
structure syntax is intentionally unsupported.

Exact field-set validation is available only for an expectation body:

```utest
body exact {
    success: boolean = true
}
```

### Captures

A capture requires an explicit assertion type:

```utest
token: string -> TOKEN
success: boolean = true -> SUCCESS
```

Comparison-only capture is a syntax error:

```utest
token = "abc" -> TOKEN
```

### Body assertions

Supported expectation body forms are:

```utest
body { ... }
body exact { ... }
body = "OK"
body contains "success"
body empty
```

`contains` and `empty` are contextual parser keywords. Until the lexer is
changed, they arrive as identifier tokens with those values.

### Assertion types

The parser supports `string`, `boolean`, `integer`, `number`, `object`, `array`,
and `null`. `null` is contextual: after `:` it is an assertion type, while after
`=` it is a literal value. `any` is no longer a reserved assertion type; after
the lexer cleanup it is an ordinary identifier and is not accepted by the parser
where an assertion type is required.

## 4. Proposed grammar

The grammar is written in an EBNF-like notation. `*` means zero or more, `+`
means one or more, and `?` means optional.

```text
suite              = block* EOF ;
block              = core-block | pipeline-block | test-block ;

core-block         = "core" "{" test-block* "}" ;
pipeline-block     = "pipeline" STRING "{" test-block+ "}" ;
test-block         = "test" STRING "{" request+ expectation+ "}" ;

request            = "request" http-method STRING request-options? ;
http-method        = "GET" | "POST" | "PUT" | "PATCH"
                   | "DELETE" | "HEAD" | "OPTIONS" ;
request-options    = "{" request-section* "}" ;
request-section    = request-headers | request-query | request-body ;

request-headers    = "headers" "{" header-value-entry* "}" ;
header-value-entry = STRING "=" value comma? ;
request-query      = "query" "{" object-value-entry* "}" ;
request-body       = "body" object-value ;

expectation        = "expect" "{" expectation-section* "}" ;
expectation-section = status-assertion
                    | response-headers
                    | body-assertion ;
status-assertion   = "status" "=" INTEGER ;

response-headers   = "headers" "{" response-header-entry* "}" ;
response-header-entry
                   = STRING ":" "string" comma?
                   | STRING "=" STRING comma?
                   | STRING contextual-contains STRING comma? ;

body-assertion     = "body" contextual-empty
                   | "body" "=" STRING
                   | "body" contextual-contains STRING
                   | "body" "exact"? assertion-object ;

assertion-object   = "{" field-assertion* "}" ;
field-assertion    = object-key field-rule capture? comma? ;
field-rule         = ":" assertion-type
                   | "=" value
                   | ":" assertion-type "=" value
                   | ":" "object" assertion-object ;
capture            = "->" IDENTIFIER ;
assertion-type     = "string" | "boolean" | "integer"
                   | "number" | "object" | "array" | "null" ;

value              = STRING | INTEGER | NUMBER | "true" | "false" | "null"
                   | array-value | object-value ;
array-value        = "[" (value ("," value)* ","?)? "]" ;
object-value       = "{" object-value-entry* "}" ;
object-value-entry = object-key "=" value comma? ;
object-key         = IDENTIFIER | reserved-word | STRING ;

contextual-empty   = IDENTIFIER("empty") ;
contextual-contains = IDENTIFIER("contains") ;
```

Commas are optional between braced entries and permitted as trailing separators.
Arrays require commas between elements and permit a trailing comma. This rule
keeps arrays unambiguous after the lexer discards whitespace and newlines.
Non-structural alphabetic keywords are accepted as keys only in an object-key
context. This is required for canonical fields such as `status`, which the lexer
otherwise classifies as a keyword token. Structural words (`core`, `pipeline`,
`test`, `request`, `expect`, `headers`, `query`, and `body`) must be quoted when
used as data keys. Reserving those words gives malformed-input recovery an
unambiguous boundary after the lexer has discarded newlines.

## 5. AST design

### Syntax-first representation

AST types are independent of `utest-domain`. This prevents parsing from
performing semantic conversion too early and allows invalid-but-parseable input
to retain enough information for the semantic validator.

`Spanned<T>` stores a value and the precise span of the source construct that
created it. Composite nodes store a full span from their opening keyword through
their closing delimiter.

The main hierarchy is:

```text
SuiteAst
└── Vec<BlockAst>
    ├── CoreBlockAst
    │   └── Vec<TestAst>
    ├── PipelineBlockAst
    │   ├── name
    │   └── Vec<TestAst>
    └── TestAst
        ├── name
        ├── Vec<RequestAst>
        └── Vec<ExpectationAst>
```

Requests and expectations use section enums and vectors. This deliberately
preserves duplicate sections and declaration order for the semantic phase.

`FieldAssertionAst` keeps type, comparison value, nested assertion object, and
capture as separate optional properties. Parser grammar ensures that at least
one assertion operation exists and that capture has an explicit type.

## 6. Parser API

The proposed public API is:

```rust
pub fn parse(tokens: &[Token]) -> ParseResult;

pub struct ParseResult {
    pub ast: SuiteAst,
    pub errors: Vec<ParserError>,
}
```

`Parser::new(tokens).parse()` is also public for consistency with the lexer API.
The parser borrows tokens and clones only literal payloads needed by the AST or
diagnostics.

Recursive object, array, and nested assertion parsing uses a default maximum
nesting depth of 128. Advanced callers may adjust it with
`Parser::with_max_nesting_depth`, capped at a non-bypassable hard limit of 256.
Crossing the configured limit produces a diagnostic and skips the remaining
balanced container iteratively.

Lexer and parser results remain separate. Callers must not invoke semantic
validation when lexer or parser diagnostics exist.

## 7. Diagnostics

Every `ParserError` contains a `ParserErrorKind` and `SourceSpan`. Proposed
diagnostics include:

- unexpected token;
- unexpected end of input;
- duplicate core block;
- empty pipeline;
- missing request in a test;
- missing expectation in a test;
- request declared after expectation;
- capture without an explicit type;
- invalid assertion construction;
- configured nesting-depth limit exceeded.

Diagnostic messages use stable categories and human-readable expected/found
descriptions. A future terminal reporter can resolve the span through
`SourceText::location`.

## 8. Error recovery

Recovery is deterministic because lexer newlines are discarded.

- Suite synchronization: `core`, `pipeline`, `test`, or EOF.
- Block synchronization: `test`, `}`, or EOF.
- Test synchronization: `request`, `expect`, `}`, or EOF.
- Request section synchronization: `headers`, `query`, `body`, `}`, or EOF.
- Expectation synchronization: `status`, `headers`, `body`, `}`, or EOF.
- Entry synchronization: comma, `}`, or EOF.

Terminal parsers never consume a delimiter or declaration keyword belonging to
their parent. For example, the `}` in `status = }` remains available to close the
expectation. Structural data keys must be quoted so the same tokens remain
unambiguous recovery boundaries. Recovery advances through discarded input with
a non-cloning cursor bump.

Every recovery loop must either advance or return. This invariant prevents
infinite loops on malformed input. Parser accessors treat a missing EOF token as
end of input, so manually constructed token streams cannot cause an out-of-range
panic.

## 9. Proposed files

New source files:

```text
crates/utest-parser/src/ast/mod.rs
crates/utest-parser/src/ast/nodes.rs
crates/utest-parser/src/parser/mod.rs
crates/utest-parser/src/parser/error.rs
crates/utest-parser/src/parser/suite_parser.rs
crates/utest-parser/src/parser/block_parser.rs
crates/utest-parser/src/parser/test_parser.rs
crates/utest-parser/src/parser/request_parser.rs
crates/utest-parser/src/parser/expectation_parser.rs
crates/utest-parser/src/parser/value_parser.rs
```

Changed source file:

```text
crates/utest-parser/src/lib.rs
```

Reviewable full-file proposals are stored in `docs/generated/parser/`.

## 10. Implementation sequence after approval

1. Add AST nodes and crate exports.
2. Add parser state, cursor helpers, spans, errors, and synchronization helpers.
3. Parse suites and block structure.
4. Parse tests and enforce request-before-expect with both sections present.
5. Parse requests and recursive values.
6. Parse expectations, assertions, contextual keywords, and captures.
7. Format and compile the parser crate.
8. Return implementation for review before adding tests.
9. After implementation acceptance, add parser unit and golden tests.
10. Reach at least 99% coverage, then add full public API Rustdoc.

## 11. Acceptance criteria

- All accepted grammar forms produce a spanned AST.
- Core is optional, order-independent, and unique.
- Empty core parses successfully.
- Empty pipeline is a parser error.
- Every test requires request and expectation syntax in that order.
- Duplicate request/expect nodes remain available for semantic validation.
- All supported values parse recursively without losing source order.
- `any` is not accepted as an assertion type.
- `null` is accepted as both a contextual assertion type and a literal value.
- Captures without an explicit type are rejected.
- Multiple independent parser errors can be collected.
- Malformed token streams do not panic or loop indefinitely.
- Deeply nested input is bounded and cannot exhaust the parser thread stack.
- Recovery preserves parent delimiters and valid top-level blocks after an error.
- Cursor-only recovery does not clone token payloads that are immediately discarded.
- Parser code does not depend on `utest-domain`, HTTP, runtime, or environment state.
