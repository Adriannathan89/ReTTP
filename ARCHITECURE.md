utest/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── rust-toolchain.toml
├── deny.toml
├── .gitignore
├── .editorconfig
├── .github/
│   └── workflows/
│       ├── ci.yml
│       └── release.yml
│
├── crates/
│   ├── utest-domain/
│   ├── utest-application/
│   ├── utest-parser/
│   ├── utest-http/
│   ├── utest-runtime/
│   ├── utest-reporter/
│   └── utest-cli/
│
├── tests/
│   ├── fixtures/
│   │   ├── valid/
│   │   └── invalid/
│   ├── integration/
│   └── e2e/
│
├── examples/
│   ├── basic.utest
│   ├── core.utest
│   ├── pipeline.utest
│   └── preprod.utest
│
├── docs/
│   ├── language-spec.md
│   ├── execution-model.md
│   ├── architecture.md
│   ├── assertions.md
│   └── cli.md
│
└── scripts/
    ├── release.sh
    └── test-e2e.sh