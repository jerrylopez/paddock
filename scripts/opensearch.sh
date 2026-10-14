# opensearch.sh: installs OpenSearch from its official APT repository.
# Usage (via Paddock): paddock setup opensearch <major> [--heap SIZE]
#
# <major> is 2 or 3; the repository provides the latest release in it.
# Configured for local development: one node on 127.0.0.1:9200 with the
# security plugin disabled (no TLS, no passwords), and a small JVM heap
# (--heap, default 512m). Under systemd, OpenSearch is enabled and started.

OPENSEARCH_SOURCE=/etc/apt/sources.list.d/opensearch.list
OPENSEARCH_CONFIG=/etc/opensearch/opensearch.yml
OPENSEARCH_HEAP=/etc/opensearch/jvm.options.d/paddock-heap.options
OPENSEARCH_URL=http://127.0.0.1:9200

option heap 512m
parse_args "$@"

# Each major's repository is signed by a different key.
case $VERSION in
  2)
    key_url=https://artifacts.opensearch.org/publickeys/opensearch.pgp
    key_fingerprint=C5B7498965EFD1C2924BA9D539D319879310D3FC
    ;;
  3)
    key_url=https://artifacts.opensearch.org/publickeys/opensearch-release.pgp
    key_fingerprint=A8B2D9E04CD51FEF6AA2DB53BA81D99981191457
    ;;
  *) require_version 2 3 ;;
esac
[[ $heap =~ ^[0-9]+[mg]$ ]] || usage_error "--heap must be a size like 512m or 2g (got '$heap')"

# Read in a subshell: os-release defines its own VERSION.
os=$(. /etc/os-release && echo "${ID:-unknown}")
[ "$os" = debian ] || die "only Debian is supported (this system is '$os')"
arch=$(dpkg --print-architecture)
case $arch in
  amd64 | arm64) ;;
  *) die "OpenSearch publishes no packages for $arch" ;;
esac
# Both answer on port 9200.
! package_installed elasticsearch ||
  die "Elasticsearch is installed and uses the same port; remove it before setting up OpenSearch"

line="deb [signed-by=/etc/apt/keyrings/opensearch-$VERSION.gpg] https://artifacts.opensearch.org/releases/bundle/opensearch/$VERSION.x/apt stable main"
if package_installed opensearch && [ -f "$OPENSEARCH_SOURCE" ] && [ "$(cat "$OPENSEARCH_SOURCE")" != "$line" ]; then
  die "OpenSearch from another major version is installed; switching to $VERSION isn't supported (remove the opensearch package and /var/lib/opensearch first)"
fi
apt_keyring "opensearch-$VERSION" "$key_url" "$key_fingerprint"
apt_source opensearch "$line"

if ! package_installed opensearch; then
  info "Installing OpenSearch $VERSION"
  apt_update
  # Skip the demo security setup (3.7+ honour these). Older packages insist on
  # an initial admin password; it goes unused because security is disabled.
  quietly as_root env DEBIAN_FRONTEND=noninteractive \
    DISABLE_INSTALL_DEMO_CONFIG=true DISABLE_SECURITY_PLUGIN=true \
    OPENSEARCH_INITIAL_ADMIN_PASSWORD="Paddock-$(od -An -N12 -tx1 /dev/urandom | tr -d ' \n')!" \
    apt-get install -y --no-install-recommends opensearch
fi

config="# Managed by Paddock (paddock setup opensearch): one local node for development.
cluster.name: paddock
path.data: /var/lib/opensearch
path.logs: /var/log/opensearch
network.host: 127.0.0.1
http.port: 9200
discovery.type: single-node
plugins.security.disabled: true"

restart=
write_file "$OPENSEARCH_CONFIG" "$config"
[ -z "$WROTE" ] || restart=1
write_file "$OPENSEARCH_HEAP" "-Xms$heap
-Xmx$heap"
[ -z "$WROTE" ] || restart=1

# Ready means the node answers and its indices are usable (health yellow+).
wait_until_ready() {
  local tries
  for tries in $(seq 120); do
    curl -fsS "$OPENSEARCH_URL/_cluster/health?wait_for_status=yellow&timeout=1s" >/dev/null 2>&1 && return 0
    sleep 1
  done
  die "OpenSearch didn't start; see /var/log/opensearch/paddock.log and journalctl -u opensearch"
}

opensearch_version=$(dpkg-query -W -f='${Version}' opensearch)
if has_systemd; then
  quietly as_root systemctl enable --now opensearch
  if [ -n "$restart" ]; then
    quietly as_root systemctl restart opensearch
  fi
  wait_until_ready
  info "OpenSearch $opensearch_version is running on http://localhost:9200 (heap $heap, security disabled)"
else
  info "OpenSearch $opensearch_version is installed (heap $heap, security disabled)"
  warn "systemd isn't running, so OpenSearch won't start on its own"
fi
