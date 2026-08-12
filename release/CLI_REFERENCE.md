# CLI Reference

## `rettp check`

Checks lexical, syntax, and semantic validity without creating an HTTP client
or sending a request:

```text
rettp check [OPTIONS] <PATH>
```

Options:

- `--env-file <FILE>` loads a dotenv-compatible UTF-8 variable file;
- `--var <NAME=VALUE>` defines a variable and may be repeated; later values win.

Source files are limited to 5 MiB. Environment files are limited to 1 MiB.
Diagnostics include the source path, line, and column without printing variable
values.

## `rettp run`

Checks a suite first, then executes it only when validation succeeds:

```text
rettp run [OPTIONS] --base-url <URL> <PATH>
```

Options:

- `--base-url <URL>` is required and accepts HTTP or HTTPS URLs;
- `--timeout <DURATION>` sets the default request timeout, for example `500ms`,
  `30s`, or `2m`;
- `--env-file <FILE>` loads predefined variables;
- `--var <NAME=VALUE>` supplies a predefined variable and may be repeated;
- `--json-file <FILE>` writes a stable redacted JSON report atomically;
- `--junit-file <FILE>` writes a redacted JUnit XML report atomically.

The base URL may contain a base path. Every DSL request path is relative to
that base URL. Credentials, query strings, and fragments are rejected in the
base URL, and redirects are not followed.

## Variable precedence

The lowest-to-highest precedence is:

```text
process environment < --env-file < --var
```

Repeated `--var` assignments use the last value:

```bash
rettp run suite.rttp \
  --base-url https://preprod.example.com \
  --env-file .env.preprod \
  --var API_TOKEN=first \
  --var API_TOKEN=final
```

Assignments split on the first `=`, so the value may itself contain `=`.
Capture names may not collide with predefined variables.

## Dotenv syntax

Supported entries include unquoted, single-quoted, and double-quoted values,
optional `export`, blank lines, and comments. Double-quoted values support
`\n`, `\r`, `\t`, `\\`, and `\"`. Variable expansion inside the dotenv file is
not performed.

## Exit codes

| Code | Meaning |
|---:|---|
| `0` | The suite passed, or `check` found no diagnostics. |
| `1` | A standalone test or pipeline failed. |
| `2` | The core failed and the suite was aborted. |
| `3` | Lexical, syntax, or semantic validation failed. |
| `4` | CLI input, source input, environment, or configuration was invalid. |
| `5` | An internal runner or report-output failure occurred. |
| `130` | Execution was interrupted with Ctrl+C. |

No report files are published when checking fails or execution is interrupted.
Existing report files remain intact when an atomic replacement cannot complete.
