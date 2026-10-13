# spawn-at Test Library & Verification Plan

## 1. Overview & Architecture

`spawn-at-verify` is the dedicated verification engine and test harness for the `spawn-at` window placement CLI and GNOME Shell extension. It validates zero-flicker window creation, exact geometry placement against a mathematical oracle, and timeline event sequencing via compositor trace analysis.

Tests are defined declaratively in `verify/<category>/<test-id>.toml` specifications, executed against a real Wayland GNOME session, and presented via both an automated CLI runner and a headless/interactive Terminal User Interface (TUI).

### 1.1 Test Suite Counts & Feature Gates Baseline

Automated test execution across the workspace is partitioned by crate and feature flags:

| Target / Feature Set | Tests Passed | Tests Ignored | Breakdown by Target / Binary |
| :--- | :---: | :---: | :--- |
| **Workspace: `spawn-at` CLI** | 158 | 0 | CLI flag parsing, IPC client, fallback, and reposition tests |
| **Workspace: `spawn-at-core`** | 34 | 0 | 30 unit tests (`lib.rs`: 21 geometry/driver + 9 `ready_wait`) + 2 golden placement integration tests (`tests/golden_placement.rs`) + 2 `tests/ready_wait_vectors.rs` (20 shared golden scripts in one test, plus the policy check) |
| **Workspace: `xtask`** | 10 | 0 | 8 unit tests (`src/main.rs`: 6 `burn_cpu`, 2 `compiler_docs`) + 2 `burn_cpu_test` (`tests/burn_cpu_test.rs`, runs the built binary) |
| **Workspace: Node.js (Extension)** | 101 | 0 | Golden anchor math, extension characterization, legacy/modern dispatchers, late-resize re-anchor (`tests/late_resize_test.js`), and 31 in `tests/ready_wait_test.js` (the same 20 golden scripts one test each, the presence and policy checks, 8 mirrored unit cases, 1 portability check) (`node --test tests/*.js`) |
| **`spawn-at-verify` (bare default)** | 236 | 0 | 169 lib + 0 main + 4 `category_library_test` + 4 `cli_exit_codes_test` + 20 `flash_test` + 5 `oracle_cross_check` + 1 `preflight_tools_test` + 21 `trace_analyser_test` + 6 `trace_state_test` + 6 `trace_window_scope_test`. The 6 lib tests and the `render_test` and `tui_test` binaries that need `feature = "tui"` are not compiled. |
| **`spawn-at-verify --features tui`** | 338 | 0 | 175 lib + 0 main + 4 `category_library_test` + 4 `cli_exit_codes_test` + 20 `flash_test` + 5 `oracle_cross_check` + 1 `preflight_tools_test` + 13 `render_test` + 21 `trace_analyser_test` + 6 `trace_state_test` + 6 `trace_window_scope_test` + 83 `tui_test` |
| **`spawn-at-verify --features tui,fixture`** | 352 | 4 | 175 lib + 10 fixture binary (`src/bin/fixture.rs`) + 0 main + 4 `category_library_test` + 4 `cli_exit_codes_test` + 4 `fixture_test` (+ 4 ignored) + 20 `flash_test` + 5 `oracle_cross_check` + 1 `preflight_tools_test` + 13 `render_test` + 21 `trace_analyser_test` + 6 `trace_state_test` + 6 `trace_window_scope_test` + 83 `tui_test` |
| **Total (Rust crates, `spawn-at-verify` counted with `tui,fixture`)** | 554 | 4 | 158 + 34 + 10 + 352; with the 101 Node tests: 655 passed |

The three `spawn-at-verify` rows and the total were re-measured on 2026-10-13 at commit `1de817f` (`cargo test -p spawn-at-verify` bare, `--features tui`, `--features tui,fixture`, summed with `awk` over the `test result:` lines; `cargo test --workspace` 427 = 158 + 23 + 10 + 236); 0 failures. The `spawn-at-core` (34), `spawn-at` (158), `xtask` (10) and Node (101) rows were re-measured on 2026-10-13 at commit `902dff3` after the ready-wait core and its JS port (section 8 of `DESIGN_READY_WAIT.md`): `cargo test --workspace` 438 = 158 + 34 + 10 + 236, `node --test tests/*.js` 101 pass, 0 failures; the verify rows did not change (352 / 338 / 236). A bare `cargo test --workspace` builds `spawn-at-verify` without features, so its number for that crate is the 236 row, not 352; do not quote it for the verify crate. Baseline before section 6.15 (`7400e57`): 305 with `tui,fixture`; section 6.15 took it to 341 (`1da549d`), section 6.16 to 352.

#### `fixture_test` Ignored Tests Documentation
The 4 tests marked `#[ignore]` in `crates/spawn-at-verify/tests/fixture_test.rs` require a live graphical display session (compositor) to execute:
1. `test_fixture_clean_exit_timeout`: Opens a live test window with softbuffer/winit event loop and verifies timing-based self-termination. Requires an active Wayland compositor or X11 display server.
2. `test_fixture_print_ready_output`: Spawns the GUI fixture process and waits for synchronization handshake token `[fixture] READY` emitted once the window surface renders. Requires live display server connection.
3. `test_fixture_signal_termination`: Spawns a live graphical test window and tests asynchronous SIGTERM handling during the active event loop. Requires live display server connection.
4. `test_fixture_size_matches_query_windows_on_all_protocols`: Spawns GUI test windows across Wayland and X11 backends and invokes `spawn-at query windows` via D-Bus to cross-check reported dimensions against requested frame bounds. Requires active GNOME Shell compositor running the `spawn-at` extension.

---

## 2. 59-Case Test Catalog across 8 Categories

The 59 rows are the planned catalogue: 36 are **Covered** (a spec file exists and `spawn-at-verify list` shows it: 36 tests) and 23 are **Planned** (no spec yet). The last 18 Covered rows are the placement resize specs of section 6.10.

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
