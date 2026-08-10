//! Sequential execution of validated suites through backend-neutral ports.
//!
//! [`ExecutionEngine`] runs the unique optional core first, then pipelines and
//! standalone tests in source order. Result blocks are always returned in
//! source order even when core was declared later in the file.

use std::time::Instant;

use utest_assertion::AssertionEngine;
use utest_domain::{
    BlockResult, CoreBlock, CoreResult, ExecutionErrorInfo, ExecutionErrorKind, ExecutionStatus,
    PipelineBlock, PipelineResult, SuiteBlock, SuiteResult, TestCase, TestResult, TestSuite,
};
use utest_http::{HttpClient, HttpError};
use utest_runtime::{CaptureEngine, RuntimeError, RuntimeResolver, VariableStore};

const CORE_FAILED_REASON: &str = "core dependency failed";
const PIPELINE_FAILED_REASON: &str = "an earlier pipeline test failed";

/// Coordinates variable resolution, HTTP execution, assertions, and captures.
///
/// The engine is immutable and contains only inexpensive configured service
/// values. One instance can therefore be reused across suites and with
/// different [`HttpClient`] implementations. Suite execution itself is
/// sequential in the MVP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExecutionEngine {
    resolver: RuntimeResolver,
    assertions: AssertionEngine,
    captures: CaptureEngine,
}

impl ExecutionEngine {
    /// Creates an engine from validated runtime and assertion configurations.
    #[must_use]
    pub const fn new(resolver: RuntimeResolver, assertions: AssertionEngine) -> Self {
        Self {
            resolver,
            assertions,
            captures: CaptureEngine,
        }
    }

    /// Returns the runtime resolver used for requests and expectations.
    #[must_use]
    pub const fn resolver(self) -> RuntimeResolver {
        self.resolver
    }

    /// Returns the assertion engine used for response evaluation.
    #[must_use]
    pub const fn assertions(self) -> AssertionEngine {
        self.assertions
    }

    /// Executes a validated suite and returns source-ordered results.
    ///
    /// `initial_variables` is cloned and never mutated. Core captures become
    /// visible to every later block, pipeline captures remain in that pipeline,
    /// and standalone captures are discarded after their test.
    ///
    /// The method performs a complete shape preflight before calling `client`.
    /// A programmatically constructed empty suite, empty pipeline, or suite
    /// with multiple core blocks returns an aborted result without network
    /// access. Source callers should still use [`crate::check_source`] before
    /// execution.
    pub async fn execute(
        &self,
        suite: &TestSuite,
        initial_variables: &VariableStore,
        client: &dyn HttpClient,
    ) -> SuiteResult {
        let suite_started = Instant::now();
        if let Err(reason) = preflight(suite) {
            return invalid_suite_result(suite, elapsed_ms(suite_started), reason);
        }

        let core_index = suite
            .blocks
            .iter()
            .position(|block| matches!(block, SuiteBlock::Core(_)));
        let mut results: Vec<Option<BlockResult>> = std::iter::repeat_with(|| None)
            .take(suite.blocks.len())
            .collect();
        let mut post_core_variables = initial_variables.clone();

        if let Some(index) = core_index {
            let SuiteBlock::Core(core) = &suite.blocks[index] else {
                unreachable!("the located core index must contain core");
            };
            let core_result = self
                .execute_core(core, &mut post_core_variables, client)
                .await;
            let core_passed = core_result.status == ExecutionStatus::Passed;
            results[index] = Some(BlockResult::Core(core_result));

            if !core_passed {
                for (result, block) in results.iter_mut().zip(&suite.blocks) {
                    if result.is_none() {
                        *result = Some(skipped_block(block, CORE_FAILED_REASON));
                    }
                }
                return SuiteResult {
                    name: suite.name.clone(),
                    status: ExecutionStatus::Aborted,
                    duration_ms: elapsed_ms(suite_started),
                    blocks: collect_results(results),
                    error: Some(ExecutionErrorInfo {
                        kind: ExecutionErrorKind::DependencyFailed,
                        message: CORE_FAILED_REASON.to_owned(),
                    }),
                };
            }
        }

        let mut suite_failed = false;
        for (index, block) in suite.blocks.iter().enumerate() {
            if Some(index) == core_index {
                continue;
            }
            let result = match block {
                SuiteBlock::Core(_) => unreachable!("preflight permits at most one core"),
                SuiteBlock::Pipeline(pipeline) => {
                    let mut variables = post_core_variables.clone();
                    let result = self
                        .execute_pipeline(pipeline, &mut variables, client)
                        .await;
                    if result.status != ExecutionStatus::Passed {
                        suite_failed = true;
                    }
                    BlockResult::Pipeline(result)
                }
                SuiteBlock::Test(test) => {
                    let mut variables = post_core_variables.clone();
                    let result = self.execute_test(test, &mut variables, client).await;
                    if result.status != ExecutionStatus::Passed {
                        suite_failed = true;
                    }
                    BlockResult::Test(result)
                }
            };
            results[index] = Some(result);
        }

        SuiteResult {
            name: suite.name.clone(),
            status: if suite_failed {
                ExecutionStatus::Failed
            } else {
                ExecutionStatus::Passed
            },
            duration_ms: elapsed_ms(suite_started),
            blocks: collect_results(results),
            error: None,
        }
    }

    async fn execute_core(
        &self,
        core: &CoreBlock,
        variables: &mut VariableStore,
        client: &dyn HttpClient,
    ) -> CoreResult {
        let started = Instant::now();
        let mut tests = Vec::with_capacity(core.tests.len());
        let mut status = ExecutionStatus::Passed;

        for (index, test) in core.tests.iter().enumerate() {
            let result = self.execute_test(test, variables, client).await;
            let result_status = result.status;
            tests.push(result);
            if result_status != ExecutionStatus::Passed {
                status = if result_status == ExecutionStatus::Aborted {
                    ExecutionStatus::Aborted
                } else {
                    ExecutionStatus::Failed
                };
                tests.extend(
                    core.tests[index + 1..]
                        .iter()
                        .map(|test| TestResult::skipped(&test.name, CORE_FAILED_REASON)),
                );
                break;
            }
        }

        CoreResult {
            status,
            duration_ms: elapsed_ms(started),
            tests,
        }
    }

    async fn execute_pipeline(
        &self,
        pipeline: &PipelineBlock,
        variables: &mut VariableStore,
        client: &dyn HttpClient,
    ) -> PipelineResult {
        let started = Instant::now();
        let mut tests = Vec::with_capacity(pipeline.tests.len());
        let mut status = ExecutionStatus::Passed;

        for (index, test) in pipeline.tests.iter().enumerate() {
            let result = self.execute_test(test, variables, client).await;
            let failed = result.status != ExecutionStatus::Passed;
            tests.push(result);
            if failed {
                status = ExecutionStatus::Failed;
                tests.extend(
                    pipeline.tests[index + 1..]
                        .iter()
                        .map(|test| TestResult::skipped(&test.name, PIPELINE_FAILED_REASON)),
                );
                break;
            }
        }

        PipelineResult {
            name: pipeline.name.clone(),
            status,
            duration_ms: elapsed_ms(started),
            tests,
        }
    }

    async fn execute_test(
        &self,
        test: &TestCase,
        variables: &mut VariableStore,
        client: &dyn HttpClient,
    ) -> TestResult {
        let started = Instant::now();
        let request = match self.resolver.resolve_request(&test.request, variables) {
            Ok(request) => request,
            Err(error) => {
                return failed_with_error(
                    test,
                    elapsed_ms(started),
                    ExecutionErrorKind::VariableResolution,
                    error,
                );
            }
        };
        let expectation = match self
            .resolver
            .resolve_expectation(&test.expectation, variables)
        {
            Ok(expectation) => expectation,
            Err(error) => {
                return failed_with_error(
                    test,
                    elapsed_ms(started),
                    ExecutionErrorKind::VariableResolution,
                    error,
                );
            }
        };
        let response = match client.execute(&request).await {
            Ok(response) => response,
            Err(error) => {
                let error = http_error_info(error);
                return TestResult::failed(
                    &test.name,
                    elapsed_ms(started),
                    Vec::new(),
                    Some(error),
                );
            }
        };
        let evaluation = match self
            .captures
            .evaluate(&self.assertions, &expectation, &response)
        {
            Ok(evaluation) => evaluation,
            Err(error) => return aborted_capture(test, elapsed_ms(started), error),
        };
        let (report, pending) = evaluation.into_parts();
        if !report.is_success() {
            return TestResult::failed(
                &test.name,
                elapsed_ms(started),
                report.into_failures(),
                None,
            );
        }
        let Some(pending) = pending else {
            return TestResult::aborted(
                &test.name,
                elapsed_ms(started),
                "successful assertion evaluation did not stage a capture transaction",
            );
        };
        if let Err(error) = variables.commit(pending) {
            return aborted_capture(test, elapsed_ms(started), error);
        }

        TestResult::passed(&test.name, elapsed_ms(started))
    }
}

fn preflight(suite: &TestSuite) -> Result<(), &'static str> {
    if suite.blocks.is_empty() {
        return Err("suite must contain at least one block");
    }
    if suite
        .blocks
        .iter()
        .filter(|block| matches!(block, SuiteBlock::Core(_)))
        .count()
        > 1
    {
        return Err("suite must contain at most one core block");
    }
    if suite
        .blocks
        .iter()
        .any(|block| matches!(block, SuiteBlock::Pipeline(pipeline) if pipeline.tests.is_empty()))
    {
        return Err("pipeline must contain at least one test");
    }
    Ok(())
}

fn invalid_suite_result(suite: &TestSuite, duration_ms: u64, reason: &str) -> SuiteResult {
    SuiteResult {
        name: suite.name.clone(),
        status: ExecutionStatus::Aborted,
        duration_ms,
        blocks: suite
            .blocks
            .iter()
            .map(|block| skipped_block(block, reason))
            .collect(),
        error: Some(ExecutionErrorInfo {
            kind: ExecutionErrorKind::Internal,
            message: reason.to_owned(),
        }),
    }
}

fn skipped_block(block: &SuiteBlock, reason: &str) -> BlockResult {
    match block {
        SuiteBlock::Core(core) => BlockResult::Core(CoreResult {
            status: ExecutionStatus::Skipped,
            duration_ms: 0,
            tests: core
                .tests
                .iter()
                .map(|test| TestResult::skipped(&test.name, reason))
                .collect(),
        }),
        SuiteBlock::Pipeline(pipeline) => BlockResult::Pipeline(PipelineResult {
            name: pipeline.name.clone(),
            status: ExecutionStatus::Skipped,
            duration_ms: 0,
            tests: pipeline
                .tests
                .iter()
                .map(|test| TestResult::skipped(&test.name, reason))
                .collect(),
        }),
        SuiteBlock::Test(test) => BlockResult::Test(TestResult::skipped(&test.name, reason)),
    }
}

fn collect_results(results: Vec<Option<BlockResult>>) -> Vec<BlockResult> {
    results
        .into_iter()
        .map(|result| result.expect("every preflighted suite block must receive a result"))
        .collect()
}

fn failed_with_error(
    test: &TestCase,
    duration_ms: u64,
    kind: ExecutionErrorKind,
    error: RuntimeError,
) -> TestResult {
    TestResult::failed(
        &test.name,
        duration_ms,
        Vec::new(),
        Some(ExecutionErrorInfo {
            kind,
            message: error.to_string(),
        }),
    )
}

fn aborted_capture(test: &TestCase, duration_ms: u64, error: RuntimeError) -> TestResult {
    TestResult::aborted(&test.name, duration_ms, error.to_string())
}

fn http_error_info(error: HttpError) -> ExecutionErrorInfo {
    let kind = match &error {
        HttpError::InvalidBaseUrl { .. } => ExecutionErrorKind::Internal,
        HttpError::InvalidRequest { .. } => ExecutionErrorKind::InvalidRequest,
        HttpError::Connection { .. } => ExecutionErrorKind::Connection,
        HttpError::Timeout { .. } => ExecutionErrorKind::Timeout,
        HttpError::InvalidResponse { .. } | HttpError::BodyTooLarge { .. } => {
            ExecutionErrorKind::InvalidResponse
        }
        _ => ExecutionErrorKind::Internal,
    };
    ExecutionErrorInfo {
        kind,
        message: error.to_string(),
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}
