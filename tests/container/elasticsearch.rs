//! `paddock setup elasticsearch`.

use crate::harness;

/// Prints the major.minor version, then indexes a document and prints the count.
const CHECK: &str = "curl -fsS http://localhost:9200 | grep -o '\"number\" : \"[0-9]*\\.[0-9]*' | grep -o '[0-9.]*$'\n\
    curl -fsS -X PUT 'http://localhost:9200/paddock/_doc/1?refresh=true' \
      -H 'Content-Type: application/json' -d '{\"ok\": true}' >/dev/null\n\
    curl -fsS http://localhost:9200/paddock/_count | grep -o '\"count\":[0-9]*'";

fn runs_under_systemd(series: &str) {
    let container = harness::SystemdContainer::start();
    let outcome = container.setup_twice_then_verify(
        "elasticsearch",
        series,
        &[],
        &format!("systemctl is-active elasticsearch\n{CHECK}"),
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(
        rerun[0].starts_with(&format!("paddock: elasticsearch: Elasticsearch {series}."))
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
        "systemctl is-active elasticsearch\n\
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
fn version_8_runs_under_systemd_and_keeps_data_across_a_restart() {
    runs_under_systemd("8.19");
}

#[test]
fn version_9_runs_under_systemd_and_keeps_data_across_a_restart() {
    runs_under_systemd("9.5");
}
