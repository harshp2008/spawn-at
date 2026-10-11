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

### Empirical Calibration Data (Live Wayland Session, GNOME 46)

Measured across repeated executions on host `harsh` (Wayland, Ubuntu 24.04 GNOME Shell 46):

| Test ID | Artificial Delay | Repetitions | Min Cloak Duration | Median Cloak Duration | Max Cloak Duration | Status |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| `cloak/delayed-paint/delay-0` | 0 ms | 10 | **18.34 ms** | **410.55 ms** | **860.20 ms** | Pass |
| `cloak/delayed-paint/delay-50` | 50 ms | 10 | **18.20 ms** | **471.02 ms** | **953.15 ms** | Pass |
| `cloak/delayed-paint/delay-150` | 150 ms | 10 | **19.12 ms** | **577.81 ms** | **1153.23 ms** | Pass |
| `cloak/delayed-paint/delay-500` | 500 ms | 10 | **17.62 ms** | **911.78 ms** | **951.86 ms** | Pass |
| `placement/bottom-right` | 0 ms (normal) | 30 | **16.05 ms** | **401.10 ms** | **836.47 ms** | Pass |

### Distribution Analysis & Removal of Ungrounded Formula
Previous revisions suggested a synthetic linear budget formula:
`Budget = max(100 ms, AppPaintDelay + 50 ms)`

Empirical multi-run data proves this formula is ungrounded:
1. **Bi-Modal Compositor Latency Distribution:**
   Under GNOME Shell Wayland, cloak duration does not increase linearly with paint delay. Instead, executions cluster into two primary modes:
   - *Fast path (~16 – 35 ms):* Occurs when the client window buffers and actor mappings synchronize cleanly with the immediate compositor frame refresh cycle.
   - *Compositor scheduling delay (~400 – 950 ms):* Occurs when extension IPC synchronization or desktop event scheduling introduces a presentation pause before the `COMMIT_RESOLVED` and `UNCLOAK` events complete.
2. **Delayed-Paint Impact:**
   For `cloak/delayed-paint/delay-500`, when the window sleeps for 500 ms before painting its first frame, the cloak remains active until the client commits (median 911.78 ms). Because zero flicker is strictly enforced, the cloaked state correctly persists until the client submits its initial frame.

### Budget Policy Recommendation
1. **Advisory Budget Warning:**
   The default 100 ms threshold remains an advisory threshold in the TUI drawer (rendered as yellow `!` with `over provisional budget`). It is intentionally NOT an automated hard failure threshold.
2. **Automated Verification Pass/Fail Invariants:**
   Automated pass/fail criteria are strictly anchored in correctness invariants rather than synthetic timing limits:
   - Window must never be visible off-target (zero flicker).
   - `CLOAK` must precede actor mapping and sizing.
   - `COMMIT_RESOLVED` and position configuration must precede `UNCLOAK`.
   - Final mapped geometry must match the oracle coordinates within tolerance (`±1 px`).

### Multi-Run CLI Flakiness Detection (`--repeat`)
`spawn-at-verify run` includes native support for multi-run flakiness detection via the `--repeat <REPEAT>` flag:
- **Flag Definition:** `--repeat <REPEAT> Repeat each test N times (cap 100) for flakiness detection [default: 1]` (pre-existing since commit `0f6bb36`).
- **Behavior:** Executes the test action across $N$ repetitions. If outcomes vary across repetitions (e.g., pass vs fail), the runner marks the result as `FinalStatus::Flaky` with an explanatory diagnostic message.

