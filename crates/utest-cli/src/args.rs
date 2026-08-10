//! Clap argument model and value-free duration validation.

use std::{path::PathBuf, time::Duration};

use clap::{Args, Parser, Subcommand};

/// Parsed top-level command-line arguments.
#[derive(Parser)]
#[command(name = "utest", version, about = "HTTP verification suite runner")]
pub(crate) struct Cli {
    /// Selected UTest operation.
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// Commands exposed by the `utest` executable.
#[derive(Subcommand)]
pub(crate) enum Command {
    /// Checks a UTest source file without executing HTTP requests.
    Check(CheckArgs),
    /// Checks and executes one UTest source file.
    Run(RunArgs),
}

/// Arguments shared by source checking and execution.
#[derive(Args)]
pub(crate) struct SourceArgs {
    /// Path to one UTF-8 `.utest` source file.
    pub(crate) path: PathBuf,

    /// Loads predefined variables from a dotenv-compatible UTF-8 file.
    #[arg(long, value_name = "FILE")]
    pub(crate) env_file: Option<PathBuf>,

    /// Defines a predefined variable; later occurrences win.
    #[arg(long = "var", value_name = "NAME=VALUE")]
    pub(crate) variables: Vec<String>,
}

/// Arguments accepted by `utest check`.
#[derive(Args)]
pub(crate) struct CheckArgs {
    /// Source and predefined-variable options.
    #[command(flatten)]
    pub(crate) source: SourceArgs,
}

/// Arguments accepted by `utest run`.
#[derive(Args)]
pub(crate) struct RunArgs {
    /// Source and predefined-variable options.
    #[command(flatten)]
    pub(crate) source: SourceArgs,

    /// Required HTTP(S) base URL used to resolve every relative request path.
    #[arg(long, value_name = "URL", required = true)]
    pub(crate) base_url: String,

    /// Default request timeout (`500ms`, `30s`, or `2m`).
    #[arg(long, value_name = "DURATION", value_parser = parse_duration)]
    pub(crate) timeout: Option<Duration>,

    /// Writes the stable redacted JSON report to this file.
    #[arg(long, value_name = "FILE")]
    pub(crate) json_file: Option<PathBuf>,

    /// Writes a redacted JUnit XML report to this file.
    #[arg(long, value_name = "FILE")]
    pub(crate) junit_file: Option<PathBuf>,
}

fn parse_duration(raw: &str) -> Result<Duration, String> {
    let (digits, multiplier) = if let Some(value) = raw.strip_suffix("ms") {
        (value, 1_u64)
    } else if let Some(value) = raw.strip_suffix('s') {
        (value, 1_000)
    } else if let Some(value) = raw.strip_suffix('m') {
        (value, 60_000)
    } else {
        return Err("expected a positive duration such as 500ms, 30s, or 2m".to_owned());
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("expected a positive integer followed by ms, s, or m".to_owned());
    }
    let amount = digits
        .parse::<u64>()
        .map_err(|_| "duration is too large".to_owned())?;
    let milliseconds = amount
        .checked_mul(multiplier)
        .ok_or_else(|| "duration is too large".to_owned())?;
    if milliseconds == 0 {
        return Err("duration must be greater than zero".to_owned());
    }
    Ok(Duration::from_millis(milliseconds))
}
