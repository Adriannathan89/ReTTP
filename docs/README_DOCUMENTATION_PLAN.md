# Root README Documentation Plan

## Objective

Add an English root-level `README.md` that acts as the single project entry
point for Rettp 0.1.0. It will be useful both to a first-time user on GitHub
and to a CI/CD maintainer adopting the runner.

## Scope

The README will document:

- project purpose, MVP capabilities, and supported platforms;
- verified installation and Linux quick start;
- a complete DSL example covering core, pipeline, capture, interpolation, and
  assertions;
- command-line usage, variables, exit codes, and report files;
- execution, assertion, and variable-scope semantics;
- GitHub Actions usage and security/redaction expectations;
- update behavior, development commands, architecture overview, limitations,
  and links to the detailed documents under `release/`.

It will not duplicate every grammar production or report-schema detail. The
README will instead link directly to the canonical language, CLI, CI, preprod,
migration, installation, changelog, and release-process documents.

## Files

| File | Change |
|---|---|
| `README.md` | New complete English project documentation. |

The complete proposed file is available at
`docs/generated/readme/README.md` for review. It does not alter runtime code,
the release workflow, release assets, or versioning.

## Validation after approval

1. verify all relative Markdown links resolve;
2. validate embedded example commands against the CLI contract;
3. review documentation for secret-safety and operational accuracy;
4. commit the documentation through the repository implementation workflow.
