# Pre-production Verification Guide

UTest is intended to run after an application deployment has completed and its
HTTP endpoint is reachable. It is a release gate, not a load generator.

## Recommended workflow

1. Deploy the candidate application to an isolated pre-production environment.
2. Wait for infrastructure health checks to report ready.
3. Load credentials from the CI secret store.
4. Run `utest check` when authoring or reviewing the suite.
5. Run the suite against the environment's explicit base URL.
6. Publish JUnit and JSON reports even when the suite exits nonzero.
7. Promote the deployment only when UTest exits `0`.

## Example suite

```utest
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

pipeline "read protected resource" {
    test "fetch resource" {
        request GET "/api/resources/${RESOURCE_ID}" {
            headers { "Authorization" = "Bearer ${SESSION_TOKEN}" }
        }
        expect {
            status = 200
            headers { "Content-Type" contains "application/json" }
            body { id: integer, state: string = "ready" }
        }
    }
}
```

Run it with:

```bash
utest run tests/preprod.utest \
  --base-url https://preprod.example.com \
  --var RESOURCE_ID=42 \
  --junit-file reports/utest.xml \
  --json-file reports/utest.json
```

Provide `API_KEY` through the process environment or a protected env file.

## Operational guidance

- Use a dedicated low-privilege verification account.
- Keep suites deterministic and idempotent where possible.
- Prefer a short pipeline that creates and then reads its own temporary data.
- Do not run destructive production scenarios without application-side safety
  controls.
- Use an explicit timeout appropriate for the service-level objective.
- Treat exit `2` as a dependency failure: the core did not establish the
  prerequisites required to interpret later tests.
- Treat exit `3` as a suite authoring/configuration problem, not an application
  regression.
- Preserve reports as restricted CI artifacts and apply normal retention rules.

Version 0.1.0 does not provide cleanup hooks or retries. The target application
or deployment job must clean up test data when required.
