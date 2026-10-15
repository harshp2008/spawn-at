# spawn-at knowledge base

**How to use this file:** read the table of contents, then only the section you need. Each entry is one short fact plus the file and heading it came from; the detail lives there, not here. Add an entry when something took more than a couple of tool calls to find (rules in `CLAUDE.md`, "Knowledge base"). Entries were added on 2026-10-15 from existing repo docs, not re-measured; "unverified" marks claims their source does not prove. Line numbers in sources may have moved.

**Last reorganised:** 2026-10-15

## Table of contents

1. [Architecture map](#architecture-map)
2. [Install and release](#install-and-release)
3. [GNOME and Wayland behavior](#gnome-and-wayland-behavior)
4. [Testing and fixtures](#testing-and-fixtures)
5. [Gotchas](#gotchas)
6. [Dead ends](#dead-ends)
7. [Open questions](#open-questions)

## Architecture map

- The neutral command layer only talks to `CompositorBackend`; `gnome::mechanics` is used from production code only inside the GNOME driver. Source: `docs/DESIGN_PLATFORMS.md`, "8. Findings worth remembering". Details: its "4. Proposed capability model".
- Backend choice: forced `SPAWN_AT_BACKEND=x11` or `sway`, else GNOME if `XDG_CURRENT_DESKTOP` has GNOME and D-Bus connects, else X11. Non-GNOME Wayland (or unknown session type) without a forced backend is an error since the unsupported-desktop check (`registry::unsupported`); a real X11 session on any desktop still gets X11; `install.sh` asks first and sets `SPAWN_AT_BACKEND=x11` on the installer child. Source: `docs/DESIGN_PLATFORMS.md`, "3. Where platform assumptions leak" row 11; `CHANGELOG.md`, "[Unreleased] Changed"; `docs/DESIGN_SWAY.md`, "7. Selection: never automatic".
- The login autostart entry runs `<binary> update --check --headless`, which needs no backend (`main.rs` returns from `Commands::Update` before `init_backend()`; `update.rs` `run_update` calls no registry code), so it works on an unsupported desktop and its content does not depend on the desktop or on `SPAWN_AT_BACKEND`. Evidence: `src/config.rs` `autostart_entry_content`, tests `autostart_file_tests`, `tests/unsupported_desktop_cli_test.rs`. Source: `docs/DESIGN_PLATFORMS.md`, "3. Where platform assumptions leak" row 5; recorded 2026-10-15.
- Sway is never selected automatically; only the forced value `sway` selects it. Source: `docs/DESIGN_SWAY.md`, "7. Selection: never automatic"; `docs/TEST_PLAN.md`, "7.1 Unit and integration tests".
- Sway backend files (`src/platform/linux/sway/`: `ipc`, `types`, `command`, `matching`, `client`, `place`, `mod`) and their roles: `docs/DESIGN_SWAY.md`, "12. Implementation status". It uses `std` sockets plus the `serde_json` already present, no new dependency ("8. IPC: hand-roll, no new dependency").
- The Sway module exists only on Linux, so the non-Linux `cargo check` runs and `check-other-os` do not compile it. Source: `docs/DESIGN_SWAY.md`, "12. Implementation status".
- Ready-wait exists twice, in Rust (`crates/spawn-at-core/src/ready_wait.rs`) and JS (`assets/gnome/modern/ready_wait.js`), tied by shared vectors in `tests/ready_wait_vectors`. Source: `docs/DESIGN_PLATFORMS.md`, "8. Findings worth remembering".
- Extension protocol numbers (4, 5) are known only to `HelperNeed::min_protocol` in `capability.rs` and the GNOME driver's own constants. Source: `docs/DESIGN_PLATFORMS.md`, "8. Findings worth remembering".
- `/proc` process ancestry lives once, in `linux/process_tree.rs` (it existed twice before step 2). Source: `docs/DESIGN_PLATFORMS.md`, "8. Findings worth remembering".
- `spawn-at-verify` depends only on `spawn-at-core` and treats the binary as a black box, but its display and log probing is GNOME/Linux tooling. Source: `docs/DESIGN_PLATFORMS.md`, "8. Findings worth remembering".
- Code layout overview: `docs/CODE_LAYOUT.md`. Per-target feasibility: `docs/DESIGN_PLATFORMS.md`, "5. Feasibility per target".

## Install and release

- The version comes from the git tag at build time (`build.rs`, `git describe --tags --exact-match`). On an exact tag the binary prints `spawn-at <version>` with no commit id; otherwise `DEV BUILD` with `Previous Commit ID`. Source: `CLAUDE.md`, "Versions and releases"; `CHANGELOG.md`, "[Unreleased] Fixed" (preflight).
- `install.sh` is fetched from `main`, so a merge ships it at once; binary changes ship with the next tag. Source: `CHANGELOG.md`, "[0.2.0-beta.3]" intro.
- A tag containing `-` makes `release.yml` publish a pre-release. Source: `CLAUDE.md`, "Versions and releases".
- `spawn-at update` picks the release asset named for its own target triple (`SPAWN_AT_TARGET`). Source: `CHANGELOG.md`, "[0.2.0-beta.4] Changed"; `build.rs`.
- Update channel `all` is a deprecated alias of `beta`. Source: `docs/TEST_PLAN.md`, "Re-measured 2026-10-14 after the update-channel fix".
- `spawn-at install` skips the log-out question when the deployed extension is byte-identical, active, and older than the running shell. The log-out is always the last action. Source: `CHANGELOG.md`, "[0.2.0-beta.3] Added / Fixed".
- Beta.4 changed no file under `assets/gnome/` or `install.sh` relative to beta.3. Source: `CHANGELOG.md`, "[0.2.0-beta.4]" intro; `docs/RELEASE_NOTES_0.2.0-beta.4.md`.

## GNOME and Wayland behavior

- Wayland has no generic "move another client's window" protocol; every compositor needs its own IPC or script (public knowledge, unverified here). Source: `docs/DESIGN_PLATFORMS.md`, "8. Findings worth remembering".
- The GNOME extension exposes only a protocol number over D-Bus, no content hash, so "is the running extension current" needs file facts (`logout.rs`). Source: `docs/DESIGN_PLATFORMS.md`, "8. Findings worth remembering".
- Measured delayed-paint, cloak budget and min-size behavior of GNOME 46: `docs/TEST_PLAN.md`, "5. Delayed-Paint Calibration & Cloak Budget Analysis" and "6. Min-size contract"; raw logs in `docs/TEST_LOG.md`.
- Sway (1.9, headless) facts: IPC frame is 14 bytes; real JSON shapes are recorded. Source: `docs/DESIGN_SWAY.md`, "2. Verified facts about Sway" and "Real JSON shapes".
- Sway placement: `--size` is the whole container (as on GNOME), corrected once after `resize set` for border/title bar, and a window raised to its minimum is re-anchored with its real size. Source: `docs/DESIGN_SWAY.md`, "12. Implementation status"; `docs/TEST_PLAN.md`, "7.1".
- Sway `state` in `query windows` is `normal` or `fullscreen`; scratchpad windows list as `minimized`. Source: `docs/DESIGN_SWAY.md`, "12. Implementation status".
- Sway cannot read the pointer, so `--anchor cursor` and `--monitor cursor` use (0, 0); maximize, minimize, restore and defocus are unsupported. Source: `docs/DESIGN_SWAY.md`, "12. Implementation status".
- The Sway one-frame flash before placement is accepted but not measured. Source: `docs/DESIGN_SWAY.md`, "5. Flicker-free strategy" and "12. Implementation status".

## Testing and fixtures

- Run the proof suite per binary, not a bare workspace number: `cargo test -p spawn-at-verify --features tui,fixture` (4 tests stay ignored, they need a live display). Counts and the table: `docs/TEST_PLAN.md`, "1.1" rows. Source: `CLAUDE.md`, "Tests".
- Refactor proof set: sorted test names per binary, `--help` of every subcommand, sha256 of goldens and vectors, the 48-case fake-curl installer matrix. Source: `docs/REFACTOR_PROOFS.md`, "What is captured" and "Commands". If `compare.sh` does not print PASS, the step is not behavior-preserving.
- A split is a pure move if a normalised `comm -23` / `comm -13` of old and new lines shows only re-wrapped lines, imports, `mod` lines and new headers. Source: `docs/REFACTOR_PROOFS.md`, "Proving a split is a pure move".
- Live Sway tests run only in a private headless Sway via `scripts/sway-headless.sh`; `scripts/sway-live.sh` had 62 checks, 0 failed, on 2026-10-15 (Sway 1.9). Source: `docs/TEST_PLAN.md`, "7.2"; `docs/DESIGN_SWAY.md`, "11. The harness".
- The Sway golden byte frames and JSON fixtures were recorded from a real Sway 1.9 (`tests/fixtures/sway/`). Source: `docs/TEST_PLAN.md`, "7.1".
- The live GNOME suites and the 4 ignored tests are not covered by the refactor proofs and must be run by the owner before a release containing a refactor. Source: `docs/REFACTOR_PROOFS.md`, "What these proofs do not cover".
- What the Sway tests do not cover (real Sway, XWayland, other versions, i3, scales other than 1 and 2, more than two outputs): `docs/TEST_PLAN.md`, "7.3 Not verified".
- Which commands are tested, at which tier, and how flicker is (and is not) measured: `docs/TEST_COVERAGE_AUDIT.md` (audit of 2026-10-15; headline: `defocus`, `restore` and `minimize` have no behavior test, the verify catalog only has `spawn` specs, flash is measured only for GNOME `spawn`). Source: that file, sections 3, 4 and 7.
- Batch 1 (2026-10-15) added the command-handler, GNOME D-Bus stub, extension, Sway-binary and suite-wiring tests; two things took long to find: `spawn --focus/--no-focus/--defocus` are parsed but never read (`run_spawn_with` always uses `FocusIntent::Exclusive`, pinned by `test_spawn_focus_modifiers_have_no_effect_today`), and a D-Bus stub on the same zbus connection as the client hangs unless `ProtocolVersion` is read before the first method call (`gnome/tests_control.rs`, `driver_for`). Source: `docs/TEST_COVERAGE_AUDIT.md`, section 7 "Update" notes.

## Gotchas

- The binary embeds its commit (or tag); after every commit `cargo build -p spawn-at` must run before a verify batch, or preflight blocks it as stale. Source: `docs/TEST_LOG.md`, "6.23 Post-reinstall live check" (Process finding); `verify/README.md`, "4. Preflight Gate & Binary Freshness".
- Plain `cargo build` / `cargo test` at the root cover only the `spawn-at` package; name the other crates. Source: `CLAUDE.md`, "Build and test commands".
- `run_spawn_with` is one function of about 524 lines at the time of writing (`spawn.rs`); splitting it was a prerequisite for a second ready-signal model. Source: `docs/DESIGN_PLATFORMS.md`, "8. Findings worth remembering".
- Only two `supports_*` capability flags are read in production (`transform.rs`, `lifecycle.rs`). Source: `docs/DESIGN_PLATFORMS.md`, "8. Findings worth remembering".
- Sway: `--no-wait` still waits for the placement, because the pipeline awaits `post_spawn` in every mode; `post_spawn` waits a fixed 5 s. Source: `docs/DESIGN_SWAY.md`, "12. Implementation status".
- Sway: only the first entry of a batch is placed (the X11 driver does the same). Source: `docs/DESIGN_SWAY.md`, "12. Implementation status".
- `escalate.rs` has macOS and Windows branches that have never been compiled. Source: `docs/DESIGN_PLATFORMS.md`, "8. Findings worth remembering".
- `gh pr edit` fails here with a Projects (classic) deprecation error; use `gh api -X PATCH`. Source: `CLAUDE.md`, "Git and PRs".

## Dead ends

- Release binary size as a refactor gate: moving code changes codegen units and inlining, so size moves by bytes to kilobytes with no behavior change (step 2: +21,792 bytes in `.text`; one comment line: 264 bytes). Kept informational only. Source: `docs/REFACTOR_PROOFS.md`, "The release binary size is not a gate".
- Reading "the crate cannot compile off Linux" from code alone was history, later fixed by step 1 and checked with `cargo check` for three targets. Source: `docs/DESIGN_PLATFORMS.md`, "8. Findings worth remembering"; `CHANGELOG.md`, "[0.2.0-beta.3] Changed".

## Open questions

- Is a one-frame flash on Sway visible on a real session? Not measured. Source: `docs/DESIGN_SWAY.md`, "12. Implementation status".
- Real Sway sessions, XWayland, other Sway versions, i3, Windows and macOS runs: all unverified. Source: `docs/TEST_PLAN.md`, "7.3 Not verified"; `docs/RELEASE_NOTES_0.2.0-beta.4.md`.
- Which platforms and compositors to support next, and the risks of each: `docs/DESIGN_PLATFORMS.md`, "7. Risks and open questions".
- Owner decisions already taken for Sway (flash acceptable, never auto-select, no maximize/minimize, no i3): `docs/DESIGN_SWAY.md`, "10. Open questions for the owner".
