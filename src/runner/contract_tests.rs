//! Tests for the install-script contract helpers in `scripts/common.sh`.
//! Each test runs a small service script through the real runner path.

use std::process::{Output, Stdio};

use super::{bash, combine, feed};

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run(script: &str, args: &[&str]) -> Run {
    run_with_env(script, args, &[])
}

fn run_with_env(script: &str, args: &[&str], env: &[(&str, &str)]) -> Run {
    let mut command = bash("demo", args);
    command
        .env_remove("PADDOCK_VERBOSE")
        .envs(env.iter().copied())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    feed(&mut child, &combine(script)).unwrap();
    let Output {
        status,
        stdout,
        stderr,
    } = child.wait_with_output().unwrap();
    Run {
        code: status.code(),
        stdout: String::from_utf8(stdout).unwrap(),
        stderr: String::from_utf8(stderr).unwrap(),
    }
}

const DEMO: &str = r#"
option database app
option root-password ""
parse_args "$@"
echo "version=$VERSION database=$database root_password=$root_password"
"#;

#[test]
fn successful_run_prints_only_what_the_script_prints() {
    let run = run(DEMO, &["8.4"]);
    assert_eq!(run.code, Some(0));
    assert_eq!(run.stdout, "version=8.4 database=app root_password=\n");
    assert_eq!(run.stderr, "");
}

#[test]
fn flags_override_defaults() {
    let run = run(
        DEMO,
        &["8.4", "--database", "shop", "--root-password=s3 cret"],
    );
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert_eq!(
        run.stdout,
        "version=8.4 database=shop root_password=s3 cret\n"
    );
}

#[test]
fn unknown_flag_is_a_usage_error_listing_supported_flags() {
    let run = run(DEMO, &["8.4", "--port", "3307"]);
    assert_eq!(run.code, Some(2));
    assert_eq!(
        run.stderr,
        "paddock: demo: unknown flag --port (supported: --database --root-password)\n"
    );
}

#[test]
fn unknown_flag_on_a_service_without_flags() {
    let run = run(r#"parse_args "$@""#, &["1.0", "--port", "1"]);
    assert_eq!(run.code, Some(2));
    assert_eq!(
        run.stderr,
        "paddock: demo: unknown flag --port (demo takes no flags)\n"
    );
}

#[test]
fn flag_without_value_is_a_usage_error() {
    let run = run(DEMO, &["8.4", "--database"]);
    assert_eq!(run.code, Some(2));
    assert_eq!(run.stderr, "paddock: demo: --database needs a value\n");
}

#[test]
fn stray_argument_is_a_usage_error() {
    let run = run(DEMO, &["8.4", "extra"]);
    assert_eq!(run.code, Some(2));
    assert_eq!(run.stderr, "paddock: demo: unexpected argument 'extra'\n");
}

#[test]
fn require_version_accepts_listed_versions() {
    let run = run(
        r#"parse_args "$@"; require_version 8.0 8.4; echo ok"#,
        &["8.4"],
    );
    assert_eq!(run.code, Some(0));
    assert_eq!(run.stdout, "ok\n");
}

#[test]
fn require_version_rejects_others_and_lists_supported() {
    let run = run(r#"parse_args "$@"; require_version 8.0 8.4"#, &["9.9"]);
    assert_eq!(run.code, Some(2));
    assert_eq!(
        run.stderr,
        "paddock: demo: unsupported version '9.9' (supported: 8.0 8.4)\n"
    );
}

#[test]
fn info_and_warn_are_prefixed() {
    let run = run("info 'installing 8.4'\nwarn 'slow mirror'", &["8.4"]);
    assert_eq!(run.code, Some(0));
    assert_eq!(run.stdout, "paddock: demo: installing 8.4\n");
    assert_eq!(run.stderr, "paddock: demo: warning: slow mirror\n");
}

#[test]
fn die_exits_1_with_only_its_message() {
    let run = run("die 'download failed'\necho unreachable", &["8.4"]);
    assert_eq!(run.code, Some(1));
    assert_eq!(run.stdout, "");
    assert_eq!(run.stderr, "paddock: demo: download failed\n");
}

#[test]
fn unexpected_failure_names_the_failing_command() {
    let run = run("cat /nonexistent-paddock-path\necho unreachable", &["8.4"]);
    assert_eq!(run.code, Some(1));
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr
            .ends_with("paddock: demo: failed: cat /nonexistent-paddock-path (exit 1)\n"),
        "{}",
        run.stderr
    );
}

#[test]
fn failure_inside_a_function_is_reported() {
    let run = run("install() { false; echo unreachable; }\ninstall", &["8.4"]);
    assert_eq!(run.code, Some(1));
    assert_eq!(run.stdout, "");
    assert_eq!(run.stderr, "paddock: demo: failed: false (exit 1)\n");
}

#[test]
fn bare_exit_is_reported() {
    let run = run("exit 3", &["8.4"]);
    assert_eq!(run.code, Some(3));
    assert_eq!(run.stderr, "paddock: demo: exited with status 3\n");
}

#[test]
fn quietly_hides_output_on_success() {
    let run = run(
        "quietly sh -c 'echo noisy; echo noisier >&2'\necho done",
        &["8.4"],
    );
    assert_eq!(run.code, Some(0));
    assert_eq!(run.stdout, "done\n");
    assert_eq!(run.stderr, "");
}

#[test]
fn quietly_shows_output_and_command_on_failure() {
    let run = run(
        "quietly sh -c 'echo step one; echo broke >&2; exit 4'\necho unreachable",
        &["8.4"],
    );
    assert_eq!(run.code, Some(4));
    assert_eq!(run.stdout, "");
    assert_eq!(
        run.stderr,
        "step one\nbroke\npaddock: demo: failed: sh -c echo step one; echo broke >&2; exit 4 (exit 4)\n"
    );
}

#[test]
fn quietly_shows_output_live_when_verbose() {
    let run = run_with_env(
        "quietly sh -c 'echo noisy'",
        &["8.4"],
        &[("PADDOCK_VERBOSE", "1")],
    );
    assert_eq!(run.code, Some(0));
    assert_eq!(run.stdout, "noisy\n");
}

#[test]
fn has_command_checks_the_path() {
    let run = run(
        "has_command bash && echo yes\nhas_command no-such-paddock-tool || echo no",
        &["8.4"],
    );
    assert_eq!(run.code, Some(0));
    assert_eq!(run.stdout, "yes\nno\n");
}

#[test]
fn service_name_comes_from_paddock() {
    let run = run(r#"echo "$SERVICE""#, &["8.4"]);
    assert_eq!(run.stdout, "demo\n");
}
