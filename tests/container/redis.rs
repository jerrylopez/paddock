//! `paddock setup redis`.

use crate::harness;

#[test]
fn runs_under_systemd_with_a_password_and_keeps_data_across_a_restart() {
    let container = harness::SystemdContainer::start();
    let outcome = container.setup_twice_then_verify(
        "redis",
        "8.10",
        &["--password", "secret"],
        "systemctl is-active redis-server\n\
         redis-cli ping 2>&1 | grep -o NOAUTH\n\
         REDISCLI_AUTH=secret redis-cli set greeting hello\n\
         REDISCLI_AUTH=secret redis-cli info server | grep -o 'redis_version:8\\.10\\.'",
    );
    assert_eq!(
        outcome.section("setup (first run)").lines().next(),
        Some("paddock: redis: Installing Redis 8.10"),
        "\n{outcome}"
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(
        rerun[0].starts_with("paddock: redis: Redis 8.10.")
            && rerun[0].ends_with(" is running on 127.0.0.1:6379"),
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("verify"),
        "active\nNOAUTH\nOK\nredis_version:8.10.\n",
        "\n{outcome}"
    );

    container.restart();
    let after = container.run(
        "for i in $(seq 30); do REDISCLI_AUTH=secret redis-cli ping 2>/dev/null | grep -q PONG && break; sleep 1; done\n\
         REDISCLI_AUTH=secret redis-cli get greeting",
    );
    after.assert_success();
    assert_eq!(after.stdout, "hello\n", "\n{after}");
}

#[test]
fn upgrades_series_and_refuses_to_go_back() {
    let outcome = harness::run(
        "paddock setup redis 7.4\n\
         paddock setup redis 8.2\n\
         redis-server --version | grep -o 'v=8\\.2\\.'\n\
         paddock setup redis 7.4 || echo \"exit=$?\"\n\
         paddock setup redis 9.9 || echo \"exit=$?\"",
    );
    outcome.assert_success();
    let lines: Vec<_> = outcome.stdout.lines().collect();
    assert!(
        lines.contains(&"paddock: redis: Upgrading Redis 7.4 to 8.2"),
        "\n{outcome}"
    );
    assert!(lines.contains(&"v=8.2."), "\n{outcome}");
    assert_eq!(
        lines[lines.len() - 2..],
        ["exit=1", "exit=2"],
        "\n{outcome}"
    );
    assert!(
        outcome
            .stderr
            .contains("paddock: redis: Redis 8.2 is installed; going back to 7.4 isn't supported"),
        "\n{outcome}"
    );
    assert!(
        outcome.stderr.contains(
            "paddock: redis: Redis 9.9 isn't available (available: 6.0 6.2 7.0 7.2 7.4 8.0 8.2"
        ),
        "\n{outcome}"
    );
}

#[test]
fn refuses_to_install_next_to_valkey() {
    let outcome = harness::run(
        "paddock setup valkey 9.1.2 >/dev/null\npaddock setup redis 8.2 || echo \"exit=$?\"",
    );
    assert!(outcome.stdout.ends_with("exit=1\n"), "\n{outcome}");
    assert!(
        outcome.stderr.contains(
            "paddock: redis: Valkey is installed and uses the same port; remove it before setting up Redis"
        ),
        "\n{outcome}"
    );
}
