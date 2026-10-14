# nginx.sh: installs nginx from nginx.org's official APT repository.
# Usage (via Paddock): paddock setup nginx stable|mainline
#
# The version picks one of nginx.org's two channels; the repository provides
# the latest release in it. The package's default site answers on port 80.
# Under systemd, nginx is enabled and started.

NGINX_KEY_URL=https://nginx.org/keys/nginx_signing.key
# The keys nginx.org lists for verifying its packages.
NGINX_KEY_FINGERPRINTS=(
  573BFD6B3D8FBC641079A6ABABF5BD827BD9BF62
  8540A6F18833A80E9C1653A42FD21310B49F6B46
  9E9BE90EACBCDE69FE9B204CBCDCD8A38D88A2B3
)
NGINX_SOURCE=/etc/apt/sources.list.d/nginx.list
NGINX_PIN=/etc/apt/preferences.d/nginx

parse_args "$@"

case $VERSION in
  stable) path=packages/debian ;;
  mainline) path=packages/mainline/debian ;;
  *) require_version stable mainline ;;
esac

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
line="deb [signed-by=/etc/apt/keyrings/nginx.gpg] https://nginx.org/$path $codename nginx"
switching=
if [ ! -f "$NGINX_SOURCE" ] || [ "$(cat "$NGINX_SOURCE")" != "$line" ]; then
  switching=1
fi
apt_source nginx "$line"

# Prefer nginx.org's packages over Debian's own nginx. A priority above 1000
# also lets apt move from mainline back to the lower-numbered stable.
pin=$'Package: *\nPin: origin nginx.org\nPin-Priority: 1001'
if [ ! -f "$NGINX_PIN" ] || [ "$(cat "$NGINX_PIN")" != "$pin" ]; then
  printf '%s\n' "$pin" | as_root tee "$NGINX_PIN" >/dev/null
fi

if ! package_installed nginx; then
  info "Installing nginx ($VERSION)"
  apt_install nginx
elif [ -n "$switching" ]; then
  # Installed from the other channel, or from Debian: install this channel's.
  info "Switching nginx to $VERSION"
  apt_update
  quietly as_root env DEBIAN_FRONTEND=noninteractive \
    apt-get -o Acquire::Retries=3 install -y --allow-downgrades --no-install-recommends nginx
fi

quietly as_root /usr/sbin/nginx -t

nginx_version=$(/usr/sbin/nginx -v 2>&1 | sed -n 's|^nginx version: nginx/||p')
if has_systemd; then
  quietly as_root systemctl enable --now nginx
  # Pick up a new binary after switching channels.
  if [ -n "$switching" ]; then
    quietly as_root systemctl restart nginx
  fi
  info "nginx $nginx_version ($VERSION) is running on port 80"
else
  info "nginx $nginx_version ($VERSION) is installed"
  warn "systemd isn't running, so nginx won't start on its own; start it with: sudo nginx"
fi
