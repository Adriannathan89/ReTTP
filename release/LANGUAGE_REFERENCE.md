# Language Reference

UTest source files are UTF-8 text. Keywords and HTTP methods are
case-sensitive. Whitespace and `//` line comments are ignored between tokens;
commas between map entries are optional.

## Suite structure

A suite contains an optional `core`, named `pipeline` blocks, and standalone
tests. A suite must contain at least one block, and every pipeline must contain
at least one test. At most one core is allowed; it may be empty and may appear
anywhere in the source.

```utest
core {
    test "authenticate" {
        request POST "/session"
        expect {
            status = 200
            body { token: string -> ACCESS_TOKEN }
        }
    }
}

pipeline "resource lifecycle" {
    test "create" {
        request POST "/items" {
            headers { "Authorization" = "Bearer ${ACCESS_TOKEN}" }
            query { notify = true }
            body { name = "sample", count = 1 }
        }
        expect {
            status = 201
            body { id: integer -> ITEM_ID }
        }
    }

    test "read" {
        request GET "/items/${ITEM_ID}"
        expect { status = 200 }
    }
}

test "health" {
    request GET "/health"
    expect { status = 204 body empty }
}
```

Each test requires exactly one `request` followed by exactly one `expect`.

## Execution order

The core always executes first, regardless of its source position. A core
failure aborts the suite and skips every remaining test. Pipelines run in source
order; a failed pipeline step skips later steps in that pipeline but does not
prevent later blocks from running. A standalone failure does not stop the
suite.

## Requests

Supported methods are `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, and
`OPTIONS`. Paths must be quoted and relative to the CLI base URL:

```utest
request PATCH "/users/${USER_ID}" {
    headers {
        "Authorization" = "Bearer ${TOKEN}"
        "X-Mode" = "direct-value"
    }
    query { verbose = true, page = 2 }
    body {
        active = true
        score = 1.5
        note = null
        tags = ["api", "preprod"]
        metadata = { source = "utest" }
    }
}
```

Request bodies are JSON objects and are allowed only for `POST`, `PUT`, and
`PATCH`. Header and query values must resolve to strings, booleans, integers, or
finite numbers; `null`, JSON object, and array values are rejected there.
Captured objects and arrays can be reused only inside a request body.

## Values

The value grammar supports strings, signed integers, finite decimal numbers,
booleans, `null`, arrays, and objects:

```utest
{
    text = "value"
    integer = -10
    number = 2.5
    enabled = true
    missing = null
    array = [1, "two", false]
    object = { nested = "value" }
}
```

## Response expectations

All expectation sections are optional, but the `expect` block itself is
required.

### Status

```utest
expect { status = 200 }
```

Valid expected status codes range from 100 through 599.

### Headers

Header names are case-insensitive. Assertions can require existence, exact
value equality, or a substring:

```utest
expect {
    headers {
        "Content-Type": string
        "X-Version" = "v1"
        "Cache-Control" contains "no-cache"
    }
}
```

An exact header assertion compares the selected header value; unrelated
response headers are not rejected.

### Text and empty bodies

```utest
expect { body = "ready" }
expect { body contains "ready" }
expect { body empty }
```

Text equality is byte-for-byte UTF-8 equality. `contains` checks a UTF-8
substring. `empty` accepts an empty response body.

### JSON object assertions

A normal object assertion is partial: undeclared fields are allowed.

```utest
expect {
    body {
        id: integer
        enabled: boolean = true
        score: number = 1
        owner: object { name: string }
    }
}
```

`body exact` rejects undeclared fields at the top-level object:

```utest
expect {
    body exact {
        id: integer
        name: string
    }
}
```

Nested `field: object { ... }` assertions remain recursive partial assertions.
For `field: object = { ... }`, every declared expected member must match, while
additional actual members are allowed. Arrays require the same length and
ordered element values. Under the `number` type, `1` and `1.0` compare equal;
under `integer`, the actual JSON number must be an integer.

Supported assertion types are `string`, `boolean`, `integer`, `number`,
`object`, `array`, and `null`. A field may combine a type and value; the value
must agree with the declared type.

## Variables and interpolation

Placeholders use `${NAME}`. Names must follow the variable identifier rules and
must be defined by the environment, an env file, `--var`, an earlier core
capture, or an earlier capture in the same pipeline.

```utest
request GET "/users/${USER_ID}" {
    headers { "Authorization" = "Bearer ${TOKEN}" }
    body { copied = "${CAPTURED_OBJECT}" }
}
```

Predefined variables are strings. A placeholder that forms the entire JSON
body field preserves a captured value's JSON type, so captured objects and
arrays are inserted as JSON rather than flattened text. A placeholder embedded
inside surrounding text uses scalar string conversion; objects and arrays are
rejected there. Resolution clones or immutably shares values, so a variable can
be used repeatedly within its scope.

Undefined, malformed, forward, or cross-pipeline references are compile-time
semantic errors and prevent all network execution.

## Captures and scope

Capture is allowed only on typed JSON response fields:

```utest
expect {
    body {
        token: string -> TOKEN
        profile: object -> PROFILE
        roles: array -> ROLES
    }
}
```

Captures are staged and committed atomically only after every assertion in the
test passes. A failed assertion or capture commits nothing.

- Core captures are visible to all later blocks.
- Pipeline captures are visible only to later tests in that pipeline.
- Standalone captures are discarded after that standalone test.
- Capture names cannot duplicate an existing predefined or in-scope variable.

Captured objects and arrays can be inserted into later JSON bodies, but cannot
be interpolated into URLs, headers, query values, or mixed strings.

## Validation and resource boundaries

The checker reports multiple lexical, syntax, and semantic diagnostics where
recovery is safe. Invalid suites never execute. Parser, JSON, assertion, and
interpolation nesting are bounded; HTTP response bodies and source inputs are
also bounded to prevent unbounded allocation.
