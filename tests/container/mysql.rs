//! `paddock setup mysql`.

use crate::harness;

/// Starts MySQL without systemd and waits until it accepts connections.
const START_MYSQL: &str = "/usr/sbin/mysqld --daemonize --user=mysql >/dev/null\n\
    for i in $(seq 30); do mysqladmin --no-defaults ping >/dev/null 2>&1 && break; sleep 1; done\n";

#[test]
fn creates_database_and_user_and_reruns_cleanly() {
    // Quotes, backslashes and `#` exercise the SQL and option-file escaping.
    let password = r#"it's "app" \ #1"#;
    let root_password = r#"r00t's "pw" \"#;
    let verify = format!(
        "{START_MYSQL}\
         MYSQL_PWD='{}' mysql -uapp -h127.0.0.1 app -Nse 'select concat(current_user(), \" \", database())'\n\
         MYSQL_PWD='{}' mysql -uroot -Nse 'select \"root ok\"'\n\
         /usr/sbin/mysqld --version",
        password.replace('\'', r"'\''"),
        root_password.replace('\'', r"'\''"),
    );
    let outcome = harness::setup_twice_then_verify(
        "mysql",
        "8.4",
        &[
            "--database",
            "app",
            "--username",
            "app",
            "--password",
            password,
            "--root-password",
            root_password,
        ],
        &verify,
    );
    let first: Vec<_> = outcome.section("setup (first run)").lines().collect();
    assert_eq!(
        first[0], "paddock: mysql: Installing MySQL 8.4",
        "\n{outcome}"
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(
        rerun[0].starts_with("paddock: mysql: MySQL 8.4.")
            && rerun[0].ends_with(" is installed (database: app, user: app)"),
        "\n{outcome}"
    );
    let verify: Vec<_> = outcome.section("verify").lines().collect();
    assert_eq!(verify[..2], ["app@% app", "root ok"], "\n{outcome}");
    assert!(verify[2].contains(" Ver 8.4."), "\n{outcome}");
    assert!(
        outcome
            .stderr
            .contains("paddock: mysql: warning: systemd isn't running"),
        "\n{outcome}"
    );
}

#[test]
fn series_8_0_installs_and_refuses_to_switch() {
    let outcome = harness::run(
        "paddock setup mysql 8.0\n\
         /usr/sbin/mysqld --version\n\
         paddock setup mysql 8.4 || echo \"exit=$?\"",
    );
    outcome.assert_success();
    let lines: Vec<_> = outcome.stdout.lines().collect();
    assert!(
        lines.iter().any(|line| line.contains(" Ver 8.0.")),
        "\n{outcome}"
    );
    assert_eq!(lines.last(), Some(&"exit=1"), "\n{outcome}");
    assert!(
        outcome.stderr.contains(
            "paddock: mysql: MySQL from mysql-8.0 is already installed; switching to 8.4 isn't supported"
        ),
        "\n{outcome}"
    );
}

#[test]
fn runs_under_systemd_and_survives_a_restart() {
    let container = harness::SystemdContainer::start();
    let outcome = container.setup_twice_then_verify(
        "mysql",
        "8.4",
        &[
            "--database",
            "app",
            "--username",
            "app",
            "--password",
            "secret",
        ],
        "systemctl is-enabled mysql\n\
         systemctl is-active mysql\n\
         MYSQL_PWD=secret mysql -uapp -h127.0.0.1 app -Nse 'select database()'",
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(
        rerun[0].ends_with(" is running on /var/run/mysqld/mysqld.sock (database: app, user: app)"),
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("verify"),
        "enabled\nactive\napp\n",
        "\n{outcome}"
    );
    assert!(
        !outcome.stderr.contains("systemd isn't running"),
        "\n{outcome}"
    );

    container.restart();
    let after = container.run(
        "systemctl is-active mysql\n\
         MYSQL_PWD=secret mysql -uapp -h127.0.0.1 app -Nse 'select 1'",
    );
    after.assert_success();
    assert_eq!(after.stdout, "active\n1\n", "\n{after}");
}
