use crate::{
    lexer::{LexResult, LexerError, LexerErrorKind, Token, TokenKind, keyword_or_identifier},
    source::{SourceSpan, SourceText},
};

#[derive(Debug)]
pub struct Lexer<'source> {
    source: &'source SourceText,

    // running point marker
    start: usize,
    current: usize,

    // tokens and errors
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
            SourceSpan::new(self.current, self.current),
        ));

        LexResult {
            tokens: self.tokens,
            errors: self.errors,
        }
    }

    fn scan_token(&mut self) {
        let character = self.advanced();

        match character {
            // Whitespace
            ' ' | '\r' | '\t' | '\n' => {}

            // Structure
            '{' => self.add_token(TokenKind::LeftBrace),
            '}' => self.add_token(TokenKind::RightBrace),
            '[' => self.add_token(TokenKind::LeftBracket),
            ']' => self.add_token(TokenKind::RightBracket),
            ':' => self.add_token(TokenKind::Colon),
            '=' => self.add_token(TokenKind::Equal),
            ',' => self.add_token(TokenKind::Comma),

            // arrow or negative number
            '-' => {
                if self.peek() == Some('>') {
                    self.advanced();
                    self.add_token(TokenKind::Arrow);
                } else if self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                    self.scan_number();
                } else {
                    self.add_error(LexerErrorKind::UnexpectedCharacter { character });
                }
            }

            // String literal
            '"' => self.scan_string(),

            // comments
            '#' => self.skip_line_comment(),

            '/' => {
                if self.peek() == Some('/') {
                    self.advanced();
                    self.skip_line_comment();
                } else {
                    self.add_error(LexerErrorKind::UnexpectedCharacter { character });
                }
            }

            // Number literal
            '0'..='9' => self.scan_number(),

            // Identifier or keyword
            ch if is_identifier_start(ch) => self.scan_identifier(),

            // Unknown character
            ch => {
                self.add_error(LexerErrorKind::UnexpectedCharacter { character: ch });
            }
        }
    }

    fn scan_identifier(&mut self) {
        while self.peek().is_some_and(is_identifier_continue) {
            self.advanced();
        }

        let value = self.current_slice();
        let kind = keyword_or_identifier(value);

        self.add_token(kind);
    }

    fn scan_number(&mut self) {
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            self.advanced();
        }

        let is_float =
            self.peek() == Some('.') && self.peek_next().is_some_and(|ch| ch.is_ascii_digit());

        if is_float {
            self.advanced();

            while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                self.advanced();
            }
        }

        let raw = self.current_slice().to_owned();

        if is_float {
            match raw.parse::<f64>() {
                Ok(value) if value.is_finite() => {
                    self.add_token(TokenKind::NumberLiteral(value));
                }
                _ => {
                    self.add_error(LexerErrorKind::InvalidNumber { value: raw });
                }
            }

            return;
        }

        match raw.parse::<i64>() {
            Ok(value) => {
                self.add_token(TokenKind::IntegerLiteral(value));
            }
            Err(_) => {
                self.add_error(LexerErrorKind::InvalidInteger { value: raw });
            }
        }
    }

    fn scan_string(&mut self) {
        let mut value = String::new();

        loop {
            let Some(character) = self.peek() else {
                self.add_error(LexerErrorKind::UnterminatedString);
                return;
            };

            match character {
                '"' => {
                    self.advanced();

                    self.add_token(TokenKind::StringLiteral(value));
                    return;
                }

                '\n' => {
                    self.add_error(LexerErrorKind::UnterminatedString);
                    return;
                }

                '\\' => {
                    self.advanced();

                    let Some(escaped) = self.peek() else {
                        self.add_error(LexerErrorKind::UnterminatedString);
                        return;
                    };

                    self.advanced();

                    match escaped {
                        '"' => value.push('"'),
                        '\\' => value.push('\\'),
                        'n' => value.push('\n'),
                        'r' => value.push('\r'),
                        't' => value.push('\t'),

                        other => {
                            self.add_error(LexerErrorKind::InvalidEscapeSequence {
                                character: other,
                            });

                            self.recover_string();

                            return;
                        }
                    }
                }

                _ => {
                    self.advanced();
                    value.push(character);
                }
            }
        }
    }

    fn recover_string(&mut self) {
        while let Some(character) = self.peek() {
            match character {
                '"' => {
                    self.advanced();
                    break;
                }

                '\n' => break,

                _ => {
                    self.advanced();
                }
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some(character) = self.peek() {
            if character == '\n' {
                break;
            }

            self.advanced();
        }
    }

    #[must_use]
    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn advanced(&mut self) -> char {
        let character = self
            .peek()
            .expect("advance is only called when a character is available");

        self.current += character.len_utf8();

        character
    }

    #[must_use]
    fn peek(&self) -> Option<char> {
        self.source.content().get(self.current..)?.chars().next()
    }

    #[must_use]
    fn peek_next(&self) -> Option<char> {
        let source = self.source.content().get(self.current..)?;
        let mut charaters = source.chars();

        charaters.next()?;
        charaters.next()
    }

    #[must_use]
    fn current_slice(&self) -> &str {
        &self.source.content()[self.start..self.current]
    }

    #[must_use]
    fn current_span(&self) -> SourceSpan {
        SourceSpan::new(self.start, self.current)
    }

    fn add_token(&mut self, kind: TokenKind) {
        self.tokens.push(Token::new(kind, self.current_span()));
    }

    fn add_error(&mut self, kind: LexerErrorKind) {
        self.errors.push(LexerError::new(kind, self.current_span()));
    }
}

#[must_use]
fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

#[must_use]
fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}
