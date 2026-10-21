# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.2.3] - 2026-10-21

Patch release: the stale-arm fix (#84, #85). It needs both the new CLI and the protocol-7 extension. After upgrading, run `spawn-at update` (it redeploys the extension) or `spawn-at install`, then log out and back in, so the new extension is loaded.

### Added

- Extension protocol 7 (modern tree only): the D-Bus methods `ArmSpawnEntry(target_id, entry_key, instructions_json)` and `DisarmEntry(target_id, entry_key)`. `ArmSpawn` and `DisarmSpawn` are unchanged. The CLI uses the new methods only when the extension reports protocol 7 or higher.

### Fixed

- An arm that no window claims no longer misplaces the next show of the same app (#84). When `spawn` is run while the app's window is already visible and the app only forwards the request to its running instance, no window is created, the instruction stayed queued in the extension for 15 s, and the next window of that app was placed from it (the oldest entry is served first). `spawn` now withdraws its own entry when the wait ends without a claim (timeout, or an app that reused an old window). This needs the new CLI and the protocol-7 extension: run `spawn-at update` or `spawn-at install`, then log out and back in. A new CLI with an older extension, and an older CLI with the new extension, behave as before. Not fixed: a window shown within the CLI's own wait of about 4 s after such an arm, `--no-wait`, a CLI that is killed (all rely on the 15 s expiry), and the legacy extension for GNOME 42 to 44. Run on GNOME Shell 50.1 (Wayland) with test windows, and with node and D-Bus stubs; not run on other Shell versions or on X11.

## [0.2.2] - 2026-10-21

Patch release: GNOME Shell 49 and 50 support (#82). After upgrading, run `spawn-at update` (it redeploys the extension) or `spawn-at install`, then log out and back in, so the new `metadata.json` is written and loaded.

### Added

- GNOME Shell 49 and 50 are listed in `assets/gnome/modern/metadata.json`. Before, GNOME 50 marked the extension OUT OF DATE and never loaded it, so no D-Bus object appeared and every `spawn` printed the "protocol mismatch" warning and `UnknownMethod: Object does not exist`. The owner ran GNOME 50.1 (Ubuntu 26.04, Wayland) by hand with the two entries added and the extension was ACTIVE and placed windows correctly; GNOME 49 and the legacy tree were not run. It reaches users with the next release, and then `spawn-at install` has to be run again to write the new `metadata.json`.

### Fixed

- When the extension does not answer on the session bus, the error now says why: not installed, installed but disabled, OUT OF DATE (the installed `metadata.json` does not list the running Shell major: upgrade spawn-at, run `spawn-at install`, log out and back in), or loaded with an older protocol. Only that last case prints "log out and back in to load the updated extension". A failed `ProtocolVersion` read is no longer counted as protocol 0, so a missing extension no longer prints "protocol mismatch (detected older extension)" or the "does not support SpawnClaimed" note. Not run on a live Shell.

### Changed

- The placement line names the request, the area and the screen position, for example `Window mapped at final size (814x601), offset (100, 100) in workarea (origin 0, 40) -> screen position (100, 140).`, instead of only `... and positioned at (100, 140).` Placement itself is unchanged. The `--pos` and `--offset` help text said "absolute target screen coordinates"; it now says the position is counted from the `--area` rectangle (the work area by default).

- Documentation site only (nothing in `spawn-at`, the extension or `install.sh` changed): a table with a long cell (240 characters or more) on the pages written from `docs/` is shown as records, one block per row with the key on the left and the other columns as labelled fields, instead of as a table with 25-character cells. `<!-- site: records -->`, `<!-- site: records key=N -->` and `<!-- site: table -->` above a table choose the layout by hand; `cargo xtask docs-check-src` checks them. A list whose keys all start with a whole number is drawn as numbered steps and any other as cards, and `<!-- site: records style=steps -->` or `style=cards` chooses between the two (#69). The step 11 row of the migration table in `docs/DESIGN_PLATFORMS.md` has its Proof text in the Proof column again.

## [0.2.1] - 2026-10-18

Patch release. Since v0.2.0, `git diff v0.2.0..HEAD --stat -- src crates assets scripts install.sh build.rs .github tests` shows changes only in `src` (`config.rs`, `main.rs`, `session.rs`, `update.rs`, `platform/installer.rs`, `platform/linux/gnome/deploy.rs`), `install.sh` and `crates/spawn-at-verify` (the developer tool); `assets` (the GNOME extension), `scripts`, `build.rs`, `.github`, `tests`, `crates/spawn-at-core` and `crates/xtask` are unchanged. See [docs/RELEASE_NOTES_0.2.1.md](docs/RELEASE_NOTES_0.2.1.md) for what to expect and what is not verified.

None of this was run on a live GNOME session. The GNOME extension (`assets/gnome/`) is unchanged.

### Fixed

- `install` without a terminal (`--headless`, or stdout not a terminal), including the `install --skip-bin --headless` that `update` runs on GNOME, no longer overwrites `update.auto_check`, `update.channel` and `update.notify`. A key the config file already has is kept unless its own flag (`--update-channel`, `--update-notify`) is given; a missing key gets the old headless default. Interactive installs behave as before (#47).
- `install --skip-bin` writes the login entry's `Exec=` from the installed binary (the requested scope, else the other scope) and falls back to the running binary only when none is installed, so an update no longer leaves the entry pointing into its deleted temp directory (#48).
- `update --check` in a terminal no longer writes the check result into the config file. `update`, `update --headless` and the silent `update --check --headless` still do; the cache keys stay in the config file (#49).
- The silent `update --check --headless`, which the login entry runs, returns before asking GitHub when `update.auto_check` is false (#50).
- `install.sh --help` (and `-h`) prints the script's own flags and exits 0 before any platform check or network call (#51).
- `spawn-at-verify`: required tests that are still pending are counted as `pending_required`, not as failed, and `report.md` shows a PENDING badge instead of NOT QUALIFIED when nothing failed and no required test was skipped. Older `report.json` files still load (#56).
- `uninstall`: the line after saying No to the log-out question no longer names `--gnome-wayland-logout=false` or `--gnome-x11-reload=false` (#57).
- `install` and `uninstall` on GNOME no longer print the success line when `gnome-extensions enable` or `disable` fails: `install` prints a plain note (a freshly copied extension may only load after the next login), `uninstall` a warning. Exit statuses are unchanged (#58).

## [0.2.0] - 2026-10-18

Stable release. Same code as v0.2.0-beta.6; documentation and version only. `src`, `assets`, `scripts`, `tests`, `build.rs`, `.github`, `install.sh`, `crates/spawn-at-core` and `crates/spawn-at-verify` are unchanged since v0.2.0-beta.6; `crates/xtask` differs only in `docs_check.rs`, the documentation-site check, which is not part of the release binary (`git diff 75e867a..HEAD --stat` on the release branch). See [docs/RELEASE_NOTES_0.2.0.md](docs/RELEASE_NOTES_0.2.0.md) for what to expect and what is not verified.

### Changed

- The crate version is 0.2.0 (the four `Cargo.toml` files and `Cargo.lock`).
- `README.md` and the documentation site no longer describe the release as a beta: the install command is the plain `install.sh`, which installs the latest stable release (`--version <tag>` and `--prerelease` select others), and the notes that tied the unsupported-desktop refusal to v0.2.0-beta.5 are removed.
- The Update page, and the pages that repeat it, now say that the child `install --skip-bin --headless` that `update` runs on GNOME is awaited, sets `auto_check`, `channel` and `notify` unconditionally, and writes the login entry with the path of the temp-directory binary, which `update` deletes afterwards (read from source, not run).

### Added (documentation only)

- Pages: Update, Installer flags, Files and paths, Environment and exit codes, Requirements, Troubleshooting and FAQ, an Experiments section with one record per experiment of `docs/TEST_LOG.md` 6.10 to 6.35 and P8, and Writing the docs (with a page template and an experiment record template).
- The command aliases and the hidden wait flags are documented; `--no-focus` is no longer in the README example.
- The documentation-site check allows the extension identifier `spawn-at@harsh.local` (`crates/xtask/src/docs_check.rs`, developer tool).
- The experiment records name the project owner where they said "the user".
- `docs/RELEASE_NOTES_0.2.0.md`.

## [0.2.0-beta.6] - 2026-10-17

Pre-release. The GNOME extension (`assets/gnome/`) is byte-identical to 0.2.0-beta.4 and 0.2.0-beta.5 (`git diff 2e64085..main -- assets` is empty), and GNOME Wayland placement behaves as before. One change is visible to users: the PATH advice that the installer prints. Everything else since 0.2.0-beta.5 is documentation, test counts and this release's version metadata. See
[docs/RELEASE_NOTES_0.2.0-beta.6.md](docs/RELEASE_NOTES_0.2.0-beta.6.md) for what to expect and what is not verified.

### Changed

- Installer: the PATH advice is now idempotent and mentions logging out; it no longer suggests appending to a shell file.

## [0.2.0-beta.5] - 2026-10-17

Pre-release. The GNOME extension (`assets/gnome/`) is byte-identical to 0.2.0-beta.4 (`git diff 2e64085..main -- assets` is empty), and GNOME Wayland placement behaves as before. Two changes are visible to users: `spawn-at` refuses a non-GNOME Wayland session instead of silently using the X11 backend, and `install.sh` asks before installing on such a desktop. Everything else below is documentation, comments, tests and the developer-only `spawn-at-verify` tool. See
[docs/RELEASE_NOTES_0.2.0-beta.5.md](docs/RELEASE_NOTES_0.2.0-beta.5.md) for what to expect and what is not verified.

### Fixed

- `spawn-at-verify tui` (developer tool): scrolling with a wheel or touchpad no longer lags or runs on after you stop. The wheel reaches the TUI as Up/Down presses; the loop handled one per frame and rebuilt the whole drawer each time (measured: 100 queued events were 100 draws and 100 full rebuilds). A frame now reads everything pending (capped), applies it in order, and draws once (1000 events: 2 draws, 0 rebuilds); a burst accelerates (1 to 6 rows per event by recent rate) and a single notch is unchanged; the drawer's content is cached while only the scroll offset changes. A blank row now sits below the `end of details` rule. Headless tests only; not tried on a real touchpad.

- `spawn-at-verify tui` (developer tool): the details drawer ends with a blank row and a dim centred `─── end of details ───` rule as wide as the drawer, so the last event is no longer flush against the border, and its title shows `▼ more` (next to `▲ more`) while there is content below. Display only: copy, save and the reports are unchanged.

- `spawn-at-verify tui` (developer tool): scrolling the details drawer to its end (`G`, or holding PageDown) no longer locks the screen. The offset was stored unclamped (`G` set `u16::MAX`) and the next draw overflowed inside ratatui's `Paragraph` (`paragraph.rs:483`, a panic in a debug build), which left the terminal in raw mode on the alternate screen, so no key, Escape included, did anything. The offset is now clamped to the real rows (wrapped, with the blank rows between events) at the key and at the draw, on every resize; a panic hook restores the terminal before the message prints. The timeline also words a `reason` alike on every event (`at-target` and `AT_TARGET` both read `at target`). Covered by headless tests; not re-run on GNOME.

- `spawn-at-verify tui` (developer tool; nothing in `spawn-at` changed): FOCUS is now the second group of the Tests list, after PREFLIGHT (order: PREFLIGHT, FOCUS, QUERIES, ROBUSTNESS, PLACEMENT, CLOAK & DELAYED PAINT, TRANSFORMS, GENERAL). An earlier change had reordered the drawer instead. Focus now goes back to the terminal after the automatic part of every attended test (placement and cloak specs had no return before), is checked and retried, and the Result box always shows the recorded terminal, each `focus` command's exit code and what had focus afterwards. Covered on a private headless Sway; not verified on GNOME.

### Added

- **The documentation site is built with Starlight (Astro) instead of mdBook** (nothing in `spawn-at`, the extension or `install.sh` changed): same pages and order, with light and dark themes, local Pagefind search and no third-party request. `cargo xtask docs-site` needs Node and npm (exact package versions in `docs-site/package.json` and `package-lock.json`); `docs.yml` no longer downloads mdBook. The page about the rustdoc output moved from `/api.html` to `/rustdoc/`, because the rustdoc output itself is at `/api/`.
- **A documentation site** (nothing in `spawn-at`, the extension or `install.sh` changed): an mdBook user guide in `docs-site/` and the rustdoc API docs under `api/`, built by the new `.github/workflows/docs.yml` and meant for <https://harshp2008.github.io/spawn-at/>. The workflow builds and checks the site on pull requests and deploys it from `main` (GitHub Pages, build type `workflow`, URL as above; read with `gh api repos/harshp2008/spawn-at/pages`). The CLI reference is written at build time from the built binary's `--help` (one page per command, nested ones included, not committed), and the build fails if the binary is missing or a listed command has no page. Every hand-written page ends with a Sources list. The site makes no third-party request (no external fonts, scripts or analytics; the playground is off). New `cargo xtask` commands: `docs-site`, `docs-cli-reference`, `docs-check-src`, `docs-check-html`; build instructions in `README.md` and `docs/CODE_LAYOUT.md`.
- **`spawn-at-verify` traces the actions of tests of an existing window** (developer tool; nothing in `spawn-at` or the extension changed). `run` and the TUI now record, for each action of a `schema_version = 2` test, the extension's own log lines for the windows the runner launched (lines of any other window are dropped before anything is stored or shown) and show them as a timeline (event, ms after the first line, key fields) in the drawer, the Result panel and the `run` report (`report.json` `action_traces`, `report.md` section 7; the TUI still writes no report). `transform` and `focus` have a trace; `minimize`/`restore` a partial one; `maximize`, `defocus`, `close` and queries log nothing for their target, shown as `not applicable: the extension logs nothing for this command` (other reasons: `no extension on this backend (<backend>)`, `capture failed: <why>`, `no lines for this window in the slice`; Cloak and Budget show `n/a` without a cloak phase). A `transform` also gets an informational flash check (cloak to reveal, the fixture's paint events against the reveal; the verdict never changes, a clean result is not a guarantee, `possible` flashes are noisy, no pixels are captured). Capture is off for a bare `TestRunner`, so tests never touch the live extension. Inventory with file:line: `docs/DESIGN_RUNNER_SETUP.md` section 5.5. The action timeline is drawn by the same parser and block renderer as a spawn's timeline (header with the pid when known, `+N.N ms` offsets, aligned humanised rows, blank rows between events, floats to one decimal), the Trace row reads `✔ clean, no violations · N events` where the checks apply, and the markdown report uses the same parser; JSON keys are unchanged (additive only). Not run on GNOME. The descriptions of `transforms/resize-flash` and `transforms/move-flash` no longer say they do not trace.
- **Catalog specs for an existing window** (developer tool; nothing in `spawn-at` changed): 18 files, 22 entries, in `verify/queries/`, `verify/transforms/`, `verify/focus/` and `verify/robustness/`: query windows/layout/pointer, resize, reposition, anchor corners, maximize/minimize and restore, focus, switch, defocus, close by id, a nonexistent id, and three Sway "unsupported" contract specs. All `required = false`, all `moves-focus` where they open a window, four `Both` specs that ask a person (and measure no flicker). On a private headless Sway 15 pass and 7 GNOME-only specs are Skipped; none has been run on GNOME. The original 45 specs are byte-identical (a golden test and unchanged `list` hashes). The suite alias `lifecycle` now selects the `focus` category; `[expect.rect]` `anchor`/`pivot` follow a `[matrix]`. How to run them: `verify/README.md`.
- **`spawn-at-verify` can test an existing window** (developer tool; nothing in `spawn-at` itself changed). A spec with `schema_version = 2` launches its own fixture windows (`[[setup]]`), acts on them by the id the runner recorded (`--id ${name}` is the only selector), may run more actions (`[[then]]`), asserts on the windows (`[[check]]`, `stdout_present`, `stdout_lists_window`, `[expect.rect]`) and tears everything down. The runner owns the fixture's process handle, re-checks the target before every action, closes a window by id only while it still carries the run's unique title, and cleans up after a failed setup, a timeout, Ctrl-C, SIGTERM, SIGHUP and a panic. A spec for another backend is Skipped with `unsupported on this backend (<backend>): spec requires ...`; `[requires] backend` accepts `sway`; a spec with a `[[setup]]` must carry the tag `moves-focus`. The 45 existing specs are unchanged (a golden test of the loaded catalog, and `list` output hashes equal to `main`). Catalog specs that use it are listed in the entry above; syntax and safety model in `verify/README.md`, design in `docs/DESIGN_RUNNER_SETUP.md`.
- `scripts/verify-sway.sh`: runs `spawn-at-verify run` against a private headless Sway (through `scripts/sway-headless.sh`), forwards a signal to the runner and stops that Sway.

### Changed

- `spawn-at-verify tui` (developer tool; nothing in `spawn-at` changed): the status marker of the selected row is now visible in every state (the grey "not run" dot was grey on the grey highlight); a verdict key (`p x s k`) updates the row and the Result box in the next redraw and the window closes afterwards (`closing window...`), with teardown trouble shown as a separate `⚠` warning that never changes the verdict; `r` and a test start show "Running" before the work begins (the automatic part blocks the UI thread, so nothing could be drawn until it ended); after the automatic part of a `Both`/`Human` test focus goes back to the window that had it at the start (only a `focus` command is sent to it; `⚠ could not return focus to your terminal` if it did not take). On GNOME the raised terminal can cover the fixture window: see `verify/README.md`, section 9.
- `spawn-at-verify run` stops cleanly on Ctrl-C, SIGTERM and SIGHUP: no new test starts, the windows it opened are closed, the report is written, the exit code is 130 (it used to die without cleanup). `signal-hook` is now a normal dependency of the crate.
- CI: `release.yml` uses newer releases of `actions/checkout` (v7.0.1) and `softprops/action-gh-release` (v3.0.3), `ci.yml` of `actions/checkout` and `actions/setup-node` (v7.1.0), all pinned by commit SHA; `docs.yml` is new. The `release.yml` change had not run on a tag before this release.
- `spawn-at-verify`: with `SPAWN_AT_BACKEND=x11` or `sway` set, the runner no longer uses the GNOME journal or calls `SetLogging` on the extension, even when `XDG_CURRENT_DESKTOP` says GNOME. With `SPAWN_AT_BACKEND=sway` it blocks every test unless `SWAYSOCK` is inside the private `scripts/sway-headless.sh` session. The `quick` suite excludes the tag `moves-focus`.

- **Unsupported desktops are no longer handled silently.** On a non-GNOME Wayland session (or one with no `XDG_SESSION_TYPE`) spawn-at used to switch to the X11 backend without a word, which sees XWayland windows only. Without `SPAWN_AT_BACKEND` set, it now exits with status 1 and an error that names `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE`, says GNOME is the supported desktop and Sway is experimental (`SPAWN_AT_BACKEND=sway`), and says how to try X11 anyway: `SPAWN_AT_BACKEND=x11` ("it may or may not work"). Unchanged: GNOME sessions (including a GNOME session whose D-Bus connection fails, which still falls back to X11), real X11 sessions on any desktop, and an explicit `SPAWN_AT_BACKEND=x11` or `sway`. `spawn-at install` and `uninstall` on such a desktop print the same error and change nothing; there are no new flags, config keys or help text on the `spawn-at` command.
- `install.sh` checks the desktop before it downloads anything. On an unsupported desktop it prints that the desktop is not supported yet and asks `Install anyway? [y/N]` (No, empty or no answer: nothing is installed, exit status 1). Without a terminal it stops with the same notice and tells you to re-run with the new `--allow-unsupported`, which skips the question. After an accepted install it runs the installer with `SPAWN_AT_BACKEND=x11` and reminds you that spawn-at needs that variable on this desktop. Supported environments (GNOME, an X11 session, a forced backend) see no change. Because `install.sh` is fetched from `main`, this part is live as soon as it is merged.

### Documentation

- A behavior audit of the docs against the code (docs only; no code changed). The docs site gains a **Known limits** page, and the command pages now say what the code does: `spawn` exits 0 on GNOME, Sway and X11 when no window appears; `spawn --json` prints a null-id result in the no-window cases; `--no-wait` still runs the Sway and X11 placement step; `close` fails when the window stays, while `focus`, `defocus`, `maximize`, `minimize`, `restore` and `transform` exit 0 after a timed-out wait and `defocus` prints its success line either way; `install` and `uninstall` print "successfully ..." even when `gnome-extensions enable` or `disable` fails; window name matching is loose; nothing withdraws a GNOME registration after a failed `spawn`, so it lives 15 s (1.2 s for a wildcard); on Sway `transform` and `query pointer` report an error for the cursor. `docs/DESIGN_PLATFORMS.md` now says the X11 `arm` returns only the launch environment, and `docs/CODE_LAYOUT.md` lost its line-count column.
- Comment-only: the comments of the Rust code in `spawn-at` (including the Sway, X11 and GNOME drivers) and `spawn-at-core` were rewritten from the function bodies, and time phrases were removed from comments in `spawn-at-verify`; `docs/COMMENT_STYLE.md` is the style guide (no release status, no commit hashes, notes about other files as plain `//` comments with a grep hint). `scripts/check-comment-only-product.sh main` passes; no code, string, attribute or `--help` text changed.
- Docs-only corrections after a fact-check of the documentation site (nothing in `spawn-at`, the extension or `install.sh` changed): the book, `README.md` and `docs/` now say that the unsupported-desktop refusal is not in v0.2.0-beta.4 or earlier; that on Sway `--anchor cursor` and `--monitor cursor` place the window as if the pointer were at (0, 0); that the X11 experiment commits are in the history of `main`; that XWayland flashed in 1, 9 and 10 of 10 runs in different logins; and that `uninstall` prints "No uninstallation required for X11." (not "No installation required"). `CLAUDE.md` now says five verify tests stay ignored.
- Comment-only: ten rustdoc links in doc comments that pointed at items not in scope or private were turned into plain code spans or full paths, and `spawn-at-core` and `spawn-at-verify` got a short crate-level overview. `cargo xtask check-comment-only` passes for every touched crate (`scripts/check-comment-only-product.sh main`). One rustdoc error is left alone and its lint is allowed in the docs build: `<INDEX>` in the help text of `--monitor` (`src/cli/args.rs`), which clap prints, so changing it would change `--help`.
- `docs/DESIGN_RUNNER_SETUP.md`: design (now implemented, see its "Implementation status") of the verify runner change that lets a spec launch its own fixture window, act on it by its recorded id, assert on it and tear it down, plus the catalog specs and the transform flash measurement that would sit on it. Awaiting owner approval; no code changed.
- `docs/TEST_COVERAGE_AUDIT.md`: which `spawn-at` commands are tested, at which tier and on which backend, how the verify suite measures flicker, how the TUI test interface works, proposed new tests and the gaps found. No code changed.
- `verify/README.md` no longer describes a web UI (`spawn-at-verify serve`, `tui --web`); neither exists. It now says which commands write `report.json` and `report.md` (`run` and `calibrate`, not the TUI). `README.md` says `--focus`, `--no-focus` and `--defocus` are accepted on `spawn` but currently have no effect. `CLAUDE.md` notes that `scripts/smoke-live.sh` is run by the owner by hand.

### Tests

- `xtask` 10 -> 36 passed (26 new unit tests of the documentation commands); the other suites are unchanged.
- Runner existing-window setup: `spawn-at-verify` 414 -> 524 passed (4 still ignored), other crates and node unchanged (`spawn-at` 443, core 64, xtask 10, node 204; Rust 1041). Each safety guarantee has a test, and each guard was broken on purpose once to see a test fail; the real `run` binary is exercised against stand-in scripts (signals, exit 130, teardown, Sway guard), and the wrapper ran against a private headless Sway (`docs/TEST_PLAN.md` 7.4).
- Command coverage batch 1 (tests and docs only; the only edits to non-test files are `#[cfg(test)]` module declarations; no behavior, `--help` text, exit code or config key changed): 31 tests for `run_focus`, `run_defocus`, `run_maximize`, `run_minimize`, `run_restore` and `apply_focus_policy`, 11 D-Bus stub tests for the GNOME driver's focus, defocus, state, move and transform wrappers, 18 node tests for the extension's `DefocusWindow` and `SetWindowState`, 10 built-binary tests against a recording Sway stand-in (focus, close, transform, query, and the unsupported commands), 5 tests that check the verify suites and categories are wired consistently. Characterization tests pin two behaviors as they are today and are not contracts: `spawn --focus/--no-focus/--defocus` have no effect, and the state commands return success after a timed-out 2 s wait. Counts: `spawn-at` 443 (was 390), `spawn-at-verify` 414 + 4 ignored (409), node 204 (186).
- The login autostart entry (`spawn-at-update-check.desktop`, `Exec=<binary> update --check --headless`) needs no backend on an unsupported desktop, and no code changed: `update` returns before `init_backend()` in `main.rs`, and `run_update` never reaches the backend registry. New tests pin the entry byte for byte on GNOME, on a non-GNOME X11 session and on the opt-in case (`SPAWN_AT_BACKEND=x11` on KDE Wayland), that uninstall removes it and a reinstall rewrites a stale one, and that `update --check --headless` on KDE Wayland exits 0 without the unsupported-desktop error.

### Fixed

- `spawn-at-verify` preflight no longer blocks every test on a commit that is exactly a release tag. A binary that prints `spawn-at <version>` (no commit id) now passes when `git describe --tags --exact-match HEAD` is `v<version>` and the tree is clean; otherwise it fails with the same stale-binary message. Unrecognised `--version` output now fails instead of being skipped. Dev builds are checked as before.

## [0.2.0-beta.4] - 2026-10-15

Pre-release. The GNOME extension (`assets/gnome/`) and `install.sh` are byte-identical to 0.2.0-beta.3, and GNOME Wayland placement behaves as before. The release restructures the code so other platforms can be added, and adds an experimental, off-by-default Sway backend. See
[docs/RELEASE_NOTES_0.2.0-beta.4.md](docs/RELEASE_NOTES_0.2.0-beta.4.md) for what to expect and what is not verified.

### Changed

- Internal restructuring (no change to commands, flags, config keys or output): the backend is split into narrow capability traits, backend selection moved to a registry, host services (binary directories, autostart path, process detach) and the log-out/update hooks sit behind interfaces, and Linux-only dependencies are only built on Linux. `spawn-at update` now asks for the release asset of its own platform (named after the target triple); on Linux x86-64 this is the same file as before. CI also checks the crate on Windows and macOS targets (those systems are still not supported).

### Added

- **EXPERIMENTAL Sway backend**, opt-in only: `SPAWN_AT_BACKEND=sway` places windows through Sway's IPC socket (`$SWAYSOCK`) with the same anchors, margins, sizes and monitor selection as on GNOME, and supports `query layout`, `query windows`, `transform`, `close` and `focus`. No helper or installation is needed. It is **never selected automatically**: every other environment, including a Sway session without the variable, gets exactly the backend it got before. With the variable set and `$SWAYSOCK` missing, empty or not a Sway socket, spawn-at exits with an error that names `$SWAYSOCK`; it does not fall back to X11. Limits: a window can show for one frame at its tiled position before it is placed (Sway cannot hold a window invisible), `--anchor cursor`, maximize, minimize and defocus are not supported, i3 is not supported. Tested only against a private headless Sway 1.9 with the repo's test window ([docs/DESIGN_SWAY.md](docs/DESIGN_SWAY.md), `scripts/sway-live.sh`); a real Sway session, real apps, XWayland windows and other Sway versions have not been tried.
- `scripts/sway-headless.sh` starts and stops a private headless Sway for tests (own runtime directory, own sockets, no access to your session), and `scripts/sway-live.sh` runs the Sway live cases in it.

## [0.2.0-beta.3] - 2026-10-15

Pre-release. Window placement, the ready-wait policy and the GNOME extension are the same as in
0.2.0-beta.2: no file under `assets/gnome/` and no file under `crates/` changed. This release is
about `spawn-at install`. See
[docs/RELEASE_NOTES_0.2.0-beta.3.md](docs/RELEASE_NOTES_0.2.0-beta.3.md) for how to use it and
what is not verified.

**Already live for everyone:** `install.sh` is fetched from `main`, so the two `install.sh` items
under Fixed (progress bar, visible retries, 404 handling, error layout) work today, before this
tag. **Ships in the binary with this release:** everything else below (the install flow, the
prompts, the log-out handling, `--clean-install`).

### Added

- On GNOME Wayland, `spawn-at install` no longer asks about (or warns about) a log-out when none is needed: the extension about to be deployed is byte-identical to the deployed one (same file set), `gnome-extensions info` reports it ACTIVE, and every deployed file is older than the running gnome-shell process. It prints a green "No log-out needed" line instead. If any of these facts cannot be read, the question and warning stay. Files that are already identical are no longer rewritten, so their modification time keeps meaning "installed before this Shell started". X11 is unchanged.
- `spawn-at install` detects an existing spawn-at binary (user location, `/usr/local/bin`, or on PATH; never the running binary) and tells you where it is and its version (`<existing> -V`, 2 s timeout, failure tolerated) next to the version being installed. In a terminal it asks: keep and upgrade in place (default), or clean install; for the same version the default changes nothing, for a newer installed version it warns about the downgrade and the default changes nothing. The question's lines are short (a long item wraps on an 80-column terminal and the prompt was drawn twice), and a single existing binary is described in one line ("Existing: /path (dev build). Installing: dev build."). Without a terminal, or when the prompt cannot be shown, nothing is uninstalled and the install behaves as before.
- `spawn-at install --clean-install` removes the existing install first (binaries, the extension, the login update-check entry; the same removal as `spawn-at uninstall`) and then installs, without asking, with or without a terminal. Your configuration file and settings are never deleted. With no existing install it prints that there is nothing to remove. `install.sh` passes the flag on (`... | bash -s -- --clean-install`).

### Changed

- The crate now compiles on non-Linux targets (checked for `x86_64-pc-windows-gnu`, `x86_64-apple-darwin` and `aarch64-apple-darwin`). Those systems are not supported yet: running spawn-at there prints "spawn-at does not support this operating system yet" and exits with status 1. Linux behavior is unchanged.
- Every interactive prompt and item now fits on one 70-column line (the longer explanation is printed above the question), and none ends with a colon, so a narrow terminal no longer prints the question twice and "Select update channel::" is gone; the prompts live in `src/prompts.rs` with a test.
- The question "Do you want to log out of your session now?" (or, on X11, "reload GNOME Shell now?") is now the very last prompt of `spawn-at install`, `spawn-at install --clean-install`, `spawn-at uninstall` and the upgrade offered by `spawn-at update`: it is no longer asked in the middle of the GNOME section, before Update Preferences. Answer No and the yellow warning is the last line; answer Yes and the log-out is the last action. Nothing is asked when no log-out is needed, in headless runs (the flags decide), or when `--gnome-wayland-logout` / `--no-action` is given.
- `spawn-at install` and `spawn-at uninstall` output (and the update manager heading): one blank line before and after each `=== ... ===` section, no double blank lines, none at the start or end. The section headings are plain text when stdout is not a terminal or `NO_COLOR` is set.
- The Wayland line "If the extension is not immediately active, log out and back in to complete loading." is printed in bold yellow, only on a terminal with `NO_COLOR` unset; the text is unchanged and it stays plain when piped.

### Fixed

- A log-out (or X11 Shell reload) is now always the last action of a run, in `spawn-at install`, `spawn-at uninstall` and the upgrade offered by `spawn-at update`: the question (or flag) only records the decision, and the advice lines ("No log-out needed", or the yellow warning) are the last thing printed, then the session action is performed. Before, answering Yes logged you out before Update Preferences and the settings were saved. The removal phase of `--clean-install` no longer prints a "logout skipped" line; there is one decision, at the end.
- `install.sh` shows curl's progress bar for the tarball download when stderr is a terminal, announces each retry of a stalled or failed download ("attempt 1 of 4 timed out or stalled, retrying in 2s..."), and prints the elapsed time when a download took longer than 5 seconds. The bar is not drawn for a failed HTTP status (a pinned version that does not exist no longer shows 100% before the error). Without a terminal (logs, CI, redirects) there is no bar and no retry text.
- `install.sh` in a terminal first asks for the first byte of the tarball (one extra small request, 2 attempts of at most 15 s) so that a definite HTTP error (404, 403, a persistent 5xx) is reported with no progress bar and no curl line; a timeout or DNS failure there is ignored and the real download reports it with its own retries. Without a terminal nothing changes (already live, see above).
- `install.sh` errors share one layout on stderr (blank line, red headline, indented causes, "What you can do:" with indented steps, blank line), including the unsupported-platform, `--version` and invalid-tag errors, which now also say what to do; the phases (resolving, downloading, extracting, launching) are separated by single blank lines. Wording and exit codes are otherwise unchanged.

## [0.2.0-beta.2] - 2026-10-14

Pre-release. Placement behavior, the ready-wait policy and the extension are the same as in
0.2.0-beta.1: no file under `assets/gnome/` changed. See
[docs/RELEASE_NOTES_0.2.0-beta.2.md](docs/RELEASE_NOTES_0.2.0-beta.2.md) for upgrade notes and
what is not verified.

### Changed

- **The login update-check autostart entry now honours `XDG_CONFIG_HOME`**, the same way the
  config file does: with an absolute `XDG_CONFIG_HOME` it is written to
  `$XDG_CONFIG_HOME/autostart/spawn-at-update-check.desktop`; with it unset the path is
  unchanged (`~/.config/autostart/...`). Before, it was always under `~/.config`, so a desktop
  session using `XDG_CONFIG_HOME` did not see it. An entry an earlier version wrote under
  `~/.config/autostart` is not moved: re-run `spawn-at install` to write the new one.
- The README no longer calls the X11 driver "finished": the intro now says what the rest of the
  README already said (X11 and XWayland keep the old 30 ms rule, can flash, and were tried with
  the test window only), and the long X11/XWayland paragraph is a short summary that links to
  [docs/TEST_PLAN.md](docs/TEST_PLAN.md) and [docs/DESIGN_READY_WAIT.md](docs/DESIGN_READY_WAIT.md).

### Fixed

- **`install.sh` resolves releases reliably.** "Could not resolve latest release via GitHub API"
  appeared intermittently because every API call hid its error and had no retry. Now: API calls
  use `--connect-timeout 10 --max-time 60 --retry 3 --retry-delay 2` and send
  `Authorization: Bearer $GITHUB_TOKEN` when it is set; if the API fails, `--prerelease` reads the
  first entry of `releases.atom` and the stable channel follows the redirect of `releases/latest`;
  the tarball always comes from `releases/download/<tag>/<asset>`, never the API; a pinned tag
  without a leading `v` is retried with one; tags are validated before they reach a URL; the
  tarball is extracted in a temporary directory that is removed afterwards. Failures print the
  cause (HTTP status, rate limit, timeout, no network) and what to do. The two `python3 -c`
  fallbacks were removed (`jq` is still used when present, otherwise `grep` and `sed`).
  `SPAWN_AT_RESOLVE_ONLY=1` prints the resolved tag and exits. All flags are unchanged. This
  landed on `main` after the beta.1 tag, so it is part of a tagged release for the first time here.
- `install.sh` sends the unsupported-platform error and the "package does not contain
  `spawn-at`" error to stderr.
- The release workflow now fails if a file it packages (`README.md`, `LICENSE`) is missing,
  instead of silently publishing a tarball without it, and checks that the tarball lists
  `./spawn-at`. It had not run on a real tag when this was written.

### Added

- **CI** (`.github/workflows/ci.yml`) on pull requests and pushes to `main`: `cargo fmt --check`,
  clippy with `-D warnings` on the whole workspace, the tests of `spawn-at`, `spawn-at-core`,
  `xtask` and `spawn-at-verify` (`--features tui,fixture`), and the Node tests. Linux x86_64
  only; the 4 tests that need a live display stay ignored. It had not run on GitHub when this
  was written.
- The `hostile` category (9 specs) is part of the `full` and `release-gate` verification suites.
  All 9 are `required = false`, so the gate's required set is unchanged.

### Internal (no user-visible change)

- Plain `cargo build` / `cargo test` at the repository root are explicitly limited to the
  `spawn-at` package (`default-members = ["."]`); this was already their behavior. Name
  `spawn-at-verify` explicitly (`-p spawn-at-verify --features tui,fixture`).
- `scripts/smoke-live.sh` no longer fails on its build step (`no bin target named
  spawn-at-verify`).
- Removed the unused `tokio` dependency of `spawn-at-verify`, the unused `get_active_driver`,
  `EMBEDDED_EXTENSION_FILES` and the re-exports `apply_focus_policy`, `run_unminimize`, `Atoms`
  and `Entry`, the never-constructed `ExpectedState::{Mapped, Position, Size}`, the never-read
  `GitHubRelease::name` field, and ten unreferenced `pub` helpers of `spawn-at-verify`.
  Test-only helpers are now `#[cfg(test)]` and the `spawn-at` modules are private, so the compiler
  reports dead code; six remaining dead-code warnings carry an explained `#[allow(dead_code)]`.
  `spawn-at --help` and every subcommand's help are byte-identical to before.
- Comments the code contradicted were corrected (`src/main.rs`, `lifecycle.rs`, the verify trace
  parser, one node test, `verify/hostile/README.md`).

### Documentation

- Broken paths fixed in `CONTRIBUTING.md`; its "adding a driver" guide now points at the real
  traits. `docs/CODE_LAYOUT.md` now describes the current tree (the never-built reorganization
  plan was removed), `AI_DEV/` was removed, and `CLAUDE.md` lists the real build and test commands.
- `docs/TEST_PLAN.md`: the 23 catalog rows without a spec file are marked "Not written", and the
  dated log (sections 6.10 to 6.35) moved to `docs/TEST_LOG.md` with a stub at each old heading.
  Docs now say that cited `scratch/` and `verify-results/` files are not in the repository.

## [0.2.0-beta.1] - 2026-10-14

Pre-release. Tested on one machine only: GNOME 46, Wayland, one monitor, with the project's own
test window. See [docs/RELEASE_NOTES_0.2.0-beta.1.md](docs/RELEASE_NOTES_0.2.0-beta.1.md) for the
caveats and [docs/TEST_PLAN.md](docs/TEST_PLAN.md) (sections 6.22 to 6.24) for the measurements.

### Added

- **Ready-wait (GNOME 46, Wayland).** The extension keeps a resized window hidden until the app
  has answered the size request or a cap runs out, instead of revealing it after a 30 ms quiet
  period. Hard limit 1500 ms. In our measurements under heavy CPU load (22 busy threads) the
  old rule flashed in 26 of 40 runs and the ready-wait in 0 of 40, by Wayland wire time; idle,
  neither flashed in 20 runs. Wire time is an optimistic bound and no screen capture was done.
- **`spawn-at config`** with the commands `show`, `get`, `set`, `unset`, `reset` and `path`, and
  these settings (see [docs/CONFIG.md](docs/CONFIG.md)):
  - `wait.cap_ms` (1 to 1500): the longest the extension waits for an app to answer a size request.
  - `wait.strategy` (`auto`, `legacy`, `ready-wait`): which rule waits; `legacy` is the pre-0.2 rule.
  - `spawn.window_timeout_ms` (200 to 60000, default 2000): how long `spawn` waits for the window to appear.

  A command-line flag beats the file, and the file beats the default. `SPAWN_AT_CONFIG=<path>`
  names a different file.
- **Placement model.** `--area workarea|screen|unbounded`, `--offset/-o` (an alias of `--pos/-p`),
  `--pivot`, `--clamp`, per-side margins and `--monitor`. `--area unbounded` uses global desktop
  coordinates and rejects margins, clamp, `--monitor` and every anchor except `cursor`. `query windows`
  reports `GEOMETRY` in global screen coordinates; `query layout --json` has `schema_version` 1
  (monitors, work areas, insets). See the README "Coordinate Model".
- **New commands and flags:** `close` (graceful close of a window), the `--id` selector (numeric
  window id from `query windows`), `spawn --json` (one JSON object on stdout; diagnostics stay on
  stderr), and `install --yes` / `--dry-run`.
- **D-Bus protocol 6 (modern extension, GNOME 45 to 48) and 5 (legacy extension, GNOME 42 to 44).**
  New over the old extension: the `ProtocolVersion` property, `DisarmSpawn`, `GetLayout`,
  `CloseWindow`, and the `SpawnClaimed` and `ResizeOutcome` signals (`ResizeOutcome` is protocol 6
  only). The older methods are still there. The CLI warns when the loaded extension reports a protocol
  older than 3.
- **A legacy extension tree for GNOME 42 to 44** (`assets/gnome/legacy`), installed on those versions;
  the modern tree is installed on 45 and newer. The legacy tree has never been run on a real GNOME 42
  to 44.
- **`XDG_CONFIG_HOME` is honoured** for the config file (`$XDG_CONFIG_HOME/spawn-at/config.toml`, else
  `~/.config/spawn-at/config.toml`); before it was always `~/.config/spawn-at/config.toml`.
- **Installer notice on untested GNOME versions.** On a GNOME version other than 46 the installer
  prints what is and is not tested and asks for confirmation (`--yes` skips the question; headless
  runs print the notice and continue).

### Changed

- **Default `wait.cap_ms` is 250 ms** (it was 150 ms in development builds). The extension's own
  default applies when nothing is set; an explicit value is sent with the spawn.
- Under load the window is hidden longer with the ready-wait. The median was 66.8 ms against 45.3 ms
  for the old rule in one measurement and 56.2 against 48.9 ms in a second, so it varies between
  runs (95th percentile 105 against 53 ms, then 116 against 71 ms). Idle there is no difference
  (about 20 ms). In the second measurement 1 of 40 idle runs with the new rule hit the 250 ms cap
  (the app answered late); the cause is unknown.
- An app that is already at its minimum size, or that never answers, makes the ready-wait run
  for the full cap on every spawn (about +230 ms in our test with the cap at 250 ms).
- The four crates are versioned 0.2.0-beta.1. The version the binary reports and the update
  check use comes from the git tag at build time, not from `Cargo.toml`.

- **Update check.** Versions are compared as semver, and `beta` is the pre-release channel (`all` is
  a deprecated alias, see Fixed). A build that is not at an exact release tag now reports itself as
  `dev` and the update check says it is up to date; before, it was compared as the last tag.
- `spawn gnome-terminal` without `--class` now looks for a window of class `org.gnome.Terminal`
  (it was `gnome-terminal-server`). An explicit `--class` is unaffected.

### Breaking changes and upgrade notes

Checked against a build of the previous code (`pre-cleanup-beta`, `875aead`) for argument parsing,
`--help`, config loading and the update check only; no real application was launched and the
Shell was not connected to.

- **`-a` no longer means `--anchor`.** The old build accepted `spawn -a center ...`; the new one
  rejects it (`unknown option '-a'` for `spawn`, `unexpected argument '-a'` for `transform`, exit 2).
  Use `--anchor`. The short flag collided with options of the programs being spawned.
- **A word starting with `-` before the command is a usage error.** `spawn-at spawn -x true` used to
  try to run a program called `-x` (exit 1, "Failed to spawn command '-x'"); it now stops with
  `unknown option '-x'; to run a command that starts with '-', put it after --` (exit 2).
  `spawn-at spawn -- -x true` is still passed through.
- **`transform` with only a target** (`--class`, `--id`, ...) and nothing to change now stops with
  `nothing to change: give --size, -p/-o, --anchor, or --pivot with --size` (exit 2). The old build
  went on to the compositor; what it did to a real window was not tested.
- **`update --channel` takes `stable` or `beta`.** `all` still works as a deprecated alias of `beta`
  and prints one line. A config file that says `channel = "all"` is read as `beta`.
- **An old config file still loads.** A file with an `[apps]` table (v0.1 settings) is accepted;
  `spawn-at config` ignores the `apps` key with a warning, and the `[update]` settings are kept.
- **Re-run `spawn-at install` after upgrading** to rewrite the autostart entry (it now holds the
  absolute binary path) and the extension files. On Wayland, log out and back in so GNOME loads the
  new extension; until then the CLI prints a protocol mismatch warning.
- **Development builds.** The old build, when not at an exact tag, offered "an update" from its last
  tag to `v0.1.0` (checked: `pre-cleanup-beta -> v0.1.0`); the new build says `dev` is up to date.
  Release builds (built at the tag) report the tag as their version.
- **Install locations are unchanged:** the binary in `~/.local/bin` (or `/usr/local/bin` for system
  scope), the extension in `~/.local/share/gnome-shell/extensions/spawn-at@harsh.local`, the
  autostart entry `~/.config/autostart/spawn-at-update-check.desktop`.

### Fixed

- **Update channel name.** The update channels are `stable` and `beta`. `all` (the beta channel's name in 0.2.0-alpha) is a deprecated alias that is read as `beta` in the config file, in `update --channel` and in `install --update-channel`; it prints one line saying so. `install` and its prompt write only `stable` or `beta`. Before this fix, `install` could write `all` while the update check treated it as `stable`, so pre-releases were never offered.
- **Login autostart entry.** `~/.config/autostart/spawn-at-update-check.desktop` had
  `Exec=spawn-at`, which is not found at login because `~/.local/bin` is not on `PATH` there. The
  installer now writes the absolute path of the binary it installed, quoted as the Desktop Entry
  specification requires. Run `spawn-at install` again to rewrite an existing file.
- `cargo clippy --workspace --all-targets -- -D warnings` passes (mechanical fixes, no behaviour change).

### Developer notes (not user-facing)

- The extension can log the paint signals of XWayland/X11 windows (`X11_OBS_*` lines, including the
  surface texture size) while logging is switched on. It changes no timing, order or outcome and was
  used to study the X11 question described under Known limitations.
- The test harness reads the client's commit time from the Wayland protocol for its flash check, and
  the test fixture has hostile-app flags and a test-only `--state-colors` option (solid magenta at the
  initial size, green at any other).

### Known limitations

- **XWayland and X11 windows still use the old 30 ms rule and can flash.** We looked for a signal that says an X11 client has painted at its new size and found none that works for every toolkit. Tried and not usable, all measured on the test window only (TEST_PLAN 6.23 to 6.29): the first `damaged` event, `first-frame`, the actor size, the buffer rect, the surface texture size, holding the cloak and waiting for a second `damaged`, resizing before the map, and a window ping (no reply API). A pixel probe could tell a blank buffer from a painted one, but it crashed GNOME Shell in testing and was removed. Then the X sync counter (6.31 to 6.33), tested with our own GTK4 client: a GTK4-style client's extended counter steps a few milliseconds after the client's final paint in 100 of 100 runs, so it is usable as an experimental signal for GTK4-style clients only. Chromium and Electron use the basic counter, which never moved, so it likely gives nothing for them. In an earlier survey 6 of 9 measured windows advertised some sync counter and about 3 of 9 (the GTK4 ones) would benefit. Sending the client a `_NET_WM_SYNC_REQUEST` ourselves (6.34) was negative at the timing tried, and the extension that causes the resize cannot send X messages. Last, an `LD_PRELOAD` shim that rewrites the geometry of an app's top-level windows (6.35) made the test GTK4 client's windows map at the target size in 80 of 80 runs, and under load it flashed 0 of 40 against 36 of 40 without it. That is one toolkit (GTK4 through Xlib) on one machine: it does not cover xcb-based clients, it did not exempt override-redirect windows, popups were not tested, the reveal came later, 10 of 40 idle runs had a 30 to 44 ms stall whose cause is unverified, and injecting a library into other programs has security and compatibility costs. No screen-capture measurement of what is visible has been made (the Mutter screencast stream could not be consumed; the portal needs a user prompt). **None of this ships in 0.2.0-beta.1:** XWayland and X11 windows keep the old 30 ms rule and can still flash. See [docs/DESIGN_READY_WAIT.md](docs/DESIGN_READY_WAIT.md), sections 12 and 15.
- On GNOME 46 the extension cannot read an app's reported minimum size (it reads 0x0), so it learns
  a minimum only by waiting.
- Untested: real X11 and XWayland applications, GNOME 42 to 45 and 47 to 48, Qt and Electron apps,
  multi-monitor and fractional scaling, and any screen-capture check of what is visible.

[Unreleased]: https://github.com/harshp2008/spawn-at/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/harshp2008/spawn-at/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/harshp2008/spawn-at/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/harshp2008/spawn-at/compare/v0.2.0-beta.6...v0.2.0
[0.2.0-beta.6]: https://github.com/harshp2008/spawn-at/compare/v0.2.0-beta.5...v0.2.0-beta.6
[0.2.0-beta.5]: https://github.com/harshp2008/spawn-at/compare/v0.2.0-beta.4...v0.2.0-beta.5
[0.2.0-beta.4]: https://github.com/harshp2008/spawn-at/compare/v0.2.0-beta.3...v0.2.0-beta.4
[0.2.0-beta.3]: https://github.com/harshp2008/spawn-at/compare/v0.2.0-beta.2...v0.2.0-beta.3
[0.2.0-beta.2]: https://github.com/harshp2008/spawn-at/compare/v0.2.0-beta.1...v0.2.0-beta.2
[0.2.0-beta.1]: https://github.com/harshp2008/spawn-at/releases/tag/v0.2.0-beta.1
