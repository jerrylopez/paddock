//! Routes a parsed command to the service that handles it.
//!
//! `SERVICES` is the single registry of what Paddock can set up. Adding a
//! service means adding `scripts/<name>.sh` and one `service!` entry here.

use std::process::ExitCode;

use crate::cli::{Command, SetupArgs};
use crate::runner;

/// A service Paddock knows how to set up.
#[derive(Debug)]
pub struct Service {
    /// Name used on the command line: `paddock setup <name> ...`.
    pub name: &'static str,
    /// One-line description shown in `paddock setup --help`.
    pub about: &'static str,
    /// The service's install script, embedded at compile time.
    pub script: &'static str,
}

/// Registers a service whose install script is `scripts/<name>.sh`, or
/// `scripts/<script>.sh` when several services share one script. The script
/// can tell them apart by `$SERVICE`.
macro_rules! service {
    ($name:literal, $about:literal) => {
        service!($name, $about, script = $name)
    };
    ($name:literal, $about:literal, script = $script:literal) => {
        Service {
            name: $name,
            about: $about,
            script: include_str!(concat!("../scripts/", $script, ".sh")),
        }
    };
}

pub const SERVICES: &[Service] = &[
    service!("php", "PHP CLI with extensions"),
    service!("php-fpm", "PHP-FPM and the PHP CLI", script = "php"),
    service!(
        "elasticsearch",
        "Elasticsearch search engine (single local node)"
    ),
    service!("mysql", "MySQL server"),
    service!("nginx", "nginx web server (stable or mainline)"),
    service!("opensearch", "OpenSearch search engine (single local node)"),
    service!("postgres", "PostgreSQL server"),
    service!("redis", "Redis key-value store"),
    service!("typesense", "Typesense search server"),
    service!("valkey", "Valkey key-value store"),
    service!("varnish", "Varnish HTTP cache"),
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
            Ok(job) => runner::run(&job),
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
