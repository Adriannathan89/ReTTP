# Execution Engine Plan

## Objective

Implement Week 9 of `TIMELINE.md`: execute a semantically valid
`utest_domain::TestSuite` through the existing runtime, HTTP, and assertion
boundaries and return a complete, source-ordered `SuiteResult`.

The execution engine belongs to `utest-application` because it coordinates use
cases. It does not parse source, construct a production HTTP adapter, render a
report, read files, or terminate the process.

## Dependency Direction

```text
utest-cli (Week 10 run command)
    -> utest-application
        -> utest-domain
        -> utest-runtime
        -> utest-http (HttpClient port only)
        -> utest-assertion
```

The application crate accepts `&dyn HttpClient`, so production execution and
deterministic fake-client tests use the same object-safe interface. The engine
does not depend on `reqwest` or Tokio directly.

## Compile Boundary

Execution accepts a converted `TestSuite`, not source text. The existing
`check_source` pipeline remains the mandatory source compiler boundary:

```text
source -> lex -> parse -> semantic validation -> TestSuite -> execute
```

Any lexical, syntax, or semantic diagnostic yields no domain suite, so no HTTP
request can be sent. Variable validation recognizes environment variables,
repeated `--var` assignments, visible core captures, and earlier captures in
the same pipeline. Any interpolation outside that set remains an
`UndefinedVariable` semantic error.

An empty source suite becomes a syntax diagnostic named `EmptySuite`. Empty
pipelines are already rejected by both parser recovery and semantic validation.
Core may remain empty under the previously accepted language rule.

The executor also performs a complete read-only shape preflight before the
first request. A programmatically constructed empty suite, empty pipeline, or
suite containing multiple core blocks is returned as an aborted invalid suite.
This defensive boundary does not replace source validation.

## Source Order and Execution Order

There are two deliberately different orders:

- blocks in `SuiteResult.blocks` always preserve source declaration order;
- the unique core block executes first, regardless of its source position;
- after a successful core, all non-core blocks execute sequentially in source
  order;
- there is no parallel execution in the MVP.

This lets reporters reproduce the authored suite while preserving core as a
dependency.

## Variable Scope Ownership

The caller supplies the initial `VariableStore`, normally populated from the
environment and CLI. The executor never mutates that caller-owned value.

- Core runs against one clone of the initial store. Every successful core
  capture is committed into that store and becomes globally visible.
- Every pipeline clones the post-core store once. Successful captures are
  committed sequentially and remain visible only to later tests in that
  pipeline.
- Every standalone test receives its own clone of the post-core store. Its
  successful captures are committed defensively but discarded when the test
  ends.
- Variables are borrowed during interpolation and resolved request values are
  owned, so a variable remains reusable for every request in its scope.

## One Test Execution

One test runs in this deterministic order:

1. resolve the request;
2. resolve the expectation;
3. execute the resolved request through `HttpClient`;
4. evaluate assertions and stage captures through `CaptureEngine`;
5. atomically commit captures only after the complete assertion succeeds.

Expectation resolution occurs before network I/O so a defensive runtime
resolution error cannot send a request. HTTP `4xx` and `5xx` responses remain
normal responses and are evaluated by assertions.

## Failure and Abort Semantics

### Core

- The first failed or aborted core test stops the core immediately.
- Remaining core tests are `Skipped`.
- Every non-core test is also `Skipped` without execution.
- An ordinary core test failure gives the core status `Failed`.
- A capture invariant failure gives the affected test and core status
  `Aborted`.
- In both cases the suite status is `Aborted`.

### Pipeline

- The first failed or aborted test stops that pipeline.
- Remaining pipeline tests are `Skipped`.
- The pipeline status is `Failed`.
- The suite continues with the next source block.

### Standalone test

- A failed or aborted standalone test does not stop the suite.
- Its captures remain isolated and are discarded.

### Final suite status

- `Passed`: every executed test passed.
- `Failed`: at least one pipeline or standalone test failed or aborted, and
  core did not fail.
- `Aborted`: core failed/aborted or executor preflight rejected the suite.
- `Skipped` is never a final suite status.

## Error Classification

`ExecutionErrorKind` gains `InvalidRequest` so request construction failures do
not masquerade as response or internal failures.

| Source | Result kind |
| --- | --- |
| runtime request/expectation resolution | `VariableResolution` |
| HTTP invalid request | `InvalidRequest` |
| HTTP connection | `Connection` |
| HTTP timeout | `Timeout` |
| malformed or oversized response | `InvalidResponse` |
| assertion mismatch | structured `AssertionFailure`, no execution error |
| capture staging/commit invariant | `Internal`, test `Aborted` |
| invalid programmatic suite shape | suite `Internal`, suite `Aborted` |

Existing runtime and HTTP errors are designed to redact variable contents,
URLs, headers, and bodies. The executor stores only those sanitized display
messages and never formats resolved request or variable values.

## Result Model Corrections

The pre-existing Week 2 result model is corrected before it becomes a public
runner contract:

- add `SuiteResult` with optional suite name, status, duration, source-ordered
  block results, and an optional suite-level error;
- add aggregate duration to `CoreResult` and `PipelineResult`;
- replace the erroneous `PipelineResult.cores: Vec<CoreResult>` with
  `PipelineResult.tests: Vec<TestResult>`;
- add `ExecutionErrorKind::InvalidRequest`;
- retain `BlockResult::{Core, Pipeline, Test}` and the existing test result
  constructors.

These result types remain serializable so Week 10 reporters can consume them
without depending on the executor implementation.

## Duration

`std::time::Instant` measures monotonic wall-clock duration:

- test duration covers resolution, HTTP, assertion, capture staging, and
  capture commit;
- block duration covers that block's actual execution;
- suite duration covers preflight and all executed blocks;
- skipped results have zero duration.

Milliseconds are converted from `u128` to `u64` with saturation rather than a
truncating cast.

## Memory and Performance Policy

- All execution is sequential, bounding simultaneous response/request state to
  one test.
- Source-order result slots hold only final reports, not responses or variable
  snapshots.
- Pipeline and standalone scopes use `VariableStore::clone`; captured JSON
  uses the private shared representation implemented in Week 8.
- No collection preallocates from an untrusted configurable upper bound.
- There is no `unsafe` code or detached asynchronous work.
- Recursive request, expected JSON, assertion, response, and capture traversal
  retain the limits established in Weeks 6–8.

## Proposed Files

Full proposed contents are stored in `docs/generated/execution/`:

```text
docs/generated/execution/README.md
docs/generated/execution/domain_result.md
docs/generated/execution/parser_error.md
docs/generated/execution/parser_suite_parser.md
docs/generated/execution/application_cargo.md
docs/generated/execution/application_lib.md
docs/generated/execution/application_execution.md
```

## Implementation Batches

### Batch 1 — Result contract and compile guards

- correct and extend the domain result model;
- reject an empty source suite;
- add domain and parser tests;
- request subagent review focused on memory safety, maintainability, and
  performance;
- reach at least 90% LLVM line coverage for changed crates;
- add complete public Rustdoc;
- run gates, `git add .`, and commit.

### Batch 2 — Sequential execution engine

- add application dependencies and execution module;
- implement preflight, core-first execution, source-ordered results, pipeline
  fail-fast, standalone continuation, scope ownership, error mapping, and
  saturated durations;
- request subagent review and comprehensive fake-HTTP-client integration tests;
- reach at least 90% LLVM line coverage;
- complete public Rustdoc, run workspace gates, `git add .`, and commit.

## Verification Gates

Each batch must pass the relevant subset and the final batch must pass all:

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

The release workflow remains disabled and will not be run.
