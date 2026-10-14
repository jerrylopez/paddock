//! `paddock setup valkey`.

use crate::harness;

#[test]
fn runs_under_systemd_with_a_password_and_keeps_data_across_a_restart() {
    let container = harness::SystemdContainer::start();
    let outcome = container.setup_twice_then_verify(
        "valkey",
        "9.1",
        &["--password", "secret"],
        "systemctl is-active valkey-server\n\
         valkey-cli ping 2>&1 | grep -o NOAUTH\n\
         VALKEYCLI_AUTH=secret valkey-cli set greeting hello\n\
         VALKEYCLI_AUTH=secret valkey-cli info server | grep -o 'valkey_version:9\\.1\\.'",
    );
    // 9.1.2 was the latest 9.1 release when this was written; newer patches count too.
    let first: Vec<_> = outcome.section("setup (first run)").lines().collect();
    assert_eq!(first.len(), 2, "\n{outcome}");
    let release = first[0]
        .strip_prefix("paddock: valkey: Installing Valkey ")
        .unwrap_or_else(|| panic!("\n{outcome}"));
    assert!(
        release.starts_with("9.1.") && release != "9.1.0",
        "should pick the latest 9.1 patch\n{outcome}"
    );
    let ready = format!("paddock: valkey: Valkey {release} is running on 127.0.0.1:6379");
    assert_eq!(first[1], ready, "\n{outcome}");
    assert_eq!(
        outcome.section("setup (second run)"),
        format!("{ready}\n"),
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("verify"),
        "active\nNOAUTH\nOK\nvalkey_version:9.1.\n",
        "\n{outcome}"
    );

    container.restart();
    let after = container.run(
        "for i in $(seq 30); do VALKEYCLI_AUTH=secret valkey-cli ping 2>/dev/null | grep -q PONG && break; sleep 1; done\n\
         VALKEYCLI_AUTH=secret valkey-cli get greeting",
    );
    after.assert_success();
    assert_eq!(after.stdout, "hello\n", "\n{after}");
}

#[test]
fn upgrades_and_refuses_to_go_back() {
    let outcome = harness::run(
        "paddock setup valkey 9.0\n\
         paddock setup valkey 9.1\n\
         valkey-server --version | grep -o 'v=9\\.1\\.'\n\
         paddock setup valkey 9.0 || echo \"exit=$?\"",
    );
    outcome.assert_success();
    let lines: Vec<_> = outcome.stdout.lines().collect();
    assert!(
        lines.iter().any(
            |line| line.starts_with("paddock: valkey: Upgrading Valkey 9.0.")
                && line.contains(" to 9.1.")
        ),
        "\n{outcome}"
    );
    assert!(lines.contains(&"v=9.1."), "\n{outcome}");
    assert_eq!(lines.last(), Some(&"exit=1"), "\n{outcome}");
    assert!(
        outcome
            .stderr
            .contains(" is installed; going back to 9.0 isn't supported"),
        "\n{outcome}"
    );
}

#[test]
fn rejects_unpublished_versions() {
    let outcome = harness::run("paddock setup valkey 99.0 || echo \"exit=$?\"");
    assert!(outcome.stdout.ends_with("exit=2\n"), "\n{outcome}");
    assert!(
        outcome
            .stderr
            .contains("paddock: valkey: Valkey 99.0 isn't published"),
        "\n{outcome}"
    );
}
