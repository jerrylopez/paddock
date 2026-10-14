# common.sh: shared prelude for every install script.
#
# Paddock places this file in front of the service script and pipes both to
# bash as one script, so everything defined here is available to the service.
# The rules these helpers support are in scripts/README.md.

set -Eeuo pipefail

# Name of the service being set up, e.g. `php`. Set by Paddock.
SERVICE=${PADDOCK_SERVICE:-unknown}
readonly SERVICE

# Version requested on the command line. Set by `parse_args`.
VERSION=

export DEBIAN_FRONTEND=noninteractive

_paddock_options=()
_paddock_failed_command=
_paddock_reported=
_paddock_apt_updated=

# Remember the last failing command so the exit summary can name it.
trap '_paddock_failed_command=$BASH_COMMAND' ERR
trap '_paddock_on_exit $?' EXIT

_paddock_on_exit() {
  [ "$1" -ne 0 ] && [ -z "$_paddock_reported" ] || return 0
  if [ -n "$_paddock_failed_command" ]; then
    printf 'paddock: %s: failed: %s (exit %s)\n' "$SERVICE" "$_paddock_failed_command" "$1" >&2
  else
    printf 'paddock: %s: exited with status %s\n' "$SERVICE" "$1" >&2
  fi
}

# --- Output ------------------------------------------------------------------

# info MESSAGE: one line of progress on stdout.
info() {
  printf 'paddock: %s: %s\n' "$SERVICE" "$*"
}

# warn MESSAGE: a non-fatal problem on stderr.
warn() {
  printf 'paddock: %s: warning: %s\n' "$SERVICE" "$*" >&2
}

# die MESSAGE: report a genuine failure and exit 1.
die() {
  printf 'paddock: %s: %s\n' "$SERVICE" "$*" >&2
  _paddock_reported=1
  exit 1
}

# usage_error MESSAGE: report a bad version or flag and exit 2.
usage_error() {
  printf 'paddock: %s: %s\n' "$SERVICE" "$*" >&2
  _paddock_reported=1
  exit 2
}

# quietly COMMAND...: run COMMAND without showing its output. If it fails,
# print the output and the command, then exit with its status.
# Set PADDOCK_VERBOSE=1 to show output as it happens instead.
quietly() {
  if [ -n "${PADDOCK_VERBOSE:-}" ]; then
    "$@"
    return
  fi
  local output status=0
  output=$("$@" 2>&1) || status=$?
  if [ "$status" -ne 0 ]; then
    [ -n "$output" ] && printf '%s\n' "$output" >&2
    printf 'paddock: %s: failed: %s (exit %s)\n' "$SERVICE" "$*" "$status" >&2
    _paddock_reported=1
    exit "$status"
  fi
}

# --- Arguments ---------------------------------------------------------------

# option NAME DEFAULT: declare a `--NAME <value>` flag. Its value is stored in
# a variable named after the flag with dashes as underscores (`--root-password`
# sets `root_password`), starting at DEFAULT.
option() {
  [[ $1 =~ ^[a-z][a-z0-9-]*$ ]] || die "invalid option name '$1'"
  _paddock_options+=("$1")
  printf -v "${1//-/_}" '%s' "$2"
}

# parse_args "$@": set VERSION from the first argument and apply flags
# declared with `option`. Unknown flags are a usage error.
parse_args() {
  [ $# -ge 1 ] || usage_error "missing version"
  VERSION=$1
  shift

  local arg name value
  while [ $# -gt 0 ]; do
    arg=$1
    case $arg in
      --*=*)
        name=${arg%%=*}
        name=${name#--}
        value=${arg#*=}
        shift
        ;;
      --*)
        name=${arg#--}
        [ $# -ge 2 ] || usage_error "--$name needs a value"
        value=$2
        shift 2
        ;;
      *)
        usage_error "unexpected argument '$arg'"
        ;;
    esac
    _paddock_has_option "$name" || usage_error "unknown flag --$name ($(_paddock_supported_flags))"
    printf -v "${name//-/_}" '%s' "$value"
  done
}

_paddock_has_option() {
  local known
  for known in "${_paddock_options[@]}"; do
    [ "$known" = "$1" ] && return 0
  done
  return 1
}

_paddock_supported_flags() {
  if [ ${#_paddock_options[@]} -eq 0 ]; then
    echo "$SERVICE takes no flags"
  else
    printf 'supported:'
    printf ' --%s' "${_paddock_options[@]}"
  fi
}

# require_version VERSION...: usage error unless VERSION is one of those listed.
require_version() {
  local supported
  for supported in "$@"; do
    [ "$VERSION" = "$supported" ] && return 0
  done
  usage_error "unsupported version '$VERSION' (supported: $*)"
}

# --- System ------------------------------------------------------------------

# has_systemd: succeed if systemd manages this system (orbs, servers), as
# opposed to a plain container.
has_systemd() {
  [ -d /run/systemd/system ]
}

# has_command NAME: succeed if NAME is on the PATH.
has_command() {
  command -v "$1" >/dev/null 2>&1
}

# as_root COMMAND...: run COMMAND as root, using sudo when not already root.
as_root() {
  _paddock_require_root
  if [ "$(id -u)" -eq 0 ]; then
    "$@"
  else
    sudo -n "$@"
  fi
}

_paddock_require_root() {
  [ "$(id -u)" -eq 0 ] || has_command sudo || die "needs root: run as root or install sudo"
}

# package_installed PACKAGE: succeed if the Debian package is installed.
package_installed() {
  local status
  status=$(dpkg-query -W -f='${Status}' "$1" 2>/dev/null) || return 1
  [ "$status" = "install ok installed" ]
}

# apt_update: refresh package lists, at most once per setup unless `apt_source`
# changes a source. Call it before `apt-cache` lookups; `apt_install` calls it
# itself when something needs installing.
apt_update() {
  [ -z "$_paddock_apt_updated" ] || return 0
  # Checked here so the error isn't buried in `quietly`'s captured output.
  _paddock_require_root
  quietly as_root apt-get update
  _paddock_apt_updated=1
}

# write_file PATH CONTENT: make PATH contain CONTENT (plus a final newline),
# writing as root only if it differs. Sets WROTE=1 if it wrote, else WROTE=.
write_file() {
  WROTE=
  if [ -f "$1" ] && [ "$(cat "$1")" = "$2" ]; then
    return 0
  fi
  _paddock_require_root
  as_root mkdir -p "$(dirname "$1")"
  printf '%s\n' "$2" | as_root tee "$1" >/dev/null
  WROTE=1
}

# apt_keyring NAME URL FINGERPRINT...: install the signing keys published at
# URL as /etc/apt/keyrings/NAME.gpg, for `signed-by=` in an `apt_source` line.
# Refuses them unless every key's fingerprint is one of those listed. Does
# nothing if the keyring already exists.
apt_keyring() {
  local name=$1 url=$2
  shift 2
  local keyring=/etc/apt/keyrings/$name.gpg
  [ ! -f "$keyring" ] || return 0

  apt_install ca-certificates curl gnupg
  local dir fingerprint expected found=()
  dir=$(mktemp -d)
  quietly curl -fsSL -o "$dir/key" "$url"
  quietly env GNUPGHOME="$dir" gpg --batch --import "$dir/key"
  mapfile -t found < <(GNUPGHOME=$dir gpg --batch --with-colons --list-keys 2>/dev/null |
    awk -F: '$1 == "pub" { primary = 1; next } primary && $1 == "fpr" { print $10; primary = 0 }')
  for fingerprint in "${found[@]}"; do
    for expected in "$@"; do
      [ "$fingerprint" = "$expected" ] && continue 2
    done
    _paddock_drop_gnupg "$dir"
    die "the signing key $fingerprint from $url isn't one of the expected keys ($*)"
  done
  if [ ${#found[@]} -eq 0 ]; then
    _paddock_drop_gnupg "$dir"
    die "no signing key found at $url"
  fi
  GNUPGHOME=$dir gpg --batch --export >"$dir/keyring.gpg"
  quietly as_root install -D -m 0644 "$dir/keyring.gpg" "$keyring"
  _paddock_drop_gnupg "$dir"
}

_paddock_drop_gnupg() {
  GNUPGHOME=$1 gpgconf --kill all >/dev/null 2>&1 || true
  rm -rf "$1"
}

# apt_source NAME LINE: make /etc/apt/sources.list.d/NAME.list contain LINE.
# If that changes the file, the next `apt_update` runs again.
apt_source() {
  local file=/etc/apt/sources.list.d/$1.list
  if [ -f "$file" ] && [ "$(cat "$file")" = "$2" ]; then
    return 0
  fi
  _paddock_require_root
  printf '%s\n' "$2" | as_root tee "$file" >/dev/null
  _paddock_apt_updated=
}

# apt_install PACKAGE...: install the packages that aren't installed yet.
# Refreshes package lists first (see `apt_update`), only if something is missing.
apt_install() {
  local package missing=()
  for package in "$@"; do
    package_installed "$package" || missing+=("$package")
  done
  [ ${#missing[@]} -eq 0 ] && return 0

  apt_update
  quietly as_root env DEBIAN_FRONTEND=noninteractive \
    apt-get install -y --no-install-recommends "${missing[@]}"
}
