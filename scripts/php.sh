# php.sh: installs the PHP CLI from packages.sury.org (Ondřej Surý's Debian repository),
# plus Composer from getcomposer.org. `paddock setup php-fpm` runs this script too and
# also installs PHP-FPM, enabled under systemd.
# Usage (via Paddock): paddock setup php|php-fpm <version> [--extensions a,b,c]
#
# <version> is major.minor, e.g. 8.3; the repository provides the latest patch.
# --extensions adds to DEFAULT_EXTENSIONS. Names are as `php -m` shows them
# (pdo_mysql, xdebug, ...); extensions built into PHP are accepted too.
# Defaults that aren't packaged for the requested version are skipped with a
# warning; extensions asked for with --extensions must be available.

# pcntl, posix, shmop and sysv* are what Red Hat's php-process package provides.
DEFAULT_EXTENSIONS=amqp,bcmath,curl,gd,gmp,imagick,intl,json,mbstring,msgpack,mysqlnd,opcache,pcntl,pdo,posix,redis,shmop,soap,sodium,sysvmsg,sysvsem,sysvshm,xml,xmlrpc,zip
SURY_KEYRING=/usr/share/keyrings/debsuryorg-archive-keyring.gpg

option extensions ""
parse_args "$@"

[[ $VERSION =~ ^[0-9]+\.[0-9]+$ ]] ||
  usage_error "version must be major.minor, e.g. 8.3 (got '$VERSION')"

# add_extensions ARRAY LIST: append the names in comma-separated LIST to ARRAY,
# lowercased, with dashes as underscores, skipping duplicates.
add_extensions() {
  local -n into=$1
  local names name
  IFS=, read -ra names <<<"$2"
  for name in "${names[@]}"; do
    name=${name//[[:space:]]/}
    name=${name,,}
    name=${name//-/_}
    [ -n "$name" ] || continue
    [[ $name =~ ^[a-z0-9_]+$ ]] || usage_error "invalid extension name '$name'"
    contains "$name" "${into[@]}" || into+=("$name")
  done
}

contains() {
  local item
  for item in "${@:2}"; do
    [ "$item" = "$1" ] && return 0
  done
  return 1
}

explicit=()
add_extensions explicit "$extensions"
requested=()
add_extensions requested "$DEFAULT_EXTENSIONS,$extensions"

# Read in a subshell: os-release defines its own VERSION.
os=$(. /etc/os-release && echo "${ID:-unknown}")
codename=$(. /etc/os-release && echo "${VERSION_CODENAME:-}")
[ "$os" = debian ] || die "only Debian is supported (this system is '$os')"

php=php$VERSION
fpm=
[ "$SERVICE" != php-fpm ] || fpm=$php-fpm

add_sury_repository() {
  if ! package_installed debsuryorg-archive-keyring; then
    apt_install ca-certificates curl
    local deb
    deb=$(mktemp --suffix=.deb)
    download https://packages.sury.org/debsuryorg-archive-keyring.deb "$deb"
    quietly as_root dpkg -i "$deb"
    rm -f "$deb"
  fi
  apt_source php "deb [signed-by=$SURY_KEYRING] https://packages.sury.org/php/ $codename main"
}

# Extensions a PHP binary loads, named like `requested`.
extensions_of() {
  "$1" -m | sed -e '/^\[/d' -e '/^$/d' -e 's/^Zend //' | tr '[:upper:] ' '[:lower:]_' | sort -u
}

loaded_extensions() {
  extensions_of "$php"
}

is_loaded() {
  grep -qx "$1" <<<"$loaded"
}

available() {
  apt-cache show "$1" >/dev/null 2>&1
}

# Composer is a single PHAR checked against getcomposer.org's published SHA-256.
install_composer() {
  # Composer 2.3+ needs PHP 7.2.5; older PHP gets the 2.2 LTS line.
  local channel=latest-2.x
  if [ "$("$php" -r 'echo PHP_VERSION_ID;')" -lt 70205 ]; then
    channel=latest-2.2.x
  fi
  local url=https://getcomposer.org/download/$channel/composer.phar dir
  info "Installing Composer"
  apt_install ca-certificates curl
  dir=$(mktemp -d)
  download "$url" "$dir/composer.phar"
  download "$url.sha256sum" "$dir/composer.phar.sha256sum"
  if ! (cd "$dir" && sha256sum --check --quiet composer.phar.sha256sum >/dev/null 2>&1); then
    rm -rf "$dir"
    die "the Composer download from $url doesn't match its checksum"
  fi
  quietly as_root install -m 0755 "$dir/composer.phar" /usr/local/bin/composer
  rm -rf "$dir"
}

available_versions() {
  apt-cache pkgnames php | sed -nE 's/^php([0-9]+\.[0-9]+)-cli$/\1/p' | sort -V | paste -sd ' '
}

add_sury_repository

fresh=
if ! package_installed "$php-cli"; then
  fresh=1
  apt_update
  available "$php-cli" ||
    usage_error "PHP $VERSION is not available (available: $(available_versions))"
  info "Installing PHP $VERSION"
  apt_install "$php-cli"
fi

# FPM goes in before extensions, so extension packages enable themselves for it.
if [ -n "$fpm" ] && ! package_installed "$fpm"; then
  info "Installing PHP-FPM $VERSION"
  apt_install "$fpm"
fi

# Extensions built into PHP are loaded already; the rest come from packages.
loaded=$(loaded_extensions)
missing=()
for name in "${requested[@]}"; do
  is_loaded "$name" || missing+=("$name")
done

if [ ${#missing[@]} -gt 0 ]; then
  # Package lists may have been cleaned since PHP was installed.
  available "$php-cli" || apt_update

  packages=() installing=() skipped=()
  for name in "${missing[@]}"; do
    package=$php-${name//_/-}
    if ! available "$package"; then
      if ! contains "$name" "${explicit[@]}"; then
        skipped+=("$name")
        continue
      fi
      # Asked for explicitly: make sure the lists are current before giving up.
      apt_update
      available "$package" ||
        usage_error "unknown PHP extension '$name' for PHP $VERSION (no package $package)"
    fi
    packages+=("$package")
    installing+=("$name")
  done

  [ ${#skipped[@]} -eq 0 ] ||
    warn "not packaged for PHP $VERSION, skipped: ${skipped[*]}"

  if [ ${#packages[@]} -gt 0 ]; then
    [ -n "$fresh" ] || info "Installing PHP $VERSION extensions: ${installing[*]}"
    apt_install "${packages[@]}"
    new_extensions=1
    loaded=$(loaded_extensions)
    for name in "${installing[@]}"; do
      is_loaded "$name" || die "extension $name is installed but PHP $VERSION doesn't load it"
    done
  fi
fi

# Make `php` run the requested version when several are installed.
if [ "$(readlink -f /etc/alternatives/php 2>/dev/null)" != "/usr/bin/$php" ]; then
  quietly as_root update-alternatives --set php "/usr/bin/$php"
fi

# The installed Composer's version; fails if it's missing or can't run on this PHP.
composer_version() {
  local output
  output=$(COMPOSER_ALLOW_SUPERUSER=1 composer --version --no-ansi 2>/dev/null) || return 1
  [[ $output =~ ^Composer\ version\ ([^ ]+) ]] || return 1
  echo "${BASH_REMATCH[1]}"
}

# Reinstall when Composer is missing or can't run on this PHP (e.g. after
# switching from 8.3 to 7.1, which needs the 2.2 LTS line).
composer=$(composer_version) || true
if [ -z "$composer" ]; then
  install_composer
  composer=$(composer_version) || die "Composer was installed but doesn't run on PHP $VERSION"
fi

ready=()
for name in "${requested[@]}"; do
  if is_loaded "$name"; then
    ready+=("$name")
  fi
done
# Drop Debian's build suffix, e.g. 7.1.33-79+0~20260703 -> 7.1.33.
php_version=$("$php" -r 'echo PHP_VERSION;')
php_version=${php_version%%-*}

if [ -z "$fpm" ]; then
  info "PHP $php_version and Composer $composer are ready (extensions: ${ready[*]})"
  exit 0
fi

fpm_binary=/usr/sbin/php-fpm$VERSION
fpm_socket=/run/php/php$VERSION-fpm.sock
fpm_extensions=$(extensions_of "$fpm_binary")
for name in "${ready[@]}"; do
  # Debian builds these for the CLI only; they make no sense in a web server.
  case $name in pcntl | readline) continue ;; esac
  grep -qx "$name" <<<"$fpm_extensions" ||
    die "extension $name loads in the PHP $VERSION CLI but not in PHP-FPM"
done
quietly as_root "$fpm_binary" --test

if has_systemd; then
  quietly as_root systemctl enable --now "$fpm"
  # A running FPM only picks up new extensions when it restarts.
  if [ -n "${new_extensions:-}" ]; then
    quietly as_root systemctl restart "$fpm"
  fi
  info "PHP $php_version, PHP-FPM on $fpm_socket and Composer $composer are ready (extensions: ${ready[*]})"
else
  info "PHP $php_version, PHP-FPM and Composer $composer are installed (extensions: ${ready[*]})"
  warn "systemd isn't running, so PHP-FPM won't start on its own; start it with: sudo mkdir -p /run/php && sudo $fpm_binary --daemonize"
fi
