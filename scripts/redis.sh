# redis.sh: installs Redis from its official APT repository (packages.redis.io).
# Usage (via Paddock): paddock setup redis <major.minor> [--password PASSWORD]
#
# <major.minor> picks a release series such as 8.2; an APT pin makes the
# repository provide its latest patch. Moving to a newer series upgrades;
# going back to an older one isn't supported.
#
# Redis listens on 127.0.0.1:6379 (the package default). --password sets
# `requirepass` in /etc/redis/paddock.conf, which includes the package's own
# redis.conf; a systemd drop-in starts redis-server with it.

REDIS_KEY_URL=https://packages.redis.io/gpg
REDIS_KEY_FINGERPRINT=54318FA4052D1E61A6B6F7BB5F4349D6BF53AA0C
REDIS_PIN=/etc/apt/preferences.d/redis
REDIS_CONFIG=/etc/redis/paddock.conf
REDIS_DROPIN=/etc/systemd/system/redis-server.service.d/paddock.conf

option password ""
parse_args "$@"

[[ $VERSION =~ ^[0-9]+\.[0-9]+$ ]] ||
  usage_error "version must be a release series like 8.2 (got '$VERSION')"
[[ $password != *[[:space:]\"\']* ]] ||
  usage_error "--password can't contain spaces or quotes"

# Read in a subshell: os-release defines its own VERSION.
os=$(. /etc/os-release && echo "${ID:-unknown}")
codename=$(. /etc/os-release && echo "${VERSION_CODENAME:-}")
[ "$os" = debian ] || die "only Debian is supported (this system is '$os')"
case $(dpkg --print-architecture) in
  amd64 | arm64 | i386 | armhf) ;;
  *) die "packages.redis.io has no packages for $(dpkg --print-architecture)" ;;
esac
# Both answer on port 6379.
if [ -e /opt/valkey/current ] || package_installed valkey-server; then
  die "Valkey is installed and uses the same port; remove it before setting up Redis"
fi

# The installed series, e.g. 8.2 from 6:8.2.3-1rl1~bookworm1.
installed=
if package_installed redis-server; then
  installed=$(dpkg-query -W -f='${Version}' redis-server | sed -E 's/^[0-9]+://; s/^([0-9]+\.[0-9]+).*/\1/')
fi
if [ -n "$installed" ] && [ "$installed" != "$VERSION" ] &&
  dpkg --compare-versions "$VERSION" lt "$installed"; then
  die "Redis $installed is installed; going back to $VERSION isn't supported (its data may not be readable by older versions)"
fi

apt_keyring redis "$REDIS_KEY_URL" "$REDIS_KEY_FINGERPRINT"
apt_source redis "deb [signed-by=/etc/apt/keyrings/redis.gpg] https://packages.redis.io/deb $codename main"
# Above 1000, so the pinned series wins over newer ones.
write_file "$REDIS_PIN" "Package: redis redis-server redis-tools
Pin: version 6:$VERSION.*
Pin-Priority: 1001"

available_series() {
  apt-cache madison redis-server | awk '{ print $3 }' |
    sed -E 's/^[0-9]+://; s/^([0-9]+\.[0-9]+).*/\1/' | sort -uV | paste -sd ' '
}

upgraded=
if [ "$installed" != "$VERSION" ]; then
  apt_update
  apt-cache madison redis-server | awk '{ print $3 }' | grep -q "^6:$VERSION\." ||
    usage_error "Redis $VERSION isn't available (available: $(available_series))"
  if [ -n "$installed" ]; then
    info "Upgrading Redis $installed to $VERSION"
    quietly as_root env DEBIAN_FRONTEND=noninteractive \
      apt-get -o Acquire::Retries=3 install -y --no-install-recommends redis-server redis-tools
    upgraded=1
  else
    info "Installing Redis $VERSION"
    apt_install redis-server
  fi
fi

config="# Managed by Paddock (paddock setup redis): the package's config plus overrides.
include /etc/redis/redis.conf"
[ -z "$password" ] || config+="
requirepass \"$password\""
write_file "$REDIS_CONFIG" "$config"
restart=$WROTE
as_root chown root:redis "$REDIS_CONFIG"
as_root chmod 640 "$REDIS_CONFIG"

redis_ping() {
  if [ -n "$password" ]; then
    REDISCLI_AUTH=$password redis-cli ping 2>/dev/null
  else
    redis-cli ping 2>/dev/null
  fi
}

redis_version=$(redis-server --version | sed -nE 's/.* v=([0-9.]+) .*/\1/p')
if has_systemd; then
  write_file "$REDIS_DROPIN" "# Managed by Paddock (paddock setup redis).
[Service]
ExecStart=
ExecStart=/usr/bin/redis-server $REDIS_CONFIG"
  if [ -n "$WROTE" ]; then
    quietly as_root systemctl daemon-reload
    restart=1
  fi
  # Restart only a server that was already running before this setup.
  was_running=$(systemctl is-active redis-server 2>/dev/null) || true
  quietly as_root systemctl enable --now redis-server
  if [ "$was_running" = active ] && [ -n "$restart$upgraded" ]; then
    quietly as_root systemctl restart redis-server
  fi
  tries=0
  until [ "$(redis_ping)" = PONG ]; do
    tries=$((tries + 1))
    [ "$tries" -lt 30 ] || die "Redis didn't start; see journalctl -u redis-server"
    sleep 1
  done
  info "Redis $redis_version is running on 127.0.0.1:6379"
else
  info "Redis $redis_version is installed"
  warn "systemd isn't running, so Redis won't start on its own; start it with: sudo -u redis redis-server $REDIS_CONFIG"
fi
