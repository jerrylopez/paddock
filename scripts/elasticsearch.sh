# elasticsearch.sh: installs Elasticsearch from Elastic's official APT repository.
# Usage (via Paddock): paddock setup elasticsearch <version> [--heap SIZE]
#
# <version> is an exact Elasticsearch 8 or 9 release such as 9.5.5; Elastic's
# repository keeps past releases and an APT pin installs exactly that one.
# Upgrading within a major is supported; going back or to another major isn't.
# Configured for local development: one node on 127.0.0.1:9200 with security
# disabled (no TLS, no passwords), and a small JVM heap (--heap, default
# 512m). Under systemd, Elasticsearch is enabled and started.

ELASTIC_KEY_URL=https://artifacts.elastic.co/GPG-KEY-elasticsearch
ELASTIC_KEY_FINGERPRINT=46095ACC8548582C1A2699A9D27D666CD88E42B4
ELASTIC_CONFIG=/etc/elasticsearch/elasticsearch.yml
ELASTIC_HEAP=/etc/elasticsearch/jvm.options.d/paddock-heap.options
ELASTIC_URL=http://127.0.0.1:9200

option heap 512m
parse_args "$@"

[[ $VERSION =~ ^([0-9]+)\.[0-9]+\.[0-9]+$ ]] ||
  usage_error "version must be an Elasticsearch release like 9.5.5 (got '$VERSION')"
major=${BASH_REMATCH[1]}
case $major in
  8 | 9) ;;
  *) usage_error "Elasticsearch $VERSION isn't supported; use an 8.x or 9.x release" ;;
esac
[[ $heap =~ ^[0-9]+[mg]$ ]] || usage_error "--heap must be a size like 512m or 2g (got '$heap')"

# Read in a subshell: os-release defines its own VERSION.
os=$(. /etc/os-release && echo "${ID:-unknown}")
[ "$os" = debian ] || die "only Debian is supported (this system is '$os')"
arch=$(dpkg --print-architecture)
case $arch in
  amd64 | arm64) ;;
  *) die "Elastic publishes no packages for $arch" ;;
esac
# Both answer on port 9200.
! package_installed opensearch ||
  die "OpenSearch is installed and uses the same port; remove it before setting up Elasticsearch"

installed=$(installed_version elasticsearch)
if [ -n "$installed" ] && [ "${installed%%.*}" != "$major" ]; then
  die "Elasticsearch $installed is installed; switching to another major version isn't supported (remove the elasticsearch package and /var/lib/elasticsearch first)"
fi
if [ -n "$installed" ] && [ "$installed" != "$VERSION" ] &&
  dpkg --compare-versions "$VERSION" lt "$installed"; then
  die "Elasticsearch $installed is installed; going back to $VERSION isn't supported"
fi

apt_keyring elasticsearch "$ELASTIC_KEY_URL" "$ELASTIC_KEY_FINGERPRINT"
apt_source elasticsearch "deb [signed-by=/etc/apt/keyrings/elasticsearch.gpg] https://artifacts.elastic.co/packages/$major.x/apt stable main"
apt_pin elasticsearch "$VERSION" elasticsearch

upgraded=
if [ "$installed" != "$VERSION" ]; then
  apt_update
  require_published elasticsearch Elasticsearch
  if [ -n "$installed" ]; then
    info "Upgrading Elasticsearch $installed to $VERSION"
    quietly as_root env DEBIAN_FRONTEND=noninteractive \
      apt-get -o Acquire::Retries=3 -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold install -y --no-install-recommends elasticsearch
    upgraded=1
  else
    info "Installing Elasticsearch $VERSION"
    # The package sets up TLS and an `elastic` password on install; the config
    # below turns security off, so they go unused.
    apt_install elasticsearch
  fi
fi

restart=
write_file "$ELASTIC_CONFIG" "# Managed by Paddock (paddock setup elasticsearch): one local node for development.
cluster.name: paddock
path.data: /var/lib/elasticsearch
path.logs: /var/log/elasticsearch
network.host: 127.0.0.1
http.port: 9200
discovery.type: single-node
xpack.security.enabled: false"
[ -z "$WROTE" ] || restart=1
write_file "$ELASTIC_HEAP" "-Xms$heap
-Xmx$heap"
[ -z "$WROTE" ] || restart=1
# The package's files in /etc/elasticsearch are readable by its group only.
as_root chown root:elasticsearch "$ELASTIC_CONFIG" "$ELASTIC_HEAP"
as_root chmod 660 "$ELASTIC_CONFIG" "$ELASTIC_HEAP"

# The install-time security setup also stores TLS passwords in the keystore;
# with security off, Elasticsearch refuses to start while they're there.
if [ -n "$restart" ]; then
  keystore=/usr/share/elasticsearch/bin/elasticsearch-keystore
  for setting in $(as_root "$keystore" list 2>/dev/null | grep '^xpack\.security\.'); do
    quietly as_root "$keystore" remove "$setting"
  done
fi

# Ready means the node answers and its indices are usable (health yellow+).
wait_until_ready() {
  local tries
  for tries in $(seq 180); do
    curl -fsS "$ELASTIC_URL/_cluster/health?wait_for_status=yellow&timeout=1s" >/dev/null 2>&1 && return 0
    sleep 1
  done
  die "Elasticsearch didn't start; see /var/log/elasticsearch/paddock.log and journalctl -u elasticsearch"
}

if has_systemd; then
  # Restart only a node that was already running before this setup.
  was_running=$(systemctl is-active elasticsearch 2>/dev/null) || true
  quietly as_root systemctl enable --now elasticsearch
  if [ "$was_running" = active ] && [ -n "$restart$upgraded" ]; then
    quietly as_root systemctl restart elasticsearch
  fi
  wait_until_ready
  info "Elasticsearch $VERSION is running on http://localhost:9200 (heap $heap, security disabled)"
else
  info "Elasticsearch $VERSION is installed (heap $heap, security disabled)"
  warn "systemd isn't running, so Elasticsearch won't start on its own"
fi
