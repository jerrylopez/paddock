//! Command-line definitions: `paddock setup <service> <version> [flags]`.
//!
//! This module only describes the shape of the command line. Which services
//! exist is owned by the registry in `dispatch`, so adding a service never
//! touches this file.

use clap::builder::PossibleValuesParser;
use clap::{Args, Parser, Subcommand};

use crate::dispatch;

#[derive(Debug, Parser)]
#[command(
    name = "paddock",
    version,
    about = "Native environment setup for Amp orbs",
    long_about = "Native environment setup for Amp orbs.\n\n\
        Installs and configures services such as PHP or MySQL directly in the orb, \
        so .agents/setup can stay a short list of `paddock setup` lines.",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Install and configure a service at a given version.
    #[command(after_help = setup_after_help())]
    Setup(SetupArgs),
}

#[derive(Debug, Args)]
pub struct SetupArgs {
    /// Service to install.
    #[arg(value_parser = PossibleValuesParser::new(dispatch::service_names()))]
    pub service: String,

    /// Version to install, e.g. `8.3`.
    pub version: String,

    /// Extra flags passed through to the service's install script.
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "FLAGS"
    )]
    pub flags: Vec<String>,
}

fn setup_after_help() -> String {
    let mut help = String::from("Services:\n");
    for service in dispatch::SERVICES {
        help.push_str(&format!("  {:<12}{}\n", service.name, service.about));
    }
    help.push_str("\nExample:\n  paddock setup php 8.3");
    help
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("paddock").chain(args.iter().copied()))
    }

    fn setup_args(cli: Cli) -> SetupArgs {
        match cli.command {
            Command::Setup(args) => args,
        }
    }

    #[test]
    fn clap_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_service_and_version() {
        let args = setup_args(parse(&["setup", "php", "8.3"]).unwrap());
        assert_eq!(args.service, "php");
        assert_eq!(args.version, "8.3");
        assert!(args.flags.is_empty());
    }

    #[test]
    fn passes_trailing_flags_through() {
        let args =
            setup_args(parse(&["setup", "php", "8.3", "--extensions", "intl,gd", "-q"]).unwrap());
        assert_eq!(args.flags, ["--extensions", "intl,gd", "-q"]);
    }

    #[test]
    fn rejects_unknown_service() {
        let err = parse(&["setup", "cobol", "1.0"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn requires_version() {
        let err = parse(&["setup", "php"]).unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn setup_help_lists_every_registered_service() {
        let help = setup_after_help();
        for service in dispatch::SERVICES {
            assert!(help.contains(service.name), "missing {}", service.name);
        }
    }
}
