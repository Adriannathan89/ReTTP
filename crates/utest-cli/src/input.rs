//! Bounded UTF-8 source-file input.

use std::{
    error::Error,
    fmt,
    fs::File,
    io::{self, Read},
    path::Path,
};

/// Maximum number of source bytes accepted by `check` and `run`.
pub(crate) const MAX_SOURCE_BYTES: usize = 5 * 1024 * 1024;

/// A value-free failure produced while reading one source file.
#[derive(Debug)]
pub(crate) enum SourceInputError {
    /// The filesystem operation failed.
    Io(io::Error),
    /// The source exceeded [`MAX_SOURCE_BYTES`].
    TooLarge,
    /// The bounded source bytes were not valid UTF-8.
    InvalidUtf8,
}

impl fmt::Display for SourceInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => fmt::Display::fmt(error, formatter),
            Self::TooLarge => write!(
                formatter,
                "source file exceeds the {MAX_SOURCE_BYTES}-byte limit"
            ),
            Self::InvalidUtf8 => formatter.write_str("source file must contain valid UTF-8"),
        }
    }
}

impl Error for SourceInputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::TooLarge | Self::InvalidUtf8 => None,
        }
    }
}

impl From<io::Error> for SourceInputError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Reads one source while enforcing the limit before UTF-8 conversion.
///
/// Metadata is used only for early rejection and an allocation hint. The
/// streaming read independently retains at most the limit plus one sentinel
/// byte, so a growing file cannot bypass the limit.
pub(crate) fn read_source(path: &Path) -> Result<String, SourceInputError> {
    let file = File::open(path)?;
    let metadata_length = file.metadata()?.len();
    let maximum = u64::try_from(MAX_SOURCE_BYTES).unwrap_or(u64::MAX);
    if metadata_length > maximum {
        return Err(SourceInputError::TooLarge);
    }

    let sentinel_limit = maximum.saturating_add(1);
    let capacity = usize::try_from(metadata_length)
        .unwrap_or(MAX_SOURCE_BYTES)
        .min(MAX_SOURCE_BYTES);
    let mut bytes = Vec::with_capacity(capacity);
    file.take(sentinel_limit).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(SourceInputError::TooLarge);
    }

    String::from_utf8(bytes).map_err(|_| SourceInputError::InvalidUtf8)
}

#[cfg(test)]
mod tests {
    use std::{error::Error as _, fs, io::Write};

    use tempfile::NamedTempFile;

    use super::{MAX_SOURCE_BYTES, SourceInputError, read_source};

    #[test]
    fn accepts_exact_limit_and_rejects_one_additional_byte() {
        let mut exact = NamedTempFile::new().expect("temporary file");
        exact
            .write_all(&vec![b' '; MAX_SOURCE_BYTES])
            .expect("write exact fixture");
        assert_eq!(
            read_source(exact.path())
                .expect("exact limit is valid")
                .len(),
            MAX_SOURCE_BYTES
        );

        let mut oversized = NamedTempFile::new().expect("temporary file");
        oversized
            .write_all(&vec![b' '; MAX_SOURCE_BYTES + 1])
            .expect("write oversized fixture");
        assert!(matches!(
            read_source(oversized.path()),
            Err(SourceInputError::TooLarge)
        ));
        assert!(SourceInputError::TooLarge.source().is_none());
    }

    #[test]
    fn rejects_invalid_utf8_without_displaying_bytes() {
        let file = NamedTempFile::new().expect("temporary file");
        fs::write(file.path(), [0xff, b's', b'e', b'c', b'r', b'e', b't']).expect("write fixture");
        let error = read_source(file.path()).expect_err("UTF-8 must be validated");
        assert!(matches!(error, SourceInputError::InvalidUtf8));
        assert_eq!(error.to_string(), "source file must contain valid UTF-8");
        assert!(!error.to_string().contains("secret"));
        assert!(error.source().is_none());
    }

    #[test]
    fn preserves_filesystem_errors_as_sources() {
        let file = NamedTempFile::new().expect("temporary file");
        let path = file.path().to_owned();
        drop(file);
        let error = read_source(&path).expect_err("missing file must fail");
        assert!(matches!(error, SourceInputError::Io(_)));
        assert!(error.source().is_some());
    }
}
