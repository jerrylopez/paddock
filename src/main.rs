//! Paddock entry point. Parsing lives in `cli`, routing in `dispatch`.

mod cli;
mod dispatch;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    dispatch::run(cli.command)
}
