# mysql.sh: installs MySQL Community Server from Oracle's APT repository.
# Usage (via Paddock): paddock setup mysql <version> [flags...]
#
# <version> picks a release series: 8.0, 8.4, 9.7 or innovation. The repository
# provides the latest release in it.
#
#   --database NAME       create a database
#   --username NAME       create a user (needs --password); with --database it
#                         gets full access to that database
#   --password PASSWORD   the user's password
#   --root-password PW    give root a password; otherwise root logs in through
#                         the Unix socket, as with `sudo mysql`
#
# With systemd (orbs, servers) MySQL is enabled and started, and systemd starts
# it again after every boot or orb wake. Without systemd (plain containers) a
# temporary server applies the flags and is stopped again.

MYSQL_KEY_URL=https://repo.mysql.com/RPM-GPG-KEY-mysql-2025
MYSQL_KEY_FINGERPRINT=BCA43417C3B485DD128EC6D4B7B3B788A8D3785C
MYSQL_SOURCE=/etc/apt/sources.list.d/mysql.list
MYSQL_SOCKET=/var/run/mysqld/mysqld.sock

option database ""
option username ""
option password ""
option root-password ""
parse_args "$@"

case $VERSION in
  8.0) suite=mysql-8.0 ;;
  8.4) suite=mysql-8.4-lts ;;
  9.7) suite=mysql-9.7-lts ;;
  innovation) suite=mysql-innovation ;;
  *) require_version 8.0 8.4 9.7 innovation ;;
esac

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
arch=$(dpkg --print-architecture)
case $arch in
  amd64 | i386) ;;
  *) die "Oracle's MySQL repository has no packages for $arch" ;;
esac

add_mysql_repository() {
  apt_keyring mysql "$MYSQL_KEY_URL" "$MYSQL_KEY_FINGERPRINT"
  # MySQL can't move an existing data directory to another series (e.g. 8.4 -> 8.0).
  if package_installed mysql-community-server && [ -f "$MYSQL_SOURCE" ]; then
    local installed
    installed=$(awk '{ print $NF }' "$MYSQL_SOURCE")
    [ "$installed" = "$suite" ] ||
      die "MySQL from $installed is already installed; switching to $VERSION isn't supported (remove mysql-community-server and /var/lib/mysql first)"
  fi
  apt_source mysql "deb [signed-by=/etc/apt/keyrings/mysql.gpg] https://repo.mysql.com/apt/debian $codename $suite"
}

# `ping` succeeds whenever the server is up, even if login would be refused.
server_running() {
  as_root mysqladmin --no-defaults --socket="$MYSQL_SOCKET" ping >/dev/null 2>&1
}

wait_for_server() {
  local tries
  for tries in $(seq 30); do
    server_running && return 0
    sleep 1
  done
  die "MySQL didn't start; see /var/log/mysql/error.log"
}

# as_mysql_root CLIENT [ARGS...]: run a MySQL client (mysql, mysqladmin) as
# root over the socket. The root password, if any, goes through a private
# option file so it never appears in a command line or an error message.
as_mysql_root() {
  local client=$1 config status=0
  shift
  config=$(mktemp)
  chmod 600 "$config"
  printf '[client]\nuser=root\n' >"$config"
  if [ -n "$root_password" ]; then
    printf 'password="%s"\n' "$(option_escape "$root_password")" >>"$config"
  fi
  as_root "$client" --defaults-extra-file="$config" --socket="$MYSQL_SOCKET" "$@" || status=$?
  rm -f "$config"
  return "$status"
}

# Escape a value for a single-quoted SQL string.
sql_escape() {
  local value=${1//\\/\\\\}
  printf '%s' "${value//\'/\\\'}"
}

# Escape a value for a double-quoted option-file value.
option_escape() {
  local value=${1//\\/\\\\}
  printf '%s' "${value//\"/\\\"}"
}

stop_temporary_server() {
  [ -z "$temporary" ] || as_mysql_root mysqladmin shutdown >/dev/null 2>&1 || true
}

add_mysql_repository

if ! package_installed mysql-community-server; then
  info "Installing MySQL $VERSION"
  apt_install mysql-community-server
fi

temporary=
if has_systemd; then
  quietly as_root systemctl enable --now mysql
elif [ -n "$database$username$root_password" ] && ! server_running; then
  # No systemd: start a server just long enough to apply the flags.
  quietly as_root /usr/sbin/mysqld --daemonize --user=mysql
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
    sql+="ALTER USER 'root'@'localhost' IDENTIFIED WITH caching_sha2_password BY '$(sql_escape "$root_password")';"
  fi
  if ! output=$(as_mysql_root mysql <<<"$sql" 2>&1); then
    stop_temporary_server
    printf '%s\n' "$output" >&2
    die "couldn't apply the database, user or root password settings (if root already has a password, pass it with --root-password)"
  fi
fi

# Waits until the server has exited, so a rerun can start a new one.
stop_temporary_server

details=()
[ -z "$database" ] || details+=("database: $database")
[ -z "$username" ] || details+=("user: $username")
summary=
if [ ${#details[@]} -gt 0 ]; then
  summary=" ($(IFS=,; echo "${details[*]}" | sed 's/,/, /g'))"
fi
mysql_version=$(/usr/sbin/mysqld --version | awk '{ print $3 }')

if has_systemd; then
  info "MySQL $mysql_version is running on /var/run/mysqld/mysqld.sock$summary"
else
  info "MySQL $mysql_version is installed$summary"
  warn "systemd isn't running, so MySQL won't start on its own; start it with: sudo mysqld --daemonize --user=mysql"
fi
