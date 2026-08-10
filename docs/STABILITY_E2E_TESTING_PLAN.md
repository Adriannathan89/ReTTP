# Week 11 Stability and End-to-End Testing Plan

## Status

This plan was reviewed, accepted, and implemented on 2026-08-10. The complete
pre-implementation file previews remain available under
`docs/generated/stability/` as design history.

## Objective

Week 11 hardens the complete MVP path built during Weeks 1–10:

```text
.utest source
  -> bounded UTF-8 input
  -> lex / parse / validate / convert
  -> variable resolution
  -> HTTP execution
  -> assertions and atomic captures
  -> suite result
  -> terminal / JSON / JUnit reporting
```

The work does not add new DSL syntax or execution semantics. It proves the
existing contracts across crate boundaries, adds graceful interruption, and
removes any confirmed panic or avoidable memory-amplification path found by
review.

## Accepted Decisions

1. `utest run` requires a successful checker result before configuration,
   runtime construction, or network access.
2. The E2E server binds only to an ephemeral loopback address and has no
   external network dependency.
3. Ctrl+C cancels the in-flight execution future and prevents later tests from
   being scheduled.
4. An interrupted run exits with conventional process code `130`.
5. Interruption emits one value-free diagnostic to stderr. It does not emit a
   terminal suite report and does not create or replace JSON/JUnit artifacts.
6. `utest check` is not made interrupt-aware because it is synchronous,
   bounded, and performs no network access.
7. A `.utest` source file is limited to 5 MiB. The reader retains at most the
   limit plus one sentinel byte and rejects larger input with exit code `4`
   before lexing.
8. Regression fixtures are checked in below
   `crates/utest-cli/tests/fixtures/{valid,invalid}`.
9. Parser fuzzing uses `cargo-fuzz` with separate front-end and complete-checker
   targets.
10. Fuzzing is scheduled and manually dispatchable. It is not a nondeterministic
    blocking pull-request job. Deterministic regression inputs remain part of
    normal blocking workspace tests.
11. Public safe APIs must not panic for any representable input unless a
    programmer-error precondition is explicitly documented.
12. Memory validation uses static allocation review and deterministic bounded
    stress tests. CI does not use an unstable absolute RSS threshold.
13. The existing line-coverage gate remains at a minimum of 90%.
14. The release workflow is not executed during this stage.

## Scope

### Local E2E server

A purpose-built test server will accept a finite script of raw HTTP responses.
Each response can be immediate, delayed, truncated, malformed, or closed before
a response is written. The harness records request bytes and exposes an
acceptance notification so signal tests send Ctrl+C only after a request is in
flight. Every socket and worker has a finite timeout; a failed test cannot wait
forever for a connection.

The full CLI process is exercised instead of calling crate functions directly.
This validates argument parsing, bounded file input, checking, Tokio runtime
creation, reqwest, execution semantics, reporters, artifact writes, stderr,
and process exit codes in one journey.

### Required E2E scenarios

The stable matrix covers:

- a successful core, pipeline capture, reuse, and standalone test journey;
- malformed status line or response framing;
- declared JSON containing invalid JSON;
- declared UTF-8 text containing invalid UTF-8;
- an unexpected or missing content type remaining binary;
- connection refusal distinct from request timeout;
- a delayed response that exceeds the configured timeout;
- a truncated response body;
- a response body exceeding the configured 10 MiB adapter limit;
- a captured-field type mismatch that fails its assertion, commits no capture,
  and skips the remaining pipeline steps;
- core failure aborting the suite and preventing every remaining request;
- pipeline failure skipping later pipeline steps while later blocks execute;
- strict secret absence in terminal, stderr, JSON, and JUnit output;
- oversized, invalid UTF-8, and exactly-at-limit source files;
- Ctrl+C while a request is in flight, including exit `130`, bounded shutdown,
  no subsequent request, and preservation of existing artifact files.

Cases already exhaustively established at a lower layer remain covered there;
the E2E suite uses one representative case per cross-layer contract rather
than duplicating the complete HTTP or assertion matrices.

### Bounded source input

`input::read_source` owns the filesystem boundary. It opens the file, uses
metadata only as an allocation hint and early rejection, and still enforces the
limit while streaming because metadata can race with a changing file. The
reader:

- allocates no more than 5 MiB plus one byte for source data;
- rejects an input larger than 5 MiB;
- performs UTF-8 validation only after the size check;
- never includes source bytes in its errors;
- preserves the existing path-prefixed CLI diagnostic.

The same reader is shared by `check` and `run`, so both commands have identical
input safety.

### Graceful Ctrl+C handling

The CLI races `ExecutionEngine::execute` against `tokio::signal::ctrl_c` using
`tokio::select!`. When the signal wins, the execution future is dropped. This
cancels the in-flight reqwest future and discards partial results and staged but
uncommitted captures. No partial `SuiteResult` is manufactured because it could
misrepresent tests that never completed.

Signal-handler installation failure is an internal runner error with exit code
`5`. The interruption message is constant and cannot contain request, response,
variable, or source values.

The Unix process-level regression test sends SIGINT after the local server has
accepted a request. Tokio's cross-platform `ctrl_c` API remains compiled on all
supported targets; the OS-signal integration test is Unix-only.

## Fuzzing Contract

### `parser_frontend`

Arbitrary bytes are converted lossily into valid Rust UTF-8 and passed through
the lexer and parser. The target asserts:

- every token and lexer diagnostic span is ordered and contained in source;
- every span boundary is a UTF-8 boundary;
- an EOF token is always present;
- parsing completes without panic for both clean and erroneous lexer output;
- parser diagnostic and suite spans remain contained in source.

### `checker`

The same bounded arbitrary source is passed to `check_source`. The target
asserts:

- success contains exactly one suite and no diagnostics;
- failure contains diagnostics and no partial suite;
- every diagnostic span is ordered, in bounds, and on UTF-8 boundaries;
- diagnostic display formatting and source location calculation do not panic.

Fuzzer inputs are capped at 64 KiB. This is intentionally below the CLI source
limit to maximize mutation throughput while deterministic tests cover the
5 MiB boundary.

### Fuzz workflow

`.github/workflows/fuzz.yaml` runs weekly and through `workflow_dispatch`. Each
target has a finite run count, input length, per-input timeout, and job timeout.
Crash artifacts are uploaded when a job fails. Generated artifacts, coverage,
and fuzz build output are ignored; seed corpus remains tracked.

## Panic Review

The review inventories production `unwrap`, `expect`, `panic`, `unreachable`,
indexing, recursion, integer conversion, and allocation sites. Each occurrence
is classified as:

- unreachable due to a private invariant that is locally evident;
- a documented programmer precondition;
- safely recoverable and therefore replaced with an error path; or
- reachable from untrusted source, HTTP, environment, CLI, or public model data
  and therefore a blocking defect.

No issue will be invented merely to change code. A newly confirmed production
defect that is not represented in the accepted previews will be documented and
submitted for approval before its fix is implemented.

## Memory and Performance Review

The review verifies:

- bounded source and env-file reads;
- bounded response-body accumulation;
- bounded parser, semantic, runtime, assertion, and JSON recursion;
- bounded interpolation and diagnostic previews;
- capture sharing without retaining unrelated response roots;
- atomic capture commit without partial exposure;
- failure retention caps;
- reporter conversion does not clone response bodies or variable stores;
- sequential execution does not retain completed response bodies unnecessarily;
- Ctrl+C drops pending network and execution state;
- E2E and fuzz workers have finite timeouts.

Absolute process RSS is deliberately excluded from CI because allocator and OS
differences make it flaky. Deterministic boundary tests and code-level ownership
review are the acceptance evidence.

## Implementation Batches

### Batch 1 — Bounded input, regression fixtures, and E2E harness

- add the 5 MiB bounded UTF-8 source reader;
- add exact boundary and rejection tests for `check` and `run`;
- add permanent valid and invalid fixtures;
- add the finite loopback fault server;
- add complete CLI journey tests for HTTP, capture, core, pipeline, redaction,
  output, and exit-code behavior.

After implementation, a subagent reviews memory safety, long-term
maintainability, and performance and writes/extends tests until modified code is
at least 90% line-covered. Findings are fixed, English API/module documentation
is completed, all gates run, and the batch is staged and committed.

### Batch 2 — Graceful interruption

- enable Tokio signal support;
- isolate the interrupt race in a small internal module;
- integrate interruption before report conversion or artifact output;
- add deterministic unit tests for race outcomes;
- add a Unix process E2E SIGINT regression.

The same subagent review, coverage, documentation, gate, stage, and commit cycle
is repeated.

### Batch 3 — Fuzzing and final stability review

- add both cargo-fuzz targets and seed corpus;
- add the scheduled/manual fuzz workflow;
- complete production panic and memory reviews;
- add regression tests for every confirmed issue;
- apply only fixes represented by an accepted proposal.

The final subagent independently reviews the whole Week 11 surface. After all
findings are resolved, the batch is documented, verified, staged, and committed.

## Required Gates

Every batch runs the relevant focused tests. The completed stage must pass:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
cargo llvm-cov --workspace --all-features --all-targets --locked \
  --fail-under-lines 90 --summary-only
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
cargo audit --deny warnings
git diff --check
```

The scheduled/manual workflow additionally runs both fuzz targets. The release
workflow is not triggered.

## Definition of Done

- Every required E2E scenario is deterministic and passes locally and in CI.
- Oversized and malformed user input is rejected without panic or unbounded
  allocation.
- Ctrl+C exits within a bounded test deadline with code `130`, starts no later
  request, leaks no values, and does not alter report artifacts.
- Parser/checker fuzz targets run with tracked regression corpus.
- No confirmed user-reachable production panic remains.
- The memory review finds no unbounded input-driven allocation in the MVP path,
  or any accepted residual risk is explicitly documented.
- Workspace coverage remains at least 90% lines.
- Formatting, strict Clippy, all tests, Rustdoc, dependency audit, and diff
  checks pass.

## Verification Result

- The stable workspace tests and both independent fuzz binaries compile.
- The full workspace LLVM line coverage is 98.44%, above the 90% gate.
- Formatting, workspace check, strict Clippy, Rustdoc warnings, dependency
  audits for both lockfiles, and diff checks pass.
- The scheduled fuzz workflow is intentionally separate from pull-request CI.
- The release workflow was not executed during this stage.
