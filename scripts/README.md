# Install-script contract

Every service script in this directory follows the same rules, so `paddock setup` behaves the
same way for every service. `common.sh` provides helpers that make the rules easy to follow.

## How a script runs

`paddock setup <service> <version> [flags...]` joins `common.sh` and `<service>.sh` into one
script and pipes it to bash:

- `$1` is the version and the remaining arguments are the flags, exactly as typed.
- `$SERVICE` is the service name, used in every message.
- Strict mode (`set -Eeuo pipefail`) is on: any failing command stops the script.
- stdin is empty, so a command that waits for input gets end-of-file instead of hanging.
  `DEBIAN_FRONTEND=noninteractive` is exported.
- stdout and stderr go straight to the terminal.

Scripts run in Amp orbs (Debian 12, a normal user with passwordless sudo, systemd) and in the
test containers (Debian, root, no sudo, no systemd). Use `as_root` for privileged commands, and
don't rely on `/usr/sbin` being on the PATH: a normal user's PATH doesn't include it.

Daemons belong to systemd. When `has_systemd` succeeds, enable and start the unit with
`as_root systemctl enable --now <unit>`: systemd keeps it running and starts it again after
every boot or orb wake, so nothing needs to go in `.agents/resume`. For software without a
unit, write one. Without systemd, still install everything, start a temporary server if the
script needs one for configuration, stop it again, and `warn` that the daemon won't start on
its own (see `mysql.sh`).

## The rules

### 1. Re-running is safe

A stale orb snapshot re-runs `.agents/setup`, so every script runs again on a machine where it
already succeeded. That second run must succeed, change nothing that's already right, and
finish quickly.

- Check before you act. Install only what's missing (`apt_install` does this), and write a file
  only if it's absent or different.
- Never fail because something already exists: a user, a database, a repository entry.
- Container tests for a service should use `harness::setup_twice_then_verify`. It runs setup
  twice in one container, with `apt` and `apt-get` made to fail during the second run: a
  rerun on an already set-up machine shouldn't need them.

### 2. Output is short

The output is read by an agent and stored in setup logs. Print a few lines saying what
happened, and nothing else.

- Use `info` for progress, roughly one line per meaningful step: `Installing PHP 8.3`.
  When there's nothing to do, say so: `PHP 8.3 is already installed`.
- Wrap noisy commands (`apt-get`, `curl`, build tools) in `quietly`. Their output is shown only
  if they fail.
- `PADDOCK_VERBOSE=1 paddock setup ...` shows everything, for debugging.

### 3. Versions are exact releases

The version is an exact release (`nginx 1.30.5`, `redis 8.2.3`), never a channel (`stable`) or a
series (`8.2`), so a setup line installs the same thing every time. Pin it with `apt_pin`, check it
with `require_published`, and prefer sources that keep past releases (or their archives) so the pin
keeps working after upstream ships the next one.

### 4. Flags have sensible defaults

`paddock setup <service> <version>` with no flags must produce a working setup.

- Declare every flag with `option NAME DEFAULT` before calling `parse_args "$@"`. The value
  lands in a variable named after the flag, with dashes as underscores.
- Flags accept `--name value` and `--name=value`. Unknown flags are rejected with the list of
  supported ones.
- If only some versions are supported, call `require_version` with that list so a bad version
  fails clearly.

### 5. Failures exit non-zero

A setup that half-worked must not look like success, or a broken snapshot gets cached.

| Exit | Meaning | How |
|---|---|---|
| 0 | Done, or nothing to do | Reach the end of the script |
| 1 | Something went wrong | `die "message"`, or any failing command |
| 2 | Bad version or flag | `usage_error`, `require_version`, `parse_args` |

- Don't hide failures with `|| true`. Use it only when a failure really is expected, and say why
  in a comment.
- An unexpected failure is reported automatically with the command that failed:
  `paddock: php: failed: curl -fsSL https://... (exit 22)`.

## Helpers

| Helper | What it does |
|---|---|
| `option NAME DEFAULT` | Declares `--NAME <value>`, stored in `$NAME` (dashes become underscores) |
| `parse_args "$@"` | Sets `$VERSION` and applies flags; usage error on anything unknown |
| `require_version V...` | Usage error unless `$VERSION` is one of the listed versions |
| `info MSG` | Progress line on stdout: `paddock: <service>: MSG` |
| `warn MSG` | Non-fatal problem on stderr |
| `die MSG` | Reports a failure and exits 1 |
| `usage_error MSG` | Reports a bad version or flag and exits 2 |
| `quietly CMD...` | Runs CMD silently; on failure prints its output and exits with its status |
| `download URL FILE` | Fetches URL into FILE with `curl`, retrying transient network errors |
| `as_root CMD...` | Runs CMD as root, through `sudo` when needed |
| `apt_pin NAME VERSION PKG...` | Pins PKGs to an exact VERSION pattern (e.g. `1.30.5-*`) at priority 1001 |
| `installed_version PKG` | PKG's installed upstream version (`8.2.3`, without epoch or Debian revision), or nothing |
| `require_published PKG LABEL` | Usage error unless `$VERSION` is published for PKG; lists nearby releases |
| `apt_install PKG...` | Installs missing Debian packages, refreshing package lists first only if something is missing |
| `apt_update` | Refreshes package lists, at most once per setup unless `apt_source` changed a source |
| `write_file PATH CONTENT` | Makes PATH contain CONTENT, writing as root only if it differs; sets `WROTE=1` if it wrote |
| `apt_keyring NAME URL FPR...` | Installs the signing keys at URL as `/etc/apt/keyrings/NAME.gpg`, refusing them unless every fingerprint is listed |
| `apt_source NAME LINE` | Makes `/etc/apt/sources.list.d/NAME.list` contain LINE; if that changes it, lists are refreshed again |
| `package_installed PKG` | Succeeds if the Debian package is installed |
| `has_command NAME` | Succeeds if NAME is on the PATH |
| `has_systemd` | Succeeds if systemd manages the system (orbs, servers), not in a plain container |

Pitfalls:

- `quietly` and `die` exit the whole script, so don't call them inside `$(...)`, where they
  would only exit the subshell.
- Don't set an `EXIT` trap; `common.sh` uses it to report failures.
- Don't source `/etc/os-release` directly: it defines its own `VERSION`. Read it in a subshell,
  e.g. `codename=$(. /etc/os-release && echo "$VERSION_CODENAME")`.

## Template

```bash
# example.sh: installs Example from its APT repository.
# Usage (via Paddock): paddock setup example <version> [flags...]

option port 8080
parse_args "$@"
[[ $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] ||
  usage_error "version must be an Example release like 2.1.0 (got '$VERSION')"

apt_keyring example https://example.org/key.asc 0123456789ABCDEF0123456789ABCDEF01234567
apt_source example "deb [signed-by=/etc/apt/keyrings/example.gpg] https://example.org/apt stable main"
apt_pin example "$VERSION-*" example

if [ "$(installed_version example)" != "$VERSION" ]; then
  apt_update
  require_published example Example
  info "Installing Example $VERSION"
  quietly as_root env DEBIAN_FRONTEND=noninteractive apt-get install -y --allow-downgrades example
fi

as_root example configure --port "$port"
info "Example $VERSION is ready on port $port"
```

Then register it in `SERVICES` in `src/dispatch.rs`: `service!("example", "Example server")`.
