# UTest — Codex Handoff: Lexer Phase

## 0. Tujuan dokumen

Dokumen ini adalah handoff lengkap untuk melanjutkan pengembangan **UTest**, sebuah universal HTTP test DSL dan runner berbasis Rust.

Codex harus menggunakan dokumen ini sebagai **reference specification**, bukan langsung menimpa implementasi lokal.

### Tugas pertama Codex

1. Baca implementasi lokal project terlebih dahulu.
2. Bandingkan implementasi lokal `utest-parser` dengan spesifikasi dan reference implementation di dokumen ini.
3. Jangan melakukan rewrite besar jika implementasi lokal sudah benar.
4. Identifikasi bug, ketidaksesuaian semantics, missing token, kesalahan UTF-8 boundary, error recovery yang salah, source span yang salah, dan public API yang tidak konsisten.
5. Jalankan atau tambahkan unit test untuk lexer.
6. Perbaiki implementation hanya jika diperlukan.
7. Pastikan command berikut lulus:

```bash
cargo fmt --all
cargo check -p utest-parser
cargo test -p utest-parser
cargo clippy -p utest-parser --all-targets -- -D warnings
```

8. Setelah selesai, laporkan file yang diubah, bug yang ditemukan, unit test yang ditambahkan, behavior yang divalidasi, serta command verification dan hasilnya.

---

# 1. Product Context

UTest adalah DSL deklaratif untuk melakukan HTTP testing terhadap aplikasi yang sudah berjalan.

Target utamanya adalah membuat satu format test yang dapat digunakan terhadap backend apa pun: Rust, Go, Java, Node.js, Python, PHP, .NET, dan teknologi lain selama menyediakan HTTP endpoint.

Runner akan mengirim request langsung ke HTTP port atau gateway aplikasi.

Contoh arah penggunaan:

```text
deploy preprod
    ↓
wait health/readiness
    ↓
utest run tests/preprod.utest
    ↓
core passed?
    ├── no  → abort / block release
    └── yes → continue remaining suite
```

UTest bukan pengganti seluruh unit testing framework bahasa.

Posisi utamanya adalah API testing, integration testing dari external HTTP boundary, post-deployment verification, preprod validation, release gating, serta contract/behavior verification.

---

# 2. DSL Semantics yang Sudah Disepakati

## 2.1 `core`

```text
core {
    test "health check" {
        ...
    }
}
```

Semantics:

- berisi mandatory test;
- dijalankan terlebih dahulu;
- sequential;
- fail-fast;
- jika salah satu core test gagal, seluruh suite di-abort;
- status suite menjadi `ABORTED`.

Execution semantics final akan diimplementasikan di application/runtime phase, bukan lexer.

## 2.2 `pipeline`

```text
pipeline "authentication flow" {
    test "login" {
        ...
    }

    test "view profile" {
        ...
    }
}
```

Semantics:

- pipeline adalah satu logical flow;
- test di dalam pipeline berjalan dari atas ke bawah;
- test dapat berbagi captured variable;
- jika satu step gagal, step berikutnya dalam pipeline di-skip;
- pipeline menjadi failed;
- suite tetap melanjutkan block setelah pipeline.

## 2.3 Independent `test`

```text
test "unauthorized request" {
    ...
}
```

Semantics:

- independen;
- kegagalan tidak langsung menghentikan suite;
- tidak boleh memiliki dependency tersembunyi pada test independen sebelumnya.

---

# 3. Assertion Semantics

## Type only

```text
access_token: string
```

Artinya field harus ada, tipe harus `string`, dan value bebas.

## Type + value

```text
success: boolean = true
```

Artinya field harus ada, tipe harus boolean, dan value harus `true`.

## Capture

```text
access_token: string -> ACCESS_TOKEN
```

Artinya field harus ada, type assertion harus sukses, dan setelah seluruh assertion test sukses value di-commit ke pipeline variable store dengan nama `ACCESS_TOKEN`.

Capture tidak dilakukan oleh lexer. Lexer hanya menghasilkan:

```text
Identifier("access_token")
Colon
TypeString
Arrow
Identifier("ACCESS_TOKEN")
```

---

# 4. Variable Semantics

String interpolation menggunakan:

```text
${VARIABLE}
```

Contoh:

```text
"Bearer ${ACCESS_TOKEN}"
```

Penting: lexer **tidak** memecah interpolation menjadi token terpisah.

Lexer menghasilkan satu token:

```rust
TokenKind::StringLiteral("Bearer ${ACCESS_TOKEN}".to_string())
```

Interpolation dilakukan pada runtime/application layer.

Pipeline capture bersifat pipeline-local. Environment dan CLI variable dapat menjadi external variable source pada fase runtime.

---

# 5. JSON / Object Semantics

Default object assertion direncanakan menggunakan partial matching.

Contoh:

```text
body {
    success: boolean = true
}
```

Response berikut tetap valid:

```json
{
  "success": true,
  "request_id": "abc"
}
```

Nested object menggunakan keyword:

```text
data: object {
    result: string
}
```

Bukan `json`, karena JSON juga mencakup array, number, boolean, string, dan null.

Exact matching akan didukung melalui syntax `exact`, tetapi detail parser/semantic bukan tanggung jawab lexer.

---

# 6. Clean Architecture Project

Workspace yang direncanakan:

```text
utest/
├── Cargo.toml
├── crates/
│   ├── utest-domain/
│   ├── utest-application/
│   ├── utest-parser/
│   ├── utest-http/
│   ├── utest-runtime/
│   ├── utest-reporter/
│   └── utest-cli/
├── tests/
├── examples/
└── docs/
```

Dependency direction:

```text
utest-domain
    ↑
utest-application
    ↑
adapters / runtime / CLI
```

`utest-parser` bertugas:

```text
Source
  ↓
Lexer
  ↓
Tokens
  ↓
Parser
  ↓
AST
  ↓
Semantic validation
  ↓
Domain Model
```

Pada fase lexer, parser crate tidak perlu mengetahui HTTP network, Tokio, Reqwest, runtime, atau execution semantics.

---

# 7. Current Timeline Position

Domain phase telah dikerjakan sebelumnya. Sekarang project berada pada:

> Minggu 3 — Lexer

Definition of Done lexer:

```text
[ ] SourceText tersedia
[ ] SourceSpan bekerja
[ ] SourceLocation bekerja

[ ] keyword dapat dibaca
[ ] identifier dapat dibaca
[ ] HTTP method dapat dibaca

[ ] string literal dapat dibaca
[ ] integer dapat dibaca
[ ] floating number dapat dibaca
[ ] negative number dapat dibaca
[ ] boolean dapat dibaca
[ ] null dapat dibaca

[ ] { } dapat dibaca
[ ] [ ] dapat dibaca
[ ] : dapat dibaca
[ ] = dapat dibaca
[ ] -> dapat dibaca
[ ] , dapat dibaca

[ ] # comment diabaikan
[ ] // comment diabaikan
[ ] whitespace diabaikan

[ ] invalid character menghasilkan error
[ ] unterminated string menghasilkan error
[ ] invalid escape menghasilkan error

[ ] lexer dapat mengumpulkan lebih dari satu error
[ ] setiap token memiliki SourceSpan
[ ] EOF selalu dihasilkan

[ ] contoh core dapat di-lex
[ ] contoh pipeline dapat di-lex
[ ] contoh independent test dapat di-lex

[ ] invalid input tidak menyebabkan panic
```

---

# 8. Target File Structure Lexer

Reference structure:

```text
crates/utest-parser/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── source.rs
│   └── lexer/
│       ├── mod.rs
│       ├── token.rs
│       ├── error.rs
│       └── scanner.rs
└── tests/
    └── lexer.rs
```

Codex harus memprioritaskan existing local structure jika hanya berbeda naming tetapi tetap bersih. Jangan rename file hanya demi mencocokkan reference ini kecuali ada alasan teknis yang jelas.

---

# 9. Cargo.toml Reference

## `crates/utest-parser/Cargo.toml`

```toml
[package]
name = "utest-parser"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
thiserror.workspace = true

[dev-dependencies]
pretty_assertions.workspace = true

[lints]
workspace = true
```

Untuk fase lexer saja, dependency seperti `reqwest`, `tokio`, `clap`, `serde_json`, `utest-http`, dan `utest-runtime` tidak diperlukan.

Jika existing local crate sudah menggunakan dependency lain karena pekerjaan sebelumnya, jangan menghapusnya tanpa memeriksa usage.

---

# 10. Full Reference Implementation

Reference implementation berikut harus digunakan untuk **validasi**, bukan sebagai perintah rewrite buta.

## 10.1 `src/source.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

impl SourceSpan {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct SourceText {
    name: String,
    content: String,
}

impl SourceText {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.content.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    #[must_use]
    pub fn slice(&self, span: SourceSpan) -> Option<&str> {
        self.content.get(span.start..span.end)
    }

    #[must_use]
    pub fn location(&self, offset: usize) -> SourceLocation {
        let offset = offset.min(self.content.len());

        let safe_offset = if self.content.is_char_boundary(offset) {
            offset
        } else {
            let mut value = offset;

            while value > 0 && !self.content.is_char_boundary(value) {
                value -= 1;
            }

            value
        };

        let prefix = &self.content[..safe_offset];

        let line = prefix
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1;

        let line_start = prefix
            .rfind('\n')
            .map_or(0, |index| index + 1);

        let column = self.content[line_start..safe_offset]
            .chars()
            .count()
            + 1;

        SourceLocation { line, column }
    }
}
```

### Design decision

`SourceSpan` menggunakan byte offsets, bukan character indices. Rust string adalah UTF-8, slicing `&str` menggunakan byte range, dan scanner `current` bergerak dengan `character.len_utf8()`.

---

## 10.2 `src/lexer/token.rs`

```rust
use crate::source::SourceSpan;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: SourceSpan,
}

impl Token {
    #[must_use]
    pub const fn new(kind: TokenKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Core,
    Pipeline,
    Test,

    Request,
    Expect,
    Body,
    Headers,
    Query,
    Status,
    Exact,

    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,

    TypeString,
    TypeBoolean,
    TypeInteger,
    TypeNumber,
    TypeObject,
    TypeArray,
    TypeAny,

    True,
    False,
    Null,

    Identifier(String),
    StringLiteral(String),
    IntegerLiteral(i64),
    NumberLiteral(f64),

    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Colon,
    Equal,
    Arrow,
    Comma,

    Eof,
}

#[must_use]
pub fn keyword_or_identifier(value: &str) -> TokenKind {
    match value {
        "core" => TokenKind::Core,
        "pipeline" => TokenKind::Pipeline,
        "test" => TokenKind::Test,

        "request" => TokenKind::Request,
        "expect" => TokenKind::Expect,
        "body" => TokenKind::Body,
        "headers" => TokenKind::Headers,
        "query" => TokenKind::Query,
        "status" => TokenKind::Status,
        "exact" => TokenKind::Exact,

        "GET" => TokenKind::Get,
        "POST" => TokenKind::Post,
        "PUT" => TokenKind::Put,
        "PATCH" => TokenKind::Patch,
        "DELETE" => TokenKind::Delete,
        "HEAD" => TokenKind::Head,
        "OPTIONS" => TokenKind::Options,

        "string" => TokenKind::TypeString,
        "boolean" => TokenKind::TypeBoolean,
        "integer" => TokenKind::TypeInteger,
        "number" => TokenKind::TypeNumber,
        "object" => TokenKind::TypeObject,
        "array" => TokenKind::TypeArray,
        "any" => TokenKind::TypeAny,

        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "null" => TokenKind::Null,

        _ => TokenKind::Identifier(value.to_owned()),
    }
}
```

### Important

HTTP methods case-sensitive pada grammar MVP:

```text
GET   valid method token
get   Identifier("get")
```

Parser nanti yang melaporkan bahwa setelah `request` ia mengharapkan HTTP method.

---

## 10.3 `src/lexer/error.rs`

```rust
use thiserror::Error;

use crate::source::SourceSpan;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum LexerErrorKind {
    #[error("unexpected character `{character}`")]
    UnexpectedCharacter {
        character: char,
    },

    #[error("unterminated string literal")]
    UnterminatedString,

    #[error("invalid escape sequence `\\{character}`")]
    InvalidEscapeSequence {
        character: char,
    },

    #[error("invalid integer literal `{value}`")]
    InvalidInteger {
        value: String,
    },

    #[error("invalid number literal `{value}`")]
    InvalidNumber {
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexerError {
    pub kind: LexerErrorKind,
    pub span: SourceSpan,
}

impl LexerError {
    #[must_use]
    pub const fn new(
        kind: LexerErrorKind,
        span: SourceSpan,
    ) -> Self {
        Self { kind, span }
    }
}
```

---

## 10.4 `src/lexer/mod.rs`

```rust
mod error;
mod scanner;
mod token;

use crate::source::SourceText;

pub use error::{
    LexerError,
    LexerErrorKind,
};

pub use scanner::Lexer;

pub use token::{
    keyword_or_identifier,
    Token,
    TokenKind,
};

#[derive(Debug)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub errors: Vec<LexerError>,
}

impl LexResult {
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }

    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[must_use]
pub fn lex(source: &SourceText) -> LexResult {
    Lexer::new(source).scan()
}
```

Lexer mengumpulkan error melalui `LexResult`, bukan berhenti pada error pertama.

---

## 10.5 `src/lexer/scanner.rs`

```rust
use crate::{
    lexer::{
        keyword_or_identifier,
        LexResult,
        LexerError,
        LexerErrorKind,
        Token,
        TokenKind,
    },
    source::{
        SourceSpan,
        SourceText,
    },
};

#[derive(Debug)]
pub struct Lexer<'source> {
    source: &'source SourceText,
    start: usize,
    current: usize,
    tokens: Vec<Token>,
    errors: Vec<LexerError>,
}

impl<'source> Lexer<'source> {
    #[must_use]
    pub fn new(source: &'source SourceText) -> Self {
        Self {
            source,
            start: 0,
            current: 0,
            tokens: Vec::new(),
            errors: Vec::new(),
        }
    }

    #[must_use]
    pub fn scan(mut self) -> LexResult {
        while !self.is_at_end() {
            self.start = self.current;
            self.scan_token();
        }

        self.tokens.push(Token::new(
            TokenKind::Eof,
            SourceSpan::new(
                self.current,
                self.current,
            ),
        ));

        LexResult {
            tokens: self.tokens,
            errors: self.errors,
        }
    }

    fn scan_token(&mut self) {
        let Some(character) = self.advance() else {
            return;
        };

        match character {
            ' ' | '\t' | '\r' | '\n' => {}

            '{' => self.add_token(TokenKind::LeftBrace),
            '}' => self.add_token(TokenKind::RightBrace),
            '[' => self.add_token(TokenKind::LeftBracket),
            ']' => self.add_token(TokenKind::RightBracket),
            ':' => self.add_token(TokenKind::Colon),
            '=' => self.add_token(TokenKind::Equal),
            ',' => self.add_token(TokenKind::Comma),

            '-' => {
                if self.peek() == Some('>') {
                    self.advance();
                    self.add_token(TokenKind::Arrow);
                } else if self
                    .peek()
                    .is_some_and(|ch| ch.is_ascii_digit())
                {
                    self.scan_number();
                } else {
                    self.push_error(
                        LexerErrorKind::UnexpectedCharacter {
                            character,
                        },
                    );
                }
            }

            '"' => self.scan_string(),

            '#' => self.skip_line_comment(),

            '/' => {
                if self.peek() == Some('/') {
                    self.advance();
                    self.skip_line_comment();
                } else {
                    self.push_error(
                        LexerErrorKind::UnexpectedCharacter {
                            character,
                        },
                    );
                }
            }

            '0'..='9' => self.scan_number(),

            ch if is_identifier_start(ch) => {
                self.scan_identifier();
            }

            ch => {
                self.push_error(
                    LexerErrorKind::UnexpectedCharacter {
                        character: ch,
                    },
                );
            }
        }
    }

    fn scan_identifier(&mut self) {
        while self
            .peek()
            .is_some_and(is_identifier_continue)
        {
            self.advance();
        }

        let value = self.current_slice();
        let kind = keyword_or_identifier(value);

        self.add_token(kind);
    }

    fn scan_number(&mut self) {
        while self
            .peek()
            .is_some_and(|ch| ch.is_ascii_digit())
        {
            self.advance();
        }

        let is_float = self.peek() == Some('.')
            && self
                .peek_next()
                .is_some_and(|ch| ch.is_ascii_digit());

        if is_float {
            self.advance();

            while self
                .peek()
                .is_some_and(|ch| ch.is_ascii_digit())
            {
                self.advance();
            }
        }

        let raw = self.current_slice().to_owned();

        if is_float {
            match raw.parse::<f64>() {
                Ok(value) => {
                    self.add_token(
                        TokenKind::NumberLiteral(value),
                    );
                }

                Err(_) => {
                    self.push_error(
                        LexerErrorKind::InvalidNumber {
                            value: raw,
                        },
                    );
                }
            }

            return;
        }

        match raw.parse::<i64>() {
            Ok(value) => {
                self.add_token(
                    TokenKind::IntegerLiteral(value),
                );
            }

            Err(_) => {
                self.push_error(
                    LexerErrorKind::InvalidInteger {
                        value: raw,
                    },
                );
            }
        }
    }

    fn scan_string(&mut self) {
        let mut value = String::new();

        loop {
            let Some(character) = self.peek() else {
                self.push_error(
                    LexerErrorKind::UnterminatedString,
                );

                return;
            };

            match character {
                '"' => {
                    self.advance();
                    self.add_token(
                        TokenKind::StringLiteral(value),
                    );
                    return;
                }

                '\n' => {
                    self.push_error(
                        LexerErrorKind::UnterminatedString,
                    );
                    return;
                }

                '\\' => {
                    self.advance();

                    let Some(escaped) = self.advance() else {
                        self.push_error(
                            LexerErrorKind::UnterminatedString,
                        );
                        return;
                    };

                    match escaped {
                        '"' => value.push('"'),
                        '\\' => value.push('\\'),
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),

                        other => {
                            self.push_error(
                                LexerErrorKind::InvalidEscapeSequence {
                                    character: other,
                                },
                            );

                            self.recover_string();
                            return;
                        }
                    }
                }

                _ => {
                    self.advance();
                    value.push(character);
                }
            }
        }
    }

    fn recover_string(&mut self) {
        while let Some(character) = self.peek() {
            match character {
                '"' => {
                    self.advance();
                    break;
                }

                '\n' => break,

                _ => {
                    self.advance();
                }
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some(character) = self.peek() {
            if character == '\n' {
                break;
            }

            self.advance();
        }
    }

    #[must_use]
    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn advance(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.current += character.len_utf8();
        Some(character)
    }

    #[must_use]
    fn peek(&self) -> Option<char> {
        self.source
            .content()
            .get(self.current..)?
            .chars()
            .next()
    }

    #[must_use]
    fn peek_next(&self) -> Option<char> {
        let source = self
            .source
            .content()
            .get(self.current..)?;

        let mut characters = source.chars();

        characters.next()?;
        characters.next()
    }

    #[must_use]
    fn current_slice(&self) -> &str {
        &self.source.content()[
            self.start..self.current
        ]
    }

    fn add_token(&mut self, kind: TokenKind) {
        self.tokens.push(
            Token::new(
                kind,
                self.current_span(),
            ),
        );
    }

    fn push_error(&mut self, kind: LexerErrorKind) {
        self.errors.push(
            LexerError::new(
                kind,
                self.current_span(),
            ),
        );
    }

    #[must_use]
    fn current_span(&self) -> SourceSpan {
        SourceSpan::new(
            self.start,
            self.current,
        )
    }
}

#[must_use]
fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic()
        || character == '_'
}

#[must_use]
fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || character == '_'
}
```

---

## 10.6 `src/lib.rs`

```rust
pub mod lexer;
pub mod source;

pub use lexer::{
    lex,
    LexResult,
    Lexer,
    LexerError,
    LexerErrorKind,
    Token,
    TokenKind,
};

pub use source::{
    SourceLocation,
    SourceSpan,
    SourceText,
};
```

---

# 11. Important Scanner State Explanation

`Lexer` menyimpan:

```rust
pub struct Lexer<'source> {
    source: &'source SourceText,
    start: usize,
    current: usize,
    tokens: Vec<Token>,
    errors: Vec<LexerError>,
}
```

`start` adalah byte offset awal token yang sedang diproses. `current` adalah byte offset posisi pembacaan scanner saat ini.

Contoh:

```text
request POST
        ^^^^
```

Saat `POST` selesai dibaca:

```text
start   = 8
current = 12
```

Maka `&source[start..current]` adalah `POST` dan token dengan span `8..12` dipush ke `tokens`.

`start` dan `current` bukan tempat menyimpan token; keduanya hanya state sementara lexer selama scanning.

---

# 12. Unit Test Reference

Codex harus terlebih dahulu melihat test lokal yang sudah ada. Tambahkan test yang belum tersedia dan jangan menduplikasi test tanpa alasan.

## `tests/lexer.rs`

```rust
use utest_parser::{
    lex,
    LexerErrorKind,
    SourceSpan,
    SourceText,
    TokenKind,
};

fn kinds(source: &str) -> Vec<TokenKind> {
    let source = SourceText::new(
        "test.utest",
        source,
    );

    let result = lex(&source);

    assert!(
        result.errors.is_empty(),
        "unexpected lexer errors: {:?}",
        result.errors
    );

    result
        .tokens
        .into_iter()
        .map(|token| token.kind)
        .collect()
}

#[test]
fn lexes_empty_source_as_eof() {
    assert_eq!(
        kinds(""),
        vec![TokenKind::Eof]
    );
}

#[test]
fn lexes_block_keywords() {
    assert_eq!(
        kinds("core pipeline test"),
        vec![
            TokenKind::Core,
            TokenKind::Pipeline,
            TokenKind::Test,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_test_syntax_keywords() {
    assert_eq!(
        kinds("request expect body headers query status exact"),
        vec![
            TokenKind::Request,
            TokenKind::Expect,
            TokenKind::Body,
            TokenKind::Headers,
            TokenKind::Query,
            TokenKind::Status,
            TokenKind::Exact,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_http_methods() {
    assert_eq!(
        kinds("GET POST PUT PATCH DELETE HEAD OPTIONS"),
        vec![
            TokenKind::Get,
            TokenKind::Post,
            TokenKind::Put,
            TokenKind::Patch,
            TokenKind::Delete,
            TokenKind::Head,
            TokenKind::Options,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn http_methods_are_case_sensitive() {
    assert_eq!(
        kinds("get post"),
        vec![
            TokenKind::Identifier("get".to_string()),
            TokenKind::Identifier("post".to_string()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_assertion_types() {
    assert_eq!(
        kinds("string boolean integer number object array any"),
        vec![
            TokenKind::TypeString,
            TokenKind::TypeBoolean,
            TokenKind::TypeInteger,
            TokenKind::TypeNumber,
            TokenKind::TypeObject,
            TokenKind::TypeArray,
            TokenKind::TypeAny,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_literal_keywords() {
    assert_eq!(
        kinds("true false null"),
        vec![
            TokenKind::True,
            TokenKind::False,
            TokenKind::Null,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_identifiers() {
    assert_eq!(
        kinds("access_token ACCESS_TOKEN _value value123"),
        vec![
            TokenKind::Identifier("access_token".to_string()),
            TokenKind::Identifier("ACCESS_TOKEN".to_string()),
            TokenKind::Identifier("_value".to_string()),
            TokenKind::Identifier("value123".to_string()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_symbols() {
    assert_eq!(
        kinds("{ } [ ] : = -> ,"),
        vec![
            TokenKind::LeftBrace,
            TokenKind::RightBrace,
            TokenKind::LeftBracket,
            TokenKind::RightBracket,
            TokenKind::Colon,
            TokenKind::Equal,
            TokenKind::Arrow,
            TokenKind::Comma,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_integer_literal() {
    assert_eq!(
        kinds("200"),
        vec![
            TokenKind::IntegerLiteral(200),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_negative_integer_literal() {
    assert_eq!(
        kinds("-42"),
        vec![
            TokenKind::IntegerLiteral(-42),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_number_literal() {
    assert_eq!(
        kinds("10.5"),
        vec![
            TokenKind::NumberLiteral(10.5),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_negative_number_literal() {
    assert_eq!(
        kinds("-10.5"),
        vec![
            TokenKind::NumberLiteral(-10.5),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_string_literal() {
    assert_eq!(
        kinds(r#""hello world""#),
        vec![
            TokenKind::StringLiteral("hello world".to_string()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_string_escape_sequences() {
    assert_eq!(
        kinds(r#""a\nb\t\"c\"\\d""#),
        vec![
            TokenKind::StringLiteral("a\nb\t\"c\"\\d".to_string()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn keeps_variable_interpolation_inside_string() {
    assert_eq!(
        kinds(r#""Bearer ${ACCESS_TOKEN}""#),
        vec![
            TokenKind::StringLiteral(
                "Bearer ${ACCESS_TOKEN}".to_string(),
            ),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_type_only_assertion() {
    assert_eq!(
        kinds("access_token: string"),
        vec![
            TokenKind::Identifier("access_token".to_string()),
            TokenKind::Colon,
            TokenKind::TypeString,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_type_and_value_assertion() {
    assert_eq!(
        kinds("success: boolean = true"),
        vec![
            TokenKind::Identifier("success".to_string()),
            TokenKind::Colon,
            TokenKind::TypeBoolean,
            TokenKind::Equal,
            TokenKind::True,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn lexes_capture_assertion() {
    assert_eq!(
        kinds("access_token: string -> ACCESS_TOKEN"),
        vec![
            TokenKind::Identifier("access_token".to_string()),
            TokenKind::Colon,
            TokenKind::TypeString,
            TokenKind::Arrow,
            TokenKind::Identifier("ACCESS_TOKEN".to_string()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn ignores_hash_comment() {
    assert_eq!(
        kinds("test # ignored\ncore"),
        vec![
            TokenKind::Test,
            TokenKind::Core,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn ignores_double_slash_comment() {
    assert_eq!(
        kinds("test // ignored\ncore"),
        vec![
            TokenKind::Test,
            TokenKind::Core,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn ignores_whitespace() {
    assert_eq!(
        kinds("\n\t test \r\n core   "),
        vec![
            TokenKind::Test,
            TokenKind::Core,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn records_token_span() {
    let source = SourceText::new(
        "test.utest",
        "test POST",
    );

    let result = lex(&source);

    assert!(result.errors.is_empty());

    assert_eq!(
        result.tokens[0].span,
        SourceSpan::new(0, 4),
    );

    assert_eq!(
        result.tokens[1].span,
        SourceSpan::new(5, 9),
    );
}

#[test]
fn eof_span_is_zero_length_at_end_of_source() {
    let source = SourceText::new(
        "test.utest",
        "test",
    );

    let result = lex(&source);

    let eof = result
        .tokens
        .last()
        .expect("EOF token should exist");

    assert_eq!(
        eof.span,
        SourceSpan::new(4, 4),
    );
}

#[test]
fn source_location_tracks_lines_and_columns() {
    let source = SourceText::new(
        "test.utest",
        "core {\n  test",
    );

    let location = source.location(9);

    assert_eq!(location.line, 2);
    assert_eq!(location.column, 3);
}

#[test]
fn source_location_is_unicode_aware_for_columns() {
    let source = SourceText::new(
        "unicode.utest",
        "你a\nx",
    );

    let location = source.location(5);

    assert_eq!(location.line, 2);
    assert_eq!(location.column, 1);
}

#[test]
fn string_token_span_uses_byte_offsets_with_unicode() {
    let source = SourceText::new(
        "unicode.utest",
        r#""你好""#,
    );

    let result = lex(&source);

    assert!(result.errors.is_empty());

    assert_eq!(
        result.tokens[0].span,
        SourceSpan::new(0, 8),
    );

    assert_eq!(
        result.tokens[0].kind,
        TokenKind::StringLiteral("你好".to_string()),
    );
}

#[test]
fn reports_unexpected_character() {
    let source = SourceText::new(
        "test.utest",
        "@",
    );

    let result = lex(&source);

    assert_eq!(result.errors.len(), 1);

    assert!(matches!(
        result.errors[0].kind,
        LexerErrorKind::UnexpectedCharacter {
            character: '@'
        }
    ));
}

#[test]
fn reports_unterminated_string_at_eof() {
    let source = SourceText::new(
        "test.utest",
        r#""hello"#,
    );

    let result = lex(&source);

    assert_eq!(result.errors.len(), 1);

    assert!(matches!(
        result.errors[0].kind,
        LexerErrorKind::UnterminatedString,
    ));
}

#[test]
fn reports_unterminated_string_at_newline() {
    let source = SourceText::new(
        "test.utest",
        "\"hello\ncore",
    );

    let result = lex(&source);

    assert_eq!(result.errors.len(), 1);

    assert!(matches!(
        result.errors[0].kind,
        LexerErrorKind::UnterminatedString,
    ));

    assert!(
        result.tokens
            .iter()
            .any(|token| token.kind == TokenKind::Core),
        "lexer should recover on the next line",
    );
}

#[test]
fn reports_invalid_escape_sequence() {
    let source = SourceText::new(
        "test.utest",
        r#""hello\q""#,
    );

    let result = lex(&source);

    assert_eq!(result.errors.len(), 1);

    assert!(matches!(
        result.errors[0].kind,
        LexerErrorKind::InvalidEscapeSequence {
            character: 'q'
        }
    ));
}

#[test]
fn collects_multiple_errors() {
    let source = SourceText::new(
        "test.utest",
        "@ ? $",
    );

    let result = lex(&source);

    assert_eq!(result.errors.len(), 3);
}

#[test]
fn always_emits_eof_even_when_errors_exist() {
    let source = SourceText::new(
        "test.utest",
        "@",
    );

    let result = lex(&source);

    assert!(matches!(
        result.tokens.last().map(|token| &token.kind),
        Some(TokenKind::Eof),
    ));
}

#[test]
fn lexes_complete_pipeline_example() {
    let source = SourceText::new(
        "pipeline.utest",
        r#"
pipeline "authentication flow" {
    test "login" {
        request POST "/login" {
            body {
                email = "${TEST_USER_EMAIL}"
                password = "${TEST_USER_PASSWORD}"
            }
        }

        expect {
            status = 200

            body {
                refresh_token: string
                access_token: string -> ACCESS_TOKEN
            }
        }
    }

    test "view data" {
        request GET "/data" {
            headers {
                Authorization = "Bearer ${ACCESS_TOKEN}"
            }
        }

        expect {
            status = 200

            body {
                success: boolean = true
            }
        }
    }
}
"#,
    );

    let result = lex(&source);

    assert!(
        result.errors.is_empty(),
        "unexpected lexer errors: {:?}",
        result.errors,
    );

    assert!(matches!(
        result.tokens.first().map(|token| &token.kind),
        Some(TokenKind::Pipeline),
    ));

    assert!(matches!(
        result.tokens.last().map(|token| &token.kind),
        Some(TokenKind::Eof),
    ));
}
```

---

# 13. Additional Test Cases Codex Harus Pertimbangkan

Codex harus memeriksa apakah local implementation membutuhkan test tambahan untuk:

## Integer overflow

```text
999999999999999999999999999999
```

Expected:

- lexer error `InvalidInteger`;
- no panic;
- scanning tetap lanjut.

## Valid token setelah lexical error

```text
@ core
```

Expected satu lexical error, `Core` tetap ditemukan, dan `Eof` tetap ada.

## Lone `-`

```text
-
```

Expected `UnexpectedCharacter('-')`.

## Slash bukan comment

```text
/
```

Expected `UnexpectedCharacter('/')`.

## Float edge `1.`

Current reference behavior:

```text
IntegerLiteral(1)
UnexpectedCharacter('.')
```

Jangan diam-diam mengubah grammar menjadi menerima `1.` kecuali specification diubah.

## Dot-leading `.5`

Current reference behavior:

```text
UnexpectedCharacter('.')
IntegerLiteral(5)
```

## Unicode

Unicode valid di dalam string. Identifier MVP ASCII-only. Unicode di luar string menjadi unexpected character kecuali language spec nanti diubah.

---

# 14. Expected Lexing Examples

## Example A

Input:

```text
status = 200
```

Output:

```text
Status
Equal
IntegerLiteral(200)
Eof
```

## Example B

Input:

```text
success: boolean = true
```

Output:

```text
Identifier("success")
Colon
TypeBoolean
Equal
True
Eof
```

## Example C

Input:

```text
access_token: string -> ACCESS_TOKEN
```

Output:

```text
Identifier("access_token")
Colon
TypeString
Arrow
Identifier("ACCESS_TOKEN")
Eof
```

## Example D

Input:

```text
request POST "/login"
```

Output:

```text
Request
Post
StringLiteral("/login")
Eof
```

## Example E

Input:

```text
"Bearer ${ACCESS_TOKEN}"
```

Output:

```text
StringLiteral("Bearer ${ACCESS_TOKEN}")
Eof
```

---

# 15. What Lexer Must NOT Validate

Lexer tidak boleh memvalidasi semantics berikut.

## Invalid HTTP status

```text
status = 999
```

Lexer tetap menghasilkan `Status`, `Equal`, `IntegerLiteral(999)`. Semantic validator nanti yang menolak status.

## Unknown method

```text
request FETCH "/users"
```

Lexer menghasilkan `Request`, `Identifier("FETCH")`, `StringLiteral("/users")`. Parser nanti yang mengatakan expected HTTP method.

## Undefined variable

```text
"Bearer ${MISSING_TOKEN}"
```

Lexer menghasilkan string literal biasa. Variable resolution bukan tugas lexer.

## Missing expect

```text
test "x" {
    request GET "/x"
}
```

Lexer tidak peduli. Parser/semantic validation yang menangani.

## Core execution semantics

Lexer tidak mengetahui `core failed → suite abort`. Ini execution engine concern.

---

# 16. Error Recovery Requirements

Lexer harus berusaha lanjut setelah lexical error jika memungkinkan.

Contoh:

```text
@ core ? test
```

Expected:

- error untuk `@`;
- `Core`;
- error untuk `?`;
- `Test`;
- `Eof`.

Untuk invalid escape dalam string:

```text
"abc\q" core
```

Current reference:

- report invalid escape;
- recover sampai closing quote/newline;
- lanjut membaca `core`.

Untuk unterminated string pada newline:

```text
"abc
core
```

Current reference:

- report unterminated string;
- newline diproses sebagai whitespace pada loop berikutnya;
- `core` tetap dapat dibaca.

---

# 17. Public API Expected

Minimal public API yang harus tersedia secara konseptual:

```rust
let source = SourceText::new(
    "auth.utest",
    content,
);

let result = lex(&source);

if result.has_errors() {
    // diagnostics
}

for token in result.tokens {
    // parser phase later
}
```

Jika local implementation memakai nama API yang sedikit berbeda tetapi sama bersihnya, Codex tidak perlu mengubah hanya demi naming.

---

# 18. Codex First Task — Detailed Execution Plan

## Step 1 — Inspect repository

Temukan root `Cargo.toml`, `crates/utest-parser`, `crates/utest-domain`, `tests`, dan `examples`.

Baca implementation lokal lexer seluruhnya. Jangan membuat assumption hanya dari handoff.

## Step 2 — Compare local lexer with this spec

Validasi:

```text
SourceText
SourceSpan
SourceLocation
Token
TokenKind
keyword classification
LexerErrorKind
LexResult
Lexer state
scan loop
UTF-8 behavior
number scanning
string scanning
comment scanning
error recovery
EOF behavior
```

## Step 3 — Preserve correct local code

Jika local implementation lebih idiomatic, lebih kecil, atau lebih maintainable tetapi behavior sama, pertahankan implementation lokal.

Handoff ini mendefinisikan behavior yang diharapkan, bukan style absolut.

## Step 4 — Add/complete tests

Prioritas test:

1. keyword;
2. method;
3. types;
4. identifier;
5. symbols;
6. integer;
7. float;
8. negative number;
9. string;
10. escape;
11. interpolation untouched;
12. comment;
13. spans;
14. Unicode;
15. error recovery;
16. multiple errors;
17. EOF;
18. complete pipeline source.

## Step 5 — Run verification

```bash
cargo fmt --all
cargo check -p utest-parser
cargo test -p utest-parser
cargo clippy -p utest-parser --all-targets -- -D warnings
```

Jika workspace config menyebabkan package name berbeda, gunakan equivalent command dan jelaskan.

## Step 6 — Do not start parser phase

Task pertama Codex hanya:

> validate dan stabilize lexer.

Jangan mulai membuat AST/parser kecuali diperlukan untuk memperbaiki compile dependency yang sudah ada.

---

# 19. Acceptance Criteria for Codex Task 1

Task dianggap selesai bila:

```text
[ ] existing local lexer sudah dibaca
[ ] implementation dibandingkan dengan handoff
[ ] behavioral bugs ditemukan/dinyatakan tidak ada
[ ] missing unit tests ditambahkan
[ ] source spans tervalidasi
[ ] UTF-8 scanner tervalidasi
[ ] error collection tervalidasi
[ ] interpolation tetap utuh dalam StringLiteral
[ ] capture arrow tervalidasi
[ ] complete DSL pipeline dapat di-lex
[ ] cargo check lulus
[ ] cargo test lulus
[ ] clippy -D warnings lulus
[ ] formatter lulus
```

---

# 20. Non-goals of Codex Task 1

Jangan mengimplementasikan:

- parser AST;
- semantic validator;
- HTTP adapter;
- Reqwest;
- assertion engine runtime;
- variable resolution;
- execution engine;
- reporter;
- CLI behavior;
- pipeline execution;
- core abort logic.

Semua itu fase berikutnya.

---

# 21. Next Timeline After Lexer

Setelah lexer dinyatakan stable:

## Minggu 4 — Parser AST

Target:

```text
Vec<Token>
   ↓
Parser
   ↓
AST
```

Planned files:

```text
src/parser/
├── mod.rs
├── suite_parser.rs
├── block_parser.rs
├── test_parser.rs
├── request_parser.rs
├── expectation_parser.rs
└── value_parser.rs

src/ast/
├── mod.rs
└── nodes.rs
```

AST harus mempertahankan source spans untuk diagnostic. Parser tidak langsung melakukan HTTP execution.

---

# 22. High-Level Project Roadmap

```text
1. Language semantics       DONE / defined
2. Domain model             DONE / implemented with user
3. Lexer                    CURRENT
4. Parser AST               NEXT
5. Semantic validation
6. HTTP adapter
7. Assertion engine
8. Variable interpolation/capture runtime
9. Execution engine
10. CLI and reporter
11. End-to-end stability
12. MVP release / preprod trial
```

---

# 23. Final Guidance to Codex

Prioritas kualitas untuk UTest:

1. predictable semantics;
2. excellent diagnostics;
3. no panic on malformed user input;
4. deterministic behavior;
5. clean architectural boundaries;
6. simple language;
7. minimal hidden magic.

Jangan memperluas DSL tanpa kebutuhan.

UTest sengaja bukan general-purpose programming language. Hindari menambahkan loops, functions, classes, macros, arbitrary scripting, atau implicit cross-test dependencies pada fase awal.

Lexer harus tetap sederhana:

```text
characters
    ↓
tokens + spans + lexical errors
```

Itu saja tanggung jawabnya.
