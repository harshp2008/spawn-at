//! `spawn-at -V` and `--version` through the built binary. The text depends on how the binary was
//! built (an exact release tag, or not), so each test accepts the form that build prints and
//! checks it fully. Neither flag needs a desktop session.

use std::process::Command;

fn run(flag: &str) -> (bool, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_spawn-at"))
        .arg(flag)
        .output()
        .unwrap();
    (
        o.status.success(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// Both flags exit 0, print the same text on stdout and nothing on stderr. A release build prints
/// the one line `spawn-at <version>`; a dev build prints `DEV BUILD <crate version>` first and
/// never starts with `spawn-at ` (the shape Win11Clip's installer reads as "no release version"),
/// then the last release, a rule and the commit line the verify preflight reads.
#[test]
fn test_version_flags_print_one_of_the_two_forms() {
    let (ok_short, short, err_short) = run("-V");
    let (ok_long, long, err_long) = run("--version");
    assert!(ok_short && ok_long);
    assert_eq!(short, long);
    assert!(
        err_short.is_empty() && err_long.is_empty(),
        "{err_short}{err_long}"
    );
    assert!(short.ends_with('\n'), "{short:?}");
    let lines: Vec<&str> = short.lines().collect();
    if lines.len() == 1 {
        let rest = lines[0].strip_prefix("spawn-at ").expect("release form");
        assert!(rest.chars().next().unwrap().is_ascii_digit(), "{short}");
    } else {
        assert_eq!(lines[0], format!("DEV BUILD {}", env!("CARGO_PKG_VERSION")));
        assert!(lines[1].starts_with("last release was "), "{short}");
        assert!(lines[2].starts_with("-----"), "{short}");
        assert!(lines[3].starts_with("Previous Commit ID: "), "{short}");
        assert_eq!(lines.len(), 4, "{short}");
    }
}
