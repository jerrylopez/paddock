//! Docker-based harness for container tests.
//!
//! Builds `tests/Dockerfile` once per test run, then runs each command in a
//! brand-new container so leftover state never masks a bug. The base image
//! defaults to Debian and can be swapped with `PADDOCK_TEST_BASE_IMAGE`.
//!
//! Plain containers have no init system. Tests of daemons that systemd starts
//! use a `SystemdContainer`, which runs systemd as PID 1 like an orb.
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
use std::thread::sleep;
use std::time::Duration;

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
    let output = docker(&["run", "--rm", image("plain"), "sh", "-euc", script]);
    Outcome::from_output(output)
}

/// A container running systemd as PID 1, like an orb, so `systemctl enable
/// --now` works. Like an orb, it has a `dev` user with passwordless sudo, and
/// `setup_twice_then_verify` runs setup as that user. Each container is
/// fresh; it's removed when dropped.
pub struct SystemdContainer {
    id: String,
}

impl SystemdContainer {
    /// Starts a container and waits for systemd to finish booting.
    pub fn start() -> Self {
        // systemd needs privileges and a writable /run. A private cgroup
        // namespace makes /sys/fs/cgroup look as it does on a real machine;
        // software that reads its own cgroup limits (OpenSearch) relies on it.
        let output = docker(&[
            "run",
            "--detach",
            "--privileged",
            "--cgroupns=private",
            "--tmpfs=/run",
            "--tmpfs=/run/lock",
            image("systemd"),
        ]);
        let container = Self {
            id: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        };
        assert!(
            output.status.success(),
            "starting a systemd container failed\n{}",
            Outcome::from_output(output)
        );
        container.wait_for_boot();
        container
    }

    /// Runs `script` with `sh -eu` in this container.
    pub fn run(&self, script: &str) -> Outcome {
        Outcome::from_output(docker(&["exec", &self.id, "sh", "-euc", script]))
    }

    /// Restarts the container, like a reboot or an orb waking up: processes
    /// are gone and systemd starts the enabled units again.
    pub fn restart(&self) {
        let output = docker(&["restart", &self.id]);
        assert!(
            output.status.success(),
            "restarting the container failed\n{}",
            Outcome::from_output(output)
        );
        self.wait_for_boot();
    }

    /// `setup_twice_then_verify` in this container, running setup as the
    /// `dev` user (through sudo, like an orb). `verify` runs as root.
    #[track_caller]
    pub fn setup_twice_then_verify(
        &self,
        service: &str,
        version: &str,
        flags: &[&str],
        verify: &str,
    ) -> Outcome {
        let outcome = self.run(&setup_twice_script(
            service,
            version,
            flags,
            verify,
            Some("dev"),
        ));
        outcome.assert_success();
        outcome
    }

    fn wait_for_boot(&self) {
        // `is-system-running --wait` returns once startup is done. In a
        // container some host-only units fail, so "degraded" is fine too.
        for _ in 0..60 {
            let state = self.run("systemctl is-system-running --wait || true");
            let state = state.stdout.trim();
            if state == "running" || state == "degraded" {
                return;
            }
            sleep(Duration::from_millis(500));
        }
        panic!("systemd didn't finish booting in the test container");
    }
}

impl Drop for SystemdContainer {
    fn drop(&mut self) {
        let _ = docker(&["rm", "--force", &self.id]);
    }
}

/// Runs `script` the way Paddock runs a service script, with `common.sh` in
/// front and piped to bash, in a fresh container. The service is named `demo`.
/// `docker_args` go to `docker run`, e.g. `&["--user", "1000"]`.
pub fn run_service_script(script: &str, args: &[&str], docker_args: &[&str]) -> Outcome {
    let mut command = Command::new("docker");
    command
        .args(["run", "--rm", "-i", "-e", "PADDOCK_SERVICE=demo"])
        .args(docker_args)
        .args([image("plain"), "bash", "-s", "--"])
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
    let outcome = run(&setup_twice_script(service, version, flags, verify, None));
    outcome.assert_success();
    outcome
}

/// The script behind `setup_twice_then_verify`. Setup runs as `user` when
/// given. During the second run, `apt` and `apt-get` in /usr/local/sbin fail:
/// that's ahead of the real ones on root's PATH and on sudo's secure_path.
fn setup_twice_script(
    service: &str,
    version: &str,
    flags: &[&str],
    verify: &str,
    user: Option<&str>,
) -> String {
    let mut setup = format!("paddock setup {} {}", quote(service), quote(version));
    for flag in flags {
        write!(setup, " {}", quote(flag)).unwrap();
    }
    if let Some(user) = user {
        setup = format!("su {user} -c {}", quote(&setup));
    }
    format!(
        "echo '== setup (first run)'\n{setup}\n\
         for tool in apt apt-get; do\n\
           printf '#!/bin/sh\\necho \"$0 must not run on the second setup\" >&2\\nexit 99\\n' >/usr/local/sbin/$tool\n\
           chmod +x /usr/local/sbin/$tool\n\
         done\n\
         echo '== setup (second run)'\n{setup}\n\
         rm /usr/local/sbin/apt /usr/local/sbin/apt-get\n\
         echo '== verify'\n{verify}\n"
    )
}

/// Name of the `plain` or `systemd` test image, building it on first use.
fn image(target: &'static str) -> &'static str {
    static PLAIN: OnceLock<String> = OnceLock::new();
    static SYSTEMD: OnceLock<String> = OnceLock::new();
    let cell = match target {
        "plain" => &PLAIN,
        "systemd" => &SYSTEMD,
        _ => unreachable!("unknown image target {target}"),
    };
    cell.get_or_init(|| {
        let base =
            env::var("PADDOCK_TEST_BASE_IMAGE").unwrap_or_else(|_| DEFAULT_BASE_IMAGE.into());
        let tag = format!(
            "paddock-test-{target}:{}",
            base.replace([':', '/', '@'], "-")
        );
        let output = docker(&[
            "build",
            "--quiet",
            "--file",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/Dockerfile"),
            "--target",
            target,
            "--build-arg",
            &format!("BASE_IMAGE={base}"),
            "--tag",
            &tag,
            env!("CARGO_MANIFEST_DIR"),
        ]);
        assert!(
            output.status.success(),
            "building the {target} test image from {base} failed\n{}",
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
