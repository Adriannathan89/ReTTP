# Semantic Validation, AST Conversion, and `rettp check` Plan

## Objective

Implement week 5 of `TIMELINE.md`: turn a syntax-valid `SuiteAst` into a
semantically valid `rettp_domain::TestSuite`, expose stable source-spanned
semantic diagnostics, and add `rettp check` with exit code `3` for lexical,
syntax, or semantic failures.

## Dependency Direction

```text
rettp-cli -> rettp-application -> rettp-parser -> rettp-domain
```

- `rettp-domain` remains independent of parsing, filesystems, and CLI concerns.
- `rettp-parser` owns AST validation and conversion because it owns `SuiteAst`
  and source spans.
- `rettp-application` orchestrates the lex/parse/validate pipeline without
  reading files or terminating the process.
- `rettp-cli` owns filesystem access, environment/CLI variables, terminal
  rendering, and process exit codes.

## Public Semantic API

```rust
pub fn validate_and_convert(
    ast: &SuiteAst,
    context: &ValidationContext,
) -> ValidationResult;
```

`ValidationResult` contains:

- `suite: Option<TestSuite>`;
- every source-spanned `ValidationError` in deterministic order.

No partial domain suite is returned when any semantic error exists. Validation
and conversion are separate internal passes so lossy `IndexMap` conversion can
never silently replace duplicate source declarations.

## Domain Alignment

Request header values change from `IndexMap<String, InterpolatedString>` to
`IndexMap<String, Value>`. This represents literal scalar/container values and
interpolated strings uniformly. `Value` gains conversions from
`InterpolatedString`, `String`, and `&str` for ergonomic request construction.

## Semantic Rules

### Suite and blocks

- At most one core block.
- A pipeline must contain at least one test.
- Pipeline and test names must not be empty.
- Top-level source order is preserved in the domain suite.
- Core tests are analyzed first for variable availability even when the core
  block occurs later in source order.

### Tests and sections

- Each test must contain exactly one request and one expectation.
- Duplicate request or expectation declarations are errors.
- Duplicate request sections (`headers`, `query`, `body`) are errors.
- Duplicate expectation sections (`status`, `headers`, `body`) are errors.
- Duplicate request headers, response headers, query keys, object keys, and
  assertion fields are errors.
- HTTP header duplication is checked case-insensitively; original spelling is
  retained in the converted domain value.
- Request paths must not be empty.
- Request bodies are allowed only for `POST`, `PUT`, and `PATCH`.
- HTTP response status codes must be in `100..=599`.

### Assertions and values

- Comparison-only fields infer their expected type from the comparison value.
- `number` accepts integer and floating-point values.
- `integer` accepts only integer values.
- Every other explicit assertion type must match its comparison value exactly.
- A capture requires an explicit type.
- A nested assertion requires the `object` type and must use partial matching.
- A field must contain a type, comparison, or nested assertion.
- Object comparisons stored in `expected_value` retain partial-comparison
  semantics for the later assertion engine.
- `body exact { ... }` maps to an exact top-level domain object assertion.

## Variable Validation

`ValidationContext` carries validated predefined variables supplied by the
environment or CLI.

Scope rules:

1. Core tests run sequentially. Their captures become globally visible.
2. Every pipeline begins with predefined and core variables, then exposes each
   successful structural capture to later tests in that pipeline only.
3. Every standalone test begins with predefined and core variables and does
   not share its captures with other standalone tests.
4. A test cannot use a capture declared by its own expectation in its request.
5. Referencing an unavailable variable is an error.
6. Redeclaring any currently visible variable is an error.
7. Separate pipeline or standalone scopes may independently use the same new
   capture name.

Interpolation is validated recursively in request and expectation strings.
`${}`, unterminated placeholders, invalid variable names, and undefined names
are semantic errors. Because lexer decoding can change byte lengths, a string
interpolation diagnostic points to the complete string literal span rather than
an unreliable subspan.

The same interpolation grammar applies to request paths and string-valued
headers. All of the following are valid when the referenced variables are in
scope:

```rettp
request GET "/data/${id}"

headers {
    "X-Direct" = "direct-value"
    "X-Variable" = "${interpolated_string}"
    "X-Mixed" = "something ${interpolated_string}"
}
```

Literal text may appear before or after a placeholder, and one string may
contain multiple non-nested placeholders. Nested placeholders remain invalid.

## Recursion Safety

The semantic walker applies its own configurable AST depth limit, defaulting to
128 and hard-capped at 256. This protects callers that construct AST nodes
programmatically and bypass the parser's nesting limit. Once the limit is
reached, that subtree produces one diagnostic and is not recursively visited.

## Check Use Case

`rettp_application::check_source` processes one named UTF-8 source in phases:

1. lex;
2. stop and return lexical diagnostics if lexing fails;
3. parse;
4. stop and return syntax diagnostics if parsing fails;
5. validate and convert;
6. return the domain suite only when all phases succeed.

This phase boundary avoids cascaded semantic diagnostics from malformed syntax.

## CLI Contract

```text
rettp check <path> [--var NAME=VALUE ...]
```

- Environment variable names and repeated `--var` names are predefined for
  semantic validation.
- `0`: source is valid.
- `3`: lexical, syntax, or semantic diagnostics.
- `1`: file I/O failure.
- `2`: invalid CLI usage or invalid `--var` declaration.

Diagnostics use a stable CI-friendly format:

```text
path:line:column: error[semantic]: undefined variable `TOKEN`
```

## Proposed Files

Full proposals are stored in `docs/generated/semantic/` for review. No source
file is changed until the proposal is accepted and applied by the maintainer.

## Testing After Acceptance

After the implementation is applied:

- unit-test every validation error and conversion mapping;
- test direct, interpolation-only, mixed, and multiple-placeholder strings in
  request paths and header values;
- test that nested, empty, invalid, undefined, and unterminated placeholders
  produce semantic diagnostics;
- test scope isolation and source-order-independent core visibility;
- test recursion limits with parsed and programmatically built ASTs;
- test `check_source` phase short-circuiting;
- integration-test CLI exit codes and diagnostic locations;
- run workspace formatting, strict Clippy, tests, Rustdoc, and the configured
  LLVM line-coverage threshold of 90%.
