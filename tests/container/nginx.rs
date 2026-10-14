//! `paddock setup nginx`.

use crate::harness;

const WELCOME: &str = "curl -fsS http://localhost/ | grep -m1 -o 'Welcome to nginx!'";

#[test]
fn serves_the_default_site_under_systemd_and_survives_a_restart() {
    let container = harness::SystemdContainer::start();
    // Not the newest stable release, so the pin has to pick it.
    let outcome = container.setup_twice_then_verify(
        "nginx",
        "1.30.4",
        &[],
        &format!(
            "systemctl is-active nginx\n\
             nginx -v 2>&1\n\
             apt-get install -y -qq curl >/dev/null 2>&1\n\
             {WELCOME}"
        ),
    );
    assert_eq!(
        outcome.section("setup (first run)"),
        "paddock: nginx: Installing nginx 1.30.4\npaddock: nginx: nginx 1.30.4 is running on port 80\n",
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("setup (second run)"),
        "paddock: nginx: nginx 1.30.4 is running on port 80\n",
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("verify"),
        "active\nnginx version: nginx/1.30.4\nWelcome to nginx!\n",
        "\n{outcome}"
    );

    container.restart();
    let after = container.run(&format!("systemctl is-active nginx\n{WELCOME}"));
    after.assert_success();
    assert_eq!(after.stdout, "active\nWelcome to nginx!\n", "\n{after}");
}

#[test]
fn switches_releases_across_channels() {
    // 1.31.x comes from the mainline repository, 1.30.x from stable; going
    // from 1.31.5 to 1.30.5 is a downgrade.
    let outcome = harness::run(
        "paddock setup nginx 1.31.5\n\
         nginx -v 2>&1\n\
         paddock setup nginx 1.30.5\n\
         nginx -v 2>&1\n\
         paddock setup nginx 1.30.99 || echo \"exit=$?\"\n\
         paddock setup nginx stable || echo \"exit=$?\"",
    );
    outcome.assert_success();
    let lines: Vec<_> = outcome.stdout.lines().collect();
    assert_eq!(
        lines,
        [
            "paddock: nginx: Installing nginx 1.31.5",
            "paddock: nginx: nginx 1.31.5 is installed",
            "nginx version: nginx/1.31.5",
            "paddock: nginx: Switching nginx 1.31.5 to 1.30.5",
            "paddock: nginx: nginx 1.30.5 is installed",
            "nginx version: nginx/1.30.5",
            "exit=2",
            "exit=2",
        ],
        "\n{outcome}"
    );
    assert!(
        outcome
            .stderr
            .contains("paddock: nginx: nginx 1.30.99 isn't published (available: "),
        "\n{outcome}"
    );
    assert!(
        outcome.stderr.contains(
            "paddock: nginx: version must be an nginx release like 1.30.5 (got 'stable')"
        ),
        "\n{outcome}"
    );
}
