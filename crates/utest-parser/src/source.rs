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
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
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

        let line = prefix.chars().filter(|&ch| ch == '\n').count() + 1;

        let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);

        let column = self.content[line_start..safe_offset].chars().count() + 1;

        SourceLocation { line, column }
    }
}
