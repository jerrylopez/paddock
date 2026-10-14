//! Release tests: package a release with `release/package.sh` and install it with
//! the generated `install.sh`, the same way an orb's `.agents/setup` would.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A packaged release in a temporary directory, served from a `file://` URL.
struct Release {
    root: PathBuf,
}

impl Release {
    /// Packages the test build of `paddock` as both architectures' binary.
    fn package(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("paddock-install-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let release = Self { root };

        let binary = env!("CARGO_BIN_EXE_paddock");
        let output = Command::new(concat!(env!("CARGO_MANIFEST_DIR"), "/release/package.sh"))
            .args([
                VERSION,
                &release.base_url(),
                release.dist().to_str().unwrap(),
            ])
            .args([binary, binary])
            .output()
            .unwrap();
        assert_success(&output);
        release
    }

    fn dist(&self) -> PathBuf {
        self.root.join("dist")
    }

    fn base_url(&self) -> String {
        format!("file://{}", self.dist().display())
    }

    fn install_dir(&self) -> PathBuf {
        self.root.join("bin")
    }

    /// Runs the documented one-liner against this release.
    fn install(&self) -> Output {
        Command::new("bash")
            .arg("-c")
            .arg(format!("curl -fsSL {}/install.sh | bash", self.base_url()))
            .env("PADDOCK_INSTALL_DIR", self.install_dir())
            .output()
            .unwrap()
    }

    fn host_binary(&self) -> PathBuf {
        self.dist()
            .join(format!("paddock-{}-linux", std::env::consts::ARCH))
    }
}

impl Drop for Release {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[track_caller]
fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "exit {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn installed_version(dir: &Path) -> String {
    let output = Command::new(dir.join("paddock"))
        .arg("--version")
        .output()
        .unwrap();
    assert_success(&output);
    stdout(&output).trim().to_owned()
}

#[test]
fn installs_the_pinned_release() {
    let release = Release::package("pinned");
    let output = release.install();
    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            "paddock: installed {VERSION} at {}\n",
            release.install_dir().join("paddock").display()
        )
    );
    assert_eq!(
        installed_version(&release.install_dir()),
        format!("paddock {VERSION}")
    );
}

#[test]
fn reinstalling_the_same_release_does_nothing() {
    let release = Release::package("rerun");
    assert_success(&release.install());
    // The download is gone, so a second run only succeeds if it skips it.
    fs::remove_file(release.host_binary()).unwrap();

    let output = release.install();
    assert_success(&output);
    assert!(stdout(&output).contains("already installed"));
}

#[test]
fn replaces_a_different_installed_binary() {
    let release = Release::package("replace");
    fs::create_dir_all(release.install_dir()).unwrap();
    fs::write(release.install_dir().join("paddock"), "old").unwrap();

    assert_success(&release.install());
    assert_eq!(
        installed_version(&release.install_dir()),
        format!("paddock {VERSION}")
    );
}

#[test]
fn rejects_a_binary_with_the_wrong_checksum() {
    let release = Release::package("tampered");
    fs::write(release.host_binary(), "tampered").unwrap();

    let output = release.install();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("checksum mismatch"));
    assert!(!release.install_dir().join("paddock").exists());
}

#[test]
fn package_writes_checksums_for_every_file() {
    let release = Release::package("sums");
    let output = Command::new("sha256sum")
        .args(["--check", "--strict", "SHA256SUMS"])
        .current_dir(release.dist())
        .output()
        .unwrap();
    assert_success(&output);
    for file in [
        "paddock-x86_64-linux",
        "paddock-aarch64-linux",
        "install.sh",
    ] {
        assert!(
            stdout(&output).contains(&format!("{file}: OK")),
            "{file} not checked"
        );
    }
}
