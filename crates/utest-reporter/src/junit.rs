//! Escaped JUnit XML rendering for CI systems.

use std::fmt::Write as _;

use crate::{ReportBlock, ReportStatus, ReportTest, RunReport};

/// Stateless renderer for JUnit-compatible XML reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JunitReporter;

impl JunitReporter {
    /// Renders a `<testsuites>` document with one final newline.
    ///
    /// Every source block becomes one ordered `<testsuite>`. A suite-level
    /// error becomes an additional synthetic suite so CI cannot treat an
    /// executor invariant failure as success.
    #[must_use]
    pub fn render(self, report: &RunReport) -> String {
        let synthetic = u64::from(report.error.is_some());
        let counts = report_counts(report);
        let tests = counts.tests.saturating_add(synthetic);
        let failures = counts.failures;
        let errors = counts.errors.saturating_add(synthetic);
        let skipped = counts.skipped;
        let mut output = String::new();
        let _ = writeln!(output, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
        let _ = writeln!(
            output,
            "<testsuites name=\"{}\" tests=\"{tests}\" failures=\"{failures}\" errors=\"{errors}\" skipped=\"{skipped}\" time=\"{}\">",
            escape_attribute(report.name.as_deref().unwrap_or(&report.source)),
            seconds(report.duration_ms)
        );
        for block in &report.blocks {
            render_block(&mut output, block);
        }
        if let Some(error) = &report.error {
            let _ = writeln!(
                output,
                "  <testsuite name=\"utest\" tests=\"1\" failures=\"0\" errors=\"1\" skipped=\"0\" time=\"0.000\">"
            );
            let _ = writeln!(output, "    <testcase name=\"suite\" time=\"0.000\">");
            let _ = writeln!(
                output,
                "      <error type=\"{}\" message=\"{}\" />",
                error.kind.as_str(),
                escape_attribute(&error.message)
            );
            let _ = writeln!(output, "    </testcase>");
            let _ = writeln!(output, "  </testsuite>");
        }
        let _ = writeln!(output, "</testsuites>");
        output
    }
}

fn render_block(output: &mut String, block: &ReportBlock) {
    let suite_name = match block.kind {
        crate::ReportBlockKind::Core => "core".to_owned(),
        crate::ReportBlockKind::Pipeline => {
            format!("pipeline:{}", block.name.as_deref().unwrap_or("unnamed"))
        }
        crate::ReportBlockKind::Test => {
            format!("test:{}", block.name.as_deref().unwrap_or("unnamed"))
        }
    };
    let counts = block_counts(block);
    let _ = writeln!(
        output,
        "  <testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" errors=\"{}\" skipped=\"{}\" time=\"{}\">",
        escape_attribute(&suite_name),
        counts.tests,
        counts.failures,
        counts.errors,
        counts.skipped,
        seconds(block.duration_ms)
    );
    for test in &block.tests {
        render_test(output, &suite_name, test);
    }
    let _ = writeln!(output, "  </testsuite>");
}

fn render_test(output: &mut String, class_name: &str, test: &ReportTest) {
    let _ = writeln!(
        output,
        "    <testcase classname=\"{}\" name=\"{}\" time=\"{}\">",
        escape_attribute(class_name),
        escape_attribute(&test.name),
        seconds(test.duration_ms)
    );
    match test.status {
        ReportStatus::Passed => {}
        ReportStatus::Skipped => {
            let message = test
                .error
                .as_ref()
                .map_or("dependency failed", |error| error.message.as_str());
            let _ = writeln!(
                output,
                "      <skipped message=\"{}\" />",
                escape_attribute(message)
            );
        }
        ReportStatus::Failed if test.error.is_none() && !test.failures.is_empty() => {
            let _ = writeln!(
                output,
                "      <failure type=\"assertion\" message=\"{} assertion failure(s)\">",
                test.failures.len()
            );
            for failure in &test.failures {
                let _ = writeln!(
                    output,
                    "{} [{}]: {}",
                    escape_text(&failure.path),
                    failure.kind.as_str(),
                    escape_text(&failure.message)
                );
                if let Some(expected) = &failure.expected {
                    let _ = writeln!(output, "expected: {}", escape_text(expected));
                }
                if let Some(actual) = &failure.actual {
                    let _ = writeln!(output, "actual: {}", escape_text(actual));
                }
            }
            let _ = writeln!(output, "      </failure>");
        }
        ReportStatus::Failed | ReportStatus::Aborted => {
            let (kind, message) = test
                .error
                .as_ref()
                .map_or(("internal", "test execution failed"), |error| {
                    (error.kind.as_str(), error.message.as_str())
                });
            let _ = writeln!(
                output,
                "      <error type=\"{}\" message=\"{}\" />",
                escape_attribute(kind),
                escape_attribute(message)
            );
        }
    }
    let _ = writeln!(output, "    </testcase>");
}

#[derive(Default)]
struct Counts {
    tests: u64,
    failures: u64,
    errors: u64,
    skipped: u64,
}

fn block_counts(block: &ReportBlock) -> Counts {
    let mut counts = Counts::default();
    for test in &block.tests {
        counts.tests = counts.tests.saturating_add(1);
        match test.status {
            ReportStatus::Passed => {}
            ReportStatus::Skipped => counts.skipped = counts.skipped.saturating_add(1),
            ReportStatus::Failed if test.error.is_none() && !test.failures.is_empty() => {
                counts.failures = counts.failures.saturating_add(1);
            }
            ReportStatus::Failed | ReportStatus::Aborted => {
                counts.errors = counts.errors.saturating_add(1);
            }
        }
    }
    counts
}

fn report_counts(report: &RunReport) -> Counts {
    let mut total = Counts::default();
    for counts in report.blocks.iter().map(block_counts) {
        total.tests = total.tests.saturating_add(counts.tests);
        total.failures = total.failures.saturating_add(counts.failures);
        total.errors = total.errors.saturating_add(counts.errors);
        total.skipped = total.skipped.saturating_add(counts.skipped);
    }
    total
}

fn seconds(duration_ms: u64) -> String {
    format!("{}.{:03}", duration_ms / 1_000, duration_ms % 1_000)
}

fn escape_attribute(value: &str) -> String {
    escape(value, true)
}

fn escape_text(value: &str) -> String {
    escape(value, false)
}

fn escape(value: &str, attribute: bool) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' if attribute => escaped.push_str("&quot;"),
            '\'' if attribute => escaped.push_str("&apos;"),
            character if is_xml_character(character) => escaped.push(character),
            _ => escaped.push('\u{fffd}'),
        }
    }
    escaped
}

fn is_xml_character(character: char) -> bool {
    matches!(character, '\u{9}' | '\u{a}' | '\u{d}')
        || matches!(character as u32, 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x10ffff)
}
