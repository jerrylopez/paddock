//! `paddock setup nginx`.

use crate::harness;

const WELCOME: &str = "curl -fsS http://localhost/ | grep -m1 -o 'Welcome to nginx!'";

#[test]
fn serves_the_default_site_under_systemd_and_survives_a_restart() {
    let container = harness::SystemdContainer::start();
    let outcome = container.setup_twice_then_verify(
        "nginx",
        "1.30",
        &[],
        &format!(
            "systemctl is-active nginx\n\
             nginx -v 2>&1 | grep -o 'nginx/1\\.30\\.'\n\
             apt-get install -y -qq curl >/dev/null 2>&1\n\
             {WELCOME}"
        ),
    );
    assert_eq!(
        outcome.section("setup (first run)").lines().next(),
        Some("paddock: nginx: Installing nginx 1.30"),
        "\n{outcome}"
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(
        rerun[0].starts_with("paddock: nginx: nginx 1.30.")
            && rerun[0].ends_with(" is running on port 80"),
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("verify"),
        "active\nnginx/1.30.\nWelcome to nginx!\n",
        "\n{outcome}"
    );

    container.restart();
    let after = container.run(&format!("systemctl is-active nginx\n{WELCOME}"));
    after.assert_success();
    assert_eq!(after.stdout, "active\nWelcome to nginx!\n", "\n{after}");
}

#[test]
fn switches_series_across_channels() {
    // 1.31 comes from the mainline repository, 1.30 from stable; going from
    // 1.31 to 1.30 is a downgrade.
    let outcome = harness::run(
        "paddock setup nginx 1.31\n\
         nginx -v 2>&1\n\
         paddock setup nginx 1.30\n\
         nginx -v 2>&1\n\
         paddock setup nginx 1.99 || echo \"exit=$?\"\n\
         paddock setup nginx stable || echo \"exit=$?\"",
    );
    outcome.assert_success();
    let lines: Vec<_> = outcome.stdout.lines().collect();
    let versions: Vec<_> = lines
        .iter()
        .filter_map(|line| line.strip_prefix("nginx version: nginx/"))
        .collect();
    assert_eq!(versions.len(), 2, "\n{outcome}");
    assert!(
        versions[0].starts_with("1.31.") && versions[1].starts_with("1.30."),
        "\n{outcome}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("paddock: nginx: Switching nginx 1.31 to 1.30")),
        "\n{outcome}"
    );
    assert_eq!(
        lines[lines.len() - 2..],
        ["exit=2", "exit=2"],
        "\n{outcome}"
    );
    assert!(
        outcome
            .stderr
            .contains("paddock: nginx: nginx 1.99 isn't available (available: "),
        "\n{outcome}"
    );
    assert!(
        outcome
            .stderr
            .contains("paddock: nginx: version must be an nginx series like 1.30 (got 'stable')"),
        "\n{outcome}"
    );
}
