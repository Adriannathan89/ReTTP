//! Interpolation placeholder scanning used by semantic validation.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InterpolationError {
    Empty,
    Unterminated,
}

pub(super) fn variable_names(raw: &str) -> Result<Vec<&str>, InterpolationError> {
    let mut names = Vec::new();
    let mut cursor = 0;

    while let Some(relative_start) = raw[cursor..].find("${") {
        let name_start = cursor + relative_start + 2;
        let Some(relative_end) = raw[name_start..].find('}') else {
            return Err(InterpolationError::Unterminated);
        };
        let name_end = name_start + relative_end;
        let name = &raw[name_start..name_end];
        if name.is_empty() {
            return Err(InterpolationError::Empty);
        }
        names.push(name);
        cursor = name_end + 1;
    }

    Ok(names)
}
