# Variable Interpolation and Capture Plan

## Objective

Implement Week 8 of `TIMELINE.md`: resolve runtime variables into requests and
expectations, extract typed JSON field captures from successful responses, and
commit those captures atomically for later tests. This stage does not send HTTP
requests or schedule `core`, `pipeline`, and standalone tests; those remain the
Week 9 execution engine's responsibility.

## Dependency Direction

```text
rettp-cli
    -> rettp-application
    -> rettp-parser
    -> rettp-runtime

rettp-runtime
    -> rettp-assertion
    -> rettp-domain
    -> rettp-http
```

`rettp-runtime` owns values that exist only while a suite runs. It converts the
unresolved domain request and expectation models into the resolved models
already consumed by `rettp-http` and `rettp-assertion`. It has no dependency on
the parser, CLI framework, async runtime, filesystem, or HTTP backend.

## Runtime Variables

`VariableStore` is an insertion-ordered, cloneable store keyed by validated
`VariableName` values. A variable contains either UTF-8 text supplied by the
environment or CLI, or a typed `serde_json::Value` captured from a response.
Captured JSON retains its actual representation, including signed and unsigned
integers, arrays, objects, booleans, strings, and null.

All values are treated as sensitive. `Debug` output, interpolation errors, and
capture errors expose variable names and type names only; they never include
the original or resolved value.

### Environment and CLI precedence

The CLI builds the store in this order:

1. load Unicode environment entries whose names satisfy `VariableName`;
2. apply each repeated `--var NAME=VALUE` argument from left to right;
3. the last CLI assignment wins, including over an environment value.

The supported syntax is deliberately one assignment per flag:

```bash
rettp check suite.rttp --var name1=10 --var name2=4
```

A comma-separated assignment list is not supported. Values may be empty and
may contain `=` because parsing splits only at the first `=`. Environment names
or values that cannot be represented as UTF-8 are skipped.

The existing semantic validator receives all store names as predefined
variables. It therefore rejects any capture that attempts to reuse an
environment or CLI name before execution begins. Runtime capture commit also
checks this invariant defensively and is atomic: if any pending name collides,
none of the pending captures are inserted.

## Scope Ownership

Week 8 provides a cloneable store and atomic capture transaction; Week 9 owns
the scope lifecycle:

- core uses the suite-global store and commits successful captures into it;
- every pipeline clones the post-core store and commits only to that clone;
- every standalone test clones the post-core store and discards local captures
  after the test;
- a failed test receives no pending capture transaction.

This matches the scope rules already enforced statically by
`rettp-parser::ValidationContext` without coupling runtime code to the parser.

## Interpolation Grammar

Runtime interpolation recognizes the same non-nested `${VARIABLE}` grammar as
semantic validation. Literal text may surround multiple placeholders. Runtime
resolution remains defensive for programmatically constructed domain models
and reports empty, unterminated, invalid, or missing placeholders without
panicking.

Scalar values render as follows:

| Stored value | Interpolated text |
| --- | --- |
| string | unchanged UTF-8 text |
| signed/unsigned integer | canonical decimal |
| number | canonical finite JSON number |
| boolean | `true` or `false` |
| null | `null` |

An interpolated output has a configurable one-MiB default limit and a hard
sixteen-MiB maximum. Length arithmetic is checked before allocation and output
is appended incrementally. Errors never contain partial resolved output.

## Structured Capture Substitution

Captured objects and arrays can only be substituted into a JSON request body
or a JSON expected value. A string node consisting of exactly one placeholder
preserves the captured structure:

```rettp
body {
    payload = "${CAPTURED_OBJECT}"
}
```

Given `CAPTURED_OBJECT = {"id": 10}`, the wire JSON is:

```json
{"payload":{"id":10}}
```

It is not an escaped JSON string. A structured value in mixed text, such as
`"prefix ${CAPTURED_OBJECT}"`, is rejected because it has no unambiguous JSON
type. Objects and arrays are also rejected in paths, headers, query parameters,
text bodies, form fields, expected headers, and expected text. Scalar
placeholders always produce strings when the DSL node itself is a string, even
inside a JSON body.

Literal domain objects and arrays remain valid JSON values. The restriction
above applies specifically to using captured structured values through string
interpolation outside JSON-value positions.

## Request and Expectation Resolution

`RuntimeResolver` converts:

```text
HttpRequestSpec + VariableStore -> ResolvedHttpRequest
ResponseExpectation + VariableStore -> ResolvedResponseExpectation
```

It resolves paths, headers, query values, request bodies, response headers,
text expectations, and recursively nested JSON expected values. Declaration
order is retained. Request timeout milliseconds become `Duration` without
changing the HTTP adapter's validation policy.

Resolution stops at the first error in deterministic declaration order.
Recursive domain values and assertions use a default depth limit of 128 and a
hard configurable maximum of 256, protecting callers that bypass parser and
semantic limits.

Captured unsigned JSON integers require an additive
`ResolvedValue::UnsignedInteger(u64)` variant. The HTTP adapter will encode it
without converting through `f64`, preserving values above `i64::MAX` exactly.

## Capture Transaction

`CaptureEngine` evaluates a resolved expectation with the existing
`AssertionEngine`. It extracts captures only when the entire assertion report
succeeds. A failed report contains no `PendingCaptures`, so callers cannot
accidentally commit values from a failed test through the normal API.

Capture extraction:

- supports only JSON object fields, as required by the DSL grammar;
- traverses nested field assertions in declaration order;
- stores the actual response value, not the expected value;
- preserves the actual JSON type and number representation;
- stages parent-field captures before nested captures;
- never mutates the variable store during evaluation.

`VariableStore::commit` preflights every pending name and inserts all values
only when no collision exists. A semantic invariant violation therefore cannot
leave a partially modified scope.

Captured storage uses private shared ownership to avoid response-sized memory
amplification. Each minimal topmost captured subtree is cloned once into an
immutable `Arc`; descendant captures retain only that shared root and a short
relative field path. Independent branches do not retain unrelated response
fields, and cloning a variable scope does not deep-clone captured JSON.

## Error Model and Redaction

`RuntimeError` distinguishes:

- malformed or undefined interpolation;
- unsupported object/array interpolation context;
- output and recursion limit violations;
- invalid non-finite numbers;
- capture body/field invariant failures;
- duplicate capture commit.

Errors may include a variable name, JSON path, context, expected type, or actual
type. They never include environment values, CLI values, captured JSON,
resolved URLs, headers, bodies, or partially interpolated strings.

## Proposed Files

Complete proposed file contents are stored in `docs/generated/runtime/`:

```text
docs/generated/runtime/README.md
docs/generated/runtime/runtime_cargo.md
docs/generated/runtime/lib.md
docs/generated/runtime/config.md
docs/generated/runtime/error.md
docs/generated/runtime/variable.md
docs/generated/runtime/interpolation.md
docs/generated/runtime/resolver.md
docs/generated/runtime/capture.md
docs/generated/runtime/http_model.md
docs/generated/runtime/http_reqwest_client.md
docs/generated/runtime/cli_cargo.md
docs/generated/runtime/cli_main.md
```

`Cargo.lock` and test files are intentionally omitted from the proposal. Cargo
regenerates the lockfile, while the required independent subagent creates tests
after each accepted implementation batch.

## Implementation Batches After Acceptance

### Batch 1 — Variable store and bounded interpolation

- add runtime dependencies, configuration, error types, variable assignments,
  environment loading, CLI precedence, and bounded scalar interpolation;
- update the CLI to preserve `--var` values while continuing to run only
  source checking;
- ask a subagent to review memory safety, maintainability, and performance and
  add unit/integration tests;
- reach at least 90% LLVM line coverage;
- complete public API Rustdoc, run quality gates, `git add .`, and commit.

### Batch 2 — Request and expectation resolution

- resolve every supported request and expectation location;
- implement typed object/array substitution in JSON-value positions;
- add exact unsigned integer support to the HTTP resolved model and adapter;
- run the required subagent review/test workflow;
- reach at least 90% LLVM line coverage;
- complete Rustdoc, run quality gates, `git add .`, and commit.

### Batch 3 — Transactional capture

- combine assertion evaluation with success-only capture staging;
- recursively extract typed captures and atomically commit them;
- verify scope cloning primitives needed by Week 9;
- run the required subagent review/test workflow;
- reach at least 90% LLVM line coverage;
- complete Rustdoc, run all workspace gates, `git add .`, and commit.

## Final Quality Gates

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo llvm-cov --workspace --all-features --all-targets --locked \
    --fail-under-lines 90 --summary-only
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
git diff --check
```

The release workflow will not be run. Every implementation batch must be
reviewed and tested by a subagent as required by `.agents/agents.md`.
