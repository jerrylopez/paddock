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

## Adding a service

1. Write `scripts/<name>.sh` following the [install-script contract](scripts/README.md):
   safe to re-run, short output, sensible defaults, non-zero exit on failure. Paddock runs it
   with bash after `scripts/common.sh`, which provides helpers for each of those rules.
2. Register it in `SERVICES` in `src/dispatch.rs`: `service!("<name>", "<one-line description>")`.
   The script is embedded into the binary at compile time, so nothing is fetched at runtime.

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
