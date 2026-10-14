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

### Elasticsearch

```sh
paddock setup elasticsearch 9.5.5
paddock setup elasticsearch 8.19.23 --heap 1g
```

Installs Elasticsearch from [Elastic's APT repository](https://www.elastic.co/docs/deploy-manage/deploy/self-managed/install-elasticsearch-with-debian-package).
The version is an exact 8.x or 9.x release; Elastic keeps past releases and an APT pin installs
exactly that one. Upgrading within a major is supported; going back isn't.

- Configured for local development: one node on `127.0.0.1:9200`, security disabled (no TLS or
  passwords), JVM heap 512m unless `--heap` says otherwise. Paddock manages
  `/etc/elasticsearch/elasticsearch.yml` and restarts Elasticsearch when it changes.
- Under systemd, Elasticsearch is enabled and started, and setup waits until the cluster is usable.
- Switching an install to another major isn't supported, and it refuses to install next to
  OpenSearch, which uses the same port.

### MariaDB

```sh
paddock setup mariadb 11.4.13
paddock setup mariadb 10.11.19 --database app --username app --password secret
```

Installs MariaDB Server from [the MariaDB Foundation's repository](https://mariadb.org/download/).
The version is an exact release. Each release has its own repository on archive.mariadb.org, which
stays available, so an install is always exactly that release; its packages take priority over
Debian's own MariaDB. Upgrading within a series (11.4.12 to 11.4.13) is supported.

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
paddock setup nginx 1.30.5
paddock setup nginx 1.31.6
```

Installs nginx from [nginx.org's repository](https://nginx.org/en/linux_packages.html). The
version is an exact release. nginx.org publishes even minor versions (1.30) in its stable repository
and odd ones (1.31) in mainline, keeping past releases in both; an APT pin installs exactly the one
asked for, and running setup with another release switches to it, older or newer. nginx.org's packages
take priority over Debian's own nginx. The default site answers on port 80. Under systemd,
nginx is enabled and started. Wiring nginx to PHP-FPM is left to presets.

### OpenSearch

```sh
paddock setup opensearch 3.9.0
paddock setup opensearch 2.19.6 --heap 1g
```

Installs OpenSearch from [its APT repository](https://opensearch.org/docs/latest/install-and-configure/install-opensearch/debian/).
The version is an exact 2.x or 3.x release; the repository keeps past releases and an APT pin installs
exactly that one. Upgrading within a major is supported; going back isn't.

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

### Valkey

```sh
paddock setup valkey 9.1.2
paddock setup valkey 9.1.2 --password secret
```

Installs Valkey from [its official release builds](https://valkey.io/download/), checked against
their published SHA-256. Valkey publishes no Debian repository (bookworm-backports only has 8.0.1),
so the version is an exact release and the Ubuntu 22.04 build is used, which runs on Debian 12.
Upgrading is supported; going back to an older version isn't.

- Installed under `/opt/valkey/<version>`, with the commands linked into `/usr/local/bin`.
- Listens on `127.0.0.1:6379`; `--password` sets `requirepass`. Config: `/etc/valkey/valkey.conf`.
- Under systemd it runs as the `valkey-server` unit (written by Paddock), as the `valkey` user.
- Refuses to install next to Redis, which uses the same port.

### PostgreSQL

```sh
paddock setup postgres 17.11
paddock setup postgres 16.15 --database app --username app --password secret
```

Installs PostgreSQL from [the PostgreSQL project's APT repository](https://wiki.postgresql.org/wiki/Apt)
(PGDG). The version is an exact release such as 17.6. The repository and its archive
(apt-archive.postgresql.org) keep every release, and an APT pin installs exactly that one. Upgrading
to a newer minor is supported; going back isn't.

- Several majors can be installed side by side. Each has its own `main` cluster and port: 5432 for
  the first, then 5433 and so on. The ready line prints the port.
- `--database` creates a database; `--username` and `--password` create a login role, which owns
  that database when both are given. Passwords never appear on a command line.
- The `postgres` superuser logs in through the Unix socket: `sudo -u postgres psql`.
- Under systemd the cluster runs as `postgresql@<major>-main`. Without systemd it's installed
  and configured, then stopped.

### RabbitMQ

```sh
paddock setup rabbitmq 4.2.9
paddock setup rabbitmq 4.2.9 --username app --password secret --vhost app
```

Installs RabbitMQ and a matching Erlang from [Team RabbitMQ's repositories](https://www.rabbitmq.com/docs/install-debian).
The version is an exact 4.x release; the repository keeps past releases and an APT pin installs
exactly that one. Upgrading is supported; going back isn't. amd64 only: Team RabbitMQ
publishes Erlang for amd64 only, and Debian's own Erlang is too old for RabbitMQ 4.

- Listens on `127.0.0.1:5672`, with the management UI on <http://localhost:15672>.
- `--username` and `--password` create an administrator; `--vhost` creates a virtual host the
  user gets full access to (otherwise the default `/`). Passwords never appear on a command line.
  The default `guest` user stays, limited to localhost.

### Redis

```sh
paddock setup redis 8.2.10
paddock setup redis 7.4.11 --password secret
```

Installs Redis from [its APT repository](https://redis.io/docs/latest/operate/oss_and_stack/install/install-stack/apt/).
The version is an exact release; the repository keeps past releases and an APT pin installs exactly
that one. Upgrading is supported; going back isn't.

- Listens on `127.0.0.1:6379` (the package default). `--password` sets `requirepass` in
  `/etc/redis/paddock.conf`, which includes the package's own `redis.conf`; a systemd drop-in
  starts `redis-server` with it, so the package's file is never edited.
- Refuses to install next to Valkey, which uses the same port.

### Varnish

```sh
paddock setup varnish 8.0.2
paddock setup varnish 6.0.18 --backend 127.0.0.1:3000 --port 6081 --memory 512m
```

Installs Varnish Cache from [its packagecloud repositories](https://packagecloud.io/varnishcache).
The version is an exact release in a supported series: 6.0 (LTS), 7.6, 7.7 or 8.0. Each series'
repository keeps past releases and an APT pin installs exactly the one asked for; running setup with
another release switches to it. The repository's packages take
priority over Debian's own Varnish.

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
