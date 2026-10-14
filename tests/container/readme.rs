//! The README's quick start, run for real so the docs can't drift.

use crate::harness;

const README: &str = include_str!("../../README.md");

/// The `paddock setup` lines of the README's quick-start `.agents/setup`.
fn quick_start_lines() -> Vec<&'static str> {
    let section = README
        .split("## Quick start")
        .nth(1)
        .expect("README has a Quick start section");
    let script = section
        .split("```bash")
        .nth(1)
        .and_then(|rest| rest.split("```").next())
        .expect("Quick start has a bash block");
    assert!(
        script.contains("/releases/download/v") && script.contains("/install.sh | bash"),
        "the quick start installs a pinned release"
    );
    script
        .lines()
        .filter(|line| line.starts_with("paddock setup "))
        .collect()
}

#[test]
fn quick_start_sets_up_an_orb() {
    let lines = quick_start_lines();
    assert!(
        !lines.is_empty(),
        "no `paddock setup` lines in the quick start"
    );
    // Like an orb's .agents/setup: as a sudo user, then again on the set-up machine.
    let setup = lines.join(" && ");
    let script = format!(
        "su dev -c '{setup}'\n\
         echo '== rerun'\n\
         su dev -c '{setup}'"
    );
    let container = harness::SystemdContainer::start();
    let outcome = container.run(&script);
    outcome.assert_success();
    let rerun: Vec<_> = outcome.section("rerun").lines().collect();
    assert_eq!(
        rerun.len(),
        lines.len(),
        "each service should print one line on a rerun\n{outcome}"
    );
}
