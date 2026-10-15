# Paddock

Paddock sets up services natively on Debian, one line per service, so an Amp orb (or any
Debian 12 server) gets the same environment every time:

```sh
paddock setup php 8.4 --extensions redis
paddock setup mysql 8.4 --database app --username app --password secret
paddock setup redis 8.2
```

Each line installs the service from its official repository, configures it for local
development, and, where it's a daemon, hands it to systemd so it comes back after every reboot
or orb wake. Running a line again on a machine that's already set up changes nothing and
finishes in a second or two.

## Quick start

Put this in your repository's `.agents/setup` (the script Amp runs to prepare an orb), make it
executable with `chmod +x .agents/setup`, and commit it:

```bash
#!/usr/bin/env bash
set -euo pipefail

# Paddock itself, pinned to one release.
curl -fsSL https://github.com/jerrylopez/paddock/releases/download/v0.1.0/install.sh | bash

# The environment this branch needs.
paddock setup php 8.4 --extensions xdebug
paddock setup mysql 8.4 --database app --username app --password secret
paddock setup redis 8.2
```

Every new orb for that branch now comes up with those services installed and running.

## Installing

```sh
curl -fsSL https://github.com/jerrylopez/paddock/releases/download/v0.1.0/install.sh | bash
```

Each release ships its own `install.sh`, with that release's download URL and SHA-256
checksums written in. It installs exactly that release to `/usr/local/bin/paddock` (using
sudo if needed; set `PADDOCK_INSTALL_DIR` to change the directory), refuses a download whose
checksum doesn't match, and does nothing if the same binary is already there.

Requirements: Debian 12 (bookworm) on x86_64 or aarch64, root or passwordless sudo, and systemd
for services to start on their own (orbs and servers have it; plain containers don't, so
services are installed and configured there but not left running). A few vendors don't publish
for aarch64: MySQL and RabbitMQ are x86_64 only.

## What's pinned

- **Paddock** is pinned by the release in the install URL. A release is one unit: the binary
  with every install script built in, so `v0.1.0` always runs the same scripts. Nothing is
  fetched at run time. Upgrading Paddock means changing that URL in a commit.
- **Services** are pinned to a release series: `php 8.4`, `redis 8.2`, `nginx 1.30`, and for
  PostgreSQL the major, `17`. Setup installs the latest patch in the series, so a new orb always
  gets the current security fixes. Channel names (`stable`, `latest`) and exact patches
  (`8.2.3`) aren't accepted.
- **Re-runs don't upgrade.** On a machine that's already set up, a re-run installs nothing and
  doesn't touch the network, so `.agents/setup` stays fast on an orb restored from a snapshot.
  That orb keeps the patch it was set up with until it's set up fresh.
- **Per branch:** `.agents/setup` lives in the repository, so each branch declares its own
  environment. Change a version on a branch and that branch's orbs get it; merge it to share it.

## Usage

```sh
paddock setup <service> <version> [flags...]
paddock setup --help        # lists the services
```

Output is a line or two per service, for example
`paddock: redis: Redis 8.2.10 is running on 127.0.0.1:6379`. Exit codes: 0 when the service is
ready (or already was), 1 when something failed, 2 for a bad version or flag. A version that
isn't available says so, and lists the available ones where the vendor publishes a list.

| Service | Example | |
|---|---|---|
| PHP CLI + Composer | `paddock setup php 8.4 --extensions redis` | [details](#php) |
| PHP-FPM | `paddock setup php-fpm 8.4` | [details](#php-fpm) |
| nginx | `paddock setup nginx 1.30` | [details](#nginx) |
| MySQL | `paddock setup mysql 8.4 --database app --username app --password secret` | [details](#mysql) |
| MariaDB | `paddock setup mariadb 11.4 --database app --username app --password secret` | [details](#mariadb) |
| PostgreSQL | `paddock setup postgres 17 --database app --username app --password secret` | [details](#postgresql) |
| Redis | `paddock setup redis 8.2 --password secret` | [details](#redis) |
| Valkey | `paddock setup valkey 9.1` | [details](#valkey) |
| Elasticsearch | `paddock setup elasticsearch 9.5` | [details](#elasticsearch) |
| OpenSearch | `paddock setup opensearch 3.9` | [details](#opensearch) |
| Typesense | `paddock setup typesense 30.2 --api-key secret` | [details](#typesense) |
| RabbitMQ | `paddock setup rabbitmq 4.2 --username app --password secret` | [details](#rabbitmq) |
| Varnish | `paddock setup varnish 8.0 --backend 127.0.0.1:8080` | [details](#varnish) |

Services listen on 127.0.0.1 and are configured for development (for example, Elasticsearch and
OpenSearch run as a single node without TLS or passwords).

## Reading the install scripts

Every service is a plain bash script in [`scripts/`](scripts/), built into the binary at
compile time, so what you read is exactly what runs:
[php](scripts/php.sh) (also PHP-FPM), [nginx](scripts/nginx.sh), [mysql](scripts/mysql.sh),
[mariadb](scripts/mariadb.sh), [postgres](scripts/postgres.sh), [redis](scripts/redis.sh),
[valkey](scripts/valkey.sh), [elasticsearch](scripts/elasticsearch.sh),
[opensearch](scripts/opensearch.sh), [typesense](scripts/typesense.sh),
[rabbitmq](scripts/rabbitmq.sh), [varnish](scripts/varnish.sh). They share
[`common.sh`](scripts/common.sh), and all follow the
[install-script contract](scripts/README.md): safe to re-run, short output, release series as
versions, sensible defaults, and a non-zero exit on failure. Signing keys are checked against
pinned fingerprints, and downloads without an APT repository against published checksums
where the vendor provides them.

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

### Elasticsearch

```sh
paddock setup elasticsearch 9
paddock setup elasticsearch 8 --heap 1g
```

Installs Elasticsearch from [Elastic's APT repository](https://www.elastic.co/docs/deploy-manage/deploy/self-managed/install-elasticsearch-with-debian-package).
The version is the major (`8` or `9`); the repository provides the latest release in it.

- Configured for local development: one node on `127.0.0.1:9200`, security disabled (no TLS or
  passwords), JVM heap 512m unless `--heap` says otherwise. Paddock manages
  `/etc/elasticsearch/elasticsearch.yml` and restarts Elasticsearch when it changes.
- Under systemd, Elasticsearch is enabled and started, and setup waits until the cluster is usable.
- Switching an install to another major isn't supported, and it refuses to install next to
  OpenSearch, which uses the same port.

### MariaDB

```sh
paddock setup mariadb 11.4
paddock setup mariadb 10.11 --database app --username app --password secret
```

Installs MariaDB Server from [the MariaDB Foundation's repository](https://mariadb.org/download/).
The version is a release series with a bookworm repository (10.11, 11.4, 11.8 and 12.3 today); the
repository provides its latest release, and its packages take priority over Debian's own MariaDB.

- Same flags as MySQL. With `--root-password`, root can log in with that password and still
  through the Unix socket (`sudo mariadb`).
- Under systemd it runs as the `mariadb` unit. Switching an install to another series isn't
  supported, and MariaDB and MySQL refuse to install next to each other.

### MySQL

```sh
paddock setup mysql 8.4
paddock setup mysql 8.0 --database app --username app --password secret
```

Installs MySQL Community Server from [Oracle's APT repository](https://repo.mysql.com/apt/debian/).
The download of Oracle's signing key is checked against its fingerprint.

- The version picks a release series: `8.0`, `8.4` or `9.7`; the repository provides
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
paddock setup nginx 1.30
paddock setup nginx 1.31
```

Installs nginx from [nginx.org's repository](https://nginx.org/en/linux_packages.html). The
version is a release series; the latest patch in it is installed. nginx.org publishes even series
(1.30) in its stable repository and odd ones (1.31) in mainline, and running setup with another
series switches to it, older or newer. nginx.org's packages
take priority over Debian's own nginx. The default site answers on port 80. Under systemd,
nginx is enabled and started. Wiring nginx to PHP-FPM is left to presets.

### OpenSearch

```sh
paddock setup opensearch 3.9
paddock setup opensearch 1.2 --heap 1g
```

Installs OpenSearch from [its APT repositories](https://opensearch.org/docs/latest/install-and-configure/install-opensearch/debian/).
The version is a series from `1.2` on (`1.2`, `1.3`, `2.0`–`2.19`, `3.x`); the repository provides
its latest release. The repositories start at 1.3 and 2.5, so 1.2 and 2.0–2.4 get their final
release (1.2.4, 2.0.1, ..., 2.4.1) from OpenSearch's release tarball instead, checked against its
signature and set up like the package, with a systemd unit Paddock writes.

- Configured for local development: one node on `127.0.0.1:9200`, security plugin disabled (no
  TLS or passwords), JVM heap 512m unless `--heap` says otherwise. Paddock manages
  `/etc/opensearch/opensearch.yml` and restarts OpenSearch when it changes.
- Under systemd, OpenSearch is enabled and started, and setup waits until the cluster is usable.
- Moving to a newer series within a major upgrades it, including from a tarball install to the
  package (1.2 to 1.3, or 2.4 to 2.5 and later), keeping the data. Going back or to another
  major isn't supported, and it refuses to install next to Elasticsearch, which uses the same
  port.

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

### Valkey

```sh
paddock setup valkey 9.1
paddock setup valkey 9.1 --password secret
```

Installs Valkey from [its official release builds](https://valkey.io/download/), checked against
their published SHA-256. Valkey publishes no Debian repository (bookworm-backports only has 8.0.1),
and the Ubuntu 22.04 build is used, which runs on Debian 12. The version is a release series;
setup installs its latest patch (found by checking which `9.1.x` builds exist). Moving to a newer
series upgrades; going back isn't supported.

- Installed under `/opt/valkey/<release>`, with the commands linked into `/usr/local/bin`.
- Listens on `127.0.0.1:6379`; `--password` sets `requirepass`. Config: `/etc/valkey/valkey.conf`.
- Under systemd it runs as the `valkey-server` unit (written by Paddock), as the `valkey` user.
- Refuses to install next to Redis, which uses the same port.

### PostgreSQL

```sh
paddock setup postgres 17
paddock setup postgres 16 --database app --username app --password secret
```

Installs PostgreSQL from [the PostgreSQL project's APT repository](https://wiki.postgresql.org/wiki/Apt)
(PGDG). The version is a major (10 to 18 today); the repository provides the latest minor release.

- Several majors can be installed side by side. Each has its own `main` cluster and port: 5432 for
  the first, then 5433 and so on. The ready line prints the port.
- `--database` creates a database; `--username` and `--password` create a login role, which owns
  that database when both are given. Passwords never appear on a command line.
- The `postgres` superuser logs in through the Unix socket: `sudo -u postgres psql`.
- Under systemd the cluster runs as `postgresql@<major>-main`. Without systemd it's installed
  and configured, then stopped.

### RabbitMQ

```sh
paddock setup rabbitmq 4.2
paddock setup rabbitmq 4.2 --username app --password secret --vhost app
```

Installs RabbitMQ and a matching Erlang from [Team RabbitMQ's repositories](https://www.rabbitmq.com/docs/install-debian).
The version is a 4.x release series; an APT pin makes the repository provide its latest patch.
Moving to a newer series upgrades; going back isn't supported. amd64 only: Team RabbitMQ
publishes Erlang for amd64 only, and Debian's own Erlang is too old for RabbitMQ 4.

- Listens on `127.0.0.1:5672`, with the management UI on <http://localhost:15672>.
- `--username` and `--password` create an administrator; `--vhost` creates a virtual host the
  user gets full access to (otherwise the default `/`). Passwords never appear on a command line.
  The default `guest` user stays, limited to localhost.

### Redis

```sh
paddock setup redis 8.2
paddock setup redis 7.4 --password secret
```

Installs Redis from [its APT repository](https://redis.io/docs/latest/operate/oss_and_stack/install/install-stack/apt/).
The version is a release series (6.0 to 8.10 today); an APT pin makes the repository provide its
latest patch. Moving to a newer series upgrades; going back isn't supported.

- Listens on `127.0.0.1:6379` (the package default). `--password` sets `requirepass` in
  `/etc/redis/paddock.conf`, which includes the package's own `redis.conf`; a systemd drop-in
  starts `redis-server` with it, so the package's file is never edited.
- Refuses to install next to Valkey, which uses the same port.

### Varnish

```sh
paddock setup varnish 8.0
paddock setup varnish 7.0 --backend 127.0.0.1:3000 --port 6081 --memory 512m
```

Installs Varnish Cache from [its packagecloud repositories](https://packagecloud.io/varnishcache).
The version is a series: `6.0` (LTS), `7.0` to `7.7`, or `8.0`; the repository provides its latest
release, and running setup with another series switches to it. The repository's packages take
priority over Debian's own Varnish. 7.0 to 7.3 predate Debian 12, so they come from their
repository's Debian 11 packages, which run on it.

- Listens on `127.0.0.1:6081` (`--port`) and caches `--backend` (default `127.0.0.1:8080`) in
  `--memory` (default `256m`).
- Paddock writes `/etc/varnish/paddock.vcl` and a systemd drop-in that starts `varnishd` with it;
  the package's `default.vcl` is left alone.

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
`harness::SystemdContainer`: it runs systemd as PID 1 and has a `dev` user with passwordless sudo,
like an orb. Its `setup_twice_then_verify` runs setup as `dev`, so problems that only show up
through sudo are caught, and `restart()` checks the service comes back after a reboot or orb wake. It needs
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
