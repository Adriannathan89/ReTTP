rettp/
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
│   ├── rettp-domain/
│   ├── rettp-application/
│   ├── rettp-parser/
│   ├── rettp-http/
│   ├── rettp-runtime/
│   ├── rettp-reporter/
│   └── rettp-cli/
│
├── tests/
│   ├── fixtures/
│   │   ├── valid/
│   │   └── invalid/
│   ├── integration/
│   └── e2e/
│
├── examples/
│   ├── basic.rttp
│   ├── core.rttp
│   ├── pipeline.rttp
│   └── preprod.rttp
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