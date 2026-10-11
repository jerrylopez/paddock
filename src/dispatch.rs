//! Routes a parsed command to the service that handles it.
//!
//! `SERVICES` is the single registry of what Paddock can set up. Adding a
//! service means adding its install script and one entry here.

use std::process::ExitCode;

use crate::cli::{Command, SetupArgs};

/// A service Paddock knows how to set up.
#[derive(Debug)]
pub struct Service {
    /// Name used on the command line: `paddock setup <name> ...`.
    pub name: &'static str,
    /// One-line description shown in `paddock setup --help`.
    pub about: &'static str,
}

pub const SERVICES: &[Service] = &[
    Service {
        name: "php",
        about: "PHP CLI and FPM",
    },
    Service {
        name: "mysql",
        about: "MySQL server",
    },
    Service {
        name: "typesense",
        about: "Typesense search server",
    },
];

pub fn service_names() -> impl Iterator<Item = &'static str> {
    SERVICES.iter().map(|service| service.name)
}

pub fn find(name: &str) -> Option<&'static Service> {
    SERVICES.iter().find(|service| service.name == name)
}

/// A fully resolved setup request, ready to hand to the runner.
#[derive(Debug)]
pub struct SetupJob {
    pub service: &'static Service,
    pub version: String,
    pub flags: Vec<String>,
}

pub fn run(command: Command) -> ExitCode {
    match command {
        Command::Setup(args) => match resolve_setup(args) {
            Ok(job) => setup(job),
            Err(message) => {
                eprintln!("paddock: {message}");
                ExitCode::FAILURE
            }
        },
    }
}

fn resolve_setup(args: SetupArgs) -> Result<SetupJob, String> {
    // clap already restricts `service` to registered names; this guards against
    // the two drifting apart.
    let service =
        find(&args.service).ok_or_else(|| format!("unknown service '{}'", args.service))?;
    Ok(SetupJob {
        service,
        version: args.version,
        flags: args.flags,
    })
}

fn setup(job: SetupJob) -> ExitCode {
    // Install logic lands with the embedded-script runner (FRGE-8).
    let mut request = format!("{} {}", job.service.name, job.version);
    for flag in &job.flags {
        request.push(' ');
        request.push_str(flag);
    }
    eprintln!("paddock: setup for `{request}` is not implemented yet");
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_names_are_unique() {
        let mut names: Vec<_> = service_names().collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SERVICES.len());
    }

    #[test]
    fn resolves_registered_service() {
        let job = resolve_setup(SetupArgs {
            service: "mysql".into(),
            version: "8.4".into(),
            flags: vec!["--port".into(), "3307".into()],
        })
        .unwrap();
        assert_eq!(job.service.name, "mysql");
        assert_eq!(job.version, "8.4");
        assert_eq!(job.flags, ["--port", "3307"]);
    }

    #[test]
    fn rejects_unregistered_service() {
        let err = resolve_setup(SetupArgs {
            service: "cobol".into(),
            version: "1.0".into(),
            flags: vec![],
        })
        .unwrap_err();
        assert!(err.contains("cobol"));
    }
}
