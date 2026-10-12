# paddock

Native environment setup for Amp orbs.

## Development

```sh
cargo run -- setup php 8.3   # build and try a command by hand
cargo test                   # unit tests + binary tests (fast, no Docker)
cargo container-tests       # container tests (needs Docker)
```

Container tests build `tests/Dockerfile`, which compiles Paddock in the official Rust image
and copies the binary into a fresh base image. Every test command runs in a new container.
The base image defaults to `debian:bookworm-slim`; override it with `PADDOCK_TEST_BASE_IMAGE`:

```sh
PADDOCK_TEST_BASE_IMAGE=ubuntu:24.04 cargo container-tests
```

To test a service, call `harness::setup_twice_then_verify` from a test in `tests/`. It runs
`paddock setup` twice in one container (the second run stands in for a stale snapshot) and
then runs your verification command, e.g. `php -v | grep -q 'PHP 8.3'`. Container test
targets need `required-features = ["container-tests"]` in `Cargo.toml`, like `container`.

In an Amp orb, Docker runs as an orb service declared in `.amp/services.yaml`; start it with
`amp orb service ensure`.
