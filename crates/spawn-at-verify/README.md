# spawn-at-verify

Verification engine, automated test harness, and interactive qualification TUI for `spawn-at`.

## Module Map

- `src/lib.rs`: Library crate root, feature gates, and module declarations (`#![warn(missing_docs)]`).
- `src/main.rs`: CLI binary entry point delegating to `src/cli/`.
- `src/cli/`: CLI subcommand handlers and argument parsing.
  - `src/cli/mod.rs`: Top-level CLI dispatcher and subcommand routing.
  - `src/cli/args.rs`: Clap argument specifications and value parsers.
  - `src/cli/check.rs`: `check` subcommand handler.
  - `src/cli/list.rs`: `list` subcommand handler.
  - `src/cli/report.rs`: `report` subcommand handler.
  - `src/cli/run.rs`: `run` subcommand handler.
  - `src/cli/tui.rs`: `tui` subcommand handler.
  - `src/cli/smoke_eval.rs`: `smoke-eval` internal helper for `scripts/smoke-live.sh`.
- `src/loader/`: TOML test specification discovery, parsing, and filtering engine.
  - `src/loader/mod.rs`: Loader exports and public API wiring.
  - `src/loader/filter.rs`: Combinable test filtering criteria and glob matching.
  - `src/loader/registry.rs`: Directory scanning, test discovery, and catalog management.
  - `src/loader/tree.rs`: Hierarchical test tree formatting and summary generation.
  - `src/loader/tests.rs`: Loader unit tests.
- `src/log_source.rs`: Platform-agnostic log slicing trait and system journal implementations.
- `src/preflight.rs`: Preflight build freshness verification and git status inspection.
- `src/probe.rs`: Desktop compositor probe, monitor topology, and display layout detection.
- `src/backend.rs`: Which `spawn-at` backend a run is aimed at (mirrors `SPAWN_AT_BACKEND` and the desktop), the `sway` requirement, when the GNOME journal applies, and the private-headless-Sway guard.
- `src/process.rs`: Timeout-bounded child process execution and non-blocking pipe draining.
- `src/report/`: Verification session report models, redaction, and export.
  - `src/report/mod.rs`: Report exports and public API wiring.
  - `src/report/models.rs`: Verification report and environment data structures.
  - `src/report/redaction.rs`: Window title redaction and security filtering.
  - `src/report/builder.rs`: Report assembly and JSON persistence.
  - `src/report/markdown.rs`: Human-readable Markdown summary generation.
  - `src/report/tests.rs`: Report unit tests.
- `src/schema/`: TOML test definition schemas, matrix expansion, and validation rules.
  - `src/schema/mod.rs`: Schema exports and public API wiring.
  - `src/schema/types.rs`: Test definition, action, expectation, and verdict structures.
  - `src/schema/matrix.rs`: Combinatorial test matrix expansion.
  - `src/schema/suite.rs`: Predefined test suite definition models.
  - `src/schema/v2.rs`: Load-time rules and run-time placeholder substitution for `schema_version = 2` specs (`[[setup]]`, `[[then]]`, `[[check]]`).
  - `src/schema/tests.rs`: Schema unit tests.
- `src/trace/`: Trace log event parser, lifecycle state machine, and budget checks.
  - `src/trace/mod.rs`: Trace exports and public API wiring.
  - `src/trace/models.rs`: Trace events, timestamps, and log models.
  - `src/trace/report.rs`: Lifecycle verification analysis report structures.
  - `src/trace/parser.rs`: Log parsing and event classification.
  - `src/trace/analyser.rs`: Lifecycle ordering, budget validation, and violation detection.
- `src/bin/fixture.rs`: Standalone GUI test fixture window binary entry point.
- `src/bin/fixture/`: Modular GUI test fixture implementation submodules.
  - `src/bin/fixture/mod.rs`: Module wiring and sub-export declarations.
  - `src/bin/fixture/font.rs`: Embedded 10x16 bitmap font data table (DejaVu Sans Mono Bold).
  - `src/bin/fixture/renderer.rs`: Softbuffer software frame drawing and bitmap text rendering.
  - `src/bin/fixture/window.rs`: Winit window creation, event loop, and signal handling.
- `src/oracle/`: Mathematical placement oracle and characterization tests.
  - `src/oracle/mod.rs`: Oracle exports and verification helpers.
  - `src/oracle/calculator.rs`: Deterministic geometry calculations, anchors, and pivots.
  - `src/oracle/tests/`: Oracle unit tests and golden placement vector comparisons.
    - `src/oracle/tests/mod.rs`: Test submodule declarations.
    - `src/oracle/tests/golden.rs`: Hand-computed golden placement calculations and tolerance checks.
    - `src/oracle/tests/vectors.rs`: Shared golden placement vector file cross-validation.
- `src/runner/`: Test execution engine and lifecycle models.
  - `src/runner/mod.rs`: Runner exports and public API wiring.
  - `src/runner/engine.rs`: TestRunner orchestrator and test suite run loop.
  - `src/runner/executor.rs`: Single test case execution, process management, and log slicing.
  - `src/runner/executor_v2.rs`: Execution of a version 2 test: setup, actions, checks, teardown.
  - `src/runner/owned.rs`: `SetupSession`: the fixture processes and windows a test launched itself, owned through their `Child` handles and torn down on every path; the `Control` and `ActionRunner` seams.
  - `src/runner/models.rs`: Execution status models, human verdicts, and options.
  - `src/runner/validation.rs`: Expectation validation, output matching, and argument building.
  - `src/runner/tests.rs`: Runner unit tests and model resolution checks.
- `src/tracker/`: Window tracking and attribution state machine.
  - `src/tracker/mod.rs`: Tracker exports and query helpers.
  - `src/tracker/state.rs`: WindowTracker state, PID/class attribution, and leak cleanup.
  - `src/tracker/tests.rs`: Window tracker unit tests and isolation invariants.
- `src/tui/`: Interactive Terminal UI for test execution and human review.
  - `src/tui/mod.rs`: TUI runner entry point, arguments, and terminal lifecycle guard.
  - `src/tui/logging.rs`: GNOME Shell extension debug logging control and terminal restoration.
  - `src/tui/app/`: Application state management and event processing.
    - `src/tui/app/mod.rs`: App module re-exports.
    - `src/tui/app/clipboard.rs`: Drawer content export (wl-copy, xclip, OSC 52, file save).
    - `src/tui/app/input.rs`: Keyboard event routing and navigation dispatch.
    - `src/tui/app/queue.rs`: Test queue execution, human verdict modals, and pause semantics.
    - `src/tui/app/state.rs`: TuiApp state machine, filters, categories, and selection.
  - `src/tui/views/`: Ratatui view rendering components.
    - `src/tui/views/mod.rs`: Main frame layout, header, body, footer, and modal routing.
    - `src/tui/views/builder.rs`: Aligned line assembly builder for detail drawer.
    - `src/tui/views/constants.rs`: Single shared drawer grid constants and column offsets.
    - `src/tui/views/footer.rs`: Contextual footer shortcut bars and responsive key grids.
    - `src/tui/views/modals/`: Help overlay and human review verdict dialogs.
      - `src/tui/views/modals/mod.rs`: Centered layout helpers and modal exports.
      - `src/tui/views/modals/help.rs`: Keyboard shortcut and navigation help overlay modal.
      - `src/tui/views/modals/verdict.rs`: Human review verdict input dialog modal.
    - `src/tui/views/theme.rs`: Color styling, status symbols, and formatting utilities.
    - `src/tui/views/wrapping.rs`: Tokenized string wrapping and responsive dimension folding.
    - `src/tui/views/panels/`: Main screen dashboard panels.
      - `src/tui/views/panels/mod.rs`: Panels module exports.
      - `src/tui/views/panels/description.rs`: Middle description and requirements pane.
      - `src/tui/views/panels/header.rs`: Top title banner, environment tags, and fixture warning.
      - `src/tui/views/panels/result_box.rs`: Bottom outcome status box and diagnostics.
      - `src/tui/views/panels/tests_list.rs`: Categorized test tree with status indicators and folding.
    - `src/tui/views/drawer/`: Detail inspection drawer sections.
      - `src/tui/views/drawer/mod.rs`: Drawer assembly and scroll container.
      - `src/tui/views/drawer/geometry_section.rs`: GEOMETRY table comparing expected and actual bounds.
      - `src/tui/views/drawer/header_section.rs`: Drawer title, ID, mode, and scenario description.
      - `src/tui/views/drawer/info_sections.rs`: SUBJECT, ACTION, and EXPECTATIONS sections.
      - `src/tui/views/drawer/result_section.rs`: RESULT verdict, timings, and failure breakdowns.
      - `src/tui/views/drawer/timeline_section.rs`: TIMELINE section rendering.
      - `src/tui/views/drawer/timeline_format.rs`: Timeline event parsing, formatting, and key humanisation.
- `tests/`: Integration test suites.
  - `tests/category_library_test.rs`: Test suite integrity, category validation, and filtering.
  - `tests/catalog_golden_test.rs`: The 45 existing specs and the five suites serialize and select exactly as in `tests/golden/catalog_v1.json`.
  - `tests/suite_wiring_test.rs`: Every suite category has specs or is a commented known-empty; every spec is selected by a suite; `quick` leaves out `moves-focus`.
  - `tests/run_signal_test.rs`: The real `run` binary against stand-in scripts, signalled mid-run: teardown and exit 130.
  - `tests/run_sway_guard_test.rs`: `run` aimed at Sway from a GNOME-looking environment: no GNOME journal, blocked outside the private session.
  - `tests/fixture_test.rs`: Fixture binary process lifecycle and protocol behaviors.
  - `tests/oracle_cross_check.rs`: Cross-check matrix between math oracle and CLI core placement.
  - `tests/trace_analyser_test.rs`: Comprehensive trace event lifecycle characterization suite.
  - `tests/tui_test.rs`: Root TUI integration test binary target (`mod tui;`).
  - `tests/tui/`: TUI scenario integration suites.
    - `tests/tui/mod.rs`: Harness setup, headless buffer rendering, and spec validators.
    - `tests/tui/app_flow.rs`: Navigation, filtering, and execution queue workflows.
    - `tests/tui/drawer_content.rs`: Drawer content ordering, geometry table, and token wrapping.
    - `tests/tui/drawer_keys_clipboard.rs`: Keyboard controls, clipboard export, and modal switching.
    - `tests/tui/golden_invariants.rs`: Golden file comparisons and text grid invariants.
    - `tests/tui/layout_responsive.rs`: Responsive width breakpoints and small height layouts.
    - `tests/tui/polish.rs`: UI spacing, padding, and outcome symbols polish specifications.
    - `tests/tui/v2_flow.rs`: Version 2 specs in the TUI (drawer lines, Auto and Both ownership of the window, Ctrl-C), with stand-in scripts.

## Optional tools

`spawn-at-verify` works without these, but they make the geometry check independent of the GNOME
extension under test:

| Tool | Debian/Ubuntu package | Used for |
| :--- | :--- | :--- |
| `xrandr` | `x11-xserver-utils` | counts active monitors (the independent probe is only trusted with exactly one) |
| `xprop` | `x11-utils` | reads mutter's `_NET_WORKAREA` hint from the XWayland/X11 root window |

```
sudo apt install x11-xserver-utils x11-utils
```

If either is missing, `spawn-at-verify run` prints a note on stderr naming the missing tool(s) and
the install command. Geometry checks then fall back to the workarea reported by `spawn-at query
layout` (and, failing that, the bounds the extension logged). Both are extension-derived: the
result still shows the comparison, but the drawer marks it "workarea from the extension, not
independently probed" and the run message says so. With no workarea at all the placement is not
verified and the test passes as "geometry unchecked" (exit code 2 for a required test; see
`run --help`). The `spawn-at` installer is not involved.

### Trace capture

Cloak and "never visible off target" tests read the extension's timing log from the journal
(`journalctl --user`, GNOME only). The extension has no way to report whether its logging is on
and starts with logging off after every reload, so before each such run `spawn-at-verify`
calls `SetLogging(true)` over D-Bus and waits for the extension's `SET_LOGGING enabled=true`
line to appear in the journal. If a run's log slice holds no event for the window the fixture
named, the result is not a clean pass: the test passes as "trace unchecked" (orange, exit code 2
for a required test; see `run --help`), the drawer shows `Trace  not analysed (no trace
captured)`, `Cloak  not measured` and `Budget  not measured`, and a `Reason` row says why (no
GNOME log source, the timing log could not be enabled, an empty slice, or a slice with only other
windows' events).
