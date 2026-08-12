# Rettp

[![CI](https://github.com/Adriannathan89/rettp/actions/workflows/ci.yaml/badge.svg)](https://github.com/Adriannathan89/rettp/actions/workflows/ci.yaml)
[![Release](https://img.shields.io/github/v/release/Adriannathan89/rettp?display_name=tag)](https://github.com/Adriannathan89/rettp/releases)

Rettp is a command-line HTTP verification runner for post-deployment and
pre-production checks. A compact UTF-8 `.rttp` suite describes HTTP requests,
response assertions, sequential pipelines, typed captures, and variable
interpolation without coupling the suite to an application language or test
framework.

The current published MVP is **v0.1.0**.

## What it provides

- `rettp check` validates a suite without creating an HTTP client or sending a
  network request.
- `rettp run` validates first and executes only a valid suite.
- HTTP(S) requests with relative paths, headers, query parameters, JSON object
  bodies, timeouts, and redirect rejection.
- `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, and `OPTIONS`.
- Status, header, text, empty-body, JSON type/value, nested, partial-object,
  exact top-level object, and array assertions.
- Predefined variables, interpolation, and typed response capture.
- Core fail-fast, sequential pipeline fail-fast, standalone-test continuation,
  Ctrl+C cancellation, and deterministic results.
- Terminal output plus redacted JSON and JUnit XML reports.
- Bounded source, response, interpolation, and nesting processing.
- Strict CI, dependency audit, coverage gate, bounded fuzzing, native release
  binaries, and SHA-256 checksums.

Rettp verifies a real deployed endpoint. It is not a load-testing tool and does
not replace unit or component tests.

## Why Rettp

Operational API checks often begin as a helpful `curl` command and gradually
become a fragile collection of shell scripts. Rettp keeps the directness of an
HTTP command-line tool while making the verification contract reviewable,
repeatable, and suitable for a deployment gate.

- Keep requests, expectations, and response captures in one small `.rttp`
  file.
- Validate the entire suite before sending the first request, including
  variable scope and interpolation rules.
- Run dependent authentication or setup flows once, then reuse typed captures
  safely in later requests.
- Publish deterministic terminal, JSON, and JUnit results for local use and
  CI systems.

### Choosing the right tool

These tools overlap, but they serve different jobs. It is normal to use Rettp
alongside them rather than replace them.

| Tool | Best for | Where Rettp differs |
|---|---|---|
| [`curl`](https://curl.se/) | Ad-hoc requests, downloading, debugging a single endpoint, and shell composition. | `curl` deliberately leaves assertions, captures, result aggregation, and a test-suite format to scripts. Rettp provides those verification semantics, but is not a general transfer client. |
| [HTTPie](https://httpie.io/) | Human-friendly interactive HTTP exploration. | HTTPie optimizes request authoring and readable responses. Rettp optimizes declarative assertions and repeatable pass/fail checks in automation. |
| [Postman](https://www.postman.com/) / [Newman](https://www.npmjs.com/package/newman) | Collaborative API collections, exploratory workflows, and JavaScript-based tests. | Rettp is a small native CLI with a purpose-built DSL, no collection runtime, and explicit bounded execution behavior. It intentionally has a narrower feature set. |
| [k6](https://k6.io/) | Load, stress, and performance testing under concurrency. | Rettp verifies functional HTTP contracts sequentially. It does not generate load or report throughput for a remote service. |
| Language test frameworks | Unit, component, and application integration tests close to source code. | Rettp is language-independent and targets an already-running endpoint; it complements—not replaces—tests inside an application repository. |

Choose Rettp when you need a small checked-in suite to answer, “does this
deployed API still satisfy the contract we rely on?” Choose `curl` or HTTPie
when you need to investigate a request interactively, and choose a load tool
when capacity is the question.

## Install

Download the matching archive and `SHA256SUMS` from the
[GitHub Releases page](https://github.com/Adriannathan89/rettp/releases).

### Linux x86-64

```bash
curl -LO https://github.com/Adriannathan89/rettp/releases/download/v0.1.0/rettp-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
curl -LO https://github.com/Adriannathan89/rettp/releases/download/v0.1.0/SHA256SUMS
grep 'rettp-v0.1.0-x86_64-unknown-linux-gnu.tar.gz$' SHA256SUMS | sha256sum --check
tar -xzf rettp-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
sudo install -m 0755 rettp /usr/local/bin/rettp
rettp --version
```

Expected output:

```text
rettp 0.1.0
```

| Platform | Architecture | Asset |
|---|---:|---|
| Linux (glibc) | x86-64 | `rettp-v0.1.0-x86_64-unknown-linux-gnu.tar.gz` |
| Windows | x86-64 | `rettp-v0.1.0-x86_64-pc-windows-msvc.zip` |
| macOS | Apple Silicon | `rettp-v0.1.0-aarch64-apple-darwin.tar.gz` |

The [installation guide](release/INSTALLATION.md) covers Windows, macOS,
checksum verification, source builds, and platform caveats.

## Quick start

Create `health.rttp`:

```rettp
test "health endpoint" {
    request GET "/health"
    expect {
        status = 200
        body empty
    }
}
```

Validate it without network access:

```bash
rettp check health.rttp
```

Run it against an application:

```bash
rettp run health.rttp --base-url http://localhost:3000
```

`--base-url` is required for `run`. It must be an HTTP(S) URL; DSL request paths
are relative to it, and redirects are not followed.

## Complete suite example

This suite authenticates in `core`, captures a token, runs a dependent pipeline,
and keeps a health check independent:

```rettp
core {
    test "create session" {
        request POST "/session" {
            headers { "X-API-Key" = "${API_KEY}" }
            body { username = "verification" }
        }
        expect {
            status = 200
            body { token: string -> SESSION_TOKEN }
        }
    }
}

pipeline "item lifecycle" {
    test "create item" {
        request POST "/items" {
            headers { "Authorization" = "Bearer ${SESSION_TOKEN}" }
            query { notify = false }
            body { name = "preprod item", count = 1 }
        }
        expect {
            status = 201
            headers { "Content-Type" contains "application/json" }
            body { id: integer -> ITEM_ID, name: string = "preprod item" }
        }
    }

    test "read item" {
        request GET "/items/${ITEM_ID}" {
            headers { "Authorization" = "Bearer ${SESSION_TOKEN}" }
        }
        expect {
            status = 200
            body {
                id: integer
                name: string = "preprod item"
                active: boolean = true
            }
        }
    }
}

test "health" {
    request GET "/health"
    expect { status = 204 body empty }
}
```

Run it with a secret supplied by the environment:

```bash
API_KEY=example-value \
rettp run preprod.rttp \
  --base-url https://preprod.example.com \
  --junit-file reports/rettp.xml \
  --json-file reports/rettp.json
```

Use a CI secret store in real environments. Do not commit sensitive values in a
suite, dotenv file, or shell history.

## Commands

```text
rettp check [OPTIONS] <PATH>
rettp run [OPTIONS] --base-url <URL> <PATH>
```

| Option | Applies to | Description |
|---|---|---|
| `--base-url <URL>` | `run` | Required HTTP(S) base URL. |
| `--timeout <DURATION>` | `run` | Default timeout, such as `500ms`, `30s`, or `2m`. |
| `--env-file <FILE>` | both | UTF-8 dotenv-compatible file. |
| `--var <NAME=VALUE>` | both | Predefined variable; later occurrences win. |
| `--json-file <FILE>` | `run` | Write a redacted JSON report atomically. |
| `--junit-file <FILE>` | `run` | Write a redacted JUnit XML report atomically. |

Variable precedence is:

```text
process environment < --env-file < --var
```

For example:

```bash
rettp run suite.rttp \
  --base-url https://preprod.example.com \
  --env-file .env.preprod \
  --var RESOURCE_ID=42
```

Assignments split on the first `=`, so values may contain `=`. The full
[CLI reference](release/CLI_REFERENCE.md) documents dotenv syntax, duration
validation, input limits, and report-output behavior.

## Language overview

### Requests and values

Requests support quoted paths, headers, query parameters, and JSON object
bodies. Request bodies are permitted only for `POST`, `PUT`, and `PATCH`.

```rettp
request PATCH "/users/${USER_ID}" {
    headers { "Authorization" = "Bearer ${TOKEN}" }
    query { verbose = true }
    body {
        name = "Ada"
        score = 1.5
        tags = ["preprod", "api"]
        metadata = { source = "rettp" }
    }
}
```

Values can be strings, signed integers, finite decimal numbers, booleans,
`null`, arrays, and objects. Headers and query values must resolve to strings,
booleans, integers, or finite numbers. `null`, arrays, and objects are rejected
there.

### Response assertions

An `expect` block can contain status, header, and body assertions:

```rettp
expect {
    status = 200
    headers {
        "Content-Type": string
        "Cache-Control" contains "no-cache"
    }
    body {
        id: integer
        score: number = 1
        profile: object { display_name: string }
    }
}
```

Object assertions are partial by default: undeclared response fields are
allowed. `body exact { ... }` rejects undeclared top-level fields. Nested object
assertions remain partial. Arrays compare their length and ordered values; under
the `number` type, `1` and `1.0` compare equal.

Text and empty body forms are also supported:

```rettp
expect { body = "ready" }
expect { body contains "ready" }
expect { body empty }
```

### Variables and captures

Placeholders use `${NAME}`. Captures require a declared field type:

```rettp
expect {
    body {
        token: string -> TOKEN
        profile: object -> PROFILE
        roles: array -> ROLES
    }
}
```

Captures commit only after every assertion in their test passes:

- Core captures are visible to every later block.
- Pipeline captures are visible only to later tests in that pipeline.
- Standalone captures are discarded after their test.
- Object and array captures may be reused as complete values only in JSON
  request bodies, never in paths, headers, query values, or mixed strings.

Undefined, malformed, forward, or cross-pipeline references are semantic errors
and prevent every network request in the suite.

Read the complete [language reference](release/LANGUAGE_REFERENCE.md) for the
full grammar and assertion semantics.

## Execution behavior

Rettp always checks before it runs:

```text
check source
  -> resolve semantic variable scopes
  -> execute core first
  -> execute pipelines and standalone tests in source order
  -> render terminal and optional reports
```

- The optional core runs first even if declared later in the source.
- A core failure aborts the suite and skips remaining blocks.
- A pipeline stops at its first failed test and skips its remaining tests; later
  pipelines and standalone blocks still run.
- A failed standalone test does not stop later standalone tests.
- Ctrl+C cancels in-flight execution, writes no new reports, and exits 130.

## Exit codes

| Code | Meaning |
|---:|---|
| `0` | The suite passed, or checking found no diagnostics. |
| `1` | A standalone test or pipeline failed. |
| `2` | The core failed and the suite was aborted. |
| `3` | Lexical, syntax, or semantic validation failed. |
| `4` | CLI, source input, dotenv, or HTTP configuration was invalid. |
| `5` | An internal runner or report-output failure occurred. |
| `130` | The process was interrupted with Ctrl+C. |

## Reports and sensitive values

Use report files for CI:

```bash
rettp run suite.rttp \
  --base-url https://preprod.example.com \
  --json-file reports/rettp.json \
  --junit-file reports/rettp.xml
```

Reports are written atomically. When checking fails or the process is
interrupted, Rettp does not publish new reports and preserves existing files.

Rettp redacts value-bearing domain data in terminal, JSON, and JUnit reporting.
Reports and CI logs should still be treated as sensitive operational artifacts.

## CI/CD usage

Rettp works as a deployment gate. A CI job should install a pinned binary,
verify its checksum, then preserve reports even on failure:

```yaml
- name: Run pre-production verification
  env:
    API_TOKEN: ${{ secrets.PREPROD_API_TOKEN }}
  run: |
    rettp run tests/preprod.rttp \
      --base-url "${{ vars.PREPROD_BASE_URL }}" \
      --junit-file reports/rettp.xml \
      --json-file reports/rettp.json
```

The [CI integration guide](release/CI_INTEGRATION.md) contains complete GitHub
Actions and GitLab CI examples, including verified installation.

## Updating

Rettp v0.1.0 has no `rettp self-update` command yet. Download the newer release
asset, verify its checksum, extract it, and replace the installed binary:

```bash
tar -xzf rettp-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz
sudo install -m 0755 rettp /usr/local/bin/rettp
rettp --version
```

Each update is a new SemVer version and GitHub Release. Existing release tags
are never overwritten.

## Development

Rettp is a Rust workspace. Build it with Rust 1.96.0:

```bash
cargo build --release --locked --package rettp-cli
./target/release/rettp --version
```

The local quality gates are:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
cargo llvm-cov --workspace --all-targets --all-features --locked --fail-under-lines 90
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
cargo check --manifest-path fuzz/Cargo.toml --bins --locked
```

The release workflow runs CI, bounded parser/checker fuzzing, version
validation, native smoke-tested builds, checksum generation, and GitHub Release
publication.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development setup, required
quality gates, documentation expectations, and pull-request process.

## Benchmark

The following is a local baseline, measured on 11 August 2026
with the released `rettp 0.1.0` Linux x86-64 binary. The host was an AMD Ryzen
5 6600H (Linux 7.0.0-28-generic). CPU affinity pinned the benchmark controller,
the Rettp processes, and the loopback server to one logical CPU (CPU 0).

The workload was [`a.rttp`](a.rttp): each successful suite invocation makes
four small JSON HTTP requests—health, login, authenticated data, and
unauthenticated data. The server listened only on `127.0.0.1`, held all
responses in memory, and performed no TLS, database, disk I/O, delay, or report
file output. Every table entry contains 20 suite invocations (80 HTTP requests)
after five warm-up invocations.

| Concurrent Rettp processes | Throughput (requests/s) | Suite p50 | Suite p95 |
|---:|---:|---:|---:|
| 1 | 348.5 | 11.19 ms | 11.63 ms |
| 2 | 361.9 | 21.85 ms | 24.06 ms |
| 4 | 354.8 | 43.22 ms | 56.76 ms |
| 8 | 73.6 | 63.36 ms | 1,074.96 ms |
| 12 | 68.2 | 109.02 ms | 1,161.58 ms |

The highest observed throughput was **369.3 requests/s** with two concurrent
Rettp processes; the table records a 361.9 requests/s sample at that level.
Repeating that level produced 354.6–369.3 requests/s. More
processes did not increase capacity: under the intentionally shared one-core
budget, the single-threaded loopback server and client processes contend for
the same CPU, increasing queueing and tail latency.

These numbers are a regression baseline for this exact binary, machine, suite,
and local server—not a production-service or internet-performance guarantee.
Real capacity depends on payload size, TLS, network latency, response bodies,
assertion complexity, remote service behavior, and reporting configuration.

## Architecture

```text
.rttp source
  -> lexer and parser
  -> semantic validation and domain conversion
  -> runtime interpolation and request resolution
  -> HTTP adapter
  -> assertion engine and captures
  -> execution results
  -> terminal, JSON, and JUnit reporters
```

| Crate | Responsibility |
|---|---|
| `rettp-domain` | Transport-independent suite, request, assertion, value, variable, and result types. |
| `rettp-parser` | Source spans, lexer, parser AST, semantic validation, and domain conversion. |
| `rettp-runtime` | Immutable variables, interpolation, request resolution, and capture commit. |
| `rettp-http` | HTTP client port, URL configuration, reqwest adapter, and bounded responses. |
| `rettp-assertion` | Response comparison and bounded structured failures. |
| `rettp-application` | Checker facade and sequential execution orchestration. |
| `rettp-reporter` | Redacted terminal, JSON, and JUnit renderers. |
| `rettp-cli` | Commands, source/env loading, output publication, and interrupts. |

## Limits and non-goals for v0.1.0

- Release binaries support Linux x86-64 glibc, Windows x86-64, and macOS Apple
  Silicon only.
- The macOS and Windows binaries are not code-signed; macOS is not notarized.
- Requests run sequentially; independent tests are not parallelized.
- No retries, eventual assertions, cookies, multipart, form DSL, cleanup hooks,
  suite filtering, tags, or per-test timeout syntax.
- No automatic self-update or package-manager distribution.

## Documentation

- [Installation and integrity verification](release/INSTALLATION.md)
- [Language reference](release/LANGUAGE_REFERENCE.md)
- [CLI reference](release/CLI_REFERENCE.md)
- [CI integration](release/CI_INTEGRATION.md)
- [Pre-production guide](release/PREPROD_GUIDE.md)
- [Migration guide](release/MIGRATION_GUIDE.md)
- [Changelog](release/CHANGELOG.md)
- [Release process](release/RELEASE_PROCESS.md)
- [Architecture notes](ARCHITECURE.md)
- [Contributing guide](CONTRIBUTING.md)

## License

No license has been declared for this repository. Contact the repository owner
before redistributing, modifying, or incorporating Rettp into another project.
