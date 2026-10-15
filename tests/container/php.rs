//! `paddock setup php`.

use crate::harness;

/// `paddock setup php`'s default extensions, in the order its ready line lists them.
const DEFAULT_PHP_EXTENSIONS: &str = "amqp bcmath curl gd gmp imagick intl json mbstring msgpack mysqlnd opcache pcntl pdo posix redis shmop soap sodium sysvmsg sysvsem sysvshm xml xmlrpc zip";

#[test]
fn installs_with_default_extensions_and_reruns_cleanly() {
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
fn pins_composer_to_a_series_and_switches_it() {
    let outcome = harness::setup_twice_then_verify(
        "php",
        "8.3",
        &["--composer", "2.2"],
        "COMPOSER_ALLOW_SUPERUSER=1 composer --version --no-ansi | cut -d' ' -f1-3\n\
         paddock setup php 8.3 --composer 2.8\n\
         paddock setup php 8.3\n\
         paddock setup php 8.3 --composer 9.9 2>&1 || echo \"exit $?\"",
    );
    let first: Vec<_> = outcome.section("setup (first run)").lines().collect();
    assert_eq!(
        first[1], "paddock: php: Installing Composer 2.2",
        "\n{outcome}"
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(rerun[0].contains(" and Composer 2.2."), "\n{outcome}");
    let verify: Vec<_> = outcome.section("verify").lines().collect();
    assert!(
        verify[0].starts_with("Composer version 2.2."),
        "\n{outcome}"
    );
    assert_eq!(
        verify[1], "paddock: php: Switching Composer 2.2 to 2.8",
        "\n{outcome}"
    );
    // Without --composer, the pinned 2.8 stays.
    assert!(
        verify[2].contains(" and Composer 2.8.") && verify[3].contains(" and Composer 2.8."),
        "\n{outcome}"
    );
    // An unknown series fails before any progress line.
    assert!(
        verify[4].starts_with("paddock: php: Composer 9.9 isn't available (available: 1.0 ")
            && verify[4].contains(" 2.8 "),
        "\n{outcome}"
    );
    assert_eq!(verify[5..], ["exit 2"], "\n{outcome}");
}

#[test]
fn adds_extensions_and_switches_versions() {
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
fn installs_as_a_sudo_user_like_an_orb() {
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
fn rejects_unavailable_versions_and_unknown_extensions() {
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
