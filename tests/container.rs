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
