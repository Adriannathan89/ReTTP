//! Pretty JSON rendering for the stable report schema.

use thiserror::Error;

use crate::RunReport;

/// Failure while serializing a JSON report.
#[derive(Debug, Error)]
#[error("failed to serialize JSON report: {source}")]
pub struct JsonReportError {
    source: serde_json::Error,
}

/// Stateless renderer for versioned JSON reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JsonReporter;

impl JsonReporter {
    /// Serializes `report` as pretty UTF-8 JSON with one final newline.
    ///
    /// # Errors
    ///
    /// Returns [`JsonReportError`] if the stable report cannot be serialized.
    pub fn render(self, report: &RunReport) -> Result<String, JsonReportError> {
        let mut output =
            serde_json::to_string_pretty(report).map_err(|source| JsonReportError { source })?;
        output.push('\n');
        Ok(output)
    }
}
