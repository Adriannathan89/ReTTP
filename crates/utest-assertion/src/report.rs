//! Result of evaluating one resolved response expectation.

use utest_domain::AssertionFailure;

/// Deterministic collection of response assertion failures.
///
/// A report succeeds only when it contains no failures. `truncated` indicates
/// that evaluation found more failures than its configured retention limit and
/// stopped without retaining the remainder.
#[derive(Debug, Clone, PartialEq)]
pub struct AssertionReport {
    failures: Vec<AssertionFailure>,
    truncated: bool,
}

impl AssertionReport {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the response evaluator is introduced in the next implementation batch"
        )
    )]
    pub(crate) fn new(failures: Vec<AssertionFailure>, truncated: bool) -> Self {
        Self {
            failures,
            truncated,
        }
    }

    /// Returns whether every evaluated assertion passed.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.failures.is_empty()
    }

    /// Returns the retained failures in deterministic evaluation order.
    #[must_use]
    pub fn failures(&self) -> &[AssertionFailure] {
        &self.failures
    }

    /// Returns the number of retained failures.
    #[must_use]
    pub fn len(&self) -> usize {
        self.failures.len()
    }

    /// Returns whether no failures were retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.failures.is_empty()
    }

    /// Returns whether failures beyond the configured limit were omitted.
    #[must_use]
    pub const fn is_truncated(&self) -> bool {
        self.truncated
    }

    /// Consumes the report and returns its retained failures.
    #[must_use]
    pub fn into_failures(self) -> Vec<AssertionFailure> {
        self.failures
    }
}

#[cfg(test)]
mod tests {
    use utest_domain::{AssertionFailure, AssertionFailureKind};

    use super::AssertionReport;

    fn failure(path: &str) -> AssertionFailure {
        AssertionFailure {
            path: path.to_owned(),
            kind: AssertionFailureKind::MissingField,
            expected: Some("present".to_owned()),
            actual: Some("missing".to_owned()),
            message: "required field is missing".to_owned(),
        }
    }

    #[test]
    fn empty_report_is_successful_and_not_truncated() {
        let report = AssertionReport::new(Vec::new(), false);

        assert!(report.is_success());
        assert!(report.is_empty());
        assert_eq!(report.len(), 0);
        assert!(report.failures().is_empty());
        assert!(!report.is_truncated());
        assert!(report.into_failures().is_empty());
    }

    #[test]
    fn failed_report_preserves_order_and_truncation_state() {
        let failures = vec![failure("$.first"), failure("$.second")];
        let report = AssertionReport::new(failures.clone(), true);

        assert!(!report.is_success());
        assert!(!report.is_empty());
        assert_eq!(report.len(), 2);
        assert_eq!(report.failures(), failures.as_slice());
        assert!(report.is_truncated());
        assert_eq!(report.into_failures(), failures);
    }
}
