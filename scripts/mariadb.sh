# mariadb.sh: installs MariaDB Server from the MariaDB Foundation's repositories.
# Usage (via Paddock): paddock setup mariadb <version> [flags...]
#
# <version> is an exact MariaDB release such as 11.4.13. Each release has its
# own repository on archive.mariadb.org, which stays available, so an install
# is always exactly that release. Upgrading within a series (11.4.12 to
# 11.4.13) is supported; going back or to another series isn't.
#
#   --database NAME       create a database
#   --username NAME       create a user (needs --password); with --database it
#                         gets full access to that database
#   --password PASSWORD   the user's password
#   --root-password PW    let root also log in with a password; root keeps
#                         logging in through the Unix socket (`sudo mariadb`)
#
# Under systemd MariaDB runs as the `mariadb` unit; without systemd a
# temporary server applies the flags and is stopped again.

MARIADB_KEY_URL=https://mariadb.org/mariadb_release_signing_key.pgp
MARIADB_KEY_FINGERPRINT=177F4010FE56CA3336300305F1656F24C74CD1D8
MARIADB_PIN=/etc/apt/preferences.d/mariadb
MARIADB_SOCKET=/run/mysqld/mysqld.sock

option database ""
option username ""
option password ""
option root-password ""
parse_args "$@"

[[ $VERSION =~ ^([0-9]+\.[0-9]+)\.[0-9]+$ ]] ||
  usage_error "version must be a MariaDB release like 11.4.13 (got '$VERSION')"
series=${BASH_REMATCH[1]}
[ -z "$database" ] || [[ $database =~ ^[A-Za-z0-9_]+$ ]] ||
  usage_error "database name may only use letters, digits and _ (got '$database')"
[ -z "$username" ] || [[ $username =~ ^[A-Za-z0-9_.-]+$ ]] ||
  usage_error "username may only use letters, digits, _, . and - (got '$username')"
[ -z "$username" ] || [ -n "$password" ] || usage_error "--username needs --password"
[ -z "$password" ] || [ -n "$username" ] || usage_error "--password needs --username"

# Read in a subshell: os-release defines its own VERSION.
os=$(. /etc/os-release && echo "${ID:-unknown}")
codename=$(. /etc/os-release && echo "${VERSION_CODENAME:-}")
[ "$os" = debian ] || die "only Debian is supported (this system is '$os')"
case $(dpkg --print-architecture) in
  amd64 | arm64 | i386 | ppc64el) ;;
  *) die "MariaDB publishes no packages for $(dpkg --print-architecture)" ;;
esac
# MySQL and MariaDB share files, the socket and the port.
for package in mysql-community-server mysql-server; do
  ! package_installed "$package" ||
    die "MySQL is installed; remove it before setting up MariaDB (they can't be installed together)"
done

installed=$(installed_version mariadb-server)
if [ -n "$installed" ] && [ "$installed" != "$VERSION" ]; then
  [ "${installed%.*}" = "$series" ] ||
    die "MariaDB $installed is installed; switching to another series isn't supported (remove mariadb-server and /var/lib/mysql first)"
  ! dpkg --compare-versions "$VERSION" lt "$installed" ||
    die "MariaDB $installed is installed; going back to $VERSION isn't supported"
fi

repository=https://archive.mariadb.org/mariadb-$VERSION/repo/debian
upgraded=
if [ "$installed" != "$VERSION" ]; then
  apt_install ca-certificates curl
  curl -fsSIL --retry 3 --retry-connrefused "$repository/dists/$codename/Release" >/dev/null 2>&1 ||
    usage_error "MariaDB $VERSION isn't published for Debian $codename (no $repository); see https://mariadb.org/download/ for releases"
fi
apt_keyring mariadb "$MARIADB_KEY_URL" "$MARIADB_KEY_FINGERPRINT"
apt_source mariadb "deb [signed-by=/etc/apt/keyrings/mariadb.gpg] $repository $codename main"
# Prefer the release's packages over Debian's own MariaDB (10.11).
write_file "$MARIADB_PIN" "Package: *
Pin: origin archive.mariadb.org
Pin-Priority: 1001"

if [ -z "$installed" ]; then
  info "Installing MariaDB $VERSION"
  apt_install mariadb-server
elif [ "$installed" != "$VERSION" ]; then
  info "Upgrading MariaDB $installed to $VERSION"
  apt_update
  quietly as_root env DEBIAN_FRONTEND=noninteractive \
    apt-get -o Acquire::Retries=3 -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold install -y --no-install-recommends mariadb-server
  upgraded=1
fi

# `ping` succeeds whenever the server is up, even if login would be refused.
server_running() {
  as_root mariadb-admin --no-defaults --socket="$MARIADB_SOCKET" ping >/dev/null 2>&1
}

wait_for_server() {
  local tries
  for tries in $(seq 30); do
    server_running && return 0
    sleep 1
  done
  die "MariaDB didn't start; see journalctl -u mariadb"
}

# Escape a value for a single-quoted SQL string.
sql_escape() {
  local value=${1//\\/\\\\}
  printf '%s' "${value//\'/\\\'}"
}

temporary=
if has_systemd; then
  was_running=$(systemctl is-active mariadb 2>/dev/null) || true
  quietly as_root systemctl enable --now mariadb
  # Pick up the new binaries after an upgrade.
  if [ "$was_running" = active ] && [ -n "$upgraded" ]; then
    quietly as_root systemctl restart mariadb
  fi
  wait_for_server
elif [ -n "$database$username$root_password" ] && ! server_running; then
  # No systemd: start a server just long enough to apply the flags.
  quietly as_root install -d -o mysql -g mysql /run/mysqld
  as_root sh -c '/usr/sbin/mariadbd --user=mysql >/dev/null 2>&1 &'
  temporary=1
  wait_for_server
fi

if [ -n "$database$username$root_password" ]; then
  sql=
  if [ -n "$database" ]; then
    sql+="CREATE DATABASE IF NOT EXISTS \`$database\`;"
  fi
  if [ -n "$username" ]; then
    pw=$(sql_escape "$password")
    sql+="CREATE USER IF NOT EXISTS '$username'@'%' IDENTIFIED BY '$pw';"
    sql+="ALTER USER '$username'@'%' IDENTIFIED BY '$pw';"
    if [ -n "$database" ]; then
      sql+="GRANT ALL PRIVILEGES ON \`$database\`.* TO '$username'@'%';"
    fi
  fi
  if [ -n "$root_password" ]; then
    sql+="ALTER USER 'root'@'localhost' IDENTIFIED VIA unix_socket OR mysql_native_password USING PASSWORD('$(sql_escape "$root_password")');"
  fi
  # Root logs in through the socket; the SQL (and any password) goes through stdin.
  if ! output=$(as_root mariadb --no-defaults --socket="$MARIADB_SOCKET" <<<"$sql" 2>&1); then
    [ -z "$temporary" ] || as_root mariadb-admin --no-defaults --socket="$MARIADB_SOCKET" shutdown >/dev/null 2>&1 || true
    printf '%s\n' "$output" >&2
    die "couldn't apply the database, user or root password settings"
  fi
fi

# Waits until the server has exited, so a rerun can start a new one.
if [ -n "$temporary" ]; then
  quietly as_root mariadb-admin --no-defaults --socket="$MARIADB_SOCKET" shutdown
fi

details=()
[ -z "$database" ] || details+=("database: $database")
[ -z "$username" ] || details+=("user: $username")
summary=
if [ ${#details[@]} -gt 0 ]; then
  summary=" ($(IFS=,; echo "${details[*]}" | sed 's/,/, /g'))"
fi

if has_systemd; then
  info "MariaDB $VERSION is running on $MARIADB_SOCKET$summary"
else
  info "MariaDB $VERSION is installed$summary"
  warn "systemd isn't running, so MariaDB won't start on its own; start it with: sudo mariadbd-safe --user=mysql &"
fi
