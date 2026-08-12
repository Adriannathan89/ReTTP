//! UTF-8 source text and position types used by the lexer.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// A half-open byte range within a [`SourceText`].
///
/// `start` is included and `end` is excluded. Offsets are bytes, rather than
/// character indices, so a span can be passed directly to Rust's string slicing
/// APIs when both bounds are UTF-8 character boundaries.
pub struct SourceSpan {
    /// Inclusive byte offset of the span's first byte.
    pub start: usize,
    /// Exclusive byte offset immediately after the span's last byte.
    pub end: usize,
}

impl SourceSpan {
    #[must_use]
    /// Creates a span from inclusive `start` to exclusive `end` byte offsets.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    #[must_use]
    /// Returns the span length in bytes.
    ///
    /// Reversed spans return zero instead of underflowing.
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    /// Returns `true` when `start` and `end` are equal.
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// A one-based source position suitable for user-facing diagnostics.
pub struct SourceLocation {
    /// One-based line number.
    pub line: usize,
    /// One-based Unicode-scalar column number.
    pub column: usize,
}

#[derive(Debug, Clone)]
/// Named UTF-8 text consumed by the lexer.
///
/// The name is typically a file path, but can be any diagnostic label, such as
/// `"stdin"`. The text is private to preserve valid UTF-8 access through the
/// methods on this type.
pub struct SourceText {
    name: String,
    content: String,
}

impl SourceText {
    #[must_use]
    /// Creates source text with a diagnostic name and UTF-8 content.
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
        }
    }

    #[must_use]
    /// Returns the source name supplied to [`SourceText::new`].
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    /// Returns the complete UTF-8 source content.
    pub fn content(&self) -> &str {
        &self.content
    }

    #[must_use]
    /// Returns the source length in bytes.
    pub fn len(&self) -> usize {
        self.content.len()
    }

    #[must_use]
    /// Returns whether the source has no bytes.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    #[must_use]
    /// Returns the source substring covered by `span`.
    ///
    /// Returns `None` if the range is out of bounds or either bound splits a
    /// UTF-8 character. This method never panics for an arbitrary span.
    pub fn slice(&self, span: SourceSpan) -> Option<&str> {
        self.content.get(span.start..span.end)
    }

    #[must_use]
    /// Converts a byte offset to a one-based line and Unicode-scalar column.
    ///
    /// Offsets beyond the source are clamped to its end. An offset inside a
    /// multi-byte UTF-8 character is moved backward to that character boundary.
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

        let line = prefix.chars().filter(|&ch| ch == '\n').count() + 1;

        let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);

        let column = self.content[line_start..safe_offset].chars().count() + 1;

        SourceLocation { line, column }
    }
}
