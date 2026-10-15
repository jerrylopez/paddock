# opensearch.sh: installs OpenSearch from its official APT repositories, or
# from its release tarballs for series that predate them.
# Usage (via Paddock): paddock setup opensearch <major.minor> [--heap SIZE]
#
# <major.minor> is an OpenSearch release series from 1.2 on, such as 3.9;
# an APT pin installs the latest patch in it. The APT repositories start at
# 1.3 and 2.5, so 1.2 and 2.0 to 2.4 get their final release from the signed
# tarball, laid out like the package and run by a unit Paddock writes. Moving
# to a newer series within a major upgrades (from a tarball to the package
# too); going back or to another major isn't supported.
# Configured for local development: one node on 127.0.0.1:9200 with the
# security plugin disabled (no TLS, no passwords), and a small JVM heap
# (--heap, default 512m). Under systemd, OpenSearch is enabled and started.

OPENSEARCH_CONFIG_DIR=/etc/opensearch
OPENSEARCH_CONFIG=$OPENSEARCH_CONFIG_DIR/opensearch.yml
OPENSEARCH_HEAP=$OPENSEARCH_CONFIG_DIR/jvm.options.d/paddock-heap.options
OPENSEARCH_URL=http://127.0.0.1:9200
# Tarball installs: one directory per release, and `current`.
OPENSEARCH_HOME=/opt/opensearch
OPENSEARCH_UNIT=/etc/systemd/system/opensearch.service

option heap 512m
parse_args "$@"

[[ $VERSION =~ ^([0-9]+)\.[0-9]+$ ]] ||
  usage_error "version must be an OpenSearch series like 3.9 (got '$VERSION')"
major=${BASH_REMATCH[1]}

# The series without an APT repository, and their final releases.
tarball=
case $VERSION in
  1.0 | 1.1) usage_error "OpenSearch $VERSION isn't supported; the oldest supported series is 1.2" ;;
  1.2) tarball=1.2.4 ;;
  2.0) tarball=2.0.1 ;;
  2.1) tarball=2.1.0 ;;
  2.2) tarball=2.2.1 ;;
  2.3) tarball=2.3.0 ;;
  2.4) tarball=2.4.1 ;;
esac

# Each major's repository is signed by a different key; 1.x and 2.x share one,
# which also signs their tarballs.
case $major in
  1 | 2)
    key_url=https://artifacts.opensearch.org/publickeys/opensearch.pgp
    key_fingerprint=C5B7498965EFD1C2924BA9D539D319879310D3FC
    ;;
  3)
    key_url=https://artifacts.opensearch.org/publickeys/opensearch-release.pgp
    key_fingerprint=A8B2D9E04CD51FEF6AA2DB53BA81D99981191457
    ;;
  *) usage_error "OpenSearch $VERSION isn't supported; use a 1.x (from 1.2), 2.x or 3.x series" ;;
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

# The installed release, from the package or else a tarball install.
tarball_installed=$(readlink "$OPENSEARCH_HOME/current" 2>/dev/null) || tarball_installed=
tarball_installed=${tarball_installed##*/}
if package_installed opensearch; then
  installed_release=$(installed_version opensearch)
else
  installed_release=$tarball_installed
fi
installed=$(cut -d. -f1-2 <<<"$installed_release")
if [ -n "$installed" ] && [ "${installed%%.*}" != "$major" ]; then
  die "OpenSearch $installed is installed; switching to another major version isn't supported (remove the opensearch package and /var/lib/opensearch first)"
fi
if [ -n "$installed" ] && [ "$installed" != "$VERSION" ] &&
  dpkg --compare-versions "$VERSION" lt "$installed"; then
  die "OpenSearch $installed is installed; going back to $VERSION isn't supported"
fi

keyring=/etc/apt/keyrings/opensearch-$major.gpg

# install_tarball: install release $tarball under $OPENSEARCH_HOME, with its
# configuration in $OPENSEARCH_CONFIG_DIR and data and logs where the package
# keeps them, after checking the download's signature.
install_tarball() {
  local url dir home
  url=https://artifacts.opensearch.org/releases/bundle/opensearch/$tarball/opensearch-$tarball-linux-${arch/amd64/x64}.tar.gz
  apt_keyring "opensearch-$major" "$key_url" "$key_fingerprint"
  dir=$(mktemp -d)
  download "$url" "$dir/opensearch.tar.gz"
  download "$url.sig" "$dir/opensearch.tar.gz.sig"
  if ! gpgv --keyring "$keyring" "$dir/opensearch.tar.gz.sig" "$dir/opensearch.tar.gz" >/dev/null 2>&1; then
    rm -rf "$dir"
    die "the OpenSearch download from $url doesn't match its signature"
  fi
  home=$OPENSEARCH_HOME/$tarball
  quietly as_root rm -rf "$home" "$OPENSEARCH_HOME/opensearch-$tarball"
  quietly as_root mkdir -p "$OPENSEARCH_HOME"
  quietly as_root tar -xzf "$dir/opensearch.tar.gz" -C "$OPENSEARCH_HOME"
  rm -rf "$dir"
  quietly as_root mv "$OPENSEARCH_HOME/opensearch-$tarball" "$home"

  # The same user, directories and ownership as the package.
  getent group opensearch >/dev/null || quietly as_root groupadd --system opensearch
  getent passwd opensearch >/dev/null ||
    quietly as_root useradd --system --gid opensearch --no-create-home --shell /usr/sbin/nologin opensearch
  quietly as_root mkdir -p "$OPENSEARCH_CONFIG_DIR" /var/lib/opensearch /var/log/opensearch
  # Like a package upgrade: the release's files replace its earlier ones; any
  # other files stay. opensearch.yml and the heap are written below.
  quietly as_root cp -R "$home/config/." "$OPENSEARCH_CONFIG_DIR/"
  # Absolute data and log paths, as the package has them, and the settings
  # the release's opensearch-tar-install.sh adds for Performance Analyzer
  # (its policy file moved from the plugin to the config directory in 2.x).
  local policy=$OPENSEARCH_CONFIG_DIR/opensearch-performance-analyzer/opensearch_security.policy
  [ -f "$policy" ] || policy=$home/plugins/opensearch-performance-analyzer/pa_config/opensearch_security.policy
  quietly as_root sed -i -E 's#=data$#=/var/lib/opensearch#; s#([=:])logs/#\1/var/log/opensearch/#' "$OPENSEARCH_CONFIG_DIR/jvm.options"
  printf '%s\n' "" "## OpenSearch Performance Analyzer" "-Dclk.tck=$(getconf CLK_TCK)" \
    "-Djdk.attach.allowAttachSelf=true" "-Djava.security.policy=$policy" \
    "--add-opens=jdk.attach/sun.tools.attach=ALL-UNNAMED" |
    as_root tee -a "$OPENSEARCH_CONFIG_DIR/jvm.options" >/dev/null
  quietly as_root chown -R opensearch:opensearch "$home" "$OPENSEARCH_CONFIG_DIR" /var/lib/opensearch /var/log/opensearch
  quietly as_root ln -sfn "$home" "$OPENSEARCH_HOME/current"
  [ -z "$tarball_installed" ] || [ "$tarball_installed" = "$tarball" ] ||
    quietly as_root rm -rf "${OPENSEARCH_HOME:?}/$tarball_installed"
}

# remove_tarball: remove a tarball install before the package replaces it,
# keeping the data in /var/lib/opensearch. Its configuration goes too, so the
# package installs its own.
remove_tarball() {
  if has_systemd; then
    quietly as_root systemctl disable --now opensearch
    quietly as_root rm -f "$OPENSEARCH_UNIT"
    quietly as_root systemctl daemon-reload
  fi
  quietly as_root rm -rf "$OPENSEARCH_HOME" "$OPENSEARCH_CONFIG_DIR"
}

upgraded=
if [ -n "$tarball" ]; then
  if [ "$installed" != "$VERSION" ]; then
    if [ -n "$installed" ]; then
      info "Upgrading OpenSearch $installed to $VERSION"
      upgraded=1
    else
      info "Installing OpenSearch $VERSION"
    fi
    apt_install ca-certificates curl gpgv
    install_tarball
  fi
else
  apt_keyring "opensearch-$major" "$key_url" "$key_fingerprint"
  apt_source opensearch "deb [signed-by=$keyring] https://artifacts.opensearch.org/releases/bundle/opensearch/$major.x/apt stable main"
  apt_pin opensearch "$VERSION.*" opensearch

  if [ "$installed" != "$VERSION" ]; then
    apt_update
    require_series opensearch OpenSearch
    if [ -n "$installed" ]; then
      info "Upgrading OpenSearch $installed to $VERSION"
      upgraded=1
    else
      info "Installing OpenSearch $VERSION"
    fi
    [ -z "$tarball_installed" ] || package_installed opensearch || remove_tarball
    # 2.5.0's post-install script fails without sysctl.
    [ "$VERSION" != 2.5 ] || apt_install procps
    # Skip the demo security setup (3.7+ honour these). Older packages insist on
    # an initial admin password; it goes unused because security is disabled.
    quietly as_root env DEBIAN_FRONTEND=noninteractive \
      DISABLE_INSTALL_DEMO_CONFIG=true DISABLE_SECURITY_PLUGIN=true \
      OPENSEARCH_INITIAL_ADMIN_PASSWORD="Paddock-$(od -An -N12 -tx1 /dev/urandom | tr -d ' \n')!" \
      apt-get -o Acquire::Retries=3 -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold install -y --no-install-recommends opensearch
  fi
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

if [ -n "$tarball" ]; then
  release=$tarball
else
  release=$(installed_version opensearch)
fi

if has_systemd; then
  if [ -n "$tarball" ]; then
    # The package's unit, adapted for the tarball: OpenSearch runs in the
    # foreground, with k-NN's native libraries found as the tarball's own
    # start script does.
    write_file "$OPENSEARCH_UNIT" "# Managed by Paddock (paddock setup opensearch).
[Unit]
Description=OpenSearch
Documentation=https://opensearch.org/

[Service]
Type=simple
RuntimeDirectory=opensearch
PrivateTmp=true
Environment=OPENSEARCH_PATH_CONF=$OPENSEARCH_CONFIG_DIR
Environment=LD_LIBRARY_PATH=$OPENSEARCH_HOME/current/plugins/opensearch-knn/lib:$OPENSEARCH_HOME/current/plugins/opensearch-knn/knnlib
WorkingDirectory=$OPENSEARCH_HOME/current
User=opensearch
Group=opensearch
ExecStart=$OPENSEARCH_HOME/current/bin/opensearch -p /run/opensearch/opensearch.pid --quiet
LimitNOFILE=65535
LimitNPROC=4096
LimitAS=infinity
LimitFSIZE=infinity
TimeoutStopSec=0
KillSignal=SIGTERM
KillMode=process
SendSIGKILL=no
SuccessExitStatus=143

[Install]
WantedBy=multi-user.target"
    [ -z "$WROTE" ] || quietly as_root systemctl daemon-reload
  fi
  # Restart only a node that was already running before this setup.
  was_running=$(systemctl is-active opensearch 2>/dev/null) || true
  quietly as_root systemctl enable --now opensearch
  if [ "$was_running" = active ] && [ -n "$restart$upgraded" ]; then
    quietly as_root systemctl restart opensearch
  fi
  wait_until_ready
  info "OpenSearch $release is running on http://localhost:9200 (heap $heap, security disabled)"
else
  info "OpenSearch $release is installed (heap $heap, security disabled)"
  if [ -n "$tarball" ]; then
    warn "systemd isn't running, so OpenSearch won't start on its own; start it with: sudo -u opensearch env OPENSEARCH_PATH_CONF=$OPENSEARCH_CONFIG_DIR $OPENSEARCH_HOME/current/bin/opensearch -d"
  else
    warn "systemd isn't running, so OpenSearch won't start on its own"
  fi
fi
