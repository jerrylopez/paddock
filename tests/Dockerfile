# syntax=docker/dockerfile:1
#
# Image for Paddock's container tests (tests/harness). Build context is the repo root.
#
# Stage 1 compiles Paddock inside the official Rust image, so the host OS doesn't matter.
# It targets musl, giving a static binary that runs on any base image in stage 2.
# Stage 2 is a fresh base image that receives only the binary; tests run in it.

ARG BASE_IMAGE=debian:bookworm-slim

FROM rust:1-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release && cp target/release/paddock /paddock

FROM ${BASE_IMAGE}
COPY --from=build /paddock /usr/local/bin/paddock
