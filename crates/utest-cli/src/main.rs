//! Command-line interface for validating and executing UTest source files.
//!
//! `check` compiles bounded UTF-8 source without network access. `run` requires
//! a base URL, performs the same complete compiler pipeline, executes only a
//! successfully converted suite, prints a strictly redacted terminal report,
//! and optionally writes JSON and JUnit artifacts.
//!
//! # Exit codes
//!
//! - `0`: help/version, successful check, or passed suite;
//! - `1`: a standalone test or pipeline failed;
//! - `2`: core failed and aborted the suite;
//! - `3`: lexical, syntax, or semantic diagnostics;
//! - `4`: invalid CLI, configuration, or input;
//! - `5`: internal runner or report-output failure.

#![forbid(unsafe_code)]

mod args;
mod command;
mod diagnostic;
mod env_file;
mod input;
mod output;

use std::process::ExitCode;

use clap::Parser;

use crate::{args::Cli, command::dispatch};

/// Parses arguments without allowing Clap to terminate the process itself.
fn main() -> ExitCode {
    match Cli::try_parse() {
        Ok(cli) => dispatch(cli),
        Err(error) => {
            let code = if error.use_stderr() { 4 } else { 0 };
            let _ = error.print();
            ExitCode::from(code)
        }
    }
}
