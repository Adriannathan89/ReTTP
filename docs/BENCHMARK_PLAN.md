# One-core benchmark plan

## Goal

Record a reproducible, local end-to-end throughput baseline for the released
`rettp` v0.1.0 binary, then publish the result in the project README.

## Workload and method

- Use the repository's `a.rttp` suite. One successful invocation performs
  four HTTP requests: health, login, authenticated data, and unauthenticated
  data.
- Serve deterministic, in-memory JSON responses from a local loopback HTTP
  server. The server performs no TLS, database, disk I/O, or artificial delay.
- Pin the benchmark controller, loopback server, and every Rettp invocation to
  logical CPU 0 using Linux CPU affinity. This deliberately measures the whole
  local client/server path under a one-logical-core budget.
- Run a warm-up, then fixed-size samples at several process-concurrency levels.
  Report the best observed request throughput and per-suite latency percentiles.
- Treat the values as a regression baseline for this machine and workload, not
  as an HTTP service or internet-performance guarantee.

## Documentation change

Add a `Benchmark` section to `README.md` between `Development` and
`Architecture`. It will state the exact binary version, host/CPU, workload,
affinity, sample shape, measured results, and limitations.

## Recorded baseline

The benchmark ran on 11 August 2026 with `rettp 0.1.0` on an AMD Ryzen 5 6600H
running Linux 7.0.0-28-generic. The highest observed throughput was 369.3
requests per second at two concurrent Rettp processes. The three runs at that
level spanned 354.6–369.3 requests per second. The README records the complete
one-sample concurrency table and scope caveats.

## Safety and cleanup

The benchmark binds only `127.0.0.1`, uses dummy credentials, writes temporary
files only under `/tmp`, and removes those files after the result has been
recorded. It does not modify source code or make network requests beyond the
local loopback server.
