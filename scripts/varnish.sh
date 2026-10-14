# varnish.sh: installs Varnish Cache from its official packagecloud repositories.
# Usage (via Paddock): paddock setup varnish <major.minor> [flags...]
#
# <major.minor> is a supported release series: 6.0 (LTS), 7.6, 7.7 or 8.0.
# Each has its own repository; an APT pin installs the latest patch in it.
# Running setup with another series switches to it (Varnish only holds a
# cache, so going back is fine too).
#
#   --backend HOST:PORT   the server Varnish caches (default 127.0.0.1:8080)
#   --port PORT           where Varnish listens on 127.0.0.1 (default 6081)
#   --memory SIZE         cache size in memory (default 256m)
#
# Paddock writes /etc/varnish/paddock.vcl and a systemd drop-in that starts
# varnishd with it; the package's default.vcl is left alone.

VARNISH_VCL=/etc/varnish/paddock.vcl
VARNISH_DROPIN=/etc/systemd/system/varnish.service.d/paddock.conf

option backend 127.0.0.1:8080
option port 6081
option memory 256m
parse_args "$@"

# Each series has its own repository and signing key.
case $VERSION in
  6.0) repo=varnish60lts key=48D81A24CB0456F5D59431D94CFCFD6BA750EDCD ;;
  7.6) repo=varnish76 key=6B803A8DF5983DDEB704FF7435BA265D95368ADC ;;
  7.7) repo=varnish77 key=7B9C4CC47FCF4E7D1B39889094C63CC2D7584016 ;;
  8.0) repo=varnish80 key=42E4EDD233A3463A90671EEAFC3FECD870B8A552 ;;
  *) require_version 6.0 7.6 7.7 8.0 ;;
esac
[[ $backend =~ ^([A-Za-z0-9.-]+):([0-9]+)$ ]] ||
  usage_error "--backend must be HOST:PORT (got '$backend')"
backend_host=${BASH_REMATCH[1]} backend_port=${BASH_REMATCH[2]}
[[ $port =~ ^[0-9]+$ ]] || usage_error "--port must be a number (got '$port')"
[[ $memory =~ ^[0-9]+[kmg]$ ]] || usage_error "--memory must be a size like 256m or 1g (got '$memory')"

# Read in a subshell: os-release defines its own VERSION.
os=$(. /etc/os-release && echo "${ID:-unknown}")
codename=$(. /etc/os-release && echo "${VERSION_CODENAME:-}")
[ "$os" = debian ] || die "only Debian is supported (this system is '$os')"
case $(dpkg --print-architecture) in
  amd64 | arm64) ;;
  *) die "Varnish publishes no packages for $(dpkg --print-architecture)" ;;
esac

apt_keyring "varnish-$repo" "https://packagecloud.io/varnishcache/$repo/gpgkey" "$key"
apt_source varnish "deb [signed-by=/etc/apt/keyrings/varnish-$repo.gpg] https://packagecloud.io/varnishcache/$repo/debian/ $codename main"
# The latest release in this series, over Debian's own Varnish (7.1).
apt_pin varnish "$VERSION.*" varnish

installed=$(installed_series varnish)
switching=
if [ "$installed" != "$VERSION" ]; then
  apt_update
  require_series varnish Varnish
  if [ -n "$installed" ]; then
    info "Switching Varnish $installed to $VERSION"
    switching=1
  else
    info "Installing Varnish $VERSION"
  fi
  quietly as_root env DEBIAN_FRONTEND=noninteractive \
    apt-get -o Acquire::Retries=3 -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold install -y --allow-downgrades --no-install-recommends varnish
fi

write_file "$VARNISH_VCL" "# Managed by Paddock (paddock setup varnish).
vcl 4.1;

backend default {
    .host = \"$backend_host\";
    .port = \"$backend_port\";
}"
restart=$WROTE
quietly as_root /usr/sbin/varnishd -C -f "$VARNISH_VCL"

start="/usr/sbin/varnishd -F -a 127.0.0.1:$port -f $VARNISH_VCL -s malloc,$memory"
if has_systemd; then
  # The package units differ by series; run varnishd in the foreground the same way for all.
  write_file "$VARNISH_DROPIN" "# Managed by Paddock (paddock setup varnish).
[Service]
Type=simple
PIDFile=
ExecStart=
ExecStart=$start"
  if [ -n "$WROTE" ]; then
    quietly as_root systemctl daemon-reload
    restart=1
  fi
  was_running=$(systemctl is-active varnish 2>/dev/null) || true
  quietly as_root systemctl enable --now varnish
  if [ "$was_running" = active ] && [ -n "$restart$switching" ]; then
    quietly as_root systemctl restart varnish
  fi
  tries=0
  until curl -s -o /dev/null "http://127.0.0.1:$port/"; do
    tries=$((tries + 1))
    [ "$tries" -lt 30 ] || die "Varnish didn't start; see journalctl -u varnish"
    sleep 1
  done
  info "Varnish $(installed_version varnish) is running on 127.0.0.1:$port, caching $backend"
else
  info "Varnish $(installed_version varnish) is installed, caching $backend"
  warn "systemd isn't running, so Varnish won't start on its own; start it with: sudo ${start/ -F / }"
fi
