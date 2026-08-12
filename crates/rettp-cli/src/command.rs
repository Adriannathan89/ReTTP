//! Command orchestration, mandatory checking, execution, and exit-code mapping.

use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
    process::ExitCode,
};

use rettp_application::{ExecutionEngine, check_source};
use rettp_domain::{BlockResult, ExecutionStatus, SuiteResult};
use rettp_http::{HttpClientConfig, ReqwestHttpClient};
use rettp_parser::{SourceText, ValidationContext};
use rettp_reporter::{ColorMode, JsonReporter, JunitReporter, RunReport, TerminalReporter};
use rettp_runtime::{VariableAssignment, VariableStore};

use crate::{
    args::{CheckArgs, Cli, Command, RunArgs, SourceArgs},
    diagnostic, env_file, input,
    interrupt::{self, InterruptOutcome},
    output,
};

const TEST_FAILED: u8 = 1;
const CORE_ABORTED: u8 = 2;
const CHECK_FAILED: u8 = 3;
const INVALID_INPUT: u8 = 4;
const INTERNAL_ERROR: u8 = 5;
const INTERRUPTED: u8 = 130;

/// Dispatches one parsed command without allowing lower layers to exit.
pub(crate) fn dispatch(cli: Cli) -> ExitCode {
    match cli.command {
        Command::Check(arguments) => check(arguments),
        Command::Run(arguments) => run(arguments),
    }
}

fn check(arguments: CheckArgs) -> ExitCode {
    let (source, variables) = match prepare_source(arguments.source) {
        Ok(prepared) => prepared,
        Err(error) => return emit_error(INVALID_INPUT, "input", &error),
    };
    let context = validation_context(&variables);
    let checked = check_source(&source, &context);
    if checked.has_errors() {
        return emit_diagnostics(&source, &checked.diagnostics);
    }
    let line = format!("{}: valid\n", source.name());
    match output::stdout(&line) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => emit_error(INTERNAL_ERROR, "output", &error.to_string()),
    }
}

fn run(arguments: RunArgs) -> ExitCode {
    if arguments.json_file.is_some()
        && arguments.json_file.as_ref() == arguments.junit_file.as_ref()
    {
        return emit_error(
            INVALID_INPUT,
            "configuration",
            "--json-file and --junit-file must use different paths",
        );
    }

    let (source, variables) = match prepare_source(arguments.source) {
        Ok(prepared) => prepared,
        Err(error) => return emit_error(INVALID_INPUT, "input", &error),
    };
    let checked = check_source(&source, &validation_context(&variables));
    if checked.has_errors() {
        return emit_diagnostics(&source, &checked.diagnostics);
    }
    let Some(suite) = checked.suite else {
        return emit_error(
            INTERNAL_ERROR,
            "runner",
            "successful checking did not produce an executable suite",
        );
    };

    let mut client_config = match HttpClientConfig::new(&arguments.base_url) {
        Ok(config) => config,
        Err(error) => return emit_error(INVALID_INPUT, "configuration", &error.to_string()),
    };
    if let Some(timeout) = arguments.timeout {
        client_config = match client_config.with_default_timeout(timeout) {
            Ok(config) => config,
            Err(error) => return emit_error(INVALID_INPUT, "configuration", &error.to_string()),
        };
    }
    let client = match ReqwestHttpClient::new(client_config) {
        Ok(client) => client,
        Err(error) => return emit_error(INTERNAL_ERROR, "runner", &error.to_string()),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => return emit_error(INTERNAL_ERROR, "runner", &error.to_string()),
    };
    let execution = runtime.block_on(interrupt::run_until_ctrl_c(
        ExecutionEngine::default().execute(&suite, &variables, &client),
    ));
    let result = match execution {
        Ok(InterruptOutcome::Completed(result)) => result,
        Ok(InterruptOutcome::Interrupted) => {
            return emit_error(
                INTERRUPTED,
                "interrupted",
                "execution interrupted by Ctrl+C",
            );
        }
        Err(error) => return emit_error(INTERNAL_ERROR, "runner", &error.to_string()),
    };
    emit_run_report(&source, &result, arguments.json_file, arguments.junit_file)
}

fn prepare_source(arguments: SourceArgs) -> Result<(SourceText, VariableStore), String> {
    let content = input::read_source(&arguments.path)
        .map_err(|error| format!("{}: {error}", arguments.path.display()))?;
    let source = SourceText::new(arguments.path.display().to_string(), content);
    let mut variables = VariableStore::from_environment();
    if let Some(path) = arguments.env_file {
        let assignments =
            env_file::load(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        variables.apply_cli(assignments);
    }
    for (index, raw) in arguments.variables.into_iter().enumerate() {
        let assignment = raw.parse::<VariableAssignment>().map_err(|error| {
            format!(
                "invalid --var assignment {}: {error}",
                index.saturating_add(1)
            )
        })?;
        variables.apply_cli([assignment]);
    }
    Ok((source, variables))
}

fn validation_context(variables: &VariableStore) -> ValidationContext {
    ValidationContext::new().with_predefined_variables(variables.names().cloned())
}

fn emit_diagnostics(
    source: &SourceText,
    diagnostics: &[rettp_application::CheckDiagnostic],
) -> ExitCode {
    for diagnostic in diagnostics {
        if diagnostic::render(source, diagnostic).is_err() {
            return ExitCode::from(INTERNAL_ERROR);
        }
    }
    ExitCode::from(CHECK_FAILED)
}

fn emit_run_report(
    source: &SourceText,
    result: &SuiteResult,
    json_file: Option<PathBuf>,
    junit_file: Option<PathBuf>,
) -> ExitCode {
    let report = RunReport::from_suite_result(source.name(), result);
    let color = if io::stdout().is_terminal() {
        ColorMode::Ansi
    } else {
        ColorMode::Plain
    };
    let terminal = TerminalReporter::new(color).render(&report);
    let json = match json_file.as_ref() {
        Some(_) => match JsonReporter.render(&report) {
            Ok(json) => Some(json),
            Err(error) => return emit_error(INTERNAL_ERROR, "report", &error.to_string()),
        },
        None => None,
    };
    let junit = junit_file.as_ref().map(|_| JunitReporter.render(&report));

    if let Err(error) = output::stdout(&terminal) {
        return emit_error(INTERNAL_ERROR, "output", &error.to_string());
    }
    if let Some((path, json)) = json_file.as_deref().zip(json.as_deref())
        && let Err(error) = output::atomic_file(path, json)
    {
        return emit_error(INTERNAL_ERROR, "report", &error.to_string());
    }
    if let Some((path, junit)) = junit_file.as_deref().zip(junit.as_deref())
        && let Err(error) = output::atomic_file(path, junit)
    {
        return emit_error(INTERNAL_ERROR, "report", &error.to_string());
    }
    execution_exit_code(result)
}

fn execution_exit_code(result: &SuiteResult) -> ExitCode {
    match result.status {
        ExecutionStatus::Passed => ExitCode::SUCCESS,
        ExecutionStatus::Failed => ExitCode::from(TEST_FAILED),
        ExecutionStatus::Aborted if core_failed(result) => ExitCode::from(CORE_ABORTED),
        ExecutionStatus::Aborted | ExecutionStatus::Skipped => ExitCode::from(INTERNAL_ERROR),
    }
}

fn core_failed(result: &SuiteResult) -> bool {
    result.blocks.iter().any(|block| {
        matches!(
            block,
            BlockResult::Core(core) if core.status == ExecutionStatus::Failed
                || core.status == ExecutionStatus::Aborted
        )
    })
}

fn emit_error(code: u8, category: &str, message: &str) -> ExitCode {
    let _ = writeln!(io::stderr().lock(), "error[{category}]: {message}");
    ExitCode::from(code)
}
