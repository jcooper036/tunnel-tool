use std::process::ExitCode;

use clap::Parser;

use tunnel_tool::cli::Cli;
use tunnel_tool::commands::{self, status::Silent};

fn main() -> ExitCode {
    match commands::run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) if err.is::<Silent>() => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
