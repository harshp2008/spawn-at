# `spawn-at-verify`: Verification Engine & Test Harness

`spawn-at-verify` is an automated and human-in-the-loop verification harness designed specifically for `spawn-at`. It verifies that window positioning is mathematically accurate and, above all, that window mapping is completely flicker-free—ensuring windows never flash at arbitrary coordinates before snapping into position.

---

## 1. Architecture

The verification suite lives in `crates/spawn-at-verify` as a developer-only workspace crate. It is never included in the production `spawn-at` binary, user installer, or release packages.

```
                           ┌──────────────────────────────┐
                           │    verify/ Test Library      │
                           │  (TOML files in categories)  │
                           └──────────────┬───────────────┘
                                          │
                                          ▼
┌─────────────────┐       ┌──────────────────────────────┐
│     TUI         │◄─────►│    spawn-at-verify Core      │
│ (ratatui/crossterm)     │  - Test Loader & Selectors   │
└─────────────────┘       │  - Independent Oracle        │
                          │  - Window Ownership Tracker  │
                          │  - Event Bus & State Sync    │
                          └──────────────┬───────────────┘
                                         │ drives via public CLI only
                                         ▼
                          ┌──────────────────────────────┐
                          │    spawn-at CLI Under Test   │
                          │  `spawn-at spawn --json` ... │
                          └──────────────┬───────────────┘
                                         │
                   ┌─────────────────────┴─────────────────────┐
                   ▼                                           ▼
      ┌─────────────────────────┐                 ┌─────────────────────────┐
      │   Fixture Application   │                 │   Compositor Extension  │
      │   (winit + softbuffer)  │                 │   (Structured Traces)   │
      └─────────────────────────┘                 └─────────────────────────┘
```

### Core Principles
1. **Black-Box CLI Invocation**: The harness drives `spawn-at` exclusively through its public CLI interface (`spawn-at spawn`, `transform`, `focus`, `query --json`, etc.) with `--bin <PATH>` override support. It never links or inspects internal Rust modules of `spawn-at`.
2. **Platform Abstraction Traits**:
   - `LogSource`: Captures system logs (e.g. `journalctl` on GNOME; no-op stubs on platforms without system log integration).
   - `TraceSource`: Reads structured compositor actor traces (`[spawn-at-trace] {json}`).
   - `EnvProbe`: Inspects session type (`wayland` vs `x11`), desktop environment, monitors, scale, and kernel.
   Platforms missing these traits build cleanly and skip dependent tests with clear, explicit skip reasons.
3. **Independent Geometric Oracle**: Expected window bounds are calculated by a standalone arithmetic oracle inside the test runner using monitor dimensions from `spawn-at query layout --json`.
   - **Semantic Authority**: The oracle follows the documented semantics of anchor, pivot, and margin from project documentation, **not from the implementation code**.
   - If the oracle ever disagrees with `spawn-at`, the failure must be flagged and reported; the oracle is never altered to accommodate diverging code behavior.
4. **Unified Session State & Lock**: The headless core maintains the canonical test execution state with a single writer lock. The terminal UI is the only interactive front end: there is no web interface (see section 9).

---

## 2. Test Definition Schema

Tests are defined in declarative TOML files stored under `verify/<category>/<test-id>.toml`.

### Field Specifications

| Field | Type | Description |
| :--- | :--- | :--- |
| `id` | `String` | Base path-based identifier matching the relative file path without `.toml` (e.g. `placement/bottom-right`). |
| `title` | `String` | Short human-readable title. |
| `description` | `String` | Detailed explanation of test intent. |
| `kind` | `String` | `"auto"` (automated assertions only), `"human"` (visual review only), or `"both"` (both required). |
| `required` | `bool` | If `true`, a pass is required for platform qualification in `SUPPORT.md`. |
| `tags` | `Array<String>` | Orthogonal test tags (e.g. `visual`, `slow`, `multi-monitor`, `destructive`, `manual-setup`, `needs-fixture`). **Note**: Platform tags (`wayland-only`, `x11-only`, `gnome-only`) are **derived automatically from `[requires]`** as the single source of truth and must not be duplicated here. |
| `requires` | `Table` | Environment constraints (`session`, `backend`, `gnome`, `monitors_min`, `scale`, `apps`). Unmet requirements cause the test to be **SKIPPED with an explicit reason**, never silently passed. Defaults to `session = ["any"]`, `backend = ["any"]`. |
| `subject` | `Table` | Target app to spawn: `app = "fixture"` or `"system:<command>"`, with profile and CLI args. |
| `action` | `Table` | The CLI subcommand to invoke (`spawn`, `transform`, etc.), arguments, and whether it is `repeatable` in-place. |
| `expect` | `Table` | Automated expectations: `exit_code`, `stderr_absent`, `stderr_present`, `diagnostics`, `never_visible_off_target`, and `rect` (oracle tolerance). |
| `human` | `Table` | Human verification prompts: `watch = "..."` describes the exact visual artifact to watch for. |
| `regression` | `Table` | For regression tests: `commit = "..."` and `note = "..."` linking to past issues. |

### Diagnostic Label Matching (`stderr_absent` / `stderr_present`)
- Entries in `stderr_absent` and `stderr_present` match **exact diagnostic label tokens** by default:
  - `"[spawn-at] ERROR:"`
  - `"[spawn-at] WARNING:"`
  - `"[spawn-at] INFO:"`
- Matching is case-sensitive literal substring comparison.
- **Regex Opt-in**: To use regular expression matching, prefix the pattern with `re:` (e.g. `re:.*failed to connect.*`).

### Matrix Expansion
Tests can define a `[matrix]` block to automatically generate parameterized combinations (e.g. initial buffer delay or anchors $\times$ pivots).
- **ID Formation**: The file's `id` is the base ID; each expansion produces:
  `<base>/<combination>`
  For example, base `cloak/delayed-paint` with `delay = [0, 50, 150, 500]` expands to:
  - `cloak/delayed-paint/delay-0`
  - `cloak/delayed-paint/delay-50`
  - `cloak/delayed-paint/delay-150`
  - `cloak/delayed-paint/delay-500`
- Real anchor and pivot identifiers are read directly from CLI argument definitions.
- The loader validates that all generated IDs are globally unique, rejecting duplicate expansions or unknown fields with file and line context.

### Version 2 specs: tests of an existing window
A spec without `schema_version` is version 1 (every spec in the catalog today): one `[action]`, launched through `spawn`. A spec with `schema_version = 2` may launch its **own fixture windows**, act on them **by the id the runner recorded**, assert on them and have them torn down. Design and reasons: `docs/DESIGN_RUNNER_SETUP.md`.

```toml
schema_version = 2
id = "transforms/resize-by-id"
category = "transforms"
title = "Resize an existing window by id"
description = "Launches the fixture at 400x300, resizes that window to 600x450 centred, and checks the rectangle."
kind = "auto"
tags = ["needs-fixture", "moves-focus"]      # moves-focus is required whenever there is a [[setup]]

[requires]
backend = ["gnome", "sway"]                  # "sway" is a value of its own

[[setup]]                                    # up to 3; the runner starts the fixture itself
name = "w"                                   # ${w} is its recorded window id
args = ["--width", "400", "--height", "300"] # whitelisted fixture flags only
# timeout_ms = 5000                          # longest wait for the window to appear

[action]                                     # the measured action: its exit code, stderr, stdout, rectangle are judged
command = "transform"
args = ["--id", "${w}", "--size", "600", "450", "--anchor", "center", "--margin", "0", "--no-focus"]

[[then]]                                     # optional further actions, each must exit 0
command = "restore"
args = ["--id", "${w}", "--no-focus"]

[[check]]                                    # assertions on a recorded window, looked at again for up to 2 s
after = 0                                    # 0 = after [action], 1 = after the first [[then]]; default = after the last
window = "w"
state = "maximized"                          # present | gone | maximized | minimized | normal | focused | unfocused
# same_rect_as = "before"                    # or: equal (2 px) to the rectangle read before [action]
# process_exited = true                      # or: the fixture process this setup started has ended

[expect]
exit_code = 0
stdout_present = ["\"id\": ${w}"]            # substrings of stdout; ${...} allowed
stdout_lists_window = "w"                    # stdout is a window list holding w (same id and unique title)
[expect.rect]                                # the oracle, judged on the recorded window re-read after the action
anchor = "center"
size = [600, 450]
margin = 0
```

**Rules the loader enforces** (`crates/spawn-at-verify/src/schema/v2.rs`; each has a test):
- `[[setup]]`, `[[then]]`, `[[check]]`, `stdout_present`, `stdout_lists_window` need `schema_version = 2`; versions other than 1 and 2 are refused; no `[subject]`.
- Commands are `focus`, `defocus`, `transform`, `maximize`, `minimize`, `restore`, `close`, `query`. Not `spawn`, `install`, `config`.
- **The only selector is `--id ${name}`** with `name` a declared setup (or `${nonexistent}`, an id the runner has just proved is not a window). A literal id, `-c`, `-t`, `--pid`, `--focused` (also as `--class=x` or `-cx`), `--id=...`, `--`, a bare `defocus`, and `defocus --to-window` with anything but `${name.title}` are load errors.
- Setup flags are whitelisted (`--width --height --min-width --min-height --decorations --paint-delay-ms --expect --state-colors` and the hostile-app flags). `--title`, `--app-id`, `--exit-after-ms`, `--protocol` belong to the runner.
- At most 3 setups; names are lower-case, unique, not `nonexistent`, not a `[matrix]` key. A `[[check]]` names a declared window and exactly one of `state`, `same_rect_as`, `process_exited`.
- Not available in version 2: `never_visible_off_target`, `[expect.resize]`, `diagnostics`; `[expect.rect]` needs a setup.
- **A spec with a `[[setup]]` must carry the tag `moves-focus`**, because a new fixture window takes focus. The `quick` suite excludes the tag.

**Results.** A setup that fails, a target that changed, a failed query and an interrupt end `Error`, never `Fail`: nothing about the product was judged. A command with the wrong exit code, a check that does not hold and a rectangle that is off are `Fail`. A spec whose `[requires] backend` does not include the selected backend is **Skipped** with the fixed text `unsupported on this backend (<backend>): spec requires <list>`; the backend is read the way the binary reads it, from `SPAWN_AT_BACKEND` (`x11` or `sway`, lower-cased, nothing trimmed) and `XDG_CURRENT_DESKTOP`.

### Running on a private headless Sway
`scripts/verify-sway.sh [-- <run arguments>]` starts a private headless Sway through `scripts/sway-headless.sh`, runs `spawn-at-verify run` against it (`SPAWN_AT_BACKEND=sway`; `DISPLAY`, `XDG_CURRENT_DESKTOP` and `DBUS_SESSION_BUS_ADDRESS` removed from the runner's environment) and stops that Sway. With no arguments it runs the specs tagged `headless-safe`. The runner also refuses (every test `Blocked`) when `SWAYSOCK` is not inside a `spawn-at-sway.*` directory with the harness marker, owned by you and not the real runtime directory. On a forced non-GNOME backend the runner does not use the GNOME journal or call `SetLogging` on the extension. Flash and trace checks do not exist on Sway.

### Ctrl-C, SIGTERM and SIGHUP in `run`
`spawn-at-verify run` stops starting tests on any of these signals, closes the windows it opened (by id) and stops the fixture processes it started, writes the report for what ran, and exits with **130**. An interrupted repetition is dropped, not counted as flaky. The wait inside one `spawn-at` action (up to the 5 s step timeout) is not interrupted.

### The specs for an existing window (22 entries, 18 files)
All are `schema_version = 2`, `required = false`, launch only the runner's own fixture (`[[setup]]`) and act with `--id ${name}`. Every spec that launches a window has the tag `moves-focus` (so `quick` leaves it out); `existing-window` marks the group; `headless-safe` marks the ones that can run unattended on a private headless Sway (`scripts/verify-sway.sh`). None carries the tag `visual`, so the `visual` suite is unchanged. They are in `full` (the suite aliases `transform`, `query`, `resilience` and `lifecycle` select the new `transforms/`, `queries/`, `robustness/` and `focus/` categories) and not in `release-gate` (nothing is required).

| Id | Kind | Backends | Unattended on headless Sway |
| :-- | :-- | :-- | :-- |
| `queries/query-windows-lists-fixture` | Auto | gnome, sway | yes |
| `queries/query-layout-json` (no window) | Auto | gnome, sway | yes |
| `queries/query-pointer` | Auto | gnome | n/a (Sway cannot read the pointer) |
| `transforms/resize-by-id` | Auto | gnome, sway | yes |
| `transforms/reposition-by-id` (position only) | Auto | gnome | n/a (a fresh tiled Sway window is work-area sized, so "size unchanged" is not 400x300) |
| `transforms/anchor-corners/anchor-{top-left,top-right,bottom-left,bottom-right,center}` (one file, a matrix) | Auto | gnome, sway | yes |
| `transforms/resize-flash`, `transforms/move-flash` | **Both** | gnome | n/a. They check the final rectangle and ask you to judge by eye; on GNOME the run also records the extension's timeline and an informational flash check (see "What is traced" below) |
| `transforms/maximize-and-restore`, `transforms/minimize-and-restore` | **Both** | gnome | n/a (Sway declares them unsupported) |
| `focus/activate-fixture`, `focus/switch-between-windows` (two windows) | Auto | gnome, sway | yes |
| `focus/defocus-active-window` | Auto | gnome | n/a (Sway: defocus unsupported) |
| `robustness/close-by-id` (window gone and the fixture process ended) | Auto | gnome, sway | yes |
| `robustness/nonexistent-window-id` (no window launched) | Auto | gnome, sway | yes |
| `robustness/sway-defocus-unsupported`, `robustness/sway-maximize-unsupported`, `robustness/sway-query-pointer-unsupported` | Auto | sway | yes |

#### What is traced for these specs

On GNOME, `run` and the TUI record, for every action of these specs, the extension's own log lines for the windows the runner launched, and show them as a timeline (event, ms after the first line of the action, key fields such as `frame=(...)`) in the drawer's TIMELINE section, the Result panel and the `run` report (`report.json` key `action_traces`, `report.md` section 7). **The TUI itself still writes no report.** The extension logs a trace for some commands only; the table is from the extension source (`docs/DESIGN_RUNNER_SETUP.md` section 5.5 has the file:line citations):

| Command | In the timeline | Cloak / Budget rows |
| :-- | :-- | :-- |
| `transform` | full: `BATCH_STEP`, `SNAPSHOT_*`, `CLOAK`, `SET_SIZE_REQUEST`, `READY_WAIT_*`, `COMMIT_*`, `POSITION_SET`, `UNCLOAK`, `UNCLOAK_4D_REVEAL`, `ANCHOR_ARMED` | cloak to reveal in ms, judged against the provisional 100 ms |
| `focus` | `RAISE`, `ACTIVATE` | `n/a`, no flash analysis (no cloak phase) |
| `minimize`, `unminimize`, `restore` | `MINIMIZE` / `UNMINIMIZE` only | `n/a` |
| `maximize`, `unmaximize`, `defocus`, `close`, queries | nothing for the target (`ANCHOR_RELEASED` if an anchor was armed) | `n/a` |

Reasons shown instead of a timeline: `not applicable: the extension logs nothing for this command`; `no extension on this backend (<backend>)` (Sway and X11 have no extension); `capture failed: <why>` (journal unreadable, timing log could not be enabled); `no lines for this window in the slice`. None of them changes a verdict, and a missing or empty slice never fails a test.

**Only lines of the windows the runner launched are kept.** The extension writes lines for every window on the desktop (`WINDOW_CREATED`, `ACTOR_MAP`, ...), so the runner filters by the recorded window ids before it stores, shows or writes anything. Capture is `RunOptions::action_trace`: on for `run` and the TUI, off for a bare `TestRunner`, so `cargo test` never touches the live extension. It reuses the timing-log enable every run makes and, for a `transform`, the same four bracketed `SetLogging` calls a fixture `spawn` makes.

**Transform flash is informational.** The fixture of a `transform` test is started with the event directory, and `detect_flash` compares its paint events from the action's start with the extension's reveal, with the same `certain` (at least one frame) and `possible` labels as `spawn`. It never changes a verdict (no spec opts in; `--strict-flash` does not apply to version 2 specs); a flash shows as `PASS (flash)`. **No pixels are captured. A clean result means no late paint or move was found in the runs made, not that none can happen. The `possible` bucket has a known noise level** (a lead below one frame period). Offsets are on the extension's clock, which is stamped at the event; the journal's own timestamp is the flush time and is not used.

A spec on a backend it does not list is **Skipped** with `unsupported on this backend (<backend>): spec requires <list>` (shown as `⊘` in the TUI list, `SKIPPED` and the reason in the result box).

**Unattended, private headless Sway only** (starts and stops its own Sway; nothing of your desktop is touched): `scripts/verify-sway.sh` runs every `headless-safe` spec (15 entries). `scripts/verify-sway.sh -- --tag existing-window` runs all 22 and shows the GNOME-only ones as Skipped.

**On GNOME, by hand, attended, when the desktop is idle** (these specs open fixture windows that take focus and move them; never started automatically, and `quick` does not include them):
1. `cargo build -p spawn-at -p spawn-at-verify --features spawn-at-verify/fixture` on a clean tree, so the binary matches `HEAD` (otherwise every test is Blocked as a stale binary).
2. The Auto specs: `target/debug/spawn-at-verify run --tag existing-window --kind auto`. You will see small windows titled `verify-<pid>-<time>-<n>-<spec>-<name>` appear, take focus, move, resize, change focus and close, one test after another; focus returns to the window you were in. The three Sway contract specs show as Skipped. The result and a report are written under `verify-results/`.
3. The Both specs: `target/debug/spawn-at-verify tui --tag human-review` (the `tui` command has `--tag` but no `--kind`). Select a spec, press `r`. The window stays open after the automatic part; answer the question shown with `p` (pass), `x` (fail), `s` (skip) or `k` (flaky); the window closes when you answer. (`run` leaves a Both window open until the end of the run, so use the TUI for them.)
4. Ctrl-C at any time closes what the run opened.
GNOME behaviour of these specs has not been run by the author of this change (no live GNOME test was allowed); the first run on GNOME is the first evidence.

---

## 3. Directory Structure & Categories

```
verify/
├── README.md                 # This specification
├── preflight/                # Environment fingerprints & version match guards
│   ├── README.md
│   └── binary-head.toml
├── placement/                # Anchor/pivot math, bounds, margins, and clamping
│   ├── README.md
│   └── bottom-right.toml
├── cloak/                    # Zero-flicker cold starts, delayed paint & traces
│   ├── README.md
│   └── delayed-paint.toml
├── launchers/                # Forking launchers, daemon handoffs, single-instance reuse
│   └── README.md
├── apps/                     # Real-world desktop application compatibility
│   └── README.md
├── transform/                # Runtime geometry repositioning & resizing
│   └── README.md
├── lifecycle/                # Focus, defocus, maximize, minimize, restore
│   └── README.md
├── query/                    # Layout, windows, and pointer query verification
│   └── README.md
├── diagnostics/              # Level colors, warning/info accuracy, NO_COLOR
│   └── README.md
├── resilience/               # Missing extension, protocol timeouts, error handling
│   └── README.md
├── regression/               # Historical bug fixes linked to commit hashes
│   └── README.md
└── suites/                   # Predefined suite definitions
    ├── README.md
    ├── quick.toml
    ├── full.toml
    ├── visual.toml
    ├── regression.toml
    └── release-gate.toml
```

---

## 4. Preflight Gate & Binary Freshness

The preflight check runs first to prevent testing stale or out-of-sync binaries:

| Binary State | Repo Git State | Preflight Verdict | Meaning / Action Required |
| :--- | :--- | :--- | :--- |
| **Clean** (matches HEAD) | Clean working tree | **PASS** | Binary is fresh and matches current committed tree. |
| **Clean** | Newer HEAD or dirty working tree | **FAIL** | Stale binary; rebuild before verifying. |
| **Dirty** (e.g. `20f032e-dirty`) | Any state | **WARN** | Built from uncommitted changes; flagged as "dirty build". |
| **Release version** (`spawn-at 0.2.0-beta.4`, no commit ID) | HEAD is exactly tag `v0.2.0-beta.4`, clean tree | **PASS** | Binary was built on that tag (`git describe --tags --exact-match HEAD`). |
| **Release version** | HEAD untagged or tagged differently, or dirty tree | **FAIL** | Same stale-binary message as for a commit mismatch. |
| **Unrecognised `-V` output** | Any state | **FAIL** | Neither `DEV BUILD` with a commit ID nor `spawn-at <version>`. |

### Dirty Build Policy
- A binary flagged as dirty may execute development tests.
- However, the session report is permanently marked with `"dirty build"`.
- A dirty-build session **can never qualify an environment for `SUPPORT.md`**.
- The `release-gate` suite strictly refuses to run with a dirty build unless explicitly overridden with `--allow-dirty` (which is recorded in the report).

---

## 5. Results Model & Qualification

### Execution Statuses
- `pending`: Not yet run.
- `running`: Actively executing.
- `auto-pass`: All automated CLI exit codes, stderr checks, oracle geometry, and traces passed.
- `auto-fail`: One or more automated assertions failed.
- `error`: Harness problem (e.g. cannot launch fixture or query layout), kept strictly distinct from product failure.
- `skipped(reason)`: Unmet environment requirement.
- `blocked`: Preflight gate failed (overridable with `--force`).

### Human Verdicts (Tracked Separately)
- `unset`: Awaiting human review.
- `pass`: Visual behavior confirmed correct.
- `fail`: Visual artifact or defect observed.
- `skip`: Manually skipped by reviewer.
- `flaky`: Inconsistent visual behavior noted.

### Qualification Rule
- **Skipped is NEVER counted as a pass.**
- A `required` test that was skipped (for example, cloak tests when trace collection is not yet supported) prevents an environment from being marked as supported.
- Reports must explicitly list all `required-but-skipped` tests with their skip reasons.

---

## 6. Window Ownership & Safe Cleanup Rules

1. **Startup Snapshot**: At session launch, the harness snapshots all existing window IDs via `spawn-at query windows --json`.
2. **Ownership Strictness**: The harness **never** closes pre-existing windows. It tracks only windows spawned during test actions.
3. **No PID or Name Killing**: Host processes (`gnome-terminal-server`, etc.) are never terminated by PID or process name. Closing test windows uses `spawn-at close --id <ID>` only, for windows the harness itself launched (ownership rules unchanged). For fixture processes the harness spawned itself, it may terminate its own child process directly. A close that leaves the window present is reported in the run, and the cap on tracked windows still applies.
4. **Concurrency Cap**: The harness tracks a maximum of 20 concurrent windows. If the cap is reached, further spawns are refused until windows are cleaned up.
5. **Version 2 specs own their windows** (`docs/DESIGN_RUNNER_SETUP.md` section 3): the runner starts the fixture itself and holds its process handle; a window is recorded only when exactly one listed window has the fixture's unique title (`verify-<pid>-<time>-<n>-<test>-<name>`) and the child's pid; every action is preceded by a re-check that the recorded id still has that title and pid; a window is closed by its id only while the id still carries that title; a signal is sent only through the unreaped child handle (SIGTERM, then SIGKILL), never to a pid read from `query windows`; teardown runs on a normal end, a setup error, a timeout, Ctrl-C/SIGTERM/SIGHUP and a panic, and a Both or Human test keeps its window until the verdict (the tracker tears it down). The fixture is always started with `--exit-after-ms` (10 s, or 5 min for Both and Human), so a killed runner leaves a fixture that ends itself. After teardown the focus goes back to the window that had it at the start, if it still exists and lost focus. A window still listed after teardown is a warning in the result message, not a failure.
6. **Execution Variants**:
   - *Rerun Fresh*: Closes previous windows owned by this test, then spawns anew.
   - *Rerun, Keep Windows*: Spawns another window without closing earlier ones (verifies placement stability under window accumulation, respecting the 20-window cap).
   - *Rerun in Place*: Re-executes repeatable actions (`transform`, `maximize`) on the existing mapped window.

---

## 7. Fixture Application & Protocol Verification

The fixture helper binary (`crates/spawn-at-verify/src/bin/fixture.rs`) supports configurable window behaviors and explicit protocol control:
- **Protocol Option**: `--protocol <auto|wayland|x11>`
- **Actual Protocol Tracking**: The runner records per test window which protocol was actually used (derived from compositor trace/log or `query windows` output).
- **Strict Protocol Enforcement**: A test requiring native Wayland fails or skips if the window was mapped under XWayland.
- **Bounded Lifetime Backstop**: The runner always injects `--exit-after-ms 10000` to fixture spawns if not specified in test arguments. This ensures that even under an abrupt test harness crash or `kill -9`, fixture processes automatically self-terminate after 10,000ms (comfortably outlasting the default 5s step timeout).
- **Missing Binary Guard**: If a test requires the fixture and `target/debug/fixture` is missing, it fails fast with a clear diagnostic instructing the user to build it via `cargo build -p spawn-at-verify --bin fixture --features fixture`.

---

## 8. Flicker & Cloak Trace Measurement

The GNOME Shell extension emits structured JSON events when logging is enabled via `SetLogging(true)`:
```text
[spawn-at-trace] {"timestamp":1791541272000,"window_id":101,"event":"map","geometry":[0,0,800,600],"opacity":0,"cloaked":true}
[spawn-at-trace] {"timestamp":1791541272010,"window_id":101,"event":"position","geometry":[1200,600,800,600],"opacity":0,"cloaked":true}
[spawn-at-trace] {"timestamp":1791541272015,"window_id":101,"event":"reveal","geometry":[1200,600,800,600],"opacity":255,"cloaked":false}
```

### Trace Assertions
1. **Off-Target Invisibility**: From initial map to reveal, window opacity must remain 0 whenever geometry differs from target coordinates.
2. **Single Reveal**: The `reveal` event must occur exactly once per placement lifecycle.
3. **No Lingering Cloaks**: If an operation succeeds, fails, or times out, all cloak state and zero-opacity overrides must be fully cleared.
4. **Scope Limitation**: Structured traces verify compositor actor lifecycle and event ordering as seen by GNOME Shell. They do not perform pixel-level framebuffer capture.

---

## 9. TUI

### Terminal User Interface (TUI)
Run with real fixture cases:
```bash
cargo run -p spawn-at-verify --bin spawn-at-verify --features tui,fixture -- tui
# or equivalently (supported via default-run in Cargo.toml):
cargo run -p spawn-at-verify --features tui,fixture -- tui
```

- **Missing Fixture Warning Banner**: If the fixture binary is missing, the TUI displays a prominent warning banner at the top instructing how to build it (`cargo build -p spawn-at-verify --bin fixture --features fixture`).
- **Keybindings**:
  - `Enter` / `d`: Toggle Detail Drawer (expands to full scrollable view)
  - `j` / `Down`, `k` / `Up`: Navigate test cases in table, or scroll detail drawer line-by-line
  - `PageDown` / `PageUp`: Scroll detail drawer by page
  - `[` / `]`: Scroll detail drawer while remaining in table view
  - `Home`: Reset scroll to top of detail drawer
  - `r`: Execute currently selected test
  - `a`: Execute all tests matching current filter
  - `v`: Open human visual review verdict modal (`[P]` Pass, `[F]` Fail, `[S]` Skip, `[K]` Flaky, `[Esc]` Cancel)
  - `f`: Cycle filter modes (`All` -> `Human Only` -> `Auto Only` -> `Failed Only`)
  - `q` / `Ctrl+C`: Graceful teardown
- **Selected-row marker**: the status marker of the selected row is drawn in a colour that differs from the row's `DarkGray` highlight in every state (not run: `White` on the selected row, `DarkGray` elsewhere; running Cyan, waiting/unchecked Yellow, pass Green, fail Red, skipped/blocked Magenta, flash and trace-unchecked as in the legend). A headless test renders the selected row in nine states and compares the marker cell with the row background.
- **Verdict keys answer at once**: `p`, `x`, `s`, `k` update the row marker and the Result box in the very next redraw. The window is closed after that frame; the Result box adds `closing window...` until it is. Nothing the teardown finds (a window that stays open, a close that fails) changes the verdict: it is shown after the verdict as a `⚠` warning. `Esc` records no verdict and closes the window; `r` resets the row to running, closes the window and runs the test again. A run ends with exactly one verdict per test.
- **`r` shows "Running" before the work starts**: a key press only records what you asked for; the automatic part (launching the fixture window, the actions, the checks, the settle wait) runs after the next redraw, because it blocks the TUI for as long as it takes (about 190 ms against the test doubles, more against a real compositor). The row and the Result box show "Running" during it; the keys are read again when it ends.
- **Category order of the Tests list** (defined once, in `category_rank`, `tui/app/state.rs`): PREFLIGHT, FOCUS, QUERIES, ROBUSTNESS, PLACEMENT, CLOAK & DELAYED PAINT, TRANSFORMS, GENERAL (the hostile-app cells). Why: the binary first; focus second because every attended spec needs focus to move and come back; the read-only window list next (ownership checks and teardown rely on it); the safety paths (close by id, bad ids), automatic, before anything attended; then the attended groups together so the terminal loses focus in one stretch; the riskiest cells last. `tests/tui/sidebar_order.rs` asserts the order built from the real catalog.
- **Focus-return diagnostics (every attended test, always shown in the Result box)**: after the automatic part of a `Both`/`Human` test of any schema version, and again after the verdict closes the window, the TUI returns focus and shows three facts: `terminal: id N class '...' title '...'` (the window that had focus when the TUI started, or `none found (<why>)`: the window query failed, or no window was reported focused with N listed), `focus command: sent Kx: #1 exit C 'message'` (or `not sent (<why>)`), and `focused afterwards: id N class '...' title '...' - matches terminal: yes|NO`. The focus is checked after a 150 ms settle, sent up to 3 times with the window list polled for 600 ms after each, so a window that takes focus late or a command applied late is handled. On GNOME, paste those lines if it fails. Version 1 specs (placement, cloak) had no return at all before this; only version 2 specs did. Covered on a private headless Sway by `scripts/focus-chain-sway.sh` (the ignored test `tests/tui/focus_chain_sway.rs`); not run on GNOME.
- **Focus returns to the terminal for the verdict**: when the automatic part of a `Both` or `Human` test ends and its fixture window stays open, the runner gives focus back to the window that had it when the test started (normally the terminal), so `p`/`x`/`s`/`k` reach the TUI and not the fixture. Only a `focus` command is ever sent to that window; it is never closed, moved, resized, minimized or used as an action target (a spec's `--id` naming it is refused). The runner then lists the windows (read only) and, if that window is not focused, the Result box shows `⚠ could not return focus to your terminal` (the verdict is not affected; click the terminal, or answer from the keyboard once it has focus). If no window had focus at the start, nothing is done. After the verdict the same code runs once more in teardown and does nothing if focus is already there. **Caveat:** on GNOME focusing a window raises it. The fixture opens at 400x300 where the compositor puts it; a terminal that is maximized, or large and centred, can cover it when it is raised. If the window you must judge is hidden, move or shrink the terminal so the two do not overlap (or place it on the other half of the screen) before pressing `r`.
- **Detail Drawer Structure**: Orders content with failure reason first at the top, followed by geometric oracle expected vs actual geometry comparison, compositor actor trace violations/warnings, captured stdout/stderr, human review watch instructions, and test metadata.
- **Raw Mode Ctrl+C & Signal Safety**: In terminal raw mode, `Ctrl+C` arrives as a crossterm key event. Both `Ctrl+C` and external `SIGTERM`/`SIGHUP` signals execute the exact same unified teardown:
  1. Restores terminal raw mode, leaves alternate screen, and restores cursor visibility.
  2. Safely cleans up test-spawned windows via `spawn-at close --id <ID>`.
  3. Faithfully restores the initial GNOME Shell extension logging state (recorded at startup from `session.log` / `SPAWN_AT_DEBUG`); never blindly disables logging if the user had it turned on.

### No web interface
There is no web UI. `spawn-at-verify --help` lists `run`, `list`, `check`, `report`, `tui`, `calibrate` and `render`; `spawn-at-verify serve` is "unrecognized subcommand" and `spawn-at-verify tui --help` has no `--web` option (checked 2026-10-15). An earlier version of this file described a loopback web server with a token; none of it exists in the code.

---

## 10. Reporting & Support Qualification

Test outputs are recorded under `verify-results/<timestamp>-<host>/` (`<timestamp>-<host>-02`, `-03`, ... when another run started in the same second):
- `report.json`: Full machine-readable execution trace.
- `report.md`: Markdown summary report.
- `slices/`: Individual per-test journal and trace logs.
- **Privacy**: Window titles are redacted by default; opted into with `--include-titles`.

**Which commands write these files.** Only `spawn-at-verify run` and `spawn-at-verify calibrate` write `report.json`, `report.md` and `slices/` (both call `write_report_artifacts`, `crates/spawn-at-verify/src/cli/run.rs` and `cli/calibrate.rs`). `spawn-at-verify report [PATH]` only re-renders a report that already exists. **The TUI (`spawn-at-verify tui`) writes no report**: results stay in memory and on screen, and the only file it writes is the text of the open detail drawer, saved with `w` as `verify-results/<test-id with / replaced by ->.log.txt`. A TUI session therefore leaves no `report.md`, and a `both` test still waiting for a human verdict is recorded nowhere unless the same tests are run with `run`. (Read from the code on 2026-10-15; the TUI was not run for this note.)

### Platform Support Qualification
A platform or desktop environment may only be documented as **Supported** in `SUPPORT.md` (that file does not exist in the repository yet) if **100% of required tests pass** (`auto-pass` and human `pass` where `kind = "both"`). `spawn-at-verify report --support-row` formats verified candidate rows.

