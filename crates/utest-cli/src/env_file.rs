//! Bounded dotenv-compatible parsing without variable expansion or value leaks.

use std::{fmt, fs::File, io::Read, path::Path};

use utest_domain::VariableName;
use utest_runtime::VariableAssignment;

/// Maximum accepted dotenv file size.
const MAX_ENV_FILE_BYTES: usize = 1024 * 1024;

/// Reads and parses one dotenv-compatible file.
///
/// Values remain opaque and are never included in returned diagnostics.
pub(crate) fn load(path: &Path) -> Result<Vec<VariableAssignment>, EnvFileError> {
    let file = File::open(path).map_err(EnvFileError::Io)?;
    let read_limit = u64::try_from(MAX_ENV_FILE_BYTES)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(EnvFileError::Io)?;
    if bytes.len() > MAX_ENV_FILE_BYTES {
        return Err(EnvFileError::TooLarge {
            limit_bytes: MAX_ENV_FILE_BYTES,
        });
    }
    let source = std::str::from_utf8(&bytes).map_err(|_| EnvFileError::InvalidUtf8)?;
    parse(source)
}

fn parse(source: &str) -> Result<Vec<VariableAssignment>, EnvFileError> {
    let mut assignments = Vec::new();
    for (index, raw_line) in source.lines().enumerate() {
        let line_number = index.saturating_add(1);
        let mut line = raw_line.trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(remainder) = line.strip_prefix("export")
            && remainder.chars().next().is_some_and(char::is_whitespace)
        {
            line = remainder.trim_start();
        }
        let (raw_name, raw_value) = line.split_once('=').ok_or(EnvFileError::InvalidEntry {
            line: line_number,
            reason: "expected NAME=VALUE",
        })?;
        let raw_name = raw_name.trim();
        let name = VariableName::new(raw_name).map_err(|_| EnvFileError::InvalidEntry {
            line: line_number,
            reason: "invalid variable name",
        })?;
        let value = parse_value(raw_value, line_number)?;
        assignments.push(VariableAssignment::new(name, value));
    }
    Ok(assignments)
}

fn parse_value(raw: &str, line: usize) -> Result<String, EnvFileError> {
    let raw = raw.trim_start();
    if let Some(quoted) = raw.strip_prefix('\'') {
        return parse_single_quoted(quoted, line);
    }
    if let Some(quoted) = raw.strip_prefix('"') {
        return parse_double_quoted(quoted, line);
    }
    Ok(parse_unquoted(raw))
}

fn parse_single_quoted(raw: &str, line: usize) -> Result<String, EnvFileError> {
    let Some(end) = raw.find('\'') else {
        return Err(EnvFileError::InvalidEntry {
            line,
            reason: "unterminated single-quoted value",
        });
    };
    validate_trailing(&raw[end + 1..], line)?;
    Ok(raw[..end].to_owned())
}

fn parse_double_quoted(raw: &str, line: usize) -> Result<String, EnvFileError> {
    let mut output = String::new();
    let mut characters = raw.char_indices();
    while let Some((index, character)) = characters.next() {
        match character {
            '"' => {
                validate_trailing(&raw[index + character.len_utf8()..], line)?;
                return Ok(output);
            }
            '\\' => {
                let Some((_, escaped)) = characters.next() else {
                    return Err(EnvFileError::InvalidEntry {
                        line,
                        reason: "unterminated escape in double-quoted value",
                    });
                };
                output.push(match escaped {
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '\\' => '\\',
                    '"' => '"',
                    _ => {
                        return Err(EnvFileError::InvalidEntry {
                            line,
                            reason: "unsupported escape in double-quoted value",
                        });
                    }
                });
            }
            character => output.push(character),
        }
    }
    Err(EnvFileError::InvalidEntry {
        line,
        reason: "unterminated double-quoted value",
    })
}

fn parse_unquoted(raw: &str) -> String {
    let comment = raw.char_indices().find_map(|(index, character)| {
        if character == '#'
            && (index == 0
                || raw[..index]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace))
        {
            Some(index)
        } else {
            None
        }
    });
    raw[..comment.unwrap_or(raw.len())].trim_end().to_owned()
}

fn validate_trailing(raw: &str, line: usize) -> Result<(), EnvFileError> {
    let trailing = raw.trim_start();
    if trailing.is_empty() || trailing.starts_with('#') {
        Ok(())
    } else {
        Err(EnvFileError::InvalidEntry {
            line,
            reason: "unexpected text after quoted value",
        })
    }
}

/// Value-free failure while loading a predefined-variable file.
#[derive(Debug)]
pub(crate) enum EnvFileError {
    /// The file could not be read.
    Io(std::io::Error),
    /// The file exceeded the fixed allocation boundary.
    TooLarge {
        /// Maximum accepted bytes.
        limit_bytes: usize,
    },
    /// The file was not valid UTF-8.
    InvalidUtf8,
    /// One source line violated the supported dotenv grammar.
    InvalidEntry {
        /// One-based line number.
        line: usize,
        /// Static explanation that never contains the parsed value.
        reason: &'static str,
    },
}

impl fmt::Display for EnvFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "could not read env file: {error}"),
            Self::TooLarge { limit_bytes } => {
                write!(formatter, "env file exceeds the {limit_bytes}-byte limit")
            }
            Self::InvalidUtf8 => formatter.write_str("env file must be valid UTF-8"),
            Self::InvalidEntry { line, reason } => {
                write!(formatter, "invalid env file entry at line {line}: {reason}")
            }
        }
    }
}

impl std::error::Error for EnvFileError {}
