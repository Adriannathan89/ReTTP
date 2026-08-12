# Migrating Native HTTP Checks to Rettp

Rettp complements unit and component tests. Migrate only the portable HTTP
verification layer that should run after deployment; keep application-internal
fixtures and white-box assertions in the native framework.

## Native test

A typical framework-specific test may construct a client, attach a token, send
a request, decode JSON, and assert individual fields:

```rust,ignore
let response = client
    .get(format!("{base_url}/users/{user_id}"))
    .bearer_auth(token)
    .send()
    .await?;
assert_eq!(response.status(), 200);
let body: User = response.json().await?;
assert_eq!(body.id, user_id);
assert!(body.active);
```

## Equivalent Rettp suite

```rettp
test "read active user" {
    request GET "/users/${USER_ID}" {
        headers { "Authorization" = "Bearer ${TOKEN}" }
    }
    expect {
        status = 200
        body {
            id: integer
            active: boolean = true
        }
    }
}
```

```bash
rettp run user.rttp \
  --base-url https://preprod.example.com \
  --var USER_ID=42 \
  --var TOKEN="$TOKEN"
```

## Migration sequence

1. Extract the request method, relative path, headers, query, and JSON body.
2. Replace environment-specific values with `${VARIABLE}` placeholders.
3. Express externally observable status, headers, and response fields as
   assertions; avoid duplicating implementation details.
4. Move prerequisite authentication or setup requests into the core.
5. Group dependent requests into pipelines and capture typed response fields.
6. Run `rettp check` before pointing the suite at any environment.
7. Compare Rettp and native test results during a trial period.
8. Remove the native post-deployment check only after the portable suite is
   stable; retain lower-level native tests.

## Semantic differences

- Rettp pipelines are sequential and fail fast.
- A core failure aborts the entire suite.
- Partial JSON objects allow undeclared response fields by default.
- Captures commit only after the complete test passes.
- Undefined variables are rejected before any network request.
- Rettp 0.1.0 has no fixture hooks, cleanup hook, retry policy, or cookie jar.
