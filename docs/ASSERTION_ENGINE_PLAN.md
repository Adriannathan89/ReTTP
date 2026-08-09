# Assertion Engine Plan

## Objective

Implement Week 7 of `TIMELINE.md`: compare a fully resolved expected response
with an actual HTTP response and return deterministic, machine-readable
assertion failures. The engine evaluates assertions only. Variable resolution,
capture storage, suite ordering, HTTP execution, and reporting remain outside
this crate.

## Dependency Direction

```text
utest-runtime (Week 8/9)
    -> utest-assertion
        -> utest-domain
        -> utest-http
```

The assertion engine is a new `utest-assertion` crate. It consumes the
backend-neutral `utest_http::HttpResponse` and uses the existing domain failure
types. It does not depend on the parser, CLI, Tokio, or `reqwest`.

## Resolved Expectation Boundary

The current `utest_domain::ResponseExpectation` may contain
`InterpolatedString` values. Passing that type directly to the assertion engine
would allow unresolved `${variable}` placeholders to be compared as literal
text by mistake.

Week 7 therefore introduces assertion-owned resolved expectation models:

- `ResolvedResponseExpectation`;
- `ResolvedHeaderAssertion`;
- `ResolvedBodyAssertion`;
- `ResolvedTextAssertion`;
- `ResolvedObjectAssertion`;
- `ResolvedFieldAssertion`.

All expected strings in these types are plain `String`, and expected JSON
values use `serde_json::Value`. Week 8 will own conversion from the unresolved
domain expectation into this resolved representation after interpolation.
Capture declarations are retained on resolved fields for that future step, but
the Week 7 engine neither extracts nor commits captures.

## Public Evaluation API

```rust
let report = AssertionEngine::default().evaluate(&expected, &actual);

if report.is_success() {
    // Every requested assertion passed.
}
```

`AssertionReport` contains failures in deterministic evaluation order and a
`truncated` flag. The engine evaluates status first, headers in declaration
order, then the response body.

The default safety limits are:

- maximum 100 stored failures;
- maximum JSON comparison depth 128;
- hard maximum configurable JSON depth 256.

Once an additional failure would exceed the failure limit, evaluation stops
and `truncated` is set. Reaching the JSON depth limit emits an `InvalidBody`
failure for the affected path and does not descend further. Recursive calls are
therefore bounded even for expectation values built programmatically.

## Assertion Semantics

### Status

- No expected status means no status assertion.
- A different status produces `StatusMismatch` at path `status`.

### Headers

Header names are matched ASCII case-insensitively.

- `Exists` succeeds when at least one value exists.
- `Exact(expected)` requires exactly one response value and exact byte-for-byte
  UTF-8 equality. Zero values, repeated values, a different value, or a
  non-UTF-8 value fails.
- `Contains(expected)` succeeds when at least one UTF-8 response value contains
  the expected substring. Additional values are allowed.

Header failures use paths such as `headers["content-type"]`. Diagnostic value
previews are bounded so a large or malicious response cannot be copied into
every failure.

### Body Classification

- `Empty` succeeds only when the raw response body has zero bytes. Whitespace
  is not empty.
- `Text` assertions accept only `ResponseBody::Text`; binary and JSON bodies do
  not silently coerce to text.
- `Json` assertions accept only `ResponseBody::Json` with an object root.
- A response-classification mismatch produces `InvalidBody`; a JSON root-type
  mismatch produces `TypeMismatch` at `$`.

### JSON Types

- `string`, `boolean`, `null`, `object`, and `array` require the corresponding
  JSON type.
- `integer` accepts signed or unsigned integral JSON numbers.
- `number` accepts integral and floating-point JSON numbers.
- A type mismatch is reported once for that field; value and nested assertions
  are skipped for that field to avoid cascading failures.

### JSON Values

- Scalar values compare exactly.
- A field declared as `number` compares integral and floating representations
  numerically, so `1` equals `1.0`.
- Cross-representation integer/float comparison is deliberately conservative
  outside the exact IEEE-754 integer range to avoid false equality caused by
  rounding.
- Arrays require the same length and order. Elements are compared recursively.
- Object values on the right side of `field: object = {...}` are recursive
  partial comparisons: every declared key is checked through the deepest leaf,
  while additional actual keys are ignored.
- Objects nested inside compared arrays retain those partial-object semantics;
  array length and order remain exact.

### Object Assertion Modes

- `body { ... }` is partial and ignores undeclared response fields.
- `body exact { ... }` rejects undeclared fields on the top-level object.
- DSL nested assertion blocks remain partial as enforced by semantic
  validation.
- The public resolved model preserves `ObjectMatchMode`, allowing trusted
  programmatic callers to express the same policy explicitly.

Missing fields produce `MissingField`, extra fields in an exact assertion
produce `UnexpectedField`, type differences produce `TypeMismatch`, and
matching types with different values produce `ValueMismatch`.

JSON body paths use `$` as their root, dot notation for identifier-like keys,
bracket notation for other keys, and numeric brackets for arrays, for example:

```text
$.user.profile[0]["display-name"]
```

## Failure Memory Policy

Failure diagnostics never embed entire objects, arrays, bodies, or unbounded
strings. Values are represented as bounded previews:

- scalar strings are truncated on a UTF-8 character boundary;
- objects are summarized by field count;
- arrays are summarized by element count;
- non-UTF-8 headers are labeled without lossy full-body allocation.

This complements the HTTP adapter's response-body byte limit and prevents a
large number of failures from multiplying retained response data.

## HTTP Model Adjustment

`ResponseHeaders::append` is currently crate-private, so a fake `HttpClient`
outside `utest-http` cannot construct a response containing headers. The method
will become public and normalize names to ASCII lowercase itself. A chainable
`with_header` builder will also be added. This keeps runtime and assertion tests
independent of a real HTTP server while preserving repeated header values.

## Proposed Files

Full proposed versions are stored in `docs/generated/assertion/`:

```text
docs/generated/assertion/README.md
docs/generated/assertion/workspace_cargo.md
docs/generated/assertion/assertion_cargo.md
docs/generated/assertion/lib.md
docs/generated/assertion/config.md
docs/generated/assertion/expectation.md
docs/generated/assertion/report.md
docs/generated/assertion/engine.md
docs/generated/assertion/http_model.md
```

`Cargo.lock` will be regenerated by Cargo during implementation and is not
duplicated as a hand-written proposal.

## Implementation Batches After Acceptance

### Batch 1 — Resolved model and configuration

- add the workspace crate and manifest;
- add resolved expectation types;
- add validated limits and assertion report;
- ask a subagent to review memory safety, maintainability, and performance and
  to add unit tests;
- reach at least 90% LLVM line coverage;
- add complete public API Rustdoc;
- run quality gates and commit the batch.

### Batch 2 — Status, headers, and text/empty body

- expose safe response-header construction;
- implement status and response-header evaluation;
- implement strict text and empty-body evaluation;
- run the required subagent review/test workflow;
- reach at least 90% LLVM line coverage;
- complete Rustdoc, run quality gates, and commit the batch.

### Batch 3 — Recursive JSON assertions

- implement bounded recursive type and value comparison;
- implement partial and exact object policies;
- implement ordered exact arrays and complete JSON paths;
- run the required subagent review/test workflow;
- reach at least 90% LLVM line coverage;
- complete Rustdoc, run all workspace gates, and commit the batch.

## Final Quality Gates

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo llvm-cov -p utest-assertion --all-targets --summary-only
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
git diff --check
```

The assertion crate must remain above the agreed 90% line-coverage threshold
and contain no `unsafe` code.
