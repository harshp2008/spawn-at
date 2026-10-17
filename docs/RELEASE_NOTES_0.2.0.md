# spawn-at 0.2.0: release notes

This is the **stable** release of the 0.2 series. It has the same code as [0.2.0-beta.6](RELEASE_NOTES_0.2.0-beta.6.md): the release changes documentation and the crate version only. Window placement, the ready-wait policy, the GNOME extension (`assets/gnome/`), `install.sh` and the command line are unchanged from beta.6. The only difference in `crates/xtask` is `docs_check.rs`, the documentation-site check, which is not part of the release binary (`git diff 75e867a..HEAD --stat` on the release branch; 75e867a is the commit of the beta.6 documentation merge).

To read what 0.2 added over 0.1, see [RELEASE_NOTES_0.2.0-beta.1.md](RELEASE_NOTES_0.2.0-beta.1.md) and the later beta notes, and [CHANGELOG.md](../CHANGELOG.md).

## What 0.2.0 is

- A command (`spawn-at`) that places a window at an anchor, margin and size with one placement model, and a driver per desktop underneath.
- **GNOME Shell on Wayland** is the supported target: the window is kept invisible until it is placed and the app has had time to draw at the new size. Only GNOME 46 has been tested.
- **Sway** is an experimental, opt-in backend (`SPAWN_AT_BACKEND=sway`).
- **X11 and XWayland** go through a native X11 driver without cloaking, so windows can flash.
- A non-GNOME Wayland session is an error unless `SPAWN_AT_BACKEND` is set.
- Documentation site (`docs-site/`), with an update page, installer flags, files and paths, environment, requirements, troubleshooting, experiment records and a writing guide.

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

- GNOME 46 on Wayland, one machine, one monitor, with the project's own test window: the measurements are in `docs/TEST_LOG.md` (sections 6.23 and 6.30 for the ready-wait). Under heavy load the old rule flashed in 26 of 40 runs and the ready-wait in 0 of 40, judged by wire time; nobody measured pixels on screen.
- Sway 1.9, headless and private, with the test window (`scripts/sway-live.sh`). Not a real Sway session.
- The test suites, measured on 2026-10-18 on `main` at `b10ca5b` with the CI commands: `spawn-at` 447, `spawn-at-core` 64, `xtask` 54, `spawn-at-verify` 619 passed and 5 ignored (4 need a live display, 1 needs a private Sway), Node 204.

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
- The release workflow on the `v0.2.0` tag has not run yet.

## How to report a problem

Open an issue at <https://github.com/harshp2008/spawn-at/issues> with your GNOME Shell (or Sway) version, session type, `spawn-at config show`, the exact command, and what was printed.
