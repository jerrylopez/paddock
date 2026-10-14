# nginx.sh: installs nginx from nginx.org's official APT repository.
# Usage (via Paddock): paddock setup nginx <major.minor>
#
# <major.minor> is an nginx release series such as 1.30; the latest patch in
# it is installed. nginx.org publishes even series (1.30) in its stable
# repository and odd ones (1.31) in its mainline repository. Running setup
# with another series switches to it, older or newer. The package's default site
# answers on port 80. Under systemd, nginx is enabled and started.

NGINX_KEY_URL=https://nginx.org/keys/nginx_signing.key
# The keys nginx.org lists for verifying its packages.
NGINX_KEY_FINGERPRINTS=(
  573BFD6B3D8FBC641079A6ABABF5BD827BD9BF62
  8540A6F18833A80E9C1653A42FD21310B49F6B46
  9E9BE90EACBCDE69FE9B204CBCDCD8A38D88A2B3
)

parse_args "$@"

[[ $VERSION =~ ^1\.([0-9]+)$ ]] ||
  usage_error "version must be an nginx series like 1.30 (got '$VERSION')"
if (( BASH_REMATCH[1] % 2 == 0 )); then
  path=packages/debian
else
  path=packages/mainline/debian
fi

# Read in a subshell: os-release defines its own VERSION.
os=$(. /etc/os-release && echo "${ID:-unknown}")
codename=$(. /etc/os-release && echo "${VERSION_CODENAME:-}")
[ "$os" = debian ] || die "only Debian is supported (this system is '$os')"
arch=$(dpkg --print-architecture)
case $arch in
  amd64 | arm64) ;;
  *) die "nginx.org has no packages for $arch" ;;
esac

apt_keyring nginx "$NGINX_KEY_URL" "${NGINX_KEY_FINGERPRINTS[@]}"
apt_source nginx "deb [signed-by=/etc/apt/keyrings/nginx.gpg] https://nginx.org/$path $codename nginx"
# The latest release in this series, over Debian's own nginx and newer series.
apt_pin nginx "$VERSION.*" nginx

installed=$(installed_series nginx)
if [ "$installed" != "$VERSION" ]; then
  apt_update
  require_series nginx nginx
  if [ -n "$installed" ]; then
    info "Switching nginx $installed to $VERSION"
  else
    info "Installing nginx $VERSION"
  fi
  # nginx keeps no data, so going back to an older series is fine.
  quietly as_root env DEBIAN_FRONTEND=noninteractive \
    apt-get -o Acquire::Retries=3 -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold install -y --allow-downgrades --no-install-recommends nginx
fi

quietly as_root /usr/sbin/nginx -t

if has_systemd; then
  was_running=$(systemctl is-active nginx 2>/dev/null) || true
  quietly as_root systemctl enable --now nginx
  # Pick up a new binary after switching series.
  if [ "$was_running" = active ] && [ "$installed" != "$VERSION" ]; then
    quietly as_root systemctl restart nginx
  fi
  info "nginx $(installed_version nginx) is running on port 80"
else
  info "nginx $(installed_version nginx) is installed"
  warn "systemd isn't running, so nginx won't start on its own; start it with: sudo nginx"
fi
