//! `paddock setup typesense`.

use crate::harness;

/// Creates a collection with the API key, then lists collection names.
const CHECK: &str = "curl -fsS http://localhost:8108/health\necho\n\
    curl -fsS -X POST http://localhost:8108/collections -H 'X-TYPESENSE-API-KEY: secret' \
      -H 'Content-Type: application/json' \
      -d '{\"name\": \"books\", \"fields\": [{\"name\": \"title\", \"type\": \"string\"}]}' >/dev/null\n\
    curl -s -o /dev/null -w '%{http_code}\\n' http://localhost:8108/collections -H 'X-TYPESENSE-API-KEY: wrong'";

#[test]
fn runs_under_systemd_with_the_api_key_and_survives_a_restart() {
    let container = harness::SystemdContainer::start();
    let outcome = container.setup_twice_then_verify(
        "typesense",
        "30.2",
        &["--api-key", "secret"],
        &format!("systemctl is-active typesense-server\n{CHECK}"),
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(
        rerun,
        [
            "paddock: typesense: Typesense 30.2 is running on http://localhost:8108 (API key in /etc/typesense/typesense-server.ini)"
        ],
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("verify"),
        "active\n{\"ok\":true}\n401\n",
        "\n{outcome}"
    );

    container.restart();
    let after = container.run(
        "for i in $(seq 60); do curl -fsS http://localhost:8108/health >/dev/null 2>&1 && break; sleep 1; done\n\
         curl -fsS http://localhost:8108/collections/books -H 'X-TYPESENSE-API-KEY: secret' | grep -o '\"name\":\"books\"'",
    );
    after.assert_success();
    assert_eq!(after.stdout, "\"name\":\"books\"\n", "\n{after}");
}

#[test]
fn upgrades_keeping_the_api_key_and_refuses_to_go_back() {
    let outcome = harness::run(
        "paddock setup typesense 30.1\n\
         grep '^api-key' /etc/typesense/typesense-server.ini >/tmp/before\n\
         paddock setup typesense 30.2\n\
         grep '^api-key' /etc/typesense/typesense-server.ini | cmp - /tmp/before && echo 'same key'\n\
         dpkg-query -W -f='${Version}\\n' typesense-server\n\
         paddock setup typesense 30.1 || echo \"exit=$?\"",
    );
    outcome.assert_success();
    let lines: Vec<_> = outcome.stdout.lines().collect();
    assert!(
        lines.contains(&"paddock: typesense: Upgrading Typesense 30.1 to 30.2"),
        "\n{outcome}"
    );
    assert!(lines.contains(&"same key"), "\n{outcome}");
    assert!(lines.contains(&"30.2"), "\n{outcome}");
    assert_eq!(lines.last(), Some(&"exit=1"), "\n{outcome}");
    assert!(
        outcome.stderr.contains(
            "paddock: typesense: Typesense 30.2 is installed; going back to 30.1 isn't supported"
        ),
        "\n{outcome}"
    );
}

#[test]
fn rejects_unpublished_versions() {
    let outcome = harness::run("paddock setup typesense 99.9 || echo \"exit=$?\"");
    assert!(outcome.stdout.ends_with("exit=2\n"), "\n{outcome}");
    assert!(
        outcome
            .stderr
            .contains("paddock: typesense: Typesense 99.9 isn't published"),
        "\n{outcome}"
    );
}
