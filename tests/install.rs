//! Installing without Rust: package the binary the way a release does
//! (`scripts/package.sh`), then install it with `scripts/install.sh` — the
//! path a teammate takes, with the download from GitHub swapped for local
//! files (`file://`). The installer is for macOS and Linux; Windows people
//! unzip a release instead, so these tests only run on Unix.
#![cfg(unix)]
// Tests may panic — that's how a test fails. (Product code may not: see Cargo.toml [lints].)
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::TempDir;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

/// The release build `install.sh` should choose on this machine. Spelled out
/// here on its own, so a wrong choice in the script fails these tests.
fn this_target() -> String {
    let os = match std::env::consts::OS {
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-musl",
        other => panic!("no release build for {other}"),
    };
    format!("{}-{os}", std::env::consts::ARCH)
}

fn script(name: &str) -> String {
    format!("{}/scripts/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Package the binary under test into `dist`, exactly as the release workflow does.
fn package(dist: &Path) {
    let out = Command::new("sh")
        .arg(script("package.sh"))
        .arg(this_target())
        .arg(env!("CARGO_BIN_EXE_hippo-task"))
        .arg(dist)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "package.sh failed:\n{}",
        text(&out.stderr)
    );
}

/// Run the installer against the release files in `dist`, installing into `bin`.
fn install(dist: &Path, bin: &Path) -> Output {
    Command::new("sh")
        .arg(script("install.sh"))
        .env_remove("HIPPO_VERSION")
        .env("HIPPO_DOWNLOAD_BASE", format!("file://{}", dist.display()))
        .env("HIPPO_INSTALL_DIR", bin)
        .output()
        .unwrap()
}

fn version_of(binary: &Path) -> String {
    let out = Command::new(binary).arg("--version").output().unwrap();
    text(&out.stdout).trim().to_string()
}

#[test]
fn the_installer_puts_a_working_hippo_task_on_disk_and_upgrades_in_place() {
    let tmp = TempDir::new("install");
    let (dist, bin) = (tmp.path().join("dist"), tmp.path().join("bin"));
    package(&dist);

    let out = install(&dist, &bin);
    assert!(
        out.status.success(),
        "install.sh failed:\n{}",
        text(&out.stderr)
    );
    let installed = bin.join("hippo-task");
    let expected = format!("hippo-task {}", env!("CARGO_PKG_VERSION"));
    assert_eq!(version_of(&installed), expected);
    assert!(
        text(&out.stdout).contains(&expected),
        "says what it installed:\n{}",
        text(&out.stdout)
    );

    // Running it again is how you upgrade: it replaces the binary in place.
    let again = install(&dist, &bin);
    assert!(again.status.success(), "{}", text(&again.stderr));
    assert_eq!(version_of(&installed), expected);
    let leftovers: Vec<String> = fs::read_dir(&bin)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        leftovers,
        ["hippo-task"],
        "nothing but the binary is left behind"
    );
}

#[test]
fn a_release_archive_holds_the_binary_the_license_and_the_readme_with_a_checksum() {
    let tmp = TempDir::new("package");
    let dist = tmp.path().join("dist");
    package(&dist);

    let name = format!("hippo-task-{}", this_target());
    let archive = dist.join(format!("{name}.tar.gz"));
    let listing = Command::new("tar")
        .arg("-tzf")
        .arg(&archive)
        .output()
        .unwrap();
    let listing = text(&listing.stdout);
    for file in ["hippo-task", "LICENSE", "README.md"] {
        let path = format!("{name}/{file}");
        assert!(
            listing.lines().any(|l| l == path),
            "{path} missing from the archive:\n{listing}"
        );
    }

    // `<sha-256>  <file name>`, the format `shasum -c` and `sha256sum -c` read.
    let sum = fs::read_to_string(dist.join(format!("{name}.tar.gz.sha256"))).unwrap();
    let (hash, file) = sum
        .trim_end()
        .split_once("  ")
        .expect("hash, two spaces, name");
    assert_eq!(hash.len(), 64, "{sum}");
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()), "{sum}");
    assert_eq!(file, format!("{name}.tar.gz"));
}

#[test]
fn a_damaged_download_is_refused_and_nothing_is_installed() {
    let tmp = TempDir::new("install-damaged");
    let (dist, bin) = (tmp.path().join("dist"), tmp.path().join("bin"));
    package(&dist);
    let archive = dist.join(format!("hippo-task-{}.tar.gz", this_target()));
    let mut bytes = fs::read(&archive).unwrap();
    bytes.push(0); // one byte too many: no longer what the release published
    fs::write(&archive, bytes).unwrap();

    let out = install(&dist, &bin);
    assert!(!out.status.success(), "{out:#?}");
    assert!(
        text(&out.stderr).contains("checksum"),
        "says why:\n{}",
        text(&out.stderr)
    );
    assert!(!bin.join("hippo-task").exists());
}

#[test]
fn a_release_that_cannot_be_downloaded_names_the_file_it_needed() {
    let tmp = TempDir::new("install-missing");
    let (dist, bin) = (tmp.path().join("dist"), tmp.path().join("bin"));
    fs::create_dir_all(&dist).unwrap(); // a "release" with no files in it

    let out = install(&dist, &bin);
    assert!(!out.status.success(), "{out:#?}");
    let wanted = format!("hippo-task-{}.tar.gz", this_target());
    assert!(
        text(&out.stderr).contains(&wanted),
        "names {wanted}:\n{}",
        text(&out.stderr)
    );
    assert!(!bin.join("hippo-task").exists());
}
