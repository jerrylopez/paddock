# php.sh: installs the PHP CLI from packages.sury.org (Ondřej Surý's Debian repository).
# Usage (via Paddock): paddock setup php <version> [--extensions a,b,c]
#
# <version> is major.minor, e.g. 8.3; the repository provides the latest patch.
# --extensions adds to DEFAULT_EXTENSIONS. Names are as `php -m` shows them
# (pdo_mysql, redis, xdebug, ...); extensions built into PHP are accepted too.

DEFAULT_EXTENSIONS=bcmath,curl,intl,mbstring,xml,zip
SURY_KEYRING=/usr/share/keyrings/debsuryorg-archive-keyring.gpg

option extensions ""
parse_args "$@"

[[ $VERSION =~ ^[0-9]+\.[0-9]+$ ]] ||
  usage_error "version must be major.minor, e.g. 8.3 (got '$VERSION')"

# Defaults plus --extensions: lowercase, dashes as underscores, no duplicates.
requested=()
IFS=, read -ra names <<<"$DEFAULT_EXTENSIONS,$extensions"
for name in "${names[@]}"; do
  name=${name//[[:space:]]/}
  name=${name,,}
  name=${name//-/_}
  [ -n "$name" ] || continue
  [[ $name =~ ^[a-z0-9_]+$ ]] || usage_error "invalid extension name '$name'"
  [[ " ${requested[*]} " == *" $name "* ]] || requested+=("$name")
done

# Read in a subshell: os-release defines its own VERSION.
os=$(. /etc/os-release && echo "${ID:-unknown}")
codename=$(. /etc/os-release && echo "${VERSION_CODENAME:-}")
[ "$os" = debian ] || die "only Debian is supported (this system is '$os')"

php=php$VERSION

add_sury_repository() {
  if ! package_installed debsuryorg-archive-keyring; then
    apt_install ca-certificates curl
    local deb
    deb=$(mktemp --suffix=.deb)
    quietly curl -fsSL -o "$deb" https://packages.sury.org/debsuryorg-archive-keyring.deb
    quietly as_root dpkg -i "$deb"
    rm -f "$deb"
  fi
  apt_source php "deb [signed-by=$SURY_KEYRING] https://packages.sury.org/php/ $codename main"
}

# Extensions PHP has loaded, named like `requested`.
loaded_extensions() {
  "$php" -m | sed -e '/^\[/d' -e '/^$/d' -e 's/^Zend //' | tr '[:upper:] ' '[:lower:]_' | sort -u
}

available_versions() {
  apt-cache pkgnames php | sed -nE 's/^php([0-9]+\.[0-9]+)-cli$/\1/p' | sort -V | paste -sd ' '
}

add_sury_repository

missing=()
if package_installed "$php-cli"; then
  loaded=$(loaded_extensions)
  for name in "${requested[@]}"; do
    grep -qx "$name" <<<"$loaded" || missing+=("$name")
  done
else
  missing=("${requested[@]}")
fi

if ! package_installed "$php-cli" || [ ${#missing[@]} -gt 0 ]; then
  apt_update
  apt-cache show "$php-cli" >/dev/null 2>&1 ||
    usage_error "PHP $VERSION is not available (available: $(available_versions))"

  packages=("$php-cli")
  for name in "${missing[@]}"; do
    package=$php-${name//_/-}
    apt-cache show "$package" >/dev/null 2>&1 ||
      usage_error "unknown PHP extension '$name' (no package $package)"
    packages+=("$package")
  done

  if package_installed "$php-cli"; then
    info "Installing PHP $VERSION extensions: ${missing[*]}"
  else
    info "Installing PHP $VERSION"
  fi
  apt_install "${packages[@]}"

  loaded=$(loaded_extensions)
  for name in "${missing[@]}"; do
    grep -qx "$name" <<<"$loaded" || die "extension $name is installed but PHP $VERSION doesn't load it"
  done
fi

# Make `php` run the requested version when several are installed.
if [ "$(readlink -f /etc/alternatives/php 2>/dev/null)" != "/usr/bin/$php" ]; then
  quietly as_root update-alternatives --set php "/usr/bin/$php"
fi

info "PHP $("$php" -r 'echo PHP_VERSION;') is ready (extensions: ${requested[*]})"
