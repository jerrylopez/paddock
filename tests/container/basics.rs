//! Smoke test and `common.sh` helpers that need a real Debian system.

use crate::harness;

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

#[test]
fn apt_source_changes_trigger_one_more_update() {
    // Count apt-get calls by putting a logging wrapper in front of the real one.
    let outcome = harness::run_service_script(
        r#"
mkdir -p /tmp/count-bin
printf '#!/bin/sh\nfor arg; do case $arg in update|install) echo "$arg" >>/tmp/apt-calls; break;; esac; done\nexec /usr/bin/apt-get "$@"\n' >/tmp/count-bin/apt-get
chmod +x /tmp/count-bin/apt-get
export PATH="/tmp/count-bin:$PATH"
line="deb [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] http://deb.debian.org/debian bookworm-backports main"
apt_install jq                # update, install
apt_source extra "$line"      # new source: lists are stale
apt_install file              # update, install
apt_source extra "$line"      # unchanged: lists still fresh
apt_install less              # install only
paste -sd ' ' /tmp/apt-calls
"#,
        &["1.0"],
        &[],
    );
    outcome.assert_success();
    assert_eq!(
        outcome.stdout, "update install update install install\n",
        "\n{outcome}"
    );
}

#[test]
fn apt_keyring_checks_fingerprints() {
    let outcome = harness::run_service_script(
        r#"
url=https://nginx.org/keys/nginx_signing.key
if (apt_keyring wrong "$url" 0000000000000000000000000000000000000000) 2>/tmp/err; then
  echo "wrong fingerprint accepted"
fi
[ -f /etc/apt/keyrings/wrong.gpg ] && echo "wrong keyring installed"
grep -o "isn't one of the expected keys" /tmp/err
apt_keyring nginx "$url" 573BFD6B3D8FBC641079A6ABABF5BD827BD9BF62 \
  8540A6F18833A80E9C1653A42FD21310B49F6B46 9E9BE90EACBCDE69FE9B204CBCDCD8A38D88A2B3
gpg --show-keys --with-colons /etc/apt/keyrings/nginx.gpg 2>/dev/null | grep -c '^pub'
"#,
        &["1.0"],
        &[],
    );
    outcome.assert_success();
    assert_eq!(
        outcome.stdout, "isn't one of the expected keys\n3\n",
        "\n{outcome}"
    );
}
