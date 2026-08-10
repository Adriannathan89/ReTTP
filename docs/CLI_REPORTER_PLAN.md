# CLI and Reporter Plan

## Objective

Implement Week 10 of `TIMELINE.md`: expose the validated execution engine as
`utest run`, render deterministic human-readable terminal output, and produce
stable JSON and JUnit XML artifacts for CI/CD.

This stage does not change DSL grammar or execution semantics. Every source
execution still crosses the complete compiler boundary before an HTTP client
is constructed or called:

```text
source -> lexical check -> syntax check -> semantic check -> TestSuite
       -> ExecutionEngine -> SuiteResult -> redacted RunReport -> reporters
```

Any compiler diagnostic produces exit code `3`, no domain suite, no HTTP
request, and no execution report.

## Dependency Direction

```text
utest-cli
    -> utest-application
    -> utest-http
    -> utest-parser
    -> utest-runtime
    -> utest-reporter

utest-reporter
    -> utest-domain
    -> serde / serde_json / thiserror
```

`utest-reporter` remains independent of Clap, Tokio, reqwest, filesystem I/O,
environment variables, and process exit codes. It accepts an immutable domain
`SuiteResult`, immediately converts it into a value-redacted public report,
and renders that safe representation.

`utest-cli` owns process concerns: argument parsing, filesystem reads/writes,
environment loading, HTTP-adapter construction, Tokio runtime construction,
stdout/stderr selection, terminal capability detection, and exit codes.

## Commands

### Check

```text
utest check FILE [--env-file FILE] [--var NAME=VALUE]...
```

`check` performs compilation only. Adding `--env-file` ensures checking and
running can use identical predefined-variable names.

### Run

```text
utest run FILE \
    --base-url https://preprod.example.com \
    [--timeout 30s] \
    [--env-file FILE] \
    [--var NAME=VALUE]... \
    [--json-file FILE] \
    [--junit-file FILE]
```

Exactly one source file is accepted. `--base-url` is required by Clap, so a
missing base URL is rejected before checking, constructing a runtime, or
sending HTTP. The HTTP adapter continues to reject credentials, query data,
fragments, redirects, unsupported schemes, and paths escaping the configured
base path.

`--timeout` accepts a positive integer followed by `ms`, `s`, or `m`. It sets
the default request timeout. A timeout explicitly declared by a DSL request
continues to override this default.

## Variable Sources and Dotenv Contract

Predefined variables are applied in this exact precedence order:

```text
process environment < --env-file < repeated --var
```

Within an env file and repeated CLI assignments, the last occurrence wins
without changing deterministic insertion order. Capture declarations may not
replace any predefined name; the existing semantic validator detects that
before execution.

The env-file reader:

- accepts UTF-8 `NAME=value` entries, blank lines, comments, optional `export`,
  single-quoted values, and double-quoted values with common escapes;
- does not perform `$NAME` or `${NAME}` expansion;
- validates names through `utest_domain::VariableName`;
- never includes a parsed value in an error or `Debug` output;
- rejects malformed quoting, trailing tokens, invalid UTF-8, and files larger
  than a documented one-MiB resource limit.

CLI assignments are retained as raw strings by Clap and parsed afterward. This
prevents Clap from echoing a secret-bearing invalid `NAME=VALUE` argument in a
value-parser diagnostic. Errors identify only the assignment position and a
value-free reason.

## Mandatory Compile Boundary

Both commands construct the same `VariableStore` and
`ValidationContext`. `run` calls `check_source` before creating
`HttpClientConfig`, `ReqwestHttpClient`, or invoking `ExecutionEngine`.

This ordering guarantees:

- lexical, syntax, and semantic failures cannot trigger network access;
- undefined interpolations are compile errors;
- empty suites and empty pipelines are rejected before runtime;
- CLI/environment/capture collisions are rejected before runtime;
- no partial suite can be executed.

`ExecutionEngine::execute` remains a low-level API for already validated or
programmatically constructed suites and retains its defensive shape preflight.

## Public Redacted Report Model

The JSON contract does not serialize `SuiteResult` directly. The reporter
creates a versioned, owned `RunReport` containing:

- `schema_version` (`1` for the MVP);
- source display path;
- optional suite name;
- suite status and duration;
- test-count summary;
- source-ordered blocks and tests;
- sanitized assertion failures and execution errors.

All enum spellings use stable `snake_case`. A dedicated model prevents an
internal domain refactor or serde annotation from silently breaking CI report
consumers.

### Strict redaction

Reporter conversion is the sole boundary from an internal result to a
publishable result. It never copies raw value-bearing messages blindly.

- HTTP status and stable type/shape summaries may be retained.
- Header expected/actual data is always replaced with `<redacted>`.
- Body value expected/actual data is always replaced with `<redacted>`.
- Execution-error text is regenerated only from its stable error kind.
- Assertion messages are regenerated only from their stable failure kind.
- Variable contents, resolved URLs, request/response bodies, headers, cookies,
  tokens, passwords, and captures are never present in a `RunReport`.

Authored metadata such as source paths, suite names, test names, pipeline
names, JSON paths, header names, and variable names remains visible because it
is needed to locate failures. Users must not place secret values in identifiers
or display names.

All terminal, JSON, and JUnit renderers consume only `RunReport`; none receives
the original `SuiteResult`.

## Terminal Reporter

Terminal output is deterministic and source ordered. It includes:

- suite status, source, total duration, and aggregate counts;
- block status, name, duration, and test count;
- each test status and duration;
- assertion path, kind, sanitized message, and safe/redacted previews;
- sanitized execution-error kind and message.

When stdout is a terminal, status markers are colored:

- passed: green;
- failed and aborted: red;
- skipped: bright black/neutral gray.

When stdout is redirected, the reporter emits plain ASCII with no ANSI escape
sequences. The CLI writes execution reports to stdout and compiler,
configuration, I/O, or runner diagnostics to stderr.

## JSON Reporter

`JsonReporter` writes pretty UTF-8 JSON with a final newline. It serializes the
stable `RunReport` schema and never the internal domain result.

Example outline:

```json
{
  "schema_version": 1,
  "source": "tests/preprod.utest",
  "name": null,
  "status": "failed",
  "duration_ms": 42,
  "summary": {
    "total": 3,
    "passed": 1,
    "failed": 1,
    "skipped": 1,
    "aborted": 0
  },
  "blocks": []
}
```

## JUnit Reporter

The document uses a `<testsuites>` root. Every DSL block becomes one ordered
`<testsuite>`:

- `core` uses the suite name `core`;
- a pipeline uses `pipeline:<pipeline-name>`;
- a standalone test uses `test:<test-name>` and contains one testcase.

Each DSL test becomes one `<testcase>`:

- assertion mismatches use one `<failure type="assertion">` with sanitized
  details;
- transport/runtime/internal failures use `<error>`;
- skipped tests use `<skipped>`;
- aborted tests use `<error type="internal">`;
- passed tests have no failure child.

A suite-level error that is not represented by a test is emitted as a
synthetic `utest/suite` testcase so CI cannot report a false pass. XML text and
attributes are escaped without copying untrusted values into unchecked markup.
Durations are emitted as seconds with exactly three fractional digits.

## Output Files

`--json-file` and `--junit-file` may be supplied together. Their exact paths
must differ. For each requested artifact the CLI:

1. renders the complete report in memory;
2. creates missing parent directories;
3. writes a restrictive temporary file in the destination directory;
4. flushes and synchronizes it;
5. atomically renames it over the destination.

Each artifact is individually atomic, so a reader never observes a partially
written report. Cross-file atomicity is not promised by the filesystem. A
render or write failure returns exit code `5`, even when suite execution itself
completed, because the requested CI artifact was not produced successfully.

## Exit Codes

The final CLI contract is:

| Code | Meaning |
| --- | --- |
| `0` | help/version, successful check, or fully passed suite |
| `1` | standalone test or pipeline failed |
| `2` | core failed/aborted and aborted the suite |
| `3` | lexical, syntax, or semantic diagnostics |
| `4` | invalid CLI, configuration, source/env input, or output-path configuration |
| `5` | runtime construction, internal invariant, report rendering, stdout, or report-file failure |

Clap usage errors are remapped from Clap's default `2` to `4`. Existing source
read failures in `check` are likewise migrated from `1` to `4`. HTTP
connection, timeout, invalid request, and invalid response errors are normal
test results: they produce `1`, except inside core where they produce `2`.

An unexpected top-level `Aborted` suite without a failing core is an internal
runner failure and produces `5`.

## Memory, Safety, and Maintainability

- Every crate continues to forbid unsafe code.
- Reporters borrow the internal result only during one bounded conversion and
  then operate solely on owned sanitized data.
- No reporter clones response bodies, variable stores, or captured JSON.
- No collection reserves from a user-configurable unbounded count.
- String building uses checked formatting and existing bounded assertion data.
- Dotenv reads are size bounded before parsing.
- Atomic temporary files avoid torn CI artifacts and default to restrictive
  permissions.
- XML is escaped centrally for attributes and text.
- ANSI output is selected from terminal capability, never embedded in JSON,
  JUnit, or redirected terminal output.
- The JSON schema version and reporter-specific DTOs isolate external
  contracts from internal domain evolution.

## Proposed Files

Full proposed contents are stored in `docs/generated/cli-reporter/`:

```text
docs/generated/cli-reporter/README.md
docs/generated/cli-reporter/reporter_cargo.md
docs/generated/cli-reporter/reporter_lib.md
docs/generated/cli-reporter/reporter_model.md
docs/generated/cli-reporter/reporter_terminal.md
docs/generated/cli-reporter/reporter_json.md
docs/generated/cli-reporter/reporter_junit.md
docs/generated/cli-reporter/cli_cargo.md
docs/generated/cli-reporter/cli_main.md
docs/generated/cli-reporter/cli_args.md
docs/generated/cli-reporter/cli_env_file.md
docs/generated/cli-reporter/cli_diagnostic.md
docs/generated/cli-reporter/cli_output.md
docs/generated/cli-reporter/cli_command.md
```

Tests are intentionally not prescribed as full files at proposal time. During
each accepted implementation batch, the required independent subagent owns
review and test construction so tests exercise the actual implementation and
must reach at least 90% LLVM line coverage.

## Implementation Batches

### Batch 1 — Stable redacted report model and JSON

- replace the reporter placeholder;
- introduce the versioned safe report DTO;
- implement strict conversion from every domain result/failure/error variant;
- implement pretty JSON output;
- add subagent review and comprehensive tests;
- require at least 90% LLVM line coverage for `utest-reporter`;
- add complete English public Rustdoc;
- run strict gates, `git add .`, and commit.

### Batch 2 — Terminal and JUnit renderers

- implement plain/ANSI terminal rendering;
- implement escaped JUnit XML and failure mapping;
- add subagent review and comprehensive golden/edge tests;
- require at least 90% LLVM line coverage for changed reporter files;
- complete public Rustdoc;
- run strict gates, `git add .`, and commit.

### Batch 3 — CLI run orchestration and atomic artifacts

- restructure CLI arguments and diagnostic rendering;
- add bounded env-file parsing and precedence;
- add mandatory base URL and timeout handling;
- enforce check-before-runtime and execute through the production adapter;
- emit terminal, JSON, and JUnit reports;
- normalize the complete exit-code contract;
- atomically write requested artifact files;
- add subagent review and deterministic CLI/E2E tests with a local server;
- require at least 90% LLVM line coverage for changed crates;
- add complete English module/public API documentation;
- run all workspace gates, `git add .`, and commit.

## Final Verification

After all accepted batches:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
cargo llvm-cov --workspace --all-features --all-targets --locked \
  --fail-under-lines 90 --summary-only
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
git diff --check
```

The release workflow remains disabled/not run, as previously requested.
