//! `paddock setup varnish`.

use crate::harness;

#[test]
fn caches_the_backend_under_systemd_and_survives_a_restart() {
    let container = harness::SystemdContainer::start();
    let outcome = container.setup_twice_then_verify(
        "varnish",
        "8.0",
        &["--backend", "127.0.0.1:8080"],
        "systemctl is-active varnish\n\
         curl -s -o /dev/null -w '%{http_code}\\n' http://127.0.0.1:6081/\n\
         apt-get install -y -qq busybox >/dev/null 2>&1\n\
         mkdir -p /tmp/www && echo 'hello from the backend' >/tmp/www/index.html\n\
         busybox httpd -p 127.0.0.1:8080 -h /tmp/www\n\
         curl -fsS http://127.0.0.1:6081/index.html\n\
         curl -fsS -D - -o /dev/null http://127.0.0.1:6081/index.html | grep -i '^age:' | grep -vq 'age: 0' && echo cached || \
           { sleep 2; curl -fsS -D - -o /dev/null http://127.0.0.1:6081/index.html | grep -iq '^age: [1-9]' && echo cached; }",
    );
    assert_eq!(
        outcome.section("setup (first run)").lines().next(),
        Some("paddock: varnish: Installing Varnish 8.0"),
        "\n{outcome}"
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(
        rerun[0].starts_with("paddock: varnish: Varnish 8.0.")
            && rerun[0].ends_with(" is running on 127.0.0.1:6081, caching 127.0.0.1:8080"),
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("verify"),
        "active\n503\nhello from the backend\ncached\n",
        "\n{outcome}"
    );

    container.restart();
    let after = container.run(
        "for i in $(seq 30); do curl -s -o /dev/null http://127.0.0.1:6081/ && break; sleep 1; done\n\
         systemctl is-active varnish\n\
         curl -s -o /dev/null -w '%{http_code}\\n' http://127.0.0.1:6081/",
    );
    after.assert_success();
    assert_eq!(after.stdout, "active\n503\n", "\n{after}");
}

#[test]
fn switches_series_including_back_to_the_lts() {
    // Debian's own Varnish (7.1) is newer than 6.0; the pin keeps the repository's.
    let outcome = harness::run(
        "paddock setup varnish 8.0\n\
         paddock setup varnish 6.0\n\
         varnishd -V 2>&1 | grep -o 'varnish-[0-9.]*'",
    );
    outcome.assert_success();
    let lines: Vec<_> = outcome.stdout.lines().collect();
    assert!(
        lines.contains(&"paddock: varnish: Switching Varnish to 6.0"),
        "\n{outcome}"
    );
    assert!(
        lines.last().unwrap().starts_with("varnish-6.0."),
        "\n{outcome}"
    );
}
