//! Container tests: run the real binary in a fresh Linux container.
//! Not built by plain `cargo test`; run with `cargo container-tests`.

mod harness;

#[test]
fn paddock_runs_in_a_fresh_container() {
    let outcome = harness::run("paddock --version");
    outcome.assert_success();
    assert_eq!(
        outcome.stdout.trim(),
        concat!("paddock ", env!("CARGO_PKG_VERSION")),
        "\n{outcome}"
    );
}

#[test]
fn apt_install_installs_missing_packages_then_skips_them() {
    let outcome = harness::run_service_script(
        r#"
apt_install jq
jq --version
# A second call must not touch apt: shadow apt-get with one that fails.
mkdir -p /tmp/fake-bin
printf '#!/bin/sh\necho "apt-get should not run" >&2\nexit 99\n' >/tmp/fake-bin/apt-get
chmod +x /tmp/fake-bin/apt-get
export PATH="/tmp/fake-bin:$PATH"
apt_install jq
echo second call skipped apt
"#,
        &["1.0"],
        &[],
    );
    outcome.assert_success();
    assert!(outcome.stdout.starts_with("jq-"), "\n{outcome}");
    assert!(
        outcome.stdout.ends_with("\nsecond call skipped apt\n"),
        "\n{outcome}"
    );
    assert_eq!(outcome.stderr, "", "apt output should be hidden\n{outcome}");
}

#[test]
fn apt_install_without_root_fails_clearly() {
    let outcome = harness::run_service_script("apt_install jq", &["1.0"], &["--user", "1000:1000"]);
    assert_eq!(outcome.code, Some(1), "\n{outcome}");
    assert_eq!(
        outcome.stderr,
        "paddock: demo: needs root: run as root or install sudo\n"
    );
}
