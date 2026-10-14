//! `paddock setup rabbitmq`.

use crate::harness;

/// Logs in as the created user via the CLI and the management API.
const CHECK: &str = "rabbitmqctl authenticate_user app \"it's secret\" 2>/dev/null | tail -n1\n\
    rabbitmqctl list_permissions -p app --silent | awk '{ print $1 }'\n\
    curl -fsS -u \"app:it's secret\" http://127.0.0.1:15672/api/whoami | grep -o '\"name\":\"app\"'";

#[test]
fn creates_user_and_vhost_under_systemd_and_survives_a_restart() {
    let container = harness::SystemdContainer::start();
    let outcome = container.setup_twice_then_verify(
        "rabbitmq",
        "4.2",
        &[
            "--username",
            "app",
            "--password",
            "it's secret",
            "--vhost",
            "app",
        ],
        &format!("systemctl is-active rabbitmq-server\n{CHECK}"),
    );
    assert_eq!(
        outcome.section("setup (first run)").lines().next(),
        Some("paddock: rabbitmq: Installing RabbitMQ 4.2"),
        "\n{outcome}"
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(
        rerun[0].starts_with("paddock: rabbitmq: RabbitMQ 4.2.")
            && rerun[0].ends_with(
                " is running on 127.0.0.1:5672 (management UI on http://localhost:15672, user: app, vhost: app)"
            ),
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("verify"),
        "active\nSuccess\napp\n\"name\":\"app\"\n",
        "\n{outcome}"
    );

    container.restart();
    let after = container.run(&format!(
        "rabbitmqctl await_startup --timeout 120 >/dev/null 2>&1\n{CHECK}"
    ));
    after.assert_success();
    assert_eq!(
        after.stdout, "Success\napp\n\"name\":\"app\"\n",
        "\n{after}"
    );
}

#[test]
fn upgrades_series_and_refuses_to_go_back() {
    let outcome = harness::run(
        "paddock setup rabbitmq 4.1 --username app --password secret\n\
         paddock setup rabbitmq 4.2\n\
         dpkg-query -W -f='${Version}\\n' rabbitmq-server | cut -d. -f1-2\n\
         paddock setup rabbitmq 4.1 || echo \"exit=$?\"\n\
         paddock setup rabbitmq 4.99 || echo \"exit=$?\"",
    );
    outcome.assert_success();
    let lines: Vec<_> = outcome.stdout.lines().collect();
    assert!(
        lines.contains(&"paddock: rabbitmq: Upgrading RabbitMQ 4.1 to 4.2"),
        "\n{outcome}"
    );
    assert!(lines.contains(&"4.2"), "\n{outcome}");
    assert_eq!(
        lines[lines.len() - 2..],
        ["exit=1", "exit=2"],
        "\n{outcome}"
    );
    assert!(
        outcome.stderr.contains(
            "paddock: rabbitmq: RabbitMQ 4.2 is installed; going back to 4.1 isn't supported"
        ),
        "\n{outcome}"
    );
    assert!(
        outcome
            .stderr
            .contains("paddock: rabbitmq: RabbitMQ 4.99 isn't available (available: 4.0 4.1 4.2"),
        "\n{outcome}"
    );
}
