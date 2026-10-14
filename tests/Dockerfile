# syntax=docker/dockerfile:1
#
# Images for Paddock's container tests (tests/container). Build context is the repo root.
#
# `build` compiles Paddock inside the official Rust image, so the host OS doesn't matter.
# It targets musl, giving a static binary that runs on any base image.
# `plain` is a fresh base image that receives only the binary; most tests run in it.
# `systemd` adds systemd as PID 1, like an orb, for tests of daemons that systemd starts.
# It's run with extra privileges (see SystemdContainer in tests/container/harness.rs).

ARG BASE_IMAGE=debian:bookworm-slim

FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release && cp target/release/paddock /paddock

FROM ${BASE_IMAGE} AS systemd
RUN apt-get update -qq \
    && apt-get install -y -qq --no-install-recommends systemd systemd-sysv dbus >/dev/null \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /paddock /usr/local/bin/paddock
STOPSIGNAL SIGRTMIN+3
CMD ["/lib/systemd/systemd"]

FROM ${BASE_IMAGE} AS plain
COPY --from=build /paddock /usr/local/bin/paddock
