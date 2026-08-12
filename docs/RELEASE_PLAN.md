# Rettp 0.1.0 Release Plan

## Objective

Publish the first MVP release from `main` as tag `v0.1.0`, with native
archives for Linux x86-64, Windows x86-64, and macOS Apple Silicon.

## Release gates

The tag is published only after the following local checks pass:

1. formatting, compilation, strict Clippy, tests, Rustdoc, and LLVM coverage;
2. both parser fuzz targets compile and the release workflow remains wired to
   run the bounded fuzz campaigns before publishing;
3. every workspace package reports version `0.1.0`;
4. the Linux release binary passes `--version` and `--help` smoke tests;
5. the release workflow and user-facing documentation pass an independent
   review for safety, maintainability, performance, and operational accuracy.

CI and release builds use Rust 1.96.0. Fuzzing uses
`nightly-2026-08-01`. Third-party actions are pinned to immutable commit SHAs,
and supported tool installers run with checksum verification and no fallback.

## Deliverables

- user and maintainer documentation under `release/`;
- release binary smoke tests for every native build runner;
- a `SHA256SUMS` file attached to the GitHub Release;
- GitHub Release assets produced only after CI and fuzzing succeed.

## Publication sequence

```text
commit documentation and workflow -> push main -> tag v0.1.0 -> push tag
    -> required CI -> required fuzzing -> version verification
    -> native builds and smoke tests -> archives and checksums
    -> GitHub Release publication
```

The assignment to complete and push the release authorizes implementation of
this plan without a separate proposal round. A failed local or remote gate must
stop publication rather than being bypassed.

Tag `v0.1.0` was an unpublished candidate whose fuzz jobs stopped during tool
installation. It has no GitHub Release or downloadable assets and is not moved
or reused. Version `0.1.0` replaces the installer with a pinned Cargo install.
