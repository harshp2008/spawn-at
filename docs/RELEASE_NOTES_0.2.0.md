# spawn-at 0.2.0: release notes

`spawn-at` is one command and one placement model, with a driver per desktop underneath. This is the **stable** release of the 0.2 series. GNOME Shell on Wayland is the supported target, and only GNOME 46 has been tested.

## Main changes since 0.1.0

- A command (`spawn-at`) that places a window at an anchor, margin and size with one placement model, and a driver per desktop underneath.
- **GNOME Shell on Wayland** is the supported target: the window is kept invisible until it is placed and the app has had time to draw at the new size. Only GNOME 46 has been tested.
- **X11 and XWayland** go through a native X11 driver without cloaking, so windows can flash.

- On GNOME 46 Wayland the extension keeps a resized window hidden until the app has answered the size request or a cap runs out (default 250 ms, hard limit 1500 ms).
- New placement options: `--area`, `--pivot`, `--clamp`, `--monitor`.
- Changed: `-o/--offset` meant an offset from the cursor in 0.1.0; in 0.2.0 it is an alias of `-p/--pos`.
- New commands: `config`, `update`, `close`, `query`, `transform`.
- New scripting flags: `--id`, `spawn --json`, `install --yes`, `--dry-run`.
- A second extension tree for GNOME 42 to 44 (never run on a real GNOME 42 to 44), and a D-Bus protocol with a version.
- An experimental Sway backend, off unless `SPAWN_AT_BACKEND=sway` is set.
- A non-GNOME Wayland session is now an error unless `SPAWN_AT_BACKEND` is set; 0.1.0 used the X11 backend without saying so.
- `install.sh` resolves releases with retries, has `--version` and `--prerelease`, and `spawn-at install` can do `--clean-install`.
- A documentation site, CI on pull requests, and test suites (counts below). The documentation site (`docs-site/`) has an update page, installer flags, files and paths, environment, requirements, troubleshooting, experiment records and a writing guide.

## What changed (technical)

To read what 0.2 added over 0.1, see [RELEASE_NOTES_0.2.0-beta.1.md](RELEASE_NOTES_0.2.0-beta.1.md) and the later beta notes, and [CHANGELOG.md](../CHANGELOG.md).

Each row gives what 0.1.0 did, what 0.2.0 does, and where the proof is. The 0.1.0 column comes from `git show v0.1.0:<path>`; "not verified" means the sources do not establish it. CHANGELOG headings are the version headings in [CHANGELOG.md](../CHANGELOG.md).

### Placement and ready-wait

| Item | 0.1.0 | 0.2.0 | Proof |
| :--- | :--- | :--- | :--- |
| When the window is revealed (GNOME) | Polls the frame rectangle every 10 ms. Reveals when position and size match, or after 5 checks if only the position matches, or after 35 checks (about 350 ms) | Keeps the window hidden until the app has answered the size request or a cap runs out. Default `wait.cap_ms` 250, hard limit 1500 ms | `v0.1.0:assets/gnome/extension.esm.js` (settling loop); CHANGELOG 0.2.0-beta.1, Added, "Ready-wait" |
| Flash count under load | not verified. The figures below are for `--wait-strategy legacy`, which the CHANGELOG calls the pre-0.2 rule; 0.1.0 itself was not measured | With 22 CPU-burn threads: legacy rule 26 of 40 runs flashed, default ready-wait 0 of 40, by Wayland wire time. No screen capture | CHANGELOG 0.2.0-beta.1, Added; `docs/TEST_LOG.md` 6.23 |
| Cost of the ready-wait | not applicable | Window hidden longer under load (median 66.8 ms against 45.3 ms in one measurement, 56.2 against 48.9 in another). An app at its minimum size, or that never answers, waits the full cap (about +230 ms with cap 250) | CHANGELOG 0.2.0-beta.1, Changed |
| Placement flags | `-p/--pos`, `-o/--offset` (offset from the cursor), `-s/--size`, `-c/--class`, `-m/--margin`, `-t/-b/-l/-r` bound flags | `--area workarea\|screen\|unbounded`, `--offset/-o` as an alias of `--pos/-p`, `--pivot`, `--clamp`, per-side margins, `--monitor` | `v0.1.0:README.md` (Options Reference); CHANGELOG 0.2.0-beta.1, Added, "Placement model" |
| Settings | no config file or `config` command in the source | `spawn-at config` with `show`, `get`, `set`, `unset`, `reset`, `path`; `wait.cap_ms`, `wait.strategy`, `spawn.window_timeout_ms`; `SPAWN_AT_CONFIG`; `XDG_CONFIG_HOME` honoured | `git grep config v0.1.0 -- src` finds no config code; CHANGELOG 0.2.0-beta.1, Added; `docs/CONFIG.md` |
| `spawn gnome-terminal` without `--class` | not verified | Looks for class `org.gnome.Terminal` (it was `gnome-terminal-server` before 0.2) | CHANGELOG 0.2.0-beta.1, Changed |

### GNOME extension

| Item | 0.1.0 | 0.2.0 | Proof |
| :--- | :--- | :--- | :--- |
| Files | One file, `assets/gnome/extension.esm.js`, `shell-version` 45 to 48 | Two trees: `assets/gnome/modern` (GNOME 45 and newer) and `assets/gnome/legacy` (GNOME 42 to 44, never run on a real GNOME 42 to 44) | `git ls-tree v0.1.0 assets/gnome`; `ls assets/gnome`; CHANGELOG 0.2.0-beta.1, Added |
| D-Bus interface | `Arm` and the older methods | Protocol 6 (modern) and 5 (legacy). Adds the `ProtocolVersion` property, `DisarmSpawn`, `GetLayout`, `CloseWindow`, and the `SpawnClaimed` and `ResizeOutcome` signals (`ResizeOutcome` only in protocol 6). The CLI warns when the extension reports a protocol older than 3 | CHANGELOG 0.2.0-beta.1, Added, "D-Bus protocol 6" |
| XWayland and X11 windows | not verified | Still the old 30 ms rule and can flash; none of the X11 experiments ship | CHANGELOG 0.2.0-beta.1, Known limitations |

### Backends

| Item | 0.1.0 | 0.2.0 | Proof |
| :--- | :--- | :--- | :--- |
| GNOME Wayland | Driver for GNOME Wayland | Same driver, plus the ready-wait above | `v0.1.0:src/drivers/mod.rs` (`get_active_driver`) |
| Other Wayland desktops | Used the X11 driver without a message | Exit 1 with an error naming `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE`, unless `SPAWN_AT_BACKEND` is set | `v0.1.0:src/drivers/mod.rs`; CHANGELOG 0.2.0-beta.5, Changed, "Unsupported desktops" |
| Sway | README: "In Development" | Experimental, opt-in with `SPAWN_AT_BACKEND=sway`; tested on a private headless Sway 1.9 only | `v0.1.0:README.md` (Platform Support); CHANGELOG 0.2.0-beta.4, Added; `docs/DESIGN_SWAY.md` |
| Windows and macOS | README: "Planned" | The crate compiles for them in CI; running it there prints "spawn-at does not support this operating system yet" and exits 1 (not run) | `v0.1.0:README.md`; CHANGELOG 0.2.0-beta.3, Changed |

### Command line

| Item | 0.1.0 | 0.2.0 | Proof |
| :--- | :--- | :--- | :--- |
| Commands | `install`, `uninstall`, `spawn` | Adds `config`, `update`, `close`, `query` (`windows`, `layout`), `transform`, and window-state commands | `v0.1.0:src/main.rs` (`Commands`); CHANGELOG 0.2.0-beta.1, Added; command pages in `docs-site/src/content/docs/commands/` |
| Selecting and scripting | `--class` | Adds `--id` (numeric id from `query windows`), `spawn --json`, `install --yes` and `--dry-run` | CHANGELOG 0.2.0-beta.1, Added |
| `-a` | not verified | Rejected; use `--anchor` (exit 2) | CHANGELOG 0.2.0-beta.1, Breaking changes |
| Word starting with `-` before the command | The trailing arguments accept words starting with `-`, so such a word is taken as part of the program and its arguments | Usage error: `unknown option '-x'; to run a command that starts with '-', put it after --` (exit 2) | `v0.1.0:src/main.rs` (the `command` argument); CHANGELOG 0.2.0-beta.1, Breaking changes (the 0.2.0 message checked against `pre-cleanup-beta`) |
| `spawn --focus`, `--no-focus`, `--defocus` | not verified | Accepted and ignored | CHANGELOG 0.2.0-beta.5, Tests; this file, Known limits |

### Installer and update

| Item | 0.1.0 | 0.2.0 | Proof |
| :--- | :--- | :--- | :--- |
| `install.sh` release lookup | One GitHub API call for the latest release with errors hidden, then the `releases/latest/download` URL if that gave no link | API calls with timeouts and `--retry 3`, a fallback to `releases.atom` and the `releases/latest` redirect, tag validation, tarball from `releases/download/<tag>/<asset>`, causes printed on failure | `v0.1.0:install.sh`; CHANGELOG 0.2.0-beta.2, Fixed |
| `install.sh` flags | No `--version`, `--prerelease` or channel handling | `--version <tag>`, `--prerelease`, `--clean-install` passed on, `--allow-unsupported` | `v0.1.0:install.sh` (no match for `channel` or `prerelease`); CHANGELOG 0.2.0-beta.3 and 0.2.0-beta.5; `docs-site/src/content/docs/installer-flags.md` |
| Download output | not verified | Progress bar on a terminal, announced retries, one error layout on stderr | CHANGELOG 0.2.0-beta.3, Fixed |
| Log-out question | not verified | Asked last; "No log-out needed" when the deployed extension is identical, ACTIVE, and older than the running Shell | CHANGELOG 0.2.0-beta.3, Added and Fixed |
| Existing install | not verified | `spawn-at install` shows the existing binary and its version and asks; `--clean-install` removes first, never your config file | CHANGELOG 0.2.0-beta.3, Added |
| Login autostart entry | not verified | `Exec=` holds the absolute binary path; honours `XDG_CONFIG_HOME` | CHANGELOG 0.2.0-beta.1, Fixed; 0.2.0-beta.2, Changed |
| Updating | No `update` command | `spawn-at update` and `update --check`; channels `stable` and `beta` (`all` is a deprecated alias of `beta`); a build not at an exact tag reports `dev` | `v0.1.0:src/main.rs`; CHANGELOG 0.2.0-beta.1, Changed and Fixed; `docs-site/src/content/docs/update.md` |
| Untested GNOME versions | not verified | The installer prints what is and is not tested and asks (`--yes` skips) | CHANGELOG 0.2.0-beta.1, Added |

### Docs, tests and CI

| Item | 0.1.0 | 0.2.0 | Proof |
| :--- | :--- | :--- | :--- |
| Documentation | `README.md`, `CONTRIBUTING.md` | Documentation site in `docs-site/` (Starlight), `docs/` design notes, test plan and log, release notes | `git ls-tree v0.1.0`; CHANGELOG 0.2.0-beta.5, Added |
| CI | `release.yml` only (it ran `cargo test --verbose`) | Adds `ci.yml` (fmt, clippy, tests, Node tests) and `docs.yml` | `git ls-tree v0.1.0 .github/workflows`; `ls .github/workflows`; CHANGELOG 0.2.0-beta.2 and 0.2.0-beta.5 |
| Release workflow | Copied `README.md` and `LICENSE` with errors hidden | Fails if a packaged file is missing and checks the tarball lists `./spawn-at` | `v0.1.0:.github/workflows/release.yml`; CHANGELOG 0.2.0-beta.2, Fixed |
| Tests | no test files; 10 inline `#[test]` functions in src (geometry.rs 4, installer.rs 2, xdg.rs 4) | Suites listed under "What was verified" | `git ls-tree -r v0.1.0` has no path containing `test`; `git grep -c "#\[test\]" v0.1.0 -- src` prints 4, 2, 4; counts below |
| Verification tool | not present | `spawn-at-verify` (developer tool, not in the release binary) | `git ls-tree v0.1.0`; `crates/spawn-at-verify` |

## How to install and update

```bash
# latest stable release (this one)
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash

# or pin a version
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --version v0.2.0
```

`--prerelease` selects the newest release including pre-releases. From an installed copy, `spawn-at update` finds and installs a newer release; `spawn-at update --check` only looks. The flags are on the docs site page "Installer flags", and what `update` does is on "Update".

## Supported targets

The release workflow builds and uploads one file, `spawn-at-x86_64-unknown-linux-gnu.tar.gz` (`.github/workflows/release.yml`). `install.sh` stops on anything that is not Linux x86_64 and suggests building from source. Windows and macOS builds are compile-checked in CI and have never been run.

## What was verified

Only what the repository records:

- GNOME 46 on Wayland, one machine, one monitor, with the project's own test window: the measurements are in `docs/TEST_LOG.md` (section 6.23). With 22 CPU-burn threads running, the old rule (`--wait-strategy legacy`) flashed in 26 of 40 runs and the default ready-wait in 0 of 40, judged by the time of the window's commit on the Wayland wire. Section 6.30 repeated it after the X11 experiments were removed (27 of 40 and 0 of 40); it is a regression check, not a separate measurement of the ready-wait. No one captured the screen, so nothing says what is visible on screen ([Findings](https://harshp2008.github.io/spawn-at/findings/)).
- Sway 1.9, headless and private, with the test window (`scripts/sway-live.sh`). Not a real Sway session.
- The test suites, counts measured on 2026-10-18 at `b10ca5b` with the CI commands. `git diff b10ca5b..290377c --stat -- src crates assets scripts install.sh build.rs .github` shows only the `version` line in three `Cargo.toml` files (`crates/spawn-at-core`, `crates/spawn-at-verify`, `crates/xtask`), so no code differs at `290377c` apart from the version and the suites were not re-run: `spawn-at` 447, `spawn-at-core` 64, `xtask` 54, `spawn-at-verify` 619 passed and 5 ignored (4 need a live display, 1 needs a private Sway), Node 204.

## Known limits

Short list; the full text is on the docs site page [Known limits](https://harshp2008.github.io/spawn-at/known-limits/) and in [Known shortcomings](https://harshp2008.github.io/spawn-at/shortcomings/). All of it was read from the code, not run on a live session, unless the page says otherwise.

- **`spawn` exits 0 when no window appears.** See [Known limits](https://harshp2008.github.io/spawn-at/known-limits/#exit-status-and-results).
- **`disarm` is not sent on Linux.** After a failed `spawn` on GNOME a registration stays for 15 seconds (1.2 seconds for a wildcard one), and an unrelated window that matches the name can be placed ([Known limits](https://harshp2008.github.io/spawn-at/known-limits/#after-a-failed-spawn)).
- **`spawn --focus`, `--no-focus` and `--defocus` are accepted and ignored** ([Known shortcomings](https://harshp2008.github.io/spawn-at/shortcomings/)).
- **`update` on GNOME resets the `[update]` settings and writes a login entry that points into the temp directory** (read from source, not run). The child `install --skip-bin --headless` sets `auto_check = true`, `channel = "stable"` and `notify = false`, and writes the login entry's `Exec=` with the path of the unpacked binary, which `update` then deletes. Nothing rewrites the entry until a later `install` without `--skip-bin`. What GNOME does at login with a missing `Exec=` target is not verified ([Update](https://harshp2008.github.io/spawn-at/update/#code-finding-the-child-install-rewrites-settings)).
- **No checksum or signature check** in `update` or `install.sh`. The download is over HTTPS from the release asset URL only ([Update](https://harshp2008.github.io/spawn-at/update/#no-checksum-or-signature-check)).
- **`install` and `uninstall` banners** can say `successfully installed and enabled` or `successfully uninstalled` after a failed `gnome-extensions` call ([Known limits](https://harshp2008.github.io/spawn-at/known-limits/#exit-status-and-results)).

## What was NOT verified

- Everything listed under Known limits as read from the code only.
- GNOME versions other than 46, real Sway sessions and Sway versions other than 1.9, X11 or XWayland with real applications, more than two outputs and output scales other than 1 and 2 (on Sway), removal at system scope.
- Windows and macOS runs.

## About this release

It has the same code as [0.2.0-beta.6](RELEASE_NOTES_0.2.0-beta.6.md): the release changes documentation and the crate version only. Window placement, the ready-wait policy, the GNOME extension (`assets/gnome/`), `install.sh` and the command line are unchanged from beta.6. The code is the commit tagged `v0.2.0-beta.6`. Under `crates/`, the only difference besides the version lines is `crates/xtask/src/docs_check.rs`, the documentation-site check, which is not part of the release binary.

## How to report a problem

Open an issue at <https://github.com/harshp2008/spawn-at/issues> with your GNOME Shell (or Sway) version, session type, `spawn-at config show`, the exact command, and what was printed.
