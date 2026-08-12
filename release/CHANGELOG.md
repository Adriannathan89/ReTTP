# Changelog

## 0.1.0

Initial MVP release.

The earlier `v0.1.0` tag was an unpublished release candidate that stopped
before fuzz execution because its tool installer rejected `cargo-fuzz`. No
binary or GitHub Release was published for that tag.

### Added

- UTF-8 Rettp DSL lexer, parser, source spans, error recovery, and semantic
  validation.
- Domain model for suites, requests, expectations, variables, captures, and
  execution results.
- HTTP adapter with HTTP(S) base URL resolution, seven request methods, bounded
  responses, timeouts, content classification, and redirect rejection.
- Status, header, text, empty-body, partial/exact object, nested object, typed,
  value, and numeric assertions.
- Immutable reusable interpolation values and atomic typed captures with core,
  pipeline, and standalone scopes.
- Sequential execution engine with core abort and pipeline fail-fast semantics.
- `check` and `run` commands, dotenv and CLI variables, terminal output, JSON
  reports, JUnit XML reports, stable exit codes, and Ctrl+C cancellation.
- Bounded input handling, regression corpus, parser fuzz targets, strict CI, and
  multi-platform GitHub Release automation.

### Release assets

- Linux x86-64 GNU archive.
- Windows x86-64 MSVC ZIP archive.
- macOS Apple Silicon archive.
- SHA-256 checksum manifest.

### Known limitations

- Tests and pipelines execute sequentially.
- Per-test retries, cookies, multipart/form requests, and per-test timeout
  syntax are not supported.
- macOS and Windows binaries are not code-signed.
- Intel macOS and Linux ARM64 release binaries are not provided.
