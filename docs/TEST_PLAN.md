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
| **`spawn-at-verify --features tui,fixture`** | 135 | 4 | 59 lib + 7 fixture binary (`src/bin/fixture.rs`) + 0 main + 4 `category_library_test` + 4 `fixture_test` (+ 4 ignored) + 5 `oracle_cross_check` + 18 `trace_analyser_test` + 38 `tui_test` |

#### `fixture_test` Ignored Tests Documentation
The 4 tests marked `#[ignore]` in `crates/spawn-at-verify/tests/fixture_test.rs` require a live graphical display session (compositor) to execute:
1. `test_fixture_clean_exit_timeout`: Opens a live test window with softbuffer/winit event loop and verifies timing-based self-termination. Requires an active Wayland compositor or X11 display server.
2. `test_fixture_print_ready_output`: Spawns the GUI fixture process and waits for synchronization handshake token `[fixture] READY` emitted once the window surface renders. Requires live display server connection.
3. `test_fixture_signal_termination`: Spawns a live graphical test window and tests asynchronous SIGTERM handling during the active event loop. Requires live display server connection.
4. `test_fixture_size_matches_query_windows_on_all_protocols`: Spawns GUI test windows across Wayland and X11 backends and invokes `spawn-at query windows` via D-Bus to cross-check reported dimensions against requested frame bounds. Requires active GNOME Shell compositor running the `spawn-at` extension.

---

## 2. 31-Case Test Catalog across 8 Categories

| ID | Title | Mode | Status | Spec / Test Path |
| :--- | :--- | :---: | :---: | :--- |
| **Category: basics** | | | | |
| `preflight/binary-head` | Binary version matches repository HEAD | Auto | **Covered** | `verify/preflight/binary-head.toml` |
| `basics/version-and-help` | CLI `--help` and `--version` formatted output | Auto | **Planned** | `verify/basics/version-and-help.toml` |
| `basics/instant-paint` | Instant paint without artificial delay | Auto | **Planned** | `verify/basics/instant-paint.toml` |
| `basics/shell-extension-active` | Extension IPC handshake and version ping | Auto | **Planned** | `verify/basics/shell-extension-active.toml` |
| **Category: placement** | | | | |
| `placement/bottom-right` | Window placed in bottom-right corner with margins | Both | **Covered** | `verify/placement/bottom-right.toml` |
| `placement/top-left` | Window placed at top-left anchor | Both | **Planned** | `verify/placement/top-left.toml` |
| `placement/center` | Window centered inside display workarea | Both | **Planned** | `verify/placement/center.toml` |
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
