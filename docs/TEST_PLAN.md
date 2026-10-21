# spawn-at Test Library & Verification Plan

Status: reference. Section 1 is the overview and the counts per binary. Sections 2 and 3 are the catalog and the CLI coverage, section 4 the animation cases, sections 5 and 6 dated investigations, and section 7 the Sway backend. The totals of the counts are on the [Testing](https://harshp2008.github.io/spawn-at/testing/#test-counts) page of the guide, not here.

Sections 6.10 to 6.35 (the dated experiment log) are in [TEST_LOG.md](TEST_LOG.md). Every old heading stays below as a stub that links to its place, so "TEST_PLAN 6.x" citations keep working.

Paths under `scratch/` or `verify-results/` cited in this plan and in TEST_LOG.md are gitignored working files of the author's machine and are not in the repository. The numbers in the text are what is kept.

## 1. Overview & Architecture

`spawn-at-verify` is the test harness for spawn-at. It checks window placement against a mathematical oracle, and the order of the cloak's events against the extension's log. A spec is a TOML file under `verify/<category>/`. A CLI runner and a terminal UI run the specs, and most of them need a live GNOME Wayland session. How to run each kind of test is on the [Testing](https://harshp2008.github.io/spawn-at/testing/) page of the guide.

### 1.1 How tests are counted

The totals are in the table on [Testing](https://harshp2008.github.io/spawn-at/testing/#test-counts), with the command that prints each one. The tables of 1.2 break them down.

- Rust: `cargo test --locked -p <crate> [--features ...] -- --list` lists the tests of each test binary without running them. The ignored ones come from the same command with `--ignored`. A count is the listed tests minus the ignored ones. `spawn-at-verify` is counted three times, because its features change the set of tests.
- Node: `node --test` on each `tests/*_test.js` file except `install_unsupported_test.js`, which runs `install.sh` against a stand-in `curl` and runs only in CI. `tests/gi_loader.js` is a helper that `node --test tests/*.js` counts as one more test.

### 1.2 Counts per binary

The rows of each table add up to the totals on the Testing page.

`spawn-at`:

| Binary | Tests |
| :-- | --: |
| Unit tests (`src/main.rs`) | 478 |
| `tests/config_cli_test.rs` | 10 |
| `tests/install_notice_cli_test.rs` | 6 |
| `tests/no_tty_cli_test.rs` | 1 |
| `tests/sway_backend_cli_test.rs` | 19 |
| `tests/unsupported_desktop_cli_test.rs` | 3 |
| `tests/version_cli_test.rs` | 1 |

The 478 unit tests by module:

| Module | Tests |
| :-- | --: |
| `platform::linux` | 197 |
| `commands` (`state_command_tests` 31, `tests` 20, `spawn` 22, `query` 7) | 80 |
| `update` | 34 |
| `cli` | 29 |
| `diagnostics` | 26 |
| `platform::upgrade` | 17 |
| `session` | 16 |
| `target` | 15 |
| `config` (`tests` 13, `autostart_file_tests` 3) | 16 |
| `ui` | 12 |
| `platform` (`installer` 8, `capability` 7, `install_notice` 8, `provisioner` 3, `update_hooks` 3, `session_actions` 2, `null_backend` 1) | 32 |
| `prompts` | 4 |

The 197 of `platform::linux` are `sway` 70, `gnome` 96, `registry` 10, `x11` 8, `host` 4, `xdg` 4,
`config_store` 2 and `tests` 3.

`spawn-at-core`:

| Binary | Tests |
| :-- | --: |
| Unit tests (`src/lib.rs`) | 60 |
| `tests/golden_placement.rs` | 2 |
| `tests/ready_wait_vectors.rs` | 2 |

The 60 unit tests are `geometry` 21, `settings` 17, `ready_wait` 9, `wait_override` 9 and
`resize_report` 4. The 2 of `ready_wait_vectors.rs` are one test over the 20 shared golden scripts and
the policy check.

`xtask`:

| Module (`src/main.rs`) | Tests |
| :-- | --: |
| `docs_check` | 32 |
| `docs_mirror` | 23 |
| `docs_cli` | 17 |
| `release` | 13 |
| `docs_html` | 11 |
| `burn_cpu` | 6 |
| `docs_site` | 3 |
| `compiler_docs` | 2 |
| Unit tests | 107 |
| `tests/burn_cpu_test.rs` (runs the built binary) | 2 |

`spawn-at-verify`, by binary and feature set (a dash means the binary is not built with that set):

| Binary | No features | `tui` | `tui,fixture` |
| :-- | --: | --: | --: |
| Unit tests (`src/lib.rs`) | 336 | 342 | 342 |
| Fixture binary (`src/bin/fixture.rs`) | - | - | 22 |
| `src/main.rs` | 0 | 0 | 0 |
| `tests/action_trace_test.rs` | 12 | 12 | 12 |
| `tests/catalog_golden_test.rs` | 1 | 1 | 1 |
| `tests/category_library_test.rs` | 5 | 5 | 5 |
| `tests/cli_exit_codes_test.rs` | 7 | 7 | 7 |
| `tests/fixture_test.rs` | - | - | 4 (and 4 ignored) |
| `tests/flash_test.rs` | 20 | 20 | 20 |
| `tests/oracle_cross_check.rs` | 5 | 5 | 5 |
| `tests/panic_hook_test.rs` | 0 | 1 | 1 |
| `tests/preflight_tools_test.rs` | 1 | 1 | 1 |
| `tests/render_test.rs` | - | 13 | 13 |
| `tests/run_signal_test.rs` | 5 | 5 | 5 |
| `tests/run_sway_guard_test.rs` | 4 | 4 | 4 |
| `tests/suite_wiring_test.rs` | 6 | 6 | 6 |
| `tests/trace_analyser_test.rs` | 21 | 21 | 21 |
| `tests/trace_state_test.rs` | 6 | 6 | 6 |
| `tests/trace_window_scope_test.rs` | 6 | 6 | 6 |
| `tests/tui_test.rs` | - | 145 (and 1 ignored) | 145 (and 1 ignored) |

The 342 unit tests of the library, counted with `tui`, by area: `runner` 135, `oracle` 43, `schema`
41, `report` 21, `trace` 19, `preflight` 13, `cli` 11, `tracker` 10, `backend` 8, `ext_logging` 8,
`loader` 7, `tui` 6, `probe` 5, `display` 4, `loadavg` 4, `log_source` 3, `no_config` 2 and `process` 2.
Without `tui` the library has 6 fewer (the 5 of `tui::app` and the 1 of `tui::logging`).

`suite_wiring_test` checks that every category a suite names has specs or is in a commented
known-empty list, and that every spec is selected by some suite. The unreachable categories `focus`
and `platform` are pinned.

Node, 13 files:

| File | Tests |
| :-- | --: |
| `tests/ready_wait_wiring_test.js` | 52 |
| `tests/records_test.js` (the docs site's records integration, not the extension) | 40 |
| `tests/ready_wait_test.js` | 31 |
| `tests/dispatcher_test.js` | 28 |
| `tests/legacy_dispatcher_test.js` | 22 |
| `tests/x11_observe_test.js` | 21 |
| `tests/window_control_test.js` | 18 |
| `tests/stale_arm_test.js` | 14 |
| `tests/wait_override_test.js` | 9 |
| `tests/extension_test.js` | 8 |
| `tests/cloak_test.js` | 5 |
| `tests/late_resize_test.js` | 5 |
| `tests/golden_anchor_test.js` | 1 |

### 1.3 Live checks and measurements that are still open

| Item | State | Where |
| :-- | :-- | :-- |
| Sway: does a window show at its tiled position before it floats? | Not measured. Nothing records `window::new` events or frames. | [DESIGN_SWAY.md](DESIGN_SWAY.md) section 5; `TEST_COVERAGE_AUDIT.md` P8 |
| `scripts/sway-live.sh`: `focus` and failure paths | Not written. | `TEST_COVERAGE_AUDIT.md` P4 |
| X11 driver against an X server | No test touches one, and there is no `Xvfb` job. Four tests named "supported on X11" pass on a connection error. | `TEST_COVERAGE_AUDIT.md` P11, gap 3 |
| The 22 catalog specs for existing windows, on GNOME | Not run. 15 passed on a private headless Sway and 7 were skipped. | Section 7.5 |
| Any setup other than GNOME 46 on Wayland | GNOME 42 to 45 and 47 or later, real XWayland and X11 apps, real Sway sessions, more than two outputs and scales other than 1 and 2 **have never been run**. | Section 7.3; README "Support" |
| Wiring test for `spawn --focus`, `--no-focus`, `--defocus` | Blocked: nothing reads the flags yet. | `TEST_COVERAGE_AUDIT.md` P10 |
| A cloak budget | No budget is supported by a measurement. The 100 ms warning is provisional. | Section 5 |
| The Shell stall after a window maps | The probe of section 5 was not repeated, so its link to the late-resize failures is a hypothesis. | Section 6.9 |
| Real non-zero minimum size from the Shell; GTK3, Qt and Electron apps | Never probed. Only zenity and the winit fixture were. | Section 6.9 |
| Product decisions of the min-size contract | Open: the anchor and the final size, the "below toolkit minimums" threshold, the exit code, oversized requests, the default margin. | Section 6.7 |

### 1.4 The ignored `fixture_test` tests

Five tests stay ignored. Four are in `crates/spawn-at-verify/tests/fixture_test.rs`, and each opens a test window, so each needs a live display session (a compositor):

| Test | What it does | Needs |
| :-- | :-- | :-- |
| `test_fixture_clean_exit_timeout` | Runs the fixture with `--exit-after-ms 100` and checks that it exits successfully after at least 80 ms | A Wayland compositor or an X11 display |
| `test_fixture_print_ready_output` | Runs the fixture with `--exit-after-ms 200 --print-ready` and checks that stdout has `[fixture] READY protocol=` | A display |
| `test_fixture_signal_termination` | Starts the fixture with `--exit-after-ms 10000`, sends SIGTERM after 150 ms, and checks that it ends cleanly | A display |
| `test_fixture_size_matches_query_windows_on_all_protocols` | Opens a 400x300 fixture over Wayland and over X11, whichever display variable is set, and compares the size that `spawn-at query windows --json` reports for its pid with the requested size | A GNOME session with the spawn-at extension |

The fifth ignored test is in `crates/spawn-at-verify/tests/tui/focus_chain_sway.rs`. It needs a private headless Sway, which `scripts/focus-chain-sway.sh` starts.

---

## 2. 59-Case Test Catalog across 8 Categories

The table lists the 59 planned catalog rows. 47 are Covered: the spec file in the last column exists. 12 are **Not written**: that spec file does not exist. No row was dropped by a decision. The last 18 Covered rows are the placement resize specs of section 6.10.

The spec files under `verify/` make 67 tests (55 auto and 12 both): 60 files, plus 3 more from the matrix of `cloak/delayed-paint` and 4 more from the matrix of `transforms/anchor-corners`. That is more than the 47 Covered rows because specs added later are not rows of the 59, such as the `hostile/` cells of 6.22 and the specs for existing windows (7.5). The others are not matched to rows one by one.

| ID | Title | Mode | Status | Spec / Test Path |
| :--- | :--- | :---: | :---: | :--- |
| *Category: basics* | | | | |
| `preflight/binary-head` | Binary version matches repository HEAD | Auto | Covered | `verify/preflight/binary-head.toml` |
| `basics/version-and-help` | CLI `--help` and `--version` formatted output | Auto | Not written | `verify/basics/version-and-help.toml` (file does not exist) |
| `basics/instant-paint` | Instant paint without artificial delay | Auto | Not written | `verify/basics/instant-paint.toml` (file does not exist) |
| `basics/shell-extension-active` | Extension IPC handshake and version ping | Auto | Not written | `verify/basics/shell-extension-active.toml` (file does not exist) |
| *Category: placement* | | | | |
| `placement/bottom-right` | Window placed in bottom-right corner with margins | Both | Covered | `verify/placement/bottom-right.toml` |
| `placement/top-left` | Window placed at top-left anchor (400x300, margin 16) | Both | Covered | `verify/placement/top-left.toml` |
| `placement/top` | Window placed at top edge (400x300, margin 16) | Auto | Covered | `verify/placement/top.toml` |
| `placement/top-right` | Window placed at top-right anchor (400x300, margin 16) | Both | Covered | `verify/placement/top-right.toml` |
| `placement/left` | Window placed at left edge (400x300, margin 16) | Auto | Covered | `verify/placement/left.toml` |
| `placement/right` | Window placed at right edge (400x300, margin 16) | Auto | Covered | `verify/placement/right.toml` |
| `placement/bottom-left` | Window placed at bottom-left anchor (400x300, margin 16) | Both | Covered | `verify/placement/bottom-left.toml` |
| `placement/bottom` | Window placed at bottom edge (400x300, margin 16) | Auto | Covered | `verify/placement/bottom.toml` |
| `placement/center` | Window centered inside display workarea (400x300, margin 16) | Auto | Covered | `verify/placement/center.toml` |
| `placement/margin-0` | Bottom-right anchor with 0 px margin | Both | Covered | `verify/placement/margin-0.toml` |
| `placement/margin-64` | Bottom-right anchor with 64 px margin | Auto | Covered | `verify/placement/margin-64.toml` |
| `placement/center-200x150` | 200x150 window centered (**flaky live**, see 2.1) | Both | Covered | `verify/placement/center-200x150.toml` |
| `placement/center-1200x800` | 1200x800 window centered (**flaky live**, see 2.1) | Auto | Covered | `verify/placement/center-1200x800.toml` |
| `placement/bottom-right-smaller` | 200x150 at bottom-right, fixture minimum 100x100 (real shrink) | Both | Covered | `verify/placement/bottom-right-smaller.toml` |
| `placement/bottom-right-larger` | 800x600 at bottom-right, fixture minimum 100x100 (real grow) | Auto | Covered | `verify/placement/bottom-right-larger.toml` |
| `placement/bottom-right-minsize-at-min` | exactly the fixture minimum 300x200, bottom-right | Auto | Covered | `verify/placement/bottom-right-minsize-at-min.toml` |
| `placement/bottom-right-below-min-width` | 200x300 vs minimum 300x200 -> final 300x300, bottom-right | Auto | Covered | `verify/placement/bottom-right-below-min-width.toml` |
| `placement/bottom-right-below-min-height` | 400x150 vs minimum 300x200 -> final 400x200, bottom-right | Auto | Covered | `verify/placement/bottom-right-below-min-height.toml` |
| `placement/bottom-right-below-min-both` | 200x150 vs minimum 300x200 -> final 300x200, bottom-right | Both | Covered | `verify/placement/bottom-right-below-min-both.toml` |
| `placement/bottom-right-far-below-min` | 50x50 vs minimum 300x200 -> final 300x200, bottom-right | Auto | Covered | `verify/placement/bottom-right-far-below-min.toml` |
| `placement/bottom-left-smaller` | 200x150 at bottom-left | Auto | Covered | `verify/placement/bottom-left-smaller.toml` |
| `placement/bottom-left-below-min-both` | 200x150 vs minimum 300x200 at bottom-left | Auto | Covered | `verify/placement/bottom-left-below-min-both.toml` |
| `placement/top-right-smaller` | 200x150 at top-right | Auto | Covered | `verify/placement/top-right-smaller.toml` |
| `placement/top-right-below-min-both` | 200x150 vs minimum 300x200 at top-right | Auto | Covered | `verify/placement/top-right-below-min-both.toml` |
| `placement/bottom-smaller` | 200x150 at the bottom edge | Auto | Covered | `verify/placement/bottom-smaller.toml` |
| `placement/bottom-below-min-both` | 200x150 vs minimum 300x200 at the bottom edge | Auto | Covered | `verify/placement/bottom-below-min-both.toml` |
| `placement/right-smaller` | 200x150 at the right edge | Auto | Covered | `verify/placement/right-smaller.toml` |
| `placement/right-below-min-both` | 200x150 vs minimum 300x200 at the right edge | Auto | Covered | `verify/placement/right-below-min-both.toml` |
| `placement/top-left-below-min-both` | control: 200x150 vs minimum 300x200 at top-left (position independent of size) | Auto | Covered | `verify/placement/top-left-below-min-both.toml` |
| `placement/top-left-smaller` | 200x150 at top-left, fixture minimum 100x100 (real shrink; the window's origin does not move) | Auto | Covered | `verify/placement/top-left-smaller.toml` |
| `placement/invalid-size-zero` | expected failure: `--size 0 150` is rejected by the CLI (exit 2, "Invalid size") | Auto | Covered | `verify/placement/invalid-size-zero.toml` |
| `placement/area-screen-vs-workarea` | Boundary bounds: screen vs workarea vs unbounded | Auto | Not written | `verify/placement/area-screen-vs-workarea.toml` (file does not exist) |
| `placement/clamp-bounds` | Clamping window when position exceeds area | Auto | Not written | `verify/placement/clamp-bounds.toml` (file does not exist) |
| `placement/explicit-pos` | Explicit coordinates `--pos X Y` with custom pivot | Auto | Not written | `verify/placement/explicit-pos.toml` (file does not exist) |
| *Category: transforms* | | | | |
| `transforms/reposition-by-id` | Reposition existing window by numeric ID | Auto | Covered (GNOME; attended) | `verify/transforms/reposition-by-id.toml` |
| `transforms/maximize-and-restore` | Maximize window and restore original dimensions | Both | Covered (GNOME; attended) | `verify/transforms/maximize-and-restore.toml` |
| `transforms/minimize-and-restore` | Minimize window and restore it (the planned `transforms/minimize-and-focus`) | Both | Covered (GNOME; attended) | `verify/transforms/minimize-and-restore.toml` |
| `transforms/resize-by-id` | Resize existing window dimensions | Auto | Covered (GNOME attended; headless Sway unattended) | `verify/transforms/resize-by-id.toml` |
| *Category: focus* | | | | |
| `focus/activate-fixture` | Focus one of two fixture windows by id (the planned `focus/activate-by-title`) | Auto | Covered (GNOME attended; headless Sway unattended) | `verify/focus/activate-fixture.toml` |
| `focus/defocus-active-window` | Relinquish keyboard focus to another window | Auto | Covered (GNOME; attended) | `verify/focus/defocus-active-window.toml` |
| `focus/passive-spawn-no-focus` | Spawn window passively without stealing focus (`--no-focus`) | Auto | Not written | `verify/focus/passive-spawn-no-focus.toml` (file does not exist) |
| `focus/switch-between-windows` | Alternate focus across two managed test windows | Auto | Covered (GNOME attended; headless Sway unattended) | `verify/focus/switch-between-windows.toml` |
| *Category: animation-and-cloak* | | | | |
| `cloak/delayed-paint/delay-0` | App paints slowly (0 ms delay) | Auto | Covered | `verify/cloak/delayed-paint.toml` (one spec, `[matrix] delay = [0, 50, 150, 500]`; this row is its `delay = 0` expansion) |
| `cloak/delayed-paint/delay-50` | App paints slowly (50 ms delay) | Auto | Covered | `verify/cloak/delayed-paint.toml` (one spec, `[matrix] delay = [0, 50, 150, 500]`; this row is its `delay = 50` expansion) |
| `cloak/delayed-paint/delay-150` | App paints slowly (150 ms delay) | Auto | Covered | `verify/cloak/delayed-paint.toml` (one spec, `[matrix] delay = [0, 50, 150, 500]`; this row is its `delay = 150` expansion) |
| `cloak/delayed-paint/delay-500` | App paints slowly (500 ms delay) | Auto | Covered | `verify/cloak/delayed-paint.toml` (one spec, `[matrix] delay = [0, 50, 150, 500]`; this row is its `delay = 500` expansion) |
| `animation/minimize-animation` | Minimize/unminimize animation flicker regression | Both | Not written | `verify/animation/minimize-animation.toml` (file does not exist) |
| *Category: queries* | | | | |
| `queries/query-layout-json` | Query display monitor and workarea layout | Auto | Covered (GNOME; headless Sway unattended) | `verify/queries/query-layout-json.toml` |
| `queries/query-windows-lists-fixture` | Query compositor windows; a fixture window is listed | Auto | Covered (GNOME attended; headless Sway unattended) | `verify/queries/query-windows-lists-fixture.toml` |
| `queries/query-pointer` | Query current mouse pointer screen coordinates | Auto | Covered (GNOME only; Sway contract in `robustness/sway-query-pointer-unsupported`) | `verify/queries/query-pointer.toml` |
| *Category: robustness* | | | | |
| `robustness/invalid-anchor-arg` | Clean error report on invalid anchor CLI value | Auto | Not written | `verify/robustness/invalid-anchor-arg.toml` (file does not exist) |
| `robustness/app-crash-early-exit` | Graceful cleanup when spawned process crashes during cloak | Auto | Not written | `verify/robustness/app-crash-early-exit.toml` (file does not exist) |
| `robustness/nonexistent-window-id` | Transform non-existent window ID returns clean error | Auto | Covered (GNOME; headless Sway unattended) | `verify/robustness/nonexistent-window-id.toml` |
| *Category: platform* | | | | |
| `platform/x11-fallback` | Platform guard and fallback under X11 session | Auto | Not written | `verify/platform/x11-fallback.toml` (file does not exist) |
| `platform/multi-monitor-secondary` | Target placement to secondary monitor index | Both | Not written | `verify/platform/multi-monitor-secondary.toml` (file does not exist) |

---

### 2.1 Known flaky: placement size variants (investigated 2026-10-12)

`placement/center-200x150` and `placement/center-1200x800` are the first specs that force a real resize. The fixture opens at 400x300 and neither spec passes `--width` or `--height`, so the window is resized after it maps. The two specs stay required.

The mechanism and the corrected frequencies are in 6.0, 6.5 and 6.6. The results after the late-resize fix are in 6.11 (a stub that links to [TEST_LOG.md](TEST_LOG.md)).

What was found, and how it is known:

- Symptom (confirmed): on a failing run spawn-at warns that the window mapped at (760, 410), the position that centres a 400x300 window in the work area, although it ends up 200x150 or 1200x800. The window keeps the top-left chosen for 400x300.
- Mechanism (confirmed by reading the code): the anchor uses the window's current frame size, never the requested size. The settle rule gives up after a 30 ms quiet period. A resize that lands after the check at `UNCLOAK_4C` and before `ANCHOR_ARMED` **is missed by both the re-anchor check and the reactive anchor**.
- Frequency (confirmed, corrected in 6.0): idle runs fail too, 1 of 50 with another window alive and 4 of 40 with none. With every CPU busy, 12 of 20 runs of `center-200x150` failed. Load is a sufficient trigger, not the only one.
- After the late-resize fix (6.11): 0 failures in 640 idle runs of the 16 resize specs, and under all-CPU load still failing (212 of 320 passed), by a different mechanism: the resize arrives after the reveal.
- Not known: whether the very first failures were caused by load, and whether the fixture's slow software drawing or the compositor dominates the delay of the new size. No fix of the extension is verified here.

## 3. CLI Coverage Matrix (`spawn-at --help`)

Each row is a command or an option from `spawn-at --help`, with the test that covers it. Covered means a spec or a unit test exists for it. Not written means the spec does not exist yet. Out of scope means the test suite does not run the command, because it changes the machine or needs the network. Characterized: no effect means a test pins that the option does nothing today; it is not a behavior contract.

| Command / Option | Status | Covering Test / Unit Test | Notes / Out of Scope Rationale |
| :--- | :---: | :--- | :--- |
| `install` | Out of scope | — | Mutates host system `$PATH` and GNOME extension directories. Dev-tool safety prevents running in automated suite. |
| `uninstall` | Out of scope | — | Removes binary and extension from user system. Out of scope for test harness. |
| `update` | Out of scope | — | Network-facing update check. Out of scope for local offline verification. |
| `spawn` | Covered | `verify/placement/bottom-right.toml` | Primary command exercised across verification library. |
| `spawn --class <CLASS>` | Covered | `verify/placement/bottom-right.toml` | Tested via fixture explicit class matching. |
| `spawn --area <AREA>` | Covered | `test_oracle_vs_core_explicit_pos_all_three_area_modes_cross_check` | Spec not written: `placement/area-screen-vs-workarea`. |
| `spawn --monitor <MON>` | Not written | Spec not written: `platform/multi-monitor-secondary` | Requires multi-head display environment. |
| `spawn --anchor <ANCHOR>`| Covered | `verify/placement/bottom-right.toml` | Tested all 9 anchors in oracle golden test vectors. |
| `spawn --pivot <PIVOT>` | Covered | `test_golden_default_pivot_all_nine_anchors` | Cross-checked across core and oracle matrix. |
| `spawn -p, --pos <X> <Y>` | Covered | `test_golden_clamp_explicit_pos_0_0` | Absolute and offset positioning cross-checked. |
| `spawn -s, --size <W> <H>`| Covered | `verify/placement/bottom-right.toml` | Sets requested initial target dimensions (400×300). |
| `spawn --clamp <CLAMP>` | Covered | `test_golden_window_larger_than_workarea_clamp_true` | Clamping behavior verified mathematically. |
| `spawn -m, --margin <M>` | Covered | `verify/placement/bottom-right.toml` | Universal margin tested against oracle math. |
| `spawn --margin-{t,b,l,r}`| Covered | `test_oracle_vs_core_directional_margins_cross_check`| Directional boundary margins verified. |
| `spawn --focus` | Characterized: no effect | `commands::spawn::tests::test_spawn_focus_modifiers_have_no_effect_today` | Accepted and parsed, never read by `spawn`: the batch always has `FocusIntent::Exclusive`. The test pins that; it is not a behavior contract. |
| `spawn --no-focus` | Characterized: no effect | same test; live spec `focus/passive-spawn-no-focus` not written | Accepted, currently no effect on `spawn` (works on `transform`, `maximize`, `restore`). |
| `spawn --defocus` | Characterized: no effect | same test; live spec `focus/defocus-active-window` not written | Accepted, currently no effect on `spawn`. |
| `spawn --json` | Covered | `verify/placement/bottom-right.toml` | Tested in CLI execution and parsed by TUI/runner. |
| `spawn --no-wait` | Not written | Spec not written: `basics/no-wait-async` (not a row of the section 2 catalog) | Asynchronous launch without compositor ack wait. |
| `transform` | Covered | `transforms/resize-by-id`, `reposition-by-id`, `anchor-corners` (5), `resize-flash`, `move-flash` | Dynamically moves/resizes live window; the two `-flash` specs are Both and measure nothing. |
| `query` | Covered | `queries/query-windows-lists-fixture`, `query-layout-json`, `query-pointer` | Compositor state inspector. |
| `focus` | Covered | `focus/activate-fixture`, `focus/switch-between-windows` | Brings window to foreground. |
| `defocus` | Covered | `focus/defocus-active-window` (GNOME), `robustness/sway-defocus-unsupported` | Unfocuses active window. |
| `maximize` | Covered | `transforms/maximize-and-restore` (GNOME, Both), `robustness/sway-maximize-unsupported` | Compositor window maximization. |
| `minimize` | Covered | `transforms/minimize-and-restore` (GNOME, Both) | Compositor window minimization. |
| `restore` | Covered | `transforms/maximize-and-restore`, `transforms/minimize-and-restore` | Returns maximized/minimized window to normal floating. |
| `close` | Covered | `test_runner_cleanup_only_ever_passes_registered_ids_to_close` | Exercised by window tracker cleanup routines. |
| `-h, --help` | Not written | Spec not written: `basics/version-and-help` | Standard help synopsis. |

---

## 4. Animation Regression Cases (`minimize-animation`)

### The problem

Under GNOME Wayland, minimizing a window starts an animation of the shell actor that scales it down towards the dock. If the extension uncloaks or moves the window during that transition, or unminimizes it without sequencing `PRE_REVEAL_UNMINIMIZE` first, the window may flash at its previous coordinates or freeze mid-animation.

### The spec

`verify/animation/minimize-animation.toml` is **not written**. The goal it is planned for: a minimized window restored to the normal state does not reveal off target and shows no visual glitch during the transition. Its trace invariants:

1. The state change `MINIMIZED -> NORMAL` precedes `UNCLOAK`.
2. No `UNCLOAK` event comes before `PRE_REVEAL_UNMINIMIZE`.
3. The frame geometry matches the target before the opacity goes back to 255.

### What covers it today

The trace analyser checks the order of events with three tests in `crates/spawn-at-verify/tests/trace_analyser_test.rs`:

- `test_scenario_minimized_passes_and_checks_pre_reveal_unminimize`
- `test_violation_order_uncloak_before_pre_reveal_unminimize`
- `test_scenario_remaximize_passes`

The attended spec `transforms/minimize-and-restore` (kind both) asks a person to judge the animation on GNOME.

---

## 5. Delayed-Paint Calibration & Cloak Budget Analysis

### Status: mostly not measured

The log slice has whole-second bounds, so it can contain the previous run's window. The trace
analyser measures the window named in the fixture's stdout, not the maximum over every lifecycle in
the slice (commit `7ffa99c`). **No budget formula is supported by any measurement.**

Labels used in sections 5 and 6:

- **CONFIRMED** means a quoted line, a count, raw output, or code that was read.
- **HYPOTHESIS** means consistent with the evidence but not demonstrated.

### Measured data (`spawn-at-verify calibrate`, GNOME 46 Wayland, one machine)

`cloak_duration_ms` is UNCLOAK minus CLOAK for the window named in the fixture's stdout. Each row
is one invocation of
`spawn-at-verify calibrate --case cloak/delayed-paint/delay-500 --runs 10`. "Repeat"
means `run --repeat 10` in one process; "separate" means 10 separate `run` processes.

| Batch and mode | Runs measured | Min | Median | Max |
| :--- | :---: | :---: | :---: | :---: |
| Batch 1, separate | 10 | 7.29 ms | 8.60 ms | 18.88 ms |
| Batch 1, repeat | 9 of 10 (1 run had no measurement; cause not investigated) | 6.85 ms | 9.63 ms | 140.18 ms |
| Batch 2, separate | 10 | 32.91 ms | 43.03 ms | 58.99 ms |
| Batch 2, repeat | 10 | 6.97 ms | 13.64 ms | 57.24 ms |

What these four batches show (CONFIRMED unless marked):

- No two runs report the same value. Identical values in neighbouring runs would be the signature
  of one leftover window being measured for several runs. CONFIRMED.
- The spread differs between batches (about 7 - 19 ms in batch 1 separate, about 33 - 59 ms in
  batch 2 separate). The investigation below explains this as two regimes.
- `cloak/delayed-paint/delay-0`, `delay-50`, `delay-150` and `placement/bottom-right` were not
  measured with `calibrate`; single `run`s of them gave about 30 - 44 ms.

### Findings from the cloak investigation

All numbers come from `spawn-at-verify calibrate` (separate-process mode `--mode separate`,
`--repeat` mode `--mode repeat`, per-run phase gaps) and throwaway scripts, on one GNOME 46 Wayland
machine (`cloak/delayed-paint/delay-500` unless stated). Each finding carries its status.

#### Finding 1: cloak time has two regimes

Status: CONFIRMED (measured); mechanism HYPOTHESIS.

| Regime | `COMMIT_RESOLVED` to `SetPositionAnchored` gap | Total cloak (UNCLOAK - CLOAK) |
| :--- | :---: | :---: |
| Isolated: no other test window alive | about 24 - 34 ms (batches of 20: 23.1 - 38.0) | about 27 - 40 ms (26.57 - 41.65) |
| Another fixture or `zenity` window alive | about 5 - 8 ms (5.3 - 10.1) | about 8 - 17 ms (7.6 - 17.5; a few runs 25 - 41 ms, see finding 4) |

- Three 20-run `--mode separate` batches (A, B and an idle-machine batch) were all in the isolated
  regime: cloak medians 31.99, 30.91 and 32.35 ms. Two 20-run `--mode repeat` batches were in the
  fast regime from run 2 on (cloak medians 8.67 and 9.16 ms) but their first run was slow (33.43 ms
  and 27.33 ms; gaps 29.5 and 24.8 ms). CONFIRMED.
- Why `--repeat` differs from separate runs: `--repeat` cleans up windows only after the last
  repetition (`runner/engine.rs:131-132`), so earlier fixture windows are alive during later runs.
  The TUI closes an automatic test's window after each run (`tui/app/queue.rs:66-68`), so TUI runs
  of automatic tests are in the isolated regime. CONFIRMED.
- A controlled switch (same command, only the background changed): no other window 25.7 - 31.0 ms;
  one fixture window alive 5.4 - 7.9 ms; that window closed 25.8 - 34.5 ms; a static GTK `zenity`
  window alive 5.6 - 7.6 ms. CONFIRMED.

#### Finding 2: no deliberate wait between `COMMIT_RESOLVED` and the next batch step

Status: CONFIRMED (code); the stall itself CONFIRMED (probe, +/-10 ms alignment); its content
HYPOTHESIS.

- Code: `commit.js:62-66` logs `COMMIT_RESOLVED` and calls `resolve()`; `extension.js:783-786`
  `await`s it; the next code that runs is the `BATCH_STEP` log at `extension.js:697`. No timer,
  idle callback, frame wait or other work is between them, and the log shows no events in the gap.
- The gap appears only when the commit resolves synchronously (requested size equal to the
  fixture's size): 24.0 - 52.3 ms in 12/12 runs. When the commit resolved from a later `size-changed`
  signal (requested size different) the gap was 0.1 - 0.3 ms in 12/12 runs, but the total cloak was
  not shorter (37.0 - 54.2 ms): the time moved into the commit wait. CONFIRMED.
- A read-only probe calling the extension's `GetCursor` D-Bus method in a loop saw the Shell main
  loop stalled for 28.5 - 33.9 ms starting 0.554 - 0.560 s after the spawn command (the map time) in
  all four spawns probed, matching the 26.9 - 32.0 ms gaps. CONFIRMED (alignment about +/-10 ms,
  because the exact `ACTOR_MAP` wall time is not known).
- What the Shell does during that stall is not shown. HYPOTHESIS: work after the window maps (for
  example first-frame handling); no measurement here identifies it.

#### Finding 3: why another window makes the stall short is unexplained

Status: HYPOTHESIS only.

- Slow batches occurred with about 10 windows open (10 windows were present in all three slow
  batches), so the trigger is not simply "another window exists".
- CPU load does not explain it: correlation of the gap with load1 over 40 runs was 0.18 and with CPU
  MHz 0.31, and an idle-machine batch (load1 0.68 - 0.95) was just as slow. CONFIRMED.
- Two earlier `separate` batches (14:37 and 18:37 on 2026-10-11) were in the fast regime; what else
  was mapped then was not recorded. Not explained.

#### Finding 4: `UNCLOAK_4C` to `UNCLOAK` is a one-frame wait

Status: code CONFIRMED; frame quantisation HYPOTHESIS.

`extension.js:861` does `await waitForFrames(global.stage, 1, 50 ms)`, then `_reveal`. It was
1.1 - 4.8 ms in the isolated batches, with occasional 10 - 35 ms waits (34.5, 19.9, 18.2, 11.1, 9.8 ms)
that are why a few fast-regime cloaks were 25 - 41 ms. That these are multiples of the frame period
was not shown.

#### Finding 5: outliers did not reproduce

Status: CONFIRMED (not reproduced); cause unknown.

The earlier 140.18 ms, 115.71 ms and one unmeasured run did not recur in more than 100 runs
afterwards (40 `--repeat` runs, 60 separate runs and about 50 scripted runs); the largest cloak seen
was 61.9 ms. `calibrate` saves the raw log slice of any run that is unmeasured or at least 50 ms
(`verify-results/calibrate-slices/`) if one recurs.


### Trace capture and "Pass (trace unchecked)"

A drawer once showed `Cloak 162.0 ms`, `! over provisional budget`, `Trace ✔ clean, no violations` and Verdict Pass for two different tests while its TIMELINE said "no trace captured". Each finding below is labelled by how it is known: CONFIRMED means read in the code or reproduced on the recording machine on 2026-10-12, and HYPOTHESIS means consistent with the evidence but not demonstrated.

#### Finding 1: where `162.0 ms` came from

Status: CONFIRMED. The explanation for the equal numbers of two tests is a HYPOTHESIS.

- The drawer's Cloak row used the trace's CLOAK..UNCLOAK time and, when there was none, fell back to `TestResult::duration_ms` (the old `tui/views/drawer/result_section.rs`). That is the wall-clock time of the whole `spawn-at spawn` process (`runner/executor.rs`, `start.elapsed()`), not a cloak time. The Budget row then judged that number against 100 ms.
- Reproduced with the previous build (8fa1975) and the extension's logging forced off: `cloak/delayed-paint/delay-0` passed with `"log_slice": ""` and `"duration_ms": 142`, which that drawer would have shown as `Cloak 142.0 ms`.
- No state is shared between tests: each result gets its own `duration_ms`.
- Why two tests showed the same number is a HYPOTHESIS. Three consecutive runs of the same test reported exactly 142 ms, and 142 recurs across older reports in `verify-results/`, so equal wall-clock values for different runs are normal. The particular 162 ms was not reproduced.

#### Finding 2: why the log slice was empty

Status: mechanism CONFIRMED; the cause in that session is a HYPOTHESIS.

- The journal source was in use (`XDG_CURRENT_DESKTOP=ubuntu:GNOME`; `run` now prints `Timing log source: journalctl`). The no-op source (`engine.rs`) is chosen only when that variable is unset or lacks "gnome".
- The extension's logger writes nothing while its logging is off (`assets/gnome/modern/logger.js`, `logTime`). The journal holds no line for window ids 870906690 or 870906691. CONFIRMED.
- That logging was off during those runs is a HYPOTHESIS. The journal shows `gnome-extensions disable spawn-at@harsh.local` at 07:37:54 IST and the next `SET_LOGGING` at `+0.00ms` at 07:40:33. A logger's clock starts at its first logged line (`_t0`), so that line was the first of a new logger instance, and a new instance starts with logging off (`_loggingEnabled` undefined, then `SPAWN_AT_DEBUG`).
- The reload and the off state in between were not observed, and who sent `SetLogging(true)` at 07:40:33 is unknown. The same logger instance was still running at the live check (clock `+977148.32 ms` at 07:56:50 = 16 min 17 s after 07:40:33).

#### Finding 3: why Trace read "clean"

Status: CONFIRMED.

`TraceReport::is_pass()` is `violations.is_empty()`, so an analysis of zero events is a pass. An empty (not missing) slice produced exactly that report, and a result with no slice at all hit an explicit `✔ clean, no violations` branch in the drawer. An absent trace is now `TraceState::NotCaptured` (`trace/state.rs`), never a report.

#### Finding 4: how logging is enabled

Status: CONFIRMED. The last point (two TUIs) was read in the code and not run.

- The extension has no getter for its logging state (`dbus.js` exposes only `SetLogging`). The state lives in the logger instance created by `enable()` and is off again after a reload.
- Turning logging off writes no line (`logger.js` clears the flag before it calls `logTime`). After `SetLogging(false)` the last line of `~/.local/state/spawn-at/session.log` was still `SET_LOGGING enabled=true`, and neither that file nor the journal contains a single `enabled=false`.
- So `tui/logging.rs::detect_current_logging_state` returns `true` once any enable has ever been logged, and `ExtensionLoggingGuard::init` then skips enabling. Only the TUI used the guard; `run` and `calibrate` never enabled logging.
- The guard restores the state it detected on exit, so a second TUI whose startup state was "off" would turn logging off for an open one. Nothing re-checked the state before a run.

#### What `spawn-at-verify` does about the trace

- Before every run that needs a trace (`never_visible_off_target`, or an id under `cloak/`), `runner/executor.rs` calls `SetLogging(true)` and polls the journal up to six times, 40 ms apart, for the `SET_LOGGING enabled=true` line (`ext_logging.rs`). Without a getter this "set and confirm" replaces "query and re-enable". The time spent is not part of the run's `duration_ms`.
- If the call fails, the echo never arrives, or the log source is the no-op one, the reason is stored as the result's `trace_note` and printed by `run`. The TUI uses the same code path (`tui/app/queue.rs` calls `execute_single_test`).
- A slice with no event for the window the fixture named (no slice, an empty slice, or only other windows' events) is "not analysed". The drawer then shows `Trace  not analysed (no trace captured)`, `Cloak  not measured`, `Budget  not measured` and a `Reason` row. The run's duration is never shown as a cloak time.
- A trace-dependent test that otherwise passes with no trace ends as `Pass (trace unchecked)` (`AutoPassTraceUnchecked` / `FinalStatus::PassTraceUnchecked`): orange, not counted as a passed required test, and a required platform test with this status fails qualification. If its geometry was also unchecked the status stays "geometry unchecked" and the message names both.
- Exit codes: 0 no failure; 1 any failure, error, flaky or blocked; 2 nothing failed but a REQUIRED test passed with its geometry or its trace unchecked. Optional unchecked passes keep exit 0 and are listed in the summary.

**Not verified**: the fixes are covered by unit and integration tests with a fake extension and by a live `run` on the recording machine. The interactive TUI itself was not driven by a script, and no extension reload (`gnome-extensions disable/enable`) was performed.

### Budget Policy
The budget has two parts: an advisory warning and the pass/fail invariants.

#### Advisory budget warning

**The 100 ms warning threshold is provisional**: it is a guess, not derived from the data above, and no measurement supports a budget that depends on the paint delay. It is shown in the TUI drawer as `over provisional budget` and is not an automated pass/fail threshold. CONFIRMED: the largest cloak observed is 61.9 ms, below 100 ms. The budget stays provisional, and the two regimes above are why a single threshold cannot be derived from one batch.

#### Pass/fail invariants

Automated pass/fail criteria are anchored in correctness invariants rather than synthetic timing limits:

- The window must never be visible off-target (zero flicker).
- `CLOAK` must precede `AFTER_SET_SIZE`, `COMMIT_RESOLVED`, `AFTER_SET_POSITION_ANCHORED` and `UNCLOAK` (a partial order checked by the trace analyser).
- `COMMIT_RESOLVED` and position configuration must precede `UNCLOAK`.
- The final mapped geometry must match the oracle coordinates within tolerance (`±1 px`).

### Multi-Run CLI Flakiness Detection (`--repeat`)

`spawn-at-verify run` detects flakiness over several runs with the `--repeat <REPEAT>` flag:

- The flag reads `--repeat <REPEAT> Repeat each test N times (cap 100) for flakiness detection [default: 1]`.
- It executes the test action N times. If outcomes vary across repetitions (for example pass and fail), the runner marks the result as `FinalStatus::Flaky` with an explanatory diagnostic message.
- All runs are stored. With `--repeat N`, `report.json` keeps the first repetition in `results` (marked `Flaky` if any repetition's outcome differed) and stores every repetition, in order, under the top-level `repeat_runs` array (`[{id, runs: [TestResult, ...]}]`).
- `report.md` gets a "Repeat Runs" table (run, statuses, duration, cloak) for each repeated test. Single runs and older reports have no `repeat_runs`.
- `spawn-at-verify calibrate` prints min/median/max across the runs and writes the same report.

### `spawn-at-verify calibrate`
`spawn-at-verify calibrate --case <id> [--runs N]` (N defaults to 10, capped at 100) measures one case's `cloak_duration_ms` twice: with N separate `run` processes and with `run --repeat N` in one process.

- Per run it prints the cloak duration, the window id named in the fixture's stdout, the number of distinct windows present in the log slice, and the report path. Then it prints min/median/max.
- For runs with no measurement or a cloak of at least 50 ms it also prints the scoped event list.
- Separate runs that start within the same second share a report directory name (second-granularity timestamps) and the output says so.

## 6. Min-size contract

Investigation of 2026-10-12. No code, spec or budget was changed. All live numbers were taken before any build or test run. Machine: GNOME Shell 46.0 on Wayland, workarea `0,40 1920x1040`, one monitor. Every claim is CONFIRMED (a quoted line, a count or raw output) or HYPOTHESIS. Throwaway scripts were kept outside the repo.

### How to read section 6

Each 6.NN is a dated investigation with a question, a method (what was searched or run), a result and a status. The status is CONFIRMED or HYPOTHESIS for a measurement or a reading of the code, and "stated" or UNDEFINED for a contract that a document gives or does not give.

The numbers are ids that other pages link to: the test log and the experiment pages cite them, so a number is never reused. For the same reason the rows of the tables in 6.1 and 6.4 and the items of 6.2 keep their numbers ("6.4 row 5", "6.2 item 9"). Sections 6.0 to 6.9 are written out here. From 6.10 on, each section is a stub that points to its record in the test log.

The question, the method, the result and the status of the whole section:

- Question: what should spawn-at do when the requested size is below the app's minimum (or above the workarea), especially with a bottom-right anchor, and is the late-resize mis-centre of section 2.1 a bug or intended?
- Method: a search for a stated contract (6.1), a reading of the code path (6.2), live probes with `zenity` (6.3), a classification (6.4), and two measurements of the 30 ms quiet window (6.5, 6.6).
- Result: the README promises that a window that ends up larger than asked is "positioned using their final size, so anchors land correctly" and that a one-second listener re-applies the anchor after late resizes. Both promises are broken whenever the toolkit's resize arrives in a gap in the extension.
- Status: that makes the mis-centre (and its bottom-right twin) a **BUG**, not intended behaviour. What the user still has to decide is listed in 6.7.

The design behind the bug is empirical minimum discovery under the cloak: the request is made while the window is hidden, the app settles at its minimum, and the settled frame is used to anchor (6.2 item 3). The bug is that a late resize is mistaken for "settled at the minimum".

### 6.0 Corrections to section 2.1 (found while doing this work)

- Question: which statements of the original section 2.1 were wrong or too strong?
- Method: the statements were compared with the findings of 6.2, 6.3, 6.5 and 6.6. Section 2.1 states the corrected version.
- Result: three corrections.
- Status: labelled in the table.

| Topic | What 2.1 said | Correction | Status |
| :--- | :--- | :--- | :--- |
| `zenity` minimum | `zenity --text-info` "has a 300x400 minimum". | 300x400 is its default size. Its real minimum is about 300 wide and 201 high: requests of 200x150 and 50x50 settle at 300x201 (6.3). The CLI text "window minimum is 300x400" for it was the size at the moment spawn-at read the window, while it was still shrinking to 300x201 (6.2 item 9, 6.4 row 3). It is a symptom of the late-resize bug, not a separate bug. | **Wrong**, CONFIRMED |
| Idle failures | The fixture failed 0 of 80 runs at idle. | Later idle batches failed: 1 of 50 (6.5) and 4 of 40 in the isolated regime (6.6). The 40 `--repeat` runs had earlier windows alive by design (`runner/engine.rs:131-132`, section 5). For the 40 separate `verify run` invocations 1.1 s apart, earlier windows may also have been alive (the fixture's backstop lifetime for automatic tests is 10 s, `runner/validation.rs:48-57`). Whether `run` closes its window when it exits was not checked. Both would be the fast regime of section 5. | Superseded; the last point is a HYPOTHESIS |
| CPU load | CPU load is the confirmed cause of the failures. | Load does raise the failure rate (12 of 20 in 2.1), but idle runs also fail, so load is a sufficient trigger, not the only one. | corrected |

### 6.1 Contract search

- Question: does any document or commit state what spawn-at should do when the requested size is below the app's minimum, or above the workarea?
- Method: a search of the places and terms below.
- Result: 14 behaviours, each with the contract that is stated for it or the word UNDEFINED (table below).
- Status: per row.

Searched:

- `README.md`
- `docs/behavior-notes.md`
- `docs/legacy-gnome.md`
- `AI_DEV/*.md` (file no longer in the repository; deleted in `1806eb7`)
- `scripts/smoke-live.sh`
- `spawn-at spawn --help`
- code comments and tests
- `git log -S`

The terms: min size, minimum, clamp, raised, size_raised, raised_w, raised_h, clampedSafeSize, reportedMinSize, "min size unreported".

`CHANGELOG.md` was not part of this search. Its matches for min size, clamp and raised (lines 288 and 350-351) are about the ready-wait, not about this contract.

`git log -S` found no commit message that states a rule beyond the ones quoted below:

- `4ae8f97` added the README text.
- `39470eb` added the per-axis flags.
- `bdedd3e` added the sub-minimum smoke cases.
- `0c548c7` is "wip: fix sub-minimum geometry overshoot centering for legacy GTK3/VTE clients".

The rows keep their numbers because 6.2 and 6.4 cite them ("6.1 row 13").

<!-- site: records style=cards -->
| Behaviour | Row | Stated contract (quote and citation) | Status |
| :--- | :-: | :--- | :--- |
| Window smaller than requested is raised to the app's minimum | 1 | "Apps that refuse a requested size can still end up larger than you asked for" (`README.md:149-150`, now `README.md:206`); "Toolkit coverage: ... including apps that enforce a minimum window size" (`README.md:148`; that sentence is no longer in the README and no current equivalent was found) | stated |
| Position uses the window's final size | 2 | "...but they're positioned using their final size, so anchors land correctly." (`README.md:150`, now `README.md:206`) | stated |
| Late resize after reveal is corrected | 3 | "If an app resizes itself after reveal ..., a 1-second listener re-applies your anchor so the window doesn't drift." (`README.md:110`; the README now says: "After the reveal the extension keeps a listener on the window's size changes: when the app resizes itself (font loading, client-side decorations) and the anchored position moves, the frame is moved back.", `README.md:124`) | stated |
| Wait for the app, not the compositor | 4 | "the extension watches the app's buffer settle (30ms quiet period, 120ms minimum, 1500ms hard cap) before revealing" (`README.md:103`; the README now says: "On GNOME 46 Wayland the extension waits until the app has answered the size request (default cap 250 ms, hard limit 1500 ms) before revealing. XWayland and X11 windows, and the legacy GNOME 42-44 extension, still use the old rule: a 30 ms quiet period, 120 ms minimum, 1500 ms hard cap.", `README.md:117`) | stated (settle rule), but silent on what happens when the app has not answered inside 30 ms |
| Anchor math with a minimum | 5 | w' = min(max(w_t, w_min), w - m_l - m_r), anchor x/y from w', h' (`AI_DEV/spawn-at_master_architecture_blueprint.md` (file no longer in the repository; deleted in `1806eb7`), section 9.4 "Anchoring"). Design document, not user documentation | stated (internal) |
| Minimum larger than available area | 6 | "the anchored edge's margin wins; the opposite edge may overflow the work area. The anchored edge is never pushed." (blueprint 9.4, "Post-clamp margin re-evaluation", `:993`). CLI text: "Window is oversized (WxH); bottom and right margins ignored." (`src/diagnostics.rs:1296`) | stated (internal and CLI message) |
| Geometry floor before asking the compositor | 7 | "Enforces a minimum geometry floor of 100x60px before passing dimensions to Mutter" (`AI_DEV/system_survey_and_understanding.md:223` (file no longer in the repository; deleted in `1806eb7`); constants `assets/gnome/modern/extension.js:33-34`) | stated (internal) |
| Exit code when the size was raised | 8 | `scripts/smoke-live.sh` sub-minimum cases (30x20 at bottom-right) "Assert zero exit code, no false timeout warnings, and workarea bounds" (commit `bdedd3e` message) | stated (test and commit message only) |
| What the CLI prints for a raised size | 9 | Diagnostic enum doc: "The requested size was below toolkit minimums and was expanded." (`src/diagnostics.rs:149`); per-axis wording at `src/diagnostics.rs:240-257`, unit-tested at `:654-726` | stated; the number in it is the size when spawn-at read the window, wrong only while the window is still resizing (6.2 item 9, 6.4 row 4) |
| Default margin when `--margin` is omitted | 10 | `--help` says only "Universal margin in pixels applied to all boundaries". Code: `self.margin.unwrap_or(16)` (`src/cli/args.rs:134`). An example in `docs/behavior-notes.md:19` shows "clamped to (16, 16)" | **UNDEFINED** in user docs |
| Window larger than the workarea | 11 | blueprint formula clamps the size to workarea minus margins (row 5); CLI only warns (row 6). Observed: the window is not clamped by spawn-at (6.3) | **UNDEFINED** for the size itself |
| Request below the app minimum with the app's minimum unknown to the extension | 12 | `FALLBACK_USED reason="min size unreported" usingFrame=` (`extension.js:1002-1007`) is the empirical path of 6.2 item 3: the settled frame is used. No user document describes it | stated in code and by the author; not in user docs |
| Which size an anchor uses while the app has not yet answered | 13 | none | **UNDEFINED** (the code uses the current frame, 6.2) |
| Resize arriving more than 1 s after the last correction | 14 | README said "1-second listener" (`:110`; it now says the listener is dropped "one second after the last correction if the size then stays quiet", `README.md:124`); `ANCHOR_RETIRE_QUIET_MS = 1000` (`extension.js:35`) | stated, limits not documented |

### 6.2 Code path of a request below the app's minimum

- Question: what does the code do, from the CLI to the placed window, when the requested size is below the app's minimum?
- Method: the code was read at the cited lines, the minimum API was probed on this Shell (item 3), and the journal of the session was counted.
- Result: items 1 to 9 below. The path has no step that tells "settled at the minimum" from "still resizing" (item 3).
- Status: CONFIRMED (code and logs) unless an item says HYPOTHESIS or UNDEFINED.

#### Item 1: the CLI sends the request

`spawn` passes the size to the extension as a batch `SetSize`, `WaitForCommit`, `SetPositionAnchored`. The extension reports the result through the `SpawnClaimed` signal with `size_raised` (`assets/gnome/modern/dbus.js:76`, `src/platform/linux/gnome/dbus.rs:36`).

#### Item 2: `_stepSetSize` sets a floor for the request

`_stepSetSize` (`extension.js:736-782`) reads the minimum with `_getMinSize` (`:298-303`: `window.get_min_size?.()`, `[0, 0]` when absent). It computes `floorW = max(100, minW)`, `floorH = max(60, minH)`, `safeW = max(w, floorW)` and `safeH = max(h, floorH)`, stores them as `ctx.clampedSafeSize`, then calls `move_resize_frame(true, x, y, safeW, safeH)`.

`clampedSafeSize` protects the compositor request from a frame smaller than 100x60 (per `AI_DEV/system_survey_and_understanding.md:223`, a file no longer in the repository, deleted in `1806eb7`: client-side-decoration padding subtraction giving negative widget allocations). It is a floor for the request, not a measured app minimum.

#### Item 3: the API route to the minimum is unavailable; the design finds the minimum empirically

Status: CONFIRMED (code and logs).

`Meta.Window.prototype.get_min_size` is `undefined` in the Mutter 14 (GNOME 46) typelib:

```
$ GI_TYPELIB_PATH=/usr/lib/x86_64-linux-gnu/mutter-14 gjs -c 'imports.gi.versions.Meta="14"; const Meta=imports.gi.Meta; print("get_min_size:", typeof Meta.Window.prototype.get_min_size)'
get_min_size: undefined
```

The journal of this session holds 722 `SET_SIZE_REQUEST` events, all with `reportedMinSize=(0x0)`, and 722 `FALLBACK_USED` events. A non-zero reported minimum has never been received in this session, so the API route (`_getMinSize`, `extension.js:298-303`) always returns `[0, 0]` on GNOME 46.

This does not mean the extension has no minimum handling. The design, as described by the author and as the code does it, is empirical discovery under the cloak:

- The window already exists and is mapped, cloaked (opacity 0). `Cloak` is the first instruction (`src/platform/linux/gnome/mechanics.rs:146`, executed at `extension.js:712`), then `SetSize` with the user's size, then `WaitForCommit` (`mechanics.rs:148-154`, `extension.js:713-715`, `:736-782`, `:783-787`). The app is asked for the size while nobody can see it.
- An app that refuses an illegal size answers with its own size, and `waitForCommit` lets that settle (`commit.js:62-66` logs `COMMIT_RESOLVED ... finalFrame=`; `SIZE_SETTLED` after 30 ms of no change, `commit.js:88-96`).
- The settled frame is then taken as the size to anchor with. `_computeAnchoredPosition` reads `window.get_frame_rect()` (`extension.js:985-994`), and `computeAnchoredPosition` uses `effW = max(frame.width, minW)` (`anchor.js:19-20`; `minW` is 0 here, so the frame alone).
- The log line `FALLBACK_USED reason="min size unreported" usingFrame=(WxH)` (`extension.js:1002-1007`, printed whenever `minW` and `minH` are both 0) records which frame was used. That line is the empirical path being taken; it is not an error.
- After the reveal, `_reanchorIfResized` (`extension.js:913-936`) and the reactive anchor (`:1025-1050`) re-anchor if the frame still changes.

What the code does not have (searched, not found):

- No step deliberately requests a tiny size to provoke the minimum. The only sizes sent are the user's (`mechanics.rs:149-152`, `src/platform/linux/gnome/driver.rs:302`), raised to at least 100x60 by the geometry floor (`extension.js:741-751`). `30x20` appears only as a user example in `scripts/smoke-live.sh:168-175` and in unit tests.
- No minimum is stored or reused per app.
- Nothing tells "settled at the minimum" from "still resizing". The only discriminators are the 30 ms quiet timer and equality with the target or safe size (`commit.js:77-83`). `_isToolkitClamped` (`extension.js:906-910`, `frame > target`) only feeds the focus-pulse decision at `UNCLOAK_4B` (`:822-824`).

That missing discriminator is exactly the section 2.1 bug: a late resize is read as "settled at the minimum". Whether other Shell versions expose `get_min_size` is not known (HYPOTHESIS: GNOME 45 to 48 behave the same; not tested).

#### Item 4: `waitForCommit`

`waitForCommit` (`assets/gnome/modern/commit.js:34-106`) ends with `AT_TARGET` when a sampled frame equals the target or the safe size. On the first `allocation` sample with unchanged size it replaces the idle timer with a 30 ms quiet timer that finishes `SIZE_SETTLED` (`commit.js:88-96`).

A below-minimum request can never reach `AT_TARGET`, because the frame never equals 200x150. So for such requests `SIZE_SETTLED` is the normal exit, not an edge case. CONFIRMED in 6.3: every zenity run below its minimum resolved `SIZE_SETTLED (300x400)`.

#### Item 5: the anchor

`_stepPosition` calls `_applyAnchoredPosition`, which calls `computeAnchoredPosition(bounds, window.get_frame_rect(), minSize, payload)` (`extension.js:985-994`, `anchor.js:13-56`). The anchor uses the window's current frame size, `max(frame, minSize)`, never the requested size:

- `AT_TARGET` reached: the frame is the requested size, so the anchor is exact.
- `SIZE_SETTLED`, app still at its old size: the anchor uses the old size (zenity: 300x400, fixture: 400x300). The window is placed for that size and resizes afterwards.
- Toolkit minimum already applied before the step: the anchor uses the minimum (the final size) and is exact.

#### Item 6: repair after the fact

`_reanchorIfResized` (`extension.js:913-936`) compares the size at `UNCLOAK_4C` with the positioned size once. A resize after that check is only caught by the reactive anchor. It is armed at the end of the reveal (`_armAnchor`, `extension.js:1025-1034`, `ANCHOR_ARMED`) and retires 1 s after its last correction (`:35`). A resize between `UNCLOAK_4C` and `ANCHOR_ARMED` is missed by both: the gap is the bug of section 2.1.

#### Item 7: the Rust side

`raised_w = final.width > requested.0`, `raised_h = final.height > requested.1`, and `size_raised = raised_w || raised_h || <extension flag>` (`src/commands/spawn.rs:493-497`). The final rectangle is the one read when spawn-at finishes, not necessarily the settled one.

The JSON output carries all three (`:499-513`). `verify_and_report_placement` is called with `min_size = None` (`:520`), so the diagnostic text takes its "minimum" from the final size (`src/diagnostics.rs:236-257`, `:423-430`).

#### Item 8: CLI output and exit code

| Case | Text |
| :--- | :--- |
| Raised | `INFO: Requested size (200x500); window minimum width is 300, placed at (810, 310) [300x500].` |
| Requested far below | `INFO: Requested size (50x50) is below toolkit minimums; window will expand.` |
| Mis-positioned | `WARNING: Window mapped at (810, 360) [300x500] but expected position was (810, 310). Final size differs from the requested size (200x500); window was not positioned at target.` |
| Oversized | `WARNING: Window is oversized (3000x2000); bottom and right margins ignored.` |

The exit code was 0 in all 60 probe runs, including every WARNING (6.3). A mis-positioned window is therefore **invisible to a script that checks only the exit code**.

#### Item 9: where the two INFO texts come from

Status: CONFIRMED (code).

The "below toolkit minimums" text is `INFO: Requested size (WxH) is below toolkit minimums; window will expand.`

- It is printed by `src/commands/spawn.rs:203-207` for `GeometryDiagnostic::SubMinimumSize`. That value is produced by `check_geometry_diagnostics` at `crates/spawn-at-core/src/geometry.rs:744-746`: `if w < 100 || h < 80`.
- It is a static threshold on the requested size, evaluated at `src/commands/spawn.rs:171-209` before the child is spawned. It does not use the observed size, the app, or any settled frame.
- So it fires for any 99-wide or 79-high request, even for an app with no minimum. It never fires for a request of 100x80 or larger, even when the app raises it (200x500 on zenity printed the other INFO below).
- The threshold also differs from the extension's own floor (100x60, `extension.js:33-34`); nobody states why (UNDEFINED).

The "window minimum" text is `INFO: Requested size (WxH); window minimum [width|height] is N, placed at (X, Y) [AxB].`

- It is built from the rectangle spawn-at read when it finished (`src/commands/spawn.rs:493-520`, then `src/diagnostics.rs:236-257`; `min_size` is `None` at `spawn.rs:520`). N and AxB are that rectangle's size.
- In the 6.3 probes this text matched the window's final size in 7 of 14 cases and did not in the other 7. All 7 mismatches are runs where the window was still resizing when spawn-at read it (300x400 at exit, 300x201 afterwards, centre and bottom-right "both" cases). No mismatch occurred when the size was stable.
- That is a symptom of the late-resize bug (6.4 row 3), not a separate wording bug.

### 6.3 Live probes

- Question: what does a request below the app's minimum, at its default size, or above the workarea end up as, and is the final position the anchor's position for the final size?
- Method: `spawn-at spawn --class '*' --size W H --anchor A --json -- env GDK_BACKEND=wayland zenity --text-info ...`, with no `--margin` (so the default 16 applies, `args.rs:134`), 5 runs per row, on an idle machine with nothing else running. "Final" is `spawn-at query windows --json` read 1.3 s after spawn-at exited, so late resizes and reactive-anchor corrections are included.
- Result: the table below. "Anchored for final size" compares the final position with the oracle for the final size and margin 16 (centre: `floor((area - size)/2)`; bottom-right: `workarea edge - margin - size`; oversized: top-left plus margin). The exit code was 0 in every run; the stderr column holds the most common line.
- Status: CONFIRMED; the observations follow the table.

| Anchor and case (requested) | Final size | Final position(s) | size_raised / raised_w / raised_h | Anchored for final size | Overflows workarea | stderr (most common) |
| :--- | :--- | :--- | :--- | :---: | :---: | :--- |
| centre, width only (200x500) | 300x500 | 810,310 (3), 810,360 (2) | true/true/false | 3/5 | no | INFO "window minimum width is 300, placed at (810, 310) [300x500]"; 2 runs WARNING "mapped at (810, 360) ... expected position was (810, 310)" |
| centre, height only (500x300) | 500x300 | 710,410 (5) | false/false/false | 5/5 | no | INFO "mapped at final size (500x300) and positioned at (710, 410)" (zenity's real minimum height is below 300, so no raise) |
| centre, both (200x150) | 300x201 | 810,459 (4), 810,360 (1) | true/true/true | 4/5 | no | INFO "window minimum is 300x400, placed at (810, 360) [300x400]" (the final size was 300x201); 1 run WARNING |
| centre, far below (50x50) | 300x201 | 810,459 (5) | true/true/true | 5/5 | no | INFO "below toolkit minimums; window will expand" |
| centre, exactly at default (300x400) | 300x400 | 810,360 (5) | false/false/false | 5/5 | no | INFO "mapped at final size (300x400)" |
| centre, larger than workarea (3000x2000) | 1920x1040 | 16,56 (3), 810,360 (2) | false/false/false | 3/5 | yes | WARNING "Window is oversized (3000x2000); bottom and right margins ignored." |
| bottom-right, width only (200x500) | 300x500 | 1604,564 (4), 1604,664 (1) | true/true/false | 4/5 | 1 run yes (y 664 + 500 = 1164 > 1080) | INFO / 1 run WARNING "mapped at (1604, 664) ... expected position was (1604, 564)" |
| bottom-right, height only (500x300) | 500x300 | 1404,764 (5) | false/false/false | 5/5 | no | INFO |
| bottom-right, both (200x150) | 300x201 | 1604,863 (3), 1604,664 (2) | true/true/true | 3/5 | no | INFO "window minimum is 300x400, placed at (1604, 664) [300x400]"; 2 runs WARNING |
| bottom-right, far below (50x50) | 300x201 | 1604,863 (3), 1604,664 (2) | true/true/true | 3/5 | no | INFO "below toolkit minimums; window will expand" |
| bottom-right, exactly at default (300x400) | 300x400 | 1604,664 (5) | false/false/false | 5/5 | no | INFO |
| bottom-right, larger than workarea (3000x2000) | 1920x1040 | 16,56 (5) | false/false/false | 5/5 | yes | WARNING "oversized ... bottom and right margins ignored" |

Raw lines of two failing runs (script output):
```
center | below-both | 200 x 150 | run2 | json 810,360 300x201 | final(query) 810,360 300x201 | raised true/true/true | anchored-for-final: NO (want 810,459) | overflow no | exit 0 | WARNING: Window mapped at (810, 360) [300x201] but expected position was (810, 459). Final size differs from the requested size (200x150); window was ...
bottom-right | below-w | 200 x 500 | run5 | json 1604,664 300x500 | final(query) 1604,664 300x500 | raised true/true/false | anchored-for-final: NO (want 1620,580) | overflow yes | exit 0 | WARNING: Window mapped at (1604, 664) [300x500] but expected position was (1604, 564). Final size differs from the requested size (200x500); window wa...
```
(The "want" column in the raw script output assumed margin 0 and is wrong; the table above uses margin 16. The positions and the verdicts "NO" for those two runs are unchanged by that.)

Observations:

- CONFIRMED: in the cases where the final size differs from the size the app opened with and the app answers after the 30 ms window, some runs end placed for the old size: 3 of 25 non-oversized centre runs and 5 of 25 bottom-right runs. At bottom-right that is a visible defect: a 300x201 window left 199 px above the corner (y 664 instead of 863), and one 300x500 window left hanging off the bottom of the workarea.
- CONFIRMED: the CLI says "window minimum is 300x400" for a window that settled at 300x201. The number is the window's size at the moment spawn-at read it, while it was still resizing (6.2 item 9): a symptom of the late-resize bug, not a wording bug. spawn-at also exits 0 on every warning.
- CONFIRMED: the oversized window is not clamped by spawn-at. It ends 1920x1040 (the whole workarea, 16 px more than workarea minus margins on each axis) and overflows by the margin on the right and bottom, at top-left plus margin. The flags `size_raised/raised_w/raised_h` are all false although the window is smaller than requested; there is no flag for "reduced". Who limited it to 1920x1040 (Mutter, GTK) is not shown (HYPOTHESIS: Mutter).
- The 300x400 "at-min" row is the app's default size, so no resize is needed there. It is the control: 10 of 10 anchored correctly.

### 6.4 Classification

- Question: for each behaviour of 6.1, is it INTENDED, a BUG, or UNDEFINED?
- Method: the stated contracts of 6.1 were compared with the code path of 6.2 and the probes of 6.3.
- Result: the table below. The row numbers are those of 6.1, with "b" rows for a second case.
- Status: the class column. A BUG is a behaviour that violates a stated contract.

| Behaviour | Row | Class | Why |
| :--- | :-: | :---: | :--- |
| Window ends larger than requested when the app has a minimum | 1 | INTENDED | `README.md:149-150` |
| Final position is correct for the final size | 2 | **BUG** when violated (centre 3/25, bottom-right 5/25 in 6.3; fixture 12 of 20 under load, 1 of 50 and 4 of 40 at idle) | violates `README.md:150` "positioned using their final size, so anchors land correctly" |
| Late resize (arriving after the 30 ms settle) leaves the window mis-centred | 3 | **BUG** for centre | violates `README.md:110` (the listener "re-applies your anchor so the window doesn't drift") in the cases where the resize lands between `UNCLOAK_4C` and `ANCHOR_ARMED` (`extension.js:913-936` and `:1025-1034`); the same reactive anchor corrected zenity 500x450 in 16 of 20 and 19 of 20 runs in two batches (section 2.1) and missed it in the rest |
| Late resize leaves the window mis-anchored at bottom-right | 3b | **BUG** for bottom-right | same promises; and the damage is larger: the corner is missed by the size difference (199 px in 6.3) and the window can overflow the workarea |
| Text "window minimum is WxH" | 4 | **BUG** symptom of row 3 (not a separate wording bug) | the number is the window's size when spawn-at read it (6.2 item 9); it matched the final size in 7 of 14 probe cases and all 7 mismatches were windows still resizing (300x400 printed, 300x201 final). When the size is stable it is correct. Fixing row 3 fixes this text |
| INFO "Requested size (WxH) is below toolkit minimums; window will expand." | 4b | UNDEFINED (threshold) | static `w < 100 \|\| h < 80` on the requested size (`geometry.rs:744-745`); differs from the extension's 100x60 floor and ignores the app |
| Exit code 0 when the window ended mis-positioned (WARNING printed) | 5 | UNDEFINED | only the sub-minimum smoke assertion (`bdedd3e`) says "zero exit code"; nothing says a WARNING about a mis-positioned window must exit 0 |
| Oversized request: anchored edge keeps its margin, opposite edge overflows | 6 | INTENDED | blueprint 9.4 conflict rule and the CLI message "bottom and right margins ignored" |
| Oversized request: size not reduced to workarea minus margins | 7 | UNDEFINED | the blueprint formula says it should be; the code does not do it; no user-facing text decides |
| Oversized request with the centre anchor ends at top-left plus margin (16,56) | 8 | UNDEFINED | follows from 6; the blueprint's centre formula would give 0,40 |
| Geometry floor 100x60 | 9 | INTENDED | `system_survey_and_understanding.md:223`, `extension.js:33-34` (not observable with zenity: its minimum is above the floor) |
| Default margin 16 | 10 | UNDEFINED in user docs | `args.rs:134` only |
| Minimum unknown to the extension on GNOME 46 (found empirically under the cloak instead) | 11 | INTENDED (design stated by the author; the code does this: 6.2 item 3). Nothing in user docs mentions it | the empirical route is the design; its flaw is row 3 |
| Late resize more than 1 s after the last correction | 12 | INTENDED (limit stated) | `README.md:110`, `extension.js:35` |

The `README.md` line numbers in this table are those of the README when the table was written; the current places of the quoted sentences are in the table of 6.1.

The late-resize mis-centre of section 2.1 **is a BUG, for centre and for bottom-right**. Reason: the README promises correct anchoring for the final size and a one-second corrector; the logs show the window's final size is known to the extension (the `UNCLOAK` event reports `frame=(760,410,200x150)` before `ANCHOR_ARMED`) yet nothing re-anchors it. It is not "intended behaviour for a resize that arrives late", because the corrector exists exactly for that case and works when the resize lands after it is armed.

### 6.5 Margin to the 30 ms quiet window, `placement/center-1200x800`

- Question: how much margin is there between the arrival of the app's `size-changed` sample and the 30 ms quiet timer that ends the wait?
- Method: the fixture was driven through `spawn-at` exactly as the spec does (`spawn --class verify.placement.m<pid> --anchor center --size 1200 800 --margin 16 --json -- target/debug/fixture ... --exit-after-ms 2500`), in 50 separate invocations 1.1 s apart. `spawn-at-verify run` was not used because it refuses a stale binary and building during measurement was not allowed. Previous windows were still alive (2500 ms lifetime), so this is the "another window alive" regime of section 5.
- Result: the milliseconds from the `allocation` `COMMIT_SAMPLE` to the `size-changed` sample, in run order (`none` = no `size-changed` sample before `SIZE_SETTLED`), and the statistics below.
- Status: the numbers and the code reading are CONFIRMED; the mechanism of the late timer is a HYPOTHESIS.

```
32.8, 25.5, 26.7, 21.4, 20.0, 22.2, 19.1, 21.6, 26.8, 23.9, 20.1, 26.8, 21.3, 21.8, 22.9, 22.5, 22.9, 19.7, 28.3, 20.3,
22.8, 21.2, 22.9, 23.7, 19.5, 24.3, 20.7, 22.1, 22.6, none, 28.9, 25.6, 26.3, 23.4, 30.8, 25.1, 20.3, 22.7, 25.9, 22.7,
23.5, 26.8, 26.4, 23.1, 20.5, 20.6, 27.6, 23.5, 19.9, 26.6
```

| Statistic (49 runs with a sample) | Value |
| :--- | :--- |
| min / mean / max | 19.1 / 23.6 / 32.8 ms |
| runs over 30 ms | 2 (32.8 and 30.8; both still resolved `AT_TARGET`) |
| runs within 5 ms of 30 (25 to 35 ms) | 16 of 49 |
| runs within 1 ms of 30 (29 to 31 ms) | 1 |
| runs with no sample before the timer | 1 of 50 (run 30: `SIZE_SETTLED`, window left at 760,410, mis-centred) |

Where the timer starts: CONFIRMED (code).

- In `check()` (`commit.js:68-97`) the `initial` sample starts no timer. The quiet timer is started on the first `allocation` sample whose size is unchanged while the idle timer is still active (`commit.js:93-96`). It runs 30 ms from that allocation sample, not from the initial one.
- In the failing run 30 the allocation sample was at +51.14 ms and `COMMIT_RESOLVED SIZE_SETTLED` at +81.99 ms (30.85 ms later), as the code predicts.

Is about 26 ms safe? **By luck, not by design.** CONFIRMED that it is not enforced; mechanism HYPOTHESIS.

- Nothing in the code orders a pending `size-changed` ahead of an expired timer: whichever the main loop dispatches first wins.
- A sample at 26 ms leaves 4 ms of slack, 16 of 49 samples were in 25 to 35 ms, and two samples at 30.8 and 32.8 ms were still processed before the 30 ms timer fired. That shows the timer callback itself can run late.
- Why it ran late there (a main-loop stall, section 5) is HYPOTHESIS. The one failing run is the case where the sample did not make it.

### 6.6 Regimes

- Question: do the failures of 6.5 depend on whether another window is alive, and does the delay track the post-map Shell stall of section 5?
- Method: the same driver as 6.5 (`--exit-after-ms 800`, a 1.6 s gap so the previous window is gone), 20 runs each. Regime (a) is isolated, with no other window of ours alive. Regime (b) keeps a separate `fixture` window (`KEEPALIVE`, 400 s lifetime) alive throughout.
- Result: the table and the per-run values below, in milliseconds from the allocation sample to `size-changed`.
- Status: the regime dependence is CONFIRMED; the link to the Shell stall is a HYPOTHESIS.

| Regime and size | Runs | `none` (SIZE_SETTLED) | Mis-centred | min / mean / max ms | over 30 ms | 25 to 35 ms |
| :--- | :-: | :-: | :-: | :--- | :-: | :-: |
| (a) isolated, 200x150 | 20 | 2 (runs 11, 12) | 2 | 6.4 / 8.6 / 12.4 | 0 | 0 |
| (a) isolated, 1200x800 | 20 | 3 (runs 1, 2, 3) | 2 | 21.2 / 24.7 / 31.5 | 1 | 5 |
| (b) window alive, 200x150 | 20 | 0 | 0 | 6.1 / 7.5 / 9.9 | 0 | 0 |
| (b) window alive, 1200x800 | 20 | 0 | 0 | 21.5 / 24.3 / 32.0 | 2 | 5 |

Per-run values:
```
(a) 200x150 : 10.3, 11.3, 8.4, 7.0, 7.9, 6.4, 8.9, 6.4, 7.3, 6.5, none, none, 11.5, 8.0, 8.8, 7.7, 11.0, 7.7, 7.3, 12.4
(a) 1200x800: none, none, none, 24.5, 23.4, 25.9, 24.9, 31.5, 24.6, 23.2, 25.6, 22.1, 21.2, 24.7, 23.1, 23.3, 21.3, 23.2, 28.2, 29.1
(b) 200x150 : 8.7, 7.1, 6.9, 7.4, 6.6, 9.9, 6.9, 6.5, 7.0, 7.1, 7.9, 6.4, 7.6, 9.7, 7.5, 6.8, 8.4, 6.1, 7.7, 8.8
(b) 1200x800: 22.4, 23.7, 26.5, 24.5, 22.4, 32.0, 23.6, 23.3, 27.7, 23.5, 23.4, 24.1, 21.6, 30.4, 22.8, 21.5, 25.0, 22.3, 22.1, 22.4
```

- CONFIRMED: failures occur only in the isolated regime (4 mis-centred of 40; 5 runs with no sample before the timer) and not with another window alive (0 of 40). In 1200x800 (a) the three `none` runs are the first three of the batch.
- CONFIRMED: the typical delay does not differ between regimes (1200x800: mean 24.7 vs 24.3 ms; 200x150: 8.6 vs 7.5 ms), and it depends strongly on window size (about 8 ms vs about 24 ms). So the 26 ms is mostly the cost of the client producing a first frame at the new size, not the Shell stall.
- Does the delay track the post-map Shell main-loop stall of section 5? Mostly no; the failures may. HYPOTHESIS. Section 5 measured a 28.5 to 33.9 ms stall in the isolated regime only. The failing runs are in that regime only. A stall of that length would delay the dispatch of a client commit, which normally arrives 6 to 12 ms (200x150) after the allocation sample, past the 30 ms timer. That fits the two 200x150 `none` runs. The probe of section 5 (a `GetCursor` loop) was not repeated here, so the stall and these failures were not shown to coincide.
- Does the 30 ms timer race it? HYPOTHESIS: a stall of about 30 ms means a due timer and a due `size-changed` are dispatched in the same loop iteration, whose order is not controlled by the extension.

### 6.7 What the user must decide

- Question: which decisions about the min-size contract belong to the user?
- Method: the open points of 6.2, 6.3 and 6.4 (the UNDEFINED and BUG rows).
- Result: the six decisions below. All are product changes, and this section recommends none.
- Status: open.

| Decision | Options |
| :--- | :--- |
| Should the anchor wait for the final size? | Re-anchor on any size change until the reactive anchor is armed (close the `UNCLOAK_4C` to `ANCHOR_ARMED` gap), arm the reactive anchor before the reveal wait, or lengthen or replace the 30 ms quiet rule. |
| The "below toolkit minimums" threshold | The INFO uses a static `w < 100 \|\| h < 80` on the request (6.2 item 9). Keep it, align it with the 100x60 floor, or drop it in favour of the observed result. |
| Exit code when the final position is wrong | Keep 0 (current, matches the smoke script) or return non-zero when the WARNING "was not positioned at target" is printed. |
| Oversized requests | Clamp the size to workarea minus margins (blueprint formula) or leave the app to size itself and only warn (current). And where the centre anchor puts an oversized window. |
| Default margin of 16 | Whether to state it in `--help` and the README, where it is undefined today (6.1 row 10). |
| Whether tests may rely on `size_raised`, `raised_w`, `raised_h` | They are computed from the rectangle at the moment spawn-at exits, so they can describe a size that later changes (6.3: json 300x400, final 300x201). |

### 6.8 Behaviours skipped by the test plan as UNDEFINED

- Question: which behaviours does the test plan leave without a spec, and which can have one?
- Method: the classes of 6.4.
- Result: no spec asserts rows 4b, 5, 7, 8 and 10 of 6.4: the static "below toolkit minimums" threshold, the exit code on a mis-positioned window, the size of an oversized window, the centre anchor with an oversized window, and the default margin. The same holds for "which size the anchor uses while the app has not answered" (6.1 row 13). Specs may be written for rows 1, 2, 3, 3b, 4 (as a symptom), 6, 9, 11 and 12 (INTENDED or BUG). Expectations come from the oracle, and the BUG rows stay failing until the product is fixed.
- Status: tests do not assert `size_raised`, `raised_w` and `raised_h`, because they describe the rectangle at spawn-at's exit. They are only recorded.

### 6.9 Not done

- Question: what did this investigation not do?
- Result: four things were not done.
  - The Shell-stall probe of section 5 was not repeated, so the link between the stall and the failures is a HYPOTHESIS.
  - No test was run with a real non-zero minimum reported by the Shell (none exists on GNOME 46).
  - Only zenity (GTK4) and the winit fixture were probed; GTK3, Qt and Electron apps were not.
  - 6.5 and 6.6 were measured with the fixture started through `spawn-at` rather than through `spawn-at-verify run`.
- Status: **not verified**.

### 6.10 Placement resize specs: set, loader rule and live results (2026-10-12)

Moved to [TEST_LOG.md](TEST_LOG.md#610-placement-resize-specs-set-loader-rule-and-live-results-2026-10-12).

### 6.11 Live results after the late-resize fix (2026-10-12)

Moved to [TEST_LOG.md](TEST_LOG.md#611-live-results-after-the-late-resize-fix-2026-10-12).

### 6.11.1 Final-state oracle, post-reveal warning and re-measure (2026-10-12)

Moved to [TEST_LOG.md](TEST_LOG.md#6111-final-state-oracle-post-reveal-warning-and-re-measure-2026-10-12).

### 6.12 Why the window is revealed at its pre-resize size: investigation (2026-10-12, read-only, nothing implemented)

Moved to [TEST_LOG.md](TEST_LOG.md#612-why-the-window-is-revealed-at-its-pre-resize-size-investigation-2026-10-12-read-only-nothing-implemented).

### 6.13 H4: does the cloak starve the app of frame callbacks? (2026-10-12, no extension change)

Moved to [TEST_LOG.md](TEST_LOG.md#613-h4-does-the-cloak-starve-the-app-of-frame-callbacks-2026-10-12-no-extension-change).

### 6.14 Mutter API leads and an "arrival wait" simulation (2026-10-12, read-only, nothing implemented)

Moved to [TEST_LOG.md](TEST_LOG.md#614-mutter-api-leads-and-an-arrival-wait-simulation-2026-10-12-read-only-nothing-implemented).

### 6.15 Flash-aware verdict: audit, ground truth from the client, validation on X11 (2026-10-13, harness only)

Moved to [TEST_LOG.md](TEST_LOG.md#615-flash-aware-verdict-audit-ground-truth-from-the-client-validation-on-x11-2026-10-13-harness-only).

### 6.16 Flash: numeric headline, per-screen visibility label (2026-10-13, harness only)

Moved to [TEST_LOG.md](TEST_LOG.md#616-flash-numeric-headline-per-screen-visibility-label-2026-10-13-harness-only).

### 6.17 BASELINE before the ready-wait change: flash on native Wayland vs XWayland vs X11 (2026-10-13, harness and docs only)

Moved to [TEST_LOG.md](TEST_LOG.md#617-baseline-before-the-ready-wait-change-flash-on-native-wayland-vs-xwayland-vs-x11-2026-10-13-harness-and-docs-only).

### 6.18 Ready-wait: live validation against the 6.17 baseline (2026-10-13, no code change)

Moved to [TEST_LOG.md](TEST_LOG.md#618-ready-wait-live-validation-against-the-617-baseline-2026-10-13-no-code-change).

### 6.19 Ready-wait: same-session A/B/C with the per-spawn override (2026-10-13, no code change)

Moved to [TEST_LOG.md](TEST_LOG.md#619-ready-wait-same-session-abc-with-the-per-spawn-override-2026-10-13-no-code-change).

### 6.20 Why arms B and C still flash under load: classification and a protocol-level re-run (2026-10-13, no code change)

Moved to [TEST_LOG.md](TEST_LOG.md#620-why-arms-b-and-c-still-flash-under-load-classification-and-a-protocol-level-re-run-2026-10-13-no-code-change).

### 6.21 Default cap 250 ms and the wire paint time: code, tests and a traced loaded A/B/C (2026-10-13)

Moved to [TEST_LOG.md](TEST_LOG.md#621-default-cap-250-ms-and-the-wire-paint-time-code-tests-and-a-traced-loaded-abc-2026-10-13).

### 6.22 Hostile apps: fixture flags, nine catalog cells and a live A/C run (2026-10-13)

Moved to [TEST_LOG.md](TEST_LOG.md#622-hostile-apps-fixture-flags-nine-catalog-cells-and-a-live-ac-run-2026-10-13).

### 6.23 Post-reinstall live check: default cap, A/D regression, XWayland signal study (2026-10-13)

Moved to [TEST_LOG.md](TEST_LOG.md#623-post-reinstall-live-check-default-cap-ad-regression-xwayland-signal-study-2026-10-13).

### 6.24 Texture reader live check and texture size as a signal (2026-10-13)

Moved to [TEST_LOG.md](TEST_LOG.md#624-texture-reader-live-check-and-texture-size-as-a-signal-2026-10-13).

### 6.25 X11 investigation, stage 1: capture rig, hold-the-cloak experiment, ping (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#625-x11-investigation-stage-1-capture-rig-hold-the-cloak-experiment-ping-2026-10-14).

### 6.26 x11-hold-damage, live A/B on the XWayland fixture (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#626-x11-hold-damage-live-ab-on-the-xwayland-fixture-2026-10-14).

### 6.27 X11 pre-map resize and pixel probe, stage 1 (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#627-x11-pre-map-resize-and-pixel-probe-stage-1-2026-10-14).

### 6.28 x11-pixel-probe and x11-premap, live on the XWayland fixture (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#628-x11-pixel-probe-and-x11-premap-live-on-the-xwayland-fixture-2026-10-14).

### 6.29 x11-pixel-probe, live retry with the Mtk.Rectangle fix (2026-10-14): stopped after phase 1, the Shell crashed in the batch

Moved to [TEST_LOG.md](TEST_LOG.md#629-x11-pixel-probe-live-retry-with-the-mtkrectangle-fix-2026-10-14-stopped-after-phase-1-the-shell-crashed-in-the-batch).

### 6.30 Regression after the removal of the X11 experiments (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#630-regression-after-the-removal-of-the-x11-experiments-2026-10-14).

### 6.31 X sync counter (`_NET_WM_SYNC_REQUEST_COUNTER`) as a client-side ready signal: feasibility (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#631-x-sync-counter-_net_wm_sync_request_counter-as-a-client-side-ready-signal-feasibility-2026-10-14).

### 6.32 A sync-capable X11 client against the winit fixture, legacy rule (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#632-a-sync-capable-x11-client-against-the-winit-fixture-legacy-rule-2026-10-14).

### 6.33 Does the X sync counter move when a client repaints? Live check with the GTK4 client (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#633-does-the-x-sync-counter-move-when-a-client-repaints-live-check-with-the-gtk4-client-2026-10-14).

### 6.34 Answering a sync request that we send ourselves: smoke (2026-10-14) (stopped after the smoke because counter A never moved)

Moved to [TEST_LOG.md](TEST_LOG.md#634-answering-a-sync-request-that-we-send-ourselves-smoke-2026-10-14-stopped-after-the-smoke-because-counter-a-never-moved).

### 6.35 Born at the target geometry: an LD_PRELOAD shim on the test client (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#635-born-at-the-target-geometry-an-ld_preload-shim-on-the-test-client-2026-10-14).


## 7. Sway backend (EXPERIMENTAL)

Design, measurements and the list of decisions: [DESIGN_SWAY.md](DESIGN_SWAY.md). Nothing in this section touches GNOME; the Sway backend is only reachable with `SPAWN_AT_BACKEND=sway`.

### 7.1 Unit and integration tests (run in CI, no Sway needed)

| Group | Where | What it pins |
| :-- | :-- | :-- |
| Frame codec | `platform/linux/sway/ipc.rs` | Golden bytes read from a private headless Sway 1.9 (request frames, subscribe/command/workspaces replies, a `window` event), limits and errors. |
| JSON and conversions | `platform/linux/sway/types.rs`, `tests/fixtures/sway/` | Layout of one output, of a scale-2 output with a 30 px bar and of two outputs; window metadata of a tiled and a floating window; scratchpad, XWayland and fullscreen cases; the four recorded `window` events; reply checking. |
| Command text, matching | `sway/command.rs`, `sway/matching.rs` | The exact `RUN_COMMAND` strings; pid, descendant pid and app id rules, the 300 ms grace, never a window from before `arm`. |
| Driver against a scripted socket | `sway/mod.rs`, `sway/tests_write.rs`, `sway/fake.rs` | Layout and windows over the socket; `arm` then `post_spawn` placing with each anchor family; no `resize` without `--size`; border and title bar corrected once; a window raised to its minimum re-anchored with its real size; close, focus, transform; unsupported operations say so. |
| Window commands through the binary | `tests/sway_backend_cli_test.rs` (19 tests) | The built binary against a recording stand-in socket: `focus` and `close` send `[con_id=5] focus` / `kill`; `transform` sends the place command (and a `focus` unless `--no-focus`); an unknown id exits 1 and sends nothing; `query layout\|windows --json`; `defocus`, `maximize`, `minimize`, `restore` and `query pointer` exit 1 with "does not support this feature" and send nothing. |
| Selection | `platform/linux/registry.rs`, `tests/sway_backend_cli_test.rs` | Only the forced value `sway` (any case, nothing trimmed) selects Sway; the old-selection equivalence test is untouched; forced Sway without a usable `$SWAYSOCK` exits 1 with a message naming it and never falls back to X11; the `sway` desktop alone does not connect to the socket. |

### 7.2 Live cases: `scripts/sway-live.sh`

Runs two private headless Sway sessions through `scripts/sway-headless.sh` with the repo's fixture window (a native Wayland client), compares everything with `swaymsg`, closes each window by its recorded id and checks that no process or directory is left. The last recorded run was on 2026-10-15, on Sway 1.9 on one machine, with the GNOME session running and `gnome-shell` keeping its PID:

```text
 passed: 62   failed: 0
```

| Session | Cases |
| :-- | :-- |
| A: one 1920x1080 output | `query layout` equals `swaymsg`; center; top-right margin 24; bottom-right margin 16 with size 30x20 (Sway raises it to 75x50 and the window is placed with that size: 1829,1014); bottom-right with 600x400; top-left 500x300 with `--no-wait`; `transform` to bottom-right 700x500 (1195,555); close by id and by class; forced Sway with `SWAYSOCK` unset, empty and missing exits 1 naming `$SWAYSOCK`. Every position within 1 px of the expectation computed from the work area `swaymsg` reports; `spawn --json` `window_id` equals the container id; `query windows` equals `swaymsg`. |
| B: scale 2, 30 px bar, second output at x=960 | `query layout` equals `swaymsg` (logical pixels, work areas start at y=30); center on monitor 0; bottom-right with size 400x300 and top-left on monitor 1. |

The border and title bar case (`--size 600 400` gives a container of exactly 600x400) is in [DESIGN_SWAY.md](DESIGN_SWAY.md), section 4.

### 7.3 Not verified

- A real Sway session or real applications.
- XWayland windows.
- Sway versions other than 1.9, and i3.
- Frame timing before the first paint, that is, whether a window shows at its tiled position first.
- `--area screen` and `--area unbounded`.
- Clients that map at a different size than the one asked.
- Fractional scales other than 1 and 2, more than two outputs, and output names other than `HEADLESS-*`.
- `scripts/sway-live.sh` under `shellcheck`. It was not installed on the author's machine; `bash -n` passes.

### 7.4 The verify runner on a private headless Sway (2026-10-15)

`scripts/verify-sway.sh` (design: [DESIGN_RUNNER_SETUP.md](DESIGN_RUNNER_SETUP.md) section 4.3) starts a private headless Sway, runs `spawn-at-verify run` against it, and stops it. It ran on Sway 1.9 with seven throwaway version 2 specs, kept in `scratch/` and not in the catalog:

- `query windows` lists the fixture.
- Resize by id with `[expect.rect]`.
- Focus the first of two windows (`focused` and `unfocused` checks).
- Close by id (`gone` and `process_exited` checks).
- Defocus is unsupported (exit 1 and the "does not support this feature" text).
- A nonexistent id.
- One spec for GNOME only.

Result: 6 passed, 1 skipped with `unsupported on this backend (sway): spec requires gnome`, 0 failed, 0 blocked. The private Sway and its directory were removed and no fixture process was left. A `SIGTERM` sent to the wrapper alone, during `--repeat 30`, ended the run with exit code 130 after "Interrupted: 1 of the selected tests ran", stopped Sway, and left nothing. During the runs `gnome-shell` kept its PID and the user journal held no line mentioning spawn-at.

Not covered: real Sway, other Sway versions, outputs other than one 1920x1080 `HEADLESS-1`, the `Both` kind (it needs a person), and flash and trace checks (Sway has neither).

### 7.5 The catalog specs for an existing window on a private headless Sway (2026-10-15)

`scripts/verify-sway.sh` with its default selection (the `headless-safe` tag) on Sway 1.9: 15 passed, 0 failed, 0 skipped, 0 blocked. The 15 are:

- `focus/activate-fixture` and `focus/switch-between-windows`
- `queries/query-layout-json` and `queries/query-windows-lists-fixture`
- `robustness/close-by-id` and `robustness/nonexistent-window-id`
- `robustness/sway-defocus-unsupported`, `robustness/sway-maximize-unsupported` and `robustness/sway-query-pointer-unsupported`
- `transforms/resize-by-id` and the five `transforms/anchor-corners/*`

With `-- --tag existing-window` all 22 entries ran: the same 15 passed and 7 were Skipped with `unsupported on this backend (sway): spec requires gnome`. The 7 are `focus/defocus-active-window`, `queries/query-pointer`, `transforms/maximize-and-restore`, `minimize-and-restore`, `move-flash`, `reposition-by-id` and `resize-flash`.

`transforms/reposition-by-id` is GNOME-only. On Sway a fresh tiled window is as large as the work area, so a position-only transform keeps 1920x1080, not the fixture's 400x300. `gnome-shell` kept its PID and the user journal held no line mentioning spawn-at. **None of the GNOME specs has been run**; the Both specs need a person. **None of the GNOME specs has been run**; the Both specs need a person.
