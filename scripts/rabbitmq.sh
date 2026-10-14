# rabbitmq.sh: installs RabbitMQ and a matching Erlang from Team RabbitMQ's APT repositories.
# Usage (via Paddock): paddock setup rabbitmq <version> [flags...]
#
# <version> is an exact RabbitMQ 4.x release such as 4.2.9; the repository
# keeps past releases and an APT pin installs exactly that one. Upgrading is
# supported; going back isn't. Team RabbitMQ publishes Erlang for amd64 only,
# and Debian's own Erlang is too old for RabbitMQ 4, so other architectures
# aren't supported.
#
#   --username NAME       create an administrator (needs --password)
#   --password PASSWORD   the administrator's password
#   --vhost NAME          create a virtual host; the user gets full access to
#                         it (otherwise to the default `/`)
#
# RabbitMQ listens on 127.0.0.1:5672, with the management UI on
# http://localhost:15672. The default `guest` user stays, limited to localhost
# as usual. Under systemd it runs as rabbitmq-server.

RABBITMQ_KEY_URL=https://keys.openpgp.org/vks/v1/by-fingerprint/0A9AF2115F4687BD29803A206B73A36E6026DFCA
RABBITMQ_KEY_FINGERPRINT=0A9AF2115F4687BD29803A206B73A36E6026DFCA
RABBITMQ_PIN=/etc/apt/preferences.d/rabbitmq
RABBITMQ_CONFIG=/etc/rabbitmq/rabbitmq.conf
RABBITMQ_PLUGINS=/etc/rabbitmq/enabled_plugins

option username ""
option password ""
option vhost ""
parse_args "$@"

[[ $VERSION =~ ^4\.[0-9]+\.[0-9]+$ ]] ||
  usage_error "version must be a RabbitMQ 4.x release like 4.2.9 (got '$VERSION'); 3.x is out of support"
[ -z "$username" ] || [[ $username =~ ^[A-Za-z0-9_.-]+$ ]] ||
  usage_error "username may only use letters, digits, _, . and - (got '$username')"
[ -z "$vhost" ] || [[ $vhost =~ ^[A-Za-z0-9_.-]+$ ]] ||
  usage_error "vhost may only use letters, digits, _, . and - (got '$vhost')"
[ -z "$username" ] || [ -n "$password" ] || usage_error "--username needs --password"
[ -z "$password" ] || [ -n "$username" ] || usage_error "--password needs --username"

# Read in a subshell: os-release defines its own VERSION.
os=$(. /etc/os-release && echo "${ID:-unknown}")
codename=$(. /etc/os-release && echo "${VERSION_CODENAME:-}")
[ "$os" = debian ] || die "only Debian is supported (this system is '$os')"
arch=$(dpkg --print-architecture)
[ "$arch" = amd64 ] ||
  die "Team RabbitMQ publishes Erlang for amd64 only, and Debian's Erlang is too old for RabbitMQ 4 ($arch isn't supported)"

installed=$(installed_version rabbitmq-server)
if [ -n "$installed" ] && [ "$installed" != "$VERSION" ] &&
  dpkg --compare-versions "$VERSION" lt "$installed"; then
  die "RabbitMQ $installed is installed; going back to $VERSION isn't supported"
fi

apt_keyring rabbitmq "$RABBITMQ_KEY_URL" "$RABBITMQ_KEY_FINGERPRINT"
repo="[signed-by=/etc/apt/keyrings/rabbitmq.gpg] https://deb1.rabbitmq.com"
apt_source rabbitmq "deb $repo/rabbitmq-erlang/debian/$codename $codename main
deb $repo/rabbitmq-server/debian/$codename $codename main"
# Erlang from Team RabbitMQ rather than Debian, and exactly this release.
write_file "$RABBITMQ_PIN" "Package: erlang*
Pin: origin deb1.rabbitmq.com
Pin-Priority: 1001

Package: rabbitmq-server
Pin: version $VERSION-*
Pin-Priority: 1001"

upgraded=
if [ "$installed" != "$VERSION" ]; then
  apt_update
  require_published rabbitmq-server RabbitMQ
  if [ -n "$installed" ]; then
    info "Upgrading RabbitMQ $installed to $VERSION"
    quietly as_root env DEBIAN_FRONTEND=noninteractive \
      apt-get -o Acquire::Retries=3 -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold install -y --no-install-recommends rabbitmq-server
    upgraded=1
  else
    info "Installing RabbitMQ $VERSION"
    apt_install rabbitmq-server
  fi
fi

# RabbitMQ's tools expect a UTF-8 locale.
rabbitmqctl() {
  as_root env LC_ALL=C.UTF-8 /usr/sbin/rabbitmqctl "$@"
}

write_file "$RABBITMQ_CONFIG" "# Managed by Paddock (paddock setup rabbitmq).
listeners.tcp.default = 127.0.0.1:5672
management.tcp.ip = 127.0.0.1
management.tcp.port = 15672"
restart=$WROTE
if ! as_root cat "$RABBITMQ_PLUGINS" 2>/dev/null | grep -q rabbitmq_management; then
  quietly as_root env LC_ALL=C.UTF-8 /usr/sbin/rabbitmq-plugins enable --offline rabbitmq_management
  restart=1
fi

# `await_startup` fails at once if the node hasn't appeared yet, so retry it.
wait_until_ready() {
  local tries
  for tries in $(seq 60); do
    rabbitmqctl await_startup --timeout 5 >/dev/null 2>&1 && return 0
    sleep 2
  done
  die "RabbitMQ didn't start; see /var/log/rabbitmq/ and journalctl -u rabbitmq-server"
}

temporary=
if has_systemd; then
  was_running=$(systemctl is-active rabbitmq-server 2>/dev/null) || true
  quietly as_root systemctl enable --now rabbitmq-server
  if [ "$was_running" = active ] && [ -n "$restart$upgraded" ]; then
    quietly as_root systemctl restart rabbitmq-server
  fi
  wait_until_ready
elif [ -n "$username$vhost" ] && ! rabbitmqctl status >/dev/null 2>&1; then
  # No systemd: start a node just long enough to apply the flags.
  quietly as_root env LC_ALL=C.UTF-8 /usr/sbin/rabbitmq-server -detached
  temporary=1
  wait_until_ready
fi

if [ -n "$vhost" ] && ! rabbitmqctl list_vhosts --silent | grep -qx "$vhost"; then
  quietly rabbitmqctl add_vhost "$vhost"
fi
if [ -n "$username" ]; then
  # Passwords go through stdin, never a command line.
  if rabbitmqctl list_users --silent | awk '{ print $1 }' | grep -qx "$username"; then
    printf '%s' "$password" | quietly rabbitmqctl change_password "$username"
  else
    printf '%s' "$password" | quietly rabbitmqctl add_user "$username"
  fi
  quietly rabbitmqctl set_user_tags "$username" administrator
  quietly rabbitmqctl set_permissions -p "${vhost:-/}" "$username" '.*' '.*' '.*'
fi

if [ -n "$temporary" ]; then
  quietly rabbitmqctl stop
fi

details=("management UI on http://localhost:15672")
[ -z "$username" ] || details+=("user: $username")
[ -z "$vhost" ] || details+=("vhost: $vhost")
summary=" ($(IFS=,; echo "${details[*]}" | sed 's/,/, /g'))"

if has_systemd; then
  info "RabbitMQ $VERSION is running on 127.0.0.1:5672$summary"
else
  info "RabbitMQ $VERSION is installed$summary"
  warn "systemd isn't running, so RabbitMQ won't start on its own; start it with: sudo rabbitmq-server -detached"
fi
