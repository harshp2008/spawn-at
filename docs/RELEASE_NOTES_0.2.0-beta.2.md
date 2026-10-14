# spawn-at 0.2.0-beta.2: release notes

This is a beta, and a cleanup release. **Window placement, the ready-wait policy and the GNOME extension are exactly as in 0.2.0-beta.1.** It has been run on one machine: GNOME 46, Wayland, one monitor. The known limits of beta.1 still apply: read [RELEASE_NOTES_0.2.0-beta.1.md](RELEASE_NOTES_0.2.0-beta.1.md), "What is measured, and the caveats" and "What is untested".

## What changed for users

- **The installer resolves releases more reliably.** `install.sh` is fetched from `main`, and it sometimes failed with "Could not resolve latest release via GitHub API" because its API calls had no retry and hid their errors. It now retries (`--connect-timeout 10 --max-time 60 --retry 3 --retry-delay 2`), uses `GITHUB_TOKEN` when you set it (the unauthenticated API limit is 60 requests an hour per IP), falls back to `releases.atom` (`--prerelease`) or the `releases/latest` redirect (stable) when the API fails, and downloads the tarball from `releases/download/<tag>/<asset>`, not from the API. Failures say what went wrong (HTTP status, rate limit, timeout, no network) and what to do. It no longer uses `python3`. Flags are unchanged. The unsupported-platform error and the "package does not contain `spawn-at`" error now go to stderr. This change reached `main` after the beta.1 tag, so beta.2 is the first tagged release that carries it.
- **The login update-check autostart entry honours `XDG_CONFIG_HOME`**, like the config file. With an absolute `XDG_CONFIG_HOME` it is written to `$XDG_CONFIG_HOME/autostart/spawn-at-update-check.desktop`. With the variable unset, empty or relative, the path is unchanged: `~/.config/autostart/spawn-at-update-check.desktop`.
- **README wording.** The intro no longer calls the X11 driver "finished": X11 and XWayland keep the old 30 ms rule, can flash, and were tried with the test window only, as the rest of the README already said. The long X11/XWayland paragraph is a short summary that links to the detailed docs.

Developer-facing (no change to what the binary does): CI on pull requests; the release workflow fails if `README.md` or `LICENSE` is missing from the tarball and checks that the tarball lists `./spawn-at`; the `hostile` verification category is in the `full` and `release-gate` suites (all 9 specs are optional, so the gate is unchanged); dead code and one unused dependency removed; docs reorganised. Details are in [CHANGELOG.md](../CHANGELOG.md).

## Upgrade notes

- **Autostart, only if you set `XDG_CONFIG_HOME`:** an autostart entry written by an earlier version under `~/.config/autostart` is **not moved or removed**. Re-run `spawn-at install` to write the entry in the new place, and delete the old file yourself if you want it gone (`spawn-at uninstall` removes only the new location). If `XDG_CONFIG_HOME` is unset, nothing changes.
- You do not need to reinstall the extension or log out: no extension file changed.
- No command-line flag, config key, exit code or D-Bus member changed. `spawn-at --help` and `--help` of each of the 13 subcommands are byte-identical to the 0.2.0-beta.1 build (compared; nested subcommands such as `config set` were not).
- Plain `cargo build` and `cargo test` at the repository root cover only the `spawn-at` package (this was already so; it is now declared). Name `spawn-at-verify` explicitly: `cargo test -p spawn-at-verify --features tui,fixture`.

## What did not change

- Placement behavior, the ready-wait policy (cap 250 ms, hard limit 1500 ms, `wait.strategy` and `wait.cap_ms`) and the legacy 30 ms rule for XWayland, X11 and the GNOME 42-44 tree.
- The GNOME extensions: no file under `assets/gnome/` differs from beta.1 (`git diff v0.2.0-beta.1..HEAD -- assets` is empty).
- The X11/XWayland findings (sync counter, `LD_PRELOAD` shim, and the rest) are documentation only. None of it ships.
- Known limits: the extension cannot read an app's reported minimum size on GNOME 46, so an app at its minimum or one that never answers costs the full cap; measurements are from the test window on one machine; wire time is an optimistic bound and no screen capture was done.

## Tests (this tree)

| Suite | Passed |
| :-- | --: |
| `spawn-at` | 211 (197 unit, 10 `config_cli`, 4 `install_notice_cli`) |
| `spawn-at-core` | 64 |
| `xtask` | 10 |
| `spawn-at-verify` (`--features tui,fixture`) | 404, 4 ignored (they need a live display) |
| Rust total | 689, 4 ignored |
| Node (`node --test tests/*.js`) | 183 |

Beta.1 had 209 `spawn-at` tests; the two new ones cover the autostart path. The rest of the counts are unchanged. `cargo fmt --check` and `cargo clippy --workspace --all-targets --features spawn-at-verify/tui,spawn-at-verify/fixture -- -D warnings` are clean.

## Not verified

- **CI and the release workflow.** `.github/workflows/ci.yml` and the new check in `release.yml` have never run on GitHub (the tag push is their first real run). Locally I checked that they parse as YAML, ran the same commands CI runs, and replayed the packaging lines in a scratch directory.
- **A clean CI machine.** `spawn-at-verify` builds with `winit` and `softbuffer`; it passed here, where system libraries may be present that a bare runner lacks.
- **macOS and Windows code.** The `cfg(target_os)` branches (for example in `escalate.rs` and `installer.rs`) have never been compiled, anywhere.
- **GNOME versions other than 46.** Nothing was run on GNOME 42 to 45 or 47 to 50, and no live GNOME session was used for this release at all: the tests are the unit, integration and node suites.
- **Real XWayland and X11 applications**, Qt and Electron apps, multi-monitor setups and fractional scaling (as in beta.1).
- **The installer against the real network.** It was tested with a fake `curl` (48 cases, before and after the stderr change), not against GitHub.
- **The autostart fix in a real desktop session.** It is tested as a path computation; nobody logged in with `XDG_CONFIG_HOME` set to check that the session ran the entry.
- The 4 ignored `spawn-at-verify` tests, and the `hostile` specs, need a live GNOME Wayland session and were not run.

## How to report a problem

The same as in beta.1: open an issue at <https://github.com/harshp2008/spawn-at/issues> with your GNOME Shell version, session type, `spawn-at config show`, the exact command, and whether `spawn-at config set wait.strategy legacy` changes the result. To go back to the pre-0.2 behaviour: `spawn-at config set wait.strategy legacy`.
