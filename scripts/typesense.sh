# typesense.sh: installs Typesense from its official .deb packages (dl.typesense.org).
# Usage (via Paddock): paddock setup typesense <version> [--api-key KEY]
#
# <version> is a Typesense release such as 30.2. Typesense publishes .deb files
# rather than an APT repository, so the version is exact. Upgrading is
# supported; going back to an older version isn't.
#
# Typesense listens on 127.0.0.1:8108. The API key is --api-key if given,
# otherwise the one already configured, otherwise a generated one; it lives in
# /etc/typesense/typesense-server.ini. Under systemd, Typesense is enabled and
# started.

TYPESENSE_CONFIG=/etc/typesense/typesense-server.ini
TYPESENSE_URL=http://127.0.0.1:8108

option api-key ""
parse_args "$@"

[[ $VERSION =~ ^[0-9]+\.[0-9]+$ ]] ||
  usage_error "version must be a Typesense release like 30.2 (got '$VERSION')"
[ -z "$api_key" ] || [[ $api_key =~ ^[A-Za-z0-9_-]+$ ]] ||
  usage_error "--api-key may only use letters, digits, _ and -"

# Read in a subshell: os-release defines its own VERSION.
os=$(. /etc/os-release && echo "${ID:-unknown}")
[ "$os" = debian ] || die "only Debian is supported (this system is '$os')"
arch=$(dpkg --print-architecture)
case $arch in
  amd64 | arm64) ;;
  *) die "Typesense publishes no packages for $arch" ;;
esac

installed=$(dpkg-query -W -f='${Version}' typesense-server 2>/dev/null) || installed=
if [ -n "$installed" ] && ! package_installed typesense-server; then
  installed=
fi

if [ "$installed" != "$VERSION" ]; then
  if [ -n "$installed" ] && dpkg --compare-versions "$VERSION" lt "$installed"; then
    die "Typesense $installed is installed; going back to $VERSION isn't supported (its data may not be readable by older versions)"
  fi
  apt_install ca-certificates curl
  url=https://dl.typesense.org/releases/$VERSION/typesense-server-$VERSION-$arch.deb
  curl -fsSI "$url" >/dev/null 2>&1 ||
    usage_error "Typesense $VERSION isn't published (no $url); see https://typesense.org/docs/ for releases"

  if [ -n "$installed" ]; then
    info "Upgrading Typesense $installed to $VERSION"
  else
    info "Installing Typesense $VERSION"
  fi
  dir=$(mktemp -d)
  quietly curl -fsSL -o "$dir/typesense.deb" "$url"
  # The package's post-install script runs systemctl and fails without it.
  # Without systemd, give it a stand-in that does nothing.
  # dpkg needs root's usual PATH (with the sbin directories), not the caller's.
  path=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
  if ! has_systemd; then
    mkdir "$dir/bin"
    printf '#!/bin/sh\nexit 0\n' >"$dir/bin/systemctl"
    chmod +x "$dir/bin/systemctl"
    path=$dir/bin:$path
  fi
  # Keep an existing config (and its API key) when upgrading.
  quietly as_root env PATH="$path" dpkg -i --force-confdef --force-confold "$dir/typesense.deb"
  rm -rf "$dir"
fi

# The API key: --api-key, else the configured one, else a new one.
if [ -z "$api_key" ]; then
  api_key=$(as_root sed -n 's/^api-key *= *//p' "$TYPESENSE_CONFIG" 2>/dev/null | head -n1) || true
  if [ -z "$api_key" ] || [ "$api_key" = '$API_KEY' ]; then
    api_key=$(LC_ALL=C tr -dc 'A-Za-z0-9' </dev/urandom | head -c 48)
  fi
fi

write_file "$TYPESENSE_CONFIG" "; Managed by Paddock (paddock setup typesense).
[server]
api-address = 127.0.0.1
api-port = 8108
data-dir = /var/lib/typesense
api-key = $api_key
log-dir = /var/log/typesense"
restart=$WROTE
as_root chmod 600 "$TYPESENSE_CONFIG"

wait_until_ready() {
  local tries
  for tries in $(seq 60); do
    curl -fsS "$TYPESENSE_URL/health" 2>/dev/null | grep -q '"ok":true' && return 0
    sleep 1
  done
  die "Typesense didn't start; see /var/log/typesense/typesense.log and journalctl -u typesense-server"
}

if has_systemd; then
  quietly as_root systemctl enable --now typesense-server
  # Pick up a new config or binary.
  if [ -n "$restart" ] || [ "$installed" != "$VERSION" ]; then
    quietly as_root systemctl restart typesense-server
  fi
  wait_until_ready
  info "Typesense $VERSION is running on http://localhost:8108 (API key in $TYPESENSE_CONFIG)"
else
  info "Typesense $VERSION is installed (API key in $TYPESENSE_CONFIG)"
  warn "systemd isn't running, so Typesense won't start on its own; start it with: sudo typesense-server --config=$TYPESENSE_CONFIG"
fi
