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
3. **Independent Geometric Oracle**: Expected window bounds are calculated by a standalone arithmetic oracle inside the test runner using monitor dimensions from `spawn-at query layout --json`. It never imports `spawn-at-core`, avoiding circular verification bugs.
4. **Unified Session State & Lock**: The headless core maintains the canonical test execution state with a single writer lock. Running `spawn-at-verify tui --web` serves both the local terminal and a mobile or secondary browser simultaneously against the exact same in-process session.

---

## 2. Test Definition Schema

Tests are defined in declarative TOML files stored under `verify/<category>/<test-id>.toml`.

### Field Specifications

| Field | Type | Description |
| :--- | :--- | :--- |
| `id` | `String` | Unique path-based identifier matching the relative file path (e.g. `placement/bottom-right`). |
| `title` | `String` | Short human-readable title. |
| `description` | `String` | Detailed explanation of test intent. |
| `kind` | `String` | `"auto"` (automated assertions only), `"human"` (visual review only), or `"both"` (both required). |
| `required` | `bool` | If `true`, a pass is required for platform qualification in `SUPPORT.md`. |
| `tags` | `Array<String>` | Labels: `visual`, `slow`, `multi-monitor`, `wayland-only`, `x11-only`, `gnome-only`, `legacy-gnome`, `destructive`, `manual-setup`, `needs-fixture`. |
| `requires` | `Table` | Environment constraints (`session`, `backend`, `gnome`, `monitors_min`, `scale`, `apps`). Unmet requirements cause the test to be **SKIPPED with an explicit reason**, never silently passed. |
| `subject` | `Table` | Target app to spawn: `app = "fixture"` or `"system:<command>"`, with profile and CLI args. |
| `action` | `Table` | The CLI subcommand to invoke (`spawn`, `transform`, etc.), arguments, and whether it is `repeatable` in-place. |
| `expect` | `Table` | Automated expectations: `exit_code`, `stderr_absent`, `stderr_present`, `diagnostics`, `never_visible_off_target`, and `rect` (oracle tolerance). |
| `human` | `Table` | Human verification prompts: `watch = "..."` describes the exact visual artifact to watch for. |
| `regression` | `Table` | For regression tests: `commit = "..."` and `note = "..."` linking to past issues. |

### Matrix Expansion
Tests can define a `[matrix]` block to automatically generate parameterized combinations (e.g. anchors $\times$ pivots).
- Real anchor and pivot identifiers are read directly from CLI argument definitions.
- Sub-test IDs are derived predictably: `<category>/<matrix-file>/<param1>-<param2>`.
- The loader validates that all generated IDs are globally unique.

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
├── cloak/                    # Zero-flicker cold starts, delayed paints, trace analysis
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

## 4. Selectors & Test Filtering

The runner and TUI filter tests dynamically via CLI flags:
- `--category <path>`: Matches categories by prefix (e.g. `placement/` or `cloak/`).
- `--tag <tag>`: Matches tags, with negation support (e.g. `--tag visual`, `--tag !slow`).
- `--kind <auto|human|both>`: Filters by verification type.
- `--required`: Runs only tests required for platform qualification.
- `--id <glob>`: Filters by specific test ID pattern.
- `--suite <name>`: Loads predefined suites from `verify/suites/<name>.toml`.
- `--repeat <N>`: Repeats each selected test $N$ times (1–100) with a configurable delay.

---

## 5. Results Model & Flakiness Detection

### Execution Statuses
- `pending`: Not yet run.
- `running`: Actively executing.
- `auto-pass`: All automated CLI exit codes, stderr checks, oracle geometry, and traces passed.
- `auto-fail`: One or more automated assertions failed.
- `error`: Harness problem (e.g. cannot launch fixture or query layout), kept strictly distinct from product failure.
- `skipped(reason)`: Unmet environment requirement (e.g. requires dual monitors on a single-monitor laptop).
- `blocked`: Preflight gate failed. Overridable with `--force` (triggers loud warning banner).

### Human Verdicts (Tracked Separately)
- `unset`: Awaiting human review.
- `pass`: Visual behavior confirmed correct.
- `fail`: Visual artifact or defect observed.
- `skip`: Manually skipped by reviewer.
- `flaky`: Inconsistent visual behavior noted.
- Free-text reviewer notes accompany every verdict.

### Final Reconciliation Matrix
- `kind = "auto"`: Automated status decides (human verdict is optional context).
- `kind = "human"`: Human verdict decides.
- `kind = "both"`: Passes **only** if both `auto-pass` and human `pass` are achieved. Any disagreement between automated assertions and human verdict is highlighted with high-contrast alert badges.

### Multi-Run Flakiness Detection
When running with `--repeat <N>`, if individual iterations yield mixed results across identical preconditions, the test status is automatically marked as **`flaky`**.

---

## 6. Window Ownership & Safe Cleanup Rules

To prevent accidental termination of user shells, editors, or background daemons:
1. **Startup Snapshot**: At session launch, the harness snapshots all existing window IDs via `spawn-at query windows --json`.
2. **Ownership Strictness**: The harness **never** closes pre-existing windows. It tracks only windows that were spawned during test actions.
3. **No PID or Name Killing**: Host processes (like `gnome-terminal-server`) host real user windows and the verification runner itself. The harness closes windows strictly by their specific window ID through the CLI (`spawn-at restore/defocus` or compositor protocols). Fixture helper processes spawned directly by the runner may be terminated by process handle.
4. **Concurrency Cap**: The harness tracks a maximum of 20 concurrent windows. If the cap is reached, further spawns are refused until windows are cleaned up.
5. **Execution Variants**:
   - *Rerun Fresh*: Closes previous windows owned by this test, then spawns anew.
   - *Rerun, Keep Windows*: Spawns another window without closing earlier ones (verifies placement stability under window accumulation, respecting the 20-window cap).
   - *Rerun in Place*: Re-executes repeatable actions (`transform`, `maximize`) on the existing mapped window.

---

## 7. Fixture Application

A dedicated, dev-only helper binary (`crates/spawn-at-verify/src/bin/fixture.rs`) built with `winit` and `softbuffer` to avoid heavy toolkit dependencies.

Configurable profiles:
- `plain`: Standard floating window with predictable title and app ID.
- `min-size`: Enforces strict toolkit minimum geometry constraints.
- `delayed-paint`: Delays first buffer presentation by $N$ ms (widens race conditions for cloak testing).
- `resize-after-map`: Dynamically requests a geometry change after initial mapping.
- `re-maximize`: Attempts to restore maximized state after placement.
- `never-map`: Creates an event loop but never maps a surface.
- `crash`: Intentionally exits abruptly prior to buffer attachment.
- `launcher`: Short-lived launcher process that hands off window creation to a background server before exiting.
- `single-instance`: Second invocation activates the existing window rather than mapping a new surface.

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
