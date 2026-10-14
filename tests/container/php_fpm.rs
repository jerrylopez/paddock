//! `paddock setup php-fpm`.

use crate::harness;

/// Sends a FastCGI request for /var/www/info.php to PHP 8.3's FPM socket.
const REQUEST: &str = "SCRIPT_FILENAME=/var/www/info.php REQUEST_METHOD=GET \
    cgi-fcgi -bind -connect /run/php/php8.3-fpm.sock | tail -n1";

#[test]
fn serves_requests_under_systemd_and_survives_a_restart() {
    let container = harness::SystemdContainer::start();
    // FPM added after the CLI must still get the CLI's extensions.
    container.run("paddock setup php 8.3").assert_success();
    let outcome = container.setup_twice_then_verify(
        "php-fpm",
        "8.3",
        &[],
        &format!(
            "systemctl is-active php8.3-fpm\n\
             apt-get install -y -qq libfcgi-bin >/dev/null 2>&1\n\
             mkdir -p /var/www\n\
             echo '<?php echo PHP_SAPI, \" \", PHP_MAJOR_VERSION, \".\", PHP_MINOR_VERSION, \" \", extension_loaded(\"redis\") ? \"redis\" : \"no redis\", PHP_EOL;' >/var/www/info.php\n\
             {REQUEST}"
        ),
    );
    assert_eq!(
        outcome.section("setup (first run)").lines().next(),
        Some("paddock: php-fpm: Installing PHP-FPM 8.3"),
        "\n{outcome}"
    );
    let rerun: Vec<_> = outcome.section("setup (second run)").lines().collect();
    assert_eq!(rerun.len(), 1, "rerun should print one line\n{outcome}");
    assert!(
        rerun[0].starts_with("paddock: php-fpm: PHP 8.3.")
            && rerun[0].contains(", PHP-FPM on /run/php/php8.3-fpm.sock and Composer 2."),
        "\n{outcome}"
    );
    assert_eq!(
        outcome.section("verify"),
        "active\nfpm-fcgi 8.3 redis\n",
        "\n{outcome}"
    );

    container.restart();
    let after = container.run(&format!("systemctl is-active php8.3-fpm\n{REQUEST}"));
    after.assert_success();
    assert_eq!(after.stdout, "active\nfpm-fcgi 8.3 redis\n", "\n{after}");
}

#[test]
fn installs_without_systemd_and_says_so() {
    let outcome =
        harness::run("paddock setup php-fpm 8.3\n/usr/sbin/php-fpm8.3 --test 2>&1 | tail -n1");
    outcome.assert_success();
    assert!(
        outcome.stdout.contains(", PHP-FPM and Composer 2.")
            && outcome.stdout.ends_with("test is successful\n"),
        "\n{outcome}"
    );
    assert!(
        outcome.stderr.contains(
            "paddock: php-fpm: warning: systemd isn't running, so PHP-FPM won't start on its own"
        ),
        "\n{outcome}"
    );
}
