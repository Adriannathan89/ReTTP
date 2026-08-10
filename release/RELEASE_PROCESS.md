# Release Process

This document is for repository maintainers. Releases are created by GitHub
Actions from version tags; manually uploading locally built binaries is not part
of the supported process.

## Version contract

All workspace packages must use the same version. Tag `vX.Y.Z` must exactly
match that workspace version without the leading `v`. For the first release:

```text
workspace version: 0.1.0
tag:               v0.1.0
```

A hyphenated version tag such as `v0.2.0-rc.1` produces a GitHub pre-release.

## Local preflight

Run from a clean `main` branch:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
cargo llvm-cov --workspace --all-targets --all-features --locked --fail-under-lines 90
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
cargo check --manifest-path fuzz/Cargo.toml --bins --locked
cargo build --release --locked --package utest-cli
./target/release/utest --version
./target/release/utest --help
```

Review `git diff --check`, package versions, changelog, installation commands,
release asset names, and the repository's intended license policy before
tagging.

## Automated workflow

Pushing `v*` starts this dependency chain:

```text
required CI
  -> bounded parser/checker fuzz campaigns
  -> workspace/tag version verification
  -> native Linux, Windows, and macOS builds
  -> native --version and --help smoke tests
  -> archive upload
  -> SHA256SUMS generation
  -> GitHub Release publication
```

The workflow uses Ubuntu 22.04 for wider glibc compatibility, a native x86-64
Windows runner, and an Apple Silicon macOS runner. Rust 1.96.0 is pinned for CI
and artifact builds, while fuzzing uses `nightly-2026-08-01`. Third-party
actions are pinned to immutable commits. A failure in any matrix entry prevents
publication. Temporary
build artifacts expire after one day; the files attached to the GitHub Release
remain the distribution channel.

## Publishing

```bash
git switch main
git pull --ff-only origin main
git tag -a v0.1.0 -m "UTest 0.1.0"
git push origin v0.1.0
```

After GitHub Actions succeeds, verify that the release contains three archives
and `SHA256SUMS`. Download each archive on its target platform, verify the
checksum, and run `utest --version` before announcing the release.

## Failure and rollback

Do not move or reuse a published version tag. If a workflow fails before the
GitHub Release is published, fix the problem on `main` and create a new patch
version. If a release is published with a functional or security defect, mark
it clearly and publish a corrected patch version; never silently replace release
assets under the same version.
