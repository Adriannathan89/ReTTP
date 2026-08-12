# HTTP Adapter Plan

## Objective

Implement week 6 of `TIMELINE.md`: provide an asynchronous, reusable HTTP
client port and a `reqwest` adapter that turns a fully resolved HTTP request
into a backend-neutral HTTP response. The adapter performs network I/O only;
variable interpolation and suite execution remain runtime responsibilities.

## Dependency Direction

```text
rettp-runtime -> rettp-http -> rettp-domain
                         \-> reqwest
```

- `rettp-domain` continues to describe unresolved suite input without an HTTP
  implementation dependency.
- `rettp-http` owns the transport port, resolved wire models, configuration,
  response model, transport errors, and the `reqwest` adapter.
- `rettp-runtime` will resolve every `${variable}` before constructing a
  `ResolvedHttpRequest` in the following timeline stage.
- The port does not depend on the parser, application, CLI, reporter, or
  runtime crates.

## Public API

The crate exposes:

```rust
#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn execute(
        &self,
        request: &ResolvedHttpRequest,
    ) -> Result<HttpResponse, HttpError>;
}
```

`ReqwestHttpClient` implements this port with one reusable `reqwest::Client`.
The trait is object-safe so a future runtime can own `Arc<dyn HttpClient>` and
replace the network adapter with a deterministic test double.

The proposal targets `reqwest` 0.13 with default features disabled, the Rustls
backend selected explicitly, and automatic system-proxy discovery disabled.
This avoids accidental native-TLS and environment-proxy dependencies while
retaining HTTP/2 support.

## Resolved Request Contract

`ResolvedHttpRequest` contains only data ready for wire encoding:

- `HttpMethod` from `rettp-domain`;
- a non-empty relative path;
- insertion-ordered headers and query parameters;
- an optional resolved body;
- an optional non-zero per-request timeout.

`ResolvedValue` deliberately stores `String`, not `InterpolatedString`. This
type-state boundary prevents the HTTP adapter from accidentally sending an
unresolved `${variable}` because resolving domain values is a runtime concern.

Supported wire conversions are intentionally strict:

| Location | String | Integer | finite Number | Boolean | Null | Array/Object |
|---|---:|---:|---:|---:|---:|---:|
| Header | yes | yes | yes | yes | no | no |
| Query | yes | yes | yes | yes | `"null"` | no |
| Form field | yes | yes | yes | yes | `"null"` | no |
| JSON body | yes | yes | yes | yes | yes | yes |

Non-finite floating-point values are invalid in every wire location. Form data
uses `application/x-www-form-urlencoded`; JSON, text, form, and binary bodies
receive a default `Content-Type` only when the request did not provide one.

## Base URL Resolution

The adapter accepts exactly one validated base URL:

- only `http` and `https` schemes are accepted;
- credentials, query strings, and fragments are rejected;
- a missing trailing slash is normalized;
- request paths must be relative to that base URL;
- absolute URLs, network-path references (`//host`), fragments, and empty
  paths are rejected;
- redirects are disabled and cannot be enabled by request input.

Leading slashes are removed before joining, so both `/users` and `users`
resolve beneath the configured base path. For example, base URL
`https://service.test/api/` and path `/users` resolve to
`https://service.test/api/users`. A path cannot replace the configured origin.
Query entries are appended after path resolution.

## Timeout and Response Limits

`HttpClientConfig` defaults to:

- 30 seconds per request;
- 10 MiB maximum response body size;
- redirects disabled unconditionally.

A request timeout overrides the default. Zero-duration timeouts and zero-byte
body limits are rejected during configuration/request validation.

The response reader checks `Content-Length` when available and also enforces
the limit while streaming chunks. The streaming check is authoritative because
servers may omit or misreport `Content-Length`. Capacity and length arithmetic
use checked/saturating bounds so untrusted response metadata cannot overflow an
allocation.

## Response Contract

`HttpResponse` preserves:

- numeric status, including normal `4xx` and `5xx` responses;
- response headers as case-insensitive names with all repeated values retained;
- raw response body bytes;
- a decoded classification of `Empty`, `Json`, `Text`, or `Binary`.

Classification rules are deterministic:

1. zero body bytes produce `Empty`;
2. `application/json` and any `application/*+json` content type must decode as
   JSON, otherwise the result is `InvalidResponse`;
3. `text/*` must decode as UTF-8 text, otherwise the result is
   `InvalidResponse`;
4. every other or missing content type remains `Binary`.

The JSON and text variants retain the same immutable `Bytes` allocation as the
raw body accessor. JSON parsing allocates only the decoded value tree; text
decoding reuses the body allocation when possible.

Response header values are stored as bytes because HTTP permits values that
are not valid UTF-8. Assertion code can request UTF-8 explicitly rather than
making the transport reject an otherwise valid response.

## Error Contract

`HttpError` has stable categories:

- `InvalidBaseUrl` for unsupported or malformed adapter configuration;
- `InvalidRequest` for invalid path, header, query, body, or timeout data;
- `Connection` for DNS, TCP, TLS, and other transport failures;
- `Timeout` for request or response-body timeout;
- `InvalidResponse` for malformed headers, declared JSON, or declared text;
- `BodyTooLarge` when the configured response limit is exceeded.

Reqwest error URLs are removed before producing messages so query credentials
cannot be leaked through diagnostics. HTTP error statuses do not become
`HttpError`; they are returned for expectation evaluation.

## Security and Memory Safety

- No `unsafe` code is required.
- Redirects are rejected at the client policy level.
- Base URLs cannot carry user-info credentials.
- Request header names and values use `http` validation, preventing CR/LF
  injection.
- Response bodies are bounded before and during collection.
- Response bytes use reference-counted immutable storage to avoid unnecessary
  copies across assertion and capture stages.
- Recursive resolved values are owned and acyclic. Runtime depth validation
  remains responsible for bounding programmatically constructed JSON trees
  before they reach serialization.
- One client and its connection pool are reused across requests.

## Proposed Files

Full proposals are stored in `docs/generated/http/`:

```text
crates/rettp-http/Cargo.toml
crates/rettp-http/src/lib.rs
crates/rettp-http/src/client.rs
crates/rettp-http/src/config.rs
crates/rettp-http/src/error.rs
crates/rettp-http/src/model.rs
crates/rettp-http/src/reqwest_client.rs
```

No Rust source or manifest is changed until this plan and every proposed file
have been reviewed and accepted.

## Implementation Workflow After Acceptance

Changes will be applied in dependency order and validated as related batches:

1. error and resolved/response models;
2. client configuration and port;
3. reqwest adapter and crate exports;
4. local-server integration tests delegated to a test-review subagent;
5. public English Rustdoc completion after the coverage gate passes.

Each batch will run formatting, strict Clippy, tests, Rustdoc warnings, and LLVM
coverage. The workspace CI threshold is 90%, while new HTTP adapter logic will
target at least 99% line coverage. Per `.agents/AGENT.md`, Git staging and
committing remain with the maintainer.

## Required Test Matrix After Acceptance

- all supported HTTP methods and normal `2xx`, `4xx`, and `5xx` responses;
- base URL normalization and rejection of absolute/cross-origin paths;
- scalar headers, query parameters, and form fields;
- rejection of null/complex headers, complex query/form values, invalid
  header syntax, and non-finite numbers;
- JSON, text, form, binary, and empty request bodies;
- default and per-request timeouts;
- connection refusal versus timeout classification;
- repeated and non-UTF-8 response headers;
- JSON, `+json`, text, binary, missing-content-type, and empty responses;
- malformed declared JSON/text without panic;
- declared and streamed over-limit response bodies;
- proof that redirects are not followed;
- a mock implementation used through `dyn HttpClient`.
