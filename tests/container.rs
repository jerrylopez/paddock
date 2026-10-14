//! Container tests: run the real binary in a fresh Linux container.
//! Not built by plain `cargo test`; run with `cargo container-tests`.

mod harness;

/// `paddock setup php`'s default extensions, in the order its ready line lists them.
const DEFAULT_PHP_EXTENSIONS: &str = "amqp bcmath curl gd gmp imagick intl json mbstring msgpack mysqlnd opcache pcntl pdo posix redis shmop soap sodium sysvmsg sysvsem sysvshm xml xmlrpc zip";

#[test]
fn paddock_runs_in_a_fresh_container() {
    let outcome = harness::run("paddock --version");
    outcome.assert_success();
    assert_eq!(
        outcome.stdout.trim(),
        concat!("paddock ", env!("CARGO_PKG_VERSION")),
        "\n{outcome}"
    );
}

#[test]
fn apt_install_installs_missing_packages_then_skips_them() {
    let outcome = harness::run_service_script(
        r#"
apt_install jq
jq --version
# A second call must not touch apt: shadow apt-get with one that fails.
mkdir -p /tmp/fake-bin
printf '#!/bin/sh\necho "apt-get should not run" >&2\nexit 99\n' >/tmp/fake-bin/apt-get
chmod +x /tmp/fake-bin/apt-get
export PATH="/tmp/fake-bin:$PATH"
apt_install jq
echo second call skipped apt
"#,
        &["1.0"],
        &[],
    );
    outcome.assert_success();
    assert!(outcome.stdout.starts_with("jq-"), "\n{outcome}");
    assert!(
        outcome.stdout.ends_with("\nsecond call skipped apt\n"),
        "\n{outcome}"
    );
    assert_eq!(outcome.stderr, "", "apt output should be hidden\n{outcome}");
}

#[test]
fn apt_install_without_root_fails_clearly() {
    let outcome = harness::run_service_script("apt_install jq", &["1.0"], &["--user", "1000:1000"]);
    assert_eq!(outcome.code, Some(1), "\n{outcome}");
    assert_eq!(
        outcome.stderr,
        "paddock: demo: needs root: run as root or install sudo\n"
    );
}

#[test]
fn apt_source_changes_trigger_one_more_update() {
    // Count apt-get calls by putting a logging wrapper in front of the real one.
    let outcome = harness::run_service_script(
        r#"
mkdir -p /tmp/count-bin
printf '#!/bin/sh\necho "$1" >>/tmp/apt-calls\nexec /usr/bin/apt-get "$@"\n' >/tmp/count-bin/apt-get
chmod +x /tmp/count-bin/apt-get
export PATH="/tmp/count-bin:$PATH"
line="deb [signed-by=/usr/share/keyrings/debian-archive-keyring.gpg] http://deb.debian.org/debian bookworm-backports main"
apt_install jq                # update, install
apt_source extra "$line"      # new source: lists are stale
apt_install file              # update, install
apt_source extra "$line"      # unchanged: lists still fresh
apt_install less              # install only
paste -sd ' ' /tmp/apt-calls
"#,
        &["1.0"],
        &[],
    );
    outcome.assert_success();
    assert_eq!(
        outcome.stdout, "update install update install install\n",
        "\n{outcome}"
    );
}

#[test]
fn php_installs_with_default_extensions_and_reruns_cleanly() {
    let outcome = harness::setup_twice_then_verify(
        "php",
        "8.3",
        &[],
        "php -v | head -n1\nphp -m\nCOMPOSER_ALLOW_SUPERUSER=1 composer --version --no-ansi",
    );
    let first: Vec<_> = outcome.section("setup (first run)").lines().collect();
    assert_eq!(
        first[..2],
        [
            "paddock: php: Installing PHP 8.3",
            "paddock: php: Installing Composer"
        ],
        "\n{outcome}"
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(
        rerun[0].starts_with("paddock: php: PHP 8.3.")
            && rerun[0].contains(" and Composer 2.")
            && rerun[0].ends_with(&format!(
                " are ready (extensions: {DEFAULT_PHP_EXTENSIONS})"
            )),
        "\n{outcome}"
    );
    let verify = outcome.section("verify");
    assert!(verify.starts_with("PHP 8.3."), "\n{outcome}");
    // `php -m` spells a few extensions differently.
    for extension in DEFAULT_PHP_EXTENSIONS.split(' ').map(|name| match name {
        "opcache" => "Zend OPcache",
        "pdo" => "PDO",
        other => other,
    }) {
        assert!(
            verify.lines().any(|line| line == extension),
            "{extension} not loaded\n{outcome}"
        );
    }
    assert!(verify.contains("Composer version 2."), "\n{outcome}");
}

#[test]
fn php_adds_extensions_and_switches_versions() {
    let outcome = harness::run(
        "paddock setup php 8.4 --extensions redis,pdo_mysql\n\
         php8.4 -m | grep -x redis\n\
         php8.4 -m | grep -x pdo_mysql\n\
         paddock setup php 8.2\n\
         php -r 'echo PHP_MAJOR_VERSION, \".\", PHP_MINOR_VERSION, \"\\n\";'\n\
         paddock setup php 8.4\n\
         php -r 'echo PHP_MAJOR_VERSION, \".\", PHP_MINOR_VERSION, \"\\n\";'",
    );
    outcome.assert_success();
    let versions: Vec<_> = outcome
        .stdout
        .lines()
        .filter(|line| !line.starts_with("paddock:"))
        .collect();
    assert_eq!(
        versions,
        ["redis", "pdo_mysql", "8.2", "8.4"],
        "\n{outcome}"
    );
}

#[test]
fn php_installs_as_a_sudo_user_like_an_orb() {
    let outcome = harness::run(
        "apt-get update -qq && apt-get install -y -qq sudo >/dev/null\n\
         useradd --create-home dev\n\
         echo 'dev ALL=(ALL) NOPASSWD:ALL' >/etc/sudoers.d/dev\n\
         su dev -c 'paddock setup php 8.3 && composer --version --no-ansi | head -n1'",
    );
    outcome.assert_success();
    let lines: Vec<_> = outcome.stdout.lines().collect();
    assert!(
        lines[lines.len() - 2].ends_with(&format!(
            " are ready (extensions: {DEFAULT_PHP_EXTENSIONS})"
        )),
        "\n{outcome}"
    );
    assert!(
        lines[lines.len() - 1].starts_with("Composer version 2."),
        "\n{outcome}"
    );
}

#[test]
fn php_rejects_unavailable_versions_and_unknown_extensions() {
    let version = harness::run("paddock setup php 9.9");
    assert_eq!(version.code, Some(2), "\n{version}");
    assert!(
        version
            .stderr
            .starts_with("paddock: php: PHP 9.9 is not available (available: "),
        "\n{version}"
    );
    assert!(version.stderr.contains(" 8.3 "), "\n{version}");

    let extension = harness::run("paddock setup php 8.3 --extensions nope");
    assert_eq!(extension.code, Some(2), "\n{extension}");
    assert_eq!(
        extension.stderr,
        "paddock: php: unknown PHP extension 'nope' for PHP 8.3 (no package php8.3-nope)\n"
    );
}

/// Starts MySQL without systemd and waits until it accepts connections.
const START_MYSQL: &str = "/usr/sbin/mysqld --daemonize --user=mysql >/dev/null\n\
    for i in $(seq 30); do mysqladmin --no-defaults ping >/dev/null 2>&1 && break; sleep 1; done\n";

#[test]
fn mysql_creates_database_and_user_and_reruns_cleanly() {
    // Quotes, backslashes and `#` exercise the SQL and option-file escaping.
    let password = r#"it's "app" \ #1"#;
    let root_password = r#"r00t's "pw" \"#;
    let verify = format!(
        "{START_MYSQL}\
         MYSQL_PWD='{}' mysql -uapp -h127.0.0.1 app -Nse 'select concat(current_user(), \" \", database())'\n\
         MYSQL_PWD='{}' mysql -uroot -Nse 'select \"root ok\"'\n\
         /usr/sbin/mysqld --version",
        password.replace('\'', r"'\''"),
        root_password.replace('\'', r"'\''"),
    );
    let outcome = harness::setup_twice_then_verify(
        "mysql",
        "8.4",
        &[
            "--database",
            "app",
            "--username",
            "app",
            "--password",
            password,
            "--root-password",
            root_password,
        ],
        &verify,
    );
    let first: Vec<_> = outcome.section("setup (first run)").lines().collect();
    assert_eq!(
        first[0], "paddock: mysql: Installing MySQL 8.4",
        "\n{outcome}"
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(
        rerun[0].starts_with("paddock: mysql: MySQL 8.4.")
            && rerun[0].ends_with(" is installed (database: app, user: app)"),
        "\n{outcome}"
    );
    let verify: Vec<_> = outcome.section("verify").lines().collect();
    assert_eq!(verify[..2], ["app@% app", "root ok"], "\n{outcome}");
    assert!(verify[2].contains(" Ver 8.4."), "\n{outcome}");
    assert!(
        outcome
            .stderr
            .contains("paddock: mysql: warning: systemd isn't running"),
        "\n{outcome}"
    );
}

#[test]
fn mysql_8_0_installs_and_refuses_to_switch_series() {
    let outcome = harness::run(
        "paddock setup mysql 8.0\n\
         /usr/sbin/mysqld --version\n\
         paddock setup mysql 8.4 || echo \"exit=$?\"",
    );
    outcome.assert_success();
    let lines: Vec<_> = outcome.stdout.lines().collect();
    assert!(
        lines.iter().any(|line| line.contains(" Ver 8.0.")),
        "\n{outcome}"
    );
    assert_eq!(lines.last(), Some(&"exit=1"), "\n{outcome}");
    assert!(
        outcome.stderr.contains(
            "paddock: mysql: MySQL from mysql-8.0 is already installed; switching to 8.4 isn't supported"
        ),
        "\n{outcome}"
    );
}
