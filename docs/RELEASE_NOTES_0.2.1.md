# spawn-at 0.2.1: release notes

This is a patch release of the 0.2 series: it fixes eight issues found after 0.2.0 and changes no window-placement behaviour. `spawn-at` places windows on GNOME Shell on Wayland, and only GNOME 46 has been tested.

## What changed

- **#47:** `install` without a terminal (`--headless`, or stdout not a terminal), which includes the `install --skip-bin --headless` that `update` runs on GNOME, no longer overwrites `update.auto_check`, `update.channel` and `update.notify`. A key the config file already has is kept unless its own flag is given.
- **#48:** `install --skip-bin` writes the login entry's `Exec=` from the installed binary, not from the running one, so an update no longer leaves the entry pointing into a deleted temp directory.
- **#49:** `update --check` in a terminal no longer writes its result into the config file.
- **#50:** the silent `update --check --headless`, which the login entry runs, returns before asking GitHub when `update.auto_check` is false.
- **#51:** `install.sh --help` (and `-h`) prints the script's own flags and exits 0 before any platform check or network call.
- **#56:** `spawn-at-verify` (developer tool) counts required tests that are still pending as pending, not as failed, and `report.md` shows PENDING instead of NOT QUALIFIED when nothing failed.
- **#57:** after you answer No to the log-out question, `uninstall` no longer names `--gnome-wayland-logout=false` or `--gnome-x11-reload=false`.
- **#58:** on GNOME, `install` and `uninstall` no longer print the success line when `gnome-extensions enable` or `disable` fails. `install` prints a plain note, `uninstall` a warning. Exit statuses are unchanged.

The GNOME extension (`assets/gnome/`) is unchanged since 0.2.0. The full list is in [CHANGELOG.md](../CHANGELOG.md).

## Not fixed here

- **#52:** `update` still exits 0 when the release lookup fails; changing it changes the exit-code contract, which this patch release leaves alone.
- **#53:** there is still no checksum or signature check in `update` or `install.sh`; it needs release tooling that does not exist yet.
- **#54:** `spawn --focus`, `--no-focus` and `--defocus` are still accepted and ignored; changing them changes the help-text contract.
- **#55:** on GNOME 46 the extension still cannot read a window's minimum size; fixing it means changing the extension, which stays byte-for-byte as in 0.2.0.

## How to install and update

```bash
# latest stable release (this one)
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash

# or pin a version
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --version v0.2.1
```

`--prerelease` selects the newest release including pre-releases. From an installed copy, `spawn-at update` finds and installs a newer release; `spawn-at update --check` only looks. The flags are on the docs site page "Installer flags", and what `update` does is on "Update".

## What was verified

Run on 2026-10-18 on the release-prep branch, with the CI commands from `.github/workflows/ci.yml`:

- `cargo fmt --check` is clean.
- `cargo clippy --workspace --all-targets --locked --features spawn-at-verify/tui,spawn-at-verify/fixture -- -D warnings` is clean.
- `cargo test --locked -p spawn-at -p spawn-at-core -p xtask`, counted per package: `spawn-at` 460, `spawn-at-core` 64, `xtask` 54, all passed.
- `cargo test --locked -p spawn-at-verify --features tui,fixture`: 625 passed and 5 ignored (4 need a live display, 1 needs a private Sway).
- `cargo xtask docs-site` builds the documentation site and its link check passes.
- The Node tests (`node --test tests/*.js`) were not run for this release prep. The CI run on `main` at `f7bacde` (the commit this branch starts from) reports 204 tests, 204 passed.
- Nothing was run on a live GNOME session. The fixes were checked by unit tests and by reading the code only.

## What was NOT verified

- The real install, uninstall and update flows on GNOME, including the fixes for #47, #48, #57 and #58.
- What GNOME does at login with a login-entry `Exec=` target that does not exist.
- Whether a fresh install makes `gnome-extensions enable` exit non-zero (the case #58 prints a note for).
- GNOME versions other than 46, real Sway sessions and Sway versions other than 1.9, X11 or XWayland with real applications.
- Windows and macOS runs.

## How to report a problem

Open an issue at <https://github.com/harshp2008/spawn-at/issues> with your GNOME Shell (or Sway) version, session type, `spawn-at config show`, the exact command, and what was printed.
