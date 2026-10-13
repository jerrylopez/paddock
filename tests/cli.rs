//! Binary tests: run the real `paddock` binary on the host.

use std::process::{Command, Output};

fn paddock(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_paddock"))
        .args(args)
        .output()
        .expect("failed to run paddock")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn version_prints_package_version() {
    let output = paddock(&["--version"]);
    assert!(output.status.success());
    assert_eq!(
        stdout(&output).trim(),
        concat!("paddock ", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_lists_setup_command() {
    let output = paddock(&["--help"]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("setup"));
}

#[test]
fn setup_help_lists_services() {
    let output = paddock(&["setup", "--help"]);
    assert!(output.status.success());
    let help = stdout(&output);
    for service in ["php", "mysql", "typesense"] {
        assert!(help.contains(service), "missing {service} in:\n{help}");
    }
}

#[test]
fn no_arguments_prints_usage_and_fails() {
    let output = paddock(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("Usage: paddock"));
}

#[test]
fn unknown_service_is_rejected() {
    let output = paddock(&["setup", "cobol", "1.0"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("invalid value 'cobol'"));
}

#[test]
fn missing_version_is_rejected() {
    let output = paddock(&["setup", "php"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("<VERSION>"));
}

#[test]
fn setup_runs_the_embedded_service_script() {
    let output = paddock(&["setup", "typesense", "27.1"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stderr(&output), "paddock: typesense: not implemented yet\n");
}

// These are rejected before Paddock touches the system, so they're safe on the host.

#[test]
fn php_rejects_patch_versions() {
    let output = paddock(&["setup", "php", "8.3.1"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "paddock: php: version must be major.minor, e.g. 8.3 (got '8.3.1')\n"
    );
}

#[test]
fn php_rejects_invalid_extension_names() {
    let output = paddock(&["setup", "php", "8.3", "--extensions", "intl,bad!"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "paddock: php: invalid extension name 'bad!'\n"
    );
}

#[test]
fn mysql_rejects_unknown_versions_listing_supported_ones() {
    let output = paddock(&["setup", "mysql", "5.7"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "paddock: mysql: unsupported version '5.7' (supported: 8.0 8.4 9.7 innovation)\n"
    );
}

#[test]
fn mysql_username_needs_password() {
    let output = paddock(&["setup", "mysql", "8.4", "--username", "app"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "paddock: mysql: --username needs --password\n"
    );
}

#[test]
fn mysql_rejects_unsafe_database_names() {
    let output = paddock(&["setup", "mysql", "8.4", "--database", "app`; drop"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "paddock: mysql: database name may only use letters, digits and _ (got 'app`; drop')\n"
    );
}
