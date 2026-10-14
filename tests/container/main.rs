//! Container tests: run the real binary in fresh Linux containers.
//! Not built by plain `cargo test`; run with `cargo container-tests`, or one
//! module with e.g. `cargo container-tests php::`.
//!
//! Each module covers one service and runs as its own CI job, so add new
//! service tests as a new module.

mod harness;

mod basics;
mod mysql;
mod php;
