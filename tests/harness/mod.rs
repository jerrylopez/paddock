//! Docker-based harness for container tests.
//!
//! Builds `tests/Dockerfile` once per test run, then runs each command in a
//! brand-new container so leftover state never masks a bug. The base image
//! defaults to Debian and can be swapped with `PADDOCK_TEST_BASE_IMAGE`.
//!
//! Container tests only build with the `container-tests` feature; run them
//! with `cargo container-tests`.

// Each test crate that includes this module uses a different subset of it.
#![allow(dead_code)]

use std::env;
use std::fmt::Write as _;
use std::io::Write as _;
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;

const DEFAULT_BASE_IMAGE: &str = "debian:bookworm-slim";

const COMMON: &str = include_str!("../../scripts/common.sh");

/// What a container run produced.
pub struct Outcome {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Outcome {
    fn from_output(output: Output) -> Self {
        Self {
            success: output.status.success(),
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }

    /// Output printed between the `== NAME` marker and the next marker.
    pub fn section(&self, name: &str) -> &str {
        let marker = format!("== {name}\n");
        let start = self
            .stdout
            .find(&marker)
            .unwrap_or_else(|| panic!("no `{name}` section\n{self}"))
            + marker.len();
        let rest = &self.stdout[start..];
        &rest[..rest.find("== ").unwrap_or(rest.len())]
    }

    /// Panics with the full container output unless the run exited 0.
    #[track_caller]
    pub fn assert_success(&self) -> &Self {
        assert!(self.success, "container run failed\n{self}");
        self
    }
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "--- stdout ---\n{}\n--- stderr ---\n{}",
            self.stdout, self.stderr
        )
    }
}

/// Runs `script` with `sh -eu` in a fresh container that has `paddock` on its PATH.
pub fn run(script: &str) -> Outcome {
    let output = docker(&["run", "--rm", image(), "sh", "-euc", script]);
    Outcome::from_output(output)
}

/// Runs `script` the way Paddock runs a service script, with `common.sh` in
/// front and piped to bash, in a fresh container. The service is named `demo`.
/// `docker_args` go to `docker run`, e.g. `&["--user", "1000"]`.
pub fn run_service_script(script: &str, args: &[&str], docker_args: &[&str]) -> Outcome {
    let mut command = Command::new("docker");
    command
        .args(["run", "--rm", "-i", "-e", "PADDOCK_SERVICE=demo"])
        .args(docker_args)
        .args([image(), "bash", "-s", "--"])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("could not run `docker`");
    // Same shape as the runner's combined script (src/runner.rs).
    let combined = format!("{{\n{COMMON}\n{script}\n}}\n");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(combined.as_bytes())
        .unwrap();
    Outcome::from_output(child.wait_with_output().unwrap())
}

/// Runs `paddock setup <service> <version> [flags]` twice in one fresh
/// container, then `verify`. The second run mirrors a stale snapshot re-running
/// `.agents/setup`: everything is already installed, so it must succeed without
/// apt (`apt` and `apt-get` fail during it). Asserts every step exited 0.
/// Each step's output is available with `Outcome::section`.
///
/// ```ignore
/// harness::setup_twice_then_verify("php", "8.3", &[], "php -v | grep -q 'PHP 8.3'");
/// ```
#[track_caller]
pub fn setup_twice_then_verify(
    service: &str,
    version: &str,
    flags: &[&str],
    verify: &str,
) -> Outcome {
    let mut setup = format!("paddock setup {} {}", quote(service), quote(version));
    for flag in flags {
        write!(setup, " {}", quote(flag)).unwrap();
    }
    let script = format!(
        "echo '== setup (first run)'\n{setup}\n\
         mkdir -p /tmp/paddock-no-apt\n\
         for tool in apt apt-get; do\n\
           printf '#!/bin/sh\\necho \"$0 must not run on the second setup\" >&2\\nexit 99\\n' >/tmp/paddock-no-apt/$tool\n\
           chmod +x /tmp/paddock-no-apt/$tool\n\
         done\n\
         echo '== setup (second run)'\nPATH=/tmp/paddock-no-apt:$PATH {setup}\n\
         echo '== verify'\n{verify}\n"
    );
    let outcome = run(&script);
    outcome.assert_success();
    outcome
}

/// Name of the test image, building it on first use.
fn image() -> &'static str {
    static IMAGE: OnceLock<String> = OnceLock::new();
    IMAGE.get_or_init(|| {
        let base =
            env::var("PADDOCK_TEST_BASE_IMAGE").unwrap_or_else(|_| DEFAULT_BASE_IMAGE.into());
        let tag = format!("paddock-test:{}", base.replace([':', '/', '@'], "-"));
        let output = docker(&[
            "build",
            "--quiet",
            "--file",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/Dockerfile"),
            "--build-arg",
            &format!("BASE_IMAGE={base}"),
            "--tag",
            &tag,
            env!("CARGO_MANIFEST_DIR"),
        ]);
        assert!(
            output.status.success(),
            "building the test image from {base} failed\n{}",
            Outcome::from_output(output)
        );
        tag
    })
}

fn docker(args: &[&str]) -> Output {
    Command::new("docker")
        .args(args)
        .output()
        .unwrap_or_else(|err| {
            panic!(
                "could not run `docker` ({err}); container tests need Docker installed and running"
            )
        })
}

/// Single-quotes `arg` for `sh`.
fn quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', r"'\''"))
}
