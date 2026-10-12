//! Runs a service's embedded install script.
//!
//! `common.sh` and the service script are joined into one script in memory and
//! piped to `bash -s`, with the version and flags as positional arguments and
//! the service name in `PADDOCK_SERVICE`. bash inherits Paddock's stdout and
//! stderr, so output appears live.

use std::io::{self, Write};
use std::process::{Child, Command, ExitCode, ExitStatus, Stdio};

use crate::dispatch::SetupJob;

/// Shared prelude placed in front of every service script.
const COMMON: &str = include_str!("../scripts/common.sh");

pub fn run(job: &SetupJob) -> ExitCode {
    let script = combine(job.service.script);
    let args: Vec<&str> = std::iter::once(job.version.as_str())
        .chain(job.flags.iter().map(String::as_str))
        .collect();

    match run_script(job.service.name, &script, &args) {
        Ok(status) => exit_code(status),
        Err(err) => {
            eprintln!("paddock: could not run bash: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Joins `common.sh` and a service script into the single script bash runs.
fn combine(service_script: &str) -> String {
    // `common.sh` goes first so its settings and helpers exist before any service
    // line runs. The braces make bash parse the whole script before running any
    // of it, so a command that reads stdin can't swallow the rest of the script.
    format!("{{\n{COMMON}\n{service_script}\n}}\n")
}

fn run_script(service: &str, script: &str, args: &[&str]) -> io::Result<ExitStatus> {
    let mut child = bash(service, args).spawn()?;
    feed(&mut child, script)?;
    child.wait()
}

/// A bash command that reads its script from stdin.
fn bash(service: &str, args: &[&str]) -> Command {
    let mut command = Command::new("bash");
    command
        .args(["-s", "--"])
        .args(args)
        .env("PADDOCK_SERVICE", service)
        .stdin(Stdio::piped());
    command
}

/// Writes `script` to the child's stdin and closes it.
fn feed(child: &mut Child, script: &str) -> io::Result<()> {
    let mut stdin = child.stdin.take().expect("stdin is piped");
    match stdin.write_all(script.as_bytes()) {
        // bash stopped reading early (e.g. a syntax error); its exit status says why.
        Err(err) if err.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        result => result,
    }
}

fn exit_code(status: ExitStatus) -> ExitCode {
    match status.code() {
        Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        None => {
            eprintln!("paddock: install script was killed by a signal");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::SERVICES;

    #[test]
    fn common_comes_before_the_service_script() {
        let script = combine("echo service");
        let common = script.find(COMMON).expect("common.sh is included");
        let service = script.find("echo service").unwrap();
        assert!(common < service);
    }

    #[test]
    fn every_embedded_script_is_valid_bash() {
        for service in SERVICES {
            let mut child = Command::new("bash")
                .args(["-n", "-s"])
                .stdin(Stdio::piped())
                .spawn()
                .unwrap();
            let mut stdin = child.stdin.take().unwrap();
            stdin.write_all(combine(service.script).as_bytes()).unwrap();
            drop(stdin);
            assert!(
                child.wait().unwrap().success(),
                "{}.sh has a syntax error",
                service.name
            );
        }
    }

    #[test]
    fn passes_version_and_flags_as_arguments() {
        let script = combine(
            r#"[ $# -eq 3 ] && [ "$1" = 8.3 ] && [ "$2" = --name ] && [ "$3" = "it's two words" ]"#,
        );
        let status = run_script("test", &script, &["8.3", "--name", "it's two words"]).unwrap();
        assert!(status.success());
    }

    #[test]
    fn passes_through_the_exit_code() {
        let status = run_script("test", &combine("exit 3"), &["1.0"]).unwrap();
        assert_eq!(status.code(), Some(3));
        assert_eq!(exit_code(status), ExitCode::from(3));
    }

    #[test]
    fn common_settings_apply_to_the_service_script() {
        // `set -e` from common.sh stops the script at the first failing command.
        let status = run_script("test", &combine("false\nexit 0"), &["1.0"]).unwrap();
        assert!(!status.success());
    }

    #[test]
    fn commands_reading_stdin_cannot_swallow_the_script() {
        let status = run_script("test", &combine("cat >/dev/null\nexit 7"), &["1.0"]).unwrap();
        assert_eq!(status.code(), Some(7));
    }
}
