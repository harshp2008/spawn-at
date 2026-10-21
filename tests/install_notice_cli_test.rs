//! The untested-platform notice of `spawn-at install`, through the built binary. A stand-in
//! `gnome-shell` first on `PATH` reports the version, `--dry-run --headless` stops before anything
//! is installed, so nothing on the machine changes. The GNOME driver is only chosen on a GNOME
//! session with a session bus, so on any other machine these tests say so and return.

use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Makes every stand-in directory unique, so parallel tests never remove each other's.
static SEQ: AtomicUsize = AtomicUsize::new(0);

fn install_dry_run(version: &str, extra: &[&str]) -> Output {
    let dir = std::env::temp_dir().join(format!(
        "spawn-at-gs-{}-{}-{}",
        std::process::id(),
        version.replace('.', "_"),
        SEQ.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let stub = dir.join("gnome-shell");
    std::fs::write(
        &stub,
        format!("#!/bin/sh\necho \"GNOME Shell {version}\"\n"),
    )
    .unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = Command::new(env!("CARGO_BIN_EXE_spawn-at"))
        .args(["install", "--dry-run", "--headless"])
        .args(extra)
        .env("PATH", path)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// True when this machine picks the GNOME driver (the dry run names the GNOME extension).
fn gnome_driver_available() -> bool {
    let o = install_dry_run("46.0", &[]);
    let t = stdout(&o);
    if t.contains("GNOME") && t.contains("extension") {
        return true;
    }
    eprintln!("skipped: no GNOME session with a session bus here ({t:?})");
    false
}

/// GNOME 42, 43 and 44 print the legacy notice (UNTESTED, never run on a real GNOME 42-44, the
/// issues URL) before anything else; a headless run still goes on (exit 0) so scripts keep working.
#[test]
fn test_legacy_versions_print_the_untested_notice_and_headless_goes_on() {
    if !gnome_driver_available() {
        return;
    }
    for v in ["42.0", "43.9", "44.2"] {
        let o = install_dry_run(v, &[]);
        let t = stdout(&o);
        assert!(o.status.success(), "{v}: {t}");
        assert!(t.contains("NOTICE") && t.contains("UNTESTED"), "{v}: {t}");
        assert!(
            t.contains("legacy extension") && t.contains("issues"),
            "{v}: {t}"
        );
        assert!(
            t.find("NOTICE").unwrap() < t.find("Dry run").unwrap(),
            "{v}: {t}"
        );
    }
}

/// 45 and 47 are untested too; `--yes` changes nothing about the notice (it is still printed).
#[test]
fn test_other_versions_are_untested_and_yes_still_prints_the_notice() {
    if !gnome_driver_available() {
        return;
    }
    for v in ["45.1", "47.0"] {
        let t = stdout(&install_dry_run(v, &["--yes"]));
        assert!(t.contains("is UNTESTED"), "{v}: {t}");
        // The wording says what IS tested (built from the verified list), not "only 46".
        assert!(
            t.contains("spawn-at has been tested on GNOME Shell 46")
                && !t.contains("only been run"),
            "{v}: {t}"
        );
    }
}

/// GNOME 46 on a Wayland session prints no notice. On an X11 session it prints the "partly
/// tested" one, never the UNTESTED one.
#[test]
fn test_46_has_no_untested_notice() {
    if !gnome_driver_available() {
        return;
    }
    let t = stdout(&install_dry_run("46.0", &[]));
    assert!(!t.contains("UNTESTED"), "{t}");
    if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland") {
        assert!(!t.contains("NOTICE"), "{t}");
    }
}

/// The flags are documented.
#[test]
fn test_install_help_lists_yes_and_dry_run() {
    let o = Command::new(env!("CARGO_BIN_EXE_spawn-at"))
        .args(["install", "--help"])
        .output()
        .unwrap();
    let t = stdout(&o);
    assert!(t.contains("--yes") && t.contains("--dry-run"), "{t}");
}

/// The dry-run output (notice, then the dry-run line) has no blank line at the start or the end
/// and never two blank lines in a row; the notice and the dry-run line are one blank line apart.
#[test]
fn test_dry_run_output_has_single_blank_lines() {
    if !gnome_driver_available() {
        return;
    }
    for v in ["43.0", "46.0"] {
        let t = stdout(&install_dry_run(v, &[]));
        assert!(!t.starts_with('\n'), "{v}: {t:?}");
        assert!(t.ends_with('\n') && !t.ends_with("\n\n"), "{v}: {t:?}");
        assert!(!t.contains("\n\n\n"), "{v}: {t:?}");
        if v == "43.0" {
            let (notice, rest) = t.split_once("\n\n").expect("blank line between blocks");
            assert!(notice.contains("UNTESTED"), "{t:?}");
            assert!(rest.starts_with("Dry run:"), "{t:?}");
        }
    }
}

/// `install --help` lists --clean-install, and says the configuration is kept.
#[test]
fn test_install_help_lists_clean_install() {
    let o = Command::new(env!("CARGO_BIN_EXE_spawn-at"))
        .args(["install", "--help"])
        .output()
        .unwrap();
    let t = stdout(&o);
    assert!(t.contains("--clean-install"), "{t}");
    assert!(t.contains("configuration and settings are kept"), "{t}");
}
