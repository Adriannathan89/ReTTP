# CI Integration

Pin a release version and verify its checksum. Avoid downloading an unpinned
`latest` asset in a production release gate.

## GitHub Actions

```yaml
name: Pre-production verification

on:
  workflow_dispatch:

permissions:
  contents: read

jobs:
  verify:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4
      - name: Install UTest 0.1.0
        shell: bash
        run: |
          version=v0.1.0
          repository=Adriannathan89/utest
          asset="utest-${version}-x86_64-unknown-linux-gnu.tar.gz"
          curl --fail --location --silent --show-error \
            --output "${asset}" \
            "https://github.com/${repository}/releases/download/${version}/${asset}"
          curl --fail --location --silent --show-error \
            --output SHA256SUMS \
            "https://github.com/${repository}/releases/download/${version}/SHA256SUMS"
          grep " ${asset}$" SHA256SUMS | sha256sum --check
          tar -xzf "${asset}"
          sudo install -m 0755 utest /usr/local/bin/utest
      - name: Run suite
        env:
          API_TOKEN: ${{ secrets.PREPROD_API_TOKEN }}
        run: |
          utest run tests/preprod.utest \
            --base-url "${{ vars.PREPROD_BASE_URL }}" \
            --junit-file reports/utest.xml \
            --json-file reports/utest.json
      - name: Upload reports
        if: always()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4
        with:
          name: utest-reports
          path: reports/
          if-no-files-found: ignore
```

The runner redacts converted report values, but CI logs and artifacts should
still be access-controlled. Pass secrets through the environment or secret
store, never in committed suite files.

## GitLab CI

```yaml
preprod-verification:
  image: ubuntu:24.04
  stage: test
  timeout: 10m
  variables:
    UTEST_VERSION: "v0.1.0"
  before_script:
    - apt-get update && apt-get install --yes ca-certificates curl
    - export ASSET="utest-${UTEST_VERSION}-x86_64-unknown-linux-gnu.tar.gz"
    - curl --fail --location --silent --show-error --output "$ASSET" "https://github.com/Adriannathan89/utest/releases/download/${UTEST_VERSION}/${ASSET}"
    - curl --fail --location --silent --show-error --output SHA256SUMS "https://github.com/Adriannathan89/utest/releases/download/${UTEST_VERSION}/SHA256SUMS"
    - grep " $ASSET$" SHA256SUMS | sha256sum --check
    - tar -xzf "$ASSET"
    - install -m 0755 utest /usr/local/bin/utest
  script:
    - utest run tests/preprod.utest --base-url "$PREPROD_BASE_URL" --junit-file reports/utest.xml --json-file reports/utest.json
  artifacts:
    when: always
    reports:
      junit: reports/utest.xml
    paths:
      - reports/utest.json
```

Configure `PREPROD_BASE_URL` and sensitive variables as protected or masked CI
variables. UTest's nonzero exit codes naturally fail the CI job.
