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
use std::process::{Command, Output};
use std::sync::OnceLock;

const DEFAULT_BASE_IMAGE: &str = "debian:bookworm-slim";

/// What a container run produced.
pub struct Outcome {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

impl Outcome {
    fn from_output(output: Output) -> Self {
        Self {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
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

/// Runs `paddock setup <service> <version> [flags]` twice in one fresh
/// container, then `verify`. The second run mirrors a stale snapshot re-running
/// `.agents/setup`, so it must succeed too. Asserts every step exited 0.
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
         echo '== setup (second run)'\n{setup}\n\
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
