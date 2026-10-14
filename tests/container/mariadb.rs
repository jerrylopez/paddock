//! `paddock setup mariadb`.

use crate::harness;

#[test]
fn creates_database_and_user_under_systemd_and_survives_a_restart() {
    let container = harness::SystemdContainer::start();
    // Quotes, backslashes and `#` exercise the SQL escaping.
    let password = r#"it's "app" \ #1"#;
    let connect = format!(
        "MYSQL_PWD='{}' mariadb -uapp -h127.0.0.1 app -Nse 'select concat(current_user(), \" \", database())'",
        password.replace('\'', r"'\''")
    );
    let outcome = container.setup_twice_then_verify(
        "mariadb",
        "11.4.12",
        &[
            "--database",
            "app",
            "--username",
            "app",
            "--password",
            password,
            "--root-password",
            "r00t",
        ],
        &format!(
            "systemctl is-active mariadb\n\
             {connect}\n\
             MYSQL_PWD=r00t mariadb -uroot -h127.0.0.1 -Nse 'select \"root password ok\"'\n\
             mariadb -uroot -Nse 'select \"root socket ok\"'"
        ),
    );
    assert_eq!(
        outcome.section("setup (first run)").lines().next(),
        Some("paddock: mariadb: Installing MariaDB 11.4.12"),
        "\n{outcome}"
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(
        rerun[0].starts_with("paddock: mariadb: MariaDB 11.4.12 ")
            && rerun[0]
                .ends_with(" is running on /run/mysqld/mysqld.sock (database: app, user: app)"),
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("verify"),
        "active\napp@% app\nroot password ok\nroot socket ok\n",
        "\n{outcome}"
    );

    container.restart();
    let after = container.run(&format!(
        "for i in $(seq 30); do mariadb-admin ping >/dev/null 2>&1 && break; sleep 1; done\n{connect}"
    ));
    after.assert_success();
    assert_eq!(after.stdout, "app@% app\n", "\n{after}");
}

#[test]
fn applies_flags_without_systemd_and_refuses_to_switch_series() {
    let outcome = harness::run(
        "paddock setup mariadb 10.11.18 --database app --username app --password secret\n\
         install -d -o mysql -g mysql /run/mysqld\n\
         mariadbd-safe --user=mysql >/dev/null 2>&1 &\n\
         for i in $(seq 30); do mariadb-admin ping >/dev/null 2>&1 && break; sleep 1; done\n\
         MYSQL_PWD=secret mariadb -uapp -h127.0.0.1 app -Nse 'select database()'\n\
         mariadb-admin shutdown\n\
         paddock setup mariadb 10.11.19\n\
         paddock setup mariadb 10.11.18 || echo \"exit=$?\"\n\
         paddock setup mariadb 11.4.13 || echo \"exit=$?\"\n\
         paddock setup mysql 8.4 || echo \"exit=$?\"",
    );
    outcome.assert_success();
    let lines: Vec<_> = outcome.stdout.lines().collect();
    assert!(
        lines.iter().any(
            |line| line.starts_with("paddock: mariadb: MariaDB 10.11.18 ")
                && line.ends_with(" is installed (database: app, user: app)")
        ),
        "\n{outcome}"
    );
    assert_eq!(
        lines[lines.len() - 3..],
        ["exit=1", "exit=1", "exit=1"],
        "\n{outcome}"
    );
    for expected in [
        "app",
        "paddock: mariadb: Upgrading MariaDB 10.11.18 to 10.11.19",
    ] {
        assert!(lines.contains(&expected), "{expected}\n{outcome}");
    }
    assert!(
        outcome.stderr.contains(
            "paddock: mariadb: MariaDB 10.11.19 is installed; going back to 10.11.18 isn't supported"
        ),
        "\n{outcome}"
    );
    assert!(
        outcome.stderr.contains(
            "paddock: mariadb: MariaDB 10.11.19 is installed; switching to another series isn't supported"
        ),
        "\n{outcome}"
    );
    assert!(
        outcome
            .stderr
            .contains("paddock: mysql: MariaDB is installed; remove it before setting up MySQL"),
        "\n{outcome}"
    );
}

#[test]
fn rejects_series_without_a_repository() {
    let outcome = harness::run("paddock setup mariadb 10.11.99 || echo \"exit=$?\"");
    assert!(outcome.stdout.ends_with("exit=2\n"), "\n{outcome}");
    assert!(
        outcome
            .stderr
            .contains("paddock: mariadb: MariaDB 10.11.99 isn't published for Debian bookworm"),
        "\n{outcome}"
    );
}
