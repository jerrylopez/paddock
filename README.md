# paddock

Native environment setup for Amp orbs.

## Services

### PHP

```sh
paddock setup php 8.3
paddock setup php 8.4 --extensions redis,pdo_mysql
```

Installs the PHP CLI on Debian from [packages.sury.org](https://packages.sury.org/php/), Ondřej
Surý's repository (the Debian counterpart of his Ubuntu PPA), and
[Composer](https://getcomposer.org) as `/usr/local/bin/composer`.

- The version is major.minor (5.6 to 8.6 today); the repository provides the latest patch.
- Default extensions: amqp, bcmath, curl, gd, gmp, imagick, intl, json, mbstring, msgpack,
  mysqlnd, opcache, pcntl, pdo, posix, redis, shmop, soap, sodium, sysvmsg, sysvsem, sysvshm,
  xml, xmlrpc, zip. Defaults not yet packaged for a version (e.g. redis on 8.6) are skipped with
  a warning.
- `--extensions` adds to the defaults, using the names `php -m` shows. Extensions you ask for
  this way must be available.
- Composer is the latest 2.x release, or the 2.2 LTS line for PHP older than 7.2.5. Its download
  is checked against the SHA-256 checksum getcomposer.org publishes.
- With several versions installed, `php` runs the one set up most recently.

### PHP-FPM

```sh
paddock setup php-fpm 8.3
paddock setup php-fpm 8.4 --extensions xdebug
```

Everything `paddock setup php` does, plus PHP-FPM for the same version, enabled and started
under systemd as `php<version>-fpm`. It listens on `/run/php/php<version>-fpm.sock`, which the
ready line prints. Extensions are shared with the CLI, except the CLI-only `pcntl` and
`readline`. Without systemd, PHP-FPM is installed and its config checked, but not started.

### MySQL

```sh
paddock setup mysql 8.4
paddock setup mysql 8.0 --database app --username app --password secret
```

Installs MySQL Community Server from [Oracle's APT repository](https://repo.mysql.com/apt/debian/).
The download of Oracle's signing key is checked against its fingerprint.

- The version picks a release series: `8.0`, `8.4`, `9.7` or `innovation`; the repository provides
  the latest release in it. Switching an existing install to another series isn't supported.
- `--database` creates a database. `--username` and `--password` create a user, with full access to
  that database when `--database` is given.
- Root logs in through the Unix socket (`sudo mysql`) unless you pass `--root-password`. Once root
  has a password, later runs that change the database or users need `--root-password` too.
- With systemd (orbs, servers), MySQL is enabled and started, and systemd starts it again after every
  boot or orb wake. Without systemd (plain containers), it's installed and configured but not left
  running.
- Oracle publishes Debian packages for amd64 and i386 only, so ARM machines aren't supported.

### nginx

```sh
paddock setup nginx stable
paddock setup nginx mainline
```

Installs nginx from [nginx.org's repository](https://nginx.org/en/linux_packages.html). The
version is one of its two channels, `stable` or `mainline`; the repository provides the latest
release in it, and running setup with the other channel switches to it. nginx.org's packages
take priority over Debian's own nginx. The default site answers on port 80. Under systemd,
nginx is enabled and started. Wiring nginx to PHP-FPM is left to presets.

### OpenSearch

```sh
paddock setup opensearch 3
paddock setup opensearch 2 --heap 1g
```

Installs OpenSearch from [its APT repository](https://opensearch.org/docs/latest/install-and-configure/install-opensearch/debian/).
The version is the major (`2` or `3`); the repository provides the latest release in it.

- Configured for local development: one node on `127.0.0.1:9200`, security plugin disabled (no
  TLS or passwords), JVM heap 512m unless `--heap` says otherwise. Paddock manages
  `/etc/opensearch/opensearch.yml` and restarts OpenSearch when it changes.
- Under systemd, OpenSearch is enabled and started, and setup waits until the cluster is usable.
- Switching an install to another major isn't supported, and it refuses to install next to
  Elasticsearch, which uses the same port.

### Typesense

```sh
paddock setup typesense 30.2
paddock setup typesense 30.2 --api-key secret
```

Installs Typesense from [its official `.deb` packages](https://typesense.org/docs/guide/install-typesense.html).
There's no APT repository, so the version is an exact release. Upgrading is supported; going
back to an older version isn't.

- Listens on `127.0.0.1:8108`. The API key is `--api-key` if given, otherwise the key already
  configured, otherwise a generated one. It's in `/etc/typesense/typesense-server.ini`, readable
  by root only.
- Under systemd, Typesense is enabled and started, and setup waits for `/health`.
- Typesense publishes no checksums, so the download is trusted on HTTPS from dl.typesense.org,
  as in its own install instructions.

## Adding a service

1. Write `scripts/<name>.sh` following the [install-script contract](scripts/README.md):
   safe to re-run, short output, sensible defaults, non-zero exit on failure. Paddock runs it
   with bash after `scripts/common.sh`, which provides helpers for each of those rules.
2. Register it in `SERVICES` in `src/dispatch.rs`: `service!("<name>", "<one-line description>")`.
   The script is embedded into the binary at compile time, so nothing is fetched at runtime.

## Development

```sh
cargo run -- setup php 8.3   # build and try a command by hand
cargo test                   # unit, binary and installer tests (fast, no Docker)
cargo container-tests       # container tests (needs Docker)
```

Container tests build `tests/Dockerfile`, which compiles Paddock in the official Rust image
and copies the binary into a fresh base image. Every test command runs in a new container.
The base image defaults to `debian:bookworm-slim`; override it with `PADDOCK_TEST_BASE_IMAGE`:

```sh
PADDOCK_TEST_BASE_IMAGE=ubuntu:24.04 cargo container-tests
```

Tests live in `tests/container/`, one module per service (`php.rs`, `mysql.rs`, ...); CI runs
each module as its own job. Run one module with `cargo container-tests -- php::`.

To test a service, call `harness::setup_twice_then_verify`. It runs `paddock setup` twice in one
container (the second run stands in for a stale snapshot, and apt is disabled during it) and
then runs your verification command, e.g. `php -v | grep -q 'PHP 8.3'`.

Plain test containers have no init system. For daemons that systemd starts, use
`harness::SystemdContainer`: it runs systemd as PID 1, like an orb, so `systemctl enable --now`
works, and `restart()` checks the service comes back after a reboot or orb wake. It needs
`--privileged`, which Docker on Linux and GitHub's runners allow.

In an Amp orb, Docker runs as an orb service declared in `.amp/services.yaml`; start it with
`amp orb service ensure`.

## Releasing

Each release is one pinned unit: static Linux binaries for x86_64 and aarch64 (the dispatcher
with every script embedded) plus an `install.sh` that installs exactly that release.

1. Bump `version` in `Cargo.toml`, run `cargo check` to update `Cargo.lock`, and merge.
2. Tag the merged commit and push the tag:

   ```sh
   git tag v0.2.0 && git push origin v0.2.0
   ```

The [release workflow](.github/workflows/release.yml) runs CI, checks the tag matches
`Cargo.toml`, cross-compiles both binaries, packages them with
[`release/package.sh`](release/package.sh), installs each one in a container of its
architecture, and publishes a GitHub release. A version with a suffix (`v0.2.0-rc.1`) is
published as a prerelease.

A release is installed with the `install.sh` from that release, so the URL is the pin:

```sh
curl -fsSL https://github.com/jerrylopez/paddock/releases/download/v0.2.0/install.sh | bash
```

[`release/install.sh`](release/install.sh) is the template. `package.sh` writes the version,
download URL and each binary's SHA-256 into it, and the installer refuses a binary whose
checksum doesn't match. If the same binary is already installed it does nothing. It installs
to `/usr/local/bin` (using sudo if needed); set `PADDOCK_INSTALL_DIR` to change that.

To build the release binaries locally:

```sh
rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl --target aarch64-unknown-linux-musl
```
