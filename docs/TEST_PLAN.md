# spawn-at Test Library & Verification Plan

## 1. Overview & Architecture

`spawn-at-verify` is the dedicated verification engine and test harness for the `spawn-at` window placement CLI and GNOME Shell extension. It validates zero-flicker window creation, exact geometry placement against a mathematical oracle, and timeline event sequencing via compositor trace analysis.

Tests are defined declaratively in `verify/<category>/<test-id>.toml` specifications, executed against a real Wayland GNOME session, and presented via both an automated CLI runner and a headless/interactive Terminal User Interface (TUI).

### 1.1 Test Suite Counts & Feature Gates Baseline

Automated test execution across the workspace is partitioned by crate and feature flags:

| Target / Feature Set | Tests Passed | Tests Ignored | Breakdown by Target / Binary |
| :--- | :---: | :---: | :--- |
| **Workspace: `spawn-at` CLI** | 158 | 0 | CLI flag parsing, IPC client, fallback, and reposition tests |
| **Workspace: `spawn-at-core`** | 23 | 0 | 21 unit tests (`lib.rs`) + 2 golden placement integration tests (`tests/golden_placement.rs`) |
| **Workspace: Node.js (Extension)** | 65 | 0 | Golden anchor math, extension characterization, legacy/modern dispatchers (`node --test tests/*.js`) |
| **`spawn-at-verify` (bare default)** | 58 | 0 | 58 lib tests. Note: 1 test (`tui::logging::tests::test_parse_logging_state_transitions`) is gated behind `feature = "tui"`. |
| **`spawn-at-verify --features tui`** | 124 | 0 | 59 lib + 0 main + 4 `category_library_test` + 5 `oracle_cross_check` + 18 `trace_analyser_test` + 38 `tui_test` |
| **`spawn-at-verify --features tui,fixture`** | 259 | 4 | 124 lib + 7 fixture binary (`src/bin/fixture.rs`) + 0 main + 4 `category_library_test` + 2 `cli_exit_codes_test` + 4 `fixture_test` (+ 4 ignored) + 5 `oracle_cross_check` + 1 `preflight_tools_test` + 12 `render_test` + 18 `trace_analyser_test` + 6 `trace_state_test` + 6 `trace_window_scope_test` + 70 `tui_test` (re-measured after the placement batch and the echo-timeout changes were added; the other rows in this table were not re-measured) |

#### `fixture_test` Ignored Tests Documentation
The 4 tests marked `#[ignore]` in `crates/spawn-at-verify/tests/fixture_test.rs` require a live graphical display session (compositor) to execute:
1. `test_fixture_clean_exit_timeout`: Opens a live test window with softbuffer/winit event loop and verifies timing-based self-termination. Requires an active Wayland compositor or X11 display server.
2. `test_fixture_print_ready_output`: Spawns the GUI fixture process and waits for synchronization handshake token `[fixture] READY` emitted once the window surface renders. Requires live display server connection.
3. `test_fixture_signal_termination`: Spawns a live graphical test window and tests asynchronous SIGTERM handling during the active event loop. Requires live display server connection.
4. `test_fixture_size_matches_query_windows_on_all_protocols`: Spawns GUI test windows across Wayland and X11 backends and invokes `spawn-at query windows` via D-Bus to cross-check reported dimensions against requested frame bounds. Requires active GNOME Shell compositor running the `spawn-at` extension.

---

## 2. 41-Case Test Catalog across 8 Categories

The 41 rows are the planned catalogue: 18 are **Covered** (a spec file exists and `spawn-at-verify list` shows it, 18 tests) and 23 are **Planned** (no spec yet).

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

`placement/center-200x150` and `placement/center-1200x800` are the first specs that force a real resize: the fixture opens at its default 400x300 (`--width`/`--height` default to 400/300, `crates/spawn-at-verify/src/bin/fixture.rs:46-51`, used for the initial size at `bin/fixture/window.rs:48,61`) and neither spec passes `--width`/`--height` (`verify/placement/center-200x150.toml`, `center-1200x800.toml`). Nothing was changed to investigate: no product code, spec, fixture or budget. The specs stay failing when they fail. Each claim is labelled **CONFIRMED** (shown by a log, a count or a line of code) or **HYPOTHESIS**.

**Symptom (CONFIRMED).** On a failing run spawn-at logs `[spawn-at] WARNING: Window mapped at (760, 410) [WxH] but expected position was (X, Y)`. (760, 410) is where a 400x300 window is centred in the workarea `0,40 1920x1040`. The window does end up WxH (the `UNCLOAK` event shows `frame=(760,410,WxH)`), it just keeps the top-left chosen for 400x300. Recorded failing run: `crates/spawn-at-verify/tests/fixtures/real_placement_center_200x150_fail_run.json`.

**Frequency (CONFIRMED, counts from `verify-results/`).**

| Condition | center-200x150 | center-1200x800 |
| :--- | :---: | :---: |
| Idle, `run --repeat 20` (all 20 runs stored, binary at HEAD) | 0/20 fail | 0/20 fail |
| Idle, 20 separate `run` invocations 1.1 s apart | 0/20 fail | 0/20 fail |
| All 22 CPUs busy (22 `while :; do :; done` loops), 20 separate invocations | **12/20 fail** | not run |
| The 03:16-03:17 batch that first showed the failures (stored reports only; same-second runs overwrote each other before the directory-name fix, so the real run count was higher) | 3 fails in 7 stored runs | 2 fails in 7 stored runs |

All failing runs show the same signature (`COMMIT_RESOLVED reason=SIZE_SETTLED finalFrame=(400x300)`, `APPLY_ANCHOR eff=(400x300)`, final `760,410`). So the failure rate is not a constant 30%: it was 0/80 at idle and 12/20 with every CPU busy. That CPU load causes it is **CONFIRMED** for this machine (same binary, same spec, only the load changed). That the earlier 03:16-03:17 failures were also caused by load is **HYPOTHESIS** (they all fall inside about 70 s, and nothing else about the machine was recorded).

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

**Fixture or product? (CONFIRMED that it is not only the fixture.)** `zenity --text-info` (GTK, `GDK_BACKEND=wayland`) has a 300x400 minimum, so 200x150 only gets raised to 300x400 (`size_raised`) and cannot test this. At sizes it can take, 20 runs each, `spawn-at spawn --class '*' --size W H --anchor center --json -- zenity ...`:

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
