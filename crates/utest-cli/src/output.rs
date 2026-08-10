//! Process output and individually atomic report-file writes.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use tempfile::NamedTempFile;

/// Writes UTF-8 text to stdout and flushes it before returning.
pub(crate) fn stdout(content: &str) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(content.as_bytes())?;
    stdout.flush()
}

/// Writes a complete artifact through a temporary file in its destination.
///
/// The rename is atomic for readers on the same filesystem. Parent directories
/// are created when absent, and an existing destination is replaced.
pub(crate) fn atomic_file(path: &Path, content: &str) -> io::Result<()> {
    let parent = usable_parent(path);
    fs::create_dir_all(&parent)?;
    let mut temporary = NamedTempFile::new_in(&parent)?;
    temporary.write_all(content.as_bytes())?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn usable_parent(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_owned()
}
