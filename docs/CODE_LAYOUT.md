# Code layout

What is in the repository now. The file list below is generated from the tracked files: line counts are `wc -l`, and the last column is the first `//!` line of the file itself (blank when the file has none). Where the tables and the code disagree, the code wins. Build and test commands are in [CONTRIBUTING.md](../CONTRIBUTING.md).

An earlier version of this file was a reorganization proposal (target layout, move map, stages 1 to 5). Only stage 1, the modularization of `spawn-at-verify`, was carried out; the rest was never built, and its paths no longer matched the tree, so it was removed. It is in git history (`git log -- docs/CODE_LAYOUT.md`).

## Workspace

| Member | Role | Notes |
| :--- | :--- | :--- |
| `.` (package `spawn-at`) | The CLI and the installer. Binary `spawn-at`. | The only default member (`default-members = ["."]` in `Cargo.toml`), so plain `cargo build` / `cargo test` cover only this package. |
| `crates/spawn-at-core` | Pure logic with no I/O: geometry, driver trait and batch types, ready-wait state machine, settings, wait override, resize report. | Modules in `src/lib.rs`: `driver`, `geometry`, `ready_wait`, `resize_report`, `settings`, `wait_override`. |
| `crates/spawn-at-verify` | Developer-only verification harness (binary `spawn-at-verify`, plus the `fixture` test-window binary). Never in the release tarball. | Features `fixture` and `tui`; name it with `-p spawn-at-verify --features tui,fixture`. |
| `crates/xtask` | Repository tooling (`cargo xtask ...`): comment-only check, doc coverage, `burn-cpu`. | |

Outside the crates: `assets/gnome/{modern,legacy}/` (the GNOME Shell extensions, embedded into the binary), `tests/*.js` (Node tests for the extension), `tests/ready_wait_vectors/` (golden event scripts run by both a Rust test and a Node test), `verify/` (test specs and suite definitions read by `spawn-at-verify`), `scripts/` (`smoke-live.sh`, and thin wrappers for `cargo xtask`), `install.sh`, `.github/workflows/` (`ci.yml`, `release.yml`).

## Facts that constrain moving files

- **Embedded extension files.** `src/platform/linux/gnome/mod.rs` embeds `assets/gnome/modern/*` and `assets/gnome/legacy/*` with `include_str!("../../../../assets/gnome/...")`. Moving that file changes the relative path. The test `test_embedded_extension_files_match_disk` in the same file asserts that the embedded lists match the directories exactly.
- **Cargo targets of `spawn-at-verify`** are declared in its `Cargo.toml`: `[[bin]]` `spawn-at-verify` (`src/main.rs`) and `fixture` (`src/bin/fixture.rs`, `required-features = ["fixture"]`), and `[[test]]` entries `fixture_test` (needs `fixture`), `oracle_cross_check`, `trace_analyser_test`, `trace_state_test`, `tui_test` (needs `tui`) and `render_test` (needs `tui`). Other files under `tests/` are found by Cargo's default discovery.
- **Release packaging.** `install.sh` downloads the release tarball and needs `spawn-at` in it; `.github/workflows/release.yml` builds `target/release/spawn-at` from the repository root. `scripts/smoke-live.sh` hard-codes `target/debug/spawn-at` and `target/debug/spawn-at-verify`.
- **Shared golden data.** `verify/golden/placement.json` is read by `crates/spawn-at-core/tests/golden_placement.rs`, `crates/spawn-at-verify/src/oracle/tests/vectors.rs` and `tests/golden_anchor_test.js`. `crates/spawn-at-verify/tests/fixtures/` holds recorded runs with a `SHA256SUMS`, and `crates/spawn-at-verify/tests/golden/` holds five `drawer_*.txt` terminal snapshots.
- **Library surface.** `spawn-at-core` re-exports `Armed`, `Batch`, `Driver`, `DriverError`, `Entry`, `FocusIntent`, `Reveal`, `Urgency` and a list of geometry items (see `src/lib.rs`). `spawn-at-verify`'s `src/lib.rs` declares `cli`, `display`, `ext_logging`, `loadavg`, `loader`, `log_source`, `no_config`, `oracle`, `preflight`, `probe`, `process`, `report`, `runner`, `schema`, `trace`, `tracker`, and `tui` (only with the `tui` feature).

## Files

### spawn-at (the CLI, `src/`)

| File | Lines | Module doc (first `//!` line of the file) |
| :--- | ---: | :--- |
| `src/cli/args.rs` | 673 | CLI Argument Structs and Definitions |
| `src/cli/mod.rs` | 84 | Command Line Interface Definitions |
| `src/cli/tests.rs` | 1338 |  |
| `src/commands/config.rs` | 40 | `spawn-at config`: show, get, set, unset, path and reset for the user's settings. It needs no |
| `src/commands/focus.rs` | 122 | Window Focus Command Handlers |
| `src/commands/lifecycle.rs` | 209 | Window Lifecycle State Command Handlers |
| `src/commands/mod.rs` | 1487 | Command Handlers |
| `src/commands/query.rs` | 817 | Compositor State Query Command Handlers |
| `src/commands/spawn.rs` | 1342 | Spawn Command Handler |
| `src/commands/transform.rs` | 372 | Window Transformation Command Handler |
| `src/config.rs` | 430 | Configuration Manager |
| `src/diagnostics.rs` | 1476 | Centralized Diagnostics Layer |
| `src/main.rs` | 379 | spawn-at: Modular Zero-Flicker Window Placement Engine |
| `src/platform/escalate.rs` | 93 | Platform Privilege Escalation |
| `src/platform/install_notice.rs` | 108 | The notice printed before an install on a platform spawn-at has not been run on, and the |
| `src/platform/installer.rs` | 235 | OS-Level Binary Installer & Uninstaller |
| `src/platform/linux/config_store.rs` | 141 | The config file on Linux: where it is and how it is read and written. |
| `src/platform/linux/gnome/dbus.rs` | 51 |  |
| `src/platform/linux/gnome/mechanics.rs` | 640 | Internal GNOME Compositor Mechanics |
| `src/platform/linux/gnome/mod.rs` | 1942 | GNOME Compositor Backend & Extension Driver |
| `src/platform/linux/mod.rs` | 676 |  |
| `src/platform/linux/x11/atoms.rs` | 37 | X11 Atoms and Diagnostic Logging |
| `src/platform/linux/x11/interception.rs` | 294 | X11 Spawn Interception Pipeline |
| `src/platform/linux/x11/mod.rs` | 395 | Native X11 Compositor Driver |
| `src/platform/linux/x11/process.rs` | 30 | Process Tree Introspection Helpers |
| `src/platform/linux/x11/session.rs` | 643 | X11 Display Session and Window Operations |
| `src/platform/linux/x11/tests.rs` | 265 | Native X11 Driver Tests |
| `src/platform/linux/xdg.rs` | 198 | Linux XDG Desktop Entry & Wayland App ID Resolver |
| `src/platform/mod.rs` | 316 | Platform & Compositor Driver Abstraction Layer |
| `src/target.rs` | 592 | Window Target Resolution Engine |
| `src/ui.rs` | 141 | Layout of the installer's terminal output: every block of output (a `=== ... ===` section or a plain group of lines) is separated from the |
| `src/update.rs` | 1459 | Update Manager & Release Checker |

### spawn-at-core (`crates/spawn-at-core/`)

| File | Lines | Module doc (first `//!` line of the file) |
| :--- | ---: | :--- |
| `crates/spawn-at-core/src/driver.rs` | 140 | Declarative Compositor Driver Abstraction |
| `crates/spawn-at-core/src/geometry.rs` | 1680 | Pure Mathematical Geometry Engine |
| `crates/spawn-at-core/src/lib.rs` | 17 | spawn-at-core |
| `crates/spawn-at-core/src/ready_wait.rs` | 535 | Ready-wait state machine |
| `crates/spawn-at-core/src/resize_report.rs` | 147 | Resize report |
| `crates/spawn-at-core/src/settings.rs` | 584 | User settings: schema, validation, defaults, precedence and the `config` commands |
| `crates/spawn-at-core/src/settings/tests.rs` | 365 |  |
| `crates/spawn-at-core/src/wait_override.rs` | 203 | Per-spawn override of the resize wait |
| `crates/spawn-at-core/tests/golden_placement.rs` | 292 |  |
| `crates/spawn-at-core/tests/ready_wait_vectors.rs` | 100 | Runs every golden script in `tests/ready_wait_vectors/scripts/` through the Rust ready-wait |

### xtask (`crates/xtask/`)

| File | Lines | Module doc (first `//!` line of the file) |
| :--- | ---: | :--- |
| `crates/xtask/src/burn_cpu.rs` | 242 | `burn-cpu`: keeps N threads of this process spinning for S seconds, to put the machine under |
| `crates/xtask/src/comment_only.rs` | 205 | `check-comment-only`: verifies a commit changes only comments relative to a base revision. |
| `crates/xtask/src/compiler_docs.rs` | 117 | Compiler-backed documentation check: collects rustc's `missing_docs` warnings. |
| `crates/xtask/src/doc_coverage.rs` | 276 | `check-doc-coverage`: text scan for undocumented items, merged with the compiler lint. |
| `crates/xtask/src/main.rs` | 67 | xtask: Internal developer and CI tooling for spawn-at. |
| `crates/xtask/tests/burn_cpu_test.rs` | 38 | Runs the built `xtask` binary's `burn-cpu` subcommand as a user would. |

### spawn-at-verify, library and binaries (`crates/spawn-at-verify/src/`)

| File | Lines | Module doc (first `//!` line of the file) |
| :--- | ---: | :--- |
| `crates/spawn-at-verify/src/bin/fixture.rs` | 393 | Standalone GUI test fixture window application (`winit` + `softbuffer`). |
| `crates/spawn-at-verify/src/bin/fixture/behaviour.rs` | 298 | Hostile-app behaviours of the fixture, as pure decisions (harness only; never in the product). |
| `crates/spawn-at-verify/src/bin/fixture/font.rs` | 404 | Bitmap font table (10x16) generated from DejaVu Sans Mono Bold. |
| `crates/spawn-at-verify/src/bin/fixture/mod.rs` | 7 | Fixture window, renderer, and bitmap font modules. |
| `crates/spawn-at-verify/src/bin/fixture/renderer.rs` | 151 | Softbuffer frame drawing and bitmap text rendering. |
| `crates/spawn-at-verify/src/bin/fixture/trace.rs` | 144 | Event files of the fixture: a developer trace and the client-side events the harness reads. |
| `crates/spawn-at-verify/src/bin/fixture/window.rs` | 383 | Window creation, winit event loop, and signal handling. |
| `crates/spawn-at-verify/src/cli/args.rs` | 310 | CLI argument parser schemas and subcommand definitions. |
| `crates/spawn-at-verify/src/cli/calibrate.rs` | 403 | Handler for `spawn-at-verify calibrate`: repeated measurement of one case's cloak duration. |
| `crates/spawn-at-verify/src/cli/check.rs` | 97 | Handler for `spawn-at-verify check` subcommand. |
| `crates/spawn-at-verify/src/cli/list.rs` | 49 | Handler for `spawn-at-verify list` subcommand. |
| `crates/spawn-at-verify/src/cli/mod.rs` | 37 | CLI argument models and subcommand handlers. |
| `crates/spawn-at-verify/src/cli/render.rs` | 54 | Handler for `spawn-at-verify render`: prints the TUI drawer for a recorded real case. |
| `crates/spawn-at-verify/src/cli/report.rs` | 67 | Handler for `spawn-at-verify report` subcommand. |
| `crates/spawn-at-verify/src/cli/run.rs` | 300 | Handler for `spawn-at-verify run` subcommand. |
| `crates/spawn-at-verify/src/cli/run_summary.rs` | 346 | Counting, summary text and process exit code for `spawn-at-verify run`. |
| `crates/spawn-at-verify/src/cli/smoke_eval.rs` | 214 | Helper CLI utilities for smoke-live.sh JSON evaluation and placement verification. |
| `crates/spawn-at-verify/src/cli/tui.rs` | 30 | Handler for `spawn-at-verify tui` subcommand. |
| `crates/spawn-at-verify/src/display.rs` | 180 | Display refresh rate, read-only: `xrandr --query` on X11, Mutter's `GetCurrentState` over |
| `crates/spawn-at-verify/src/ext_logging.rs` | 605 | Control of the GNOME extension's timing log over session D-Bus. |
| `crates/spawn-at-verify/src/lib.rs` | 24 | spawn-at-verify |
| `crates/spawn-at-verify/src/loadavg.rs` | 124 | System load average (`/proc/loadavg`), recorded with each run so a slow batch can be told |
| `crates/spawn-at-verify/src/loader/filter.rs` | 42 | Test filter criteria and pattern matching. |
| `crates/spawn-at-verify/src/loader/mod.rs` | 12 | Test loader and suite registry for spawn-at-verify. |
| `crates/spawn-at-verify/src/loader/registry.rs` | 232 | Test registry discovery, loading, and collection management. |
| `crates/spawn-at-verify/src/loader/tests.rs` | 231 | Unit tests for test loader, filtering, and tree visualization. |
| `crates/spawn-at-verify/src/loader/tree.rs` | 76 | Hierarchical formatting and summary generation for test definitions. |
| `crates/spawn-at-verify/src/log_source.rs` | 192 | Log Capture Abstraction (`LogSource` trait). |
| `crates/spawn-at-verify/src/main.rs` | 10 | CLI binary entrypoint for spawn-at-verify. |
| `crates/spawn-at-verify/src/no_config.rs` | 50 | Keeps the user's `spawn-at` config file out of harness runs. |
| `crates/spawn-at-verify/src/oracle/calculator.rs` | 244 | Mathematical placement calculations, anchor formulas, and pivot offsets. |
| `crates/spawn-at-verify/src/oracle/check.rs` | 730 | Comparison of an observed placement against the oracle's expected rectangle. |
| `crates/spawn-at-verify/src/oracle/mod.rs` | 15 | Mathematical placement oracle and golden vector cross-validation. |
| `crates/spawn-at-verify/src/oracle/tests/golden.rs` | 516 | Hand-computed golden placement calculations and tolerance checks. |
| `crates/spawn-at-verify/src/oracle/tests/min_size.rs` | 168 | Oracle with a fixture minimum: expected size is `max(requested, minimum)` per axis, the |
| `crates/spawn-at-verify/src/oracle/tests/mod.rs` | 5 | Oracle test suite organizing hand-computed golden vectors and file-based checks. |
| `crates/spawn-at-verify/src/oracle/tests/vectors.rs` | 99 | Shared golden placement vector file cross-validation. |
| `crates/spawn-at-verify/src/preflight.rs` | 318 | Preflight Gate: binary freshness & git status inspection. |
| `crates/spawn-at-verify/src/probe.rs` | 380 | Environment probe: checks desktop environment, session type, and app availability. |
| `crates/spawn-at-verify/src/process.rs` | 166 | Safe Process Execution Pipeline |
| `crates/spawn-at-verify/src/report/builder.rs` | 215 | Session report builder, qualification evaluator, and artifact writer. |
| `crates/spawn-at-verify/src/report/markdown.rs` | 225 | Markdown session report rendering. |
| `crates/spawn-at-verify/src/report/mod.rs` | 14 | Verification session report models, redaction, and JSON/Markdown export. |
| `crates/spawn-at-verify/src/report/models.rs` | 119 | Session report and qualification summary data structures. |
| `crates/spawn-at-verify/src/report/redaction.rs` | 117 | Title redaction, privacy filtering, and timestamp formatting utilities. |
| `crates/spawn-at-verify/src/report/tests.rs` | 668 | Unit tests for report structures, redaction, and formatting. |
| `crates/spawn-at-verify/src/runner/engine.rs` | 175 | Test runner execution engine, repeat loops, and process orchestration. |
| `crates/spawn-at-verify/src/runner/executor.rs` | 536 | Single-test execution, process invocation, log capture, and attribution. |
| `crates/spawn-at-verify/src/runner/flash.rs` | 342 | Flash check of one run: where the fixture writes its events, how long the run watches after the |
| `crates/spawn-at-verify/src/runner/flash_tests.rs` | 306 | Wiring of the flash check: the journal slice, the fixture's event text and the anchor brackets |
| `crates/spawn-at-verify/src/runner/mod.rs` | 29 | Test execution engine, results aggregation, and validation checks. |
| `crates/spawn-at-verify/src/runner/models.rs` | 375 | Outcome models, execution options, and verdict resolution rules for test runs. |
| `crates/spawn-at-verify/src/runner/resize_expect_tests.rs` | 135 | `[expect.resize]`: the ready-wait outcome a spec declares, read from the CLI's `--json`. |
| `crates/spawn-at-verify/src/runner/tests.rs` | 751 | Unit tests for test runner execution, parsing, and verdict resolution. |
| `crates/spawn-at-verify/src/runner/trace_capture_tests.rs` | 492 | Runs `execute_single_test` against `/bin/echo` and a fake extension to check that logging is |
| `crates/spawn-at-verify/src/runner/validation.rs` | 407 | Expectation verification, output matching, and action argument building. |
| `crates/spawn-at-verify/src/runner/wire_paint_tests.rs` | 263 | Wire paint times in the flash check: the client's commit read from the fixture's protocol dump |
| `crates/spawn-at-verify/src/schema/matrix.rs` | 171 | Test definition matrix expansion and categorization helpers. |
| `crates/spawn-at-verify/src/schema/mod.rs` | 12 | Declarative TOML test definition schemas, matrix expansion, and validation rules. |
| `crates/spawn-at-verify/src/schema/suite.rs` | 65 | Test suite definition schema models. |
| `crates/spawn-at-verify/src/schema/tests.rs` | 234 | Unit tests for schema models and matrix expansion rules. |
| `crates/spawn-at-verify/src/schema/types.rs` | 258 | Core declarative TOML schema data models. |
| `crates/spawn-at-verify/src/trace/analyser.rs` | 266 | Trace invariant analysis, lifecycle verification, and timing budget checks. |
| `crates/spawn-at-verify/src/trace/flash.rs` | 765 | Flash detection: was the window shown before it was ready, or did it change after the reveal? |
| `crates/spawn-at-verify/src/trace/mod.rs` | 16 | Trace log event parser, lifecycle state machine, and budget checks. |
| `crates/spawn-at-verify/src/trace/models.rs` | 112 | Trace data models, events, and violation structures. |
| `crates/spawn-at-verify/src/trace/parser.rs` | 261 | Trace log parsing, event extraction, and regex tokenization. |
| `crates/spawn-at-verify/src/trace/phases.rs` | 105 | Phase gaps inside one window's placement trace, for diagnosing where cloak time goes. |
| `crates/spawn-at-verify/src/trace/report.rs` | 72 | Trace analysis summary and outcome reporting models. |
| `crates/spawn-at-verify/src/trace/state.rs` | 70 | Whether a log slice holds an analysable trace for a window, and if not, why not. |
| `crates/spawn-at-verify/src/trace/wait.rs` | 320 | The wait rule each run used, for A/B runs inside one login (`docs/DESIGN_READY_WAIT.md` |
| `crates/spawn-at-verify/src/trace/wire.rs` | 264 | Paint times read from the fixture's Wayland protocol dump. |
| `crates/spawn-at-verify/src/tracker/mod.rs` | 11 | Window tracking and window leak cleanup prevention across test sessions. |
| `crates/spawn-at-verify/src/tracker/state.rs` | 349 | Window tracker state, attribution algorithms, and window leak cleanup. |
| `crates/spawn-at-verify/src/tracker/tests.rs` | 458 | Unit tests for window tracker attribution, cap limits, and cleanup rules. |
| `crates/spawn-at-verify/src/tui/app/clipboard.rs` | 250 | Clipboard provider integrations (OSC 52, wl-copy, xclip) and drawer export. |
| `crates/spawn-at-verify/src/tui/app/input.rs` | 262 | Keyboard event handling and navigation dispatch. |
| `crates/spawn-at-verify/src/tui/app/mod.rs` | 9 | Interactive TUI application architecture (state, input, execution queue, clipboard). |
| `crates/spawn-at-verify/src/tui/app/queue.rs` | 191 | Execution queue processing, test running, and human review verdict recording. |
| `crates/spawn-at-verify/src/tui/app/state.rs` | 396 | TUI state models, interactive app state, and selection navigation. |
| `crates/spawn-at-verify/src/tui/logging.rs` | 128 | Extension Logging State Detection & Restoration Guard. |
| `crates/spawn-at-verify/src/tui/mod.rs` | 164 | Terminal User Interface (TUI) Module for spawn-at-verify. |
| `crates/spawn-at-verify/src/tui/render.rs` | 216 | Headless rendering of the detail drawer for recorded real runs. |
| `crates/spawn-at-verify/src/tui/views/builder.rs` | 193 | Drawer content line assembly builder. |
| `crates/spawn-at-verify/src/tui/views/constants.rs` | 10 | Drawer layout sizing constants and column offsets. |
| `crates/spawn-at-verify/src/tui/views/drawer/geometry_section.rs` | 267 | Drawer GEOMETRY section rendering: expected (oracle) vs actual placement with a verdict. |
| `crates/spawn-at-verify/src/tui/views/drawer/header_section.rs` | 81 | Drawer header section rendering (Title, ID, Mode, Description). |
| `crates/spawn-at-verify/src/tui/views/drawer/info_sections.rs` | 293 | Drawer informational sections: WHAT TO LOOK FOR, SPEC and OUTPUT (each a separate function so the drawer can order them). |
| `crates/spawn-at-verify/src/tui/views/drawer/mod.rs` | 138 | Drawer widget rendering and content assembly. |
| `crates/spawn-at-verify/src/tui/views/drawer/result_section.rs` | 644 | Drawer RESULT section rendering (Verdict, Cloak duration, provisional timing budget). |
| `crates/spawn-at-verify/src/tui/views/drawer/timeline_format.rs` | 313 | Formatting, parsing, and key humanisation for drawer timeline events. |
| `crates/spawn-at-verify/src/tui/views/drawer/timeline_section.rs` | 186 | Drawer TIMELINE section rendering. |
| `crates/spawn-at-verify/src/tui/views/footer.rs` | 352 | TUI bottom footer bars and responsive keybinding grids. |
| `crates/spawn-at-verify/src/tui/views/mod.rs` | 161 | TUI views module organizing layout, widgets, drawer, and modals. |
| `crates/spawn-at-verify/src/tui/views/modals/help.rs` | 361 | Keyboard shortcuts and navigation help overlay modal. |
| `crates/spawn-at-verify/src/tui/views/modals/mod.rs` | 30 | Modal overlays: keyboard shortcuts help and human review verdict dialog. |
| `crates/spawn-at-verify/src/tui/views/modals/verdict.rs` | 94 | Human review verdict input dialog modal. |
| `crates/spawn-at-verify/src/tui/views/panels/description.rs` | 386 | Test description pane and line wrapping count helpers. |
| `crates/spawn-at-verify/src/tui/views/panels/header.rs` | 117 | Top header banner and missing fixture warning panel. |
| `crates/spawn-at-verify/src/tui/views/panels/mod.rs` | 11 | Main dashboard panel widgets and calculations. |
| `crates/spawn-at-verify/src/tui/views/panels/result_box.rs` | 180 | Test result status display box and height calculation. |
| `crates/spawn-at-verify/src/tui/views/panels/tests_list.rs` | 318 | Test category tree and list widget panel. |
| `crates/spawn-at-verify/src/tui/views/theme.rs` | 106 | Theme styling, color mapping, and status indicators. |
| `crates/spawn-at-verify/src/tui/views/wrapping.rs` | 143 | Text tokenization, word wrapping, and dimension string normalisation. |

### spawn-at-verify, integration tests (`crates/spawn-at-verify/tests/`)

| File | Lines | Module doc (first `//!` line of the file) |
| :--- | ---: | :--- |
| `crates/spawn-at-verify/tests/category_library_test.rs` | 359 | Headless test verifying category schema, loader validation, and library coverage. |
| `crates/spawn-at-verify/tests/cli_exit_codes_test.rs` | 137 | The `run` exit codes are documented in `run --help` and match the summary policy. |
| `crates/spawn-at-verify/tests/fixture_test.rs` | 254 |  |
| `crates/spawn-at-verify/tests/flash_test.rs` | 664 | Flash detection on synthetic traces: the extension's log lines and the fixture's event file |
| `crates/spawn-at-verify/tests/oracle_cross_check.rs` | 577 | Cross-check test comparing spawn-at-verify independent geometric oracle |
| `crates/spawn-at-verify/tests/preflight_tools_test.rs` | 28 | `run` prints an install hint on stderr when the optional X11 tools are not on PATH. |
| `crates/spawn-at-verify/tests/render_test.rs` | 363 | `spawn-at-verify render`: recorded real cases only, correct case selected, nothing wider than |
| `crates/spawn-at-verify/tests/trace_analyser_test.rs` | 630 | Comprehensive test suite for the trace event parser and lifecycle verification engine. |
| `crates/spawn-at-verify/tests/trace_state_test.rs` | 81 | `assess_trace` must tell "analysed" from "no trace captured": an empty or foreign log slice |
| `crates/spawn-at-verify/tests/trace_window_scope_test.rs` | 209 | Regression tests: the analyser and `TestResult` must measure the window the fixture named in |
| `crates/spawn-at-verify/tests/tui/app_flow.rs` | 312 | App workflow integration tests (initialization, navigation, filtering, verdicts). |
| `crates/spawn-at-verify/tests/tui/category_counts.rs` | 150 | Category heading counts in the tests list: only a final Pass counts as passed; tests that |
| `crates/spawn-at-verify/tests/tui/drawer_content.rs` | 590 | Drawer content ordering, formatting, geometry table, and timeline tests. |
| `crates/spawn-at-verify/tests/tui/drawer_geometry.rs` | 212 | GEOMETRY table and RESULT "Geometry" row: Expected comes from the oracle, the mark comes from |
| `crates/spawn-at-verify/tests/tui/drawer_keys_clipboard.rs` | 427 | Drawer keybindings, shortcuts help modal, and clipboard copy/save tests. |
| `crates/spawn-at-verify/tests/tui/drawer_labels.rs` | 258 | Drawer-wide shared label column: width rule, cap, truncation and wrapped continuation. |
| `crates/spawn-at-verify/tests/tui/drawer_order.rs` | 120 | Drawer section order, anchor/margin de-duplication and the auto-test verdict row. |
| `crates/spawn-at-verify/tests/tui/drawer_trace.rs` | 333 | RESULT section when the log slice holds no trace for the window: "not analysed", never a |
| `crates/spawn-at-verify/tests/tui/drawer_workarea.rs` | 182 | Which workarea the geometry check trusts: the probe is primary, the extension's logged bounds |
| `crates/spawn-at-verify/tests/tui/flash.rs` | 219 | "PASS (flash)" in the TUI: its own symbol and colour in the list, its own verdict and rows in |
| `crates/spawn-at-verify/tests/tui/footer_widths.rs` | 160 | Keys bar completeness at every width: no shortcut action may silently disappear. |
| `crates/spawn-at-verify/tests/tui/golden_invariants.rs` | 471 | Responsive golden file assertions and drawer grid invariant tests. |
| `crates/spawn-at-verify/tests/tui/layout_responsive.rs` | 470 | Responsive layout and breakpoint rendering integration tests. |
| `crates/spawn-at-verify/tests/tui/mod.rs` | 337 | Common test harness helpers and submodule declarations for TUI test suite. |
| `crates/spawn-at-verify/tests/tui/polish.rs` | 455 | UI polish, padding, spacing, and characterization specification tests. |
| `crates/spawn-at-verify/tests/tui_test.rs` | 4 | Integration test suite for the spawn-at-verify TUI interface. |

### GNOME extension (`assets/gnome/`)

| File | Lines | Module doc (first `//!` line of the file) |
| :--- | ---: | :--- |
| `assets/gnome/legacy/anchor.js` | 53 | spawn-at — pure anchor and bounding box calculation (Legacy GNOME 42-44). |
| `assets/gnome/legacy/cloak.js` | 159 | spawn-at — actor opacity cloaking and snapshot management (Legacy GNOME 42-44). |
| `assets/gnome/legacy/commit.js` | 152 | spawn-at — Mutter commit & frame presentation detection (Legacy GNOME 42-44). |
| `assets/gnome/legacy/dbus.js` | 113 | spawn-at — D-Bus interface definition and session export (Legacy GNOME 42-44). |
| `assets/gnome/legacy/extension.js` | 1622 | spawn-at — precise window spawning, sizing, positioning and orchestration |
| `assets/gnome/legacy/logger.js` | 321 | spawn-at — persistent session logging and enriched telemetry (Legacy GNOME 42-44). |
| `assets/gnome/legacy/pulse.js` | 205 | spawn-at — VTE / terminal cursor heartbeat and focus pulse tracking (Legacy GNOME 42-44). |
| `assets/gnome/legacy/validator.js` | 158 |  |
| `assets/gnome/modern/anchor.js` | 60 | spawn-at — pure anchor and bounding box calculation. |
| `assets/gnome/modern/cloak.js` | 180 | spawn-at — actor opacity cloaking and snapshot management. |
| `assets/gnome/modern/commit.js` | 321 | spawn-at — Mutter commit & frame presentation detection. |
| `assets/gnome/modern/dbus.js` | 115 | spawn-at — D-Bus interface definition and session export. |
| `assets/gnome/modern/extension.js` | 1799 | spawn-at — precise window spawning, sizing, positioning and orchestration |
| `assets/gnome/modern/logger.js` | 324 | spawn-at — persistent session logging and enriched telemetry. |
| `assets/gnome/modern/pulse.js` | 224 | spawn-at — VTE / terminal cursor heartbeat and focus pulse tracking. |
| `assets/gnome/modern/ready_wait.js` | 164 | spawn-at — ready-wait state machine (JS port of crates/spawn-at-core/src/ready_wait.rs). |
| `assets/gnome/modern/validator.js` | 204 |  |
| `assets/gnome/modern/x11_observe.js` | 259 | spawn-at — observe-only logging of the paint signals of X11 and XWayland windows. |

### Node tests (`tests/`)

| File | Lines | Module doc (first `//!` line of the file) |
| :--- | ---: | :--- |
| `tests/cloak_test.js` | 148 |  |
| `tests/dispatcher_test.js` | 1531 |  |
| `tests/extension_test.js` | 234 |  |
| `tests/gi_loader.js` | 157 |  |
| `tests/golden_anchor_test.js` | 152 |  |
| `tests/late_resize_test.js` | 195 |  |
| `tests/legacy_dispatcher_test.js` | 1265 |  |
| `tests/ready_wait_test.js` | 120 |  |
| `tests/ready_wait_wiring_test.js` | 713 |  |
| `tests/wait_override_test.js` | 71 |  |
| `tests/x11_observe_test.js` | 320 |  |

## Specs read by `spawn-at-verify`

`verify/<category>/*.toml` are the test specs, `verify/suites/*.toml` the suite definitions and `verify/golden/` the shared golden data. `spawn-at-verify list` shows what is loaded; see [verify/README.md](../verify/README.md).

## Historical evidence

`docs/evidence/stage1/` holds raw command outputs (fmt, clippy, `--help`, test lists) captured when stage 1 of the earlier reorganization was proven to be a pure move. It is kept as a record only.
