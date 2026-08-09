//! Command-line interface for UTest.
//!
//! The `check` command validates a `.utest` source file without making HTTP
//! requests. It reads the file, obtains predefined variable names from the
//! process environment and repeated `--var NAME=VALUE` arguments, and delegates
//! the compiler pipeline to [`utest_application::check_source`].
//!
//! # Exit codes
//!
//! - `0`: the source is valid;
//! - `1`: the source file could not be read;
//! - `2`: command-line usage is invalid (reported by Clap);
//! - `3`: lexical, syntax, or semantic checking failed.
//!
//! # Examples
//!
//! ```text
//! utest check examples/basic.utest
//! utest check examples/interpolation.utest \
//!     --var id=42 \
//!     --var interpolated_string=value
//! ```

use std::{fs, path::PathBuf, process::ExitCode};

use clap::{Parser as ClapParser, Subcommand};
use utest_application::{CheckDiagnostic, check_source};
use utest_domain::VariableName;
use utest_parser::{SourceText, ValidationContext};

/// Exit status used when source validation reports diagnostics.
const CHECK_FAILED_EXIT_CODE: u8 = 3;

/// Parsed top-level command-line arguments.
#[derive(Debug, ClapParser)]
#[command(name = "utest", version, about = "HTTP verification suite runner")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Commands exposed by the `utest` executable.
#[derive(Debug, Subcommand)]
enum Command {
    /// Checks a UTest file without executing HTTP requests.
    Check {
        /// Path to the `.utest` source file.
        path: PathBuf,

        /// Defines a variable for semantic checking (`NAME=VALUE`).
        #[arg(long = "var", value_name = "NAME=VALUE", value_parser = parse_variable)]
        variables: Vec<VariableName>,
    },
}

/// Parses process arguments, dispatches the selected command, and returns its
/// process exit status.
fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Check { path, variables } => check(path, variables),
    }
}

/// Checks one file using CLI and environment variables as predefined names.
///
/// Variable values are intentionally not interpreted at this stage because
/// source checking only needs to establish whether referenced names exist.
fn check(path: PathBuf, variables: Vec<VariableName>) -> ExitCode {
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) => {
            eprintln!("{}: error[io]: {error}", path.display());
            return ExitCode::FAILURE;
        }
    };

    let source = SourceText::new(path.display().to_string(), content);
    let environment_variables = std::env::vars_os()
        .filter_map(|(name, _)| name.into_string().ok())
        .filter_map(|name| VariableName::new(name).ok());
    let context = ValidationContext::new()
        .with_predefined_variables(environment_variables)
        .with_predefined_variables(variables);
    let report = check_source(&source, &context);

    if report.is_success() {
        println!("{}: valid", source.name());
        return ExitCode::SUCCESS;
    }

    for diagnostic in &report.diagnostics {
        render_diagnostic(&source, diagnostic);
    }
    ExitCode::from(CHECK_FAILED_EXIT_CODE)
}

/// Writes one compiler-style diagnostic to standard error.
///
/// Locations use one-based line and column numbers calculated from the
/// diagnostic's byte span.
fn render_diagnostic(source: &SourceText, diagnostic: &CheckDiagnostic) {
    let location = source.location(diagnostic.span.start);
    eprintln!(
        "{}:{}:{}: error[{}]: {}",
        source.name(),
        location.line,
        location.column,
        diagnostic.phase().as_str(),
        diagnostic.kind,
    );
}

/// Validates the `NAME=VALUE` form accepted by `--var`.
///
/// The name follows [`VariableName`] rules. The value may be empty and is
/// ignored until runtime interpolation is implemented.
fn parse_variable(raw: &str) -> Result<VariableName, String> {
    let Some((name, _value)) = raw.split_once('=') else {
        return Err("expected NAME=VALUE".to_owned());
    };
    VariableName::new(name).map_err(|error| error.to_string())
}
