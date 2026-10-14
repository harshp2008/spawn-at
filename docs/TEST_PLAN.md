# spawn-at Test Library & Verification Plan

## 1. Overview & Architecture

`spawn-at-verify` is the dedicated verification engine and test harness for the `spawn-at` window placement CLI and GNOME Shell extension. It validates zero-flicker window creation, exact geometry placement against a mathematical oracle, and timeline event sequencing via compositor trace analysis.

Tests are defined declaratively in `verify/<category>/<test-id>.toml` specifications, executed against a real Wayland GNOME session, and presented via both an automated CLI runner and a headless/interactive Terminal User Interface (TUI).

### 1.1 Test Suite Counts & Feature Gates Baseline

Automated test execution across the workspace is partitioned by crate and feature flags:

| Target / Feature Set | Tests Passed | Tests Ignored | Breakdown by Target / Binary |
| :--- | :---: | :---: | :--- |
| **Workspace: `spawn-at` CLI** | 205 | 0 | CLI flag parsing, IPC client, fallback, and reposition tests; 6 added for extension protocol 6: 3 `--json` `resize` shape/compatibility tests (`commands/spawn.rs`) and 3 claim tests against protocol 5 and 6 stub services on the session bus (`platform/linux/gnome/mod.rs`); 11 added for the per-spawn wait override: 4 flag tests (`cli/tests.rs`: parse before the command, default absent, bad text still parses, hidden from short and long help), 3 `run_spawn` tests with a recording backend (`commands/spawn.rs`: flags reach the armed entry, no flags leave no override and no `wait` key, bad values dropped with one warning each) and 4 instruction tests (`platform/linux/gnome/mechanics.rs`: default bytes unchanged, override fields only when set, independent fields, old JSON still reads); 16 added for `spawn-at config` (docs/CONFIG.md): 10 in `tests/config_cli_test.rs` (the built binary with a temporary `XDG_CONFIG_HOME`), 3 `run_spawn` tests (`commands/spawn.rs`: default instruction bytes unchanged, file values reach the instruction and flags win, a bad config warns and the spawn goes on), 1 `config.rs` (the update checker keeps settings and unknown keys) and 2 `platform/linux/config_store.rs`; 8 added for the untested-platform install notice: 4 unit tests in `platform/install_notice.rs` (46 on Wayland silent, 42-44 legacy notice, other versions and X11, the confirmation gate) and 4 in `tests/install_notice_cli_test.rs` (the built binary with a stand-in `gnome-shell`, `install --dry-run --headless`; they return early on a machine without a GNOME session) |
| **Workspace: `spawn-at-core`** | 64 | 0 | 60 unit tests (`lib.rs`: 21 geometry/driver + 9 `ready_wait` + 4 `resize_report` + 9 `wait_override` + 17 `settings`) + 2 golden placement integration tests (`tests/golden_placement.rs`) + 2 `tests/ready_wait_vectors.rs` (20 shared golden scripts in one test, plus the policy check) |
| **Workspace: `xtask`** | 10 | 0 | 8 unit tests (`src/main.rs`: 6 `burn_cpu`, 2 `compiler_docs`) + 2 `burn_cpu_test` (`tests/burn_cpu_test.rs`, runs the built binary) |
| **Workspace: Node.js (Extension)** | 176 | 0 | 9 in `tests/wait_override_test.js` (the validator's reading of the per-spawn override: absent adds nothing, `auto` adds nothing, bounds 1 to 1500, bad values named and cut to 32 characters, legacy validator drops the fields) and 11 more in `tests/ready_wait_wiring_test.js` (44 there now: override honoured, invalid ignored, default unchanged line for line, scoped to one spawn on one extension instance); the rest as before: Golden anchor math, extension characterization, legacy/modern dispatchers, late-resize re-anchor (`tests/late_resize_test.js`), 31 in `tests/ready_wait_test.js` (the same 20 golden scripts one test each, the presence and policy checks, 8 mirrored unit cases, 1 portability check), and 33 of the 44 in `tests/ready_wait_wiring_test.js` (the 20 golden scripts again through `CommitLatch.waitForReady` on a virtual clock, plus 13 extension scenarios through `_runBatch`; the other 11 are the override tests above) (`node --test tests/*.js`); 17 added for the observe-only X11 logging (DESIGN_READY_WAIT section 12): 11 in `tests/x11_observe_test.js` (formats, default-off does no work, handler set empty after the window ends, damaged unavailable noted once) and 6 in `tests/ready_wait_wiring_test.js` (the same through the extension: logging off adds no handler or line, logging on gives the joinable lines and the same outcome, disable ends a watch) |
| **`spawn-at-verify` (bare default)** | 276 | 0 | 205 lib + 0 main + 5 `category_library_test` + 7 `cli_exit_codes_test` + 20 `flash_test` + 5 `oracle_cross_check` + 1 `preflight_tools_test` + 21 `trace_analyser_test` + 6 `trace_state_test` + 6 `trace_window_scope_test`. The 6 lib tests and the `render_test` and `tui_test` binaries that need `feature = "tui"` are not compiled. |
| **`spawn-at-verify --features tui`** | 378 | 0 | 211 lib + 0 main + 5 `category_library_test` + 7 `cli_exit_codes_test` + 20 `flash_test` + 5 `oracle_cross_check` + 1 `preflight_tools_test` + 13 `render_test` + 21 `trace_analyser_test` + 6 `trace_state_test` + 6 `trace_window_scope_test` + 83 `tui_test` |
| **`spawn-at-verify --features tui,fixture`** | 404 | 4 | 211 lib + 22 fixture binary (`src/bin/fixture.rs`) + 0 main + 5 `category_library_test` + 7 `cli_exit_codes_test` + 4 `fixture_test` (+ 4 ignored) + 20 `flash_test` + 5 `oracle_cross_check` + 1 `preflight_tools_test` + 13 `render_test` + 21 `trace_analyser_test` + 6 `trace_state_test` + 6 `trace_window_scope_test` + 83 `tui_test` |
| **Total (Rust crates, `spawn-at-verify` counted with `tui,fixture`)** | 683 | 4 | 205 + 64 + 10 + 404; with the 176 Node tests: 859 passed |

The three `spawn-at-verify` rows and the total were re-measured on 2026-10-13 at commit `1de817f` (`cargo test -p spawn-at-verify` bare, `--features tui`, `--features tui,fixture`, summed with `awk` over the `test result:` lines; `cargo test --workspace` 427 = 158 + 23 + 10 + 236); 0 failures. The `spawn-at-core` (34), `spawn-at` (158), `xtask` (10) and Node (101) rows were re-measured on 2026-10-13 at commit `902dff3` after the ready-wait core and its JS port (section 8 of `DESIGN_READY_WAIT.md`): `cargo test --workspace` 438 = 158 + 34 + 10 + 236, `node --test tests/*.js` 101 pass, 0 failures; the verify rows did not change (352 / 338 / 236). A bare `cargo test --workspace` builds `spawn-at-verify` without features, so its number for that crate is the 236 row, not 352; do not quote it for the verify crate. The `spawn-at` (164), `spawn-at-core` (38), `xtask` (10) and Node (134) rows and the total were re-measured on 2026-10-13 after the ready-wait wiring (section 8 and 9 of `DESIGN_READY_WAIT.md`): `cargo test --workspace --features spawn-at-verify/tui,spawn-at-verify/fixture` 564 passed, 4 ignored (164 + 38 + 352 + 10), `node --test tests/*.js` 134 pass, 0 failures; the `spawn-at-verify` count with `tui,fixture` is unchanged at 352 (the bare 236 and `tui` 338 rows were not re-run). Baseline before section 6.15 (`7400e57`): 305 with `tui,fixture`; section 6.15 took it to 341 (`1da549d`), section 6.16 to 352.

**Re-measured 2026-10-13 after the per-spawn wait override** (section 10 of `DESIGN_READY_WAIT.md`; commits `44ee6b7`, `a1bb985`, `8edebca`, `80ef74a`), all on the new tree, 0 failures: `cargo test -p spawn-at-verify` 252 (was 236), `--features tui` 354 (338), `--features tui,fixture` 368 passed and 4 ignored (352); `spawn-at` 175 (164), `spawn-at-core` 47 (38), `xtask` 10 (10); `cargo test -p spawn-at -p spawn-at-core -p xtask` plus the `tui,fixture` run summed per binary with `scratch/counts.sh`: **600 passed, 4 ignored**; `node --test tests/*.js` 154 (134). The 16 new verify tests are 9 in `trace/wait.rs`, 4 in `runner/tests.rs`, 1 in `report/tests.rs` and 2 in `cli_exit_codes_test`. Not re-run: a bare `cargo test --workspace` number (it builds `spawn-at-verify` without features; do not quote it for that crate). Nothing here ran in a GNOME Shell.

**Re-measured 2026-10-13 after the default cap 250 and the wire paint time** (commits `e9d5c77`, `8d65a48`; section 6.21), 0 failures: `cargo test -p spawn-at-verify` 265 (was 252), `--features tui` 367 (354), `--features tui,fixture` 381 passed and 4 ignored (368); `spawn-at` 175, `spawn-at-core` 47, `xtask` 10 unchanged; `node --test tests/*.js` 154 unchanged. Total with `tui,fixture`: 613 Rust, 767 with node. The 13 new verify tests are 5 in `trace/wire.rs`, 6 in `runner/wire_paint_tests.rs`, 1 in `report/tests.rs` and 1 in `cli_exit_codes_test`. Summed per binary from the `test result:` lines; the bare `cargo test --workspace` (497, default features) is not quoted for `spawn-at-verify`.

**Re-measured 2026-10-13 after `spawn-at config`** (`docs/CONFIG.md`), 0 failures, summed per binary from the `test result:` lines: `spawn-at` 191 (was 175), `spawn-at-core` 64 (47), `xtask` 10; `spawn-at-verify` bare 267 (265), `--features tui` 369 (367), `--features tui,fixture` 383 passed and 4 ignored (381; the 2 new tests are in `no_config.rs`); node 154 unchanged. Total with `tui,fixture`: 648 Rust, 802 with node. No new live section: the one live check (a harness run and a direct spawn under a user config of `cap_ms = 7`) is recorded in `docs/CONFIG.md`.

**Re-measured 2026-10-13 after the untested-platform install notice**, 0 failures, summed per binary from the `test result:` lines: `spawn-at` 199 (was 191), `spawn-at-core` 64, `xtask` 10, `spawn-at-verify` `tui,fixture` 383 passed and 4 ignored (bare 267, `tui` 369); node 154. Total with `tui,fixture`: 656 Rust, 810 with node. Run here: GNOME Shell 46.0 on Wayland, where the real `install --dry-run --headless` prints no notice; the 42-44, 45 and 47 cases run against a stand-in `gnome-shell` on `PATH`, never on a real GNOME of those versions.

**Re-measured 2026-10-13 after the observe-only X11 logging**, 0 failures, summed per binary: `spawn-at` 199, `spawn-at-core` 64, `xtask` 10 (unchanged); `spawn-at-verify` bare 269 (was 267), `--features tui` 371 (369), `--features tui,fixture` 385 passed and 4 ignored (383; the 2 new tests are in `log_source.rs` and `runner/wire_paint_tests.rs`); node 171 (154). Total with `tui,fixture`: 658 Rust, 829 with node. Nothing ran in a GNOME Shell; the extension code is exercised against mocks only.

**Re-measured 2026-10-13 after the autostart path fix and the texture reader** (commits `f77b415`, `dddc95e`), 0 failures, summed per binary from the `test result:` lines: `spawn-at` 202 (was 199; 3 new in `config.rs`: absolute `Exec=` path, quoting of a path with a space, `$` and `"`, refusal of a relative path), `spawn-at-core` 64, `xtask` 10 (unchanged); `spawn-at-verify` `--features tui,fixture` 402 passed and 4 ignored (unchanged; bare and `tui` rows not re-run); node 176 (was 171; 5 new in `tests/x11_observe_test.js` for `readTextureSize` and its log fields). Total with `tui,fixture`: 678 Rust, 854 with node. Nothing ran in a GNOME Shell.

**Re-measured 2026-10-13 after the hostile-app fixture flags and cells** (commit `70dc120`, section 6.22), 0 failures, summed per binary: `spawn-at` 199, `spawn-at-core` 64, `xtask` 10 (unchanged); `spawn-at-verify` bare 276 (was 269), `--features tui` 378 (371), `--features tui,fixture` 402 passed and 4 ignored (385); node 171 (unchanged). The 17 new tests: 10 in the fixture binary (7 `fixture/behaviour.rs`, 3 flag parsing), 6 in `runner/resize_expect_tests.rs`, 1 in `category_library_test`. Total with `tui,fixture`: 675 Rust, 846 with node.

#### `fixture_test` Ignored Tests Documentation
The 4 tests marked `#[ignore]` in `crates/spawn-at-verify/tests/fixture_test.rs` require a live graphical display session (compositor) to execute:
1. `test_fixture_clean_exit_timeout`: Opens a live test window with softbuffer/winit event loop and verifies timing-based self-termination. Requires an active Wayland compositor or X11 display server.
2. `test_fixture_print_ready_output`: Spawns the GUI fixture process and waits for synchronization handshake token `[fixture] READY` emitted once the window surface renders. Requires live display server connection.
3. `test_fixture_signal_termination`: Spawns a live graphical test window and tests asynchronous SIGTERM handling during the active event loop. Requires live display server connection.
4. `test_fixture_size_matches_query_windows_on_all_protocols`: Spawns GUI test windows across Wayland and X11 backends and invokes `spawn-at query windows` via D-Bus to cross-check reported dimensions against requested frame bounds. Requires active GNOME Shell compositor running the `spawn-at` extension.

---

## 2. 59-Case Test Catalog across 8 Categories

The 59 rows are the planned catalogue: 36 are **Covered** (a spec file exists and `spawn-at-verify list` shows it: 36 tests) and 23 are **Planned** (no spec yet). The last 18 Covered rows are the placement resize specs of section 6.10. Since 6.22 `spawn-at-verify list` shows 45 tests: these 36 plus the nine `hostile/` cells, which are not rows of the 59.

| ID | Title | Mode | Status | Spec / Test Path |
| :--- | :--- | :---: | :---: | :--- |
| **Category: basics** | | | | |
| `preflight/binary-head` | Binary version matches repository HEAD | Auto | **Covered** | `verify/preflight/binary-head.toml` |
| `basics/version-and-help` | CLI `--help` and `--version` formatted output | Auto | **Planned** | `verify/basics/version-and-help.toml` |
| `basics/instant-paint` | Instant paint without artificial delay | Auto | **Planned** | `verify/basics/instant-paint.toml` |
| `basics/shell-extension-active` | Extension IPC handshake and version ping | Auto | **Planned** | `verify/basics/shell-extension-active.toml` |
| **Category: placement** | | | | |
| `placement/bottom-right` | Window placed in bottom-right corner with margins | Both | **Covered** | `verify/placement/bottom-right.toml` |
| `placement/top-left` | Window placed at top-left anchor (400x300, margin 16) | Both | **Covered** | `verify/placement/top-left.toml` |
| `placement/top` | Window placed at top edge (400x300, margin 16) | Auto | **Covered** | `verify/placement/top.toml` |
| `placement/top-right` | Window placed at top-right anchor (400x300, margin 16) | Both | **Covered** | `verify/placement/top-right.toml` |
| `placement/left` | Window placed at left edge (400x300, margin 16) | Auto | **Covered** | `verify/placement/left.toml` |
| `placement/right` | Window placed at right edge (400x300, margin 16) | Auto | **Covered** | `verify/placement/right.toml` |
| `placement/bottom-left` | Window placed at bottom-left anchor (400x300, margin 16) | Both | **Covered** | `verify/placement/bottom-left.toml` |
| `placement/bottom` | Window placed at bottom edge (400x300, margin 16) | Auto | **Covered** | `verify/placement/bottom.toml` |
| `placement/center` | Window centered inside display workarea (400x300, margin 16) | Auto | **Covered** | `verify/placement/center.toml` |
| `placement/margin-0` | Bottom-right anchor with 0 px margin | Both | **Covered** | `verify/placement/margin-0.toml` |
| `placement/margin-64` | Bottom-right anchor with 64 px margin | Auto | **Covered** | `verify/placement/margin-64.toml` |
| `placement/center-200x150` | 200x150 window centered (**flaky live**: fails in about 2 of 5 runs, see 2.1) | Both | **Covered** | `verify/placement/center-200x150.toml` |
| `placement/center-1200x800` | 1200x800 window centered (**flaky live**: fails in about 1 of 6 runs, see 2.1) | Auto | **Covered** | `verify/placement/center-1200x800.toml` |
| `placement/bottom-right-smaller` | 200x150 at bottom-right, fixture minimum 100x100 (real shrink) | Both | **Covered** | `verify/placement/bottom-right-smaller.toml` |
| `placement/bottom-right-larger` | 800x600 at bottom-right, fixture minimum 100x100 (real grow) | Auto | **Covered** | `verify/placement/bottom-right-larger.toml` |
| `placement/bottom-right-minsize-at-min` | exactly the fixture minimum 300x200, bottom-right | Auto | **Covered** | `verify/placement/bottom-right-minsize-at-min.toml` |
| `placement/bottom-right-below-min-width` | 200x300 vs minimum 300x200 -> final 300x300, bottom-right | Auto | **Covered** | `verify/placement/bottom-right-below-min-width.toml` |
| `placement/bottom-right-below-min-height` | 400x150 vs minimum 300x200 -> final 400x200, bottom-right | Auto | **Covered** | `verify/placement/bottom-right-below-min-height.toml` |
| `placement/bottom-right-below-min-both` | 200x150 vs minimum 300x200 -> final 300x200, bottom-right | Both | **Covered** | `verify/placement/bottom-right-below-min-both.toml` |
| `placement/bottom-right-far-below-min` | 50x50 vs minimum 300x200 -> final 300x200, bottom-right | Auto | **Covered** | `verify/placement/bottom-right-far-below-min.toml` |
| `placement/bottom-left-smaller` | 200x150 at bottom-left | Auto | **Covered** | `verify/placement/bottom-left-smaller.toml` |
| `placement/bottom-left-below-min-both` | 200x150 vs minimum 300x200 at bottom-left | Auto | **Covered** | `verify/placement/bottom-left-below-min-both.toml` |
| `placement/top-right-smaller` | 200x150 at top-right | Auto | **Covered** | `verify/placement/top-right-smaller.toml` |
| `placement/top-right-below-min-both` | 200x150 vs minimum 300x200 at top-right | Auto | **Covered** | `verify/placement/top-right-below-min-both.toml` |
| `placement/bottom-smaller` | 200x150 at the bottom edge | Auto | **Covered** | `verify/placement/bottom-smaller.toml` |
| `placement/bottom-below-min-both` | 200x150 vs minimum 300x200 at the bottom edge | Auto | **Covered** | `verify/placement/bottom-below-min-both.toml` |
| `placement/right-smaller` | 200x150 at the right edge | Auto | **Covered** | `verify/placement/right-smaller.toml` |
| `placement/right-below-min-both` | 200x150 vs minimum 300x200 at the right edge | Auto | **Covered** | `verify/placement/right-below-min-both.toml` |
| `placement/top-left-below-min-both` | control: 200x150 vs minimum 300x200 at top-left (position independent of size) | Auto | **Covered** | `verify/placement/top-left-below-min-both.toml` |
| `placement/top-left-smaller` | 200x150 at top-left, fixture minimum 100x100 (real shrink; the window's origin does not move) | Auto | **Covered** | `verify/placement/top-left-smaller.toml` |
| `placement/invalid-size-zero` | expected failure: `--size 0 150` is rejected by the CLI (exit 2, "Invalid size") | Auto | **Covered** | `verify/placement/invalid-size-zero.toml` |
| `placement/area-screen-vs-workarea` | Boundary bounds: screen vs workarea vs unbounded | Auto | **Planned** | `verify/placement/area-screen-vs-workarea.toml` |
| `placement/clamp-bounds` | Clamping window when position exceeds area | Auto | **Planned** | `verify/placement/clamp-bounds.toml` |
| `placement/explicit-pos` | Explicit coordinates `--pos X Y` with custom pivot | Auto | **Planned** | `verify/placement/explicit-pos.toml` |
| **Category: transforms** | | | | |
| `transforms/reposition-by-id` | Reposition existing window by numeric ID | Auto | **Planned** | `verify/transforms/reposition-by-id.toml` |
| `transforms/maximize-and-restore` | Maximize window and restore original dimensions | Both | **Planned** | `verify/transforms/maximize-and-restore.toml` |
| `transforms/minimize-and-focus` | Minimize window and unminimize via focus | Both | **Planned** | `verify/transforms/minimize-and-focus.toml` |
| `transforms/resize-by-id` | Resize existing window dimensions | Auto | **Planned** | `verify/transforms/resize-by-id.toml` |
| **Category: focus** | | | | |
| `focus/activate-by-title` | Activate window matching title substring | Auto | **Planned** | `verify/focus/activate-by-title.toml` |
| `focus/defocus-active-window` | Relinquish keyboard focus to previous window | Auto | **Planned** | `verify/focus/defocus-active-window.toml` |
| `focus/passive-spawn-no-focus` | Spawn window passively without stealing focus (`--no-focus`) | Auto | **Planned** | `verify/focus/passive-spawn-no-focus.toml` |
| `focus/switch-between-windows` | Alternate focus across two managed test windows | Auto | **Planned** | `verify/focus/switch-between-windows.toml` |
| **Category: animation-and-cloak** | | | | |
| `cloak/delayed-paint/delay-0` | App paints slowly (0 ms delay) | Auto | **Covered** | `verify/cloak/delayed-paint/delay-0.toml` |
| `cloak/delayed-paint/delay-50` | App paints slowly (50 ms delay) | Auto | **Covered** | `verify/cloak/delayed-paint/delay-50.toml` |
| `cloak/delayed-paint/delay-150` | App paints slowly (150 ms delay) | Auto | **Covered** | `verify/cloak/delayed-paint/delay-150.toml` |
| `cloak/delayed-paint/delay-500` | App paints slowly (500 ms delay) | Auto | **Covered** | `verify/cloak/delayed-paint/delay-500.toml` |
| `animation/minimize-animation` | Minimize/unminimize animation flicker regression | Both | **Planned** | `verify/animation/minimize-animation.toml` |
| **Category: queries** | | | | |
| `queries/query-layout` | Query display monitor and workarea layout | Auto | **Planned** | `verify/queries/query-layout.toml` |
| `queries/query-windows` | Query active compositor windows with metadata | Auto | **Planned** | `verify/queries/query-windows.toml` |
| `queries/query-pointer` | Query current mouse pointer screen coordinates | Auto | **Planned** | `verify/queries/query-pointer.toml` |
| **Category: robustness** | | | | |
| `robustness/invalid-anchor-arg` | Clean error report on invalid anchor CLI value | Auto | **Planned** | `verify/robustness/invalid-anchor-arg.toml` |
| `robustness/app-crash-early-exit` | Graceful cleanup when spawned process crashes during cloak | Auto | **Planned** | `verify/robustness/app-crash-early-exit.toml` |
| `robustness/nonexistent-window-id` | Transform non-existent window ID returns clean error | Auto | **Planned** | `verify/robustness/nonexistent-window-id.toml` |
| **Category: platform** | | | | |
| `platform/x11-fallback` | Platform guard and fallback under X11 session | Auto | **Planned** | `verify/platform/x11-fallback.toml` |
| `platform/multi-monitor-secondary` | Target placement to secondary monitor index | Both | **Planned** | `verify/platform/multi-monitor-secondary.toml` |

---

### 2.1 Known flaky: placement size variants (investigated 2026-10-12)

**Status after the late-resize fix (commit `9e6b65d`, live-measured later the same day): see 6.11.** Idle: 0 failures in 640 runs of the 16 resize specs. Under all-CPU load: still failing (212 of 320 passed), by a different mechanism than the one described below (the resize arrives after the reveal, not between `UNCLOAK_4C` and `ANCHOR_ARMED`). The text below is the pre-fix investigation and is left as written.

`placement/center-200x150` and `placement/center-1200x800` are the first specs that force a real resize: the fixture opens at its default 400x300 (`--width`/`--height` default to 400/300, `crates/spawn-at-verify/src/bin/fixture.rs:46-51`, used for the initial size at `bin/fixture/window.rs:48,61`) and neither spec passes `--width`/`--height` (`verify/placement/center-200x150.toml`, `center-1200x800.toml`). Nothing was changed to investigate: no product code, spec, fixture or budget. The specs stay failing when they fail. Each claim is labelled **CONFIRMED** (shown by a log, a count or a line of code) or **HYPOTHESIS**.

**Symptom (CONFIRMED).** On a failing run spawn-at logs `[spawn-at] WARNING: Window mapped at (760, 410) [WxH] but expected position was (X, Y)`. (760, 410) is where a 400x300 window is centred in the workarea `0,40 1920x1040`. The window does end up WxH (the `UNCLOAK` event shows `frame=(760,410,WxH)`), it just keeps the top-left chosen for 400x300. Recorded failing run: `crates/spawn-at-verify/tests/fixtures/real_placement_center_200x150_fail_run.json`.

**Frequency (CONFIRMED, counts from `verify-results/`).**

| Condition | center-200x150 | center-1200x800 |
| :--- | :---: | :---: |
| Idle, `run --repeat 20` (all 20 runs stored, binary at HEAD) | 0/20 fail | 0/20 fail |
| Idle, 20 separate `run` invocations 1.1 s apart | 0/20 fail | 0/20 fail |
| All 22 CPUs busy (22 `while :; do :; done` loops), 20 separate invocations | **12/20 fail** | not run |
| The 03:16-03:17 batch that first showed the failures (stored reports only; same-second runs overwrote each other before the directory-name fix, so the real run count was higher) | 3 fails in 7 stored runs | 2 fails in 7 stored runs |

All failing runs show the same signature (`COMMIT_RESOLVED reason=SIZE_SETTLED finalFrame=(400x300)`, `APPLY_ANCHOR eff=(400x300)`, final `760,410`). So the failure rate is not a constant 30%: it was 0/80 in the first idle batches and 12/20 with every CPU busy. **Corrected later (section 6.0, 6.5, 6.6): idle runs also fail** (1 of 50 with another window alive; 4 of 40 with no other window), so the "0/80 at idle" above is not a safe rate. That CPU load can cause it is **CONFIRMED** for this machine (same binary, same spec, only the load changed): load is a sufficient trigger, not the only one. That the earlier 03:16-03:17 failures were also caused by load is **HYPOTHESIS** (they all fall inside about 70 s, and nothing else about the machine was recorded).

**Where a failing run and a passing run diverge (CONFIRMED, window-scoped events, ms from the window's first event).** `center-200x150`, failing (`verify-results/20261012-031711-harsh`) against passing:

| Event | Failing | Passing |
| :--- | :--- | :--- |
| `SET_SIZE_REQUEST` requested (200x150), frame (50,90,400x300) | +43.60 | +49.95 |
| `COMMIT_SAMPLE source=allocation frame=(400x300)` | +45.40 | +51.90 |
| `COMMIT_SAMPLE source=size-changed frame=(200x150)` | **never sampled** | +61.07 (9.2 ms after the allocation sample) |
| `COMMIT_RESOLVED` | +76.86 `SIZE_SETTLED finalFrame=(400x300)` (31.5 ms after the allocation sample) | +61.15 `AT_TARGET finalFrame=(200x150)` |
| `FALLBACK_USED "min size unreported"` | +77.22 usingFrame=(400x300) | +61.48 usingFrame=(200x150) |
| `APPLY_ANCHOR` | `eff=(400x300)` final=(760,410) | `eff=(200x150)` final=(860,485) |
| `UNCLOAK_4C` | `positioned=(400x300) actual=(400x300) needsReanchor=false` | `positioned=(200x150) actual=(200x150) needsReanchor=false` |
| `UNCLOAK` (reveal) | +81.44 `frame=(760,410,200x150)` | +69.26 `frame=(860,485,200x150)` |
| `ANCHOR_ARMED` | +81.51 (after the resize) | +69.37 |

`center-1200x800` diverges at the same step: failing run (`20261012-031622-harsh`) has no `size-changed` sample, resolves `SIZE_SETTLED (400x300)` 32.5 ms after the allocation sample and anchors with `eff=(400x300)`; the passing run gets `size-changed (1200x800)` and `AT_TARGET`.

**The three mechanisms, from the code (all CONFIRMED by reading; none edited).**

1. *Size used by the anchor.* `computeAnchoredPosition` takes the window's **current** frame size, `Math.max(frame.width, minW)` (`assets/gnome/modern/anchor.js:19-20`), and `_computeAnchoredPosition` passes `window.get_frame_rect()` (`assets/gnome/modern/extension.js:985`). It never uses the requested size (`ctx.targetW/H`, `ctx.safeW/H`). When `COMMIT_RESOLVED` is not at target, the anchor therefore uses whatever size the window has at that instant, here 400x300. `FALLBACK_USED reason="min size unreported"` only means the toolkit reported a 0x0 minimum (`extension.js:1003`); it is logged on every passing run too (80 of 80 idle runs), so it does not distinguish failing from passing.
2. *The settle rule gives up after 30 ms.* `waitForCommit` resolves `AT_TARGET` as soon as a sample matches the target, but on the first `allocation` sample with an unchanged size it replaces the idle timer with a 30 ms quiet timer that finishes `SIZE_SETTLED` (`assets/gnome/modern/commit.js:11,88-96`, `COMMIT_QUIET_MS = 30`). If the client has not committed the new size within about 30 ms of that sample, the step ends with the old size and the anchor is computed from it (mechanism 1). Failing runs resolve 31.5 and 32.5 ms after the allocation sample.
3. *The late resize is missed by the reanchor logic.* `_reanchorIfResized` compares the size at `UNCLOAK_4C` with the size positioned (`extension.js:913-936`); a resize that lands after that check is only noticed by the reactive anchor, which is armed at the end of the reveal (`_armAnchor`, `ANCHOR_ARMED`, `extension.js:1025-1034`). In the failing run the resize happened between `UNCLOAK_4C` (+77.72) and `ANCHOR_ARMED` (+81.51): `UNCLOAK` already shows `200x150`, so no `size-changed` reached an armed anchor and no `ANCHOR_CORRECTION` was issued.

**Margin to the 30 ms limit at idle (CONFIRMED, 20 runs each, allocation sample to `size-changed` sample, ms).** `center-200x150`: 6.0 to 9.9 (median 7.6). `center-1200x800`: 19.1 to 33.6 (median 24.3). The 1200x800 client is much closer to the limit. One idle run measured 33.6 ms and still resolved `AT_TARGET`, so the 30 ms timer does not fire at exactly 30 ms; why (timer lateness or dispatch order in the shell main loop) is **HYPOTHESIS**, not examined.

**Fixture or product? (CONFIRMED that it is not only the fixture.)** `zenity --text-info` (GTK, `GDK_BACKEND=wayland`) opens at 300x400. **Corrected later (section 6.0, 6.3): 300x400 is its default size, not its minimum; its minimum is about 300x201** (requests of 200x150 and 50x50 settle at 300x201). The original sentence here said 200x150 "only gets raised to 300x400" and so cannot test this, which was wrong for that reason. The sizes below were still resizes away from 300x400. At sizes it can take, 20 runs each, `spawn-at spawn --class '*' --size W H --anchor center --json -- zenity ...`:

| Client / size | Mis-centred (final position = centre of the pre-resize 300x400 frame, 810,360) |
| :--- | :---: |
| zenity 500x450, run 1 | 4/20 |
| zenity 500x450, run 2 | 1/20 |
| zenity 1200x800 | 2/20 |

zenity never reaches the target inside the 30 ms window: all 60 runs resolved `SIZE_SETTLED (300x400)` and anchored with `eff=(300x400)`. Mis-centred and centred runs differ only in when the resize lands: centred runs resize after `ANCHOR_ARMED` and the reactive anchor corrects them (`ANCHOR_CORRECTION size=(500x450) from=(810,360) to=(710,335)`); mis-centred runs resize between `UNCLOAK_4C` and `ANCHOR_ARMED`, as in the fixture. So mechanisms 2 and 3 are in the extension, not in the fixture.

**Does the fixture ack the configure promptly? (HYPOTHESIS.)** `Resized` redraws immediately when the window has painted (`bin/fixture/window.rs:195-208`), and the event loop wakes at least every 16 ms (`window.rs:226`). At idle the fixture delivers the new size 6 to 10 ms (200x150) or 19 to 34 ms (1200x800) after the first allocation sample, and slower under load; the request-to-`COMMIT_RESOLVED` time was 33 to 213 ms in the 12 loaded failing runs. That the delay is dominated by the fixture's debug-build software drawing, rather than compositor scheduling, was not measured.

**Should the specs pass `--width`/`--height`? (opinion, HYPOTHESIS about intent.)** Not these two: if the fixture started at 200x150 or 1200x800 the `SetSize` would be a no-op and the spec would no longer exercise a resize, which is what they were added for. A pair of "already the right size" specs would be a separate, additional test. The specs are required, so until the extension is fixed they fail under load.

**Not done / not known.** The extension was not changed, so no fix is verified. The load test was run only for `center-200x150`. Whether the 03:16-03:17 failures were load is unproven.

## 3. CLI Coverage Matrix (`spawn-at --help`)

| Command / Option | Status | Covering Test / Unit Test | Notes / Out of Scope Rationale |
| :--- | :---: | :--- | :--- |
| `install` | Out of scope | — | Mutates host system `$PATH` and GNOME extension directories. Dev-tool safety prevents running in automated suite. |
| `uninstall` | Out of scope | — | Removes binary and extension from user system. Out of scope for test harness. |
| `update` | Out of scope | — | Network-facing update check. Out of scope for local offline verification. |
| `spawn` | **Covered** | `verify/placement/bottom-right.toml` | Primary command exercised across verification library. |
| `spawn --class <CLASS>` | **Covered** | `verify/placement/bottom-right.toml` | Tested via fixture explicit class matching. |
| `spawn --area <AREA>` | **Covered** | `test_oracle_vs_core_explicit_pos_all_three_area_modes_cross_check` | Planned spec: `placement/area-screen-vs-workarea`. |
| `spawn --monitor <MON>` | **Planned** | Planned: `platform/multi-monitor-secondary` | Requires multi-head display environment. |
| `spawn --anchor <ANCHOR>`| **Covered** | `verify/placement/bottom-right.toml` | Tested all 9 anchors in oracle golden test vectors. |
| `spawn --pivot <PIVOT>` | **Covered** | `test_golden_default_pivot_all_nine_anchors` | Cross-checked across core and oracle matrix. |
| `spawn -p, --pos <X> <Y>` | **Covered** | `test_golden_clamp_explicit_pos_0_0` | Absolute and offset positioning cross-checked. |
| `spawn -s, --size <W> <H>`| **Covered** | `verify/placement/bottom-right.toml` | Sets requested initial target dimensions (400×300). |
| `spawn --clamp <CLAMP>` | **Covered** | `test_golden_window_larger_than_workarea_clamp_true` | Clamping behavior verified mathematically. |
| `spawn -m, --margin <M>` | **Covered** | `verify/placement/bottom-right.toml` | Universal margin tested against oracle math. |
| `spawn --margin-{t,b,l,r}`| **Covered** | `test_oracle_vs_core_directional_margins_cross_check`| Directional boundary margins verified. |
| `spawn --focus` | **Covered** | Default mode in `execute_test` | Default behavior. |
| `spawn --no-focus` | **Planned** | Planned: `focus/passive-spawn-no-focus` | Passive background launch. |
| `spawn --defocus` | **Planned** | Planned: `focus/defocus-active-window` | Force yield focus back to predecessor. |
| `spawn --json` | **Covered** | `verify/placement/bottom-right.toml` | Tested in CLI execution and parsed by TUI/runner. |
| `spawn --no-wait` | **Planned** | Planned: `basics/no-wait-async` | Asynchronous launch without compositor ack wait. |
| `transform` | **Planned** | Planned: `transforms/reposition-by-id` | Dynamically moves/resizes live window. |
| `query` | **Planned** | Planned: `queries/query-layout` | Compositor state inspector. |
| `focus` | **Planned** | Planned: `focus/activate-by-title` | Brings window to foreground. |
| `defocus` | **Planned** | Planned: `focus/defocus-active-window` | Unfocuses active window. |
| `maximize` | **Planned** | Planned: `transforms/maximize-and-restore` | Compositor window maximization. |
| `minimize` | **Planned** | Planned: `transforms/minimize-and-focus` | Compositor window minimization. |
| `restore` | **Planned** | Planned: `transforms/maximize-and-restore` | Returns maximized/minimized window to normal floating. |
| `close` | **Covered** | `test_runner_cleanup_only_ever_passes_registered_ids_to_close` | Exercised by window tracker cleanup routines. |
| `-h, --help` | **Planned** | Planned: `basics/version-and-help` | Standard help synopsis. |

---

## 4. Animation Regression Cases (`minimize-animation`)

### Problem Statement
Under GNOME Wayland, minimizing a window triggers an asynchronous Clutter/Mutter shell actor animation (scaling down towards the icon in the dash/dock). If an uncloak action or reposition is applied during an active transition, or if unminimize occurs without properly sequencing `PRE_REVEAL_UNMINIMIZE`, the window may flash at its previous unminimized coordinates or freeze mid-animation.

### Specification: `verify/animation/minimize-animation.toml`
- **Goal:** Verify that a minimized window restored to normal state does not reveal off-target or emit visual glitches during compositor transition.
- **Trace Invariants:**
  1. Window state transition `MINIMIZED -> NORMAL` must precede `UNCLOAK`.
  2. No `UNCLOAK` event may occur prior to `PRE_REVEAL_UNMINIMIZE`.
  3. Frame buffer geometry must match target dimensions before opacity is restored to 255.
- **Test Implementation:**
  - Automated trace validation implemented in `tests/trace_analyser_test.rs`:
    - `test_scenario_minimized_passes_and_checks_pre_reveal_unminimize`
    - `test_violation_order_uncloak_before_pre_reveal_unminimize`
    - `test_scenario_remaximize_passes`

---

## 5. Delayed-Paint Calibration & Cloak Budget Analysis

### Status: mostly not measured

An earlier revision of this section published min/median/max cloak durations and a two-mode
("fast path" / "compositor scheduling delay") explanation. Those numbers were produced while the
trace analyser could measure a leftover window from the previous run instead of the window named
in the fixture's stdout (fixed in commit `7ffa99c`: the log slice has whole-second bounds, so it can
contain the previous run's window, and `cloak_duration_ms` was the maximum over every lifecycle in
the slice). They, and the explanations built on them, are withdrawn. No budget formula is
supported by any measurement.

### Measured data (`spawn-at-verify calibrate`, GNOME 46 Wayland, one machine)

`cloak_duration_ms` is UNCLOAK minus CLOAK for the window named in the fixture's stdout. Each row
is one invocation of
`spawn-at-verify calibrate --case cloak/delayed-paint/delay-500 --runs 10` after the fix. "Repeat"
means `run --repeat 10` in one process; "separate" means 10 separate `run` processes.

| Batch | Mode | Runs measured | Min | Median | Max |
| :--- | :--- | :---: | :---: | :---: | :---: |
| 1 | separate | 10 | 7.29 ms | 8.60 ms | 18.88 ms |
| 1 | repeat | 9 of 10 (1 run had no measurement; cause not investigated) | 6.85 ms | 9.63 ms | 140.18 ms |
| 2 | separate | 10 | 32.91 ms | 43.03 ms | 58.99 ms |
| 2 | repeat | 10 | 6.97 ms | 13.64 ms | 57.24 ms |

What these four batches show (CONFIRMED unless marked):
- Before the fix, neighbouring runs repeatedly reported identical values (for example 57.64 ms
  twice, 8.20 ms three times), the signature of one leftover window being measured for several
  runs. After the fix no two runs report the same value. **CONFIRMED.**
- The spread differs between batches (about 7 - 19 ms in batch 1 separate, about 33 - 59 ms in
  batch 2 separate). The investigation below explains this as two regimes.
- `cloak/delayed-paint/delay-0`, `delay-50`, `delay-150` and `placement/bottom-right` were not
  re-measured with `calibrate` after the fix; single `run`s of them gave about 30 - 44 ms before
  the regimes were understood.

### Findings from the cloak investigation

All numbers: `spawn-at-verify calibrate` (separate-process mode `--mode separate`, `--repeat` mode
`--mode repeat`, per-run phase gaps) and throwaway scripts, on one GNOME 46 Wayland machine
(`cloak/delayed-paint/delay-500` unless stated). Status of each statement is stated explicitly.

**1. Cloak time has two regimes. CONFIRMED (measured), mechanism HYPOTHESIS.**

| Regime | `COMMIT_RESOLVED` to `SetPositionAnchored` gap | Total cloak (UNCLOAK - CLOAK) |
| :--- | :---: | :---: |
| Isolated: no other test window alive | about 24 - 34 ms (batches of 20: 23.1 - 38.0) | about 27 - 40 ms (26.57 - 41.65) |
| Another fixture or `zenity` window alive | about 5 - 8 ms (5.3 - 10.1) | about 8 - 17 ms (7.6 - 17.5; a few runs 25 - 41 ms, see 4) |

- **CONFIRMED:** three 20-run `--mode separate` batches (A, B and an idle-machine batch) were all in
  the isolated regime: cloak medians 31.99, 30.91 and 32.35 ms. Two 20-run `--mode repeat` batches
  were in the fast regime from run 2 on (cloak medians 8.67 and 9.16 ms) but their first run was slow
  (33.43 ms and 27.33 ms; gaps 29.5 and 24.8 ms).
- **CONFIRMED, why `--repeat` differs from separate runs:** `--repeat` cleans up windows only after
  the last repetition (`runner/engine.rs:131-132`), so earlier fixture windows are alive during
  later runs. The TUI closes an automatic test's window after each run (`tui/app/queue.rs:66-68`),
  so TUI runs of automatic tests are in the isolated regime.
- **CONFIRMED, controlled switch** (same command, only the background changed): no other window
  25.7 - 31.0 ms; one fixture window alive 5.4 - 7.9 ms; that window closed 25.8 - 34.5 ms; a static
  GTK `zenity` window alive 5.6 - 7.6 ms.

**2. There is no deliberate wait between `COMMIT_RESOLVED` and the next batch step.
CONFIRMED (code); the stall itself CONFIRMED (probe, +/-10 ms alignment); its content HYPOTHESIS.**
- Code: `commit.js:62-66` logs `COMMIT_RESOLVED` and calls `resolve()`; `extension.js:783-786`
  `await`s it; the next code that runs is the `BATCH_STEP` log at `extension.js:697`. No timer,
  idle callback, frame wait or other work is between them, and the log shows no events in the gap.
- The gap appears only when the commit resolves synchronously (requested size equal to the
  fixture's size): 24.0 - 52.3 ms in 12/12 runs. When the commit resolved from a later `size-changed`
  signal (requested size different) the gap was 0.1 - 0.3 ms in 12/12 runs, but the total cloak was
  not shorter (37.0 - 54.2 ms): the time moved into the commit wait. **CONFIRMED.**
- A read-only probe calling the extension's `GetCursor` D-Bus method in a loop saw the Shell main
  loop stalled for 28.5 - 33.9 ms starting 0.554 - 0.560 s after the spawn command (the map time) in
  all four spawns probed, matching the 26.9 - 32.0 ms gaps. **CONFIRMED** (alignment about
  +/-10 ms, because the exact `ACTOR_MAP` wall time is not known).
- What the Shell does during that stall is **NOT shown. HYPOTHESIS:** work after the window maps
  (for example first-frame handling); no measurement here identifies it.

**3. Why another window makes the stall short is unexplained. HYPOTHESIS only.**
- Slow batches occurred with about 10 windows open (10 windows were present in all three slow
  batches), so the trigger is not simply "another window exists".
- CPU load does not explain it: correlation of the gap with load1 over 40 runs was 0.18 and with CPU
  MHz 0.31, and an idle-machine batch (load1 0.68 - 0.95) was just as slow. **CONFIRMED.**
- Two earlier `separate` batches (14:37 and 18:37 on 2026-10-11) were in the fast regime; what else
  was mapped then was not recorded. Not explained.

**4. `UNCLOAK_4C` to `UNCLOAK` is a one-frame wait. Code CONFIRMED; frame quantisation HYPOTHESIS.**
`extension.js:861` does `await waitForFrames(global.stage, 1, 50 ms)`, then `_reveal`. It was
1.1 - 4.8 ms in the isolated batches, with occasional 10 - 35 ms waits (34.5, 19.9, 18.2, 11.1, 9.8 ms)
that are why a few fast-regime cloaks were 25 - 41 ms. That these are multiples of the frame period
was not shown.

**5. Outliers did not reproduce. CONFIRMED (not reproduced); cause unknown.** The earlier 140.18 ms,
115.71 ms and one unmeasured run did not recur in more than 100 runs afterwards (40 `--repeat` runs,
60 separate runs and about 50 scripted runs); the largest cloak seen was 61.9 ms. `calibrate` saves the raw
log slice of any run that is unmeasured or at least 50 ms (`verify-results/calibrate-slices/`) if one
recurs.


### Trace capture and "Pass (trace unchecked)"

A drawer once showed `Cloak 162.0 ms`, `! over provisional budget`, `Trace ✔ clean, no violations` and Verdict Pass for two different tests while its TIMELINE said "no trace captured". Findings, each labelled by how it is known (CONFIRMED = read in the code or reproduced on the recording machine on 2026-10-12; HYPOTHESIS = consistent with the evidence but not demonstrated):

1. **Where `162.0 ms` came from. CONFIRMED.** The drawer's Cloak row used the trace's CLOAK..UNCLOAK time and, when there was none, fell back to `TestResult::duration_ms` (the old `tui/views/drawer/result_section.rs`): the wall-clock time of the whole `spawn-at spawn` process (`runner/executor.rs`, `start.elapsed()`), not a cloak time. The Budget row then judged that number against 100 ms. Reproduced with the previous build (8fa1975) and the extension's logging forced off: `cloak/delayed-paint/delay-0` passed with `"log_slice": ""` and `"duration_ms": 142`, which that drawer would have shown as `Cloak 142.0 ms`. No state is shared between tests: each result gets its own `duration_ms`. **Why two tests showed the same number: HYPOTHESIS.** Three consecutive runs of the same test reported exactly 142 ms, and 142 recurs across older reports in `verify-results/`, so equal wall-clock values for different runs are normal; the particular 162 ms was not reproduced.
2. **Why the log slice was empty. CONFIRMED mechanism, HYPOTHESIS for that session.** The journal source was in use (`XDG_CURRENT_DESKTOP=ubuntu:GNOME`; `run` now prints `Timing log source: journalctl`); the no-op source (`engine.rs`) is chosen only when that variable is unset or lacks "gnome". The extension's logger writes nothing while its logging is off (`assets/gnome/modern/logger.js`, `logTime`). The journal holds no line for window ids 870906690 or 870906691 (CONFIRMED). That logging was off during those runs is a HYPOTHESIS: the journal shows `gnome-extensions disable spawn-at@harsh.local` at 07:37:54 IST and the next `SET_LOGGING` at `+0.00ms` at 07:40:33; a logger's clock starts at its first logged line (`_t0`), so that line was the first of a new logger instance, and a new instance starts with logging off (`_loggingEnabled` undefined, then `SPAWN_AT_DEBUG`). The reload and the off state in between were not observed, and who sent `SetLogging(true)` at 07:40:33 is unknown. The same logger instance was still running at the live check (clock `+977148.32 ms` at 07:56:50 = 16 min 17 s after 07:40:33).
3. **Why Trace read "clean". CONFIRMED.** `TraceReport::is_pass()` is `violations.is_empty()`, so an analysis of zero events is a pass; an empty (not missing) slice produced exactly that report, and a result with no slice at all hit an explicit `✔ clean, no violations` branch in the drawer. An absent trace is now `TraceState::NotCaptured` (`trace/state.rs`), never a report.
4. **How logging is enabled. CONFIRMED.** The extension has no getter for its logging state (`dbus.js` exposes only `SetLogging`). The state lives in the logger instance created by `enable()` and is off again after a reload. Turning logging off writes no line (`logger.js` clears the flag before it calls `logTime`): after `SetLogging(false)` the last line of `~/.local/state/spawn-at/session.log` was still `SET_LOGGING enabled=true`, and neither that file nor the journal contains a single `enabled=false`. So `tui/logging.rs::detect_current_logging_state` returns `true` once any enable has ever been logged, and `ExtensionLoggingGuard::init` then skips enabling. Only the TUI used the guard; `run` and `calibrate` never enabled logging. The guard restores the state it detected on exit, so a second TUI whose startup state was "off" would turn logging off for an open one (read in the code, not run). Nothing re-checked the state before a run.

**What `spawn-at-verify` does now.**
- Before every run that needs a trace (`never_visible_off_target`, or an id under `cloak/`), `runner/executor.rs` calls `SetLogging(true)` and polls the journal up to six times, 40 ms apart, for the `SET_LOGGING enabled=true` line (`ext_logging.rs`). Without a getter this "set and confirm" replaces "query and re-enable". The time spent is not part of the run's `duration_ms`. If the call fails, the echo never arrives, or the log source is the no-op one, the reason is stored as the result's `trace_note` and printed by `run`. The TUI uses the same code path (`tui/app/queue.rs` calls `execute_single_test`).
- A slice with no event for the window the fixture named (no slice, an empty slice, or only other windows' events) is "not analysed". The drawer then shows `Trace  not analysed (no trace captured)`, `Cloak  not measured`, `Budget  not measured` and a `Reason` row. The run's duration is never shown as a cloak time.
- A trace-dependent test that otherwise passes with no trace ends as `Pass (trace unchecked)` (`AutoPassTraceUnchecked` / `FinalStatus::PassTraceUnchecked`): orange, not counted as a passed required test, and a required platform test with this status fails qualification. If its geometry was also unchecked the status stays "geometry unchecked" and the message names both.
- Exit codes: 0 no failure; 1 any failure, error, flaky or blocked; 2 nothing failed but a REQUIRED test passed with its geometry or its trace unchecked. Optional unchecked passes keep exit 0 and are listed in the summary.

**Not verified.** The fixes are covered by unit and integration tests with a fake extension and by a live `run` on the recording machine; the interactive TUI itself was not driven by a script, and no extension reload (`gnome-extensions disable/enable`) was performed.

### Budget Policy
1. **Advisory budget warning:**
   The 100 ms warning threshold is **provisional**: it is a guess, not derived from the data above,
   and no measurement supports a budget that depends on the paint delay. It is shown in the TUI
   drawer as `over provisional budget` and is not an automated pass/fail threshold. **CONFIRMED:** the
   largest cloak observed after the fix is 61.9 ms, below 100 ms; the budget stays provisional, and
   the two regimes above are why a single threshold cannot be derived from one batch.
2. **Automated Verification Pass/Fail Invariants:**
   Automated pass/fail criteria are strictly anchored in correctness invariants rather than synthetic timing limits:
   - Window must never be visible off-target (zero flicker).
   - `CLOAK` must precede `AFTER_SET_SIZE`, `COMMIT_RESOLVED`, `AFTER_SET_POSITION_ANCHORED` and `UNCLOAK` (partial order checked by the trace analyser).
   - `COMMIT_RESOLVED` and position configuration must precede `UNCLOAK`.
   - Final mapped geometry must match the oracle coordinates within tolerance (`±1 px`).

### Multi-Run CLI Flakiness Detection (`--repeat`)
`spawn-at-verify run` includes native support for multi-run flakiness detection via the `--repeat <REPEAT>` flag:
- **Flag Definition:** `--repeat <REPEAT> Repeat each test N times (cap 100) for flakiness detection [default: 1]` (pre-existing since commit `0f6bb36`).
- **Behavior:** Executes the test action across $N$ repetitions. If outcomes vary across repetitions (e.g., pass vs fail), the runner marks the result as `FinalStatus::Flaky` with an explanatory diagnostic message.

- **All runs are stored:** with `--repeat N`, `report.json` keeps the first repetition in `results` (marked `Flaky` if any repetition's outcome differed) and stores every repetition, in order, under the top-level `repeat_runs` array (`[{id, runs: [TestResult, ...]}]`). `report.md` gets a "Repeat Runs" table (run, statuses, duration, cloak) for each repeated test. Single runs and older reports have no `repeat_runs`. Earlier versions kept only the first run; that was a limitation of the report, not a design choice. `spawn-at-verify calibrate` prints min/median/max across the runs and writes the same report.

### `spawn-at-verify calibrate`
`spawn-at-verify calibrate --case <id> [--runs N]` (N defaults to 10, capped at 100) measures one case's `cloak_duration_ms` twice: with N separate `run` processes and with `run --repeat N` in one process. It prints, per run, the cloak duration, the window id named in the fixture's stdout, the number of distinct windows present in the log slice, and the report path, then min/median/max. For runs with no measurement or a cloak of at least 50 ms it also prints the scoped event list. Separate runs that start within the same second share a report directory name (second-granularity timestamps) and the output says so.

## 6. Min-size contract

Investigation of 2026-10-12. No code, spec or budget was changed. All live numbers were taken before any build or test run. Machine: GNOME Shell 46.0 on Wayland, workarea `0,40 1920x1040`, one monitor. Every claim is **CONFIRMED** (a quoted line, a count or raw output) or **HYPOTHESIS**. Throwaway scripts were kept outside the repo.

**Question.** What should spawn-at do when the requested size is below the app's minimum (or above the workarea), especially with a bottom-right anchor, and is the late-resize mis-centre of section 2.1 a bug or intended?

**Short answer.** The README promises that a window ends up larger than asked is "positioned using their final size, so anchors land correctly" and that a one-second listener re-applies the anchor after late resizes. Both promises are broken whenever the toolkit's resize arrives in a gap in the extension. That makes the mis-centre (and its bottom-right twin) a **BUG**, not intended behaviour. The design behind it is empirical minimum discovery under the cloak (the request is made while the window is hidden, the app settles at its minimum, the settled frame is used to anchor; 6.2 item 3); the bug is that a late resize is mistaken for "settled at the minimum". What the user still has to decide is listed in 6.7.

### 6.0 Corrections to section 2.1 (found while doing this work)

These statements in section 2.1 were wrong or too strong. They have been corrected in place in 2.1 (with a pointer here); this list records what changed.

1. 2.1 said `zenity --text-info` "has a 300x400 minimum". **Wrong, CONFIRMED.** 300x400 is its default size. Its real minimum is about 300 wide and 201 high: requests of 200x150 and 50x50 settle at 300x201 (6.3). The CLI text "window minimum is 300x400" for it was the size at the moment spawn-at read the window, while it was still shrinking to 300x201 (6.2 item 9, 6.4 row 3); it is a symptom of the late-resize bug, not a separate bug.
2. 2.1 says the fixture failed 0 of 80 runs at idle. **Superseded.** Later idle batches failed: 1 of 50 (6.5) and 4 of 40 in the isolated regime (6.6). The 40 `--repeat` runs had earlier windows alive by design (`runner/engine.rs:131-132`, section 5). For the 40 separate `verify run` invocations 1.1 s apart, earlier windows may also have been alive (the fixture's backstop lifetime for automatic tests is 10 s, `runner/validation.rs:48-57`); whether `run` closes its window when it exits was not checked, so that is a HYPOTHESIS. Both would be the fast regime of section 5.
3. 2.1 gives CPU load as the confirmed cause of the failures. Load does raise the failure rate (12 of 20 in 2.1), but idle runs also fail, so load is a sufficient trigger, not the only one.

### 6.1 Contract search

Searched: `README.md`, `docs/behavior-notes.md`, `docs/legacy-gnome.md`, `AI_DEV/*.md`, `scripts/smoke-live.sh`, `spawn-at spawn --help`, code comments, tests and `git log -S` for: min size, minimum, clamp, raised, size_raised, raised_w, raised_h, clampedSafeSize, reportedMinSize, "min size unreported". There is no CHANGELOG file in the repo. `git log -S` found no commit message that states a rule beyond the ones quoted below (`4ae8f97` added the README text, `39470eb` added the per-axis flags, `bdedd3e` added the sub-minimum smoke cases, `0c548c7` is "wip: fix sub-minimum geometry overshoot centering for legacy GTK3/VTE clients").

| # | Behaviour | Stated contract (quote and citation) | Status |
| :-: | :--- | :--- | :--- |
| 1 | Window smaller than requested is raised to the app's minimum | "Apps that refuse a requested size can still end up larger than you asked for" (`README.md:149-150`); "Toolkit coverage: ... including apps that enforce a minimum window size" (`README.md:148`) | stated |
| 2 | Position uses the window's final size | "...but they're positioned using their final size, so anchors land correctly." (`README.md:150`) | stated |
| 3 | Late resize after reveal is corrected | "If an app resizes itself after reveal ..., a 1-second listener re-applies your anchor so the window doesn't drift." (`README.md:110`) | stated |
| 4 | Wait for the app, not the compositor | "the extension watches the app's buffer settle (30ms quiet period, 120ms minimum, 1500ms hard cap) before revealing" (`README.md:103`) | stated (settle rule), but silent on what happens when the app has not answered inside 30 ms |
| 5 | Anchor math with a minimum | w' = min(max(w_t, w_min), w - m_l - m_r), anchor x/y from w', h' (`AI_DEV/spawn-at_master_architecture_blueprint.md`, section 9.4 "Anchoring"). Design document, not user documentation | stated (internal) |
| 6 | Minimum larger than available area | "the anchored edge's margin wins; the opposite edge may overflow the work area. The anchored edge is never pushed." (blueprint 9.4, "Post-clamp margin re-evaluation", `:993`). CLI text: "Window is oversized (WxH); bottom and right margins ignored." (`src/diagnostics.rs:1296`) | stated (internal and CLI message) |
| 7 | Geometry floor before asking the compositor | "Enforces a minimum geometry floor of 100x60px before passing dimensions to Mutter" (`AI_DEV/system_survey_and_understanding.md:223`; constants `assets/gnome/modern/extension.js:33-34`) | stated (internal) |
| 8 | Exit code when the size was raised | `scripts/smoke-live.sh` sub-minimum cases (30x20 at bottom-right) "Assert zero exit code, no false timeout warnings, and workarea bounds" (commit `bdedd3e` message) | stated (test and commit message only) |
| 9 | What the CLI prints for a raised size | Diagnostic enum doc: "The requested size was below toolkit minimums and was expanded." (`src/diagnostics.rs:149`); per-axis wording at `src/diagnostics.rs:240-257`, unit-tested at `:654-726` | stated; the number in it is the size when spawn-at read the window, wrong only while the window is still resizing (6.2 item 9, 6.4 row 4) |
| 10 | Default margin when `--margin` is omitted | `--help` says only "Universal margin in pixels applied to all boundaries". Code: `self.margin.unwrap_or(16)` (`src/cli/args.rs:134`). An example in `docs/behavior-notes.md:19` shows "clamped to (16, 16)" | **UNDEFINED** in user docs |
| 11 | Window larger than the workarea | blueprint formula clamps the size to workarea minus margins (row 5); CLI only warns (row 6). Observed: the window is not clamped by spawn-at (6.3) | **UNDEFINED** for the size itself |
| 12 | Request below the app minimum with the app's minimum unknown to the extension | `FALLBACK_USED reason="min size unreported" usingFrame=` (`extension.js:1002-1007`) is the empirical path of 6.2 item 3: the settled frame is used. No user document describes it | stated in code and by the author; **not in user docs** |
| 13 | Which size an anchor uses while the app has not yet answered | none | **UNDEFINED** (the code uses the current frame, 6.2) |
| 14 | Resize arriving more than 1 s after the last correction | README says "1-second listener" (`:110`); `ANCHOR_RETIRE_QUIET_MS = 1000` (`extension.js:35`) | stated, limits not documented |

### 6.2 Code path of a request below the app's minimum

1. **CLI sends the request.** `spawn` passes the size to the extension as a batch `SetSize`, `WaitForCommit`, `SetPositionAnchored`. The extension reports the result through the `SpawnClaimed` signal with `size_raised` (`assets/gnome/modern/dbus.js:76`, `src/platform/linux/gnome/dbus.rs:36`).
2. **`_stepSetSize`** (`extension.js:736-782`) reads the minimum with `_getMinSize` (`:298-303`: `window.get_min_size?.()`, `[0, 0]` when absent), computes `floorW = max(100, minW)`, `floorH = max(60, minH)` and `safeW = max(w, floorW)`, `safeH = max(h, floorH)`, stores them as `ctx.clampedSafeSize`, then calls `move_resize_frame(true, x, y, safeW, safeH)`. **`clampedSafeSize` protects** the compositor request from a frame smaller than 100x60 (per `AI_DEV/system_survey_and_understanding.md:223`: client-side-decoration padding subtraction giving negative widget allocations). It is a floor for the request, not a measured app minimum.
3. **The API route to the minimum is unavailable on this Shell; the design finds the minimum empirically. CONFIRMED (code and logs).** `Meta.Window.prototype.get_min_size` is `undefined` in the Mutter 14 (GNOME 46) typelib:
   ```
   $ GI_TYPELIB_PATH=/usr/lib/x86_64-linux-gnu/mutter-14 gjs -c 'imports.gi.versions.Meta="14"; const Meta=imports.gi.Meta; print("get_min_size:", typeof Meta.Window.prototype.get_min_size)'
   get_min_size: undefined
   ```
   and the journal of this session holds 722 `SET_SIZE_REQUEST` events, all with `reportedMinSize=(0x0)`, and 722 `FALLBACK_USED` events. A non-zero reported minimum has never been received in this session, so the API route (`_getMinSize`, `extension.js:298-303`) always returns `[0, 0]` on GNOME 46. **This does not mean the extension has no minimum handling.** An earlier version of this section said the minimum-aware logic "never runs"; that was wrong. The design, as described by the author and as the code does it, is empirical discovery under the cloak:
   - The window already exists and is mapped, cloaked (opacity 0): `Cloak` is the first instruction (`src/platform/linux/gnome/mechanics.rs:146`, executed at `extension.js:712`), then `SetSize` with the user's size, then `WaitForCommit` (`mechanics.rs:148-154`, `extension.js:713-715`, `:736-782`, `:783-787`). The app is asked for the size while nobody can see it.
   - An app that refuses an illegal size answers with its own size; `waitForCommit` lets that settle (`commit.js:62-66` logs `COMMIT_RESOLVED ... finalFrame=`; `SIZE_SETTLED` after 30 ms of no change, `commit.js:88-96`).
   - **The settled frame is then taken as the size to anchor with:** `_computeAnchoredPosition` reads `window.get_frame_rect()` (`extension.js:985-994`), `computeAnchoredPosition` uses `effW = max(frame.width, minW)` (`anchor.js:19-20`, `minW` is 0 here, so the frame alone), and the log line `FALLBACK_USED reason="min size unreported" usingFrame=(WxH)` (`extension.js:1002-1007`, printed whenever `minW` and `minH` are both 0) records which frame was used. That line is the empirical path being taken; it is not an error.
   - After the reveal, `_reanchorIfResized` (`extension.js:913-936`) and the reactive anchor (`:1025-1050`) re-anchor if the frame still changes.
   - **What the code does not have (searched, not found):** (1) no step that deliberately requests a tiny size to provoke the minimum: the only sizes sent are the user's (`mechanics.rs:149-152`, `src/platform/linux/gnome/mod.rs:882`), raised to at least 100x60 by the geometry floor (`extension.js:741-751`); `30x20` appears only as a user example in `scripts/smoke-live.sh:168-175` and in unit tests; (2) no stored or reused minimum per app; (3) nothing that tells "settled at the minimum" from "still resizing": the only discriminators are the 30 ms quiet timer and equality with the target or safe size (`commit.js:77-83`); `_isToolkitClamped` (`extension.js:906-910`, `frame > target`) only feeds the focus-pulse decision at `UNCLOAK_4B` (`:822-824`). That missing discriminator is exactly the section 2.1 bug: a late resize is read as "settled at the minimum". Whether other Shell versions expose `get_min_size` is not known (HYPOTHESIS: GNOME 45 to 48 behave the same; not tested).
4. **`waitForCommit`** (`assets/gnome/modern/commit.js:34-106`): `AT_TARGET` when a sampled frame equals the target or the safe size. On the first `allocation` sample with unchanged size it replaces the idle timer with a 30 ms quiet timer that finishes `SIZE_SETTLED` (`commit.js:88-96`). A below-minimum request can never reach `AT_TARGET` (the frame never equals 200x150), so **for such requests `SIZE_SETTLED` is the normal exit, not an edge case**. CONFIRMED in 6.3: every zenity run below its minimum resolved `SIZE_SETTLED (300x400)`.
5. **Anchor.** `_stepPosition` -> `_applyAnchoredPosition` -> `computeAnchoredPosition(bounds, window.get_frame_rect(), minSize, payload)` (`extension.js:985-994`, `anchor.js:13-56`). **The anchor uses the window's current frame size**, `max(frame, minSize)`, never the requested size:
   - `AT_TARGET` reached: the frame is the requested size, so the anchor is exact.
   - `SIZE_SETTLED`, app still at its old size: the anchor uses the old size (zenity: 300x400, fixture: 400x300). The window is placed for that size and resizes afterwards.
   - Toolkit minimum already applied before the step: the anchor uses the minimum (the final size) and is exact.
6. **Repair after the fact.** `_reanchorIfResized` (`extension.js:913-936`) compares the size at `UNCLOAK_4C` with the positioned size once. A resize after that check is only caught by the reactive anchor, which is armed at the end of the reveal (`_armAnchor`, `extension.js:1025-1034`, `ANCHOR_ARMED`) and retires 1 s after its last correction (`:35`). A resize between `UNCLOAK_4C` and `ANCHOR_ARMED` is missed by both: the gap is the bug of section 2.1.
7. **Rust side.** `raised_w = final.width > requested.0`, `raised_h = final.height > requested.1`, `size_raised = raised_w || raised_h || <extension flag>` (`src/commands/spawn.rs:493-497`), where the final rectangle is the one read when spawn-at finishes (not necessarily the settled one). The JSON output carries all three (`:499-513`); `verify_and_report_placement` is called with `min_size = None` (`:520`), so the diagnostic text takes its "minimum" from the final size (`src/diagnostics.rs:236-257`, `:423-430`).
8. **CLI output and exit code.** Raised: `INFO: Requested size (200x500); window minimum width is 300, placed at (810, 310) [300x500].` Requested far below: `INFO: Requested size (50x50) is below toolkit minimums; window will expand.` Mis-positioned: `WARNING: Window mapped at (810, 360) [300x500] but expected position was (810, 310). Final size differs from the requested size (200x500); window was not positioned at target.` Oversized: `WARNING: Window is oversized (3000x2000); bottom and right margins ignored.` The exit code was 0 in all 60 probe runs, including every WARNING (6.3). A mis-positioned window is therefore invisible to a script that checks only the exit code.
9. **Where the two INFO texts come from. CONFIRMED (code).**
   - `INFO: Requested size (WxH) is below toolkit minimums; window will expand.` is printed by `src/commands/spawn.rs:203-207` for `GeometryDiagnostic::SubMinimumSize`. That value is produced by `check_geometry_diagnostics` at `crates/spawn-at-core/src/geometry.rs:744-746`: `if w < 100 || h < 80`. **It is a static threshold on the requested size**, evaluated at `src/commands/spawn.rs:171-209` before the child is spawned. It does not use the observed size, the app, or any settled frame. So it fires for any 99-wide or 79-high request, even for an app with no minimum, and never fires for a request of 100x80 or larger even when the app raises it (200x500 on zenity printed the other INFO below). The threshold also differs from the extension's own floor (100x60, `extension.js:33-34`); nobody states why (UNDEFINED).
   - `INFO: Requested size (WxH); window minimum [width|height] is N, placed at (X, Y) [AxB].` is built from the rectangle spawn-at read when it finished (`src/commands/spawn.rs:493-520` -> `src/diagnostics.rs:236-257`, `min_size` is `None` at `spawn.rs:520`); N and AxB are that rectangle's size. In the 6.3 probes this text matched the window's final size in 7 of 14 cases and did not in the other 7. **All 7 mismatches are runs where the window was still resizing when spawn-at read it** (300x400 at exit, 300x201 afterwards, centre and bottom-right "both" cases); no mismatch occurred when the size was stable. That is a symptom of the late-resize bug (6.4 row 3), not a separate wording bug.

### 6.3 Live probes

`spawn-at spawn --class '*' --size W H --anchor A --json -- env GDK_BACKEND=wayland zenity --text-info ...`, no `--margin` (so the default 16 applies, `args.rs:134`), 5 runs per row, idle machine, nothing else running. "Final" is `spawn-at query windows --json` read 1.3 s after spawn-at exited, so late resizes and reactive-anchor corrections are included. "Anchored for FINAL size" compares the final position with the oracle for the final size and margin 16 (centre: `floor((area - size)/2)`; bottom-right: `workarea edge - margin - size`; oversized: top-left plus margin). Exit code was 0 in every run; the stderr line is the most common one.

| Anchor | Case (requested) | Final size | Final position(s) | size_raised / raised_w / raised_h | Anchored for final size | Overflows workarea | stderr (most common) |
| :--- | :--- | :--- | :--- | :--- | :---: | :---: | :--- |
| centre | width only (200x500) | 300x500 | 810,310 (3), 810,360 (2) | true/true/false | 3/5 | no | INFO "window minimum width is 300, placed at (810, 310) [300x500]"; 2 runs WARNING "mapped at (810, 360) ... expected position was (810, 310)" |
| centre | height only (500x300) | 500x300 | 710,410 (5) | false/false/false | 5/5 | no | INFO "mapped at final size (500x300) and positioned at (710, 410)" (zenity's real minimum height is below 300, so no raise) |
| centre | both (200x150) | 300x201 | 810,459 (4), 810,360 (1) | true/true/true | 4/5 | no | INFO "window minimum is 300x400, placed at (810, 360) [300x400]" (the final size was 300x201); 1 run WARNING |
| centre | far below (50x50) | 300x201 | 810,459 (5) | true/true/true | 5/5 | no | INFO "below toolkit minimums; window will expand" |
| centre | exactly at default (300x400) | 300x400 | 810,360 (5) | false/false/false | 5/5 | no | INFO "mapped at final size (300x400)" |
| centre | larger than workarea (3000x2000) | 1920x1040 | 16,56 (3), 810,360 (2) | false/false/false | 3/5 | yes | WARNING "Window is oversized (3000x2000); bottom and right margins ignored." |
| bottom-right | width only (200x500) | 300x500 | 1604,564 (4), 1604,664 (1) | true/true/false | 4/5 | 1 run yes (y 664 + 500 = 1164 > 1080) | INFO / 1 run WARNING "mapped at (1604, 664) ... expected position was (1604, 564)" |
| bottom-right | height only (500x300) | 500x300 | 1404,764 (5) | false/false/false | 5/5 | no | INFO |
| bottom-right | both (200x150) | 300x201 | 1604,863 (3), 1604,664 (2) | true/true/true | 3/5 | no | INFO "window minimum is 300x400, placed at (1604, 664) [300x400]"; 2 runs WARNING |
| bottom-right | far below (50x50) | 300x201 | 1604,863 (3), 1604,664 (2) | true/true/true | 3/5 | no | INFO "below toolkit minimums; window will expand" |
| bottom-right | exactly at default (300x400) | 300x400 | 1604,664 (5) | false/false/false | 5/5 | no | INFO |
| bottom-right | larger than workarea (3000x2000) | 1920x1040 | 16,56 (5) | false/false/false | 5/5 | yes | WARNING "oversized ... bottom and right margins ignored" |

Raw lines of two failing runs (script output):
```
center | below-both | 200 x 150 | run2 | json 810,360 300x201 | final(query) 810,360 300x201 | raised true/true/true | anchored-for-final: NO (want 810,459) | overflow no | exit 0 | WARNING: Window mapped at (810, 360) [300x201] but expected position was (810, 459). Final size differs from the requested size (200x150); window was ...
bottom-right | below-w | 200 x 500 | run5 | json 1604,664 300x500 | final(query) 1604,664 300x500 | raised true/true/false | anchored-for-final: NO (want 1620,580) | overflow yes | exit 0 | WARNING: Window mapped at (1604, 664) [300x500] but expected position was (1604, 564). Final size differs from the requested size (200x500); window wa...
```
(The "want" column in the raw script output assumed margin 0 and is wrong; the table above uses margin 16. The positions and the verdicts "NO" for those two runs are unchanged by that.)

Observations:
- **CONFIRMED:** in the cases where the final size differs from the size the app opened with and the app answers after the 30 ms window, some runs end placed for the old size: 3 of 25 non-oversized centre runs and 5 of 25 bottom-right runs. At bottom-right that is a visible defect: a 300x201 window left 199 px above the corner (y 664 instead of 863), and one 300x500 window left hanging off the bottom of the workarea.
- **CONFIRMED:** the CLI says "window minimum is 300x400" for a window that settled at 300x201. The number is the window's size at the moment spawn-at read it, while it was still resizing (6.2 item 9): a symptom of the late-resize bug, not a wording bug. spawn-at also exits 0 on every warning.
- **CONFIRMED:** the oversized window is not clamped by spawn-at: it ends 1920x1040 (the whole workarea, 16 px more than workarea minus margins on each axis) and overflows by the margin on the right and bottom, at top-left plus margin. The flags `size_raised/raised_w/raised_h` are all false although the window is smaller than requested; there is no flag for "reduced". Who limited it to 1920x1040 (Mutter, GTK) is not shown (HYPOTHESIS: Mutter).
- The 300x400 "at-min" row is the app's default size, so no resize is needed there. It is the control: 10 of 10 anchored correctly.

### 6.4 Classification

| # | Behaviour | Class | Why |
| :-: | :--- | :---: | :--- |
| 1 | Window ends larger than requested when the app has a minimum | **INTENDED** | `README.md:149-150` |
| 2 | Final position is correct for the final size | **BUG** when violated (centre 3/25, bottom-right 5/25 in 6.3; fixture 12 of 20 under load, 1 of 50 and 4 of 40 at idle) | violates `README.md:150` "positioned using their final size, so anchors land correctly" |
| 3 | Late resize (arriving after the 30 ms settle) leaves the window mis-centred | **BUG** for centre | violates `README.md:110` (the listener "re-applies your anchor so the window doesn't drift") in the cases where the resize lands between `UNCLOAK_4C` and `ANCHOR_ARMED` (`extension.js:913-936` and `:1025-1034`); the same reactive anchor corrected zenity 500x450 in 16 of 20 and 19 of 20 runs in two batches (section 2.1) and missed it in the rest |
| 3b | Late resize leaves the window mis-anchored at bottom-right | **BUG** for bottom-right | same promises; and the damage is larger: the corner is missed by the size difference (199 px in 6.3) and the window can overflow the workarea |
| 4 | Text "window minimum is WxH" | **BUG symptom of row 3** (not a separate wording bug) | the number is the window's size when spawn-at read it (6.2 item 9); it matched the final size in 7 of 14 probe cases and all 7 mismatches were windows still resizing (300x400 printed, 300x201 final). When the size is stable it is correct. Fixing row 3 fixes this text |
| 4b | INFO "Requested size (WxH) is below toolkit minimums; window will expand." | **UNDEFINED** (threshold) | static `w < 100 \|\| h < 80` on the requested size (`geometry.rs:744-745`); differs from the extension's 100x60 floor and ignores the app |
| 5 | Exit code 0 when the window ended mis-positioned (WARNING printed) | **UNDEFINED** | only the sub-minimum smoke assertion (`bdedd3e`) says "zero exit code"; nothing says a WARNING about a mis-positioned window must exit 0 |
| 6 | Oversized request: anchored edge keeps its margin, opposite edge overflows | **INTENDED** | blueprint 9.4 conflict rule and the CLI message "bottom and right margins ignored" |
| 7 | Oversized request: size not reduced to workarea minus margins | **UNDEFINED** | the blueprint formula says it should be; the code does not do it; no user-facing text decides |
| 8 | Oversized request with the centre anchor ends at top-left plus margin (16,56) | **UNDEFINED** | follows from 6; the blueprint's centre formula would give 0,40 |
| 9 | Geometry floor 100x60 | **INTENDED** | `system_survey_and_understanding.md:223`, `extension.js:33-34` (not observable with zenity: its minimum is above the floor) |
| 10 | Default margin 16 | **UNDEFINED** in user docs | `args.rs:134` only |
| 11 | Minimum unknown to the extension on GNOME 46 (found empirically under the cloak instead) | **INTENDED** (design stated by the author; the code does this: 6.2 item 3). Nothing in user docs mentions it | the empirical route is the design; its flaw is row 3 |
| 12 | Late resize more than 1 s after the last correction | **INTENDED** (limit stated) | `README.md:110`, `extension.js:35` |

**The late-resize mis-centre of section 2.1 is a BUG, for centre and for bottom-right.** Reason: the README promises correct anchoring for the final size and a one-second corrector; the logs show the window's final size is known to the extension (the `UNCLOAK` event reports `frame=(760,410,200x150)` before `ANCHOR_ARMED`) yet nothing re-anchors it. It is not "intended behaviour for a resize that arrives late", because the corrector exists exactly for that case and works when the resize lands after it is armed.

### 6.5 Margin to the 30 ms quiet window, `placement/center-1200x800`

Done with the fixture driven through `spawn-at` exactly as the spec does (`spawn --class verify.placement.m<pid> --anchor center --size 1200 800 --margin 16 --json -- target/debug/fixture ... --exit-after-ms 2500`), 50 separate invocations 1.1 s apart, because `spawn-at-verify run` refuses a stale binary and building during measurement was not allowed. Previous windows were still alive (2500 ms lifetime), so this is the "another window alive" regime of section 5. Milliseconds from the `allocation` `COMMIT_SAMPLE` to the `size-changed` sample, in run order (`none` = no `size-changed` sample before `SIZE_SETTLED`):

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

- **Where the timer starts. CONFIRMED (code).** In `check()` (`commit.js:68-97`) the `initial` sample starts no timer; the quiet timer is started on the first `allocation` sample whose size is unchanged while the idle timer is still active (`commit.js:93-96`). It runs 30 ms from that allocation sample, not from the initial one. In the failing run 30 the allocation sample was at +51.14 ms and `COMMIT_RESOLVED SIZE_SETTLED` at +81.99 ms (30.85 ms later), as the code predicts.
- **Is ~26 ms safe? By luck, not by design. CONFIRMED that it is not enforced; mechanism HYPOTHESIS.** Nothing in the code orders a pending `size-changed` ahead of an expired timer: whichever the main loop dispatches first wins. A sample at 26 ms leaves 4 ms of slack, 16 of 49 samples were in 25 to 35 ms, and two samples at 30.8 and 32.8 ms were still processed before the 30 ms timer fired, which shows the timer callback itself can run late. Why it ran late there (a main-loop stall, section 5) is HYPOTHESIS. The one failing run is the case where the sample did not make it.

### 6.6 Regimes

Same driver as 6.5 (`--exit-after-ms 800`, 1.6 s gap so the previous window is gone), 20 runs each. (a) isolated: no other window of ours alive; (b) a separate `fixture` window (`KEEPALIVE`, 400 s lifetime) kept alive throughout. Milliseconds allocation to `size-changed`.

| Regime | Size | Runs | `none` (SIZE_SETTLED) | Mis-centred | min / mean / max ms | over 30 ms | 25 to 35 ms |
| :--- | :--- | :-: | :-: | :-: | :--- | :-: | :-: |
| (a) isolated | 200x150 | 20 | 2 (runs 11, 12) | 2 | 6.4 / 8.6 / 12.4 | 0 | 0 |
| (a) isolated | 1200x800 | 20 | 3 (runs 1, 2, 3) | 2 | 21.2 / 24.7 / 31.5 | 1 | 5 |
| (b) window alive | 200x150 | 20 | 0 | 0 | 6.1 / 7.5 / 9.9 | 0 | 0 |
| (b) window alive | 1200x800 | 20 | 0 | 0 | 21.5 / 24.3 / 32.0 | 2 | 5 |

Per-run values:
```
(a) 200x150 : 10.3, 11.3, 8.4, 7.0, 7.9, 6.4, 8.9, 6.4, 7.3, 6.5, none, none, 11.5, 8.0, 8.8, 7.7, 11.0, 7.7, 7.3, 12.4
(a) 1200x800: none, none, none, 24.5, 23.4, 25.9, 24.9, 31.5, 24.6, 23.2, 25.6, 22.1, 21.2, 24.7, 23.1, 23.3, 21.3, 23.2, 28.2, 29.1
(b) 200x150 : 8.7, 7.1, 6.9, 7.4, 6.6, 9.9, 6.9, 6.5, 7.0, 7.1, 7.9, 6.4, 7.6, 9.7, 7.5, 6.8, 8.4, 6.1, 7.7, 8.8
(b) 1200x800: 22.4, 23.7, 26.5, 24.5, 22.4, 32.0, 23.6, 23.3, 27.7, 23.5, 23.4, 24.1, 21.6, 30.4, 22.8, 21.5, 25.0, 22.3, 22.1, 22.4
```

- **CONFIRMED:** failures occur only in the isolated regime (4 mis-centred of 40; 5 runs with no sample before the timer) and not with another window alive (0 of 40). In 1200x800 (a) the three `none` runs are the first three of the batch.
- **CONFIRMED:** the typical delay does **not** differ between regimes (1200x800: mean 24.7 vs 24.3 ms; 200x150: 8.6 vs 7.5 ms) and it depends strongly on window size (about 8 ms vs about 24 ms). So the ~26 ms is mostly the cost of the client producing a first frame at the new size, not the Shell stall.
- **Does the delay track the post-map Shell main-loop stall of section 5? Mostly no; the failures may. HYPOTHESIS.** Section 5 measured a 28.5 to 33.9 ms stall in the isolated regime only. The failing runs are in that regime only, and a stall of that length would delay the dispatch of a client commit that normally arrives 6 to 12 ms (200x150) after the allocation sample past the 30 ms timer, which fits the two 200x150 `none` runs. The probe of section 5 (a `GetCursor` loop) was not repeated here, so the stall and these failures were not shown to coincide. **Does the 30 ms timer race it? HYPOTHESIS:** a ~30 ms stall means a due timer and a due `size-changed` are dispatched in the same loop iteration, whose order is not controlled by the extension.

### 6.7 What the user must decide

1. **Should the anchor wait for the final size?** Options: re-anchor on any size change until the reactive anchor is armed (close the `UNCLOAK_4C` to `ANCHOR_ARMED` gap), arm the reactive anchor before the reveal wait, or lengthen/replace the 30 ms quiet rule. All are product changes; this section recommends none.
2. **The "below toolkit minimums" threshold.** The INFO uses a static `w < 100 || h < 80` on the request (6.2 item 9); keep it, align it with the 100x60 floor, or drop it in favour of the observed result.
3. **Exit code when the final position is wrong.** Keep 0 (current, matches the smoke script) or return non-zero when the WARNING "was not positioned at target" is printed.
4. **Oversized requests.** Clamp the size to workarea minus margins (blueprint formula) or leave the app to size itself and only warn (current). And where the centre anchor puts an oversized window.
5. **Default margin of 16** in `--help` and the README.
6. **Whether tests may rely on `size_raised`/`raised_w`/`raised_h`.** They are computed from the rectangle at the moment spawn-at exits, so they can describe a size that later changes (6.3: json 300x400, final 300x201).

### 6.8 Behaviours skipped by the test plan as UNDEFINED

Rows 4b, 5, 7, 8 and 10 of 6.4 (the static "below toolkit minimums" threshold, exit code on a mis-positioned window, size of an oversized window, centre anchor with an oversized window, default margin), plus "which size the anchor uses while the app has not answered" (6.1 row 13). No spec asserts any of them. Specs may be written for rows 1, 2, 3, 3b, 4 (as a symptom), 6, 9, 11 and 12 (INTENDED or BUG); expectations come from the oracle, and the BUG rows stay failing until the product is fixed. Tests do not assert `size_raised`/`raised_w`/`raised_h` (they describe the rectangle at spawn-at's exit); they are only recorded.

### 6.9 Not done

The Shell-stall probe of section 5 was not repeated, so the link between the stall and the failures is a HYPOTHESIS. No test was run with a real non-zero minimum reported by the Shell (none exists on GNOME 46). Only zenity (GTK4) and the winit fixture were probed; GTK3, Qt and Electron apps were not. 6.5 and 6.6 were measured with the fixture started through `spawn-at` rather than through `spawn-at-verify run`.

### 6.10 Placement resize specs: set, loader rule and live results (2026-10-12)

**Specs (17, category placement).** Every spec states `margin = 16` and its fixture minimum (`min_width` / `min_height` under `[subject]`, passed to the fixture as `--min-width` / `--min-height`); expectations come from the oracle (`max(requested, minimum)` per axis, anchor for that final size, far-edge containment), never from pixel values in the spec. The `[matrix]` mechanism was **not usable**: it substitutes only strings (`schema/matrix.rs:115-143`, not `expect.rect.size` or the minimum fields) and builds a cartesian product, so paired sizes such as 200x150 / 800x600 cannot be expressed. One file per case instead.

| Anchor | Cases (requested vs fixture minimum) | Mode |
| :--- | :--- | :--- |
| bottom-right | smaller 200x150 (min 100x100), larger 800x600 (min 100x100), minsize-at-min 300x200 (min 300x200), below-min-width 200x300, below-min-height 400x150, below-min-both 200x150, far-below-min 50x50 (all min 300x200) | `both` for smaller and below-min-both, `auto` for the rest |
| bottom-left, top-right, bottom, right | smaller 200x150 (min 100x100); below-min-both 200x150 (min 300x200) | `auto` |
| top-left | below-min-both (control: its position does not depend on size) | `auto` |
| (none) | `invalid-size-zero`: expected failure, see below | `auto` |

**Loader rule.** A spec whose `[expect.rect]` states neither `margin` nor all four per-side margins is a load error naming the spec id and file (`loader/registry.rs`; test `test_spec_without_explicit_margin_is_an_error`). The oracle assumes margin 0, spawn-at defaults to 16 (`src/cli/args.rs:134`). **Existing specs affected: none** (every spec with an `[expect.rect]` already stated its margin; checked with `grep` over `verify/`).

**Expected failure.** The spec format can express it (`exit_code = 2`, `stderr_present = ["[spawn-at] ERROR: Invalid size: ..."]`, no `[expect.rect]`). The runner could not run it: a `spawn` with a fixture subject always went through window attribution, which failed with "could not attribute; windows left open: []" (0 of 20 passed). Fixed in the harness (`runner/validation.rs` `expects_no_window`, used by `runner/executor.rs`): attribution is skipped when a non-zero exit code was expected and returned. After the fix: 20 of 20 in regime (a) and 20 of 20 in regime (b).

**Isolation check (regime a).** `spawn-at-verify run` ends with `tracker.cleanup_test_windows` (`runner/engine.rs:170-171`), which closes the windows it tracked. Verified, not assumed: before each of the 820 isolated invocations (340 + 60 + 60 + 340 + 20) the driver counted windows whose class starts with `verify.` (`spawn-at query windows --json`) and running `fixture` processes (`ps -eo comm=`): **0 windows and 0 processes before 819 of them**. The one exception is run 1 of the second `center-200x150` batch, which started right after a direct fixture drive whose last window (800 ms lifetime) was still closing; that run passed (8.5 ms) and is kept. 0 and 0 again after the last batch. Between runs the driver waited 1.5 s.

**Regimes.** (a) ISOLATED: 20 separate `spawn-at-verify run --glob placement/<id>` invocations per spec, 1.5 s apart, nothing else alive. (b) `--repeat 20`: earlier windows stay alive until the end (`engine.rs:140-170`), the fast regime of section 5. `size`, `position` and `raised` below are the rectangle and flags spawn-at printed when it exited (the harness never re-reads the window, so a window that resizes afterwards is shown at its old size); `raised` is recorded only, never asserted. No run had a trace note and no run overflowed the workarea.

| Batch | What | Auto check passes (16 resize specs x 20) | Runs with no `size-changed` before the 30 ms `SIZE_SETTLED` |
| :--- | :--- | :---: | :---: |
| (a1) isolated, first batch | started minutes after `cargo build`, 11 min long | **95 / 320 (29.7%)** | 249 |
| (a2) isolated, clean rerun | nothing else running, same specs | **318 / 320 (99.4%)** | 2 |
| (b) `--repeat 20` | earlier windows alive | 315 / 320 (98.4%) | 6 |

Regime (b), recounted from the 16 stored `report.json` files (`repeat_runs`): **5 failures, one each on bottom-right-smaller, -minsize-at-min, -below-min-width, -below-min-height and -far-below-min** (19/20 each); the other 11 specs passed 20/20. Six runs had no `size-changed` sample before `SIZE_SETTLED`, the five failures plus one run of bottom-right-larger that still passed (it was placed correctly). So "6" counts runs without a sample, not failures; an earlier summary that spoke of six failing bottom-right specs was wrong.

(a1) and (a2) are the same 16 specs on the same binaries. Three of the specs re-run alone right after (a1) passed 60 of 60 (bottom-right-smaller 20, bottom-right-larger 20, top-left-below-min-both 20), as did `center-200x150` (20+20) and `center-1200x800` (20) via `run`, and 60 direct fixture drives (20 each without minimum flags, with `--min-width 100 --min-height 100`, and bottom-right with the flags). **Why (a1) was slow is not shown. HYPOTHESIS:** the Shell or machine was busy after the builds that preceded it; no load was recorded. Because of it there is no single "isolated" rate: 29.7% and 99.4% were both measured, on different minutes. Failures are reported as they came; no expectation was changed.

**Per spec, regime (a2) / (b)** (a1 passes in brackets; size and position are the values at exit):

| id | (a2) | (b) | (a1) | final size | final position |
| :--- | :---: | :---: | :---: | :--- | :--- |
| bottom-right-smaller | 19/20 | 19/20 | [7] | 200x150 (1 run 400x300) | 1704,914 (1 run 1504,764) |
| bottom-right-larger | 20/20 | 20/20 | [14] | 800x600 | 1104,464 |
| bottom-right-minsize-at-min | 20/20 | 19/20 | [7] | 300x200 | 1604,864 |
| bottom-right-below-min-width | 20/20 | 19/20 | [7] | 300x300 | 1604,764 |
| bottom-right-below-min-height | 20/20 | 19/20 | [7] | 400x200 | 1504,864 |
| bottom-right-below-min-both | 20/20 | 20/20 | [4] | 300x200 | 1604,864 |
| bottom-right-far-below-min | 20/20 | 19/20 | [3] | 300x200 | 1604,864 |
| bottom-left-smaller | 20/20 | 20/20 | [6] | 200x150 | 16,914 |
| bottom-left-below-min-both | 20/20 | 20/20 | [7] | 300x200 | 16,864 |
| top-right-smaller | 20/20 | 20/20 | [4] | 200x150 | 1704,56 |
| top-right-below-min-both | 20/20 | 20/20 | [4] | 300x200 | 1604,56 |
| bottom-smaller | 20/20 | 20/20 | [4] | 200x150 | 860,914 |
| bottom-below-min-both | 19/20 | 20/20 | [3] | 300x200 (1 run 400x300) | 810,864 (1 run 760,764) |
| right-smaller | 20/20 | 20/20 | [4] | 200x150 | 1704,485 |
| right-below-min-both | 20/20 | 20/20 | [2] | 300x200 | 1604,460 |
| top-left-below-min-both | 20/20 | 20/20 | [12] | 300x200 | 16,56 |
| invalid-size-zero | 20/20 | 20/20 | n/a | no window (exit 2) | n/a |

- Raised flags (`size_raised/raised_w/raised_h`), recorded only: `true/true/true` for the below-min-both and far-below cases, `true/true/false` for below-min-width, `true/false/true` for below-min-height, `false/false/false` for at-min, smaller and larger. In the failing runs of "smaller" they read `true/true/true` because the rectangle was still 400x300 when spawn-at exited: another instance of "the flags describe the rectangle at exit" (section 6.7 item 6).
- Failure signature, all regimes: the printed rectangle is the fixture's default 400x300 at the position for 400x300 (for example bottom-right 1504,764), `COMMIT_RESOLVED SIZE_SETTLED`, no `size-changed` sample inside the 30 ms window. This is the late-resize bug (6.4 rows 3 and 3b); the oracle judges it against the final size, so these runs fail on both position and size.

**Harness against direct drive, `placement/center-200x150`, regime (a), allocation -> `size-changed`:**

| Source | Runs | Mean | No sample in 30 ms |
| :--- | :---: | :---: | :---: |
| `spawn-at-verify run`, isolated (batch 1) | 20 | 10.3 ms | 0 |
| `spawn-at-verify run`, isolated (batch 2, after the direct drives) | 20 | 9.7 ms | 0 |
| fixture driven through `spawn-at`, isolated (section 6.6, earlier) | 18 | 8.6 ms | 2 |
| same, today, no minimum flags | 20 | 8.3 ms | 0 |
| same, today, `--min-width 100 --min-height 100` | 20 | 9.5 ms | 0 |
| same, today, bottom-right + `--min-width 100 --min-height 100` | 20 | 9.5 ms | 0 |

`run` is 1.4 to 2.0 ms slower on average than the direct drive of the same minute (10.3 and 9.7 against 8.3 ms). That the harness itself adds this (it passes `--json`, and probes and snapshots windows around the action) is a **HYPOTHESIS**; the difference is small against the 30 ms window and was not shown to matter. The fixture minimum flags did not change the timing by more than 1.2 ms (8.3 against 9.5 ms). `center-1200x800` via `run`, isolated: 20 of 20, mean 28.1 ms (the closest approach to the 30 ms limit of any measured group).

**Skipped as UNDEFINED (no spec asserts them):** exit code when the window ends mis-positioned (6.4 row 5); size of an oversized request, and where a centre-anchored oversized window goes (rows 7, 8; no overflow case spec was written, although the oracle table-tests it); the INFO "below toolkit minimums" threshold `w < 100 || h < 80` (`geometry.rs:744-745`), which differs from the extension's 100x60 floor (`extension.js:33-34`) (row 4b); the default margin 16 as a documented behaviour (row 10); which size the anchor uses while the app has not answered (6.1 row 13). **Not asserted by design:** `size_raised`, `raised_w`, `raised_h`.

**Not done.** Why (a1) was slow. Whether the harness adds 1 to 2 ms. A recorded real failing run of one of these specs for `render` (the fixtures list in `crates/spawn-at-verify/tests/fixtures/README.md` is unchanged; the existing `real_placement_center_200x150_fail_run.json` is the late-resize recording). GTK3, Qt and Electron apps. The fixture minimum is enforced by mutter through the client's size hints; whether other toolkits behave the same was not tested.

### 6.11 Live results after the late-resize fix (2026-10-12)

**Correction, added with 6.11.1 below.** The "failures" counted in this section were judged on the rectangle `spawn-at` printed at exit. In 195 of the 215 loaded fixture failures the extension moved the window to exactly the expected place a few milliseconds after `spawn-at` had printed (the other 20 are the top-left control, where only the printed size was stale), so those windows did not end up mis-placed; the printed rectangle was stale. The oracle now judges the window's final rectangle (6.11.1), and the tables in this section are the "judged on the printed rectangle" numbers. They are kept as measured.

Fix under test: `9e6b65d` (modern tree only), a second size check in the reveal's own tick that logs `LATE_RESIZE_CORRECTED` and re-anchors. Binary under test `c7c4084` (HEAD; `is_dirty_build: false` in every report). No product code, spec, oracle, tolerance or fixture was changed for these measurements; the only new code is `cargo xtask burn-cpu` (`crates/xtask/src/burn_cpu.rs`). All tables: GNOME 46 Wayland, one machine (22 CPUs, `--virtual-monitor 1920x1080`, workarea `0,40 1920x1040`), other desktop windows open (4) and ordinary desktop load. Raw output is kept locally in the gitignored `verify-results/gate-20261012/`; the throwaway drivers were in the gitignored `scratch/`. Each statement is labelled **CONFIRMED** (shown by a log, a count or a line of code) or **HYPOTHESIS**.

**Step 0: the running shell has the fix. CONFIRMED.** (1) The installed `extension.js` has the same sha256 as `assets/gnome/modern/extension.js` (`be88c6c6...8d8a`); it was written at 13:41:27 and `gnome-shell` (pid 2464446) started at 13:41:43, after the log-out and log-in. That alone is circumstantial (**HYPOTHESIS**, not tested here: the Shell caches imported modules, which is why a log-out and log-in, not `gnome-extensions disable/enable`, was used). (2) Direct: the string `LATE_RESIZE_CORRECTED` exists only in the fix (`git log -S` finds `9e6b65d` alone; absent from the legacy tree), and the journal, written by `gnome-shell[2464446]`, holds `+403921.11ms | LATE_RESIZE_CORRECTED win=[id=604717961 ... class="verify.placement.bottomrightsmaller"] positioned=(400x300) actual=(200x150)` (13:53:19, attempt 5 of `bottom-right-smaller` under `burn-cpu`; that run passed). An idle spawn with a 120 ms fixture paint delay did not trigger it (`ACTOR_MAP` came 160 ms after `WINDOW_CREATED` and the resize was answered within 10 ms of the size request, so nothing landed between `UNCLOAK_4C` and the reveal), so load was the trigger.

**`burn-cpu`.** `cargo xtask burn-cpu --threads N --seconds S` spins N threads inside one process (no child processes), N in 1..=1024, 0 < S <= 86400. Checked: 22 threads for 4 s gave 23 threads and 2140% CPU in `ps`, exit 0, and no `xtask` process afterwards (`pgrep -x xtask`); `--threads 0` exits 1 with the usage text. The load phase started it with 22 threads and a 90 s warm-up, so the 1-minute load average had risen (21 to 29, limit for "busy" is 11) before the first measured run.

**Load average (the new `report.json` fields).** The busy rule is `max(load_avg_start.one, load_avg_end.one) > cpu_count / 2 = 11`. `run` and `report.md` print no warning; the warning is produced only by the TUI drawer (`tui/views/drawer/mod.rs:47`), which was **not** opened here, so the "busy" column below is computed from the stored fields with the same rule.

| Gate | 1-min load at run start / end (range over the stored runs) | Runs over the busy limit |
| :--- | :--- | :---: |
| (a) isolated, idle | 0.80 - 2.67 at start and 0.80 - 2.67 at end (start and end differ within a report in 20 of 320) | 0 of 320 |
| (a) `--repeat 20`, idle | 1.39 - 1.83 / 1.36 - 1.81 | 0 of 320 |
| (b) isolated, `burn-cpu` | 21.13 - 28.83 at start and 21.13 - 28.83 at end (differ within a report in 94 of 320) | 320 of 320 |
| (b) `--repeat 20`, `burn-cpu` | 27.01 - 29.67 / 27.19 - 29.72 | 320 of 320 |
| (c) zenity, `burn-cpu` | 29.23 at start, 26.50 at end (`/proc/loadavg` read by the driver; no report) | busy |
| (d) calibrate delay-0 / 50 / 500 | 1.42 / 1.55 / 1.52 at start; 1.74 after the last | 0 |
| (d) full library `run` | 1.47 / 1.62 | 0 |
| (e) text editor x 20 | 1.48 / 1.05 | 0 |

The idle baseline here is about 1 to 2, not 0.7 to 0.9 as in section 5, because the desktop session itself (this assistant's client, an IDE) was running; the user did not touch the machine during the measurements.

**Before and after.** "Before" is section 6.10 (pre-fix). Auto check = `auto_status` of the stored result (the rectangle `spawn-at` printed against the oracle), unchanged.

| Gate | Before (pre-fix) | After |
| :--- | :---: | :---: |
| (a) 16 specs x 20, isolated, idle | 95 / 320 (29.7%) and 318 / 320 (99.4%) (two batches, 6.10) | **320 / 320 (100%)** |
| (a) 16 specs x 20, `--repeat 20`, idle | 315 / 320 (98.4%) | **320 / 320 (100%)** |
| (b) 16 specs x 20, isolated, 22-thread `burn-cpu` | not measured; nearest: `center-200x150` 8 / 20 passed with 22 busy shell loops (2.1) | **212 / 320 (66.3%)**, 108 failures |
| (b) same, `--repeat 20` | not measured | **213 / 320 (66.6%)**, 107 failures |
| (c) zenity 500x450, centre, 20 runs, `burn-cpu` | 16 / 20 and 19 / 20 centred in two batches (2.1; load not recorded) | **17 / 20** centred; 3 failures (not a late resize, see below) |

Target "0 failures in (a) and (b)": **met for (a), NOT met for (b).** Nothing was relaxed to make a gate pass.

`LATE_RESIZE_CORRECTED` counts (window-scoped, from each result's trace; the journal total since the shell started, 152, equals the sum of these plus the one step 0 event):

| Gate | Runs with the event | Of those, passing | Failing runs with the event |
| :--- | :---: | :---: | :---: |
| (a) isolated | 0 | 0 | 0 of 0 failing |
| (a) `--repeat 20` | 1 (`bottom-right-larger`) | 1 | 0 of 0 failing |
| (b) isolated | 60 | 60 | **0 of 108** |
| (b) `--repeat 20` | 85 | 85 | **0 of 107** |
| (c) zenity | 5 | 5 | **0 of 3** |
| (d) calibrate, full library, (e) | 0 | n/a | n/a |

By code reading each event is a window the old code would have left anchored for its old size (the resize landed after `UNCLOAK_4C` and before the reveal; nothing can land between the new check and `ANCHOR_ARMED`). That the old code would have failed those 145 fixture runs (60 + 85) and the 5 zenity runs was not tested (the old code was not re-run): **HYPOTHESIS** as a count, **CONFIRMED** as a mechanism.

**What the 215 failing fixture runs under load have in common. CONFIRMED (every trace; `verify-results/gate-20261012/b_load.fail_traces.txt`, `b_rep.fail_traces.txt`).** All 215 resolve `COMMIT_RESOLVED reason=SIZE_SETTLED finalFrame=(400x300)`, have no `size-changed` sample before it, show `UNCLOAK_4C positioned=(400x300) actual=(400x300)`, reveal at the size the anchor was computed for (215 of 215), and log `ANCHOR_ARMED`. The app's resize therefore arrived **after** the reveal, which is the reactive anchor's job, not the new check's. `spawn-at` had by then printed the old rectangle (for example `x=760,y=764,w=400,h=300`) and exited, and the harness judges that printed rectangle. In 195 of the 215 the full journal holds an `ANCHOR_CORRECTION` for the window to exactly the position and size the spec expects (97 + 98; for example `size=(300x200) from=(760,764) to=(810,864)`, 43.8 ms after `ANCHOR_ARMED`); the other 20 (11 + 9) are `top-left-below-min-both`, whose position does not depend on the size, so only the printed size was stale (`16,56 400x300`, expected `300x200`). Across all 277 windows that got an `ANCHOR_CORRECTION` in the load phase the first one came 2.0 to 134.4 ms after `ANCHOR_ARMED` (median 26.5, 90th percentile 58.7). So the log says the extension ended every one of the 215 windows where the oracle expects it, but the window was revealed at the old size and position first. That the move is visible on screen was not looked at (**HYPOTHESIS**); the final window geometry was not re-read for the fixture runs (the harness closes the windows), so "ended correct" rests on the logged correction (**CONFIRMED** as a log, **HYPOTHESIS** as pixels).

A failing trace, `bottom-below-min-both` under load (window-scoped, from `COMMIT_RESOLVED` to the end of the run; no `LATE_RESIZE_CORRECTED`):

```
COMMIT_RESOLVED reason=SIZE_SETTLED finalFrame=(400x300) target=(200x150) safe=(200x150)
APPLY_ANCHOR eff=(400x300) bounds=(0,40,1920x1040) final=(760,764)
UNCLOAK_4C positioned=(400x300) actual=(400x300) needsReanchor=false
UNCLOAK opacity=255 frame=(760,764,400x300)
UNCLOAK_4D_REVEAL requestedSize=(200x150) postFrame=(760,764,400x300)
ANCHOR_ARMED
ANCHOR_CORRECTION size=(300x200) from=(760,764) to=(810,864)      (+43.77 ms after ANCHOR_ARMED)
printed by spawn-at: x=760 y=764 w=400 h=300  ->  auto-fail: expected [810, 864, 300x200]
```

**Per spec, auto check passes (a) isolated / (a) `--repeat` / (b) isolated / (b) `--repeat`; events in (b) / (b) `--repeat`.** Marked **known flaky under CPU load**: every spec but `bottom-right-larger`. At idle none failed in 640 runs.

| id | (a) | (a) rep | (b) | (b) rep | events (b) / rep | known flaky under load |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| bottom-right-smaller | 20 | 20 | 15 | 11 | 2 / 4 | yes |
| bottom-right-larger | 20 | 20 | 20 | 20 | 0 / 0 | no (40 of 40 under load; why is not explained) |
| bottom-right-minsize-at-min | 20 | 20 | 9 | 11 | 2 / 4 | yes |
| bottom-right-below-min-width | 20 | 20 | 15 | 11 | 8 / 5 | yes |
| bottom-right-below-min-height | 20 | 20 | 10 | 14 | 4 / 10 | yes |
| bottom-right-below-min-both | 20 | 20 | 14 | 13 | 4 / 6 | yes |
| bottom-right-far-below-min | 20 | 20 | 12 | 15 | 3 / 6 | yes |
| bottom-left-smaller | 20 | 20 | 10 | 14 | 2 / 5 | yes |
| bottom-left-below-min-both | 20 | 20 | 14 | 12 | 5 / 5 | yes |
| top-right-smaller | 20 | 20 | 13 | 16 | 6 / 6 | yes |
| top-right-below-min-both | 20 | 20 | 16 | 12 | 6 / 5 | yes |
| bottom-smaller | 20 | 20 | 16 | 16 | 3 / 9 | yes |
| bottom-below-min-both | 20 | 20 | 11 | 11 | 3 / 5 | yes |
| right-smaller | 20 | 20 | 17 | 16 | 5 / 5 | yes |
| right-below-min-both | 20 | 20 | 11 | 10 | 4 / 7 | yes |
| top-left-below-min-both | 20 | 20 | 9 | 11 | 3 / 3 | yes (control; only the printed size is stale) |
| **total** | **320** | **320** | **212** | **213** | **60 / 85** | |

Two failing runs from the step 0 loop (attempts 1 and 2, `bottom-right-smaller`, load 19 - 20, before the gates) have the same trace: `ANCHOR_ARMED` at `+389516.69`, `ANCHOR_CORRECTION size=(200x150) from=(1504,764) to=(1704,914)` at `+389572.24`; they are not counted in the tables.

**(c) zenity, 20 runs under load, `spawn-at spawn --class '*' --size 500 450 --anchor center --json -- env GDK_BACKEND=wayland zenity --text-info`, window re-read with `spawn-at query windows` 1.3 s after exit.** 17 of 20 ended at `710,335 500x450` (the centre); 12 of those had an `ANCHOR_CORRECTION`, 5 a `LATE_RESIZE_CORRECTED`. The 3 failures (runs 1, 3, 5) ended at `810,360 300x400` and printed `Requested size (500x450), final size is 300x400; placed at (810, 360) [300x400].` There is **no `SIZE_SETTLED` to `ANCHOR_ARMED` trace for them because the extension never dispatched**: the journal for each has only `ARM_SPAWN`, `WINDOW_CREATED` and `ACTOR_MAP` (`class="zenity"`), no `DISPATCH`, `CLOAK` or `SET_SIZE_REQUEST`. `LATE_RESIZE_CORRECTED` did not fire (0 of 3). Time from `ARM_SPAWN` to `WINDOW_CREATED`: **1338, 1242 and 1356 ms in the 3 failures, 469 to 1083 ms in all 17 dispatched runs** (a clean split, CONFIRMED). A wildcard target (`--class '*'`) expires after `WILDCARD_EXPIRY_MS = 1200` (`extension.js:32`, `:1394-1397`; `README.md:104`), which fits: the window appeared after the registration had expired. That this is the cause is a **HYPOTHESIS** (the code that consumes `_wildcardTarget` was not traced); that a named `--class` target (15 s expiry) would be unaffected was not run. This is a startup-latency problem of the app under load, not a resize race, and it exists independently of the fix.

**(d) Cloak duration, `spawn-at-verify calibrate --runs 20` (20 separate processes, then 20 runs of `--repeat` in one process; all 40 runs measured for each delay; no run lacked a measurement).** `cloak_duration_ms` = `UNCLOAK` minus `CLOAK`, ms.

| Case | Mode | After: min | median | max | Before (section 5, delay-500 only) |
| :--- | :--- | :---: | :---: | :---: | :--- |
| delay-0 | separate | 17.67 | 18.92 | 34.28 | not measured with `calibrate` |
| delay-0 | repeat | 17.43 | 18.85 | 20.31 | not measured |
| delay-50 | separate | 16.73 | 18.59 | 19.32 | not measured |
| delay-50 | repeat | 18.31 | 19.30 | **55.95** | not measured |
| delay-500 | separate | 15.55 | 17.67 | 18.82 | 26.6 - 41.7 isolated (medians 30.9 - 32.4); 7.29 / 8.60 / 18.88 and 32.91 / 43.03 / 58.99 in the two batches |
| delay-500 | repeat | 9.80 | 17.62 | 19.85 | 7.6 - 17.5 fast regime (medians 8.7, 9.2); 6.85 / 9.63 / 140.18 and 6.97 / 13.64 / 57.24 in the two batches |

The 100 ms budget did not regress: the largest of 120 measured runs is 55.95 ms, below both 100 ms and the 61.9 ms largest seen in section 5 after its fix. **No claim of "no change beyond noise" is made.** There is no "before" for delay-0 and delay-50, and the delay-500 "before" batches differ from each other by more than the difference to today (isolated medians 31 - 43 ms before, 17.7 ms now; fast-regime medians 8.7 - 13.6 ms before, 17.6 ms now). Two things are known: (1) **CONFIRMED by code (`extension.js:863-880`)**: when no late resize is found the new loop does one synchronous size comparison and awaits nothing, so in a run without a `LATE_RESIZE_CORRECTED` the fix adds no wait; the 120 measured cloak runs had none. (2) The median `UNCLOAK_4C` to `UNCLOAK` gap is today 10.5 - 13.2 ms in the six batches where section 5 recorded 1.1 - 4.8 ms, and the median `COMMIT_RESOLVED` to position gap is 5.3 - 6.6 ms, the range of section 5's fast regime (5.3 - 10.1 ms), where its isolated regime had 23 - 38 ms; the machine state (windows open, uptime of the Shell) differs. Whether the old extension would give today's numbers cannot be shown without reloading the old code (a log-out) and was not done: **HYPOTHESIS** that the change is the machine's state, not the code. The 55.95 ms run (delay-50, `--repeat`, run 15) has `4c->uncloak=47.9 ms`, the existing one-frame wait (`FRAME_WAIT_CEILING_MS = 50`) running to near its ceiling, no `LATE_RESIZE_CORRECTED`, `COMMIT_RESOLVED reason=AT_TARGET` (raw slice in `verify-results/calibrate-slices/1791794390-repeat-run15.log`); why the frame took 48 ms is not known.

**Full library `spawn-at-verify run` (no filter).** `Total: 35, Passed: 27, Failed: 0, Skipped: 0, Blocked: 0`; the 8 non-passed entries are `kind = both` specs awaiting a human verdict (`PENDING`): all 8 have `auto_status: auto-pass`, as do all 35 results. Exit code 0. Session `verify-results/20261012-084106-harsh`.

**(e) Real app: `spawn-at spawn --size W H --anchor bottom-right --json gnome-text-editor`, 10 runs per size, window re-read 1.5 s after exit, editor killed between runs.** Workarea `0,40 1920x1040` (right edge 1920, bottom edge 1080).

| Request | Runs | Printed at exit | Final (re-read) | Edges (right / bottom) | Overflow | Events |
| :--- | :---: | :--- | :--- | :---: | :---: | :---: |
| 30x20 | 10 / 10 identical | `1513,964 391x100` | `1513,964 391x100` | 1904 / 1064 | **no** (16 px inside both edges) | 0 |
| 3000x2000 | 10 / 10 identical | `16,56 3000x2000` | `16,56 1920x1040` | 1936 / 1096 | **yes, 16 px past the right and the bottom edge** | 0 |

30x20: the editor's own minimum is 391x100 and the anchor is correct for it (CLI: `window minimum is 391x100, placed at (1513, 964)`). 3000x2000: the CLI prints `WARNING: Window is oversized (3000x2000); bottom and right margins ignored.`, the app is limited to 1920x1040, and the position is the top-left plus margin, so the window overflows by the margin. This is 6.4 rows 6 (INTENDED) and 7 (size not reduced to the workarea: UNDEFINED); it is reported, not changed. The printed 3000x2000 is the rectangle at exit; the re-read 1.5 s later is 1920x1040.

**Effect on section 6.4.** Rows 2, 3 and 3b (final position correct for the final size; late resize) are fixed for a resize that lands before the reveal (idle: 0 of 640 failed; 150 windows under load, 145 fixture and 5 zenity, were re-anchored by the new check). They are **not** fixed for a resize that lands after the reveal under heavy load: the reactive anchor moves the window afterwards (log), but `spawn-at` prints the old rectangle and the window is shown first at the old size. Row 4 (the "window minimum is WxH" text taken while the window is still resizing) has the same cause and is unchanged.

**Not done.** Whether the 215 + 3 failures are visible on screen. The TUI drawer's busy warning. A "before" for the cloak numbers of delay-0 and delay-50, and for the 16 specs under load (the old extension is not loadable without a log-out). A fixture-side re-read of the final geometry after a failing run. A named `--class` (instead of `'*'`) zenity run under load. Why `bottom-right-larger` never failed under load. GTK3, Qt and Electron apps.

### 6.11.1 Final-state oracle, post-reveal warning and re-measure (2026-10-12)

**What changed in `spawn-at-verify` (no product code).** Commit `d01687b`: when the rectangle `spawn-at` printed fails the geometry check, the runner keeps the window alive for `SETTLE_MS = 1300` ms (1 s for the extension's late-resize listener plus 300 ms), re-reads it with `spawn-at query windows --json` (the same read the zenity gate used: the entry with the claimed window id, fields `x y w h`) and judges the geometry on that final rectangle. Printed != final adds the warning `spawn-at printed a stale rectangle (printed A, final B)` to the result message, the drawer and `report.md`; it is not a failure. A wrong final rectangle still fails (the message names the final and the printed rectangle); a window that cannot be re-read keeps the verdict on the printed rectangle and says so. A printed rectangle that already matches waits for nothing: `settle_final_rect` calls its wait closure only when `settle_needed` is true (test `test_no_settle_wait_when_the_printed_rectangle_matches` counts the calls: 0). The final rectangle is stored in `TestResult.final_rect` (absent from `report.json` when no settle happened), and the log slice is taken again after the wait. `duration_ms` is not extended by the wait. Commit `e47c41f`: the trace analyser warns `placement corrected after reveal (+N ms)` for the first `ANCHOR_CORRECTION` after `UNCLOAK` (N = time after the `UNCLOAK`); a correction before the `UNCLOAK` (`LATE_RESIZE_CORRECTED`) is not reported; the drawer shows each trace warning as a `Warning` row and `report.md` has a `## 5. Warnings` section. Both are tested in both directions (see the test names `test_final_rect_wrong_fails`, `test_stale_print_with_right_final_passes_with_warning`, `test_printed_right_needs_no_settle_and_no_warning`, `test_correction_after_reveal_is_a_warning_with_its_delay`, `test_correction_before_reveal_is_not_a_warning`). **CONFIRMED** (tests ran).

**Counting caveat. CONFIRMED by code.** The post-reveal warning is read from the run's log slice. With no settle the slice ends when `spawn-at` exits (whole-second bounds), so a correction made later is not seen; the post-reveal counts below are lower bounds for runs that did not settle.

**Live re-measure.** Binary `e47c41f` (HEAD, clean build, preflight pass), fix `9e6b65d` live (same shell as 6.11), 16 resize specs x 20, each run a separate `spawn-at-verify run --glob placement/<id>`, isolated (no `verify.*` window and no fixture process before any run; 0 exceptions in 640), once idle and once under `cargo xtask burn-cpu --threads 22` (90 s warm-up). Raw: `verify-results/gate-20261012/g3/` (gitignored).

| Condition | Final-pass | Fails | Settle reads (stale print) | Post-reveal-correction warnings | `LATE_RESIZE_CORRECTED` | Busy (load1 > 11) | 1-min load over the batch |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :--- |
| idle, isolated | **320 / 320** | **0** | 3 | **3** | 0 | 0 / 320 | 1.26 - 6.24 (rose while the batch ran; start 1.32, end 3.01) |
| `burn-cpu` 22 threads, isolated | **320 / 320** | **0** | 99 | 134 | 62 | **320 / 320** | 37.97 - 48.53 (start 37.19, end 44.95) |

Target "idle: 0 failures and 0 post-reveal corrections": **0 failures met; 0 post-reveal corrections NOT met (3 of 320).** Nothing was relaxed.

The three idle post-reveal corrections (all passed on the final rectangle; none had `LATE_RESIZE_CORRECTED`; the window was shown at 400x300, the resize arrived after the reveal):

| Run | Printed | Final (re-read 1.3 s later) | Correction after `UNCLOAK` |
| :--- | :--- | :--- | :---: |
| `bottom-left-below-min-both` 12 | `16,764 400x300` | `16,864 300x200` | +42.8 ms |
| `bottom-below-min-both` 10 | `760,764 400x300` | `810,864 300x200` | +53.3 ms |
| `right-smaller` 18 | `1504,410 400x300` | `1704,485 200x150` | +7.8 ms |

Trace of the first of them (window-scoped, `COMMIT_RESOLVED` to the end; `BATCH_STEP`, `POSITION_SET`, `UNCLOAK_4A/4B` omitted):

```
+9041193.61 COMMIT_RESOLVED reason=SIZE_SETTLED finalFrame=(400x300) target=(200x150) safe=(200x150)
+9041196.08 APPLY_ANCHOR eff=(400x300) bounds=(0,40,1920x1040) final=(16,764)
+9041196.90 AFTER_SET_POSITION_ANCHORED opacity=0 frame=(16,764,400x300)
+9041201.22 UNCLOAK_4C positioned=(400x300) actual=(400x300) needsReanchor=false
+9041219.35 UNCLOAK opacity=255 frame=(16,764,400x300)
+9041220.07 ANCHOR_ARMED
+9041262.17 ANCHOR_CORRECTION size=(300x200) from=(16,764) to=(16,864)
```

All three idle traces are in `g3/g3_idle.warn_traces.txt`. Idle was 0 of 640 failing in 6.11 on the printed rectangle, with 0 `LATE_RESIZE_CORRECTED` and no post-reveal warning then (the warning did not exist), so whether these 3 post-reveal corrections are new or were present and invisible before is not known. Their printed rectangles were stale and would have failed a printed-rectangle check, and 0 of 640 failed in 6.11, so the rate differed between the batches; the idle 1-min load here rose to 6.24 against 0.8 - 2.7 in 6.11 (the cause of the difference is not established).

**Per condition, loaded: how the 99 stale prints relate to the 134 warnings.** 94 runs have both a stale print and a post-reveal correction; 40 have a post-reveal correction with the printed rectangle already final (`spawn-at` waited long enough to print the corrected rectangle but the window was revealed at the old geometry first; 18 of these are `bottom-right-larger`); 5 have a stale print and no `ANCHOR_CORRECTION` (all `top-left-below-min-both`, the control: the position does not depend on the size, so only the printed size was stale and the extension logged no correction). Every settle read found the window at the expected rectangle (99 of 99): 0 failures. **CONFIRMED** (`g3_load.flat.tsv`). The delay of the first post-reveal correction after `UNCLOAK`: idle n=3, 7.8 / 42.8 / 53.3 ms (min / median / max); loaded n=134, min 2.5, median 33.8, p90 140.2, max 330.7 ms.

**Per spec** (each cell 20 runs; final-pass | stale-print warnings | post-reveal-correction warnings | `LATE_RESIZE_CORRECTED`; busy flag: idle 0 of 20 for every spec, loaded 20 of 20 for every spec):

| Spec | idle | loaded |
| :--- | :---: | :---: |
| bottom-right-smaller | 20 \| 0 \| 0 \| 0 | 20 \| 6 \| 6 \| 6 |
| bottom-right-larger | 20 \| 0 \| 0 \| 0 | 20 \| 0 \| 18 \| 1 |
| bottom-right-minsize-at-min | 20 \| 0 \| 0 \| 0 | 20 \| 6 \| 9 \| 3 |
| bottom-right-below-min-width | 20 \| 0 \| 0 \| 0 | 20 \| 6 \| 7 \| 5 |
| bottom-right-below-min-height | 20 \| 0 \| 0 \| 0 | 20 \| 9 \| 10 \| 8 |
| bottom-right-below-min-both | 20 \| 0 \| 0 \| 0 | 20 \| 6 \| 7 \| 4 |
| bottom-right-far-below-min | 20 \| 0 \| 0 \| 0 | 20 \| 8 \| 10 \| 2 |
| bottom-left-smaller | 20 \| 0 \| 0 \| 0 | 20 \| 6 \| 8 \| 6 |
| bottom-left-below-min-both | 20 \| 1 \| 1 \| 0 | 20 \| 3 \| 6 \| 2 |
| top-right-smaller | 20 \| 0 \| 0 \| 0 | 20 \| 8 \| 10 \| 3 |
| top-right-below-min-both | 20 \| 0 \| 0 \| 0 | 20 \| 4 \| 7 \| 4 |
| bottom-smaller | 20 \| 0 \| 0 \| 0 | 20 \| 6 \| 7 \| 7 |
| bottom-below-min-both | 20 \| 1 \| 1 \| 0 | 20 \| 8 \| 10 \| 3 |
| right-smaller | 20 \| 1 \| 1 \| 0 | 20 \| 8 \| 8 \| 4 |
| right-below-min-both | 20 \| 0 \| 0 \| 0 | 20 \| 10 \| 11 \| 1 |
| top-left-below-min-both (control) | 20 \| 0 \| 0 \| 0 | 20 \| 5 \| 0 \| 3 |
| **total** | **320 \| 3 \| 3 \| 0** | **320 \| 99 \| 134 \| 62** |

**What the loaded failures of 6.11 were. CONFIRMED.** On the final rectangle there are none: 0 of 320 failed, where 6.11 had 108 and 107 failures on the printed rectangle for the same specs under load. The windows were revealed at the size the app had before its resize and moved to the expected place by the extension's reactive anchor 2.5 - 330.7 ms after the reveal; `spawn-at` printed the earlier rectangle. What is still wrong, and now shows as warnings, is that the window is shown at the wrong geometry first and `spawn-at`'s printed rectangle is not where the window ends up. Whether the jump is visible on screen was not observed (**HYPOTHESIS**: it is, for one to a few frames; the actor is uncloaked at the old geometry).

The 6.11 "known flaky under CPU load" marks stay as a statement about the printed rectangle (`spawn-at`'s own output), not about final placement; the specs no longer fail the harness for it. The load differed from 6.11 (37.97 - 48.53 now against 21.13 - 28.83 then, same 22 threads), so the stale-print rate (99 / 320 against 108 / 320) is not a controlled comparison.

**Cloak gap check, `UNCLOAK_4C` to `UNCLOAK`, stored pre-fix reports against today. No logout.** The gap is read the way `calibrate` prints `4c->uncloak`: for the claimed window, the first `UNCLOAK` minus the first `UNCLOAK_4C` in the report's own log slice. All `report.json` files in `verify-results/` (2544 dirs) were scanned for `cloak/delayed-paint/*` results (and `repeat_runs`); "pre-fix" = `generated_at` before 2026-10-12 13:41:43 +0530 (the shell with the fix started; epoch 1791792703). The script is `scratch/gap.jq` (throwaway, not committed); output: `g3/cloak_gap_all.tsv`, `g3/cloak_gap_stats.txt`. Median / p95 (nearest rank) in ms:

| Case | Mode | Pre-fix: all stored sessions | Pre-fix: sessions since `calibrate` phase gaps (`444f409`, 10-12 07:09 +0530 on) | Today (fix live, `calibrate --runs 20 --mode both`, 20 runs) |
| :--- | :--- | :---: | :---: | :---: |
| delay-0 | separate | n=48, 5.63 / 13.04 | n=40, **5.44 / 9.72** | **13.15 / 14.30** |
| delay-0 | repeat | none stored | none stored | 12.20 / 13.90 |
| delay-50 | separate | n=9, 11.60 / 14.57 (10-11 sessions only) | none stored | **12.40 / 13.90** |
| delay-50 | repeat | none stored | none stored | 12.75 / 14.30 |
| delay-500 | separate | n=102, 2.50 / 10.68 | n=58, **2.55 / 3.98** | **11.55 / 12.40** |
| delay-500 | repeat | none stored | none stored | 10.45 / 12.20 |

Pre-fix delay-0 and delay-500 rows are the comparable ones (same specs, separate `run` processes, same harness era in the middle column). **There is no comparable pre-fix data for delay-50 beyond 9 single runs from 10-11, none for any `--repeat` regime, and no load average for any pre-fix session (`load_avg_*` was added in `3bef115` today), so "same regime" cannot be checked.** The 150 ms case has 9 pre-fix runs and 1 today and is not compared.

Result: **the gap is larger today by about 7.7 ms (delay-0 median 5.44 to 13.15) and 9.0 ms (delay-500 median 2.55 to 11.55)** against the same-era sessions. The pre-fix gaps are bimodal: a fast mode (about 1.2 - 5 ms; 92 of 102 delay-500 runs) and a slow mode (about 9 - 15 ms; 10 of 102 delay-500 runs, median 10.84, p95 12.60). Of the 42 stored post-fix runs per case (the 20 + 20 of `calibrate` plus 2 others), 40 of 42 delay-500 runs and 42 of 42 delay-0 runs are in the slow mode (slow-mode medians: delay-500 pre 10.84 / post 11.28; delay-0 pre 7.18 over 21 runs / post 12.90). **CONFIRMED:** the numbers above. **CONFIRMED by code:** the fix adds no `await` and no wait when no late resize is found (`_reanchorLateResize` returns false after one `get_frame_rect()`; `extension.js` `LATE_RESIZE_MAX_PASSES` loop), and no run in the 120 cloak runs logged `LATE_RESIZE_CORRECTED`. **HYPOTHESIS (not tested, needs the old extension, i.e. a log-out):** the shift to the slow mode comes from the compositor's frame timing at that time (the frame wait before the reveal returns at once when frames are already being drawn and waits for the next frame when the shell is idle), not from the fix. The cloak duration itself (`CLOAK` to `UNCLOAK`) stayed far under the 100 ms budget (max 55.95 ms of 120 runs, 6.11); the budget has not regressed, the gap has risen. A before/after with the old extension would settle the cause; it was not done.

**Read-only checks.**

- (a) The verify runner never passes the flag that makes `spawn-at` non-blocking (`--no-wait`, global, `src/cli/mod.rs:24-26`). The only place the crate writes it is the TUI's window cleanup, `crates/spawn-at-verify/src/tui/app/state.rs:427`: `cmd.args(["focus", "--id", &term_id.to_string(), "--no-wait"]);`, a `focus` command, never a `spawn`. The `spawn` command line is built by `build_action_command_args` (`crates/spawn-at-verify/src/runner/validation.rs:12-20`: the action's command, then `test.action.args`, then fixture arguments) and run at `crates/spawn-at-verify/src/runner/executor.rs:195-196`; no spec under `verify/` contains `--no-wait` (`grep -rn no-wait verify crates` finds only `state.rs:427`). **CONFIRMED.**
- (b) `verify-results/gate-20261012/load_phase_journal.txt` cannot answer this: it holds only `ANCHOR_ARMED` (657), `ANCHOR_CORRECTION` (277), `ANCHOR_RELEASED` (660) and `LATE_RESIZE_CORRECTED` (150) lines, no `ARM_SPAWN`. The same time range (`+1696451.16 ms` to `+3350877.85 ms` on the shell's clock) was read from the journal instead (`g3/arm_release_journal.txt`, `journalctl --user -b _COMM=gnome-shell`). 697 `ARM_SPAWN` lines fall in it; a spawn counts when at least one earlier window was `ANCHOR_ARMED` and not yet `ANCHOR_RELEASED`. **Isolated runs (6.11 gate (b): 320 placement spawns, plus 37 `verify.cloak.delayedpaint` spawns whose origin was not traced): 0 of 357. `--repeat 20` runs: 304 of 319** (by design the previous repeat's window is retired only on the next arm, and `ANCHOR_RELEASED reason=RETIRED` follows the next `ARM_SPAWN`). **zenity (wildcard `*`): 0 of 20.** The classification of a spawn as isolated / repeat / zenity is by the first overlap and by `target="zenity"`/`"*"`; it is not an independent label. **CONFIRMED** for the counts. So in the isolated regime no run's `ARM_SPAWN` came before the previous run's `ANCHOR_RELEASED`; in `--repeat` it almost always does, which is the regime's definition.

**Not done.** Whether a stale-print jump is visible on screen. The TUI drawer's busy warning (the load flag was computed from `report.json`). The cloak gap with the old extension (needs a log-out). A named `--class` zenity run under load. The 37 `verify.cloak.delayedpaint` spawns inside the load-phase journal range were not traced to a run.

### 6.12 Why the window is revealed at its pre-resize size: investigation (2026-10-12, read-only, nothing implemented)

No product code changed. Labels: CONFIRMED = read from stored or live data in this section; HYPOTHESIS = not tested.

**Mechanism (CONFIRMED from `assets/gnome/modern/commit.js` and 640 stored g3 runs).**

- `CommitLatch.waitForCommit` starts its 30 ms quiet timer at `commit.js:92-95` on any `allocation` sample when the idle timer is still pending, even if the frame size did not change.
- In all 640 stored g3 runs (320 idle, 320 loaded) the first non-initial sample is an `allocation` at the initial 400x300. None was a `size-changed`.
- `SIZE_SETTLED` on a frame other than the final one occurred in 205/320 loaded runs and 3/320 idle runs. Of the 205: 134 were corrected after the reveal, 62 were caught by `LATE_RESIZE_CORRECTED` before it, 9 were not observed in the slice (8 of them top-left anchored, so no position change to log).
- Every post-reveal correction run (134 loaded, 3 idle) has `SIZE_SETTLED` with settled frame != requested size. No run whose latch ended `AT_TARGET` (43 loaded, 139 idle) was corrected.
- That the first allocation is the app's own initial buffer is a HYPOTHESIS: at idle it arrives 1.2 ms (median) after the request, which looks like the compositor reacting to our own `move_resize_frame`. The live sampling below did not settle it.

**Correction to the premise "bottom-right-larger never flashes" (CONFIRMED).** In the stored loaded runs bottom-right-larger is the worst class: 20/20 settled early, 18 post-reveal corrections, 1 pre-reveal correction. It has zero stale-print warnings only because spawn-at printed the rectangle after the grow had landed.

**Per request class, stored loaded runs (n = 20 / 100 / 20 / 180).**

| class | settled on a stale frame | post-reveal corrected | median request->first sample | median settled->real resize (corrected runs) |
| :--- | ---: | ---: | ---: | ---: |
| grow | 20 | 18 | 10.1 ms | 74.5 ms |
| legal shrink | 65 | 39 | 14.9 ms | 47.7 ms |
| exactly at min | 12 | 9 | 14.0 ms | 46.4 ms |
| below min | 108 | 68 | 14.7 ms | 44.3 ms |

At idle the app answers in a median of 9.3 ms (legal shrink), 10.1 ms (at min), 10.5 ms (below min), 17.3 ms (grow) after the request.

- H1 (quiet clock starts at the first sample, not at an answer): supported as the mechanism.
- H2 (below-min is slower than legal): not supported. The classes have the same idle response time and the same flash rate (38% vs 39% loaded).
- H3 (shrink vs grow is the discriminator): not supported as stated. Grow flashes most, because the app needs longer to answer (17 ms vs about 10 ms at idle) and the 30 ms window is about 29 ms after the first sample.
- Every corrected run had the anchor computed for 400x300 (the default), never the final size.

**Delay from SIZE_SETTLED to the app's real resize**, runs that were corrected after the reveal:

| level | n | median | p90 | p95 | p99 | max |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: |
| stored g3 loaded (22 threads) | 134 | 50.6 | 168.7 | 235.2 | 307.6 | 350.0 |
| live idle, sampler harness | 28 | 9.8 | 20.1 | 21.1 | 25.0 | 25.0 |
| live 1 thread | 18 | 8.7 | 16.7 | 30.0 | 30.0 | 30.0 |
| live 11 threads | 105 | 19.9 | 37.4 | 91.2 | 150.8 | 251.4 |
| live 22 threads | 91 | 39.0 | 84.3 | 109.4 | 369.1 | 369.1 |

The loaded tail is long (p99 above 300 ms), so a bounded grace would leave a residual; see the decision below.

**Screencast run, 17:16:12 IST (window 604720095), from the journal.** CLOAK to UNCLOAK is 605.6 ms, matching the drawer. Times in ms from SET_SIZE_REQUEST: first allocation sample +354.9 (frame still 400x300), SIZE_SETTLED +540.1, UNCLOAK +605.1, ANCHOR_CORRECTION +685.2. The cloak ended through the quiet timer (`SIZE_SETTLED`); no cap or timeout fired. The shell main loop was stalled twice: 355 ms before the first sample and 185 ms for a 30 ms timer. The 120 ms idle timer did not fire first because the sample callback removed it (HYPOTHESIS about dispatch order). The screencast session left no session directory in `verify-results/`.

**Signals available on GNOME 46 (typelib `Meta-14`, mutter 46.2, CONFIRMED by gjs introspection and `Meta-14.gir`).**

- `Meta.Window`: signals `size-changed`, `position-changed`, `shown`, `unmanaged`; methods `get_buffer_rect`, `get_frame_rect`, `get_pid`, `get_client_type` (`WAYLAND=0`, `X11=1`), `get_sandboxed_app_id`, `get_stable_sequence`. `get_pid` is documented as spoofable by the client; `get_sandboxed_app_id` covers flatpak and snap and may be null.
- `Meta.WindowActor`: signals `damaged`, `first-frame`, `thawed`; method `has_damage`; property `allocation`. `Meta.ShapedTexture` has a `size-changed` signal.
- `Meta.SurfaceActor` is not in the typelib. `get_pid`, `get_client_type` and `get_sandboxed_app_id` are already called by `logger.js` and `extension.js` inside the shell.
- Not tested live: that `damaged` or `ShapedTexture::size-changed` fires on a client commit after our request.

**Live /proc sampling (item 6).** A throwaway Rust sampler in `scratch/` (not committed) read `/proc/<pid>/stat` and `schedstat` about every 1 ms, with the clock tied to the journal by 16 `SetLogging` echoes per run (round trip 0.3 ms idle, offset spread 0.3 ms). One sample costs 1.2-1.6 µs idle (median).

| level | runs | settled on a stale frame | corrected after reveal | mean 1-min load |
| :--- | ---: | ---: | ---: | ---: |
| idle (3 phases) | 84 | 64 | 28 | 1.2-1.4 |
| 1 thread | 32 | 27 | 18 | 2.3 |
| 11 threads | 120 | 114 | 105 | 12.5 |
| 22 threads | 120 | 96 | 91 | 25.0 |

- The idle stale rate here (65-84%) is far above the 1% of the stored g3 idle runs and 0-1 of 8 in two of my control batches. The cause is not established. Killing the previous fixture with SIGTERM made the next spawn slow in one control (7/8), but the later phases without the kill were also slow. HYPOTHESIS: cold CPU frequency state (intel_pstate, powersave, idle cores at 400 MHz). The load comparison above is therefore not a clean one.
- "App has run since the request and is asleep at SIZE_SETTLED" does not separate the groups. CPU advanced since the request in 100% of stale and non-stale runs. At 11 threads 112/114 stale runs were asleep (non-stale: 5/6); at 22 threads 62/96 stale vs 6/24 non-stale were asleep, and run-queue wait of at least 1 ms was in 93/96 stale vs 24/24 non-stale. The state at settle is the same as at a correct settle, so a scheduler-aware settle (option E) would not tell them apart. CONFIRMED on this data.
- Real apps (3 runs each, idle and 11 threads): gnome-text-editor 30x20, nautilus and gnome-terminal (1 and 3 windows) settled by `SIZE_SETTLED` and none flashed; firefox (snap) flashed 6/6 (corrected 135-312 ms after the request). The window pid matched the sampled process in every run. Chrome was never claimed (`REJECT_UNMATCHED`, class `google-chrome` without `--class`), so there is no Chrome timing.
- Facts per app: all report `proto=Wayland`. Terminal windows share one `gnome-terminal-server` pid (3 and up to 7 windows per pid). Nautilus shares the already running service pid. Firefox runs from `/snap/firefox/...`; its `app` field shows `firefox_firefox`. The `app=` field in the log merges the GTK application id and the sandboxed id, so the sandbox id alone was not observed. Reading `/proc/<pid>/maps`: about 0.2 ms for GTK apps (110-130 KB), 0.8-1.0 ms for firefox (240-270 KB), up to 2 ms at 11 threads. Toolkit libraries seen: libgtk-3 (terminal, firefox), libgtk-4 + libadwaita (editor, nautilus, chrome).

**Decision (rule: short tail -> B + C, long tail -> E if validated, else accept).** The loaded p99 of settled -> real resize is 150-370 ms, so the tail is long. E is not validated, so no scheduler-based settle is recommended. Cost from the stored loaded runs, simulated per option (estimates, not implemented):

| option | flash fixed (of 134) | extra cloak median / p95 |
| :--- | ---: | ---: |
| A: quiet clock from a size-change, 120 ms idle timeout | 83 | 25.9 / 77.8 ms |
| B, G = 100 ms | 109 | 35.1 / 100 ms |
| B, G = 200 ms | 124 | 35.1 / 200 ms |
| B, G = 300 ms | 130 | 35.1 / 230 ms |
| D: accept | 0 | 0 |

A and B cost up to G on an app that never answers (a min-bound app already at its minimum, such as gnome-text-editor at 30x20, which today settles in about 32-40 ms), which breaks the 100 ms provisional cloak budget on every first spawn; C (learned minimum per app id, in memory) would remove that cost for later spawns. The pragmatic recommendation is D now, with the residual of this change being the corrected-after-reveal warning already shown by the verify tool: expect a post-reveal correction in roughly 40% of runs under a 22-thread load. Idle stays low (about 1%) in the verify harness. A grace needs the idle-rate discrepancy above resolved first.

**Nested shell (item 5, report only).** `gnome-shell --headless --wayland --no-x11 --wayland-display=NAME --virtual-monitor 1920x1080` starts on GNOME 46 inside `dbus-run-session` in about 1 s and gets its own D-Bus. The spawn-at extension was not shown to load there. The verify tool reads the user journal (`journalctl --user`), so it would need a file log source or the shell started as a transient user unit, and `WAYLAND_DISPLAY` and `DBUS_SESSION_BUS_ADDRESS` set for it. Isolation of settings must be set up before `dbus-run-session` starts (see the incident below); the safe route is `GSETTINGS_BACKEND=keyfile` plus `XDG_CONFIG_HOME` exported before it. If a nested shell is not workable, a developer switch (a variant name read per spawn from a file in `$XDG_RUNTIME_DIR`, never committed) would let one log-out test several variants.

**Incident during this investigation (CONFIRMED).** A nested-shell experiment wrote `org.gnome.shell enabled-extensions` into the real dconf database: the D-Bus-activated dconf service in the private session did not inherit my `XDG_CONFIG_HOME`. The value was `['spawn-at@harsh.local']` for a while and is back to the original 25-entry list (compared by eye against the value captured at the start). I did not manage to restore it myself; how it came back is not established.

**Not done / partial.** The user stopped the run at about 19:3x; the 22-thread v2 phase had completed by then. No phase was rerun. Real apps were not run at 22 threads, Chrome was not claimed, and `term1` at 11 threads has one run with no events. No fix was implemented.

### 6.13 H4: does the cloak starve the app of frame callbacks? (2026-10-12, no extension change)

H4: the app applies our resize only when it gets a `wl_surface.frame` callback; the compositor does not paint a cloaked (opacity 0) actor, so callbacks stop, and the app resizes only after the reveal. Labels: CONFIRMED = measured here; SOURCE = read from mutter 46.2 source through WebFetch (a small model summarised the files, so the quotes are indirect); HYPOTHESIS = not tested.

**Verdict: H4 is not supported.** Two apps whose resize path is known show no gating by frame callbacks; mutter delivers callbacks to a cloaked surface; the delay sits in the configure's delivery and the app's own CPU time.

**Tooling.** `SPAWN_AT_FIXTURE_TRACE_DIR=<dir>` (commit `fb4e082`) makes the fixture write `fixture-<pid>.log`: its own `Resized`, `RedrawRequested` and commit lines in CLOCK_MONOTONIC ns plus the libwayland client dump (`WAYLAND_DEBUG=client`). CONFIRMED about that dump: its `[ms.µs]` stamp is CLOCK_REALTIME µs modulo 2^32 (matched to 6 µs), not the monotonic clock; I map it with the fixture's own `COMMIT_DONE` lines or the boot offset. The extension's `+N ms` clock was tied to CLOCK_MONOTONIC with 16 `SetLogging` echoes per batch (offset drift between batch start and end: 2-64 µs). After new commits `spawn-at` must be rebuilt (`cargo build -p spawn-at`) or the verify preflight reports a stale binary.

#### 1. Stored g3 runs and the sampler runs (post-reveal corrected runs only)

Times in ms. `a` = resize - UNCLOAK, `b` = resize - SETTLED, `c` = UNCLOAK - SETTLED. The resize time is the `ANCHOR_CORRECTION`. These runs are selected by `a > 0`, so `b` correlates with `c` partly by construction.

| source | n | a median / p90 / p99 / max | b median / p90 / p99 / max | c median | slope of b on c, corr |
| :--- | ---: | :--- | :--- | ---: | :--- |
| g3 loaded (22 threads) | 134 | 33.8 / 141.7 / 297.3 / 330.7 | 50.6 / 168.7 / 307.6 / 350.0 | 13.9 | 1.73, 0.39 |
| sampler harness idle | 28 | 3.6 / 10.7 / 11.7 / 11.7 | 9.8 / 20.1 / 25.0 / 25.0 | 4.8 | 1.20, 0.83 |
| sampler 1 thread | 18 | 3.8 / 12.7 / 24.4 / 24.4 | 8.7 / 16.7 / 30.0 / 30.0 | 4.5 | 2.51, 0.27 |
| sampler 11 threads | 105 | 9.0 / 26.5 / 95.7 / 200.3 | 19.9 / 37.4 / 150.8 / 251.4 | 10.2 | 2.82, 0.91 |
| sampler 22 threads | 91 | 28.6 / 71.9 / 337.4 / 337.4 | 39.0 / 84.3 / 369.1 / 369.1 | 12.0 | 3.79, 0.44 |

- H4 predicts `a` small and about constant per level (one frame). Idle matches (3.6 ms). Loaded does not: the median grows with load (9.0 at 11 threads, 28.6 at 22) and the p90 is 26-142 ms. In the stored loaded runs corr(a, c) is 0.18 (a is not tied to the reveal time), mean a 56.7, sd 62.5.
- Runs where the app resized before the reveal exist at every level, so a cloaked app does resize: sampler data, share of runs: idle 36 caught by the late check + 12 settled `AT_TARGET` of 84; 22 threads 5 + 21 of 120. Stored loaded: 62 late-checked + 43 `AT_TARGET` of 320.
- Non-stale runs, `AT_TARGET` runs with a `size-changed` sample before the quiet timer: size-changed minus the first allocation (the quiet clock start) has median 29.1 ms idle (n=12, in the sampler harness), 19.3 ms at 22 threads (n=21), and in the stored loaded runs 25.7 ms (p95 30.9). So the successful runs land just inside the 30 ms window.

#### 2. Fixture instrumentation through the normal `run` path

`bottom-right-smaller` and `bottom-smaller`, 20 each, interleaved. Conditions: back-to-back (about 1 s apart), idle with 30 s gaps, 22-thread `burn-cpu` (60 s warm-up). All 120 runs produced a trace and parsed.

| | back-to-back | idle, 30 s gap | 22 threads |
| :--- | ---: | ---: | ---: |
| runs / stale `SIZE_SETTLED` / corrected after reveal | 40 / 0 / 0 | 40 / 0 / 0 | 40 / 14 / 10 |
| (a) configure received after SET_SIZE_REQUEST, median / p95 / max | 7.9 / 10.9 / 11.6 | 8.5 / 10.8 / 13.7 | 23.1 / 59.1 / 120.4 |
| ack - configure, median | 0.1 | 0.1 | 0.2 |
| (b) `Resized` event - configure, median / p95 / max | 0.3 / 0.4 / 0.4 | 0.3 / 0.4 / 0.4 | 0.7 / 4.8 / 20.0 |
| (c) resized commit - `Resized` event, median / p95 / max | 0.7 / 1.1 / 1.5 | 0.8 / 1.2 / 1.2 | 4.8 / 21.1 / 35.6 |
| `wl_surface.frame` requests by the fixture | 0 | 0 | 0 |

- **The fixture never asks for a frame callback** (winit 0.29 + softbuffer; 0 `wl_surface.frame` lines in the 120 dumps). H4 cannot apply to it. It applies the configure within 0.3 ms (median) of receiving it.
- **Stage (a) carries the delay.** Under load the 14 stale runs had the configure at a median 41.7 ms (p90 71.1, max 120.4) against 19.3 ms (max 25.3) for the 26 runs that settled correctly. A stale run is one whose configure arrives after about 25-30 ms. Stage (c) adds to it (stale runs: commit - configure median 14.3 ms vs 5.4 overall).
- Where UNCLOAK falls (22 threads): in the 26 correct runs the configure and the commit were both before UNCLOAK. In the 14 stale runs the configure was before UNCLOAK in 7 and the commit in 5, so 9 resized commits happened after the reveal. SIZE_SETTLED came before the configure in 8 of the 14.
- (b) "configure early, applied only at the next callback" is not what happens: the apply step is sub-millisecond at idle and 0.7 ms median loaded (the 20 ms maximum is CPU starvation).
- Idle shows no stale runs in this path (0 of 80), while my earlier sampler harness showed 65-84%. The difference is not explained (the sampler harness spawned the fixture itself); the 6.12 idle comparison should be read with that in mind.

#### 3. Toolkit apps (WAYLAND_DEBUG on the real app)

- **gnome-text-editor (GTK4), idle, 12 runs alternating 500x300 and 580x380.** The resized commit follows the configure by 3-6 ms and always precedes the next frame callback `done` (commit 10-15 ms, next `done` 19-51 ms), so GTK4 did not wait for a callback to commit the resize. The editor remembers its window size, so the first 22-thread batch (11 of 12 runs opened already at the target size) was discarded and redone with alternating sizes.
- **Callbacks while cloaked, from mutter's own timestamp in `wl_callback.done`** (idle editor runs): the first `done` after the request was emitted 5.4-7.5 ms after the request, with the window cloaked (UNCLOAK 18-50 ms). So a cloaked surface does receive callbacks. A callback requested at the resized commit (about 12 ms) was not answered until 46-50 ms, 0.6-1.0 ms before UNCLOAK logs, i.e. in the stage update that `waitForFrames` forces; in between there was no stage update. This is the part of H4's premise that holds: after the first frame, callbacks for the cloaked surface wait for the next stage update. It did not delay the resize.
- **Firefox (snap), idle, 6 runs, 300x200 request (Firefox's minimum is 625 wide).** Mutter stamped its frame-callback `done` events 6.6-17.7 ms after the request, but the client printed them at 108-133 ms together with the configure; before that its socket was not read for about 190 ms (the last event it dispatched was 56 ms before the request). So the 135-312 ms to the correction is the browser's startup, not the compositor withholding callbacks. After the configure arrived it committed 0.3-0.5 ms later. SIZE_SETTLED came at 32-42 ms, UNCLOAK at 40-59 ms, in every run before the configure was even read.
- Under 22 threads, 14 alternating-size editor runs gave 11 real resizes (one run had no events, two opened already at the target size). 5 of the 11 settled stale: configure 22-45 ms, commit 42-83 ms after the request. In 4 of the 5 the commit came after the configure by 10-37 ms, so stage (c) matters for GTK4 under load.

#### 4. Source reading (SOURCE, mutter 46.2) and what `waitForFrames` waits for

- Frame callbacks are queued per surface (`wl_surface_frame`, `meta_wayland_actor_surface_queue_frame_callbacks`) and sent by `meta_wayland_actor_surface_emit_frame_callbacks`. The only caller found is `emit_frame_callbacks_for_stage_view` in `meta-wayland.c`, driven by the stage's `after-update` signal (immediately on non-native backends, via a per-view timerfd source at the frame deadline on native). It tests that the surface has an actor and that `meta_surface_actor_wayland_is_view_primary` is true; it has no opacity test. A surface that fails the test stays queued. No timer for unpainted surfaces was found. `meta_wayland_actor_surface_apply_state` calls `meta_surface_actor_schedule_update` when callbacks are pending.
- `meta_window_actor_wayland` frame hooks (`queue_frame_drawn`, `frame_complete`, `after_paint`) are empty; `meta_window_move_resize_frame` calls `meta_window_move_resize_internal` directly. Where the Wayland configure is flushed was not in the portion read.
- Not read: the body of `meta_surface_actor_wayland_is_view_primary`, Clutter's paint path for opacity 0. Inferred from the measurement above, not from source: callbacks are emitted only on a stage update, and nothing forces one for a cloaked window between its first frame and the extension's pre-reveal `waitForFrames`.
- `waitForFrames(stage, 1, ceiling)` (`commit.js:144-172`) connects `after-update`, calls `stage.queue_redraw()` and resolves after one `after-update` or the ceiling timer. So it forces a stage redraw, and that update is the one that flushes the pending callbacks (observed: `done` 0.6-1.0 ms before UNCLOAK).

#### 5. What the data points to, and the options

Which stage: (a) the configure reaching the client late, and for slow starters the client not reading its socket (Firefox); then (c) commit late under CPU starvation. Not (b). Because the delay is the app's own and the CPU's, a cloak that is painted differently would not change it.

| fix | expected effect | risk | one-log-out test |
| :--- | :--- | :--- | :--- |
| (i) cloak at opacity 1 | none expected: callbacks already arrive while cloaked and the fixture does not wait for them | visible ghost at the target-off position; effects | variant read per spawn |
| (ii) zero-size clip / off-screen move while cloaked | none expected for the same reason | unredirect and animation side effects, clip bugs | variant read per spawn |
| (iii) wait in compositor frames instead of ms | only changes the clock of the quiet timer; the stale cases have the configure at 25-120 ms, i.e. several frames late | longer cloak at idle if the count is high | variant read per spawn |

Developer switch for a single log-out (never committed): the extension reads a variant name from a file in `$XDG_RUNTIME_DIR` at each spawn and picks the cloak/settle behaviour; the verify run above is then repeated per variant without logging out.

**Recommendation: no cloak change.** Keep the existing late-resize check and the post-reveal anchor correction, which already give the right final placement. If the flash must shrink, the data in 6.12 and the stage numbers here support a bounded grace after `SIZE_SETTLED` while the size differs from the request, with a learned minimum per app so min-bound apps do not pay it again; its cost is up to the grace on a first spawn of an app that never answers. Expected residual under a 22-thread load: about 25% of runs corrected after the reveal in the `run` path (10 of 40), higher for browser-class apps.

**Not done.** No extension or product change. Chrome, nautilus and terminals were not traced at the protocol level. The idle discrepancy between the sampler harness and the `run` path is open. Whether mutter defers the configure send to the frame clock was not established (configure arrival at idle is 6-14 ms after the call, never earlier than 6 ms).

### 6.14 Mutter API leads and an "arrival wait" simulation (2026-10-12, read-only, nothing implemented)

Provenance tags: **[typelib]** = measured on this shell with gjs introspection of `Meta-14.typelib` (mutter 46.2); **[headers]** = grep of the installed `libmutter-14-dev` headers; **[web]** = read through WebFetch (a small model summarised the upstream file, so treat it as unverified); **[sim]** = offline simulation on the stored runs; **[inference]** = my reasoning.

#### 1. Window API on GNOME 46

| question | answer on this shell | source |
| :--- | :--- | :--- |
| `Meta.Window` signals | `workspace-changed`, `focus`, `raised`, `unmanaging`, `unmanaged`, `position-changed`, `monitor-changed(gint)`, `shown`, `size-changed`, `highest-scale-monitor-changed`. No `configure`. | CONFIRMED [typelib] |
| `configure` signal | Exists only on `Meta.WaylandSurface`, with no arguments (`configure()`). `Meta.WaylandSurface` exposes only `window` / `get_window`; `Meta.Window` has no `get_wayland_surface`. | CONFIRMED [typelib] |
| `add_external_constraint` / `remove_external_constraint` | not present (`typeof` is `undefined`; no `MetaExternalConstraint`; not in the 46.2 headers) | CONFIRMED [typelib][headers] |
| `get_min_size` / `get_max_size` | not present on `Meta.Window` (46.2 typelib and headers) | CONFIRMED [typelib][headers] |
| other constraint-like API | `Meta.Window.override_constraints(top, left, right, bottom)` taking `Meta.WindowConstraint` (`none`/`window`/`monitor`); declared in a private introspected header; it sets edge-constraint state, not a callback | CONFIRMED [typelib] |

Where the names appear upstream [web, unverified]:

- `meta_window_get_min_size`, `meta_window_get_max_size`, `meta_window_add_external_constraint`, `meta_window_remove_external_constraint` are absent from `window.h` at tags 47.0, 48.0 and 49.4 and present at 50.0 and `main`. The mutter NEWS lists "Add support for external window constraints" under 50.alpha and "Introduce API to get window min/max sizes" under 50.0. So the answer to "which version added get_min_size" is GNOME/mutter 50 (HYPOTHESIS, from summaries).
- Signatures at 50.0 [web]: `gboolean meta_window_get_min_size (MetaWindow *window, int *width, int *height)` (same for max), `void meta_window_add_external_constraint (MetaWindow *window, MetaExternalConstraint *constraint)` (same for remove). `MetaExternalConstraint` is a GObject interface with one virtual function `gboolean constrain (MetaExternalConstraint *constraint, MetaWindow *window, MetaExternalConstraintInfo *info)`; `info` carries `new_rect` (`MtkRectangle *`, in/out), `flags` (`MetaExternalConstraintFlags`, MOVE or RESIZE) and `resize_gravity`. The callback may modify `new_rect` and returns TRUE when the window is fully constrained.

When `configure` is emitted [web, unverified]: in `meta_wayland_surface_configure_notify()` with `g_signal_emit (surface, surface_signals[SURFACE_CONFIGURE], 0)`; the `MetaWaylandWindowConfiguration` is passed only to the shell-surface call that follows, not to the signal. So it is a notification that a configure is about to go out; it can change nothing and carries no size. I did not find who calls it.

When an external constraint runs [web, unverified]: `constrain_external` is the last entry of `all_constraints` in 50.0 (`PRIORITY_EXTERNAL_CONSTRAINT = 5`, "highest priority"), so it is evaluated in `meta_window_constrain` on every pass. It has no action check. It passes a copy of the proposed rect, the resize gravity and flags derived from the MOVE/RESIZE action bits, then writes the possibly modified rect back. For a client-driven commit (`META_MOVE_RESIZE_WAYLAND_FINISH_MOVE_RESIZE`) the constraint code runs (the 46.2 summary says position-only constraints still apply for that flag) and `constrain_external` does not skip it, but that flag maps to neither MOVE nor RESIZE in the flags passed out, so a constraint would have to read the size from `new_rect` itself.

Could a constraint pin the anchor corner in the same operation as a late client resize? Inference [inference] from the above: yes in principle. If the constraint recomputes `new_rect.x/y` from `new_rect.width/height` and the anchor each time it runs, the client's resized commit and the repositioning happen in one `meta_window_move_resize_internal`, so no separate corrector move and no intermediate frame at the wrong position. Not testable here: the API does not exist on 46 (and the extension cannot add a C vfunc; it would be a JS class implementing the interface, which I have not checked GJS can do). Whether it also runs on non-resizing moves (it would fight the user dragging) is unknown.

Does the extension's min-size path activate on 50? `_getMinSize` calls `window.get_min_size?.()` and reads `size[0]`, `size[1]` (`extension.js:300-305`, same in `legacy/extension.js:310`). On 46 the method is missing, so it returns `[0,0]`; that is why every trace says `reportedMinSize=(0x0)` and `FALLBACK_USED reason="min size unreported"`. On 50, GJS would return `[true, width, height]` for a `gboolean` plus two out arguments [inference, GJS convention not tested]; `_num(true)` is 0 (`Number.isFinite(true)` is false), so the code would read `[0, width]` and place the width in the height slot. The path would activate and be wrong. It needs `[ok, w, h]` handling before it is relied on.

#### 2. "Arrival wait" simulation

Rule simulated: instead of the 30 ms quiet rule, wait until the frame first equals the requested size (a `size-changed` sample at that size), cap C; on the cap, fall back to today's rule. Data: the 320 stored g3 loaded runs, the 320 idle runs, and the 120 fixture-trace runs from 6.13 (run path). All times in ms after SET_SIZE_REQUEST; "extra" = new settle time minus today's `SIZE_SETTLED`/`AT_TARGET` time (negative = saved). Arrival time = the first `size-changed` sample at the final size when the app answered inside the latch, otherwise the post-reveal `ANCHOR_CORRECTION` time, or for runs caught by the late check the check time (an upper bound, so costs for those runs are overstated). Runs with no recorded arrival (8 top-left below-min runs and one grow run, all loaded) count as cap hits and avoid nothing. Fallback after the cap is modelled as: settle at max(C, today's time), or for below-min runs that answered before the cap, max(C, answer + 30 ms).

Stored loaded runs (22 threads), by class. Flash = corrected after the reveal.

| class | n | flash today | C | avoided | cap hits | extra median / p95 / max |
| :--- | ---: | ---: | ---: | ---: | ---: | :--- |
| (i) legal shrink | 100 | 39 | 100 / 150 / 250 | 16 / 33 / 36 | 25 / 7 / 4 | 14.0 / 54.7 / 66.8; 14.5 / 80.8 / 116.8; 14.6 / 97.8 / 208.3 |
| (i) grow | 20 | 18 | 100 / 150 / 250 | 4 / 11 / 16 | 15 / 8 / 3 | 52.1 / 64.2 / 64.8; 74.5 / 114.2 / 114.8; 74.5 / 214.2 / 214.8 |
| (ii) at-min | 20 | 9 | 100 / 150 / 250 | 3 / 8 / 8 | 7 / 1 / 1 | 8.6 / 42.6 / 58.0; 8.6 / 65.3 / 83.7; 8.6 / 65.3 / 83.7 |
| (iii) below-min, min unknown | 180 | 68 | 100 / 150 / 250 | 32 / 50 / 63 | 180 / 180 / 180 | 40.8 / 66.2 / 79.0; 89.9 / 113.3 / 122.5; 189.0 / 213.3 / 216.4 |
| (iii) below-min, min known | 180 | 68 | 100 / 150 / 250 | 32 / 50 / 63 | 49 / 27 / 13 | 11.3 / 49.5 / 64.5; 14.9 / 93.0 / 114.5; 16.0 / 181.2 / 214.5 |

Below-min with the minimum unknown never reaches the requested size, so every run waits for the cap: the median extra is about C minus today's time. With the minimum known (target = max(request, min)) the arrival is the answer itself and the wait ends there. Known-min minimum extra is -184.8 ms (a run whose quiet timer was starved today). At idle (320 runs): legal shrink, grow and at-min cost about 0 (median -0.1); below-min with min unknown costs a median 58.8 / 108.8 / 208.8 ms for C = 100 / 150 / 250; with min known it saves a median 30.8 ms (the 30 ms quiet rule is no longer needed), at idle avoided flashes 0 / 0 / 2 of 2.

Fixture-trace runs (run path, two legal-shrink specs, 40 each): back-to-back and idle with 30 s gaps have no flashes and the arrival wait changes nothing (extra 0). At 22 threads: 10 flashes today; avoided 9 / 9 / 10 for C = 100 / 150 / 250; cap hits 1 / 1 / 0; extra median 0, p95 29.8 / 35.5 / 35.5, max 40.3 / 66.4 / 84.5.

Comparison with the fixed grace of 6.12, recomputed with the same code on the same 320 loaded runs (mean extra is over all runs, flashes of 134):

| scheme | flashes avoided | mean extra ms | p95 extra ms | avoided per mean ms |
| :--- | ---: | ---: | ---: | ---: |
| fixed grace G = 50 / 100 / 150 / 200 / 300 | 66 / 109 / 118 / 124 / 130 | 28.5 / 38.3 / 43.4 / 47.0 / 51.9 | 63 / 100 / 150 / 200 / 230 | 2.3 / 2.9 / 2.7 / 2.6 / 2.5 |
| arrival wait, min unknown, C = 100 / 150 / 250 | 55 / 102 / 123 | 30.9 / 61.2 / 120.0 | 64 / 111 / 211 | 1.8 / 1.7 / 1.0 |
| arrival wait, min known, C = 100 / 150 / 250 | 55 / 102 / 123 | 10.7 / 19.1 / 27.7 | 58 / 96 / 178 | 5.2 / 5.3 / 4.4 |
| arrival wait or answer-then-quiet (no cap on answered below-min runs), C = 100 / 150 / 250 | 55 / 102 / 123 | 23.0 / 33.5 / 43.4 | 63 / 103 / 181 | 2.4 / 3.1 / 2.8 |

At idle the fixed grace costs a mean 0.7 ms (G ≥ 100, 3 of 3 flashes avoided); arrival wait with min unknown costs 32-117 ms on the 180 below-min specs; with min known it saves 17 ms on average.

Which fixes more flashes per ms of cloak? Arrival wait with a known minimum, clearly (4.4-5.3 per mean ms against 2.5-2.9), and it shortens the cloak at idle. With the minimum unknown it is worse than the fixed grace (1.0-1.8) because every below-min run waits for the cap. The hybrid (arrival for compliant sizes, today's answer-then-quiet when the app answers with a different size) is about equal to the fixed grace (2.4-3.1) and costs nothing at idle.

What the simulation cannot show:

- **Real apps.** Only the fixture (every request is answered, either at the target or at the minimum) and the stored g3 runs. An app that never changes its frame (gnome-text-editor at 30x20, a fixed-size dialog) is not in the data; for it the wait always hits the cap.
- **Arrival times are approximations** (post-reveal correction time, a late-check upper bound for 62 loaded runs); runs without a correction (8 top-left below-min, 1 grow) are cap hits by assumption.
- **The minimum is taken from the final size.** On 46 there is no API for it; learning it from a previous spawn is not modelled, and a first spawn pays the unknown-min cost.
- **Feedback.** It assumes the app's timing is unchanged by a longer cloak. 6.13 shows callbacks to a cloaked surface wait for the next stage update, which could slow a toolkit app's second frame.
- **No second resize.** Apps that resize twice, snap to a grid, or are clamped by the work area are listed below, not simulated.
- **Fixture-trace runs** are two specs of the same size class.

#### 3. Edge cases that break "wait until frame == target", and the fallback

| case | what happens | fallback |
| :--- | :--- | :--- |
| App ignores the request (fixed-size dialog, `resizeable` false) | the frame never changes; every spawn waits the full cap | skip the wait when `window.allows_resize()` / `resizeable` is false [typelib: both exist]; otherwise the cap, then today's quiet rule |
| Terminal or other size-increment app | frame snaps to a grid and never equals the request (observed in 6.12: gnome-terminal asked 500x300, ended 494x292) | on the first `size-changed` to a different size switch to today's quiet rule (the "answer-then-quiet" hybrid); never wait for equality after an answer |
| App resizes twice (overshoot then settle, or reaches the target and then re-lays out) | equality can fire on the transient size, or fire and a later resize arrives | finish at the first equality as today's `AT_TARGET` does; the existing late-resize check and reactive anchor handle the second resize |
| Growth beyond the work area, or a request the compositor clamps | mutter constrains the frame, so it never equals the request | set the target to the request clamped to the work area the extension already computes; otherwise cap then quiet rule |
| Maximised, fullscreen or tiled window | constraints force the size; the extension already unmaximises first | skip the wait for fullscreen and tiled windows; for maximised, unmaximise then wait as normal; on any mismatch use the quiet rule |
| X11 client | mutter applies the resize and size hints itself and updates the frame rect immediately, so equality is immediate or the hints clamp it | if `get_client_type()` is X11 and the frame after the call differs from the request, do not wait for equality (use the quiet rule) |
| Unknown minimum (below-min request) | never equal; waits for the cap on every spawn | treat the first different-size `size-changed` as the answer (quiet rule from there); keep a per-app learned minimum in memory only |
| Request equals the current size | already equal | finish immediately |
| Cap reached | | today's behaviour, so the worst case is the cap added on top of the old path |

#### 4. Recommendation

Do not build the arrival wait with the cap as the only stop. The data favour: (1) wait for equality with the requested size only for requests that can be met (resizeable, not fullscreen or tiled, within the work area); (2) the first `size-changed` to any other size counts as the answer and hands over to today's quiet rule, which keeps the below-min cost at the old level; (3) a cap of 150 ms (102 of 134 loaded flashes avoided at a mean 33.5 ms extra with the hybrid, 0 cost at idle); (4) a learned per-app minimum, in memory, to turn the below-min case into the known-min case (19 ms mean extra, 5.3 flashes per ms, and a saving at idle). External constraints and `get_min_size` need GNOME 50 and should be revisited there; the existing `_getMinSize` must be fixed for the `[ok, w, h]` return before then.

**Not done.** No code. The constraint path, GJS's handling of the interface and of out-arguments, and the upstream version claims were not verified beyond the summaries above.

### 6.15 Flash-aware verdict: audit, ground truth from the client, validation on X11 (2026-10-13, harness only)

Labels: CONFIRMED = measured or read in this repository; SOURCE = read from code (the GJS lines come from a summarising fetch, so they are indirect); HYPOTHESIS = not tested. Commits `199a6d5` (fixture events), `2abd62e` (detector), `c00ddd6` (runner), `1dc2752` (UI, report, `--strict-flash`), `27a7303` (fix, below), `1da549d` (frame note). No extension change; no setting, `/sys`, `/proc` write or nested shell was used.

#### 1. Audit: what counts as a flash today, and what cannot be seen (SOURCE)

| Signal | Where | Effect |
| :--- | :--- | :--- |
| `placement corrected after reveal (+N ms)`: the first `ANCHOR_CORRECTION` after `UNCLOAK` | `trace/analyser.rs` §6 | warning only |
| stale printed rectangle: stdout rectangle differs from the one re-read after the 1300 ms settle | `runner/models.rs` `run_warnings`, `oracle::stale_print_warning` | warning only; it shows spawn-at exited before the window was in place, not what was drawn |
| `OffTargetVisibility`: opacity > 0 between `CLOAK` and `UNCLOAK` | `trace/analyser.rs` §4 | fails the run, but only when the spec has `never_visible_off_target = true` (`needs_trace`); `placement/bottom-right-larger` has it `false` |
| `MultipleReveal`, `LingeringCloak`, watchdog, cloak over 100 ms | analyser | other defects, not flashes |

Not seen by any of them:
- **A size change after the reveal with no position correction.** `ANCHOR_CORRECTION` is logged only when the recomputed position differs from the current one (`extension.js:1074-1090`). A top-left anchor does not move when the size changes, so the log is silent.
- **A window visible before the cloak.** Samples are checked only between `CLOAK` and `UNCLOAK`; nothing compares the client's first paint with `CLOAK`. Not implemented here either (a client paint is not proof of being on screen).
- **X11, frame rect lies.** `move_resize_frame` changes the manager's rectangle at once, so `AFTER_SET_SIZE` reads 800x600 0.06-0.2 ms after the request and `COMMIT_RESOLVED` says `AT_TARGET` in 0.15 ms whatever the client has painted. Every trace-based check passes by construction. CONFIRMED in 60 stored traces (section 5).
- **A flash that is a paint cost.** Nothing in the extension's log says when the client drew.

#### 2. Ground truth from the client and the definition

The fixture writes `fixture-<pid>.events` when `SPAWN_AT_FIXTURE_EVENT_DIR` is set (the harness sets it per run in a private 0700 directory, removed afterwards): `FIRST_PAINT WxH` (first paint at each new size, stamped after `present()` returns), `RESIZED WxH`, `MOVED X,Y`, all in `CLOCK_MONOTONIC` ns. Unlike `SPAWN_AT_FIXTURE_TRACE_DIR` it leaves stderr and `WAYLAND_DEBUG` alone.

**Flash** = the reveal (`UNCLOAK`) comes before the fixture's first paint at its final size, or the painted size changes by more than 2 px per axis, or the position differs by more than 2 px from the one logged at `UNCLOAK`, within 1500 ms after the reveal. Implemented as: size on `FIRST_PAINT` events against the content shown at the reveal (kind `size-change-after-reveal`; `paint-after-reveal` when nothing was painted yet); position by the last client position against the frame logged at `UNCLOAK`; an `ANCHOR_CORRECTION` after `UNCLOAK` always counts (source `extension`).

**Correction found by the first live run (commit `27a7303`).** The first version also used `Resized`/`Moved` as the time of a change and reported "position 50,90 -> 1104,464 11.4 ms after the reveal" on X11. That was the fixture being *told*, late, of a move the manager had made before the reveal (the extension had logged 1104,464 at `UNCLOAK`). Those events are delivery times and lag the real change, so they are now logged but not used as times; test `test_late_moved_report_of_a_move_before_the_reveal_is_not_a_flash`.

**Clock alignment.** The extension prints `+N ms` from `_t0`, the first `logTime` after its logger started (`logger.js:297-304`); `N` comes from `GLib.get_monotonic_time()`, the same `CLOCK_MONOTONIC` as the fixture's `clock_gettime`. The origin is recovered per run: the harness sends 4 `SetLogging(true)` calls, each between two monotonic reads; each leaves a `SET_LOGGING enabled=true` echo at `X` ms, so the origin is in `[before - X, after - X]` (+-5 us for the two-decimal print) and the 4 intervals are intersected (`ClockAnchor`). Intervals that do not overlap give no anchor (the run falls back to the extension-only signal and says so in `flash_notes`). Measured half-width: **1.5-3.0 ms** over 90 runs. An order that falls inside the half-width is listed with `certain: false` and not counted.

What can misalign them: a `gdbus` start-up that is slow on one side only (the intersection is as tight as the best leg of the best call, not the mean); a reloaded extension (new origin; the echoes belong to the new one, so a stale anchor is not reused); the journal's flush delay (the run polls up to 1 s for the `UNCLOAK` line); the echoes of another `SetLogging` caller in the same second; `UNCLOAK` is logged when opacity is set, but the compositor shows it at its next frame (up to 16.7 ms at 60 Hz, HYPOTHESIS about this machine's refresh rate, not read). A flash event that came less than 16.7 ms after the reveal is therefore counted but carries the note "may not have reached the screen" (commit `1da549d`). The fixture stamps a paint when `present()` returns; on X11 the server draws a moment later.

#### 3. Verdict, summary, report

Placement PASS/FAIL is unchanged. A pass with a certain flash is **PASS (flash)**: pink `✱` in the list, `✱ Pass (flash)` and FLASH rows in the drawer (basis, each event, `?` for uncertain ones), `· n flash` on the category heading, a `Flash:` line in the `run` summary, `6. Flash` in `report.md`, and on each result in `report.json` `flash` (bool), `flash_events` (kind, source, `at_ms_after_reveal`, `certain`, `detail`), `flash_basis` (`client` / `extension-only` / `none`), `flash_notes`, `flash_clock_uncertainty_ms`. Reports from before read back as `flash: false`. The exit code is unchanged. `run --strict-flash` (default off) turns a flashing pass into a failure (exit 1, message `Flash (--strict-flash): ...`). Cost: a fixture spawn waits until 1500 ms after its reveal (not counted in `duration_ms`), about 1.5 s per run.

#### 4. Tests (`cargo test -p spawn-at-verify --features tui,fixture`: 341 passed, 4 ignored; baseline 305)

`flash_test` 16: X11 lying frame rect (flash on the paint 25 ms after the reveal; the same trace without client events is "clean" and the old warnings are empty), paint before the reveal (no flash), top-left in-place shrink (no `ANCHOR_CORRECTION`, old warnings empty, size flash 31 ms after), first paint with nothing painted, order inside the uncertainty (listed, not counted), 2 px / 3 px tolerance and the 1500 ms watch, extension correction listed once, late `Moved` report, final client position, under-one-frame note, no `UNCLOAK`, event-file and echo parsing, anchor intersection and refusal. Runner wiring 9 (lib), report JSON/markdown 2, run summary 2, TUI 5, `run --help` 1, fixture 1.

#### 5. Validation on this session (X11, `XDG_SESSION_TYPE=x11`, GNOME Shell 46.0, gjs 1.80.2, 22 CPUs)

Isolated `run` invocations 1.5 s apart (regime (a) of 6.10), 10 per spec per batch, debug `spawn-at` and fixture unless noted; the top-left shrink spec and the control are throwaway specs in `scratch/` (no committed spec was added). Load average (1 min) at batch start: val1 about 2-3; val2 **14.4 at the first run** (right after a 17 CPU-minute release build, 3.8-4.1 at the last runs); val3 3.8 -> 1.9.

| batch | spec | runs / pass | **flash (new)** | runs with an old warning | lead of the paint after `UNCLOAK`, ms (n, min / median / max) |
| :--- | :--- | :---: | :---: | :---: | :--- |
| val1 | `placement/bottom-right-larger` | 10 / 10 | **9** | 0 | 9, 3.1 / 5.7 / 14.3 |
| val1 | top-left shrink 400x300 -> 200x150 | 10 / 10 | 0 (1 uncertain) | 0 | - |
| val1 | control: fixture `--paint-delay-ms 400` | 10 / 10 | 10 | 0 | 10, 383.5 / 393.8 / 404.1 |
| val2 | `bottom-right-larger` | 10 / 10 | **10** | 0 | 10, 7.4 / 11.6 / 26.9 |
| val2 | top-left shrink | 10 / 10 | 1 | 0 | 1, 3.3 |
| val2 | control | 10 / 10 | 10 | 0 | 10, 384.9 / 395.0 / 401.7 |
| val3, **release fixture** | `bottom-right-larger` | 10 / 10 | **0** | 0 | - |
| val3, release fixture | top-left shrink | 10 / 10 | 0 | 0 | - |
| val3, release fixture | control | 10 / 10 | 10 | 0 | 10, 368.7 / 377.1 / 382.6 |

- **Old warnings caught 0 of 90 runs**, including the 30 control runs whose first paint came about 0.4 s after the reveal and the 20 `bottom-right-larger` runs of val1/val2 where the new check found 19 flashes. `final_rect` was never recorded (the printed rectangle was always right).
- The control proves the check fires on a known late paint; val3 proves it stays silent when the client is fast. `bottom-right-larger` with the debug-build fixture paints the 800x600 buffer 3-27 ms after the reveal; with the release-build fixture before it. So the late paint is largely the debug fixture's software rendering, which is what a slow real app looks like to the extension: it cannot see it (section 1). It reproduces the shape of the screen recording (small content visible at the final top-left until the 800x600 paint), not its duration; **17 of the 19 measured leads of this spec (3.1 to 26.9 ms) are shorter than one 60 Hz frame (16.7 ms), so how many were actually visible is not shown**. A screen capture would settle it; none was taken.
- Top-left shrink: 1 of 20 debug runs flashed (3.3 ms), so this spec is not a reliable flasher here; the shrink repainted before the reveal. The "size change with no position correction" case is covered by the synthetic trace, not by a live reproduction.
- Not run: Wayland (there the fixture gets no `Moved` events, so only the extension's `ANCHOR_CORRECTION` covers position; whether the frame rect lies there was not tested), real toolkit apps, the full suite.

#### 6. The gap between `COMMIT_RESOLVED` and `BATCH_STEP SetPositionAnchored` (read-only)

CONFIRMED from `commit.js:42-70` and `extension.js:696-720`: with the frame already at the target, `waitForCommit` runs `check('initial')` inside the Promise executor, so `finish('AT_TARGET')` logs `COMMIT_RESOLVED` and calls `resolve()` synchronously, 0.15 ms after `BATCH_STEP WaitForCommit` (in a stored trace). Nothing else runs in `finish`. The next statement of the extension is the continuation of `await this._stepWaitForCommit(...)` in `_runBatch`, then the loop logs `BATCH_STEP` for the next op. There is **no timer, wait or log line in between**: 0 lines for the window between the two in the samples I looked at (3 runs), and a 60-run set shows 8.9 / 11.7 / **13.6** / 15.2 / 17.1 / 24.2 ms (min / p25 / median / p75 / p90 / max), all `AT_TARGET`. Your 25.8 ms is just above that maximum (a screen recording loads the compositor, HYPOTHESIS).

So the time is spent between `resolve()` and the continuation running, i.e. before JavaScript is re-entered. SOURCE (GJS 1.80.2 `gjs/promise.cpp`, summarised): the promise-job source has priority `10 * G_PRIORITY_HIGH` (-1000) and runs `js::RunJobs` from the main loop; jobs are not run when a callback returns to C. That rules out the continuation being starved by compositor sources (they have a lower priority), and leaves one reading: the callback that runs the batch (a `window-created` / `map` signal handler, which ran `Cloak`, `SetSize` and `WaitForCommit` synchronously) returned at its first `await`, and the native code that emitted the signal kept running (other handlers, mutter/Shell map handling of the freshly resized 800x600 X11 window) for 9-24 ms before control reached the main loop. That is HYPOTHESIS: which native work it is was not profiled, and it is not shown that this work is the same on Wayland.

Is it idle cloak time? The window is cloaked (opacity 0) throughout, so the gap is part of `UNCLOAK - CLOAK` and of the 100 ms budget, but it is **not the extension idling**: no wait of the extension is running, and the main thread is probably busy (HYPOTHESIS, consistent with the gap not clustering at multiples of 16.7 ms). Note that it is also the time in which the client could paint: on X11 the manager's frame is already 800x600, so shortening this gap would not by itself make the client paint sooner.

**Not done.** A screen capture to see which leads are visible; a profile of the native work in the gap; Wayland and real-app runs; the "window visible before the cloak" check; a committed top-left shrink spec; whether the 1.5 s wait per fixture run should be opt-out for large suites.

### 6.16 Flash: numeric headline, per-screen visibility label (2026-10-13, harness only)

Labels: CONFIRMED = measured or read here; HYPOTHESIS = not tested. Commit `1de817f`. No extension change; the only things read from the system were `xrandr --query` and one read-only `GetCurrentState` call to check its output format.

**Why.** Observed by eye, about 70% of "PASS (flash)" runs show the flash. Section 6.15 counted every late paint, including sub-frame ones (3-27 ms), as a flash. Lateness is the defect and is the same on every screen, so the headline stays numeric; whether a given late paint can be seen depends on the refresh rate, so that is a label.

**Definition (CONFIRMED, code and tests).**
- **Numeric flash** (`flash`, the list icon, the headline counts, `--strict-flash`): the reveal comes before the final paint (or an `ANCHOR_CORRECTION`, or a different final position) by a lead strictly above `max(measured clock uncertainty, --flash-threshold-ms)`. The uncertainty is the half-width of the clock anchor (1.5-3.0 ms measured). A lead at or below it, or at or below the threshold, is listed (`numeric: false`) and not counted. `--flash-threshold-ms` defaults to 0 and only raises the floor.
- **Visibility label** (per numeric event, per screen): `frame_ms = 1000 / refresh_hz`; `certain` when `lead_ms >= frame_ms`, `possible` when shorter; `visible_probability = min(1, lead_ms / frame_ms)`. Run level: `flash_certain` and `flash_possible` count numeric events by label, `visible_probability = 1 - prod(1 - p)` over the numeric events. The probability treats the reveal as landing at a random point of the frame period: HYPOTHESIS, not measured (the extension reveals right after a stage update, so the phase may not be uniform).
- **Refresh rate**, read-only (`src/display.rs`): X11 `xrandr --query` (the `*` rate of the primary output), Wayland Mutter `DisplayConfig.GetCurrentState` over `gdbus` (the `is-current` mode; the parser is tested on this machine's real output), otherwise 60 Hz with `refresh rate not read: <why>` in `flash_notes`. This machine: `xrandr` 60.00 on eDP-1, recorded in `environment.refresh_hz`, `frame_ms`, `refresh_source` of every report.
- **Report JSON** (per result): `flash`, `flash_certain`, `flash_possible`, `refresh_hz`, `frame_ms`, `visible_probability`, `flash_events[]` with `kind`, `source`, `lead_ms` (was `at_ms_after_reveal`), `numeric` (was `certain`), `visibility`, `visible_probability`, `detail`. The 60 Hz note on events added in `1da549d` is gone (replaced by the label).
- **UI.** List icon `✱` and the `· n flash` heading count follow the numeric flash. The drawer's Flash row reads `flash (C certain, P possible)`; each event ends with `[visible: certain, lead >= 16.7 ms frame]` or `[visible: possible, 30% at 60 Hz]`. `run` prints `[C certain, P possible on this screen]` after the flashed ids; `report.md` has a "Visible on this screen" column and the split counts with the display.

**Tests (352 passed / 4 ignored with `tui,fixture`; `flash_test` 20, +4):** lead above, at and below the frame period (50 Hz = exactly 20 ms: 30 ms certain, 20 ms certain, 5 ms possible 25%); lead inside the clock uncertainty (0.5 ms listed, 1.5 ms counted); threshold floor (5 ms lead counts at 4.9, not at 5.0 or 8.0); the same 10 ms lead at 60 Hz (possible, 60%) and 144 Hz (certain, headline identical); two events combined (0.72); `--strict-flash` fails a sub-frame numeric flash and passes a listed-only one; xrandr and Mutter parsers; JSON keys; drawer label; `run --help`.

**Validation, this session** (X11, `XDG_SESSION_TYPE=x11`, GNOME Shell 46.0, 22 CPUs, 60.00 Hz; load average (1 min) 2.1-2.6 over the 20 runs): 20 isolated `run --glob placement/bottom-right-larger` invocations, 1.5 s apart, debug `spawn-at` and fixture, all exit 0, all 20 PASS, clock uncertainty 2.0-2.5 ms.

| | runs |
| :--- | ---: |
| numeric flashes (headline) | **19 of 20** (run 18: lead -0.2 ms, inside the uncertainty, not counted) |
| certain (lead >= 16.7 ms) | 0 |
| possible (lead < 16.7 ms) | 19 |
| leads, ms (min / median / max) | 3.1 / 10.8 / 15.1 |
| old warnings (stale print, post-reveal correction) | 0 of 20 |
| mean `visible_probability`, flashing runs / all 20 runs (a non-flash counts 0) | **0.575 / 0.546** |

**Against the observation (about 70%).** The model expects 55% to 57% of these runs to show the flash, against roughly 70% seen. They are in the same range but not the same number, and the comparison does not support more than that: the 70% is **an impression by eye, not a measurement** (no count of runs, no recording was analysed, and the user's runs are not these 20); with 19 numeric runs the model's own spread is large (the sum of 19 Bernoulli draws at these probabilities has a standard deviation of about 2 runs, i.e. 10 points of the 20); and the probability model is a HYPOTHESIS. Two readings are open and were not separated: the real exposure is longer than the lead (the compositor needs more than one frame, or the window is shown at the old content for longer), or the user counts a different or milder artefact as a flash. Only a screen capture of the same runs would show which; none was taken.

**Not done.** Wayland and a refresh rate other than 60 Hz on a live screen (the 144 Hz case is a synthetic test and the Mutter parser runs on a sample, not a 144 Hz display); real apps; any screen capture.


### 6.17 BASELINE before the ready-wait change: flash on native Wayland vs XWayland vs X11 (2026-10-13, harness and docs only)

Labels: CONFIRMED = measured in this section; HYPOTHESIS = not tested. **This is the baseline taken before the ready-wait change: no product or extension code was touched, and nothing in it is a result of that change.** Session: `XDG_SESSION_TYPE=wayland`, GNOME Shell 46, 22 CPUs, 60.00 Hz (`mutter-displayconfig`), binary commit `0756167` (the catalog commit adding `placement/top-left-smaller`, 36 tests). Every run is one isolated `run` invocation, specs interleaved, 1.5 s apart, 10 runs per spec per batch, all exit 0 and `auto-pass`. `bottom-right-smaller` is a `Both` spec, so its final status stays `pending` (human verdict unset) by design.

| batch | fixture | `spawn-at` | load (1 min) at run start |
| :--- | :--- | :--- | :--- |
| idle, debug | debug | debug | 1.38 - 2.91 |
| idle, release | release (`cargo build --release --bin fixture`) | **debug copy** (a release `spawn-at` build was not made; the harness requires the binary's commit to equal `HEAD`) | 1.05 - 2.37 |
| loaded | debug | debug | 20.53 - 24.49 (mean 22.7), after 22 `burn-cpu` threads and a 70 s warm-up |
| XWayland | debug (`--protocol x11`) | debug | 1.67 - 1.96 (waited for the load to fall after the burn) |

**Idle, native Wayland (CONFIRMED).** Numeric flash count was **0 in all 80 runs**: for each of `bottom-right-larger`, `bottom-right-smaller`, `top-left-smaller` and the delayed-paint control, 10 debug and 10 release runs. So certain 0, possible 0, `lead_ms` none, expected visible probability 0, **old-warning count 0** (no row in `report.md` "5. Warnings"; no `ANCHOR_CORRECTION` or `LATE_RESIZE_CORRECTED` in any of the 60 runs of the three real specs). Clock uncertainty 1.5 - 2.2 ms. In the debug `bottom-right-larger` runs `AFTER_SET_SIZE` still shows the old frame `(50,90,400x300)`, the first `COMMIT_SAMPLE` is `(400x300)`, and `COMMIT_RESOLVED reason=AT_TARGET finalFrame=(800x600)` comes 12.9 - 20.6 ms after `SET_SIZE_REQUEST`, i.e. the extension waited for the client and revealed at the new size.

**The delayed-paint control does not work on Wayland (CONFIRMED that it produced 0 flashes in 20 of 20 runs; the cause is a HYPOTHESIS).** On X11 it flashed 10 of 10 (6.15). Here `CLOAK` came 430.6 - 444.3 ms (debug) and 424.8 - 439.9 ms (release) after `WINDOW_CREATED` in all 20 runs, so the 400 ms `--paint-delay-ms` moved the point at which the extension saw the window, not a late repaint of a shown window. HYPOTHESIS: on Wayland the surface is only mapped once the first buffer is committed. Consequence: on Wayland the control does not prove that the check can fire; the loaded batch below is what shows it can.

**Loaded, 22 threads, debug (CONFIRMED).**

| spec | runs with a numeric flash | numeric events | certain / possible | `lead_ms` median / max | mean expected visible probability per run | runs with an old warning |
| :--- | :---: | :---: | :---: | :--- | :---: | :---: |
| `bottom-right-smaller` | **6 / 10** | 11 | 8 / 3 | 29.7 / 78.9 | 0.581 | 6 |
| `bottom-right-larger` | **9 / 10** | 16 | 14 / 2 | 32.1 / 60.0 | 0.769 | 9 |

The runs with an old warning are the runs with an `ANCHOR_CORRECTION` after the reveal (6 and 9; one run of each also logged `LATE_RESIZE_CORRECTED`), and they are the same runs that have a numeric flash. Clock uncertainty under load was 6.3 - 17.2 ms (idle 1.5 - 2.2), so the counting floor was higher than idle. Together 15 of 20 runs (75%) were corrected after the reveal.

**Against TEST_PLAN 6.12 and 6.13 (CONFIRMED for the numbers, cause not established).** 6.12 measured 91 of 120 runs (76%) corrected after reveal at 22 threads, which agrees with the 75% here. 6.13 measured 10 of 40 (25%) for `bottom-right-smaller` and `bottom-smaller` at 22 threads, which does not: `bottom-right-smaller` alone was 6 of 10 here. Different spec mix (6.13 pooled `bottom-smaller`), different batch (10 runs, not 40) and a small sample all fit; which one explains the gap was not tested. The burn threads were stopped afterwards: `pgrep -cx xtask` printed 0 and `pgrep -cx fixture` printed 0.

**XWayland: `bottom-right-larger` with `--protocol x11` inside this Wayland session, 10 debug runs (CONFIRMED).**
- Recorded as XWayland: `proto=XWayland` in the extension log of 10 of 10 runs (every native Wayland run, 100 of 100 across the batches above, says `proto=Wayland`).
- The frame rect jumps to the target at once, like native X11: `AFTER_SET_SIZE frame=(50,90,800x600)`, first `COMMIT_SAMPLE source=initial` already `(800x600)`, and `COMMIT_RESOLVED reason=AT_TARGET` 0.1 - 0.3 ms after `SET_SIZE_REQUEST` in 10 of 10 runs (native Wayland: 12.9 - 20.6 ms, frame still 400x300).
- Flash: **1 of 10 runs** had a numeric flash (`size-change-after-reveal`, lead 6.4 ms, possible, visible probability 0.38); the same batch listed a second event of 0.98 ms that is inside the uncertainty and not counted. Old warnings 0, `ANCHOR_CORRECTION` 0. HYPOTHESIS for why X11-style lying frame rect gives fewer flashes than the 9 and 10 of 10 seen on X11 (6.15 val1/val2): not examined. Only idle, debug and one spec were run for XWayland.

**Native Wayland vs XWayland vs X11 (baseline).** The X11 column is **not re-measured**: it is copied from 6.15 section 5 (earlier X11 session, same machine, same harness, 10 runs per cell, debug unless noted), because this session is Wayland.

| | native Wayland (this section) | XWayland (this section) | native X11 (6.15, earlier session) |
| :--- | :--- | :--- | :--- |
| window recorded as | `proto=Wayland` | `proto=XWayland` | X11 |
| frame rect right after `SetSize` | old size (400x300) | target at once (800x600) | target at once (lying frame rect) |
| `COMMIT_RESOLVED` after the request | 12.9 - 20.6 ms | 0.1 - 0.3 ms | not recorded in this form |
| `bottom-right-larger`, idle, debug: flash runs | **0 / 10** | **1 / 10** | 9 / 10 (val1), 10 / 10 (val2) |
| `bottom-right-larger`, idle, release fixture | **0 / 10** | not run | 0 / 10 (val3) |
| `top-left-smaller`, idle, debug / release | 0 / 10, 0 / 10 | not run | 0 (1 uncertain) and 1 / 10 (val1, val2), 0 / 10 release |
| `bottom-right-smaller`, idle, debug / release | 0 / 10, 0 / 10 | not run | not run |
| delayed-paint control, idle | **0 / 20** (does not fire) | not run | 10 / 10 (val1, val2, val3) |
| `bottom-right-larger`, 22 threads, debug | **9 / 10** | not run | not run |
| `bottom-right-smaller`, 22 threads, debug | **6 / 10** | not run | not run |
| old warnings, idle | 0 of 60 | 0 of 10 | 0 of 90 |

**Not done.** XWayland under load, with the release fixture, or for any spec but `bottom-right-larger`; `top-left-smaller` under load; native X11 in this session (needs a log-out); a release-built `spawn-at`; any screen capture; real toolkit apps. The scratch specs (delayed-paint control, `xwayland-bottom-right-larger`) and batch scripts live in `scratch/` and are not committed; raw per-run JSON stayed in the session scratchpad and is not stored in the repository.


### 6.18 Ready-wait: live validation against the 6.17 baseline (2026-10-13, no code change)

Labels: CONFIRMED = measured or read in this section; HYPOTHESIS = not tested. Nothing in the product, the extension or the harness was changed; the only files written are this section and throwaway scripts in `scratch/` (not committed). Tests were not run in this step. Binary under test: debug `spawn-at` and debug fixture at `6ecc537` (the ready-wait wiring); the extension in `~/.local/share/gnome-shell/extensions/spawn-at@harsh.local/` contains `ready_wait.js` (installed 11:18:31 IST, user logged back in afterwards). Session: `XDG_SESSION_TYPE=wayland`, GNOME Shell 46.0, 22 CPUs, 60 Hz. Same 4 scratch specs, same `scratch/baseline_batch.sh` (one isolated `run` per spec, specs interleaved, 1.5 s apart) as 6.17.

**Preflight (all CONFIRMED before any measurement).**

| check | result |
| :--- | :--- |
| session type | `wayland` (env and `loginctl`) |
| extension state | `gnome-extensions info spawn-at@harsh.local`: `Enabled: Yes`, `State: ACTIVE` (the uuid is `spawn-at@harsh.local`; the name `spawn-at@harshp2008.github.com` does not exist) |
| protocol | `gdbus ... Properties.Get ... ProtocolVersion` returned `uint32 6` |
| native spawn | `READY_WAIT_START ... strategy=ready-wait kind=NativeWayland target=(800x600) current=(400x300) cap=150 quiet=30 hard=1500`, then `READY_WAIT_RESOLVED ... outcome=reached ... waited_ms=41.18 reason=at-target` |
| `--protocol x11` spawn | `READY_WAIT_START ... proto=XWayland strategy=legacy kind=XWayland why=not-native-wayland timeoutMs=300`, then `READY_WAIT_RESOLVED strategy=legacy outcome=none reason=AT_TARGET` |

**Two things the reader must know before the numbers.**

1. **No same-session before/after exists (CONFIRMED).** The baseline (6.17) was taken in the login before this one. The extension has no switch back to the old rule (`grep` of `assets/gnome/modern/extension.js` finds only the `strategy=legacy` path for X11/XWayland), and the fixture binary is the same file in both (built 10:38, the baseline started 10:42). So every difference below is "this login with the change" against "the earlier login without it". The XWayland result below shows that the timing of the reveal can differ between logins on the unchanged legacy path.
2. **No cloak durations for these batches were written down before (CONFIRMED).** TEST_PLAN 6.13, 6.15 and 6.17 contain no `UNCLOAK - CLOAK` figure for them (6.17 has `COMMIT_RESOLVED` 12.9 - 20.6 ms and, for the delayed-paint control, `CLOAK` 430 - 444 ms after `WINDOW_CREATED`). The "baseline cloak" below is therefore **recomputed here from the baseline runs' own lines in `~/.local/state/spawn-at/session.log`** (the log was not rotated; windows picked by the batch start/end times in the baseline `meta.txt`: idle debug 10:42:05 - 10:44:28 IST, 40 windows; loaded 10:49:38 - 10:51:12, 20 windows; XWayland 10:54:06 - 10:54:41, 10 windows). Definition: first `UNCLOAK` minus first `CLOAK` of a window, extension clock; median, p95 (nearest rank) and max. The log is local and not stored in the repository.

**Idle, native Wayland, debug fixture: 4 specs x 10 runs (11:22:45 - 11:25:09 IST, load at run start 1.04 - 1.53).** All 40 exit 0; `bottom-right-larger`, `top-left-smaller`, `flash-control-delayed-paint` 10 of 10 `auto-pass`, `bottom-right-smaller` 10 of 10 `pending` (a `Both` spec, as in 6.17).

| | baseline (6.17 + recomputed cloak) | this run |
| :--- | :--- | :--- |
| numeric flashes | 0 of 40 (0 of 80 with release) | **0 of 40** (0 of 10 per spec) |
| cloak ms, all 40: median / p95 / max | 19.09 / 21.22 / 48.94 | **41.10 / 50.72 / 52.12** |
| cloak median per spec: larger / smaller / top-left / control | 19.75 / 18.74 / 18.73 / 20.20 | 41.53 / 38.42 / 39.50 / 48.59 |
| `bottom-right-larger`: `SET_SIZE_REQUEST` to `COMMIT_RESOLVED` | 12.9 - 20.6 ms, median 15.8 | 29.7 - 40.6 ms, median 35.3 |
| `UNCLOAK_4C` to `UNCLOAK` gap, median (n=40) | 6.7 ms | 11.2 ms |
| `READY_WAIT_RESOLVED` outcomes | none logged | **40 `reached` (reason `at-target`)**; 0 `adjusted`, 0 `timed-out`, 0 `skipped` |
| `waited_ms`, all 40: min / median / p95 / max | - | 22.4 / 29.6 / 39.3 / 40.5 (per spec median: larger 35.2, smaller 26.6, top-left 25.6, control 35.2) |
| warnings (post-reveal correction, stale print, cloak over 100 ms) | 0 | 0 |

Reading (CONFIRMED for the numbers, cause HYPOTHESIS): in an idle run the machine waits for the app's `size-changed` and ends on it (`COMMIT_SAMPLE source=size-changed` and `READY_WAIT_RESOLVED` under 0.2 ms apart in the run inspected). The app's answer came about 20 ms later in this login (35 ms after the request, against 15 ms), and the `4C -> UNCLOAK` gap is about 4.5 ms longer; together that is the +22 ms of cloak. Whether the later answer is a property of this login, of the load of other programs on the machine, or of the wiring was not separated (HYPOTHESIS that the machine only observes and does not slow the client, since `SetSize` is issued before the wait in both builds; no run without the wiring exists in this login to show it).

**Loaded, 22 `burn-cpu` threads, debug fixture: 10 runs each of `bottom-right-smaller`, `bottom-right-larger`, `top-left-smaller` (11:36:13 - 11:38:37 IST).** `target/debug/xtask burn-cpu --threads 22`, 70 s warm-up, load at run start 21.5 - 26.5 (mean 25.2; baseline 20.5 - 24.5, mean 22.7: **this batch ran under about 10% more load**), clock uncertainty 5.1 - 17.0 ms (baseline 6.3 - 17.2). All 30 exit 0. Burn threads stopped afterwards: `pgrep -cx xtask` printed 0 and `pgrep -cx fixture` printed 0.

| spec | baseline flash runs | this run | numeric events (certain / possible) | `lead_ms` median / max | baseline events (certain / possible), lead median / max |
| :--- | :---: | :---: | :---: | :--- | :--- |
| `bottom-right-smaller` | 6 / 10 | **0 / 10** | 0 | - | 11 (8 / 3), 29.7 / 78.9 |
| `bottom-right-larger` | 9 / 10 | **5 / 10** | 7 (5 / 2) | 50.3 / 76.4 | 16 (14 / 2), 32.1 / 60.0 |
| comparable 20 runs | **15 / 20** | **5 / 20** | 7 (5 / 2) | all 7: median 50.3, max 76.4 | 27 (22 / 5) |
| `top-left-smaller` (no baseline under load) | not measured | 2 / 10 | 2 (2 / 0) | 25.0 / 28.6 | - |
| all 30 | - | 7 / 30 | 9 (7 / 2) | median 28.6 | - |

- **Relative reduction against the baseline 15 of 20: 10 of 15 = 66.7%** (`bottom-right-smaller` 100%, `bottom-right-larger` 44%). Fisher exact, two-sided, 5/20 against 15/20: p = 0.0038 (a difference that size is unlikely to be chance; the point estimate is still below 70%, and 4/20 would have been 73.3%). Calculated here, not by the harness.
- **What kind of flash is left (CONFIRMED).** Events by kind, baseline -> now: `position-change-after-reveal` (an `ANCHOR_CORRECTION` after the reveal) 15 -> 2; `size-change-after-reveal` (the client's first paint at the final size after the reveal) 12 -> 7. Runs with a correction after the reveal: baseline 15 of 20, now 2 of 20 (`ANCHOR_CORRECTION` appears twice in the log, both `bottom-right-larger`, +61.0 and +50.3 ms). The 7 `size-change-after-reveal` events are 14.1 - 76.4 ms (`bottom-right-larger`) and 21.4, 28.6 ms (`top-left-smaller`).
- **Outcomes of `READY_WAIT_RESOLVED` (30 runs, CONFIRMED): 27 `reached` (`at-target`), 3 `timed-out` (reason `cap`, all `bottom-right-larger`, `waited_ms` exactly 150.0), 0 `adjusted`, 0 `skipped`.** Flash by outcome: **3 of 3 `timed-out` runs flashed; 4 of 27 `reached` runs flashed** (2 `bottom-right-larger`, 2 `top-left-smaller`; all `size-change-after-reveal`). `waited_ms` median / p95 / max: all 104.4 / 150.0 / 150.0; `bottom-right-smaller` 88.0 / 106.4 / 106.4; `bottom-right-larger` 136.6 / 150.0 / 150.0; `top-left-smaller` 101.2 / 142.5 / 142.5.
- **Cloak (CONFIRMED).** Median / p95 / max, ms: all 30 117.7 / 166.6 / 166.9 (`bottom-right-smaller` 105.1, `bottom-right-larger` 141.9, `top-left-smaller` 113.9 per-spec medians); baseline (20 runs) 49.9 / 75.8 / 79.8. The cloak-duration warning (budget 100 ms, 6.12/5) fired in **24 of 30** runs; baseline max 79.8 ms, so 0 of 20.
- Why 4 `reached` runs still flashed was not examined. The machine ends on a frame rect at the target; the flash is the client's first paint at that size arriving after the reveal. HYPOTHESIS: under load, the frame rect follows the committed buffer but the paint reaches the screen a frame or more later; not tested.

**XWayland: 10 runs of `bottom-right-larger --protocol x11`, run twice (batch A 11:41:39, load at run start 1.88 - 2.47; baseline 1.67 - 1.96; batch B 11:45:42, load 0.67 - 0.87).**

| | baseline (6.17) | batch A | batch B (repeat) |
| :--- | :--- | :--- | :--- |
| window recorded as / strategy | `proto=XWayland` | `proto=XWayland`, `strategy=legacy kind=XWayland`, 10 of 10 | same, 10 of 10 |
| `READY_WAIT_RESOLVED` | none | `legacy outcome=none reason=AT_TARGET`, 10 of 10 | same, 10 of 10 |
| `COMMIT_RESOLVED` after the request | 0.1 - 0.3 ms | 0.2 - 0.3 ms | not computed |
| **numeric flashes** | **1 of 10** | **9 of 10** (3 certain, 6 possible; lead median 16.0, max 25.3 ms) | **10 of 10** (1 certain, 9 possible; median 11.6, max 21.0 ms) |
| `ANCHOR_CORRECTION` / late resize | 0 | 0 | 0 |
| cloak ms, median | 31.59 (p95 36.80) | 12.85 (p95 25.96) | 11.66 (p95 19.75) |
| `UNCLOAK_4C` to `UNCLOAK`, median | 28.4 ms | 9.3 ms | 8.7 ms |

**The XWayland result does not match the baseline (CONFIRMED), and it is not the wiring (CONFIRMED by the diff).** The strategy is `legacy` in 20 of 20 as required. `git diff 0756167 HEAD -- assets/gnome/modern/` shows the legacy path calls the same `waitForCommit` as before; the only changes to it are two added log lines and `resolve()` becoming `resolve(reason)`, and the event sequence of a baseline window and a new window is identical (same steps, same `AT_TARGET` at 0.2 ms). The difference is the time between `UNCLOAK_4C` and `UNCLOAK` (the forced stage update, 28 ms before, 9 ms now): HYPOTHESIS that in the baseline login the reveal came late enough for the X11 client to paint at 800x600 first (1 of 10), and in this login it comes earlier and the paint follows (9 and 10 of 10), which is what native X11 showed in 6.15 (9 and 10 of 10). Why the stage update is quicker in this login was not examined. Practical consequence: the 1 of 10 in 6.17 was a property of that login, not of the XWayland path, and XWayland should be treated as flashing on this machine.

**Real apps, outcome survey from the extension log only (no screen capture; 15 spawns).** `spawn-at spawn --class <id> --anchor bottom-right --size W H --margin 16`, sizes alternating; each new window was found by diffing `query windows` and closed with `spawn-at close --id <that id>` (never by class, because the user's own terminal window was open); afterwards `query windows` listed only the 3 windows that existed before.

| app | requested (alternating) | outcome x5 | `waited_ms` (min / median / max) | cloak ms (min / median / max) | cap or hard limit |
| :--- | :--- | :--- | :--- | :--- | :--- |
| gnome-terminal (grid snapping), opens at 974x697 | 803x601, 905x517 | **5 `adjusted`**, reason `quiet`; final 794x589 (x3) and 902x508 (x2), no correction after the reveal | 58.5 / 62.8 / 67.7 | 66.4 / 68.0 / 80.3 | none |
| gnome-text-editor (remembers its size), opened at the previous size | 500x300, 580x380 | **5 `reached`**, `at-target`; each run a real resize (`current` was the other size) | 31.1 / 33.6 / 36.2 | 35.0 / 50.2 / 50.9 | none |
| Firefox 157.0.1 (snap), fresh throwaway profile, `--no-remote`, cold start each time | 700x500, 760x560 (minimum width 625) | **1 `timed-out` (run 1), 4 `reached`** | run 1 150.0 (cap); runs 2-5 50.3 / 55.8 / 60.0 | 64.9 / 69.2 / 156.7 | **cap once** (run 1); hard limit none |

- Firefox run 1 (CONFIRMED, log): the window still had its initial 1600x971 after 150 ms (`COMMIT_RESOLVED reason=TIMEOUT_NO_CHANGE`), was revealed at that size at (304,93), and `ANCHOR_CORRECTION size=(700x500)` came 68.9 ms after the reveal. Runs 2-5 had `current` equal to the other requested size (Firefox kept the size of the previous run) and reached the target in 50 - 60 ms. So the cold first start of a profile hits the cap; later starts did not. HYPOTHESIS that this matches the 6.13 finding that Firefox does not read its socket for about 190 ms at start-up; not traced.
- Firefox, CLI side (CONFIRMED, CLI output): in 3 of 5 spawns (runs 2, 4, 5) `spawn-at` printed `WARNING: Timed out waiting for window to map ... did not appear within 2s` and exit code 0, although the extension log has a `READY_WAIT_START` / `READY_WAIT_RESOLVED` pair for all five windows. Not examined further; it concerns the CLI's 2 s wait for the window, not the ready-wait.
- **No hard-limit (1500 ms) outcome occurred anywhere in this section.** The largest `waited_ms` is 150.0 (the cap), reached by 3 loaded fixture runs and 1 Firefox run.
- Clean-up (CONFIRMED): every survey window closed by id; no `firefox` process left (`pgrep -cx firefox` 0); the throwaway profile `~/snap/firefox/common/spawn-at-survey-profile` (53 MB, created for this survey) deleted.

**Judged against the proposed criteria.**

| # | criterion | result | met? |
| :---: | :--- | :--- | :---: |
| 1 | idle stays at 0 flashes | 0 of 40 (4 specs x 10, debug fixture); baseline 0 of 40 debug, 0 of 80 with release. Release fixture not re-run. | **Met** |
| 2 | loaded flashes drop by at least 70% against the baseline 15 of 20 | 5 of 20 = 66.7% reduction (p = 0.0038 against the baseline). 3.3 points short; one fewer flash would have met it. This batch also ran under about 10% more load. | **Not met** |
| 3 | idle cloak median rises by no more than 10 ms | 19.09 -> 41.10 ms = **+22.0 ms** (p95 21.22 -> 50.72). Per spec +19.7 to +28.4. (The design's own A3 asked for +5 ms.) | **Not met** |
| 4 | hung or refusing cases end by the hard limit | **Not shown.** No hung app and no refusing app was run (the fixture has no stall flag yet, `5.3`); the hard limit was never reached. What was seen: 4 runs (3 loaded fixture, 1 Firefox cold start) ended at the 150 ms cap, all revealed, none left cloaked. | **Not shown** |

Also against the design's loaded criterion A2 (flash <= 5 of 100, p95 cloak within the 100 ms budget): loaded flash is 5 of 20 and the loaded cloak p95 is 166.6 ms, so neither holds; this section does not use A2 for the verdict because the user's criteria above replace it.

**Before / after, one table.**

| measure | before (6.17 baseline) | after (this section) |
| :--- | :--- | :--- |
| idle flashes (debug) | 0 / 40 | 0 / 40 |
| idle cloak median / p95 (ms) | 19.1 / 21.2 | 41.1 / 50.7 |
| loaded flash runs, `bottom-right-smaller` | 6 / 10 | 0 / 10 |
| loaded flash runs, `bottom-right-larger` | 9 / 10 | 5 / 10 |
| loaded flash runs, the two together | 15 / 20 | 5 / 20 (-66.7%) |
| loaded runs with a correction after the reveal | 15 / 20 | 2 / 20 |
| loaded `lead_ms` median (flash events) | about 30 (29.7 and 32.1) | 50.3 (`bottom-right-larger`, 7 events); 28.6 over all 9 events |
| loaded cloak median / p95 (ms) | 49.9 / 75.8 | 117.7 / 166.6 |
| loaded runs over the 100 ms cloak budget | 0 / 20 | 24 / 30 |
| loaded outcomes | - | 27 reached, 3 timed-out (cap), 0 adjusted, 0 skipped |
| XWayland `bottom-right-larger` flash runs | 1 / 10 | 9 / 10 and 10 / 10 (two batches); strategy `legacy` |
| real apps | - | terminal 5 adjusted; text editor 5 reached; Firefox 4 reached + 1 cap (cold) |

**Not shown / not done.**
- A same-session before/after (needs the old rule switchable or a log-out between builds); every comparison here crosses two logins, and the XWayland result shows that crosses a real change in reveal timing.
- Whether the +22 ms idle cloak comes from the wiring or from this login (the app answered about 20 ms later; not separated).
- A hung app, a refusing app (below its minimum), a not-resizable app (`Skipped`), the hard limit, the learned-minimum cache (design A4 - A7).
- The release fixture, a release `spawn-at`, 100-run cells, native X11, XWayland under load, `top-left-smaller` baseline under load.
- Any screen capture: nothing here says which of the 7 flashes (5 certain by lead >= 16.7 ms) is visible to a person.
- Why 4 of 27 `reached` loaded runs still flashed, why the loaded batch's `bottom-right-larger` timed out at the cap 3 times in 10, and the cause of the changed XWayland timing.
- Firefox with a real profile, a warm Firefox, other sizes; only the extension log was read.
- Scripts used (`scratch/rw_extract.sh`, `scratch/rw_stats.sh`, `scratch/app_survey.sh`, `scratch/baseline_batch.sh`) are throwaway and not committed; raw per-run JSON stayed in the session scratchpad.

### 6.19 Ready-wait: same-session A/B/C with the per-spawn override (2026-10-13, no code change)

Labels: CONFIRMED = measured or read in this section; HYPOTHESIS = not tested. Nothing in the product, the extension or the harness was changed; the only file written in the repository is this section. Tests (`cargo test`) were not run in this step. Arms, all in one login and one extension instance, chosen per spawn with the hidden flags from `44ee6b7` / `80ef74a`: **A** = `--wait-strategy legacy` (the 30 ms rule), **B** = `--wait-strategy ready-wait --wait-cap-ms 150`, **C** = `--wait-strategy ready-wait --wait-cap-ms 250`. Binary under test: debug `spawn-at`, `spawn-at-verify` and `fixture` built from `5a2c7be`'s code (`binary_commit` in the report is `5a2c7be`, `is_dirty_build` false). Session: `XDG_SESSION_TYPE=wayland`, GNOME Shell 46, 22 CPUs, 60 Hz, one monitor.

**What A is (CONFIRMED by reading, HYPOTHESIS for equivalence).** A is the legacy rule reached through the override, in the same extension that holds the ready-wait code. It is not a build from before the change. 6.18 showed by diff that the legacy path calls the same `waitForCommit`; that A behaves like the pre-change build is therefore expected but not shown.

**Preflight (all CONFIRMED before any measurement).**

| check | result |
| :--- | :--- |
| session type | `wayland` (env and `loginctl`) |
| extension | `gnome-extensions info spawn-at@harsh.local`: `Enabled: Yes`, `State: ACTIVE`; `diff -rq assets/gnome/modern ~/.local/share/gnome-shell/extensions/spawn-at@harsh.local` printed nothing (files installed 12:27:22 IST, GNOME Shell process started 12:30:49 IST, so the running code is the installed code) |
| protocol | `gdbus ... Properties.Get ... ProtocolVersion` returned `(<uint32 6>,)` |
| `--wait-strategy legacy` by hand (fixture, 200x150 bottom-right) | `READY_WAIT_OVERRIDE ... strategy=legacy cap_ms=none unused=none ignored=none`, `READY_WAIT_START ... strategy=legacy kind=NativeWayland why=override-legacy timeoutMs=300`, `READY_WAIT_RESOLVED ... strategy=legacy outcome=none elapsed_ms=28.7 reason=AT_TARGET` |
| `--wait-strategy ready-wait --wait-cap-ms 250` by hand | `READY_WAIT_OVERRIDE ... strategy=ready-wait cap_ms=250 unused=none ignored=none`, `READY_WAIT_START ... cap=250 quiet=30 hard=1500`, `READY_WAIT_RESOLVED ... outcome=reached waited_ms=28.5 reason=at-target` |

Both checks passed, so the run went ahead. One thing to know about the by-hand check: the extension's timing log is off by default, so the first attempt produced no lines. I turned it on with the same D-Bus call the harness makes at the start of every `run` (`org.gnome.Shell.Extensions.SpawnAt.SetLogging true`, an in-memory flag in the extension; no gsettings or dconf key was written, and nothing under `/sys` or `/proc`). The windows of the by-hand spawns were closed by id.

**The batch script (`scratch/ab_batch.sh`, throwaway) needed fixes (CONFIRMED).**
- The arm rotation started at the second arm: iteration 1 ran B C A, not A B C. Fixed (`k + i - 1`); a 3-iteration trial then ran `A B C`, `B C A`, `C A B`.
- It copied "the newest `verify-results/*/report.md`" after each run, which is a stale file if a run fails to write one. Removed: the `--json` stdout already holds `log_slice`, `flash_events`, `wait_strategy`, `wait_cap_ms`.
- My first trial passed the fixture as `--bin` (the script takes the `spawn-at` binary); every run exited 1 with `unexpected argument 'spawn'`. A caller mistake, not a script bug; the usage line now says so and the trial was discarded.
- Position balance over the real batches (how often each arm ran first / second / third): idle 21/18/21 (A), 21/21/18 (B), 18/21/21 (C); loaded bottom-right 14/12/14, 14/14/12, 12/14/14; loaded top-left 4/3/3, 3/4/3, 3/3/4. 20 iterations do not divide by 3, so the balance is not exact.
- Every run is checked against its arm: `wait_strategy` and `wait_cap_ms` in the report (A: legacy and no cap; B: ready-wait and 150; C: ready-wait and 250), `READY_WAIT_OVERRIDE` in the window's own log lines naming the same strategy, `wait_notes` empty, spawn `exit_code` 0. **0 of 330 runs failed a check** (idle 180, loaded 150), so none was dropped. The check script is `scratch/ab_rows.jq` / `ab_stats.jq`, also throwaway.

**Definitions.** Flash run = a run with at least one numeric flash event (the harness's client-side basis, 6.15 / 6.16). "Certain" = `lead_ms` of at least one frame (16.7 ms), "possible" = below one frame; the counts of events at or above and below 16.67 ms equal the harness's certain and possible counts in every arm (69 / 11, 13 / 8, 6 / 6, CONFIRMED). Cloak = first `UNCLOAK` minus first `CLOAK` of the window on the extension clock, taken from the run's own `log_slice`. `waited_ms` is from `READY_WAIT_RESOLVED`; arm A has no `waited_ms`, so its `elapsed_ms` is shown (the legacy rule's time in the wait). Median of an even count = mean of the two middle values; p95 = nearest rank. Fisher exact, two-sided, was computed with `jq` (`lgamma`) and checked against known values (5/20 against 15/20 gives 0.00385, the 6.18 figure). Runs are `spawn-at-verify run --dir scratch/flashlib --bin target/debug/spawn-at --glob placement/<spec> --json <arm flags>`, one process per run, specs and arms interleaved, 1.5 s sleep after each run.

**Idle, debug fixture: `bottom-right-smaller`, `bottom-right-larger`, `top-left-smaller`, 20 runs per arm per spec (60 per arm), 13:06:16 - 13:16:47 IST. All 180 exit 0.** Mean load average (1 min, at run start): A 1.51, B 1.51, C 1.49.

| | A legacy | B cap 150 | C cap 250 |
| :--- | :--- | :--- | :--- |
| flash runs | 1 / 60 | **0 / 60** | **0 / 60** |
| numeric events (certain / possible); `lead_ms` | 2 (0 / 2); 8.4 and 9.2 ms | 0 | 0 |
| cloak ms median / p95 / max | 41.2 / 65.7 / 68.6 | 39.8 / 50.5 / 52.8 | 40.6 / 51.0 / 53.5 |
| cloak median change against A | - | **-1.4 ms** | **-0.6 ms** |
| `READY_WAIT_RESOLVED` outcome | none / AT_TARGET x49, none / SIZE_SETTLED x11 | reached / at-target x60 | reached / at-target x60 |
| `waited_ms` (A: `elapsed_ms`) median / p95 / max | 29.0 / 33.6 / 43.8 | 29.8 / 38.7 / 40.8 | 29.0 / 39.8 / 40.0 |
| runs with a correction after the reveal | 1 | 0 | 0 |
| runs over the 100 ms cloak budget | 0 | 0 | 0 |
| relative flash reduction against A, Fisher p | - | 100% (1 to 0), p = 1.0 | 100% (1 to 0), p = 1.0 |

Cloak median per spec (A / B / C, ms): `bottom-right-smaller` 40.3 / 38.6 / 38.6; `bottom-right-larger` 50.3 / 40.5 / 48.9; `top-left-smaller` 40.6 / 42.8 / 40.4. The 11 A runs that ended on `SIZE_SETTLED` were all `bottom-right-larger` (cloak median of those 11: 62.9 ms; the other 49: 40.2 ms), and the one A flash was one of them. The cap was never reached at idle (largest `waited_ms` 40.8). The 1 of 60 for A is not a significant difference from 0 of 60 (p = 1.0); the figure to take from this table is "idle ready-wait stayed at 0 and its cloak was not longer than the legacy rule's".

**Loaded, 22 `burn-cpu` threads (`xtask burn-cpu --threads 22`, 70 s warm-up, load average 22.1 at the end of the warm-up), debug fixture. `bottom-right-smaller` and `bottom-right-larger` 20 runs per arm per spec (13:23:06 - 13:33:16 IST), `top-left-smaller` 10 per arm (13:33:16 - 13:35:50). All 150 exit 0.** Mean load average at run start: A 25.80, B 25.86, C 25.77 (ranges 22.1 - 27.8, 22.6 - 28.3, 22.7 - 28.1), so the three arms ran under the same load. The burn was stopped by its PID; afterwards `pgrep -cx xtask` printed 0 and `pgrep -cx fixture` printed 0.

| all 50 runs per arm | A legacy | B cap 150 | C cap 250 |
| :--- | :--- | :--- | :--- |
| flash runs | 47 / 50 | 15 / 50 | 11 / 50 |
| numeric events (certain / possible) | 80 (69 / 11) | 21 (13 / 8) | 12 (6 / 6) |
| `lead_ms` median / max | 39.5 / 278.5 | 18.9 / 49.4 | 16.9 / 30.4 |
| cloak ms median / p95 / max | 95.6 / 133.8 / 140.9 | 131.6 / 167.0 / 183.9 | 129.3 / 200.9 / 263.4 |
| `READY_WAIT_RESOLVED` | SIZE_SETTLED x49, AT_TARGET x1 | reached x40, **timed-out (cap) x10** | reached x49, **timed-out (cap) x1** |
| `waited_ms` (A: `elapsed_ms`) median / p95 / max | 86.0 / 119.6 / 128.8 | 120.9 / 150.0 / 150.0 | 119.1 / 188.2 / 250.0 |
| runs with a correction after the reveal | 40 | 6 | 1 |
| runs over the 100 ms cloak budget / over 150 ms | 19 / 0 | 41 / 13 | 42 / 16 |
| **relative flash reduction against A** | - | **68.1%**, Fisher p = 1.6e-11 | **76.6%**, Fisher p = 5.3e-14 |

| by spec (flash runs; cloak median ms) | A | B | C |
| :--- | :--- | :--- | :--- |
| `bottom-right-smaller` (20) | 20; 95.9 | 5; 105.7 | 4; 107.5 |
| `bottom-right-larger` (20) | 20; 93.6 | 8; 154.3 | 5; 162.6 |
| the two bottom-right specs together (40) | 40 / 40 | 13 / 40, **-67.5%**, p = 1.6e-11 | 9 / 40, **-77.5%**, p = 3.8e-14 |
| `top-left-smaller` (10) | 7; 103.2 | 2; 115.5, -71.4%, p = 0.070 | 2; 113.2, -71.4%, p = 0.070 |

- **Flash by outcome (CONFIRMED).** B: 8 of 40 `reached` runs flashed, 7 of 10 `timed-out` runs flashed. C: 10 of 49 `reached`, 1 of 1 `timed-out`. B's 10 cap timeouts were 9 in `bottom-right-larger`, 1 in `top-left-smaller`, 0 in `bottom-right-smaller`; C's one was in `bottom-right-larger`. The flashes left in B and C are mostly `reached` runs (the client's first paint at the final size arriving after the reveal; why was not examined, as in 6.18).
- **Residual size of the flashes (CONFIRMED).** The largest `lead_ms` is 278.5 ms in A, 49.4 ms in B, 30.4 ms in C. 40 of A's 50 runs have an `ANCHOR_CORRECTION` after the reveal (B 6, C 1); why the legacy rule reveals early in these runs was not traced.
- **B against C (CONFIRMED arithmetic).** 15 of 50 against 11 of 50: Fisher p = 0.49 (bottom-right only, 13 / 40 against 9 / 40: p = 0.45). At this sample size B and C are not distinguishable on flashes; both are far from A.

**Comparison with 6.18 (CONFIRMED numbers, cause HYPOTHESIS).** 6.18 reported +22 ms idle cloak and loaded 5 / 20 for the change, each against the baseline from the earlier login. In this login the legacy arm itself has an idle cloak median of 41.2 ms (6.17 baseline login: 19.1 ms) and flashes in 40 of 40 loaded bottom-right runs (baseline login: 15 of 20, under a lower mean load of 22.7 against 25.8 here). So (CONFIRMED) the +22 ms idle cloak seen in 6.18 is also present in the legacy arm of this login: it came with the login, not with the ready-wait rule, and the cross-login loaded comparison understated how much A flashes in a loaded session like this one. Why the login differs (HYPOTHESIS: the same difference as the XWayland `UNCLOAK_4C -> UNCLOAK` timing in 6.18) was not examined.

**Real apps, same session, A against B (extension log only; no screen capture).** `spawn-at spawn --class <id> --anchor bottom-right --size W H --margin 16 [override flags] -- <app>`; arm order A B B A A B B A A B, sizes alternating per spawn so each spawn is a real resize for apps that remember their size. Each new window was found by diffing `query windows` and closed with `spawn-at close --id <that id>`; afterwards `query windows` listed only the 2 windows that existed before (the user's), and `pgrep -cx` printed 0 for `gnome-text-edit`, `firefox`, `gnome-terminal-` (the server), `fixture`, `xtask`. 13:36 - 13:40 IST.

| app (requested sizes) | arm | outcome x5 | `waited_ms` (A: `elapsed_ms`) min / median / max | cloak ms min / median / max | CLI "did not appear" |
| :--- | :--- | :--- | :--- | :--- | :--- |
| gnome-text-editor (500x300, 580x380) | A | AT_TARGET x4, SIZE_SETTLED x1 | 27.4 / 30.6 / 33.8 | 34.3 / 35.4 / 50.5 | 0 / 5 |
| | B | reached x5 | 27.4 / 31.9 / 38.0 | 36.1 / 36.9 / 48.2 | 0 / 5 |
| gnome-terminal (803x601, 905x517; ends 794x589 and 902x508 in both arms) | A | SIZE_SETTLED x5 | 32.6 / 33.3 / 71.8 | **162.7 / 178.4 / 208.6** | 0 / 5 |
| | B | adjusted (reason `quiet`) x5 | 61.8 / 63.6 / 74.7 | **68.0 / 80.5 / 81.4** | 0 / 5 |
| Firefox 157.0.1 snap, throwaway profile, `--no-remote` (700x500, 760x560) | A | AT_TARGET x4, SIZE_SETTLED x1 | 24.7 / 48.5 / 56.6 | 30.0 / 56.0 / 74.2 | **0 / 5** |
| | B | reached x5 | 51.1 / 56.3 / 59.2 | 60.9 / 72.2 / 74.0 | **0 / 5** |

- **Terminal (CONFIRMED numbers, cause HYPOTHESIS).** The legacy arm's cloak was 2 to 2.5 times the ready-wait arm's in all 5 pairs (163 - 209 ms against 68 - 81 ms). In the one A window read in the log (`id=1876180992`) the time was spent between `UNCLOAK_4B` and `UNCLOAK_4C` (131 ms, the forced stage update), not in the wait (33 ms). Why the stage update takes longer after a 33 ms wait than after a 62 ms wait was not traced; HYPOTHESIS that it waits for the terminal's own frame. Whether it is visible was not captured. No ready-wait terminal run had a correction after the reveal; none of the A runs did either.
- **Text editor (CONFIRMED).** No difference in wait or cloak between the arms (medians 30.6 / 31.9 ms wait, 35.4 / 36.9 ms cloak). Every B run was a real resize (`current` was the other size) and ended `at-target`.
- **Firefox (CONFIRMED log).** One uncounted warm-up spawn (default strategy, `auto`, which is ready-wait) created the profile: `timed-out`, `waited_ms` 150.0 (cap), window still 1600x971, revealed at that size at (304, 93), `ANCHOR_CORRECTION` to 700x500 85 ms after the reveal. The 10 counted spawns were all warm starts (the profile existed, Firefox had exited between spawns). A run 4 (legacy): `SIZE_SETTLED` after 31 ms while the frame was still 700x500 (requested 760x560), revealed at 700x500, `ANCHOR_CORRECTION` to 760x560 22 ms after the reveal. B had 0 corrections in 5. Whether a person sees A's correction was not captured. B hit the 150 ms cap 0 times in 15 real-app spawns (the only real-app cap hit was the cold warm-up).
- **"did not appear" (CONFIRMED count, meaning HYPOTHESIS).** 0 of 5 in arm A, 0 of 5 in arm B, 0 of 1 in the warm-up, and 0 of 30 over all three apps. 6.18 saw 3 of 5. It did not reproduce in either arm, so this run cannot say whether the warning predates the change; it gives no sign that the override or the ready-wait causes it (arm A, which does not wait for the app, did not show it either). What differed from 6.18 (profile reused between spawns, a wait for the Firefox process to exit between spawns, a separate login) was not isolated.

**Judged, arm B and C against A (same session).**

| # | criterion | B cap 150 | C cap 250 |
| :---: | :--- | :--- | :--- |
| 1 | idle stays at 0 flashes | **Met**: 0 / 60 (A: 1 / 60) | **Met**: 0 / 60 |
| 2 | loaded flashes down at least 70% | **Not met**: 68.1% (15 / 50 against 47 / 50); bottom-right only 67.5%; top-left alone 71.4% (n = 10, p = 0.07). One flash fewer (14 / 50) would be 70.2%. | **Met**: 76.6% (11 / 50); bottom-right only 77.5%; top-left alone 71.4% |
| 3 | idle cloak median up by no more than 10 ms | **Met**: -1.4 ms (39.8 against 41.2) | **Met**: -0.6 ms (40.6 against 41.2) |
| 4 | hung or refusing apps end by the hard limit (from 6.18) | **Not shown** | **Not shown** |

**Cap 150 against cap 250: what each buys and costs (CONFIRMED numbers).**

| measure | A | B 150 | C 250 | C minus B |
| :--- | :--- | :--- | :--- | :--- |
| **loaded** flash runs (of 50) | 47 | 15 | 11 | -4 |
| flashes avoided against A (runs) | - | 32 | 36 | +4 |
| loaded runs ending at the cap | - | 10 | 1 | -9 |
| loaded cloak median (ms), change against A | 95.6 | 131.6 (+36.0) | 129.3 (+33.7) | -2.3 |
| loaded cloak p95 (ms), change against A | 133.8 | 167.0 (+33.2) | 200.9 (+67.1) | **+33.9** |
| loaded cloak max (ms), change against A | 140.9 | 183.9 (+43.0) | 263.4 (+122.5) | **+79.5** |
| loaded runs over 100 ms / over 150 ms cloak | 19 / 0 | 41 / 13 | 42 / 16 | +1 / +3 |
| **idle** flash runs (of 60) | 1 | 0 | 0 | 0 |
| idle cloak median / p95 / max (ms) | 41.2 / 65.7 / 68.6 | 39.8 / 50.5 / 52.8 | 40.6 / 51.0 / 53.5 | +0.8 / +0.5 / +0.7 |
| idle runs reaching the cap | - | 0 | 0 | 0 |

**Cap recommendation: 250 (a recommendation; the decision is the user's).** At idle the two caps are indistinguishable and neither is ever reached (largest wait 40.8 ms), so the cap costs nothing there. Under load only 250 meets the 70% criterion (76.6% against 68.1%), because it turns 9 of B's 10 cap timeouts into runs that reached the target. The price is the tail of the cloak: p95 +34 ms and max +80 ms against 150 (263 ms at worst), paid only in runs where the app is that slow. Reasons to hold this loosely: (1) B and C are not separable on flashes at n = 50 (p = 0.49); B misses the 70% line by one flash; (2) both caps leave 41 - 42 of 50 loaded runs over the 100 ms cloak budget (A: 19 of 50), so no cap restores the budget at this load; (3) the load is extreme (load average 26 on 22 CPUs, all cores spinning), and a cap above 250, or one in between (200), was not tried; (4) the debug fixture, not a release build.

**What this section cannot show (stated plainly).**
- **Hung or refusing apps.** The fixture has no flag to stall or refuse a resize, so the 1500 ms hard limit, the `skipped` outcome and the not-resizable case were never exercised. The only cap hits were slow-but-responsive runs (loaded fixture, cold Firefox).
- **Whether any flash is visible.** There is no screen capture: "flash" is the harness's numeric client-side measure and "certain" is a lead of at least one frame; nothing here says which of the 21 and 12 remaining events a person would see. The real-app rows come from the extension log only.
- **X11 and XWayland.** Not run (the legacy path for X11 and XWayland is a separate path; 6.18 recorded its behaviour).
- **Pre-change build.** A is the legacy rule through the override, not a build without the ready-wait code.
- **Release build, other hardware, other sessions.** One machine, one login, debug fixture and debug `spawn-at`; load generated only by `burn-cpu`; 20 iterations per arm per spec do not divide evenly by the 3 rotations.
- **Real apps.** n = 5 per arm and app; Firefox warm profile only (after one uncounted cold warm-up); sizes limited to two per app; the terminal's cloak cause and the 6.18 Firefox CLI warning were not traced.
- **Not examined.** Why `reached` loaded runs still flash (8 of 40 in B, 10 of 49 in C); why the login differs from the baseline login.

**Clean-up and files (CONFIRMED).** Every spawned window was closed by id; no `fixture`, `xtask`, `firefox`, `gnome-text-edit` or terminal-server process remained; the throwaway Firefox profile `~/snap/firefox/common/spawn-at-ab-profile` (59 MB, created for this section) was deleted. Scripts (`scratch/ab_batch.sh`, `ab_loaded.sh`, `ab_rows.sh`, `ab_rows.jq`, `ab_stats.sh`, `ab_stats.jq`, `app_ab.sh`, `rw_extract2.sh`) are throwaway, gitignored and not committed; raw per-run JSON (330 fixture runs) and the app logs stayed in the session scratchpad and are not in the repository.

### 6.20 Why arms B and C still flash under load: classification and a protocol-level re-run (2026-10-13, no code change)

Labels: CONFIRMED = measured or read in this section; HYPOTHESIS = not tested. Nothing in the product, the extension, the fixture or the harness was changed; the only file written in the repository is this section. Tests (`cargo test`) were not run. Question: why do B (cap 150) and C (cap 250) still flash in 21 loaded runs, and is that the mechanism of A's shrink flashes? The user's observation to test: the remaining flashes do not look like "the app answered late".

**Data and what was done beyond reading it.**
- **Dataset (CONFIRMED).** `~/spawn-at-ab-data` is identical to `/tmp/myload` (`diff -rq` printed nothing). 150 run JSONs: `br/` (bottom-right-smaller and -larger, 20 per arm per spec, 14:02:55 - 14:12:46 IST) and `tl/` (top-left-smaller, 10 per arm, 14:12:46 - 14:15:15), 22 `burn-cpu` threads, `binary_commit` `0f90bc1`, not dirty, Wayland, 22 CPUs. Mean load at run start: A 22.62, B 22.63, C 22.62. `br/exits.txt` also holds 32 lines of an earlier, aborted batch (13:54 - 13:57); their JSONs were overwritten by the real batch (every JSON is dated 14:03:01 or later), so they are not in the data. The numbers reproduce exactly: flashed runs A 31 (larger 20/20, smaller 7/20, top-left 4/10), B 13 (6, 5, 2), C 8 (3, 4, 1); every run has the right `wait_strategy` / `wait_cap_ms` and no `wait_notes`.
- **Two live batches of my own, to see the protocol (disclosed because the task named a dataset).** The fixture can write its Wayland protocol dump and its own events with `SPAWN_AT_FIXTURE_TRACE_DIR` (`trace.rs`; libwayland timestamps on `CLOCK_REALTIME`, fixture events on `CLOCK_MONOTONIC`, related by the `TRACE_START` line). I ran arms B and C through the unchanged harness with it set: **loaded** 80 runs (bottom-right-smaller and -larger, 20 per arm per spec, 22 burn threads, 15:32 - 15:39 IST) and **idle** 40 runs (15:44 - 15:46 IST). Only fixture windows were created; the harness closes them; the burn was stopped by PID (`pgrep -cx xtask` and `pgrep -cx fixture` printed 0 afterwards). The protocol dump perturbs timing; the flash rate of the `reached` runs is nevertheless the same as in the untraced data (traced 12 of 72 = 16.7%, user's untraced B+C 16 of 94 = 17.0%). No gsettings or dconf key was written. As in 6.19, `SetLogging true` (an in-memory flag, the harness calls it itself) was sent over D-Bus, 120 times for the clock calibration below. My batch scripts read `/proc/loadavg` (read only) for the load figures; nothing was written under `/proc` or `/sys`.
- **Clock alignment (CONFIRMED).** 120 bracketed `SetLogging` calls (a throwaway Rust `CLOCK_MONOTONIC` reader in `scratch/`) before and after the loaded batch give the offset extension clock to monotonic as an interval 6.02 ms wide (the two calibrations overlap), so a comparison between an extension line and a client line is good to about +/-3 ms. The harness's own per-run anchor is 4 - 17 ms wide.

**Item 1 and 2: every flashed B and C run (21) and every flashed A shrink run (11), classified (user's dataset).** All 32 have `kind=NativeWayland`. Ready-wait engaged in all 100 B and C runs (`strategy=ready-wait`, no `why`, `target` differs from `current`); no run was `Skipped` or `adjusted`. Classes: **a** = ready-wait reported `reached` and the frame was at the target at the anchor, at `UNCLOAK_4C`, at the reveal and at the end, yet the harness saw a first paint after the reveal; **b** = ended at the cap (`timed-out`); **c** = position-only; **d** = ready-wait not the path (A is the legacy rule by design; d1 = the legacy quiet rule resolved on the old 400x300 frame, d2 = legacy reached the target); **e** = other / cannot tell. `lead` is milliseconds after the reveal; "certain" = at least one 16.7 ms frame.

| arm | spec | run | READY_WAIT outcome / reason | waited_ms | cloak ms | numeric flash events (lead ms, label) | what changed after the reveal | class |
| :--- | :--- | :---: | :--- | ---: | ---: | :--- | :--- | :---: |
| A | br-smaller | 1 | legacy / SIZE_SETTLED | 342.9 | 392.2 | position 178 (certain); first paint 174.9 (certain) | both: position (1504,764) -> (1704,914), and size | d1 |
| A | br-smaller | 2 | legacy / SIZE_SETTLED | 134.7 | 145.8 | position 8.7 (possible); first paint 17.8 (certain) | both | d1 |
| A | br-smaller | 3 | legacy / SIZE_SETTLED | 98.7 | 108.1 | position 7.7 (possible); first paint 17.7 (certain) | both | d1 |
| A | br-smaller | 9 | legacy / AT_TARGET | 103.2 | 107.3 | first paint 15.2 (possible) | content only (frame already 200x150 at (1704,914)) | d2 |
| A | br-smaller | 11 | legacy / SIZE_SETTLED | 98.3 | 105.7 | position 6.3 (possible); first paint 9.9 (possible) | both | d1 |
| A | br-smaller | 15 | legacy / SIZE_SETTLED | 113.4 | 125.2 | position 29.1 (certain); first paint 35.1 (certain) | both | d1 |
| A | br-smaller | 19 | legacy / SIZE_SETTLED | 84.6 | 90.3 | position 23.2 (certain); first paint 36.3 (certain) | both | d1 |
| A | tl-smaller | 5 | legacy / SIZE_SETTLED | 101.3 | 108.4 | first paint 16.9 (certain) | size (frame 400x300 at the reveal) | d1 |
| A | tl-smaller | 6 | legacy / SIZE_SETTLED | 99.3 | 112.0 | first paint 17.0 (certain) | size (frame 400x300 at the reveal) | d1 |
| A | tl-smaller | 9 | legacy / SIZE_SETTLED | 83.6 | 99.2 | first paint 24.2 (certain) | size (frame 400x300 at the reveal) | d1 |
| A | tl-smaller | 10 | legacy / SIZE_SETTLED | 118.7 | 124.6 | first paint 32.5 (certain) | size (frame 400x300 at the reveal) | d1 |
| B | br-larger | 2 | timed-out / cap | 150 | 155.5 | position 30.4 (certain); first paint 36.2 (certain) | both: position (1504,764) -> (1104,464), and size | b |
| B | br-larger | 3 | reached / at-target | 122.9 | 127.6 | first paint 10.7 (possible) | content only (frame already 800x600, same position) | a |
| B | br-larger | 4 | timed-out / cap | 150 | 157.6 | position 23.5 (certain); first paint 12.4 (possible) | both | b |
| B | br-larger | 10 | reached / at-target | 111.5 | 115.9 | first paint 19.1 (certain) | content only | a |
| B | br-larger | 17 | timed-out / cap | 150 | 169.3 | position 12.2 (possible); first paint 24.7 (certain) | both | b |
| B | br-larger | 18 | timed-out / cap | 150 | 162.0 | position 21.4 (certain); first paint 18.4 (certain) | both | b |
| B | br-smaller | 1 | timed-out / cap | 150 | 258.8 | position 141.9 (certain); first paint 170.1 (certain) | both: position (1504,764) -> (1704,914), and size | b |
| B | br-smaller | 4 | reached / at-target | 87.5 | 95.6 | first paint 12.1 (possible) | content only (frame already 200x150) | a |
| B | br-smaller | 12 | reached / at-target | 86.5 | 91.0 | first paint 11.1 (possible) | content only | a |
| B | br-smaller | 15 | reached / at-target | 89.2 | 96.5 | first paint 27.7 (certain) | content only | a |
| B | br-smaller | 19 | reached / at-target | 109.8 | 114.4 | first paint 17.3 (certain) | content only | a |
| B | tl-smaller | 1 | reached / at-target | 101.9 | 111.1 | first paint 20.0 (certain) | content only | a |
| B | tl-smaller | 2 | reached / at-target | 97.3 | 102.2 | first paint 16.9 (certain) | content only | a |
| C | br-larger | 2 | reached / at-target | 125.7 | 130.8 | first paint 22.2 (certain) | content only | a |
| C | br-larger | 4 | reached / at-target | 83.2 | 87.4 | first paint 15.9 (possible) | content only | a |
| C | br-larger | 16 | reached / at-target | 156.9 | 161.5 | first paint 11.6 (possible) | content only | a |
| C | br-smaller | 3 | reached / at-target | 109.4 | 118.9 | first paint 25.7 (certain) | content only | a |
| C | br-smaller | 9 | reached / at-target | 141.4 | 157.4 | first paint 15.8 (possible) | content only | a |
| C | br-smaller | 10 | reached / at-target | 110.3 | 116.2 | first paint 17.5 (certain) | content only | a |
| C | br-smaller | 19 | reached / at-target | 138.8 | 148.0 | first paint 17.5 (certain) | content only | a |
| C | tl-smaller | 7 | reached / at-target | 74.2 | 81.2 | first paint 18.4 (certain) | content only | a |

("A waited_ms" is the legacy rule's `elapsed_ms`; "content only" = the only numeric event is the fixture's first paint at the final size after the reveal, no frame size or position change.)

| class | A br-smaller | A tl-smaller | B larger | B smaller | B tl | C larger | C smaller | C tl | total |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| a reached, frame at target, first paint after the reveal | 0 | 0 | 2 | 4 | 2 | 3 | 4 | 1 | **B 8, C 8** |
| b ended at the cap | 0 | 0 | 4 | 1 | 0 | 0 | 0 | 0 | **B 5, C 0** |
| c position-only | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| d1 legacy, resolved on the old frame | 6 | 4 | - | - | - | - | - | - | **A 10** |
| d2 legacy, reached the target | 1 | 0 | - | - | - | - | - | - | **A 1** |
| e other | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| flashed runs | 7 | 4 | 6 | 5 | 2 | 3 | 4 | 1 | A 11 / B 13 / C 8 |

**Item 4: the shrink specs (CONFIRMED, user's dataset, all 90 shrink runs, not only flashed).** The request is 200x150 against the fixture's default 400x300 (half of each side, a quarter of the area; the fixture's minimum is 100x100, so it is legal). For B and C the ready-wait engaged in 60 of 60 (`target=(200x150) current=(400x300)`). The frame the anchor used equals the frame at `UNCLOAK_4C` in 60 of 60. The frame at the reveal is the target in 59 of 60 (the one exception is B bottom-right-smaller run 1, the cap timeout, still 400x300), and the printed final rectangle is the target in 59 of 60 (same run). A (control): the frame at the reveal is the target in 14 of 20 (bottom-right) and 6 of 10 (top-left); in the others it is the old 400x300. The anchor depends on the frame size for bottom-right (stale 400x300 gives (1504,764), the right frame (1704,914): 200 px and 150 px away) and not for top-left (16,56 either way), which is why A's top-left flashes carry no position event and its bottom-right flashes carry one.

**What the extension log shows for class a (CONFIRMED, user's dataset).** In all 16 `reached` flashes: `READY_WAIT_RESOLVED outcome=reached reason=at-target` came from a `size-changed` sample at the target, `APPLY_ANCHOR eff` is the target size, `UNCLOAK_4C actual` is the target size, the frame logged at `UNCLOAK` is the target size at the final position, no `ANCHOR_CORRECTION`, and the printed rectangle is the target. The `size-changed` sample was seen 4.2 - 15.9 ms (median 5.4) before the reveal. The only event after the reveal is the fixture's `FIRST_PAINT` at the target size, 10.7 - 27.7 ms (median 17.4) after the reveal, i.e. 15.5 - 35.3 ms (median 23.4) after the sample, larger than the harness's own anchor uncertainty (+/-4.3 - 15.3 ms) in all 16. Across all 113 `reached`-type runs with a `size-changed` sample (B, C and A's AT_TARGET), the flash rate falls with the time between `size-changed` and the reveal: under 5 ms 8 of 23, 5 - 8 ms 5 of 25, 8 - 12 ms 3 of 34, 12 - 20 ms 1 of 27, over 20 ms 0 of 4. And in 85 of the 96 `reached`-type runs that did not flash, the first paint at the target size lies inside the clock uncertainty of the reveal (median -0.9 ms), so the reveal sits right on top of that paint.

**The protocol-level re-run (CONFIRMED, my traced batches: 80 loaded runs, 72 `reached` and 8 `timed-out`; 40 idle, all `reached`).** Times in ms; `commit` = the `wl_surface.commit` carrying the target-size buffer in the fixture's own protocol dump; `stamp` = the fixture's `FIRST_PAINT` event, which the harness compares with the reveal.

| measure | loaded, `reached` (72) | idle (40) |
| :--- | :--- | :--- |
| harness flash runs | 12 (B 5 of 33, C 7 of 39) | 0 |
| `commit` minus `UNCLOAK` | **-39.9 / -12.2 / -3.3 (min / median / max): 0 of 72 after the reveal, 72 of 72 at least 3.3 ms before** | -10.7 median, max -3.3 |
| the 12 flashed runs: `commit` minus `UNCLOAK` | -14.1 to -3.3 (11 of 12 beyond the +/-3 ms alignment; the closest is C bottom-right-smaller run 5 at -3.3) | - |
| `stamp` minus `commit` | flashed runs 13.6 / 18.4 / 23.4; others 0 / 0 / 19 (17 of 60 above 3 ms) | **0 in 40 of 40** |
| extension `size-changed` minus `commit` (min / median / max) | -0.8 / 3.0 / 16.1 | -0.9 / 0.9 / 4.0 |
| configure received after `SET_SIZE_REQUEST` (median / max) | 93.1 / 148.5 | 26.2 median |
| `Resized` event to `commit`, i.e. drawing in the debug fixture (median) | 800x600: 48.6 (max 75.4); 200x150: 4.1 (max 37.7) | 8.2 and 1.0 |

- **Reading (CONFIRMED for the numbers).** In the traced loaded batch the extension's `size-changed` follows the client's commit of the target-size buffer by about 3 ms (so `size-changed` is the commit), and the commit is always before the reveal. Every flashed `reached` run is a run where the fixture's `FIRST_PAINT` stamp was written 13.6 - 23.4 ms after the commit it describes. At idle the stamp is never late and nothing flashes. So for the traced `reached` runs the harness's flash is a late timestamp, not content that arrived after the reveal.
- **Cause of the late stamp (HYPOTHESIS).** `FIRST_PAINT` is written by `note_paint` after `draw_frame` returns, i.e. after `buffer.present()` (attach, damage, commit, flush, `softbuffer` 0.4.8). No protocol line lies between the commit and the stamp in the six dumps read in full (the five traced timelines below and C bottom-right-larger #10). The delay is zero at idle, bimodal under 22 CPU-bound threads (0 or 5 - 23 ms), and not tied to the draw time. HYPOTHESIS: after the flush wakes the compositor, the fixture thread waits for a CPU behind the burn threads before it reaches the next `clock_gettime`. Not tested (it would need the stamp taken before `present`, or a scheduler trace).
- **Not shown.** That the committed buffer was on screen in the first frame after the reveal. The extension forces a stage update (`UNCLOAK_4B` -> `UNCLOAK`) after the commit in these runs, which is why it is expected, but no screen capture exists.
- **For the user's own 16 untraced `reached` flashes (HYPOTHESIS).** They were not traced. They match the traced class in everything the extension log shows (class a; flash rate 17.0% against 16.7%; the harness's first-paint lead 10.7 - 27.7 ms against 8.9 - 26.0 ms in the 12 traced flashes), so I expect the same cause; it is inferred, not shown for those runs.

**Timelines (CONFIRMED). Milliseconds after `SET_SIZE_REQUEST` (ext) on one clock, client lines from the fixture's dump and events. Traced re-run, loaded.** Rows: MAP = `ACTOR_MAP`; configure / ack / `Resized` / buffer / commit / `FIRST_PAINT` are the client; the rest are extension lines. No `MOVED` event occurs in any of the 120 traced runs; the harness's position events come from the extension's `ANCHOR_CORRECTION`.

| t | line | B grow, reached, flashed (bottom-right-larger #4) | B shrink, reached, flashed (bottom-right-smaller #5) | C shrink, reached, flashed (bottom-right-smaller #14) |
| :--- | :--- | :--- | :--- | :--- |
| MAP / `CLOAK` | ext | -0.5 / -0.1 | -0.6 / -0.1 | -0.7 / -0.1 |
| `SET_SIZE_REQUEST` | ext | 0.0 (800x600) | 0.0 (200x150) | 0.0 (200x150) |
| `READY_WAIT_START` (cap) | ext | 0.3 (150) | 0.2 (150) | 0.3 (250) |
| configure for the target | client | 100.1 | 110.0 | 97.2 |
| ack, `set_window_geometry`, `Resized` | client | 100.3 / 100.4 / 101.1 | 110.2 / 110.3 / 110.7 | 97.4 / 97.5 / 97.9 |
| buffer for the target created | client | 101.4 | 110.8 | 98.1 |
| `wl_surface.commit` of the target buffer | client | **131.3** | **120.7** | **100.3** |
| `COMMIT_SAMPLE source=size-changed` | ext | 133.5 | 120.0 | 99.8 |
| `READY_WAIT_RESOLVED reached` (waited_ms) | ext | 133.8 (133.6) | 120.2 (120.0) | 100.1 (99.8) |
| `APPLY_ANCHOR eff` -> `POSITION_SET` | ext | 800x600 -> (1104,464) at 134.2 / 134.3 | 200x150 -> (1704,914) at 120.8 / 121.0 | 200x150 -> (1704,914) at 100.7 / 100.8 |
| `UNCLOAK_4B` / `UNCLOAK_4C` (needsReanchor) | ext | 135.0 / 135.0 (false) | 121.7 / 121.7 (false) | 101.6 / 101.7 (false) |
| **`UNCLOAK`** | ext | **138.2** | **130.1** | **112.6** |
| **`FIRST_PAINT` target size (stamp)** | client | **145.0** (6.8 after the reveal; 13.7 after the commit) | **134.8** (4.7; 14.1) | **121.8** (9.2; 21.5) |
| `ANCHOR_CORRECTION` | ext | none | none | none |

For each of the three, the commit precedes the reveal (by 6.9, 9.4 and 12.3 ms) and the stamp follows it.

Two traced timed-out runs (class b), same layout:

| t | B grow, timed-out, flashed (bottom-right-larger #11) | B shrink, timed-out, position-only flash (bottom-right-smaller #11) |
| :--- | :--- | :--- |
| `SET_SIZE_REQUEST` | 0.0 (800x600) | 0.0 (200x150) |
| configure / `Resized` / buffer created | 118.8 / 119.3 / 119.6 | 126.8 / 127.6 / 127.8 |
| `READY_WAIT_RESOLVED timed-out`, waited 150 (`COMMIT_RESOLVED TIMEOUT_NO_CHANGE`, frame 400x300) | 153.4 | 161.5 |
| `APPLY_ANCHOR eff` -> `POSITION_SET` | 400x300 -> (1504,764) at 154.8 / 154.9 | 400x300 -> (1504,764) at 162.5 / 162.7 |
| `UNCLOAK_4C` (needsReanchor) | 155.9 (false, 400x300) | 164.1 (false, 400x300) |
| `wl_surface.commit` of the target buffer (client) | 234.5 (**72.7 after the reveal**) | 179.1 (11.0 **before** the reveal, 15 ms after 4C) |
| `UNCLOAK`, frame logged | 161.8, (1504,764,400x300) | 190.1, (1504,764,400x300) |
| `ANCHOR_CORRECTION` | 238.4: (1504,764) -> (1104,464), size 800x600 | 209.1: (1504,764) -> (1704,914), size 200x150, **19.0 after the reveal** |

In the shrink one the commit left the client before the reveal, but the frame the extension logged at the reveal was still 400x300 (`size-changed` was processed after it): the extension's own log marks a position change after the reveal, so that flash is real, and it is an anchor computed from the stale frame (class c in its symptom, class b in its cause: the cap ended the wait at 161.5 ms, 17.6 ms before the commit).

And an A (legacy) shrink from the user's dataset, extension log only (bottom-right-smaller #15): `CLOAK` 0.0, `SET_SIZE_REQUEST` 0.2, `READY_WAIT_START strategy=legacy why=override-legacy` 0.7, `COMMIT_SAMPLE allocation (400x300)` 82.1, **`COMMIT_RESOLVED SIZE_SETTLED finalFrame=(400x300)` 114.0**, `APPLY_ANCHOR eff=(400x300)` -> (1504,764) at 114.9 / 115.1, `UNCLOAK_4C actual=(400x300)` 116.9, `UNCLOAK` 125.2 with frame (1504,764,400x300), **`ANCHOR_CORRECTION size=(200x150) (1504,764) -> (1704,914)` 154.3 (29.1 after the reveal)**, first paint at 200x150 35.1 ms after the reveal. The app had not answered when the legacy rule stopped waiting.

**Is it the same mechanism as A's shrink flashes? No (CONFIRMED for the classes; the cause of class a is partly HYPOTHESIS).**
- **A (11 flashed shrink runs):** 10 of 11 are the legacy quiet rule resolving on the old 400x300 frame (class d1): the reveal happens with the wrong size, bottom-right is anchored on the stale frame and corrected 6 - 178 ms later, the top-left ones change size. That is "the app answered late" in the plain sense, and it is what ready-wait removes. The 11th (d2, AT_TARGET) looks like class a.
- **B and C reached (16 of 21, class a):** ready-wait waited for the app, the frame, the anchor and the reveal are all at the target, and nothing moves. The user's observation is **CONFIRMED for these 16**: they are not "the app answered late". In the traced re-run the content was committed before the reveal and the flash is the fixture's late `FIRST_PAINT` stamp.
- **B timeouts (5 of 21, class b):** the observation does **not** hold for these. The app really was late for a cap of 150 ms: on this loaded debug fixture the configure arrives 94 ms (median, max 149) after the request and drawing takes 49 ms (800x600), so the commit lands at about 145 ms, next to the cap; 4 of the 8 traced timeouts flashed and 3 of those 4 committed 35 - 73 ms after the reveal. C (cap 250) had none in the user's data (in my traced C batch one run's cap timer fired 450 ms late: `elapsed_ms` 700.8 against `waited_ms` 250, cloak 866 ms, commit before the reveal, not flashed; a Shell-side stall is the likely reason, HYPOTHESIS, not examined).

**Item 5: what the evidence supports, what it does not, proposals (not implemented).**

| class | share of the 21 B/C flashes | supported by | not shown | smallest fix (proposal) | measurement needed to confirm |
| :--- | :--- | :--- | :--- | :--- | :--- |
| a reached, then "first paint after the reveal" | 16 | frame state at every step (user's data); 72 of 72 traced `reached` runs committed at least 3.3 ms before the reveal; stamp lag 13.6 - 23.4 ms in the 12 flashed, 0 at idle | that the buffer was displayed in the first frame; the cause of the stamp lag; the 16 untraced runs individually | harness / fixture only: take the `FIRST_PAINT` time before `present()` (or write the commit time) so the stamp cannot be later than the commit; no product change | re-run the traced loaded batch with the stamp moved: `reached` flashes should fall to about 0 and equal the dump's commit times; plus a screen capture or a compositor-side frame time for the first frame after `UNCLOAK`; a release fixture (drawing time 49 ms in debug) and one toolkit app for comparison |
| b cap timeout | 5 (B only) | log lines and dump: configure at 90 - 120 ms, drawing 30 - 115 ms, commit after the cap, position corrected after the reveal | whether 250 is enough on a release fixture or a real app; the Shell stall tail | raise the default cap from 150 to 250 (as 6.19 recommended); for the sub-case where the commit lands between `UNCLOAK_4C` and `UNCLOAK`, re-read the frame when the reveal is applied | cap sweep 150 / 200 / 250 / 300, 100 runs per arm, loaded, debug and release fixture; count timeouts, commits between 4C and `UNCLOAK`, and corrections after the reveal |
| c position-only | 0 as the first cause; 1 symptom in a traced timeout | one traced run (above) | frequency | none beyond b (a stale frame is only anchored on after a cap) | the same sweep, counting runs whose only event is a position correction |
| d ready-wait not engaged | 0 for B and C; A by design | all 100 B and C runs engaged | - | none (A is the control) | - |
| e other | 0 | - | - | - | - |

**What cannot be shown here.** Hung or refusing apps (no fixture flag), the 1500 ms hard limit, visibility on screen (no capture), X11 and XWayland, a release-build fixture (the debug fixture's software drawing is 49 ms at 800x600 under load and makes the waits longer), real toolkit apps under this analysis (their first-paint timing is not stamped by this fixture and a real late paint cannot be excluded), and the 16 untraced `reached` runs individually. Traced runs carry the protocol dump's own overhead. The alignment between the extension and client clocks is +/-3 ms (one run is at the edge).

**Files (CONFIRMED).** Throwaway and not committed: `scratch/lf_rows.jq`, `lf_abs.jq`, `lf_trace.awk`, `lf_client.awk`, `lf_join.sh`, `lf_timeline.sh`, `lf_calib.sh`, `lf_trace_loaded.sh`, `mono.rs` (a 10-line `CLOCK_MONOTONIC` reader). Raw traced runs and the dumps stayed in the session scratchpad. After the work: no `fixture` or `xtask` process, no window of mine open (`query windows` lists the user's 2).

### 6.21 Default cap 250 ms and the wire paint time: code, tests and a traced loaded A/B/C (2026-10-13)

Labels: CONFIRMED = measured or read in this section; HYPOTHESIS = not tested. Sections 6.18 to 6.20 are unchanged: their 150 ms figures are what was measured then. Commits: `e9d5c77` (default cap 250), `8d65a48` (wire paint time in the harness), this section. The fixture (`src/bin/fixture/`) is **not** changed: `FIRST_PAINT` is still stamped after `present()` returns, so a real late paint stays visible in the stamp too (6.20 asked for exactly that).

**1. Default cap 150 to 250 ms (CONFIRMED, code and tests; not live).**
- Changed: `Policy::default()` (`ready_wait.rs`), `DEFAULT_POLICY` (`ready_wait.js`), `tests/ready_wait_vectors/policy.json`, golden vectors 02 (answer at 240 ms is `reached`), 03 (answer at 300 ms is `timed-out` at 250), 04 (answer exactly at 250 ms is `reached`), 05 (refusing app times out at 250), the Rust and node tests that name the cap, and `DESIGN_READY_WAIT.md` (sections 0, 4, 6, 8, 9, 10 and the new section 11). `quiet_ms` 30 and `hard_ms` 1500 are unchanged; vectors 12 to 15 do not depend on the cap. No README states 150.
- The installed extension is **not** reinstalled and keeps its default of 150. Every live run below passes the cap explicitly (`--wait-cap-ms 150` or `250`); the extension log of all 80 B and C runs shows `READY_WAIT_START ... cap=150` or `cap=250` as asked, so the installed extension honours the override (CONFIRMED). The new default itself has not run in a Shell (HYPOTHESIS that it behaves like `--wait-cap-ms 250`: same code path, `mergePolicy` with no `cap_ms`).

**2. Wire paint time in the harness (CONFIRMED, code, tests and live).**
- `trace/wire.rs` reads the fixture's protocol dump (`fixture-<pid>.log`): per `wl_surface`, the first `commit` that follows an `attach` of a buffer of a new size (sizes from `wl_shm_pool.create_buffer`). The dump's `[ms.us]` prefix is `CLOCK_REALTIME` microseconds modulo 2^32; the `TRACE_START` line gives one reading of both clocks, so each commit lands on `CLOCK_MONOTONIC` and goes through the **same** clock anchor and uncertainty as before. `detect_flash` takes the paints from the commits when there are any, else from the `FIRST_PAINT` stamps; verdict, visibility labels and `--strict-flash` follow. A dump that was asked for but has no commit leaves the stamps in use and adds a `flash_notes` line.
- Reported per run: `paint_source` (`wire` or `stamp`), `paint_size`, `paint_wire_ms` and `paint_stamp_ms` (ms after the reveal, middle of the anchor), `paint_stamp_lag_ms` (stamp minus commit) and `flash_by_stamp` (the verdict the stamp alone would have given). The event text says `(wire commit)`.
- Live check of the clock conversion: with no load the commit and the stamp of the same paint agree to 7 us in both smoke runs (`paint_stamp_lag_ms` 0.0074 and 0.0071), and the idle batch below gives a median lag of 0.01 ms, maximum 0.02 ms over 30 runs. A wrong offset would show as a constant shift; there is none.
- **Default on, `--no-wire-paint` opts out (CONFIRMED by the measurement below).** The dump writes about 37 KB per run into the run's private temporary directory (removed afterwards) and redirects the fixture's stderr into that file, so the fixture's own stderr messages are not shown while it is on.

| cost of the dump, same session, interleaved, rotated order, `ready-wait --wait-cap-ms 250`, `bottom-right-larger` | idle (no burn), 30 pairs, off / on | loaded (22 burn threads, load1 mean 25.3), 20 pairs, off / on |
| :--- | :---: | :---: |
| valid runs / outcome `reached` | 30 / 30, 30 / 30 | 20 / 20, 20 / 20 |
| run duration median / p95 / max ms | 122 / 142 / - , 122 / 162 / - | 464 / 595 / 632 , 455 / 533 / 557 |
| cloak ms median / p95 / max | 20.2 / 48.4 / 255.4 , 19.7 / 51.3 / 55.0 | 76.0 / 117.6 / 141.6 , 73.9 / 116.0 / 117.5 |
| ready-wait waited_ms median / p95 / max | - | 65.2 / 112.9 / 136.6 , 64.2 / 101.4 / 105.4 |
| clock anchor half-width, median ms | 1.98 , 2.04 | 12.7 , 12.0 |
| flash runs (off: on the stamp; on: on the wire time) | 0 , 0 | 3 , 0 (the stamp alone would have said 2) |
| stamp lag, on arm: median / p95 / max ms | 0.01 / - / 0.02 | 0 / 14.5 / 35.2 |

No measurable cost at these sizes (CONFIRMED for n = 30 and 20 pairs: the medians differ by 0.5 ms (idle cloak) and 9 ms (loaded duration); the p95 of run duration differs by 20 ms at idle (on is higher) and by 62 ms loaded (on is lower), so the sign of the p95 difference changes with the load). The largest idle cloak value, 255 ms, is a single run of the *off* arm (iteration 20, load1 1.3); the cause was not looked at. A cost smaller than the run-to-run spread would not be seen (not shown).

**3. Traced loaded same-session A/B/C (CONFIRMED).** 22 `xtask burn-cpu` threads for the whole phase after a 70 s warm-up (load1 19.9 at the start of the batch, 26.2 at the end), `bottom-right-smaller` and `bottom-right-larger`, 20 per arm per spec, 120 runs in one login with the arm order rotated every iteration (A B C, B C A, C A B, ...; the first arm of iterations 1 to 20 cycles A B C A B C ...). A = `--wait-strategy legacy`, B = `ready-wait --wait-cap-ms 150`, C = `ready-wait --wait-cap-ms 250`, all with the wire paint time on (then the opt-in flag `--wire-paint`, now the default). 120 of 120 runs valid (exit 0, no wait notes, the extension's own `READY_WAIT_OVERRIDE`/`START` lines show the arm's rule and cap), `paint_source` is `wire` in 120 of 120, load1 mean per arm 24.97 / 25.05 / 25.05. After the batch `pgrep -cx xtask` and `pgrep -cx fixture` both printed 0 (script, and again by hand), and `query windows` lists only the user's two windows.

| arm | spec | n | flash on the old stamp | flash on the wire time | stamp only | wire only |
| :--- | :--- | ---: | ---: | ---: | ---: | ---: |
| A legacy | bottom-right-larger | 20 | 19 | 19 | 0 | 0 |
| A legacy | bottom-right-smaller | 20 | 5 | 5 | 0 | 0 |
| B cap 150 | bottom-right-larger | 20 | 4 | 0 | 4 | 0 |
| B cap 150 | bottom-right-smaller | 20 | 0 | 0 | 0 | 0 |
| C cap 250 | bottom-right-larger | 20 | 4 | 0 | 4 | 0 |
| C cap 250 | bottom-right-smaller | 20 | 3 | 0 | 3 | 0 |
| **A / B / C total** | | 40 each | **24 / 4 / 7** | **24 / 0 / 0** | 0 / 4 / 7 | 0 / 0 / 0 |

Against A (Fisher two-sided): old stamp B 4/40 (-83.3%, p = 4.2e-6) and C 7/40 (-70.8%, p = 1.9e-4); wire time B 0/40 and C 0/40 (-100%, p = 7.7e-10). B against C is not different on either measure (4 against 7 on the stamp, 0 against 0 on the wire). The stamp made C look worse than B (7 against 4), which is the stamp lag, not the cap.

| arm | wait outcome | n | flash on the old stamp | flash on the wire time |
| :--- | :--- | ---: | ---: | ---: |
| A | legacy rule (no machine, no outcome) | 40 | 24 | 24 |
| B | reached | 40 | 4 | 0 |
| B | timed-out | 0 | - | - |
| C | reached | 40 | 7 | 0 |
| C | timed-out | 0 | - | - |

**This batch produced no timed-out run at either cap** (6.20: 5 of 50 B runs timed out in the user's loaded batch), so the class "timed-out" cannot be compared here. That is weak evidence that the cap-150 timeouts are gone, not proof: 0 of 40 here against 5 of 50 in 6.20 is Fisher two-sided p = 0.063 (CONFIRMED, computed), and about 4 timeouts would be expected in 40 at the 6.20 rate. Different load, a different day's login state, and the 6.20 user batch also had the `top-left-smaller` spec.

**Does the wire time hide real late paints? Not in A.** The legacy arm, whose flashes are real (the reveal happens before the app has resized, 6.20 class d), gives 24 flash runs on the wire time and the same 24 on the stamp; stamp lag in A is median 0, p95 7.8, maximum 11.6 ms, at or above 10 ms in 1 of 40 runs.

| arm | anchor half-width median / max ms | wire commit of the final size against the reveal: median / min / max ms | commit after the reveal (middle estimate) | commit inside the anchor (could be after) | commit 5 ms or more before | stamp lag median / p95 / max ms | lag 10 ms or more |
| :--- | :---: | :---: | ---: | ---: | ---: | :---: | ---: |
| A | 12.8 / 17.4 | 21.6 / -22.7 / 84.6 | 24 | 33 | 15 | 0 / 7.8 / 11.6 | 1 |
| B | 11.9 / 16.6 | -6.4 / -35.1 / 1.2 | 4 | 25 | 23 | 0 / 15.5 / 48.1 | 9 |
| C | 13.0 / 17.5 | -4.9 / -24.4 / 6.1 | 10 | 34 | 17 | 0 / 16.4 / 19.5 | 11 |

**Reading it honestly.**
- **CONFIRMED:** all 11 runs that the old stamp called a flash in B and C were `reached` runs whose stamp came 10.3 to 48.1 ms after the commit of the same buffer; no wire verdict is a flash. This is the 6.20 finding (stamp lag, not a late app) reproduced with the harness itself, 11 of 11 times, with the cause visible in every run's JSON.
- **Limit (CONFIRMED):** the flash verdict needs the lead to exceed the clock anchor. Under this load the harness's anchor (4 bracketed `SetLogging` calls per run) is wide: half-width 12 to 13 ms (17.5 at most), against 2 ms at idle. So "0 wire flashes" means *no commit is provably after the reveal*; it is not proof that every commit was before it. Ten C runs and four B runs have the commit after the reveal by the middle estimate (at most 6.1 and 1.2 ms), all inside the anchor, so they are neither flashes nor shown to be clean. B has 25 and C 34 runs of 40 with the commit inside the anchor. The 11 stamp-only runs have their commit at -5.5 to +6.1 ms against anchors of 6 to 15.7 ms. What 6.20 could prove (72 of 72 traced `reached` runs with the commit at least 3.3 ms before the reveal) needed a 120-call calibration of +-3 ms; the harness's per-run anchor cannot do that under load.
- **HYPOTHESIS (not tested):** more bracketed calls per run, or keeping only the narrowest brackets, would narrow the anchor under load (6.20: 120 calls gave +-3 ms; the harness uses `ANCHOR_SAMPLES` = 4). Not changed here.
- **Also unproven:** libwayland stamps a request when it is marshalled, before the connection is flushed. If the fixture thread is descheduled between marshal and flush, the compositor gets the commit later than the wire time says, so the wire time is the earliest the commit could have arrived. How large that gap is under this load was not measured (the compositor-side receive time is not in any log read here). The wire time therefore removes the stamp's lag but can still be a little early; it is the better lower bound, not the exact time.

| the 11 runs the old stamp called a flash and the wire time did not | wire commit, ms after the reveal | stamp, ms after the reveal | stamp lag ms | anchor half-width ms |
| :--- | ---: | ---: | ---: | ---: |
| B larger #14 | -5.5 | 8.3 | 13.8 | 6.0 |
| B larger #1 | 0.1 | 15.0 | 14.9 | 13.0 |
| B larger #3 | -1.3 | 46.8 | 48.1 | 13.6 |
| B larger #4 | -2.0 | 12.7 | 14.8 | 12.5 |
| C larger #11 | 2.5 | 18.1 | 15.6 | 12.5 |
| C larger #17 | 2.1 | 13.6 | 11.5 | 13.0 |
| C larger #4 | 2.2 | 14.0 | 11.8 | 12.4 |
| C larger #9 | 0.4 | 15.3 | 14.9 | 13.5 |
| C smaller #11 | 0.1 | 13.4 | 13.3 | 13.0 |
| C smaller #18 | -0.2 | 17.7 | 17.9 | 15.7 |
| C smaller #9 | 6.1 | 16.5 | 10.3 | 13.3 |

**Cap 150 against 250 when the app answers (CONFIRMED, loaded, 40 runs each, all `reached`):** cloak median / p95 / max 67.5 / 109.2 / 143.9 ms (B) against 68.3 / 96.7 / 106.6 ms (C); `waited_ms` median 55.4 against 57.2 ms. A longer cap costs nothing when the app answers inside it; its cost is only for an app that never answers (not exercised here). Arm A, for scale: cloak median 46.8 ms, p95 53.6 ms.

**Tests (CONFIRMED, run).** Per binary, `cargo test -p spawn-at-verify --features tui,fixture`: 201 lib (+12: 5 in `trace/wire.rs`, 6 in `runner/wire_paint_tests.rs`, 1 in `report/tests.rs`), 10 fixture binary, 0 main, 4 `category_library_test`, 7 `cli_exit_codes_test` (+1), 4 `fixture_test` (+ 4 ignored), 20 `flash_test`, 5 `oracle_cross_check`, 1 `preflight_tools_test`, 13 `render_test`, 21 `trace_analyser_test`, 6 `trace_state_test`, 6 `trace_window_scope_test`, 83 `tui_test` = **381 passed, 4 ignored**. Bare 265, `--features tui` 367. `spawn-at` 175, `spawn-at-core` 47, `xtask` 10, node 154 (all pass after the cap change; the vectors and wiring tests moved with it). Rust total with `tui,fixture` 613, with node 767 (counts table in 1.1 updated). No test needs a display; the wire tests use text fixtures.

**Still unshown.** On-screen visibility (no screen capture, no compositor frame time); hostile apps (hung, refusing, restless: no fixture flag, the cap-150 timeouts of 6.20 did not recur in the 40 B runs here, and the timed-out class is empty); X11 and XWayland (the dump is Wayland-only, so an X11 fixture would yield no commit and fall back to the stamp with a note: HYPOTHESIS, not run); a release-build fixture; real toolkit apps under the wire time; the new default cap in a reinstalled Shell; the compositor-side arrival time of a commit; a harness anchor narrow enough under load to prove commits earlier than the reveal.

**Disclosures.** The harness sends four bracketed `SetLogging(true)` calls per run (in-memory flag, as in every flash run); no gsettings or dconf key was written, nothing under `/sys` or `/proc` was written (the batch scripts read `/proc/loadavg`); no nested shell; the extension was not reinstalled. The fixture windows were opened and closed by the harness by id. Throwaway scripts (not committed): `scratch/w21_loaded.sh`, `w21_rows.jq`, `w21_rows.sh` and the existing `ab_batch.sh`, `ab_stats.jq`; raw JSONs are in the session scratchpad.

### 6.22 Hostile apps: fixture flags, nine catalog cells and a live A/C run (2026-10-13)

Labels: CONFIRMED = measured or read in this section; HYPOTHESIS = not tested. Fixture, harness and catalog only: the product (CLI, core, extension) is not changed; two findings about it are reported below with evidence and left alone. Installed extension: the previous build (ready-wait present, default cap 150, hard limit 1500), not reinstalled; the ready-wait arm passes `--wait-cap-ms 250` explicitly and the extension log shows `cap=250` in every C run. Commits `70dc120` (code, tests, cells), the follow-up fixture fix and this section.

**1. Fixture flags (harness only, `fixture` feature, never in the product binary).** Each decision is a pure function (`fixture/behaviour.rs`), tested without a display; the window loop carries it out.

| Flag | What it does on Wayland | Shown live (direct spawn, fixture event file) |
| :--- | :--- | :--- |
| `--resize-delay-ms N` | sleeps N ms before painting at a new size; winit has already acknowledged the configure, so the compositor waits for a commit that is N ms late | `RESIZE_DELAY 400` written 36 ms after map; the cells below wait 119 ms for N = 100 and hit the cap for N = 400 |
| `--min-size WxH`, `--max-size WxH` | xdg-toplevel `set_min_size` / `set_max_size`; the **compositor** enforces them, so "refusing" is a clamp of the request, not an app ignoring a configure | `refuse-grow`: request 800x600, window ends at 500x400 |
| `--fixed-size` | not resizable (winit sets min = max) | `non-resizable`: `Skipped(not-resizable)` |
| `--resize-increment WxH` | **cannot be a hint**: no Wayland protocol, winit ignores it. Emulated as a terminal does it: the app rounds its size down to the grid and sets its own window size | `SNAP 800x600->777x574` written; window ends at 777x574 |
| `--stall-after-map-ms N` | blocks the event loop N ms after the first paint (N under 4000, enforced, so GNOME's 5 s not-responding dialog is not reached) | `STALL_BEGIN` at 0.0 ms, `STALL_END` at 2500.1 ms, the queued `RESIZED` events after it |
| `--flap-ms T` | after the first resize, every 20 ms set the next of two alternating window sizes (both differ from the requested one) for T ms, then settle | `FLAP_TICK`s 17 to 25 ms apart (jitter stays under the 30 ms quiet window) |

**What did not work first, and why (CONFIRMED).** My first version of grid-snap and flap painted a smaller or larger *buffer* only. The smoke run showed `Reached(800x600)` at 16 ms for grid-snap: **winit calls `xdg_surface.set_window_geometry(configured size)` on every configure** (`winit-0.29.15 .../wayland/window/state.rs:676`), so the frame stays at the configured size whatever buffer is committed. The fixture now also calls `request_inner_size`, which updates that geometry for a floating window; both cells then behaved as designed. A clamp that is *not* known to the compositor (an app that silently ignores a configure) is not modelled by any flag; `hung` is the nearest case.

**2. The cells (`verify/hostile/`, category `hostile`, 9 specs; catalog 36 -> 45) and what each declares.** `[expect.resize]` is new: outcome, reason, `waited_ms` range, final size, read from the `resize` object of the CLI's `--json`; judged for ready-wait runs only (a legacy run reports no outcome and is not judged). The expectations were written from the design and the extension source **before** the first live run; nothing was changed afterwards except a "KNOWN MISMATCH" note in two descriptions.

| Cell | Fixture | Request | Declared outcome (cap 250) |
| :--- | :--- | :--- | :--- |
| slow-below-cap | `--resize-delay-ms 100` | 800x600 | Reached, at-target, 90 to 249 ms, 800x600 |
| slow-above-cap | `--resize-delay-ms 400` | 800x600 | TimedOut, cap, 240 to 330 ms, window still 400x300 |
| refuse-grow | `--max-size 500x400` | 800x600 | Adjusted, quiet, under 250 ms, 500x400 |
| clamp-to-min | `--min-size 300x200` | 200x150 | Reached, at-target, 300x200 (target raised to the minimum) |
| default-equals-min | `--min-size 400x300` (= start) | 200x150 | Reached at once (waited 0), 400x300 |
| grid-snap | `--resize-increment 37x41` | 800x600 | Adjusted, quiet, 777x574 |
| non-resizable | `--fixed-size` | 800x600 | Skipped, not-resizable, 400x300 |
| hung | `--stall-after-map-ms 2500` | 800x600 | TimedOut, cap, 240 to 330 ms, 400x300 (hard limit not reached) |
| restless | `--flap-ms 2000` | 800x600 | TimedOut, **hard**, 1490 to 1600 ms |

**3. Live run (CONFIRMED).** One login, idle (load1 1.5 to 1.2), `bottom-right` 16 px margin, 10 runs per cell per arm, arm order rotated per iteration by `ab_batch.sh` (A B, B A, ...), 190 runs. A = `--wait-strategy legacy`, C = `--wait-strategy ready-wait --wait-cap-ms 250`; plus **D = ready-wait with `--wait-cap-ms 1500` on `hung` only** (10 runs), my addition: with cap 250 a hung app never reaches the 1500 ms hard limit, and the hard limit had never run live. All 190 runs valid (the arm's rule and cap in the extension log, exit 0, no notes). The wire paint time is on (6.21): the flash counts on the wire time and on the old stamp are equal in every cell and arm.

| Cell | Arm | Verdict vs declared | Wait outcome (n=10) | Wait ms med (min-max) | Cloak ms med / p95 / max | Frame at reveal | Later size / flash (wire = stamp) |
| :--- | :-: | :-: | :--- | :--- | :--- | :--- | :--- |
| slow-below-cap | A | not judged | legacy SIZE_SETTLED x10 | 32.1 (31.7-33.9) | 47.2 / 50.8 / 50.8 | 400x300 x10 | 800x600 painted 70.9 ms after the reveal (max 85.6); flash 10/10 |
| | C | pass 10 | Reached/at-target x10 | 119 (115-121) | 133.3 / 135.1 / 135.1 | 800x600 x10 | none; flash 0/10 |
| slow-above-cap | A | not judged | SIZE_SETTLED x10 | 32.2 (31.7-34.3) | 42.3 / 49.5 / 49.5 | 400x300 x10 | 800x600 375 ms after the reveal (max 384); flash 10/10 |
| | C | pass 10 | TimedOut/cap x10 | 250 (250-250) | 266.7 / 267.9 / 267.9 | 400x300 x10 | 800x600 151 ms after the reveal (max 154); flash 10/10 |
| refuse-grow | A | not judged | SIZE_SETTLED x10 | 43.7 (39.2-46) | 50.7 / 54.3 / 54.3 | 500x400 x10 | none; 0/10 |
| | C | pass 10 | Adjusted/quiet x10 | 41.5 (39-44) | 50.7 / 52.4 / 52.4 | 500x400 x10 | none; 0/10 |
| clamp-to-min | A | not judged | SIZE_SETTLED x10 | 40.3 (38.7-44.3) | 51.0 / 61.6 / 61.6 | 300x200 x10 | none; 0/10 |
| | C | **FAIL 10** | Adjusted/quiet x10 (declared Reached) | 39 (38-41) | 50.8 / 52.2 / 52.2 | 300x200 x10 | none; 0/10 |
| default-equals-min | A | not judged | SIZE_SETTLED x10 | 32.1 (31.8-33.3) | 35.4 / 50.9 / 50.9 | 400x300 x10 | none; 0/10 |
| | C | **FAIL 10** | TimedOut/cap x10 (declared Reached, waited 0) | 250 (250-250) | 267.8 / 268.4 / 268.4 | 400x300 x10 | none; 0/10 |
| grid-snap | A | not judged | SIZE_SETTLED x10 | 46.0 (44.4-52) | 50.7 / 66.5 / 66.5 | 777x574 x10 | none; 0/10 |
| | C | pass 10 | Adjusted/quiet x10 | 45 (44-47) | 51.7 / 65.5 / 65.5 | 777x574 x10 | none; 0/10 |
| non-resizable | A | not judged | SIZE_SETTLED x10 | 32.5 (31.3-34) | 49.7 / 51.0 / 51.0 | 400x300 x10 | none; 0/10 |
| | C | pass 10 | Skipped/not-resizable x10 | 0 (0-0) | 18.9 / 20.8 / 20.8 | 400x300 x10 | none; 0/10 |
| hung | A | not judged | SIZE_SETTLED x10 | 32.6 (31.9-35.2) | 47.8 / 51.2 / 51.2 | 400x300 x10 | not seen (see below); 0/10 |
| | C | pass 10 | TimedOut/cap x10 | 250 (250-250) | 266.0 / 268.0 / 268.0 | 400x300 x10 | not seen (see below); 0/10 |
| | D | (declared for cap 250: fails 10 by design) | TimedOut/**hard** x10 | 1500 (1500-1500) | 1516.2 / 1518.5 / 1518.5 | 400x300 x10 | 800x600 976 ms after the reveal (max 997.5), position corrected; flash 10/10 |
| restless | A | not judged | HARD_CAP x9, SIZE_SETTLED x1 (958.5 ms) | 1500.9 (958.5-1501.4) | 1532.1 / 1534.6 / 1534.6 | mid-flap: 784x584 x4, 760x560 x6 | window keeps changing (median 26 events per run); flash 10/10 |
| | C | pass 9, **FAIL 1** | TimedOut/hard x9, Adjusted/quiet x1 (322 ms, run 1) | 1500 (322-1500) | 1512.5 / 1533.7 / 1533.7 | mid-flap: 784x584 x7, 760x560 x3 | as A; flash 10/10 |

**Final size against the request.** Every cell's window ended at the size in the "frame at reveal" column except where a later size is listed: `slow-below-cap` and `slow-above-cap` reach 800x600 only after the reveal (A both, C only above the cap), `restless` never rests at 800x600 while it flaps. The requested sizes: 800x600 for all but `clamp-to-min` and `default-equals-min` (200x150).

**Hard limit, live for the first time (CONFIRMED).** `hung` with cap 1500 (D) and `restless` with cap 250 (C) both end in `TimedOut` with `waited_ms` exactly 1500 and `reason=hard` (with cap 1500 the report says `hard`, not `cap`). The window was revealed 1513 to 1518 ms after the size request in D and 1503 to 1534 ms in the nine restless C runs that hit it, i.e. 3 to 34 ms after the limit, at 400x300 (hung, the stalled app's old size) and at the flapping size 784x584 or 760x560 (restless), never at 800x600. A stuck or cloaked window did not occur: an `UNCLOAK` is in 190 of 190 runs, the longest cloak is 1534.6 ms against the 6000 ms watchdog, every run exited 0, and after each batch `query windows` listed only the user's two windows and `pgrep -cx fixture` printed 0.

**Judgment: every mismatch against its declared expectation.**

| # | Cell/arm | Declared | Seen | Cause and class |
| :-: | :--- | :--- | :--- | :--- |
| 1 | clamp-to-min C, 10 of 10 | Reached 300x200 | Adjusted/quiet 300x200 at 39 ms | **Product finding (CONFIRMED).** The extension logs `reportedMinSize=(0x0)` in all 20 clamp/default-equals-min C and A runs although the fixture declared 300x200 / 400x300, and the `Meta-14.typelib` of GNOME 46 has no min-size accessor (`strings` finds none; `_getMinSize` calls `window.get_min_size?.()` and falls back to 0x0). So `SET_SIZE_CLAMPED` cannot fire on GNOME 46 and the machine waits for the unraised 200x150. Effect here is benign: the first answer is the minimum, 39 ms. Not fixed. |
| 2 | default-equals-min C, 10 of 10 | Reached at once, waited 0 | TimedOut/cap at 250 ms, window unchanged | Same cause: the target stays 200x150, the app at its minimum never answers, the cap ends it. **Cost (CONFIRMED): cloak 267.8 ms against 35.4 ms for legacy, +232 ms for an app that cannot change**; this is design risk 3 / decision D2 (first-spawn cost for an app at its minimum) happening live, without the learned-minimum cache or probe. Not fixed. |
| 3 | restless C, 1 of 10 (run 1) | TimedOut/hard | Adjusted/quiet at 322 ms | **Fixture timing, not product.** One flap tick gap exceeded the 30 ms quiet window, so the quiet rule completed on the flapping size. The legacy arm had the same once (SIZE_SETTLED at 958.5 ms). HYPOTHESIS: first run after a rebuild; the direct trace showed tick gaps of 17 to 25 ms, so this is a rare jitter, 2 in 20. |
| 4 | hung D, 10 of 10 | (spec is for cap 250) | TimedOut/hard at 1500 | Not a mismatch: D is the hard-limit probe and was judged by the hard-limit paragraph above; the spec fails by design at another cap. |

No mismatch for slow-below-cap, slow-above-cap, refuse-grow, grid-snap, non-resizable, and hung (C): 60 of 60 runs match their declaration.

**What the cells show about the strategies (CONFIRMED, idle, debug fixture).**
- `slow-below-cap`: legacy reveals at 400x300 after a 32 ms quiet window and the app paints 71 ms later (flash 10 of 10); ready-wait waits 119 ms and flashes 0 of 10, at +86 ms cloak.
- `slow-above-cap`: both flash 10 of 10; ready-wait reveals 151 ms before the app paints instead of 375 ms.
- `hung`: both reveal at the old size (ready-wait at 266 ms, legacy at 48 ms). **The harness did not see the late resize in A and C**: the app wakes about 2.5 s after it maps, 2.2 s after those reveals, outside the 1500 ms post-reveal watch, so flash shows 0 of 10 there; only D, revealed at 1516 ms, sees it (976 ms after the reveal). That 0 is a limit of the watch, not a clean result.
- `non-resizable`: ready-wait is faster (18.9 ms against 49.7 ms cloak median).
- `refuse-grow` and `grid-snap`: both strategies reveal at the app's own size; ready-wait reports it (`Adjusted`) in about 40 ms.

**Tests (CONFIRMED, run).** Fixture binary 20 (was 10): 7 in `fixture/behaviour.rs` (size parsing, stall limit, snap, default plan, delay on a new size only, increment with `SNAP` event, flap sizes) and 3 flag-parsing tests. `spawn-at-verify` lib +6 in `runner/resize_expect_tests.rs` (spec parsing, printed JSON, match, each mismatch named, size tolerance, legacy not judged and a missing outcome fails); `category_library_test` +1 (catalog is 45 specs, the nine hostile cells with a declared outcome and one hostile flag each). Counts in 1.1: `tui,fixture` 402 passed, 4 ignored; total 675 Rust, 846 with node.

**Still unshown.** Load (everything here is idle; 6.19 and 6.20 showed the loaded cases differ); a release-build fixture; real hostile apps (a real hung or refusing Electron/GTK3/Qt app) and any app that ignores a configure without telling the compositor; whether GNOME shows a not-responding dialog for the 2500 ms stall (no screen capture; the stall is under the 5 s ping limit); on-screen visibility of any flash; X11 and XWayland (every cell requires Wayland); cap sweeps (only 250, and 1500 for `hung`); the legacy arm judged against an expectation (it reports no outcome); the late resize of a hung app in A and C (outside the 1500 ms watch); multi-monitor and fractional scaling; the cause of the one restless jitter. The two product findings above are open.

**Disclosures.** The harness sends four bracketed `SetLogging(true)` calls per run (in-memory flag); no gsettings or dconf key was written, nothing under `/sys` or `/proc` was written (the batch scripts read `/proc/loadavg`), no nested shell, the extension was not reinstalled. Windows were spawned by the harness (closed by id) and by four direct `spawn-at spawn` probes for the event check (each closed by id with `spawn-at close --id`). Throwaway scripts, not committed: `scratch/h22_batch.sh`, `h22_rows.jq`, `h22_rows.sh` (copies of the 6.19 to 6.21 ones); raw JSONs are in the session scratchpad.

### 6.23 Post-reinstall live check: default cap, A/D regression, XWayland signal study (2026-10-13)

Labels: CONFIRMED = measured or read in this section; HYPOTHESIS = not tested. No code is changed. Raw results are in `scratch/p23r/` (gitignored, on disk). A previous run of this check ended when GNOME Shell crashed (`libmutter:ERROR window.c:1518 meta_window_unmanage: assertion failed focus_window != window`, mutter 46.2, 20:28:39) after a real `gnome-text-editor` forced to X11 was closed while focused; the extension had only logged `REJECT_UNMATCHED` for it. **No real X11 or XWayland application is run in this section**; XWayland is the fixture with `--protocol x11` only. Nothing from the crashed run is reused: it was a different login (gnome-shell pid 116188; this one is 159346), and its scratch files are scripts and result folders without a record of which build or login made them, so every number below was measured again after the 20:29 login.

**Phase 1, preflight (CONFIRMED, all passed).**
- Session `wayland` (`XDG_SESSION_TYPE` and `loginctl`), `gnome-extensions info spawn-at@harsh.local`: Enabled, State ACTIVE. `ProtocolVersion` over D-Bus: 6.
- `diff -r assets/gnome/modern ~/.local/share/gnome-shell/extensions/spawn-at@harsh.local` is empty (11 files, including `ready_wait.js` and `x11_observe.js`). `target/debug/spawn-at` and `spawn-at-verify` are newer than every source file (`find -newer` lists nothing).
- Journal since the 20:29 login: no `[SpawnAt]` line and no line naming the extension with error or fail. Other lines in the same journal that are not from the extension: `clutter_actor_set_allocation_internal: ... isnan` (20:45:47) and three `meta_wayland_buffer_process_damage: assertion 'buffer->resource' failed` (20:52 to 20:56), both from `gnome-shell` itself.
- Default cap: a harness run with no override reports `wait_strategy ready-wait`, `wait_cap_ms 250`, and its log has `READY_WAIT_START ... strategy=ready-wait ... cap=250 quiet=30 hard=1500`. A direct `spawn-at spawn` of the fixture with `SPAWN_AT_CONFIG` pointing at a missing file logs `cap=250`, with no `READY_WAIT_OVERRIDE` line. `spawn-at config show` says `wait.cap_ms 250 default (not sent: the extension decides)`.
- Temporary config (`[wait] cap_ms = 400`): `config show` says `400 file`; the spawn logs `READY_WAIT_OVERRIDE ... cap_ms=400` then `READY_WAIT_START ... cap=400`. A harness run with that same `SPAWN_AT_CONFIG` in its environment still reports `wait_cap_ms 250` and `cap=250`: the harness ignores the user's config file.
- Disclosure: two `SetLogging(true)` / `SetLogging(false)` pairs by `gdbus` for the direct spawns; the fixture windows exited on their own (`--exit-after-ms`), `pgrep -cx fixture` printed 0.
- The journal check above was made before these preflight spawns; their `[spawn-at-time]` lines are normal logging, not errors.

**Phase 2, regression A (legacy by override) against D (no override, the new default), native Wayland fixture (CONFIRMED).** Specs `placement/bottom-right-smaller` (400x300 to 200x150) and `placement/bottom-right-larger`; arm order rotated every iteration (A,D then D,A, ...), one `run` per spec per arm, 1.5 s apart, same login, same session. Idle: 10 per arm per spec (40 runs). Loaded: 22 `xtask burn-cpu` threads for the whole phase after a 70 s warm-up (load average 19.65 after warm-up, 25.9 at the end), 20 per arm per spec (80 runs). Every run exited 0 and all 120 rows were valid (arm D: no `READY_WAIT_OVERRIDE` line, `READY_WAIT_START cap=250`; arm A: `READY_WAIT_OVERRIDE strategy=legacy`). The paint time of every verdict is the wire commit (`paint_source=wire`, 120 of 120). Flash is the numeric wire-time flash of 6.16 (reveal before the client's commit at the final size).

| Load | Arm | Spec | Runs | Wire-time flash runs | Cloak median / p95 / max (ms) | Outcome |
| :-- | :-: | :-- | --: | --: | :-- | :-- |
| idle | A | all | 20 | 0 | 19.6 / 34.9 / 51.1 | `AT_TARGET` 20 |
| idle | D | all | 20 | 0 | 19.5 / 35.0 / 49.1 | `reached/at-target` 20 |
| idle | A | smaller / larger | 10 / 10 | 0 / 0 | 19.0 / 51.1 / 51.1 and 20.1 / 34.9 / 34.9 | `AT_TARGET` |
| idle | D | smaller / larger | 10 / 10 | 0 / 0 | 19.3 / 35.0 / 35.0 and 19.8 / 49.1 / 49.1 | `reached/at-target` |
| loaded | A | all | 40 | **26** (51 events, 38 `certain`, 13 `possible`) | 45.3 / 53.0 / 70.2 | `AT_TARGET` 13, `SIZE_SETTLED` 27 |
| loaded | D | all | 40 | **0** | 66.8 / 104.7 / 116.2 | `reached/at-target` 40 |
| loaded | A | smaller / larger | 20 / 20 | 6 / 20 | 44.9 / 50.0 / 70.2 and 45.3 / 53.0 / 54.3 | |
| loaded | D | smaller / larger | 20 / 20 | 0 / 0 | 46.5 / 66.6 / 76.0 and 83.2 / 113.9 / 116.2 | |

- **Loaded flash: D 0 of 40 against A 26 of 40 runs** (Fisher two-sided p = 6e-11). Idle: 0 of 20 in both arms, so idle shows nothing to separate. For A the median lead of the flashes is 22.8 ms and the largest 68.2 ms; `ANCHOR_CORRECTION` appears in 26 runs of A (I did not check that they are the same 26) and in none of D. Every D run ended `reached` at the target; no D run hit the cap (cap 250 ms, the longest wait 111.6 ms).
- **Cost of D under load: cloak median 66.8 ms against 45.3 ms for A (+21.5 ms), p95 104.7 against 53.0, max 116.2 against 70.2.** Most of it is the larger window: 83.2 against 45.3 ms median. Idle there is no difference (19.5 against 19.6 ms).
- **No `X11_OBS` line appears in any of the 120 native Wayland runs** (`grep -c X11_OBS` over every `log_slice`: 0 idle, 0 loaded).
- Burn: started with PID 189009, stopped by that PID by the batch script; afterwards `pgrep -cx xtask` printed 0, `pgrep -cx fixture` printed 0 and `ps -p 189009` found nothing.
- **Process finding (CONFIRMED).** My first idle batch ran 40 runs that were all `Blocked by preflight failure: Stale binary: binary commit '6ebace0' does not match repository HEAD '1655116'`, because I had committed the phase 1 draft first. The binary embeds the commit it was built at, so after every docs commit `cargo build -p spawn-at` has to run before the next harness batch (it rebuilt in 1 s; the product code is unchanged). Those 40 blocked rows are kept in `scratch/p23r/idle_blocked_stale_binary/` and are not measurements; the idle numbers above are the rerun.

**Phase 3, XWayland signal study, fixture only (`--protocol x11`), `bottom-right-smaller` (400x300 to 200x150) and `bottom-right-larger` (to 800x600), 20 runs per spec idle and 20 per spec under the same 22-thread burn (load average 19.97 after the 70 s warm-up, 24.7 at the end). 80 runs, all exit 0; the fixture exits by itself (`--exit-after-ms 2500`), nothing was closed by hand; afterwards `pgrep -cx xtask` and `pgrep -cx fixture` printed 0 and the burn PID was gone.** No real X11 or XWayland application was started.

*Method (CONFIRMED).* The harness deletes the fixture's event files and its clock anchor is good to about 2 ms, which is the size of the effects here. So these runs are direct `spawn-at spawn --json` calls with the arguments the harness builds from the `xwayland-*` specs (no override, `SPAWN_AT_CONFIG` pointing at a missing file), `SPAWN_AT_FIXTURE_EVENT_DIR` set, and `SetLogging(true)` for the batch (false again at the end). The extension's `X11_OBS_*` `mono_ms` and the fixture's event nanoseconds are the same `CLOCK_MONOTONIC`, so no anchor is needed; every time below is milliseconds after the `set_size` mark (S). "Final paint" is the **last** `FIRST_PAINT` at the target size (what the harness uses): under load the fixture logged that size twice in 11 of 40 runs (once before its own 400x300 paint, once after). The first extraction of mine used the first one; I re-ran it with the last before writing this. The extension had `damaged` and `first-frame` available in 80 of 80 runs (`damaged=available first_frame=available`), 351 signal lines in all: 271 `damaged`, 80 `first-frame`, one `first-frame` per run, every watch ended `reason=reveal+500ms` (80 of 80).

| Median (min to max), ms after S unless noted | idle smaller | idle larger | loaded smaller | loaded larger |
| :-- | :-: | :-: | :-: | :-: |
| `first-frame` | 2.4 (1.9 to 5.2) | 2.3 (1.6 to 3.5) | 5.1 (3.3 to 8.1) | 6.0 (3.4 to 8.8) |
| `COMMIT_RESOLVED` (legacy rule, `AT_TARGET`) | 2.7 (2.2 to 5.7) | 2.6 (1.8 to 4.1) | 5.9 (3.7 to 9.9) | 6.7 (3.9 to 10.2) |
| Reveal (R) | 29.1 (16.3 to 34.0) | 30.6 (17.0 to 38.5) | 21.6 (14.3 to 36.5) | 21.4 (15.9 to 31.4) |
| Client's final `FIRST_PAINT` | 3.9 (3.5 to 7.2) | 19.9 (18.8 to 27.9) | 54.3 (11.6 to 172.5) | 166.4 (89.9 to 305.9) |
| **`damaged` events between S and R** (runs per count) | 1: 20 | 1: 20 | 0: 5, 1: 15 | 0: 10, 1: 10 |
| **`damaged` events after R, in the 500 ms tail** | 1: 20 | 1: 1, 2: 19 | 1: 2, 2: 2, 3: 5, 4: 8, 5: 3 | 3: 4, 4: 13, 5: 3 |
| First `damaged` after S | 5.3 (4.7 to 10.2) | 5.5 (4.7 to 8.5) | 11.6 (7.6 to 34.6) | 23.2 (11.1 to 38.2) |
| First `damaged` minus `COMMIT_RESOLVED` (runs that had one before R) | 2.5 (2.1 to 4.7), 20 | 2.9 (2.2 to 4.5), 20 | 4.5 (2.9 to 17.7), 15 | 12.2 (5.3 to 16.0), 10 |
| First `damaged` minus R (negative = before the reveal) | -23.3 (-28.8 to -8.7) | -26.0 (-32.5 to -11.7) | -7.3 (-20.0 to +12.8), after R in 5 | -5.2 (-15.2 to +20.4), after R in 10 |
| **First `damaged` minus the client's final `FIRST_PAINT`** (runs that had one before R) | +1.3 (+0.2 to +4.0), 20; before the paint in **0** | **-14.3 (-20.2 to -13.3), 20; before the paint in 20** | -45.2 (-160.8 to +4.9), 15; before the paint in **14** | **-202.9 (-293.5 to -105.9), 10; before the paint in 10** |
| Runs with at least one `damaged` before the client's final paint | 0 of 20 | 20 of 20 | 19 of 20 | 20 of 20 |
| First `damaged` at or after the final paint, minus that paint | 1.3 (0.2 to 4.0) | 13.5 (2.9 to 31.8) | 4.4 (0.2 to 20.3) | 4.8 (2.4 to 8.0) |
| Reveal before the client's final paint (legacy rule on XWayland) | 0 of 20 | 6 of 20 | 18 of 20 | 20 of 20 |
| Reveal minus final paint | +25.0 | +10.8 | -43.4 | -155.6 |

*Facts about the signals (CONFIRMED, from the 80 runs).*
1. **A `damaged` event does arrive before the client has painted at the final size**: in 59 of 80 runs there is at least one (idle larger 20 of 20; loaded larger 20 of 20; loaded smaller 19 of 20; idle smaller 0 of 20). In idle larger the first one comes 13.3 to 20.2 ms before the final paint, in loaded larger 106 to 293 ms before it.
2. **The first `damaged` does not depend on the client's painting.** Its time after S is the same in idle smaller and idle larger (median 5.3 and 5.5 ms) although the client's final paint is at 3.9 and 19.9 ms. Under load (the 25 runs that had a `damaged` before the reveal) the client's `RESIZED` at the final size comes from 10 ms before to 169 ms after the first `damaged`, while the first `damaged` itself stays within 7.6 to 38.2 ms of S. In all 271 `damaged` lines the frame rect is already the final position; in all 80 `first-frame` lines it is still the old one (100,140), and `first-frame` comes before the client's `RESIZED` and final paint in 80 of 80 runs, 0.2 to 0.7 ms before `COMMIT_RESOLVED` idle (up to 1.9 ms under load). HYPOTHESIS for the cause: the first `damaged` is the damage of the extension's own move to the final position (`UNCLOAK_4C positioned=`): it follows that log line by 1.6 to 3.9 ms idle (median 2.1 smaller, 2.4 larger) but by 1.5 to 28.7 ms under load, so the link is loose when the shell is busy. Not isolated by an experiment.
3. **No run had more than one `damaged` between S and R** (0 or 1 in 80 of 80), including the 14 idle larger runs in which the client had finished painting 3 to 19 ms before the reveal: in none of those did the paint's own damage arrive before the reveal (0 of 14). It arrived after it: first `damaged` after R at 0.9 to 20.7 ms (median 1.9) idle, 3.0 to 135.9 ms (median 14.8) loaded. In the loaded runs, where the client paints after the reveal in 38 of 40, the first `damaged` at or after the `FIRST_PAINT` stamp comes 0.2 to 20.3 ms after it (median 4.4 smaller, 4.8 larger). HYPOTHESIS (refuted in 6.26: a cloaked window does get `damaged`, but not the client's): a cloaked window gets no `damaged` for its repaint, only the reveal and later paints do. Not tested: it needs a run that keeps the cloak until the client has painted, which is a code change.
4. **`texture=(na)` in 351 of 351 lines** (idle 139, loaded 212). `actor=` equals the frame size and `buffer=` equals the frame rect in 351 of 351 lines: on XWayland the sizes the extension can read are the window manager's, not the client's, which is why `frame == target` is true at `COMMIT_RESOLVED` in every run.
5. **Stamp lag.** On X11 there is no protocol dump, so the fixture's `FIRST_PAINT` stamp cannot be checked against a wire commit (`paint_source` would be `stamp`; the harness smoke runs showed it). From the 120 native Wayland runs of phase 2, where it can be measured, the stamp is late by 0.00 to 0.01 ms idle (median 0.01) and by a median 0.03 ms under load with a 95th percentile of 5.1 ms (arm A) or 16.2 ms (arm D) and a maximum of 10.4 or 18.9 ms. HYPOTHESIS that the XWayland fixture's stamp lags the same way. Effect on the findings above: the "damaged before the paint" margins of idle larger (13 to 20 ms against 0.01 ms of lag) and of loaded larger (106 to 293 ms against at most about 19 ms) hold; of the 14 early cases in loaded smaller, 3 (margins 4.0, 12.0 and 16.1 ms) are within the largest lag seen (18.9 ms), 2 more (24.8 and 26.4 ms) are within 8 ms of it, and the other 9 (41 ms and more) are not.

**Candidate rules, judged on this data (CONFIRMED where counted, HYPOTHESIS where marked).**

| Candidate | Result | Verdict |
| :-- | :-- | :-- |
| (i) first `damaged` after `set_size` means "painted" | Idle larger: fires 13 to 20 ms **before** the paint, 20 of 20. Loaded: no `damaged` before the legacy reveal in 15 of 40 runs, and when there is one it is before the paint in 24 of 25 (by up to 293 ms). Idle smaller: after the paint by 0.2 to 4.0 ms in 20 of 20, but the same event arrives at the same time when the client needs 20 ms, so that is a coincidence of a fast shrink. | **Not reliable.** It marks the extension's own reposition, not the client. |
| (ii) wait for a second `damaged`, or `damaged` followed by a quiet period | A second `damaged` before the reveal occurred in 0 of 80 runs; the quiet-period variant fires after the first one and fails like (i). Holding the cloak for a `damaged` that, per fact 3, may not be delivered while cloaked would stall until the cap in every run. | **Not usable to gate the reveal on this evidence.** Whether the paint's `damaged` arrives while the window stays cloaked is the open question (fact 3). |
| (iii) `first-frame` | One per run, at S + 1.6 to 8.8 ms, 0.2 to 1.9 ms before `COMMIT_RESOLVED`, before the client's resize and paint in 80 of 80 | **Not a paint signal.** |
| (iv) texture size equals the target | **Could not be evaluated**: `texture=(na)` in 351 of 351 lines. `_x11ObserveHost.textureSize` calls `actor.get_texture?.()` and then `get_width` / `get_height` on the result, and every failure along that path (no method, a result without those methods, an exception) is swallowed into `null`, so the log cannot say which one happened. `strings` on the Meta-14 typelib of GNOME 46 finds `get_texture` and `meta_shaped_texture_get_texture` but cannot say which class owns the bare name. **Correction (CONFIRMED later by `gjs` introspection, DESIGN_READY_WAIT section 12): `Meta.WindowActor.get_texture()` exists and returns a `Meta.ShapedTexture`, which has no `get_width`; the size is on its `get_texture()`, a `Meta.MultiTexture`. The reader is fixed in the extension source but not yet installed or run.** The text that follows is the original reasoning, kept as written. HYPOTHESIS, not run in a Shell: the shaped texture is a child of the window actor (`actor.get_first_child()` is the surface actor) and the surface actor's `get_texture()` returns it. Accessors to try, in this order: `surface.get_texture()` then `get_width()` / `get_height()` on it; `get_texture()` of that shaped texture (a `Cogl.Texture`) with its `get_width()` / `get_height()`. Each must be guarded the way the current one is. On XWayland the surface texture is the committed buffer, so it is the only candidate with a chance to follow the client. | **Open.** |
| (v) `get_buffer_rect()` or the actor size equals the target | Equal to the frame in 351 of 351 lines | **Not a paint signal** (it is the window manager's rectangle). |

**Is any candidate reliable enough to build on? No.** (i), (iii) and (v) fire before the client has painted, (ii) cannot be observed before the reveal, and (iv) was not measurable. What the data does support: the paint's own `damaged` arrives 0.2 to 20 ms after the client's final paint (median about 4.5 ms) whenever the window is visible, so a post-reveal check could confirm a paint but not prevent a flash. The next experiment, which is a code change and was not made here, is (iv) with a corrected accessor, logged in the same format, plus a run that holds the cloak to see whether a cloaked window gets `damaged` for its repaint.

*Consequences for the product, unchanged by this phase.* XWayland and X11 windows keep the legacy rule. With it the fixture was revealed before its final paint in 6 of 20 idle larger runs, 18 of 20 loaded smaller and 20 of 20 loaded larger, by a median of 155.6 ms in loaded larger (stamp-based; the lag bound above is at most about 19 ms).

*Disclosures.* Each of the two 40-run batches sent one `SetLogging(true)` and one `SetLogging(false)` by `gdbus` (in-memory flag); before them, two harness smoke runs and one direct probe of the XWayland fixture were made to check the log format (their fixture windows exited by themselves or were closed by the harness by id); no gsettings or dconf key was written, nothing under `/sys` or `/proc` was written (the scripts read `/proc/loadavg`), no nested shell, no code changed. Throwaway scripts, not committed: `scratch/p23_loaded.sh`, `p23_stats.sh`, `p23_stats.jq`, `p23_x11_batch.sh`, `p23_x11_loaded.sh`, `p23_x11_rows.sh`, `p23_x11_d1.sh`; raw results in `scratch/p23r/` (`idle`, `loaded`, `x11idle`, `x11loaded`, the per-run TSVs and `x11_summary.txt`).

**Phase 4, cleanup, counts, and what this section does not cover.**

*Cleanup (CONFIRMED, checked after the last batch).* `pgrep -cx xtask` 0 and `pgrep -cx fixture` 0; the two burn PIDs (189009 in phase 2, 215234 in phase 3) are gone (`ps -p` finds nothing); `spawn-at query windows` lists only the user's five windows (Claude, kitty, Antigravity, two GNOME terminals). Every fixture window of this section was either closed by the harness by id or exited by itself; no other window was closed. The `SetLogging` flag was set back to false by each script's exit trap. GNOME Shell did not crash during this section (same `gnome-shell` pid 159346 throughout; the journal's only gnome-shell assertion lines since the login are the four listed in phase 1, all before 20:59, the time of the first spawn of this section).

*Test counts (CONFIRMED, run, no code changed, so unchanged from 6.22).* `cargo test -p spawn-at-verify --features tui,fixture`: 211 lib, 20 fixture binary, 0 main, 5 `category_library_test`, 7 `cli_exit_codes_test`, 4 `fixture_test` (4 ignored), 20 `flash_test`, 5 `oracle_cross_check`, 1 `preflight_tools_test`, 13 `render_test`, 21 `trace_analyser_test`, 6 `trace_state_test`, 6 `trace_window_scope_test`, 83 `tui_test` = **402 passed, 4 ignored**. `cargo test -p spawn-at -p spawn-at-core -p xtask`: `spawn-at` 199 (185 + 10 + 4), `spawn-at-core` 64 (60 + 2 + 2), `xtask` 10 (8 + 2) = 273. Rust total 675, with `node --test tests/*.js` (171 pass, 0 fail) 846. No bare `cargo test --workspace` number is quoted for `spawn-at-verify`.

*Result in one place.* (1) Preflight passed: protocol 6, installed files identical to `assets/gnome/modern`, default cap 250 in the log and in `config show`, a `cap_ms = 400` file is honoured by a direct spawn and ignored by the harness. (2) Native Wayland fixture, same login: idle 0 of 20 wire-time flashes in both arms; loaded D (new default) 0 of 40 against A (legacy) 26 of 40, at the price of +21.5 ms median cloak (66.8 against 45.3 ms; p95 104.7 against 53.0 ms); no `X11_OBS` line in any of the 120 native runs. (3) XWayland fixture: `damaged` exists, but the first one comes from the shell (about 5 ms after `set_size`, after the extension's own move), not from the client; no candidate built on `damaged`, `first-frame` or the readable sizes is reliable; the texture candidate was not measurable (`texture=(na)`).

*Not covered by this section.* Native X11 (a session with `XDG_SESSION_TYPE=x11`); **any real X11 or XWayland application** (deliberately none after the earlier crash: the only XWayland client was the fixture, which is cooperative, repaints at once and exits cleanly, so its signal timing may differ from GTK, Qt, Electron or Xlib clients that resize late or never); Qt and Electron apps of any kind, native or XWayland; GTK apps; screen capture or any measurement of what is visible on the screen (a "flash" here is the reveal coming before the client's commit or stamp, not a recorded frame); the texture-size candidate; whether a cloaked window gets `damaged` for its repaint (needs a code change); caps other than 250 (and the 400 of the config check); fractional scaling and more than one monitor; a release-build fixture; the cause of the GNOME Shell crash of the earlier run (the extension only logged `REJECT_UNMATCHED` for that window; whether closing a focused XWayland window crashes mutter 46.2 regardless of this extension was not tested and must not be tried on a real application without the user present).

### 6.24 Texture reader live check and texture size as a signal (2026-10-13)

Labels: CONFIRMED = measured or read in this section; HYPOTHESIS = not tested. No code changed. Fixture only (`--protocol x11` for XWayland); no real X11 or XWayland application. Raw results in `scratch/p24r/` (gitignored).

**Phase 1, preflight (CONFIRMED, all passed).** After the reinstall and a new login (gnome-shell pid 245567, started 22:05:22): session `wayland`; `spawn-at@harsh.local` Enabled, ACTIVE; `ProtocolVersion` 6; `diff -r assets/gnome/modern ~/.local/share/gnome-shell/extensions/spawn-at@harsh.local` is empty and the installed `x11_observe.js` contains `readTextureSize`; no `SpawnAt`, `spawn-at`, `JS ERROR`, assertion or critical line from gnome-shell in the journal since that login; `~/.config/autostart/spawn-at-update-check.desktop` now has `Exec=/home/harsh/.local/bin/spawn-at update --check --headless` (absolute path, written by the installer). `target/debug/spawn-at` was built at `64faab9`, the commit of this tree. No `xtask` or `fixture` process was running.

**Phase 2, does the new reader work? (CONFIRMED: yes.)** Direct `spawn-at spawn --json` of the fixture as in 6.23 (`--exit-after-ms 2500`, `SPAWN_AT_FIXTURE_EVENT_DIR` set, `SPAWN_AT_CONFIG` a missing file, `SetLogging(true)` for the batch only): 5 iterations of `smaller` and `larger` = 10 runs with `--protocol x11` and 10 with `--protocol wayland`, all exit 0.
- XWayland: 10 `X11_OBS_START`, 35 `X11_OBS_SIGNAL` lines (10 `first-frame`, 25 `damaged`). **Every line has `texture_via=shaped>multi`**, 35 of 35; the second accessor (`shaped>multi>plane0`) was never needed and **no `texture_err` appears**. `texture=(na)` appears in 0 lines (it was 351 of 351 in 6.23). Texture values: `first-frame` 400x300 in 10 of 10 (the client's initial buffer, while the frame is already at the target), `damaged` 200x150 in 10 and 800x600 in 15.
- Native Wayland: 10 windows found (`window_id` set in 10 of 10), **0 `X11_OBS` lines** in the journal for the batch.
- **A first attempt failed for a reason in my shell, not in the reader (CONFIRMED).** All 10 XWayland spawns timed out (`window_id` null, "did not appear within 2s") with 0 `X11_OBS` lines. My shell's `XAUTHORITY` (`.mutter-Xwaylandauth.UOHTW3`) was stale after the new login; the live Xwayland was started with `.X0APW3`, and the fixture printed `Authorization required, but no authorization protocol specified`. I exported the live file name for the rerun (an environment variable of the child processes, nothing written). The 10 failed rows are kept in `scratch/p24r/p2x11_failed_stale_xauthority/` and are not data.

**Phase 3, is the texture size a usable signal? (CONFIRMED data, verdict: not usable.)** The 6.23 method: direct spawns, `--protocol x11`, `SPAWN_AT_FIXTURE_EVENT_DIR`, 20 runs per spec idle (80 runs of `smaller` and `larger` in total are 40 idle + 40 loaded), then the same under 22 `xtask burn-cpu` threads after a 70 s warm-up (load average 19.5, 24.8 at the end). All 80 runs exit 0, all windows found, 80 `X11_OBS_START`. Times are ms after the `set_size` mark S. "Texture match" is the first `X11_OBS_SIGNAL` (either signal) whose `texture=` equals the target size (200x150 or 800x600); "final paint" is the last fixture `FIRST_PAINT` at the target size, as in 6.23. Early = match before the final paint; late = at or after it. The log is only sampled at signals, so a match is seen at the first signal after it happens.

| Cell (20 runs) | Early / late / never | Match before the legacy reveal / after / never | First match, median (min to max) | Final paint, median (min to max) | Gap match minus paint, median (min to max) | Match minus reveal, median |
| :-- | :-: | :-: | :-- | :-- | :-- | :-: |
| idle smaller | 2 / 18 / 0 | 20 / 0 / 0 | 6.0 (3.5 to 10.2) | 4.2 (3.3 to 9.4) | +1.2 (-1.6 to +5.2); early runs -1.6 (-1.6 to -0.2), late runs +1.2 (+0.1 to +5.2) | -23.1 |
| idle larger | **20** / 0 / 0 | 20 / 0 / 0 | 5.8 (4.6 to 10.5) | 21.6 (19.1 to 29.6) | **-15.6 (-23.4 to -13.0)** | -23.9 |
| loaded smaller | **17** / 3 / 0 | 9 / 11 / 0 | 22.4 (7.1 to 67.3) | 73.6 (12.2 to 278.1) | -48.6 (-232.9 to +17.4); early runs -54.0, late runs +17.2 (+3.1 to +17.4) | +8.2 |
| loaded larger | **20** / 0 / 0 | 4 / 16 / 0 | 25.9 (10.1 to 49.8) | 188.1 (67.5 to 408.7) | **-171.0 (-358.9 to -57.4)** | +9.6 |

*Does the texture size follow the client's buffer or only the frame? (CONFIRMED: it does not follow the client's painting.)*
- In all 267 `damaged` lines (40, 60, 77 and 90 per cell) the texture equals the **target** size. In all 80 `first-frame` lines it is 400x300, the fixture's initial size, while the frame is already at the target. So the texture goes from the old size to the target at the first `damaged`, which is the shell-side event of 6.23 (about 5 ms after `set_size` idle), and stays there.
- That moment does not depend on when the client paints: idle larger, match at 5.8 ms and final paint at 21.6 ms; loaded larger, match at 25.9 ms and paint at 188.1 ms (up to 359 ms later). The texture equalled the size of the latest `FIRST_PAINT` before the signal in only 60 of 80 (idle larger), 48 of 110 (loaded larger) and 50 of 97 (loaded smaller) signal lines, and the frame size in 60, 90 and 77. In 20 of 20 idle larger and 20 of 20 loaded larger runs the texture had the target size before the client had painted at it.
- HYPOTHESIS for the mechanism, not tested: the surface the shell reads is resized when the X window is configured (the window manager's size), not when the client's next buffer is committed. The data only shows that its timing is that of the shell, not of the client.

*What the rule "reveal when texture size == target" would have done (CONFIRMED on these runs; "flash" means revealed before the client's final paint, by the fixture's stamp, whose lag is at most about 19 ms under load per 6.23).*
- It would have revealed before the client painted in **59 of 80 runs** (idle smaller 2, idle larger 20, loaded smaller 17, loaded larger 20), by a median 15.6 ms (idle larger) and 171.0 ms (loaded larger). The 2 idle smaller cases are 0.2 and 1.6 ms early, within what stamp lag could explain, so only 57 are clearly early; the clearly early ones are 13 ms or more (idle larger) up to 359 ms.
- It would not have waited unnecessarily in the sense of staying cloaked after the client painted: the late cases are 21 runs with a median gap of +1.2 ms idle (max +5.2) and +17.2 ms loaded (max +17.4), which is no more than the signal's own sampling delay. The failure is the other direction.
- Compared with the legacy rule it would have revealed 10 to 35 ms earlier than legacy in all 40 idle runs, and its flashes would be 2 of 20 (idle smaller) and 20 of 20 (idle larger), against 0 and 6 of 20 for the legacy rule in 6.23 (the reveal-before-paint counts of that section).

**Verdict: not usable.** The texture size reaches the target at the first `damaged` in 80 of 80 runs, before the client has painted in 59 of them, so "texture == target" does not say "the client painted at the target". It is the same signal as the frame size and as the first `damaged` of 6.23. Still open from 6.23: whether a cloaked window gets `damaged` for its repaint; that and a client-side signal are the remaining leads (HYPOTHESIS: the X sync counter `_NET_WM_SYNC_REQUEST_COUNTER`, which GTK and Qt clients use and winit may not; not looked at here).

**Phase 4, cleanup and counts (CONFIRMED).** After the last batch: `pgrep -cx xtask` 0, `pgrep -cx fixture` 0, burn PID 253639 gone, `spawn-at query windows` lists only the user's two windows (Claude, kitty). All fixture windows exited by themselves (`--exit-after-ms 2500`); nothing was closed by hand. No code changed, no tests added: `spawn-at` 202 (188 + 10 + 4), `spawn-at-core` 64, `xtask` 10, `spawn-at-verify` with `tui,fixture` 402 passed and 4 ignored, node 176; Rust total 678, with node 854, all unchanged from the counts of the previous step.

**Not covered.** Any real X11 or XWayland application, native X11, Qt, GTK and Electron clients (the fixture is a cooperative winit client that repaints at once); screen capture; whether a cloaked window gets `damaged` for its repaint; the texture size of a client that resizes its buffer late or never (the `hostile/` flags exist but were not run on `--protocol x11`); multi-monitor and fractional scaling. Disclosures: `SetLogging` true and false once per batch (in-memory); `XAUTHORITY` exported in my shell for the child processes; no gsettings or dconf write, nothing under `/sys` or `/proc` written, no nested shell. Throwaway scripts (not committed): `scratch/p24_batch.sh`, `p24_loaded.sh`, `p24_rows.sh`; raw results in `scratch/p24r/`.

### 6.25 X11 investigation, stage 1: capture rig, hold-the-cloak experiment, ping (2026-10-14)

> Experiment code removed; see branch `archive/x11-experiments`.

Labels: CONFIRMED = measured or read in this section; HYPOTHESIS = not tested. Fixture only; nothing installed; no default changed. Raw results in `scratch/p26/` (gitignored). Baseline: branch `refactor/v0.2.0-beta.1`, HEAD `b09e27f`, tree clean, extension ACTIVE, installed files identical to `assets/gnome/modern`; counts at the start (from the previous run, code unchanged since): `spawn-at` 205, `spawn-at-core` 64, `xtask` 10, `spawn-at-verify` (`tui,fixture`) 402 + 4 ignored, node 176.

**Phase 1a, is a window ping observable? (CONFIRMED, `gjs` with the Meta-14 typelib, outside the Shell.)**
- `Meta.Window.check_alive(timestamp: guint32) -> void` exists; `Meta.Window` has the read-only boolean property `is-alive`; `Meta.prefs_get_check_alive_timeout()` returns 5000 ms here. `Meta.Window` also has `can_close`, `delete`, `kill`; `Meta.Display` has `close`. The typelib strings show `META_DEBUG_PING` and `check_alive_timeout`.
- **No signal or property reports a pong or its time.** `Meta.Window` has 10 signals (`workspace-changed, focus, raised, unmanaging, unmanaged, position-changed, monitor-changed, shown, size-changed, highest-scale-monitor-changed`); none concerns a ping. `Meta.Display` has 29, none concerns a ping. The only observable is `notify::is-alive`, which changes only when a window fails to answer within the 5000 ms timeout (HYPOTHESIS about when mutter flips it; read from the property's existence, not run). A reply that arrives in time leaves no trace the extension can read, and `check_alive` is an action that makes mutter send a ping (and may bring up the "not responding" dialog for a hung app), not an observation.
- Verdict: **no usable ping/pong API.** Nothing is added for it.

**Phase 1b, capture options (introspection and harmless calls only).**

| Method | What it is | Prompt-free? | Result here |
| :-- | :-- | :-- | :-- |
| `org.gnome.Shell.Screenshot` | one still image per call | no: **AccessDenied** ("Screenshot is not allowed"), CONFIRMED by one call that wrote nothing | unusable; one frame per call could not see a 16 ms flash anyway |
| `org.freedesktop.portal.ScreenCast` (v5) and `.Screenshot` | portal; `CreateSession`, `SelectSources`, `Start`, `OpenPipeWireRemote` | **no**: the portal asks the user to choose what to share (HYPOTHESIS: standard behaviour, deliberately not called) | skipped by the no-prompt rule |
| `org.gnome.Mutter.ScreenCast` (v4) | mutter's own API: `CreateSession`, `RecordMonitor`, `Start`, signal `PipeWireStreamAdded(node)` | **yes**: session, stream and a PipeWire node (`gnome-shell`, `Stream/Output/Video`) were created and stopped with no dialog on the D-Bus path (CONFIRMED; I cannot see the screen, so a panel recording indicator is possible) | stream exists; **no consumer reached a frame** (below) |
| Consumers present locally | `pw-cli`, `pw-dump`, `pw-record` (audio), `gst-launch-1.0` 1.24.2 with `pipewiresrc`, `ffmpeg`, `gnome-screenshot`, ImageMagick `import`, `grim`; gcc and the libpipewire 1.0.5 headers | n/a | `grim` is a wlroots-protocol tool (HYPOTHESIS: not supported by mutter, not run); `gnome-screenshot` and `import` are still-image tools |

**The Mutter stream could not be consumed (CONFIRMED, 3 attempts, then stopped as agreed).** With a `gjs` holder keeping the session alive, `gst-launch-1.0 pipewiresrc` with `path=<node id>`, `path=<object.serial>` and `target-object=<object.serial>` each ended with `stream error: target not found` (the stream goes `connecting`, `paused`, then `error`, 2 ms later). The node is visible to `pw-dump` while the session lives. The portal's own route passes a restricted PipeWire connection (`OpenPipeWireRemote`), which needs the prompt. HYPOTHESIS: the session manager does not link a client on the default connection to this node; a native `pw_stream` client would get the same answer. Not tried (same server-side lookup).

**Phase 1c, decision: no prompt-free capture method reached any frame, so phase 2 (capture rig, fixture colour option, calibration against wire time and stamp time) is skipped.** The frame interval and jitter of a capture were therefore never measured. What would unblock it: either a consumer that can attach to the Mutter node on the default connection (unknown), or accepting one portal prompt (not done, by rule).

**Phase 2, capture rig: skipped** (see 1c). Not built, not measured: the fixture colour option, the frame classifier, the loaded A against D calibration, and the comparison of capture-visible flashes with the wire-time and stamp-time verdicts. Whether the capture agrees with either is therefore unknown (HYPOTHESIS only). No burn threads or fixtures were started in this section (`pgrep -cx xtask` and `-cx fixture` print 0).

**Phase 3a, experiment built, NOT installed, NOT run in a Shell: `x11-hold-damage` (CONFIRMED by node and Rust tests only).**
- Selection: the hidden `--wait-strategy x11-hold-damage` flag (and `--wait-cap-ms` for its cap). The config file and `spawn-at config set` refuse the value (`wait.strategy must be auto, legacy or ready-wait`); `Auto` stays the default; the instruction carries the value only when asked for by name (test: `{"WaitForCommit":{"timeout_ms":300,"strategy":"x11-hold-damage"}}` against `{"WaitForCommit":{"timeout_ms":300}}`).
- Rule, for an XWayland or X11 window with a size request only (new module `x11_hold.js`): the window stays cloaked after `set_size`; the first `damaged` is ignored; the second ends the wait as `reached` with `reason=x11-second-damaged`; if none comes, the cap (`cap_ms`, default 250) ends it as `timed-out` with `reason=x11-cap`, or `x11-hard` when the cap is 1500 (a larger cap is clamped to the hard limit). A window that goes away ends it as `skipped`/`x11-unmanaged`; a missing `damaged` signal ends it at once as `skipped`/`x11-no-damaged-signal`. The log lines are the existing ones (`READY_WAIT_START`, `COMMIT_RESOLVED`, `READY_WAIT_RESOLVED`) with `strategy=x11-hold-damage`, plus `X11_HOLD_DAMAGED n=<k> ignored=<bool>`; the `ResizeOutcome` signal carries the same label as its reason.
- Unaffected, by test: a native Wayland window given the value gets the same log lines as with no override apart from the `READY_WAIT_OVERRIDE` note (`unused=strategy(x11-hold-damage:not-x11)`), the same signals and the same outcome; an X11 spawn without the value, and an X11 spawn with it but no size request, take the original rule with no `X11_HOLD` line; the existing default-path wiring tests pass unchanged. Not byte-checked against a live Shell.
- Tests added: 8 in `tests/x11_hold_test.js` (second damaged, cap, caller cap, hard clamp, unmanaged, missing signal, late damaged, label and reason mapping), 4 in `tests/ready_wait_wiring_test.js` (X11 reached on the second damaged, cap, native unaffected, no size request and default), 1 in `wait_override.rs` plus the round-trip list, 1 in `settings/tests.rs` assertion, 1 in `mechanics.rs`. Open question the live run answers: whether the paint's `damaged` arrives while the window stays cloaked (HYPOTHESIS: 6.23 saw none before the reveal, but the cloak was lifted at 17 to 38 ms there).

**Phase 3b, ping/pong logging: nothing added** (1a: no signal or property carries a reply or its time; `check_alive` is an action).

**Phase 3c, counts (CONFIRMED, run).** `spawn-at` 206 (was 205; +1 `mechanics.rs`), `spawn-at-core` 65 (+1 `wait_override.rs`; the `settings` test gained an assertion), `xtask` 10, `spawn-at-verify` (`tui,fixture`) 402 passed and 4 ignored, node 188 (was 176; +8 `x11_hold_test.js`, +4 wiring). Rust total 683, with node 871; `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean. Counts table in 1.1 updated.

**Phase 3d, to install for the live stage (not done).** Files: `assets/gnome/modern/x11_hold.js` (new), `extension.js`, `commit.js`, `validator.js`, all embedded in the rebuilt `spawn-at` binary together with the Rust change (`wait_override.rs`, `settings.rs`, `mechanics.rs`, `mod.rs` asset list): run `cargo build -p spawn-at`, then `spawn-at install`, then **log out and back in** (Wayland) so GNOME loads the extension. The installed `x11_observe.js` and the other files are unchanged.

**Skipped and why.** Phase 2 (no prompt-free consumer for the Mutter screencast; portal needs a prompt). Ping logging (no API). No live run of the experiment (rule: build only).

### 6.26 x11-hold-damage, live A/B on the XWayland fixture (2026-10-14)

> Experiment code removed; see branch `archive/x11-experiments`.

Labels: CONFIRMED = measured or read in this section; HYPOTHESIS = not tested. Fixture only (`--protocol x11`); no real X11 or XWayland application. Defaults unchanged. Raw results in `scratch/p27r/` (gitignored).

**Phase 1, preflight (CONFIRMED).** Session `wayland`; `spawn-at@harsh.local` Enabled, ACTIVE; protocol 6; `diff -r assets/gnome/modern ~/.local/share/gnome-shell/extensions/spawn-at@harsh.local` empty and `x11_hold.js` installed; new login (gnome-shell pid 362194, started 01:40:17); no `SpawnAt`, `spawn-at`, `JS ERROR` or critical line from gnome-shell since then; `target/debug/spawn-at` built at the HEAD of the previous step. No `xtask` or `fixture` process running. My shell's `XAUTHORITY` was stale again after the login (`.UOHTW3`, live `.7MHVW3`), so the live file is exported for the child processes (an environment variable only).

**Offline check of the harness against the new labels (CONFIRMED, read and tested).**
- `ResizeReport.reason` is a free `String` (`resize_report.rs`): `x11-second-damaged`, `x11-cap`, `x11-hard`, `x11-unmanaged`, `x11-no-damaged-signal` pass to the JSON unchanged and the kind stays one of `reached`, `timed-out`, `skipped` (new test). Nothing in `spawn-at-verify` matches on the legacy `COMMIT_RESOLVED` reasons (`AT_TARGET`, `HARD_CAP`, ...); the timeline drawer prints `COMMIT_RESOLVED` key/value pairs generically, so `X11_HOLD_SECOND_DAMAGED` and `X11_HOLD_CAP` display as they come. The journal filter keeps every `[spawn-at-time]` line, including `X11_HOLD_DAMAGED` (new test). Flash detection reads the reveal and the fixture's events, not the wait strategy.
- **Two small gaps, fixed (commit `21cf0d4`, `trace/wait.rs`):** `wait_cap_ms` was read only for `ready-wait`, so a hold run reported no cap; and a requested `x11-hold-damage` whose `READY_WAIT_OVERRIDE` line was missing was not noted. Both now treat `x11-hold-damage` like `ready-wait`. One test for the `WaitInfo` reading with the hold lines. Not needed and not changed: `[expect.resize]` (compares the reason text), flash detection, the analyser.

**Phase 2, smoke (CONFIRMED).** Direct `spawn-at spawn --json --wait-strategy x11-hold-damage` of the fixture, idle, `SetLogging(true)` for the run only.
- **XWayland fixture, `larger` (400x300 to 800x600):** `READY_WAIT_OVERRIDE strategy=x11-hold-damage cap_ms=none unused=none ignored=none`; `READY_WAIT_START strategy=x11-hold-damage kind=XWayland target=(800x600) cap=250 hard=1500 reveal_on_damaged=2`; `X11_HOLD_DAMAGED n=1 ignored=true since_start_ms=11.4`; `X11_HOLD_DAMAGED n=2 ignored=false since_start_ms=45.1`; `COMMIT_RESOLVED reason=X11_HOLD_SECOND_DAMAGED finalFrame=(800x600)`; **`READY_WAIT_RESOLVED strategy=x11-hold-damage outcome=reached elapsed_ms=45.2 waited_ms=45 reason=x11-second-damaged damaged_seen=2`**; reveal 13.9 ms later (`UNCLOAK_4D_REVEAL`). A `damaged` did arrive while the window was cloaked, and both `damaged` lines show the frame still at the old position (50,90): the move to the final position came after the wait.
- **Native Wayland fixture, same flag:** `READY_WAIT_OVERRIDE ... unused=strategy(x11-hold-damage:not-x11)`, then the normal `READY_WAIT_START strategy=ready-wait kind=NativeWayland ... cap=250`, `COMMIT_RESOLVED reason=AT_TARGET`, `READY_WAIT_RESOLVED strategy=ready-wait outcome=reached ... reason=at-target`. 0 `X11_HOLD` and 0 `X11_OBS` lines in the journal of the run.

**Phase 3, A/B on the XWayland fixture (CONFIRMED data; verdict: not usable as a ready signal).** Arm **A** = `--wait-strategy legacy`, arm **H** = `--wait-strategy x11-hold-damage`; direct `spawn-at spawn --json` of the fixture with `--protocol x11`, `SPAWN_AT_FIXTURE_EVENT_DIR` set, arms rotated per iteration and spec order alternating, 3.2 s apart, one login. 20 runs per spec per arm idle (80 runs), then the same under 22 `xtask burn-cpu` threads after a 70 s warm-up (80 runs; load average 18.0 after the warm-up, 23.7 at the end). All 160 runs exit 0 and have their window; burn PID gone and `pgrep -cx xtask`/`fixture` 0 afterwards. Times are ms after the `set_size` mark S. **Final paint = the LAST fixture `FIRST_PAINT` at the target size (6.23).** "Flash" = reveal before that stamp (the stamp can be late by up to about 19 ms under load, 6.23), so "clearly early" counts reveals more than 19 ms before it. Cloak = `UNCLOAK` minus `CLOAK` log stamps.

| Cell (20 runs) | Arm | H label distribution | Reveal minus final paint, median (min to max) | Flashes (reveal before paint) | of which clearly early | Cloak median / p95 / max |
| :-- | :-: | :-- | :-- | :-: | :-: | :-- |
| idle smaller | A | - | +26.1 (+9.9 to +35.3) | 0 | 0 | 32.0 / 33.5 / 39.4 |
| idle smaller | H | second-damaged 20 | +40.3 (+28.7 to +49.5) | 0 | 0 | 45.5 / 51.2 / 59.5 |
| idle larger | A | - | +6.9 (-5.3 to +13.6) | **3** | 0 | 30.2 / 34.2 / 34.4 |
| idle larger | H | second-damaged 20 | +26.0 (+12.1 to +42.0) | **0** | 0 | 48.1 / 54.6 / 63.7 |
| loaded smaller | A | - | -41.7 (-676.9 to +5.5) | **17** | 16 | 25.9 / 35.1 / 227.1 |
| loaded smaller | H | second-damaged 20 | +8.0 (-42.0 to +36.3) | **6** | 1 | 53.2 / 75.7 / 107.0 |
| loaded larger | A | - | -147.9 (-322.2 to -60.1) | **20** | 20 | 28.3 / 38.5 / 62.5 |
| loaded larger | H | second-damaged 19, cap 1 | -123.9 (-223.2 to +10.1) | **18** | 18 | 66.2 / 219.6 / 262.4 |

No H run ended on `x11-hard`, `x11-unmanaged` or `x11-no-damaged-signal` (the `damaged` signal existed in all 80 H runs). The only cap run (loaded larger, run 1) saw no `damaged` at all during its 262 ms, then the first one at 277.9 ms: the shell itself was stalled.

**Did the second `damaged` come after the client's final paint, or is it another shell-side event? (CONFIRMED: shell-side.)**

| Cell | H ended on the 2nd damaged | After the client's final paint / before it | t2 minus final paint, median (min to max) | t1 / t2 ms after S, median | Runs with no 2nd damaged |
| :-- | :-: | :-: | :-- | :-: | :-: |
| idle smaller | 20 | 20 / 0 | +26.4 (+12.9 to +35.2) | 3.7 / 30.6 | 0 |
| idle larger | 20 | 20 / 0 | +10.8 (+2.2 to +19.8) | 5.2 / 33.6 | 0 |
| loaded smaller | 20 | 7 / **13** | -4.8 (-53.3 to +23.1) | 15.4 / 38.1 | 0 |
| loaded larger | 19 | 2 / **17** (all more than 19 ms before) | -128.4 (-195.2 to +6.0) | 20.7 / 56.2 | 1 (cap) |

- The second `damaged` arrives at about 30 to 34 ms after S idle in **both** specs, although the client paints at 4.3 ms (smaller) and 20.8 ms (larger): it is not tied to the client's paint (a paint-driven event would follow the stamp by a constant few ms, 6.23 saw 0.2 to 8 ms for the visible window; here it follows it by 26 and 11 ms). Under load it comes before the client has painted in 30 of 39 runs, by up to 195 ms. Its correlation with the paint time under load (Pearson r 0.94 and 0.80) comes from a few stalls that delay the shell and the client together, not from the paint driving it (HYPOTHESIS for that reading).
- **A cloaked window does get `damaged`** (CONFIRMED: at least 2 during the hold in 79 of the 80 H runs; the 80th is the cap run above). This contradicts the 6.25 reading "a cloaked window may get none"; the events are there but they are not the client's.
- What H did buy is a longer, fixed-looking cloak: +13 to +18 ms idle and +27 to +38 ms median under load (p95 up to 220 ms), which cuts loaded `smaller` flashes from 17 to 6 of 20 and loaded `larger` from 20 to 18 of 20, and idle `larger` from 3 to 0 of 20. That is a delay, not a readiness signal (HYPOTHESIS: a fixed 40 to 50 ms hold would give about the same; not run).

**Verdict: not usable** as a ready signal for XWayland/X11. What would change it: a second `damaged` (or any shell signal) that tracks the client's commit, e.g. one that follows the stamp by a near-constant lag in loaded runs; or a client-side signal (the X sync counter, still untried). Open: a fixed-hold control arm, and real X11 clients (the fixture is cooperative).

**Phase 4, capture diagnostic (CONFIRMED: no frame received; 3 attempts, then stopped).** A Mutter ScreenCast session was held open by a `gjs` holder (prompt-free, as in 6.25) and torn down after each attempt.
- **The node while held (`pw-dump`):** one node, `node.name=gnome-shell`, `media.class=Stream/Output/Video`, `media.name=meta-screen-cast-src`, `object.serial` a fresh number each session (426, 436, 449; `object.id` 83), `state=suspended`, `node.driver=true`, `node.want-driver=true`, `stream.is-live=true`, `client.id` of the shell, one `output_1` port, 0 links in the graph. PipeWire 1.0.5, WirePlumber 0.4.17, both active.
- **Attempt 1:** `gst-launch-1.0 pipewiresrc target-object=gnome-shell` (the `node.name`): `stream error: target not found`.
- **Attempt 2:** one Python Gst script (scratch, throwaway) building the pipeline and trying `path=<id>`, `target-object=<serial>` and `target-object=<name>` in turn: all three ended with the same `stream error: target not found` on the bus.
- **Attempt 3:** `PIPEWIRE_DEBUG=4` on the same pipeline: only the client-side module-loading lines; no server-side reason is printed for the refusal.
- Together with 6.25 (path by id, path by serial, `target-object` by serial) that is every spelling of the target, all refused. **Why, my reading (HYPOTHESIS, not shown):** the node is a `Stream/Output/Video` driver with no link and no `node.target`, created by the shell for one consumer; WirePlumber's policy does not link a second client on the default connection to it, while the portal route hands the consumer a restricted connection (`OpenPipeWireRemote`) where the target is resolved without the session manager. Not tried: `WIREPLUMBER_DEBUG` output, a native `pw_stream` client, a `wpctl`/`pw-link` manual link (video, so `pw-link` does not apply to a format-negotiated stream without a consumer node).
- **Next steps if the rig is wanted (not built):** either accept one portal prompt with the user present (`org.freedesktop.portal.ScreenCast`, `OpenPipeWireRemote`, then `pipewiresrc fd=<fd> path=<node>`), or read the WirePlumber policy log (`WIREPLUMBER_DEBUG=3`) to see why the target is rejected. The frame rate, jitter and what a capture shows remain unmeasured.

**Phase 5, cleanup and counts (CONFIRMED).** After the last run: `pgrep -cx xtask` 0, `pgrep -cx fixture` 0, no `gst-launch`, holder or probe process; the only `gjs` processes are the desktop's own, started at login (notifications, screen saver, gsconnect, ding); no `gnome-shell` PipeWire node left; burn PID 373596 gone; `spawn-at query windows` lists only the user's two windows (Claude, kitty). All fixture windows exited by themselves (`--exit-after-ms 2500`); nothing was closed by hand. Counts (run): `spawn-at` 206, `spawn-at-core` 66 (+1: the report-label test), `xtask` 10, `spawn-at-verify` bare 278, `--features tui` 380, `--features tui,fixture` 404 passed and 4 ignored (+2: the journal-filter and `WaitInfo` tests), node 188; Rust total 686, with node 874; `cargo fmt --check` and `clippy -D warnings` clean. The 1.1 table is updated, including the stale "60 unit tests" text of the `spawn-at-core` row (now 62 lib tests: 21 + 9 + 5 + 10 + 17).

**Result in one place.** (1) The harness took the new labels as free text; two small gaps (cap of a hold run, missing override note) were fixed with tests. (2) `x11-hold-damage` works as built: XWayland fixture ends on `x11-second-damaged` in 79 of 80 runs (1 cap), native Wayland is unaffected. (3) It does not work as a ready signal: the second `damaged` is a shell-side event about 30 ms after `set_size` that does not follow the client's paint (before the paint in 30 of 39 loaded runs); the fewer flashes it gives are the effect of a longer cloak, at +13 to +38 ms median. (4) No frame could be captured from the Mutter screencast node in 3 more attempts. Next: a fixed-hold control arm; the portal prompt for the capture rig; real X11 clients; the X sync counter.

**Not verified.** Real X11/XWayland applications and clients that resize late or never; screen capture of what is visible; the cause of the shell-side second `damaged` (an extension step, the frame clock, or XWayland itself); whether a fixed delay of 40 to 50 ms would do as well as H (not run); H on more than one monitor or fractional scaling.

### 6.27 X11 pre-map resize and pixel probe, stage 1 (2026-10-14)

> Experiment code removed; see branch `archive/x11-experiments`.

Labels: CONFIRMED = measured or read in this section; HYPOTHESIS = not tested. Fixture only (`--protocol x11`); nothing installed; no default changed. Raw results in `scratch/p28r/` (gitignored). Phase 0: branch `refactor/v0.2.0-beta.1`, HEAD `2916599`, tree clean, extension ACTIVE, installed files identical to `assets/gnome/modern`; baseline counts from the last run: `spawn-at` 206, `spawn-at-core` 66, `xtask` 10, `spawn-at-verify` (`tui,fixture`) 404 + 4 ignored, node 188.

**Phase 1, timeline from existing data (CONFIRMED; no live runs).** 240 XWayland fixture runs from 6.23, 6.24 and the legacy arm of 6.26 (`scratch/p23r`, `p24r`, `p27r`): extension log (`WINDOW_CREATED`, `ACTOR_MAP`, `X11_OBS_MARK set_size`, first-frame, reveal) joined to the fixture's own events on `CLOCK_MONOTONIC`. S = the resize call (`set_size` mark). Median, with the range in brackets, ms relative to S:

| Dataset (40 runs each) | window-created | actor map | first-frame | first `FIRST_PAINT` | first paint before S | first paint at the target size |
| :-- | :-: | :-: | :-: | :-: | :-: | :-: |
| 6.23 idle | -21.0 (-30.4 to -16.2) | -0.4 | +2.3 (1.6 to 5.2) | -14.4 (-25.0 to -8.4) | 40 | 0 (all 400x300) |
| 6.24 idle | -22.9 (-30.0 to -13.5) | -0.4 | +2.4 (1.4 to 3.7) | -16.2 (-26.1 to -5.3) | 40 | 0 (all 400x300) |
| 6.26 A idle | -22.2 (-52.1 to -8.1) | -0.4 | +2.2 (1.1 to 4.1) | -16.1 (-43.0 to -1.7) | 40 | 0 (all 400x300) |
| 6.23 loaded | -37.2 (-62.5 to -23.2) | -1.0 | +5.7 (3.3 to 8.8) | +14.8 (-48.0 to +162.6) | 16 | 11 |
| 6.24 loaded | -40.9 (-74.3 to -27.9) | -1.1 | +5.1 (3.2 to 12.9) | +15.1 (-42.1 to +221.6) | 13 | 6 |
| 6.26 A loaded | -37.4 (-277.3 to -25.6) | -0.9 | +5.2 (3.8 to 38.0) | -1.6 (-122.0 to +188.0) | 20 | 9 |

**(a) Is the resize issued before or after the app's first paint and first-frame? After the first paint in all 120 idle runs, and in 49 of 120 loaded runs; before first-frame in 240 of 240.** The resize is issued from the batch, which is dispatched at the actor's `map` (`DISPATCH` and `set_size` come 0.3 to 8 ms after `ACTOR_MAP`), about 21 ms (idle) and 37 to 41 ms (loaded) after `window-created`. The fixture's first paint comes 3.5 to 12.7 ms after `window-created` idle, so it is 14 to 16 ms before the resize at the median; under load it is spread from 122 ms before to 222 ms after. `first-frame` is emitted after the map, so it is always after the resize call (minimum +1.1 ms in 240 runs) and, idle, after the first paint in 120 of 120 (0 of 120 before it); under load it came before the first paint in 65 of 120 runs, so it is not a paint signal.

**(b) Size of the app's first paint: 400x300 in all 120 idle runs; under load 94 of 120 at 400x300 and 26 of 120 already at the target size.** When the resize came before the first paint (first paint after S, 71 loaded runs) the paint was at the target size in 26 and still at 400x300 in 45, that stale paint arriving 0.9 to 221.6 ms (median 19.2 ms) after the resize call. So a resize a little before the first paint is not enough; the app must have the configure before it first draws (the fixture's `RESIZED` event can trail its paint).

**(c) Earliest hook: `window-created`. The pid, WM_CLASS and the other identifiers are available there for the fixture's X11 windows (CONFIRMED).** The `WINDOW_CREATED` log line already prints `class="verify.placement.bottomrightlarger"` and the pid, and `_handleWindowCreated` already claims the armed instructions and cloaks the actor at that point (`_claimInstructions`); only the dispatch waits for `map` (`state.mapped`). So the instructions, with the size request and the strategy, are in hand 21 to 41 ms (median) before the current resize. HYPOTHESIS: for X11 apps that set `WM_CLASS` late the claim falls to the existing `notify::wm-class` retry, which runs later; not seen with the fixture. A resize at `window-created` would precede the fixture's first paint by about 7 ms idle (3.5 to 12.7), less under load (7 to 295 ms after).

**Timestamps missing from the logs:** none needed for this phase. Not logged and not available: when the X window is created inside the app before `window-created`, and when the app receives the configure (the fixture's `RESIZED` event is the nearest, and it can trail the paint).

**Phase 2, built, NOT installed, NOT run in a Shell (CONFIRMED by node and Rust tests only).**
- **a) Fixture `--state-colors`** (off by default; none existed before: the fixture painted a fixed slate background with text). Every buffer becomes one solid opaque colour chosen from its size: magenta `0xFFFF00FF` at the initial size ("before the resize"), green `0xFF00FF00` at any other size ("painted at the final size"). A pure function (`behaviour::state_color`) with a test, a flag-parse test, and a process-wide switch set once at start; without the flag `draw_frame` is unchanged. Fixture binary tests 20 to 22.
- **c-prelude, readback accessors, introspected with `gjs` outside the Shell (CONFIRMED, Meta-14):** `Meta.ShapedTexture.get_image(clip: Mtk.Rectangle, nullable) -> cairo.Surface` and `Meta.WindowActor.get_image(clip)` exist; `Cogl.Texture` has `is_get_data_supported` and `set_*` but no `get_data`; GJS cairo `ImageSurface` has `getWidth/getHeight/getStride/getFormat/writeToPNG` and **no pixel getter**. So the probe reads `WindowActor.get_image(clip)`, writes the small surface as a PNG in the runtime directory and decodes it with `GdkPixbuf` (`get_pixels`), imported lazily at the first probe so the extension loads without it. HYPOTHESIS, not run in a Shell: that a plain `{x, y, width, height}` object is accepted as the `Mtk.Rectangle` clip, that `get_image` on a cloaked actor returns its texture, and that the PNG round trip is cheap enough; any failure is logged with its own error text per sample.
- **b) `x11-premap`** (hidden `--wait-strategy` value, refused by the config file like `x11-hold-damage`): for an XWayland/X11 window with a size request that is claimed at `window-created` (before `map`), the extension resizes the still-cloaked window at once (`PREMAP_RESIZE`), connects `first-frame` first so one before the call can be told from one after it, and the batch then skips its own resize (`SET_SIZE_PREMAPPED`) and waits for the first `first-frame` after the resize, bounded by `cap_ms` (default 250, counted from the early resize) and the 1500 ms hard limit. Labels: `x11-premap-first-frame` (reached), `x11-premap-cap` and `x11-premap-hard` (timed-out), `x11-premap-unmanaged` and `x11-premap-no-first-frame-signal` (skipped); `X11_PREMAP_FIRST_FRAME` lines log each first-frame with its texture size and time since the resize. A claim that only happens at `map` gets no early resize and the original rule (`unused=strategy(x11-premap:not-premapped)`).
- **c) `x11-pixel-probe`** (hidden value; observe-only; the wait is the one the window would have had): with logging on, XWayland/X11 only, at each `damaged` and `first-frame` it queues a timer (it never reads inside the signal handler), then samples four 2x2 clips at 1/4 and 3/4 of the texture and logs `X11_PROBE_SAMPLE signal= seq= mono_ms= texture=(WxH) cost_ms= s0..s3=#RRGGBBAA/<transparent|uniform|mixed>/h:<hash>` (or `err:<message>`); at most 16 probes per window, at least 8 ms apart, one at a time, watch ended 500 ms after the reveal. Matching a sample against the fixture colour is done afterwards from the log.
- **Safety (CONFIRMED by tests):** every new path is in try/catch and a host that throws never throws into a signal emission (tests); with the strategy absent nothing is connected, resized or read (tests); native Wayland windows get the same trace line for line apart from one `READY_WAIT_OVERRIDE` note (wiring tests for both values); an X11 spawn with the probe has the same frame and the same non-probe lines as a default X11 spawn. NOT covered: a native crash in Mutter from an early `move_resize_frame` or from `get_image` (HYPOTHESIS that neither crashes; the first live run should be idle, with the user at the machine, on the fixture only).
- **d) Tests and counts (CONFIRMED, run):** node 213 (was 188; +9 `x11_premap_test.js`, +8 `x11_probe_test.js`, +8 wiring: native unaffected for both values, probe observe-only on X11, probe with logging off, pre-map through the real `window-created` and `map` handlers in three timings, claimed-after-map, native or plain through discovery); `spawn-at` 207 (+1), `spawn-at-core` 67 (+1: the spellings), `xtask` 10, `spawn-at-verify` bare 279, `--features tui` 381, `--features tui,fixture` 407 passed and 4 ignored (+3: two fixture, one `WaitInfo`); Rust total 691, with node 904; `cargo fmt --check` and `clippy --workspace --all-targets -D warnings` clean. Table in 1.1 updated. The asset-list guard test (`test_embedded_extension_files_match_disk`) failed once when the two new files were not yet listed, and passes now.

**e) To install for the live stage (not done).** New: `assets/gnome/modern/x11_premap.js`, `assets/gnome/modern/x11_probe.js`. Changed: `extension.js`, `validator.js` (and `x11_hold.js`, `commit.js` are as installed since the last stage). All are embedded in the rebuilt `spawn-at` binary together with the Rust changes (`wait_override.rs`, `settings`, `mod.rs` asset list, `trace/wait.rs`): `cargo build -p spawn-at`, `spawn-at install`, then **log out and back in** (Wayland). The fixture needs `cargo build -p spawn-at-verify --features tui,fixture` for `--state-colors`.

**Skipped and why.** No live run of anything new (rule: build only). No change to the harness beyond `WaitInfo` recognising the two spellings.

**Live-stage plan.** (1) Preflight as in 6.26, then one idle smoke of each value on the fixture with the user watching: `x11-premap` (`PREMAP_RESIZE`, `SET_SIZE_PREMAPPED`, the resolved label) and `x11-pixel-probe` (`X11_PROBE_START readback=`, the first `X11_PROBE_SAMPLE`, its `cost_ms`); stop at once if the Shell misbehaves. (2) **A/B for pre-map:** arm A = `--wait-strategy legacy`, arm P = `x11-premap`, 20 runs per spec per arm idle, then the same under 22 burn threads with a 70 s warm-up, one session, arms rotated, the 6.23 final-paint definition (last `FIRST_PAINT` at the target size), with the fixture's first-paint size read from its event file: report the size of the first paint (target or stale), the label distribution, reveal minus final paint, flashes, and the cloak. Add a fixed-hold control arm if time allows (does P beat a plain longer cloak?). (3) **Pixel probe:** the fixture with `--state-colors` under `x11-pixel-probe`, idle and loaded, 20 runs per spec: per sample class and colour at each `damaged` and `first-frame` against the fixture's `FIRST_PAINT` stamps, to see whether "blank new buffer" and "painted" are separable (magenta/green against transparent or black), and what the readback costs.

### 6.28 x11-pixel-probe and x11-premap, live on the XWayland fixture (2026-10-14)

> Experiment code removed; see branch `archive/x11-experiments`.

Labels: CONFIRMED = measured or read in this section; HYPOTHESIS = not tested. Fixture only (`--protocol x11`, `--state-colors` for the probe); no real X11 or XWayland application. Defaults unchanged. Raw results in `scratch/p29r/` (gitignored).

**Phase 0, preflight (CONFIRMED).** Session `wayland`; `spawn-at@harsh.local` Enabled, ACTIVE; protocol 6; installed files identical to `assets/gnome/modern`, including `x11_premap.js` and `x11_probe.js`; new login (gnome-shell pid 531812, started 09:26:52); no `SpawnAt`, `spawn-at`, `JS ERROR`, critical or assertion line from gnome-shell since then; HEAD `cd7967f`, tree clean; `target/debug/fixture` supports `--state-colors` (built with `--features tui,fixture`); the live `XAUTHORITY` (`.I93PW3`) is exported for child processes. **How probe logging is enabled:** the probe does nothing unless the extension's logging flag is on, and the batch scripts send `SetLogging(true)` over D-Bus once at the start of a batch (in-memory flag) and `SetLogging(false)` at the end and on exit; nothing else is switched.

**Phase 1a, smoke of `x11-pixel-probe` (CONFIRMED: the readback does not work as built; phase 1 stopped).** 3 idle `larger` spawns with `--wait-strategy x11-pixel-probe --state-colors`, exit 0, window found. The probe started (`X11_PROBE_START readback=available max=16 gap_ms=8 clip=2x2`), the wait was the normal one (`READY_WAIT_START strategy=legacy kind=XWayland why=not-native-wayland`), and probes ran at `first-frame` (texture 400x300) and `damaged` (texture 800x600), 2 to 3 per run, ended `reason=reveal+500ms`, no skipped-by-error. **Every sample of all 8 probes is an error: `Object 0x... is not a subclass of GObject_Boxed, it's a Object`.** GJS refuses the plain `{x, y, width, height}` object as the `Mtk.Rectangle` clip argument of `WindowActor.get_image`, before any readback happens; so the logged `cost_ms` of 0.03 to 0.16 is the failed call, not a readback cost, and nothing was written to the runtime directory (0 PNG files). Whether `get_image` works on a cloaked actor, and what a readback costs, are therefore **not measured**. No gnome-shell assertion, critical or JS error in the journal during the runs. **Cause and fix (HYPOTHESIS for the call, CONFIRMED for the type):** a real boxed rectangle is needed; `new Mtk.Rectangle({x, y, width, height})` is constructible with `gi://Mtk` outside the Shell (`Mtk-14.typelib` is in the mutter directory). The extension must be changed to import `gi://Mtk` lazily and pass that object, and re-installed with a log-out and log-in, which cannot be done inside this stage. So phase 1b (batch, false-positive and latency analysis, offline rule) is **not run**: it would only log 4 errors per probe.

**Phase 1 verdict: inconclusive; the pixel probe was not measured.** The only live result is the failure of 1a: the clip argument was refused, so no pixel of any window was read, no "blank new buffer" or "painted" sample exists, and the generic and fixture-colour reveal rules could not be evaluated offline (nothing to evaluate). Limits that stay whatever the result (HYPOTHESIS about real apps): a solid-colour fixture cannot prove a blank test general, because a real app's first frame may itself be white or black, an app may repaint the same colour, and a 2x2 clip can miss content; a fixture colour rule ("reveal when green") is only an upper bound. What would change this: install the in-tree fix (commit after `b87e9ac`, which passes a real `Mtk.Rectangle`, lazily imported with GdkPixbuf), log out and back in, and rerun phase 1a (3 smokes: the clip accepted, `get_image` on a cloaked actor, readback cost under about 15 ms), then the 1b batch.

**Phase 2a, `x11-premap` smoke (CONFIRMED; 5 runs per spec per arm, same session, arms rotated, `--protocol x11` without `--state-colors`; arm A = `legacy` as the reference).** One idle run first, journal clean: `PREMAP_RESIZE` at `window-created` (frame 400x300 to 800x600 before any map), `SET_SIZE_PREMAPPED` 17.0 ms later at the map, `first-frame` 19.4 ms after the resize, `READY_WAIT_RESOLVED strategy=x11-premap outcome=reached ... reason=x11-premap-first-frame`. All 40 smoke runs exit 0; gnome-shell pid unchanged and no assertion, critical or JS error in the journal (the pre-map `move_resize_frame` did not upset Mutter in these runs; HYPOTHESIS that it never does). Idle, then 22 burn threads with a 70 s warm-up. "Hidden" = `window-created` to `UNCLOAK` (the batch's own CLOAK line starts later and understates it for both arms). Flash = reveal before the last `FIRST_PAINT` at the target size (6.23).

| Cell (5 runs) | Arm | First paint at the target size | Labels | Flashes (clearly early, > 19 ms) | Reveal minus final paint, median (min to max), ms | Hidden median (min to max), ms |
| :-- | :-: | :-: | :-- | :-: | :-- | :-- |
| idle smaller | A | 0 of 5 (all 400x300) | - | 0 | +26.9 (+4.6 to +36.8) | 55.3 (51.4 to 56.8) |
| idle smaller | P | **3 of 5** (200x150) | first-frame 5 | 0 | +38.3 (+21.7 to +47.5) | 52.9 (37.1 to 56.5) |
| idle larger | A | 0 of 5 | - | 1 (0) | +11.9 (-2.6 to +13.2) | 56.2 (40.0 to 73.5) |
| idle larger | P | **3 of 5** (800x600) | first-frame 5 | 1 (0) | +7.6 (-4.8 to +18.6) | 49.7 (38.8 to 52.8) |
| loaded smaller | A | 2 of 5 | - | 4 (2) | -13.4 (-218.4 to +5.8) | 70.5 (49.5 to 201.2) |
| loaded smaller | P | 2 of 5 | first-frame 5 | 3 (1) | -4.4 (-31.8 to +26.5) | 76.7 (54.1 to 89.1) |
| loaded larger | A | 2 of 5 | - | 5 (5) | -171.8 (-316.6 to -117.8) | 75.8 (49.1 to 92.3) |
| loaded larger | P | **5 of 5** (800x600) | first-frame 5 | **5 (5)** | **-253.5 (-299.8 to -204.7)** | 58.6 (44.8 to 65.6) |

- **Pre-map does change the app's first paint sometimes** (idle: 0 of 10 at the target for A, 6 of 10 for P), but not reliably, and a first paint at the target size is not the same as a paint that is done: in loaded `larger` all five P first paints are at 800x600 yet they come 147 to 249 ms after the resize and the final paint at 255 to 358 ms, while `first-frame` arrives about 2 ms after the map (44 to 65 ms after the resize), so the window is revealed 205 to 300 ms before the app has painted anything it will keep. `first-frame` is a map event, not an app event, with the resize moved earlier or not.
- **Gate for the full A/B: FAILED on both conditions.** (1) First paint at the target size in loaded P runs: 7 of 10 (70%), below the 80% bar (loaded smaller 2 of 5, loaded larger 5 of 5). (2) Flashes are not clearly lower than legacy: loaded P 8 of 10 against 9 of 10 for A in the same session (and 37 of 40 for legacy in 6.26); idle the same (1 of 10 against 1 of 10). So the 20-per-arm A/B was **not run**. The smoke is small (5 per cell); the failure of condition 2 is by a wide margin for `larger` (5 of 5 and 5 of 5 flash, reveal 205 to 300 ms early).
- **Verdict for `x11-premap`: not usable as a ready signal** on this evidence. What would change it: a signal that follows the app's commit rather than the map, with the resize at `window-created` (the early resize itself works and is cheap).

**Phase 3, cleanup and counts (CONFIRMED).** After the last run: `pgrep -cx xtask` 0, `fixture` 0, no `gst-launch`; no PNG in the runtime directory (the probe wrote none; the removal was a no-op); `spawn-at query windows` lists only the user's windows (Claude, a GNOME terminal, kitty); gnome-shell pid 531812, unchanged since the login, and no assertion, critical, JS error or `[SpawnAt]` error line in its journal since then; the burn threads were stopped by the batch script (`pgrep -cx xtask` 0). Counts (run): `spawn-at` 207, `spawn-at-core` 67, `xtask` 10, `spawn-at-verify` (`tui,fixture`) 407 passed and 4 ignored, node 213; Rust total 691, with node 904, unchanged; `cargo fmt --check` and `clippy -D warnings` clean. The in-tree probe fix (not installed, no new test: it needs `gi://Mtk`, which the node harness does not provide) is the only code change of this stage.

**Result in one place.** (1) The pixel probe's readback failed at the first call (plain object refused as the clip); fixed in the tree, to be installed and rerun; nothing about blank-versus-painted is known yet. (2) Pre-map resize works live and moves the app's first paint to the target size in 6 of 10 idle runs, but `first-frame` is a map event and arrives 205 to 300 ms before a loaded app has painted; the gate for the full A/B failed on both conditions. (3) No new reliable X11/XWayland signal. **Not verified:** `get_image` on a cloaked actor and its cost; whether the early resize is safe for every X11 client; real X11 apps; more than 5 runs per premap cell.

### 6.29 x11-pixel-probe, live retry with the Mtk.Rectangle fix (2026-10-14): stopped after phase 1, the Shell crashed in the batch

> Experiment code removed; see branch `archive/x11-experiments`.

Labels: CONFIRMED = measured or read in this section; HYPOTHESIS = not tested. Fixture only (`--protocol x11 --state-colors`). Raw files in `scratch/p30r/` (gitignored).

**Phase 0, preflight (CONFIRMED).** Session `wayland`; extension Enabled, ACTIVE; protocol 6; installed files identical to `assets/gnome/modern` including the `Mtk.Rectangle` fix (`02ae11e`); HEAD `2d8fc5e`; gnome-shell pid 553983 (started 09:57:09). The journal of that shell already held one Mutter line before any spawn: `Oct 14 09:57:15 harsh gnome-shell[553983]: surface_state_changed: assertion 'wl_window->has_last_sent_configuration' failed`. I called it a baseline and went on. It is a Wayland window-state assertion at login, 6 s after the shell started and 71 s before the first probe run; it is not from spawn-at, and it was not the crash (the shell lived until 09:59:04), but I should have stopped and asked at that point under the rule "stop if the journal shows a gnome-shell assertion".

**Phase 1, smoke of `x11-pixel-probe`: it works in 3 of 3 (CONFIRMED).** 3 idle `larger` spawns, 09:58:25 to 09:58:32, exit 0, window found, journal clean of new assertions.
- The `Mtk.Rectangle` clip is accepted, `get_image` returns a surface for the cloaked actor, and the PNG round trip works: the last file left in the runtime directory was a valid 2x2 PNG with four `#00FF00` pixels (removed afterwards).
- Samples: `first-frame` (texture 400x300) `s0..s3=#00000000/transparent/h:69691905` in 3 of 3 runs (a blank, fully transparent buffer right after the map); `damaged` (texture 800x600) `#00ff00ff` in 2 of 2 runs that had one, the fixture's green (the first-frame sample of the first run came before the app painted).
- Readback cost per probe (4 clips): 2.26, 2.37 (first-frame) and 2.68, 2.65 ms (damaged) in the warm runs; **89.8 ms for the first probe after the extension started** (one-time cost: the lazy GdkPixbuf and Mtk imports and the first GPU readback), a stall of the Shell's main loop.
- Logged class `mixed` for the green samples is a decoding bug of mine (CONFIRMED: the PNG is solid): for an opaque RGB image gdk-pixbuf pads each row (2 pixels of 3 bytes in an 8-byte row) and my RGB-to-RGBA conversion read the padding as pixels. The first pixel is right. Not fixed in the tree yet.

**Phase 2, not run: the batch crashed GNOME Shell and its data is discarded (CONFIRMED facts below).** The idle batch (20 per spec, arm `x11-pixel-probe`, `--state-colors`, logging switched on by the script) started at 09:59:04.
- **Crash time: 09:59:04**, in the first second of the batch, at the first run (`smaller`, window id 2872954786): journal `SET_LOGGING enabled=true`, `ARM_SPAWN`, `WINDOW_CREATED` at +38497 ms, then `static JSObject* CairoSurface::from_c_ptr(JSContext*, cairo_surface_t*): assertion 'surface' failed`, `Call to function "_addTimer/id<" (GLib.SourceFunc) terminated with uncatchable exception`, `GNOME Shell crashed with signal 5`. systemd: `org.gnome.Shell@wayland.service: Main process exited, code=dumped, status=5/TRAP`. The user session was restarted (new gnome-shell pid 558192 at 09:59:18/19); the user's session apps were lost.
- **Cause (CONFIRMED from the log, HYPOTHESIS for why the surface was null):** the callback is the probe's timer (`_addTimer`). `WindowActor.get_image(clip)` returned a NULL `cairo_surface_t` (`may_return_null=true` in the typelib), and GJS asserts and aborts on a NULL cairo surface return instead of giving JS `null`, so the `if (!surface)` check and the `try/catch` of the probe never ran. Likely trigger: the first probe of a fresh window before its texture had content. The probe as built can crash the Shell and must not be used.
- **Runs finished: 0 valid.** The batch script recorded 23 runs (`exits.txt`, all "exit=0", the last at 10:01:32) before it was stopped, but the first of them started after the crash: runs 1 to 4 (09:59:07 to 09:59:17) went to a dead Shell and the rest to the new session, where they found no extension-claimed window (about 7 s each instead of 3.2 s). Nothing in them is data; the directory is kept as `scratch/p30r/idle_DISCARDED_shell_crashed/`. The loaded batch never started (no burn threads).
- **Leftovers:** none (`pgrep` finds no fixture, xtask, burn or probe process); the probe PNG in the runtime directory was removed.

### 6.30 Regression after the removal of the X11 experiments (2026-10-14)

Labels: CONFIRMED = measured or read in this section; HYPOTHESIS = not tested. No code changed; fixture only. Raw results in `scratch/p31r/` (gitignored).

**Phase 1, preflight (CONFIRMED).** HEAD `acccca8`, tree clean. Session `wayland`; `spawn-at@harsh.local` Enabled, ACTIVE; protocol 6; installed files identical to `assets/gnome/modern` and exactly the 11 files of the tree (`x11_hold.js`, `x11_premap.js` and `x11_probe.js` are not installed); new login (gnome-shell pid 581106, started 10:22:56). **The journal since that login has no gnome-shell or mutter assertion, no signal line, no "uncatchable exception", no critical, no JS error and no `[SpawnAt]` line** (full-journal grep, before any spawn). The live Xwayland authority file is `.2BNMW3` (a second, older file `.H1LOW3` is also in the runtime directory) and is exported for child processes. `spawn-at --version` (installed and debug build): `DEV BUILD`, previous commit `acccca8`. `config` round trip on a throwaway file (`SPAWN_AT_CONFIG`): `show` lists the three keys with defaults; `set wait.cap_ms 400` then `get` 400 and `show` source `file`; `unset` then `get` 250; `config set wait.strategy x11-hold-damage` is refused (`[spawn-at] ERROR: wait.strategy must be auto, legacy or ready-wait (got 'x11-hold-damage')`, exit 1). **`spawn --wait-strategy x11-hold-damage` is not an error but a warning** (`[spawn-at] WARNING: ignoring --wait-strategy 'x11-hold-damage' (expected auto, legacy or ready-wait)`) and the spawn goes on with the default, which is how every invalid value of that hidden flag has always been handled. The autostart entry reads `Exec=/home/harsh/.local/bin/spawn-at update --check --headless` (absolute path).

**Phase 2, native Wayland A/D regression (CONFIRMED).** Same method as 6.23: the harness `run` on `placement/bottom-right-smaller` and `placement/bottom-right-larger`, arm A = `--wait-strategy legacy`, arm D = no override (the default), arms rotated every iteration, same login, wire-time flash verdict (`paint_source=wire`, 160 of 160). Idle: 20 per arm per spec (80 runs); loaded: 22 `xtask burn-cpu` threads after a 70 s warm-up (load average 19.05, 24.9 at the end), 20 per arm per spec = 40 per arm (80 runs). All 160 runs exit 0, 0 invalid rows, 0 `X11_OBS` lines in any native run, the burn stopped by its PID (`pgrep -cx xtask` 0, `fixture` 0 afterwards); the journal had no assertion, signal, "uncatchable" or critical line at any of the checks (before the idle batch, after it, after the loaded batch).

| Measure | This run | 6.23 | Consistent? |
| :-- | :-: | :-: | :-- |
| Idle wire-time flash runs, A | 0 of 40 | 0 of 20 | yes |
| Idle wire-time flash runs, D | **1 of 40** | 0 of 20 | one run, see below (Fisher p = 1.0 against A) |
| Idle outcomes, D | reached/at-target 39, timed-out/cap 1 | reached 20 | one cap run |
| Idle cloak median / p95 / max, A (ms) | 20.8 / 49.7 / 51.5 | 19.6 / 34.9 / 51.1 | yes (p95 is one or two slow runs) |
| Idle cloak median / p95 / max, D (ms) | 20.6 / 50.8 / 274.1 | 19.5 / 35.0 / 49.1 | median yes, max is the cap run |
| Loaded wire-time flash runs, A | **27 of 40** (52 events: 39 certain, 13 possible) | 26 of 40 (51 events: 38 / 13) | yes |
| Loaded wire-time flash runs, D | **0 of 40** | 0 of 40 | yes (Fisher p = 1.6e-11 against A) |
| Loaded outcomes, D | reached/at-target 40 | reached 40 | yes |
| Loaded cloak median / p95 / max, A (ms) | 48.9 / 70.9 / 110.8 | 45.3 / 53.0 / 70.2 | median yes; p95 and max higher |
| Loaded cloak median / p95 / max, D (ms) | 56.2 / 115.5 / 244.5 | 66.8 / 104.7 / 116.2 | median lower, p95 similar, max higher |
| Loaded D `larger` cloak median (ms) | 77.9 | 83.2 | yes |

- **The loaded result is reproduced:** A flashes in 27 of 40 loaded runs (6.23: 26), D in 0 of 40, and the cost of D is the same kind of increase in cloak (median 56.2 against 48.9 ms for A here; 66.8 against 45.3 in 6.23). The differences in the medians (D 56.2 against 66.8, A 48.9 against 45.3) are within what two 40-run batches on a busy machine vary by (HYPOTHESIS: no repeat of the same batch was made to measure that spread); the larger p95 and maxima come from a few slow runs (loaded D `larger` has one wait of 232.5 ms, close to the 250 ms cap).
- **One idle difference, stated plainly:** in the idle batch one D run (`bottom-right-larger` #17, load average 0.90) hit the cap: the app did not answer the 800x600 request within 250 ms (`READY_WAIT_RESOLVED outcome=timed-out elapsed_ms=251.5 reason=cap`), the window was revealed at 400x300 and corrected 49 ms later (`ANCHOR_CORRECTION`), and the fixture's commit at the final size came 117 ms after the reveal (two certain flash events). 6.23 had no such run in 20 idle D runs, this batch 1 in 40; I did not find the cause (a stall of the fixture or the Shell on an idle machine, HYPOTHESIS), it is the cap working as designed for a late answer, and one run does not show a regression. The legacy arm had no such run (0 of 40).

**Phase 3, XWayland fixture smoke on the legacy rule (CONFIRMED).** 5 runs per spec (10 runs), `--protocol x11`, direct `spawn-at spawn --json` with no override, `SetLogging(true)` for the batch only. All exit 0 and every window found. All 10 took the normal path: `READY_WAIT_START strategy=legacy kind=XWayland why=not-native-wayland`, 10 `READY_WAIT_RESOLVED`, 10 `COMMIT_RESOLVED`. The observe-only lines are still there and clean: 10 `X11_OBS_START`, 30 `X11_OBS_MARK`, 35 `X11_OBS_SIGNAL`, 10 `X11_OBS_END`, and all 35 signal lines carry `texture_via=shaped>multi` (no `texture_err`, no `(na)`). **0 lines** of `X11_PROBE`, `PREMAP`, `X11_HOLD`, `get_image` or `readback`; no assertion, signal, "uncatchable" or critical line in the journal afterwards.

**Phase 4, read-only release checks (CONFIRMED by reading `.github/workflows/release.yml`; nothing edited).**
- **Trigger:** `push` of a tag matching `v*` (so `v0.2.0-beta.1` and `v0.2.0` both match).
- **Job (one, `ubuntu-24.04`, `contents: write`):** checkout, stable Rust toolchain, `cargo test --verbose` (at the repository root, so the root package `spawn-at` only, not the workspace crates or the node tests), `cargo build --release --locked`, package, upload.
- **Artifact:** one file, `spawn-at-x86_64-unknown-linux-gnu.tar.gz`, containing the `spawn-at` release binary plus `README.md` and `LICENSE` (the extension is embedded in the binary). No checksum file, no other platform.
- **Release notes:** `generate_release_notes: true` (GitHub's automatic notes); `docs/RELEASE_NOTES_0.2.0-beta.1.md` and `CHANGELOG.md` are not used.
- **Prerelease: the workflow does not mark the release as a prerelease** (`softprops/action-gh-release@v2` with no `prerelease:` input; its default is false), and it is not a draft. **A `v0.2.0-beta.1` tag would therefore publish as an ordinary, "latest" release.** This matters because `update.rs` and the stable channel skip releases that are flagged `prerelease`, and `install.sh --prerelease` is how the README tells users to install a beta (HYPOTHESIS about GitHub's treatment: a release for a semver tag with a `-beta.1` suffix is not auto-flagged; the flag is set by the workflow or by hand afterwards). To decide before tagging: add `prerelease: ${{ contains(github.ref_name, '-') }}` to the upload step, or flag the release by hand.
- **Versions (CONFIRMED):** `Cargo.toml` of the root package, `spawn-at-core`, `spawn-at-verify` and `xtask` are all `0.2.0-beta.1`, and `Cargo.lock` has the same for the three that it lists by version.

**Phase 5, docs (CONFIRMED).** In `docs/RELEASE_NOTES_0.2.0-beta.1.md` the observe-only X11/XWayland logging is no longer a feature bullet: it is one clause of a single "Developer-only note" bullet together with the test harness. In `CHANGELOG.md` it is removed from "Added" (the harness bullet too) and now sits under a new "Developer notes (not user-facing)" subsection before "Known limitations".

**Phase 6, cleanup and counts (CONFIRMED).** No `xtask` or fixture process remains, no probe file in the runtime directory, `spawn-at query windows` lists only the user's two windows (Claude, kitty); gnome-shell pid 581106, unchanged since the login, and no assertion, crash, signal, "uncatchable", critical, JS or `[SpawnAt]` line in the journal at any check (start, after each batch, end). Counts (run): `spawn-at` 205, `spawn-at-core` 64, `xtask` 10, `spawn-at-verify` (`tui,fixture`) 404 passed and 4 ignored, node 176; unchanged from the removal step.

**Result in one place.** Nothing regressed: the loaded A against D result of 6.23 reproduces (A 27 of 40 flash, D 0 of 40), the idle numbers match apart from one D run that hit the 250 ms cap (cause not found), the XWayland path and its observe-only lines are clean, the removed strategy value is ignored with a warning (and refused by `config set`), and the journal stayed clean. **Not verified:** the cause of the single idle cap run; the spread of repeated identical batches; real X11 apps; anything on GNOME versions other than 46; the release workflow was read, not run.

### 6.31 X sync counter (`_NET_WM_SYNC_REQUEST_COUNTER`) as a client-side ready signal: feasibility (2026-10-14) DRAFT, finalized at the end

Labels: CONFIRMED = measured or read in this section; HYPOTHESIS = not tested. Nothing in the Shell, no code or extension change, no install; read-only property queries and one fixture spawn only. Raw output in `scratch/p32r/` (gitignored). HEAD `a950022`, tree clean; gnome-shell pid 581106 since 10:22:56, no assertion, crash, signal, "uncatchable" or critical line in the journal at the start. The live Xwayland authority (`.2BNMW3`) was exported for the queries.

**Phase 1, survey of the X11 windows that are open (CONFIRMED, `xwininfo`, `xprop`, read-only).** No real X11 or XWayland application window is open: the user's windows (Claude, kitty, terminals) are native Wayland. The only X clients are desktop helpers, none of which advertises sync:

| Window | `WM_CLASS` | `_NET_WM_SYNC_REQUEST` in `WM_PROTOCOLS` | `_NET_WM_SYNC_REQUEST_COUNTER` |
| :-- | :-- | :-: | :-: |
| `gsd-xsettings` | `gsd-xsettings` | no | not set |
| `ibus-x11` | `ibus-x11` | no | not set |
| `mutter-x11-frames` | `mutter-x11-frames` | no | not set |
| `GNOME Shell`, `ibus-xim`, `mutter guard window` | none | no | not set |

So none of the user's open apps advertises it, because none of them is an X11 client at the moment (which says nothing about whether the same apps would advertise it under X11: see the earlier measurement below).

**Phase 2, the fixture (CONFIRMED).** One `--protocol x11` fixture window (id 16777220, class `verify.p32`) kept open for 7 s and queried: `WM_PROTOCOLS = WM_DELETE_WINDOW, _NET_WM_PING`, **no `_NET_WM_SYNC_REQUEST`, and `_NET_WM_SYNC_REQUEST_COUNTER` is not set**; the X server does have the SYNC extension (`xdpyinfo` lists it). This agrees with the earlier measurement (DESIGN_READY_WAIT section 3.2: the fixture does not advertise it, and winit 0.29.15 contains no sync-request code, found by grepping the vendored crate). **The fixture is therefore not a client the idea can be tested with, and phase 3 (counter log, 20 idle and 20 loaded runs per spec) was not run**, as the task said to stop here.
