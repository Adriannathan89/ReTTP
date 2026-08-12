# Contributing to Rettp

Thank you for improving Rettp. Contributions that make HTTP verification more
reliable, understandable, and safe are welcome.

## Before you begin

Please open an issue or start a discussion before substantial changes to the
DSL, public APIs, execution semantics, report format, or release process. A
short design discussion prevents incompatible syntax and behavior from being
implemented twice.

Small fixes, tests, documentation improvements, and focused performance fixes
can normally be submitted directly as a pull request.

## Development setup

Rettp is a Rust workspace. Install Rust **1.96.0** and use a supported Linux,
macOS, or Windows development environment.

```bash
git clone https://github.com/Adriannathan89/rettp.git
cd rettp
cargo build --workspace --locked
cargo test --workspace --all-targets --all-features --locked
```

Run the CLI from the workspace during development:

```bash
cargo run --locked --package rettp-cli -- --help
```

The HTTP integration tests use loopback listeners. Ensure that the local
environment permits binding to `127.0.0.1`.

## Project layout

| Area | Purpose |
|---|---|
| `crates/rettp-domain` | Transport-independent domain types. |
| `crates/rettp-parser` | Source spans, lexer, parser, semantic validation, and domain conversion. |
| `crates/rettp-runtime` | Variables, interpolation, request resolution, and captures. |
| `crates/rettp-http` | HTTP port, configuration, reqwest adapter, and response bounds. |
| `crates/rettp-assertion` | Response assertion engine and failure diagnostics. |
| `crates/rettp-application` | Checking and sequential suite execution. |
| `crates/rettp-reporter` | Terminal, JSON, and JUnit reporting. |
| `crates/rettp-cli` | Command-line interface, input loading, and report publication. |
| `fuzz` | Parser and checker fuzz targets. |
| `release` | User-facing release documentation. |

## Making a change

1. Create a focused branch from `main`.
2. Keep the change scoped and preserve the existing crate boundaries.
3. Add or update tests for every behavioral change, including failure paths.
4. Add Rustdoc for new or changed public APIs. Update the README and relevant
   release documentation when user-visible behavior changes.
5. Run the required checks below before opening a pull request.

Avoid unrelated formatting changes or dependency upgrades in the same pull
request. They make behavioral review and release validation harder.

## Required checks

Run these commands from the repository root:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
cargo llvm-cov --workspace --all-targets --all-features --locked --fail-under-lines 90
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
cargo check --manifest-path fuzz/Cargo.toml --bins --locked
git diff --check
```

The coverage threshold is a repository-wide minimum. New behavior should be
covered directly; do not lower the threshold or exclude code merely to make a
report pass.

## Code and documentation standards

- Rettp forbids `unsafe` code. Preserve memory bounds, depth limits, and
  deterministic ordering when changing parsers, interpolation, HTTP handling,
  captures, assertions, or reporting.
- Treat request and response values as sensitive. Diagnostics and reports must
  remain redacted; never add raw values, credentials, tokens, or secrets to
  output, test fixtures committed to the repository, or documentation.
- Prefer clear ownership and explicit error handling over implicit global
  state. Keep public APIs small and document their contracts in English.
- Keep DSL examples valid and representative. Update
  `release/LANGUAGE_REFERENCE.md` whenever syntax or language semantics change.
- Format with `cargo fmt`; Clippy warnings are errors in CI.

## Pull requests

Use a concise title that states the user-visible outcome. In the description,
include:

- the problem and the chosen approach;
- any syntax, API, report, or compatibility impact;
- tests and checks you ran; and
- follow-up work that is intentionally out of scope.

Pull requests should be reviewable, pass CI, and avoid mixing refactors with
behavioral changes where practical. Maintainers may ask for a design note or
additional regression coverage before merging changes that affect validation,
runtime behavior, or security boundaries.

## Reporting security issues

Do not publish credentials, private endpoints, or exploitable details in a
public issue. Contact the repository owner privately through the contact method
listed on the GitHub repository and include a minimal reproduction, affected
version, impact, and any suggested mitigation. Please allow time for a fix
before public disclosure.

## License

This repository currently has no declared license. By submitting a
contribution, you confirm that you are allowed to submit the work, but do not
assume that the repository grants reuse rights beyond those stated by its
owner. Contact the maintainer if you need clarification before contributing.
