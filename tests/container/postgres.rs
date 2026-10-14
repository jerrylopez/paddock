//! `paddock setup postgres`.

use crate::harness;

/// Connects over TCP as the created role and prints who and where it is.
const CONNECT: &str = "PGPASSWORD=\"it's secret\" psql -h 127.0.0.1 -p 5432 -U app -d app -tAc \
    'select current_user, current_database()'";

#[test]
fn creates_database_and_user_under_systemd_and_survives_a_restart() {
    let container = harness::SystemdContainer::start();
    let outcome = container.setup_twice_then_verify(
        "postgres",
        "17.10",
        &[
            "--database",
            "app",
            "--username",
            "app",
            "--password",
            "it's secret",
        ],
        &format!("systemctl is-active postgresql@17-main\n{CONNECT}"),
    );
    assert_eq!(
        outcome.section("setup (first run)").lines().next(),
        Some("paddock: postgres: Installing PostgreSQL 17.10"),
        "\n{outcome}"
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(
        rerun[0].starts_with("paddock: postgres: PostgreSQL 17.10 ")
            && rerun[0].ends_with(" is running on port 5432 (database: app, user: app)"),
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("verify"),
        "active\napp|app\n",
        "\n{outcome}"
    );

    container.restart();
    let after = container.run(&format!(
        "for i in $(seq 30); do pg_isready -q -h 127.0.0.1 -p 5432 && break; sleep 1; done\n{CONNECT}"
    ));
    after.assert_success();
    assert_eq!(after.stdout, "app|app\n", "\n{after}");
}

#[test]
fn installs_archived_releases_side_by_side_without_systemd() {
    let outcome = harness::run(&format!(
        "paddock setup postgres 16.12 --database app --username app --password \"it's secret\"\n\
         paddock setup postgres 17.6\n\
         pg_ctlcluster 16 main start\n\
         {CONNECT}\n\
         paddock setup postgres 17.99 || echo \"exit=$?\""
    ));
    outcome.assert_success();
    let lines: Vec<_> = outcome.stdout.lines().collect();
    assert!(
        lines.iter().any(
            |line| line.starts_with("paddock: postgres: PostgreSQL 16.12 ")
                && line.ends_with(" is installed on port 5432 (database: app, user: app)")
        ),
        "\n{outcome}"
    );
    assert!(
        lines.iter().any(
            |line| line.starts_with("paddock: postgres: PostgreSQL 17.6 ")
                && line.ends_with(" is installed on port 5433")
        ),
        "\n{outcome}"
    );
    assert_eq!(
        lines[lines.len() - 2..],
        ["app|app", "exit=2"],
        "\n{outcome}"
    );
    assert!(
        outcome
            .stderr
            .contains("paddock: postgres: PostgreSQL 17.99 isn't published (available: 17."),
        "\n{outcome}"
    );
}
