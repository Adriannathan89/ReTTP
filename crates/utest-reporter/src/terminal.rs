//! Deterministic plain-text and ANSI terminal rendering.

use std::fmt::Write as _;

use crate::{ReportBlock, ReportStatus, ReportTest, RunReport};

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_RED: &str = "\x1b[31m";
const ANSI_GRAY: &str = "\x1b[90m";

/// Controls whether terminal status labels contain ANSI color sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// Emit portable plain ASCII.
    #[default]
    Plain,
    /// Color passed labels green, failed/aborted labels red, and skipped labels gray.
    Ansi,
}

/// Stateless renderer for human-readable execution reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalReporter {
    color: ColorMode,
}

impl TerminalReporter {
    /// Creates a terminal reporter with explicit color behavior.
    #[must_use]
    pub const fn new(color: ColorMode) -> Self {
        Self { color }
    }

    /// Returns the configured color behavior.
    #[must_use]
    pub const fn color_mode(self) -> ColorMode {
        self.color
    }

    /// Renders a complete report with one final newline.
    #[must_use]
    pub fn render(self, report: &RunReport) -> String {
        let mut output = String::new();
        let suite_name = report.name.as_deref().unwrap_or("unnamed suite");
        let _ = writeln!(output, "UTest: {:?}", report.source);
        let _ = writeln!(
            output,
            "{} {:?} ({} ms)",
            self.status(report.status),
            suite_name,
            report.duration_ms
        );

        for block in &report.blocks {
            self.render_block(&mut output, block);
        }

        let summary = report.summary;
        let _ = writeln!(
            output,
            "Summary: {} total, {} passed, {} failed, {} skipped, {} aborted",
            summary.total, summary.passed, summary.failed, summary.skipped, summary.aborted
        );
        if let Some(error) = &report.error {
            let _ = writeln!(
                output,
                "Suite error [{}]: {}",
                error.kind.as_str(),
                escape_control_characters(&error.message)
            );
        }
        output
    }

    fn render_block(self, output: &mut String, block: &ReportBlock) {
        let name = match block.name.as_deref() {
            Some(name) => format!("{} {name:?}", block.kind.as_str()),
            None => block.kind.as_str().to_owned(),
        };
        let _ = writeln!(
            output,
            "  {} {} ({} tests, {} ms)",
            self.status(block.status),
            name,
            block.tests.len(),
            block.duration_ms
        );
        for test in &block.tests {
            self.render_test(output, test);
        }
    }

    fn render_test(self, output: &mut String, test: &ReportTest) {
        let _ = writeln!(
            output,
            "    {} {:?} ({} ms)",
            self.status(test.status),
            test.name,
            test.duration_ms
        );
        for failure in &test.failures {
            let _ = writeln!(
                output,
                "      - {} [{}]: {}",
                escape_control_characters(&failure.path),
                failure.kind.as_str(),
                escape_control_characters(&failure.message)
            );
            if let Some(expected) = &failure.expected {
                let _ = writeln!(
                    output,
                    "        expected: {}",
                    escape_control_characters(expected)
                );
            }
            if let Some(actual) = &failure.actual {
                let _ = writeln!(
                    output,
                    "        actual: {}",
                    escape_control_characters(actual)
                );
            }
        }
        if let Some(error) = &test.error {
            let _ = writeln!(
                output,
                "      error [{}]: {}",
                error.kind.as_str(),
                escape_control_characters(&error.message)
            );
        }
    }

    fn status(self, status: ReportStatus) -> String {
        let label = format!("[{}]", status.label());
        if self.color == ColorMode::Plain {
            return label;
        }
        let color = match status {
            ReportStatus::Passed => ANSI_GREEN,
            ReportStatus::Failed | ReportStatus::Aborted => ANSI_RED,
            ReportStatus::Skipped => ANSI_GRAY,
        };
        format!("{color}{label}{ANSI_RESET}")
    }
}

fn escape_control_characters(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_control() {
            escaped.extend(character.escape_default());
        } else {
            escaped.push(character);
        }
    }
    escaped
}

impl Default for TerminalReporter {
    fn default() -> Self {
        Self::new(ColorMode::Plain)
    }
}
