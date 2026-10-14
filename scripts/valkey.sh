# valkey.sh: installs Valkey from its official release binaries (download.valkey.io).
# Usage (via Paddock): paddock setup valkey <version> [--password PASSWORD]
#
# <version> is a Valkey release such as 9.1.2. Valkey publishes no Debian
# repository (bookworm-backports only has 8.0.1), so Paddock installs the
# official build for Ubuntu 22.04, which runs on Debian 12, checked against its
# published SHA-256. Upgrading is supported; going back to an older version isn't.
#
# Valkey listens on 127.0.0.1:6379; --password sets `requirepass`. Under
# systemd it runs as the `valkey-server` unit.

VALKEY_HOME=/opt/valkey
VALKEY_CONFIG=/etc/valkey/valkey.conf
VALKEY_UNIT=/etc/systemd/system/valkey-server.service
VALKEY_TOOLS=(valkey-server valkey-cli valkey-benchmark valkey-check-aof valkey-check-rdb valkey-sentinel)

option password ""
parse_args "$@"

[[ $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] ||
  usage_error "version must be a Valkey release like 9.1.2 (got '$VERSION')"
[[ $password != *[[:space:]\"\']* ]] ||
  usage_error "--password can't contain spaces or quotes"

# Read in a subshell: os-release defines its own VERSION.
os=$(. /etc/os-release && echo "${ID:-unknown}")
[ "$os" = debian ] || die "only Debian is supported (this system is '$os')"
case $(dpkg --print-architecture) in
  amd64) arch=x86_64 ;;
  arm64) arch=arm64 ;;
  *) die "Valkey publishes no builds for $(dpkg --print-architecture)" ;;
esac
# Both answer on port 6379.
for package in redis redis-server; do
  ! package_installed "$package" ||
    die "Redis is installed and uses the same port; remove it before setting up Valkey"
done

installed=$(readlink "$VALKEY_HOME/current" 2>/dev/null) || installed=
installed=${installed##*/}
if [ -n "$installed" ] && [ "$installed" != "$VERSION" ] &&
  dpkg --compare-versions "$VERSION" lt "$installed"; then
  die "Valkey $installed is installed; going back to $VERSION isn't supported (its data may not be readable by older versions)"
fi

new_binary=
if [ ! -x "$VALKEY_HOME/$VERSION/bin/valkey-server" ]; then
  apt_install ca-certificates curl
  url=https://download.valkey.io/releases/valkey-$VERSION-jammy-$arch.tar.gz
  curl -fsSI --retry 3 --retry-connrefused "$url" >/dev/null 2>&1 ||
    usage_error "Valkey $VERSION isn't published (no $url); see https://valkey.io/download/ for releases"
  if [ -n "$installed" ]; then
    info "Upgrading Valkey $installed to $VERSION"
  else
    info "Installing Valkey $VERSION"
  fi
  dir=$(mktemp -d)
  download "$url" "$dir/valkey.tar.gz"
  download "$url.sha256" "$dir/valkey.tar.gz.sha256"
  expected=$(awk '{ print $1 }' "$dir/valkey.tar.gz.sha256")
  actual=$(sha256sum "$dir/valkey.tar.gz" | awk '{ print $1 }')
  if [ "$expected" != "$actual" ]; then
    rm -rf "$dir"
    die "the Valkey download from $url doesn't match its checksum"
  fi
  quietly tar -xzf "$dir/valkey.tar.gz" -C "$dir"
  quietly as_root mkdir -p "$VALKEY_HOME/$VERSION"
  quietly as_root cp -R "$dir/valkey-$VERSION-jammy-$arch/." "$VALKEY_HOME/$VERSION/"
  rm -rf "$dir"
  new_binary=1
fi

# Point `current` and the commands on the PATH at this version.
if [ "$installed" != "$VERSION" ]; then
  quietly as_root ln -sfn "$VALKEY_HOME/$VERSION" "$VALKEY_HOME/current"
  new_binary=1
fi
for tool in "${VALKEY_TOOLS[@]}"; do
  if [ "$(readlink "/usr/local/bin/$tool" 2>/dev/null)" != "$VALKEY_HOME/current/bin/$tool" ]; then
    quietly as_root ln -sfn "$VALKEY_HOME/current/bin/$tool" "/usr/local/bin/$tool"
  fi
done

if ! id valkey >/dev/null 2>&1; then
  quietly as_root useradd --system --home-dir /var/lib/valkey --shell /usr/sbin/nologin valkey
fi
quietly as_root install -d -o valkey -g valkey -m 0750 /var/lib/valkey

config="# Managed by Paddock (paddock setup valkey).
bind 127.0.0.1 -::1
port 6379
dir /var/lib/valkey
daemonize no"
[ -z "$password" ] || config+="
requirepass \"$password\""
write_file "$VALKEY_CONFIG" "$config"
restart=$WROTE
as_root chown root:valkey "$VALKEY_CONFIG"
as_root chmod 640 "$VALKEY_CONFIG"

valkey_ping() {
  if [ -n "$password" ]; then
    VALKEYCLI_AUTH=$password REDISCLI_AUTH=$password /usr/local/bin/valkey-cli ping 2>/dev/null
  else
    /usr/local/bin/valkey-cli ping 2>/dev/null
  fi
}

if has_systemd; then
  write_file "$VALKEY_UNIT" "# Managed by Paddock (paddock setup valkey).
[Unit]
Description=Valkey key-value store
After=network.target

[Service]
Type=notify
ExecStart=/usr/local/bin/valkey-server $VALKEY_CONFIG --supervised systemd
User=valkey
Group=valkey
RuntimeDirectory=valkey
LimitNOFILE=65535
Restart=on-failure

[Install]
WantedBy=multi-user.target"
  [ -z "$WROTE" ] || quietly as_root systemctl daemon-reload
  # Restart only a server that was already running before this setup.
  was_running=$(systemctl is-active valkey-server 2>/dev/null) || true
  quietly as_root systemctl enable --now valkey-server
  if [ "$was_running" = active ] && [ -n "$restart$new_binary" ]; then
    quietly as_root systemctl restart valkey-server
  fi
  tries=0
  until [ "$(valkey_ping)" = PONG ]; do
    tries=$((tries + 1))
    [ "$tries" -lt 30 ] || die "Valkey didn't start; see journalctl -u valkey-server"
    sleep 1
  done
  info "Valkey $VERSION is running on 127.0.0.1:6379"
else
  info "Valkey $VERSION is installed"
  warn "systemd isn't running, so Valkey won't start on its own; start it with: sudo -u valkey valkey-server $VALKEY_CONFIG --daemonize yes"
fi
