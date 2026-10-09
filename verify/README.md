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
┌─────────────────┐       ┌──────────────────────────────┐       ┌─────────────────┐
│     TUI         │◄─────►│    spawn-at-verify Core      │◄─────►│     Web UI      │
│ (ratatui/crossterm)     │  - Test Loader & Selectors   │       │  (Embedded HTML/│
└─────────────────┘       │  - Independent Oracle        │       │   CSS/JS + SSE) │
                          │  - Window Ownership Tracker  │       └─────────────────┘
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
4. **Unified Session State & Lock**: The headless core maintains the canonical test execution state with a single writer lock. Running `spawn-at-verify tui --web` serves both the local terminal and a mobile or secondary browser simultaneously against the exact same in-process session.

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
| **No Commit ID** (release build) | Any state | **SKIP** | Release tarball without git metadata; skipped with reason. |

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
5. **Execution Variants**:
   - *Rerun Fresh*: Closes previous windows owned by this test, then spawns anew.
   - *Rerun, Keep Windows*: Spawns another window without closing earlier ones (verifies placement stability under window accumulation, respecting the 20-window cap).
   - *Rerun in Place*: Re-executes repeatable actions (`transform`, `maximize`) on the existing mapped window.

---

## 7. Fixture Application & Protocol Verification

The fixture helper binary (`crates/spawn-at-verify/src/bin/fixture.rs`) supports configurable window behaviors and explicit protocol control:
- **Protocol Option**: `--protocol <auto|wayland|x11>`
- **Actual Protocol Tracking**: The runner records per test window which protocol was actually used (derived from compositor trace/log or `query windows` output).
- **Strict Protocol Enforcement**: A test requiring native Wayland fails or skips if the window was mapped under XWayland.

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

## 9. TUI & Web Interface

### Terminal User Interface (TUI)
- **Left Pane**: Category tree with status glyphs (`✔`, `✖`, `●`, `⊘`) and progress counters.
- **Center Pane**: Test title, description, preconditions, execution steps, oracle expectation, watch text, automated result, human verdict, and run count.
- **Right/Bottom Pane**: Live log viewer with "spawn-at only" filter toggle, auto-scroll, and search.
- **Status & Footer**: Key binding hints, unverified required test counters, and preflight status.
- **SSH Copy Support**: Copies logs using OSC 52 escape sequences and writes temporary disk logs for remote retrieval.

### Web UI
- Served via `spawn-at-verify serve --bind 127.0.0.1:<PORT>` or concurrently inside `tui --web`.
- **Security**:
  - Bound to loopback by default.
  - Any non-loopback bind requires a high-entropy startup token printed to console and validated via header/cookie.
  - Constant-time token comparison prevents timing attacks.
  - Strict `Host` header and `Origin` validation prevents DNS rebinding and cross-site request forgery.
  - Fixed action endpoints only: cannot execute arbitrary commands or file paths.
- **Mobile Friendly**: Large touch targets for phone review while watching the physical monitor.
- **Clipboard Fallback**: Textarea select-all and download links for HTTP connections where `navigator.clipboard` is unavailable.

---

## 10. Reporting & Support Qualification

Test outputs are recorded under `verify-results/<timestamp>-<host>/`:
- `report.json`: Full machine-readable execution trace.
- `report.md`: Markdown summary report.
- `slices/`: Individual per-test journal and trace logs.
- **Privacy**: Window titles are redacted by default; opted into with `--include-titles`.

### Platform Support Qualification
A platform or desktop environment may only be documented as **Supported** in `SUPPORT.md` if **100% of required tests pass** (`auto-pass` and human `pass` where `kind = "both"`). `spawn-at-verify report --support-row` formats verified candidate rows.

