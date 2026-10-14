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
        "paddock: mysql: unsupported version '5.7' (supported: 8.0 8.4 9.7)\n"
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

#[test]
fn nginx_rejects_channels_and_exact_releases() {
    for version in ["stable", "1.30.5"] {
        let output = paddock(&["setup", "nginx", version]);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            stderr(&output),
            format!(
                "paddock: nginx: version must be an nginx series like 1.30 (got '{version}')\n"
            )
        );
    }
}

#[test]
fn opensearch_rejects_bad_heap_sizes() {
    let output = paddock(&["setup", "opensearch", "3.9", "--heap", "lots"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "paddock: opensearch: --heap must be a size like 512m or 2g (got 'lots')\n"
    );
}

#[test]
fn postgres_rejects_minor_versions() {
    let output = paddock(&["setup", "postgres", "17.2"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "paddock: postgres: version must be a PostgreSQL major version like 17 (got '17.2')\n"
    );
}

#[test]
fn varnish_rejects_bad_backends() {
    let output = paddock(&["setup", "varnish", "8.0", "--backend", "localhost"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "paddock: varnish: --backend must be HOST:PORT (got 'localhost')\n"
    );
}

#[test]
fn rabbitmq_rejects_3x_series() {
    let output = paddock(&["setup", "rabbitmq", "3.13"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "paddock: rabbitmq: version must be a RabbitMQ 4.x series like 4.2 (got '3.13'); 3.x is out of support\n"
    );
}

#[test]
fn services_take_series_not_exact_releases() {
    for (service, version) in [
        ("redis", "8.2.3"),
        ("elasticsearch", "9.5.5"),
        ("opensearch", "3.9.0"),
        ("rabbitmq", "4.2.9"),
        ("varnish", "8.0.2"),
        ("mariadb", "11.4.13"),
        ("mysql", "8.4.6"),
        ("valkey", "9.1.2"),
        ("postgres", "17.6"),
        ("php", "8.3.1"),
    ] {
        let output = paddock(&["setup", service, version]);
        assert_eq!(output.status.code(), Some(2), "{service} {version}");
        assert!(
            stderr(&output).contains(&format!("'{version}'")),
            "{service}: {}",
            stderr(&output)
        );
    }
}
