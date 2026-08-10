# UTest 0.1.0

UTest is a command-line HTTP verification runner for post-deployment and
pre-production checks. A UTF-8 `.utest` suite describes requests, response
assertions, sequential pipelines, and typed response captures without coupling
the suite to an application language or test framework.

Version 0.1.0 is the first MVP release. It supports:

- syntax and semantic validation without network access;
- HTTP(S) requests using relative paths and a required base URL;
- `core`, `pipeline`, and standalone `test` execution;
- status, header, text, empty-body, and typed JSON assertions;
- predefined variables, interpolation, and typed response capture;
- terminal, redacted JSON, and redacted JUnit XML reports;
- bounded source, response, interpolation, and nesting processing;
- graceful cancellation with exit code 130 on Ctrl+C.

## Quick start

Download the archive for your platform from the GitHub Release, follow the
[installation guide](INSTALLATION.md), and verify the executable:

```bash
utest --version
```

From a repository checkout, validate and run the bundled example:

```bash
utest check examples/basic.utest
utest run examples/basic.utest --base-url http://localhost:3000
```

## Documentation

- [Installation and integrity verification](INSTALLATION.md)
- [Language reference](LANGUAGE_REFERENCE.md)
- [CLI reference](CLI_REFERENCE.md)
- [GitHub Actions and GitLab CI examples](CI_INTEGRATION.md)
- [Pre-production usage guide](PREPROD_GUIDE.md)
- [Migration guide](MIGRATION_GUIDE.md)
- [Release process](RELEASE_PROCESS.md)
- [Changelog](CHANGELOG.md)

## Supported release targets

| Platform | Architecture | Release asset |
|---|---:|---|
| Linux (glibc) | x86-64 | `utest-v0.1.0-x86_64-unknown-linux-gnu.tar.gz` |
| Windows | x86-64 | `utest-v0.1.0-x86_64-pc-windows-msvc.zip` |
| macOS | Apple Silicon | `utest-v0.1.0-aarch64-apple-darwin.tar.gz` |

Intel macOS, Linux ARM64, package-manager installation, retries, cookies,
multipart requests, and parallel execution are not part of version 0.1.0.
