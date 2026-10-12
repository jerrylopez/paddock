//! Paddock entry point. Parsing lives in `cli`, routing in `dispatch`, and
//! running install scripts in `runner`.

mod cli;
mod dispatch;
mod runner;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    dispatch::run(cli.command)
}
