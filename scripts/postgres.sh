# postgres.sh: installs PostgreSQL from the PostgreSQL project's APT repository (PGDG).
# Usage (via Paddock): paddock setup postgres <version> [flags...]
#
# <version> is an exact PostgreSQL release such as 17.6. The repository and
# its archive (apt-archive.postgresql.org) keep every release, and an APT pin
# installs exactly that one. Upgrading to a newer minor is supported; going
# back isn't. Several majors can be installed side by side: each has its own
# `main` cluster, and Debian gives each a port (5432 for the first).
#
#   --database NAME       create a database
#   --username NAME       create a login role (needs --password); with
#                         --database it owns that database
#   --password PASSWORD   the role's password
#
# The `postgres` superuser logs in through the Unix socket (`sudo -u postgres
# psql`). Under systemd the cluster runs as postgresql@<major>-main and starts
# after every boot or orb wake; without systemd it's started only to apply the
# flags.

PGDG_KEY_URL=https://www.postgresql.org/media/keys/ACCC4CF8.asc
PGDG_KEY_FINGERPRINT=B97B0AFCAA1A47F044F244A07FCC7D46ACCC4CF8

option database ""
option username ""
option password ""
parse_args "$@"

[[ $VERSION =~ ^([0-9]+)\.[0-9]+$ ]] ||
  usage_error "version must be a PostgreSQL release like 17.6 (got '$VERSION')"
major=${BASH_REMATCH[1]}
[ -z "$database" ] || [[ $database =~ ^[a-z_][a-z0-9_]*$ ]] ||
  usage_error "database name may only use lowercase letters, digits and _ (got '$database')"
[ -z "$username" ] || [[ $username =~ ^[a-z_][a-z0-9_]*$ ]] ||
  usage_error "username may only use lowercase letters, digits and _ (got '$username')"
[ -z "$username" ] || [ -n "$password" ] || usage_error "--username needs --password"
[ -z "$password" ] || [ -n "$username" ] || usage_error "--password needs --username"

# Read in a subshell: os-release defines its own VERSION.
os=$(. /etc/os-release && echo "${ID:-unknown}")
codename=$(. /etc/os-release && echo "${VERSION_CODENAME:-}")
[ "$os" = debian ] || die "only Debian is supported (this system is '$os')"
case $(dpkg --print-architecture) in
  amd64 | arm64 | ppc64el) ;;
  *) die "the PostgreSQL repository has no packages for $(dpkg --print-architecture)" ;;
esac

installed=$(installed_version "postgresql-$major")
if [ -n "$installed" ] && [ "$installed" != "$VERSION" ] &&
  dpkg --compare-versions "$VERSION" lt "$installed"; then
  die "PostgreSQL $installed is installed; going back to $VERSION isn't supported"
fi

apt_keyring postgresql "$PGDG_KEY_URL" "$PGDG_KEY_FINGERPRINT"
# The main repository has the latest minors; the archive has every release.
apt_source postgresql "deb [signed-by=/etc/apt/keyrings/postgresql.gpg] https://apt.postgresql.org/pub/repos/apt $codename-pgdg main
deb [signed-by=/etc/apt/keyrings/postgresql.gpg] https://apt-archive.postgresql.org/pub/repos/apt $codename-pgdg-archive main"
apt_pin "postgresql-$major" "$VERSION-*" "postgresql-$major" "postgresql-client-$major"

upgraded=
if [ "$installed" != "$VERSION" ]; then
  apt_update
  require_published "postgresql-$major" PostgreSQL
  if [ -n "$installed" ]; then
    info "Upgrading PostgreSQL $installed to $VERSION"
    quietly as_root env DEBIAN_FRONTEND=noninteractive \
      apt-get -o Acquire::Retries=3 -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold install -y --no-install-recommends "postgresql-$major" "postgresql-client-$major"
    upgraded=1
  else
    info "Installing PostgreSQL $VERSION"
    apt_install "postgresql-$major"
  fi
fi

# Installing the package creates the `main` cluster; recreate it if it's gone.
if [ ! -d "/etc/postgresql/$major/main" ]; then
  quietly as_root pg_createcluster "$major" main
fi
port=$(as_root pg_conftool -s "$major" main show port)

running() {
  as_root pg_isready -q -p "$port" >/dev/null 2>&1
}

wait_until_ready() {
  local tries
  for tries in $(seq 30); do
    running && return 0
    sleep 1
  done
  die "PostgreSQL $major didn't start; see /var/log/postgresql/postgresql-$major-main.log"
}

temporary=
if has_systemd; then
  was_running=$(systemctl is-active "postgresql@$major-main" 2>/dev/null) || true
  quietly as_root systemctl enable --now "postgresql@$major-main"
  # Pick up the new binaries after an upgrade.
  if [ "$was_running" = active ] && [ -n "$upgraded" ]; then
    quietly as_root systemctl restart "postgresql@$major-main"
  fi
  wait_until_ready
elif [ -n "$database$username" ] && ! running; then
  # No systemd: start the cluster just long enough to apply the flags.
  quietly as_root pg_ctlcluster "$major" main start
  temporary=1
  wait_until_ready
fi

# Escape a value for a single-quoted SQL string.
sql_escape() {
  printf '%s' "${1//\'/\'\'}"
}

if [ -n "$database$username" ]; then
  sql="\\set ON_ERROR_STOP on"
  if [ -n "$username" ]; then
    sql+="
SELECT format('CREATE ROLE %I LOGIN', '$username')
  WHERE NOT EXISTS (SELECT FROM pg_roles WHERE rolname = '$username') \\gexec
ALTER ROLE \"$username\" WITH LOGIN PASSWORD '$(sql_escape "$password")';"
  fi
  if [ -n "$database" ]; then
    sql+="
SELECT format('CREATE DATABASE %I', '$database')
  WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname = '$database') \\gexec"
    if [ -n "$username" ]; then
      sql+="
ALTER DATABASE \"$database\" OWNER TO \"$username\";"
    fi
  fi
  # The SQL (with the password) goes through stdin, never a command line.
  if ! output=$(as_root runuser -u postgres -- psql --quiet --no-psqlrc -p "$port" -d postgres <<<"$sql" 2>&1); then
    [ -z "$temporary" ] || as_root pg_ctlcluster "$major" main stop >/dev/null 2>&1 || true
    printf '%s\n' "$output" >&2
    die "couldn't apply the database and user settings"
  fi
fi

if [ -n "$temporary" ]; then
  quietly as_root pg_ctlcluster "$major" main stop
fi

details=()
[ -z "$database" ] || details+=("database: $database")
[ -z "$username" ] || details+=("user: $username")
summary=
if [ ${#details[@]} -gt 0 ]; then
  summary=" ($(IFS=,; echo "${details[*]}" | sed 's/,/, /g'))"
fi
pg_version=$("/usr/lib/postgresql/$major/bin/postgres" --version | awk '{ print $3 }')

if has_systemd; then
  info "PostgreSQL $pg_version is running on port $port$summary"
else
  info "PostgreSQL $pg_version is installed on port $port$summary"
  warn "systemd isn't running, so PostgreSQL won't start on its own; start it with: sudo pg_ctlcluster $major main start"
fi
