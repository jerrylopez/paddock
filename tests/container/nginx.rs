//! `paddock setup nginx`.

use crate::harness;

const WELCOME: &str = "curl -fsS http://localhost/ | grep -m1 -o 'Welcome to nginx!'";

#[test]
fn serves_the_default_site_under_systemd_and_survives_a_restart() {
    let container = harness::SystemdContainer::start();
    let outcome = container.setup_twice_then_verify(
        "nginx",
        "stable",
        &[],
        &format!(
            "systemctl is-active nginx\n\
             apt-get install -y -qq curl >/dev/null 2>&1\n\
             {WELCOME}"
        ),
    );
    assert_eq!(
        outcome.section("setup (first run)").lines().next(),
        Some("paddock: nginx: Installing nginx (stable)"),
        "\n{outcome}"
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(
        rerun[0].starts_with("paddock: nginx: nginx 1.")
            && rerun[0].ends_with(" (stable) is running on port 80"),
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("verify"),
        "active\nWelcome to nginx!\n",
        "\n{outcome}"
    );

    container.restart();
    let after = container.run(&format!("systemctl is-active nginx\n{WELCOME}"));
    after.assert_success();
    assert_eq!(after.stdout, "active\nWelcome to nginx!\n", "\n{after}");
}

#[test]
fn switches_between_mainline_and_stable() {
    // Mainline has the higher version number, so going back to stable is a downgrade.
    let outcome = harness::run(
        "paddock setup nginx mainline\n\
         nginx -v 2>&1\n\
         paddock setup nginx stable\n\
         nginx -v 2>&1\n\
         paddock setup nginx stable",
    );
    outcome.assert_success();
    let versions: Vec<u32> = outcome
        .stdout
        .lines()
        .filter_map(|line| line.strip_prefix("nginx version: nginx/1."))
        .map(|rest| rest.split('.').next().unwrap().parse().unwrap())
        .collect();
    assert_eq!(versions.len(), 2, "\n{outcome}");
    assert!(
        versions[0] > versions[1],
        "mainline should be newer\n{outcome}"
    );
    assert!(
        outcome
            .stdout
            .contains("paddock: nginx: Switching nginx to stable\n"),
        "\n{outcome}"
    );
    assert!(
        outcome.stdout.ends_with(" (stable) is installed\n"),
        "\n{outcome}"
    );
}
