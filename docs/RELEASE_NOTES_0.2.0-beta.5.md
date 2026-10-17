# spawn-at 0.2.0-beta.5: release notes

This is a **prerelease** (beta). **On GNOME Wayland nothing visible changes.** Window placement, the ready-wait policy and the GNOME extension (`assets/gnome/`) are byte-identical to 0.2.0-beta.4 (`git diff 2e64085..main -- assets` is empty; 2e64085 is the commit the v0.2.0-beta.4 tag points at). GNOME Wayland was last verified live by the maintainer on GNOME 46, Wayland, one monitor, the only configuration ever tested; no live run was made while preparing this release. The known limits of beta.1 to beta.4 still apply: read [RELEASE_NOTES_0.2.0-beta.1.md](RELEASE_NOTES_0.2.0-beta.1.md), "What is measured, and the caveats" and "What is untested".

## What is new

- **Unsupported desktops are no longer handled silently.** On a non-GNOME Wayland session (or one with no `XDG_SESSION_TYPE`), beta.4 and earlier switched to the X11 backend without a word, which sees XWayland windows only. From this release, without `SPAWN_AT_BACKEND` set, spawn-at exits with status 1 and an error that names `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE`, says GNOME is the supported desktop and Sway is experimental (`SPAWN_AT_BACKEND=sway`), and says how to try X11 anyway: `SPAWN_AT_BACKEND=x11` ("it may or may not work"). Unchanged: GNOME sessions (including a GNOME session whose D-Bus connection fails, which still falls back to X11), real X11 sessions on any desktop, and an explicit `SPAWN_AT_BACKEND=x11` or `sway`. `spawn-at install` and `uninstall` on such a desktop print the same error and change nothing. `update --check --headless` (the login update check) still works there and exits 0.
- **`install.sh` checks the desktop before it downloads anything.** On an unsupported desktop it says so and asks `Install anyway? [y/N]` (No, empty or no answer: nothing is installed, exit status 1). Without a terminal it stops with the same notice and tells you to re-run with the new `--allow-unsupported`. After an accepted install it runs the installer with `SPAWN_AT_BACKEND=x11` and reminds you that spawn-at needs that variable on this desktop. Supported environments see no change. `install.sh` is fetched from `main`, so this part was live from the moment it was merged, not from this tag.
- **A documentation site**, built with Starlight, at <https://harshp2008.github.io/spawn-at/>: user guide, CLI reference generated from `--help`, and the rustdoc output. It includes a **Known limits** page (summarised below).
- **Docs and comments corrected against the code** (no code change): the command pages now state exit statuses, `--json` fallbacks, the `--no-wait` placement step, and the `install` and `uninstall` messages as the code produces them. The comments of the Rust code were rewritten from the function bodies; `docs/COMMENT_STYLE.md` is the style guide.
- **Developer tool only:** `spawn-at-verify` can test an existing window (version 2 specs, 22 new catalog entries), traces the extension's log per action, stops cleanly on Ctrl-C, SIGTERM and SIGHUP, and its TUI scrolls, orders and returns focus better. None of it is in the release tarball (`spawn-at-verify` is never packaged). Details in [CHANGELOG.md](../CHANGELOG.md).
- **CI:** the actions in `release.yml` and `ci.yml` are newer releases pinned by commit SHA. The `release.yml` change had not run on a tag before this one.

## Known limits

The same content, longer, is on the docs site page "Known limits". Everything below was read from the code, not run on a live session unless stated.

- **`spawn` exits 0 when no window appears**, on GNOME, Sway and X11. It exits non-zero only for an empty command, invalid options, `--area screen` without the helper it needs, or a program that cannot be started. `spawn --json` then prints `window_id` `null` with zero position and size.
- **The state commands exit 0 when the state does not change.** `focus`, `defocus`, `maximize`, `minimize`, `restore` and `transform` wait up to 2 seconds, drop the result and exit 0. `defocus` prints `Defocused window ...` either way. `close` is the exception: it fails with `did not close within ...` after 1500 ms. This is pinned by tests and is not fixed, because it would change an exit code.
- **`install` and `uninstall` banners.** After a non-zero `gnome-extensions enable`, `install` prints a note and then `successfully installed and enabled`. `uninstall` ignores the result of `gnome-extensions disable` and always prints `successfully uninstalled`.
- **A stale registration after a failed `spawn` on GNOME.** The extension's disarm call is never sent on Linux. A registration with a name lasts 15 seconds, a wildcard one 1.2 seconds, and in that time an unrelated window that the extension matches to the name can be placed.
- **Matching is not strict.** `spawn --class` and the name `spawn` derives from the command match the whole class, its last dot part, hyphenated tails, daemon suffixes and, loosely, any title containing the name. The GNOME extension accepts a name that contains, or is contained in, a window identifier. Selectors on existing windows (`-c`, `-t`) are the whole class or last dot part, and a title substring.
- **"Reached" geometry is approximate.** Position within 1 px; size at least the request; after 300 ms `spawn` reports the last rectangle it read.
- **Sway is experimental**, opt-in only (`SPAWN_AT_BACKEND=sway`), and tested only on a private headless Sway 1.9 with the repo's own test window. `--anchor cursor` and `--monitor cursor` use (0, 0) in `spawn` without a message and fail in `transform`; `defocus`, `maximize`, `minimize`, `restore` and `query pointer` are unsupported; the window can flash for one frame.
- **X11 and XWayland can flicker.** The native X11 driver has no cloak, and no test talks to an X server. On XWayland the project's test window flashed in 1, 9 and 10 of 10 runs in different logins. See `docs/TEST_LOG.md`, sections 6.16 to 6.18, and the X11 page of the docs site.
- **GNOME 46 is the only version tested.** Other GNOME versions, more than one monitor, fractional scaling and Qt or Electron apps are untested.
- **Windows and macOS:** the crate compiles for them in CI; nothing was run there.

## How to install

```bash
# beta.5 is a prerelease, so --prerelease is needed to pick it as "latest"
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --prerelease

# or pin this version
curl -fsSL https://raw.githubusercontent.com/harshp2008/spawn-at/main/install.sh | bash -s -- --version v0.2.0-beta.5
```

Without `--prerelease` or `--version`, `install.sh` asks GitHub for the latest stable release, which does not include a prerelease.

## Upgrade notes

- No extension reinstall is needed: no file under `assets/gnome/` changed since beta.4.
- The only behavior change in the binary is the unsupported-desktop error above. No command-line flag, config key or D-Bus member was added or removed in `spawn-at`. `install.sh` gains `--allow-unsupported`.
- If you use spawn-at on KDE, Hyprland or another non-GNOME Wayland desktop and relied on the silent X11 fallback, set `SPAWN_AT_BACKEND=x11` to keep it.

## Tests (this tree)

| Suite | Passed |
| :-- | --: |
| `spawn-at` | 443 |
| `spawn-at-core` | 64 |
| `xtask` | 53 |
| `spawn-at-verify` (`--features tui,fixture`) | 619, 5 ignored (4 need a live display, 1 needs a private Sway) |
| Rust total | 1179, 5 ignored |
| Node (`node --test tests/*.js`) | 204 |

## What was NOT verified

- Everything listed under Known limits as read from the code only.
- GNOME versions other than 46 on Wayland, and X11 sessions with real applications.
- Real Sway sessions and Sway other than 1.9.
- Windows and macOS runs.
- The release workflow on this tag: its updated actions have not run on a tag before.

## How to report a problem

Open an issue at <https://github.com/harshp2008/spawn-at/issues> with your GNOME Shell (or Sway) version, session type, `spawn-at config show`, the exact command, and what was printed.
