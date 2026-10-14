//! `paddock setup redis`.

use crate::harness;

#[test]
fn runs_under_systemd_with_a_password_and_keeps_data_across_a_restart() {
    let container = harness::SystemdContainer::start();
    // Not the newest 8.10 release, so the pin has to pick it.
    let outcome = container.setup_twice_then_verify(
        "redis",
        "8.10.1",
        &["--password", "secret"],
        "systemctl is-active redis-server\n\
         redis-cli ping 2>&1 | grep -o NOAUTH\n\
         REDISCLI_AUTH=secret redis-cli set greeting hello\n\
         REDISCLI_AUTH=secret redis-cli info server | grep -o 'redis_version:[0-9.]*'",
    );
    assert_eq!(
        outcome.section("setup (first run)"),
        "paddock: redis: Installing Redis 8.10.1\npaddock: redis: Redis 8.10.1 is running on 127.0.0.1:6379\n",
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("setup (second run)"),
        "paddock: redis: Redis 8.10.1 is running on 127.0.0.1:6379\n",
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("verify"),
        "active\nNOAUTH\nOK\nredis_version:8.10.1\n",
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
fn upgrades_and_refuses_to_go_back() {
    let outcome = harness::run(
        "paddock setup redis 7.4.10\n\
         paddock setup redis 8.2.9\n\
         redis-server --version | grep -o 'v=[0-9.]*'\n\
         paddock setup redis 7.4.10 || echo \"exit=$?\"\n\
         paddock setup redis 8.2.99 || echo \"exit=$?\"\n\
         paddock setup redis 8.2 || echo \"exit=$?\"",
    );
    outcome.assert_success();
    let lines: Vec<_> = outcome.stdout.lines().collect();
    assert!(
        lines.contains(&"paddock: redis: Upgrading Redis 7.4.10 to 8.2.9"),
        "\n{outcome}"
    );
    assert!(lines.contains(&"v=8.2.9"), "\n{outcome}");
    assert_eq!(
        lines[lines.len() - 3..],
        ["exit=1", "exit=2", "exit=2"],
        "\n{outcome}"
    );
    for expected in [
        "paddock: redis: Redis 8.2.9 is installed; going back to 7.4.10 isn't supported",
        "paddock: redis: Redis 8.2.99 isn't published (available: 8.2.",
        "paddock: redis: version must be a Redis release like 8.2.3 (got '8.2')",
    ] {
        assert!(outcome.stderr.contains(expected), "{expected}\n{outcome}");
    }
}

#[test]
fn refuses_to_install_next_to_valkey() {
    let outcome = harness::run(
        "paddock setup valkey 9.1.2 >/dev/null\npaddock setup redis 8.2.9 || echo \"exit=$?\"",
    );
    assert!(outcome.stdout.ends_with("exit=1\n"), "\n{outcome}");
    assert!(
        outcome.stderr.contains(
            "paddock: redis: Valkey is installed and uses the same port; remove it before setting up Redis"
        ),
        "\n{outcome}"
    );
}
