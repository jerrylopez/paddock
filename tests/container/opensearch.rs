//! `paddock setup opensearch`.

use crate::harness;

/// Prints the major.minor version, then indexes a document and prints the count.
const CHECK: &str = "curl -fsS http://localhost:9200 | grep -o '\"number\" : \"[0-9]*\\.[0-9]*' | grep -o '[0-9.]*$'\n\
    curl -fsS -X PUT 'http://localhost:9200/paddock/_doc/1?refresh=true' \
      -H 'Content-Type: application/json' -d '{\"ok\": true}' >/dev/null\n\
    curl -fsS http://localhost:9200/paddock/_count | grep -o '\"count\":[0-9]*'";

fn runs_under_systemd(series: &str) {
    let container = harness::SystemdContainer::start();
    let outcome = container.setup_twice_then_verify(
        "opensearch",
        series,
        &[],
        &format!("systemctl is-active opensearch\n{CHECK}"),
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(
        rerun[0].starts_with(&format!("paddock: opensearch: OpenSearch {series}."))
            && rerun[0]
                .ends_with(" is running on http://localhost:9200 (heap 512m, security disabled)"),
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("verify"),
        format!("active\n{series}\n\"count\":1\n"),
        "\n{outcome}"
    );

    container.restart();
    let after = container.run(
        "systemctl is-active opensearch\n\
         for i in $(seq 120); do\n\
           curl -fsS 'http://localhost:9200/_cluster/health?wait_for_status=yellow&timeout=1s' >/dev/null 2>&1 && break\n\
           sleep 1\n\
         done\n\
         curl -fsS http://localhost:9200/paddock/_count | grep -o '\"count\":[0-9]*'",
    );
    after.assert_success();
    assert_eq!(after.stdout, "active\n\"count\":1\n", "\n{after}");
}

#[test]
fn version_1_2_from_its_tarball_runs_under_systemd_and_keeps_data_across_a_restart() {
    runs_under_systemd("1.2");
}

#[test]
fn version_2_runs_under_systemd_and_keeps_data_across_a_restart() {
    runs_under_systemd("2.19");
}

#[test]
fn version_3_runs_under_systemd_and_keeps_data_across_a_restart() {
    runs_under_systemd("3.9");
}

#[test]
fn upgrades_a_tarball_install_to_the_package_and_keeps_its_data() {
    let container = harness::SystemdContainer::start();
    let outcome = container.run(&format!(
        "su dev -c 'paddock setup opensearch 1.2'\n\
         {CHECK}\n\
         su dev -c 'paddock setup opensearch 1.3'\n\
         systemctl is-active opensearch\n\
         curl -fsS http://localhost:9200 | grep -o '\"number\" : \"[0-9]*\\.[0-9]*' | grep -o '[0-9.]*$'\n\
         curl -fsS http://localhost:9200/paddock/_count | grep -o '\"count\":[0-9]*'\n\
         ls /opt/opensearch /etc/systemd/system/opensearch.service 2>&1 | grep -c 'No such file'"
    ));
    outcome.assert_success();
    let lines: Vec<_> = outcome.stdout.lines().collect();
    assert_eq!(
        lines,
        [
            "paddock: opensearch: Installing OpenSearch 1.2",
            "paddock: opensearch: OpenSearch 1.2.4 is running on http://localhost:9200 (heap 512m, security disabled)",
            "1.2",
            "\"count\":1",
            "paddock: opensearch: Upgrading OpenSearch 1.2 to 1.3",
            lines[5],
            "active",
            "1.3",
            "\"count\":1",
            "2",
        ],
        "\n{outcome}"
    );
    assert!(
        lines[5].starts_with("paddock: opensearch: OpenSearch 1.3."),
        "\n{outcome}"
    );
}
