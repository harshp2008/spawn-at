# Code layout

**Status:** reference. The file tables list the tracked source files; the workspace members, Cargo targets and module lists were checked against the tree on 2026-10-20.

What is in the repository now. The file list below is generated from the tracked files: the last column is the first sentence of the file's own module header (the `//!` lines of a Rust file, which can run over several lines; the leading comment of a JavaScript file), or `no module header` when the file has none. The tables carry no line counts; use `wc -l` when you need one. Where the tables and the code disagree, the code wins. Build and test commands are in [CONTRIBUTING.md](../CONTRIBUTING.md).

An earlier version of this file was a reorganization proposal (target layout, move map, stages 1 to 5). Only stage 1, the modularization of `spawn-at-verify`, was carried out; the rest was never built, and its paths no longer matched the tree, so it was removed. It is in git history (`git log -- docs/CODE_LAYOUT.md`).

## Workspace

| Member | Role | Notes |
| :--- | :--- | :--- |
| `.` (package `spawn-at`) | The CLI and the installer. Binary `spawn-at`. | The only default member (`default-members = ["."]` in `Cargo.toml`), so plain `cargo build` / `cargo test` cover only this package. |
| `crates/spawn-at-core` | Pure logic with no I/O: geometry, driver trait and batch types, ready-wait state machine, settings, wait override, resize report. | Modules in `src/lib.rs`: `driver`, `geometry`, `ready_wait`, `resize_report`, `settings`, `wait_override`. |
| `crates/spawn-at-verify` | Developer-only verification harness (binary `spawn-at-verify`, plus the `fixture` test-window binary). Never in the release tarball. | Features `fixture` and `tui`; name it with `-p spawn-at-verify --features tui,fixture`. |
| `crates/xtask` | Repository tooling (`cargo xtask ...`): comment-only check, doc coverage, `burn-cpu`, and the documentation-site commands (`docs-site`, `docs-cli-reference`, `docs-check-src`, `docs-check-html`). | No dependencies. |

Outside the crates: `assets/gnome/{modern,legacy}/` (the GNOME Shell extensions, embedded into the binary), `tests/*.js` (Node tests for the extension), `tests/ready_wait_vectors/` (golden event scripts run by both a Rust test and a Node test), `verify/` (test specs and suite definitions read by `spawn-at-verify`), `tests/fixtures/sway/` (JSON and event lines recorded from a private headless Sway 1.9, read by the Sway unit tests and by `tests/sway_backend_cli_test.rs`), `scripts/` (`smoke-live.sh`, `sway-headless.sh` (a private headless Sway for the Sway backend tests, see [DESIGN_SWAY.md](DESIGN_SWAY.md)), `sway-live.sh` (the Sway live cases, run inside that private Sway), and thin wrappers for `cargo xtask`, among them `check-comment-only-product.sh`), `install.sh`, `.github/workflows/` (`ci.yml`, `release.yml`, `docs.yml`), `docs-site/` (the Starlight user guide: `astro.config.mjs`, `package.json` and `src/content/docs/`; see "Documentation site" below).

## Facts that constrain moving files

- **Embedded extension files.** `src/platform/linux/gnome/deploy.rs` embeds `assets/gnome/modern/*` and `assets/gnome/legacy/*` with `include_str!("../../../../assets/gnome/...")`. Moving that file changes the relative path. The test `test_embedded_extension_files_match_disk` in `gnome/tests.rs` asserts that the embedded lists match the directories exactly.
- **Cargo targets of `spawn-at-verify`** are declared in its `Cargo.toml`: `[[bin]]` `spawn-at-verify` (`src/main.rs`) and `fixture` (`src/bin/fixture.rs`, `required-features = ["fixture"]`), and `[[test]]` entries `fixture_test` (needs `fixture`), `oracle_cross_check`, `trace_analyser_test`, `trace_state_test`, `tui_test` (needs `tui`) and `render_test` (needs `tui`). Other files under `tests/` are found by Cargo's default discovery.
- **Release packaging.** `install.sh` downloads the release tarball and needs `spawn-at` in it; `.github/workflows/release.yml` builds `target/release/spawn-at` from the repository root. `scripts/smoke-live.sh` hard-codes `target/debug/spawn-at` and `target/debug/spawn-at-verify`.
- **Shared golden data.** `verify/golden/placement.json` is read by `crates/spawn-at-core/tests/golden_placement.rs`, `crates/spawn-at-verify/src/oracle/tests/vectors.rs` and `tests/golden_anchor_test.js`. `crates/spawn-at-verify/tests/fixtures/` holds recorded runs with a `SHA256SUMS`, and `crates/spawn-at-verify/tests/golden/` holds five `drawer_*.txt` terminal snapshots.
- **Library surface.** `spawn-at-core` re-exports `Armed`, `Batch`, `Driver`, `DriverError`, `Entry`, `FocusIntent`, `Reveal`, `Urgency` and a list of geometry items (see `src/lib.rs`). `spawn-at-verify`'s `src/lib.rs` declares `backend`, `cli`, `display`, `ext_logging`, `loadavg`, `loader`, `log_source`, `no_config`, `oracle`, `preflight`, `probe`, `process`, `report`, `runner`, `schema`, `trace`, `tracker`, and `tui` (only with the `tui` feature).

## Files

### spawn-at (the CLI, `src/`)

| File | Module doc (first `//!` line of the file) |
| :--- | :--- |
| `src/cli/args.rs` | CLI Argument Structs and Definitions |
| `src/cli/mod.rs` | Command Line Interface Definitions |
| `src/cli/tests.rs` | no module header |
| `src/commands/config.rs` | `spawn-at config`: show, get, set, unset, path and reset for the user's settings. |
| `src/commands/focus.rs` | Window Focus Command Handlers |
| `src/commands/lifecycle.rs` | Window Lifecycle State Command Handlers |
| `src/commands/mod.rs` | Command Handlers |
| `src/commands/query.rs` | Compositor State Query Command Handlers |
| `src/commands/spawn.rs` | Spawn Command Handler |
| `src/commands/transform.rs` | Window Transformation Command Handler |
| `src/config.rs` | Configuration Manager |
| `src/diagnostics.rs` | Centralized Diagnostics Layer |
| `src/main.rs` | spawn-at: Modular Zero-Flicker Window Placement Engine |
| `src/platform/capability.rs` | What a backend can do, and narrow views of the backend trait. |
| `src/platform/escalate.rs` | Platform Privilege Escalation |
| `src/platform/install_notice.rs` | The notice printed before an install on a platform spawn-at has not been run on, and the confirmation that goes with it. |
| `src/platform/host.rs` | What the operating system decides: bin dirs, autostart path, executable test, detaching a child. |
| `src/platform/installer.rs` | OS-Level Binary Installer & Uninstaller |
| `src/platform/linux/config_store.rs` | The config file on Linux: where it is and how it is read and written. |
| `src/platform/linux/host.rs` | The Linux Host: the values the installer, the update check and the launcher always used. |
| `src/platform/linux/gnome/args.rs` | Command-line flags of the GNOME install and uninstall steps. |
| `src/platform/linux/gnome/dbus.rs` | The session-bus interface of the spawn-at GNOME Shell extension, as a zbus proxy. |
| `src/platform/linux/gnome/deploy.rs` | Deploying and removing the GNOME Shell extension (`install` / `uninstall`), the embedded extension files, and the session reload helpers. |
| `src/platform/linux/gnome/detect.rs` | Detection of the GNOME Shell version and of a Wayland session. |
| `src/platform/linux/gnome/driver.rs` | The GNOME Wayland driver: D-Bus communication with the extension, arming, claims, and the window queries and actions of the compositor backend. |
| `src/platform/linux/gnome/logout.rs` | Decides whether a GNOME Wayland install still needs a log-out. |
| `src/platform/linux/gnome/mechanics.rs` | Internal GNOME Compositor Mechanics |
| `src/platform/linux/gnome/mod.rs` | GNOME Compositor Backend & Extension Driver |
| `src/platform/linux/gnome/tests.rs` | Unit tests of the GNOME backend: the install and uninstall flag defaults, the Shell version parser, the embedded extension file lists against `assets/gnome`, the legacy and modern D-Bus XML, and `GnomeWaylandDriver` against stub services on the session bus (close, layout, windows, claims, capabilities, provisioner). |
| `src/platform/linux/mod.rs` | Everything Linux-specific: the three drivers (`gnome`, `sway`, `x11`), the backend choice (`registry`), the Linux [`host::NativeHost`] and config store, and [`LinuxBackend`], the one value the commands hold on this OS. |
| `src/platform/linux/process_tree.rs` | Process Tree Introspection Helpers |
| `src/platform/linux/registry.rs` | Which Linux backend a session gets: the choice that `LinuxBackend::bootstrap` always made, as a pure function |
| `src/platform/linux/sway/client.rs` | A blocking connection to the Sway IPC socket. |
| `src/platform/linux/sway/command.rs` | The text of the commands sent with `RUN_COMMAND`. |
| `src/platform/linux/sway/fake.rs` | A scripted stand-in for the Sway IPC socket, for tests (compiled only for tests). |
| `src/platform/linux/sway/ipc.rs` | The Sway (i3) IPC wire format: framing only, no sockets. |
| `src/platform/linux/sway/matching.rs` | Which new window belongs to the process spawn-at launched. |
| `src/platform/linux/sway/mod.rs` | Sway backend (EXPERIMENTAL) |
| `src/platform/linux/sway/place.rs` | The write side over blocking connections: waiting for the new window, placing it, and the single-window commands. |
| `src/platform/linux/sway/tests_write.rs` | Placement, matching and the single-window commands against the scripted fake IPC server (`fake.rs`). |
| `src/platform/linux/sway/types.rs` | The JSON Sway sends, parsed into plain types, and the conversions to what the neutral code uses (`WindowMetadata`, `MonitorLayout`). |
| `src/platform/linux/x11/atoms.rs` | X11 Atoms and Diagnostic Logging |
| `src/platform/linux/x11/interception.rs` | X11 Spawn Interception Pipeline |
| `src/platform/linux/x11/mod.rs` | Native X11 Compositor Driver |
| `src/platform/linux/x11/session.rs` | X11 Display Session and Window Operations |
| `src/platform/linux/x11/tests.rs` | Native X11 Driver Tests |
| `src/platform/linux/xdg.rs` | Linux XDG Desktop Entry & Wayland App ID Resolver |
| `src/platform/mod.rs` | Platform & Compositor Driver Abstraction Layer |
| `src/platform/null_backend.rs` | The backend of an operating system spawn-at does not support. |
| `src/platform/provisioner.rs` | Setup and teardown of what a backend needs on the machine (for GNOME: the Shell extension). |
| `src/platform/session_actions.rs` | What ends a run: logging out, or reloading the shell. |
| `src/platform/update_hooks.rs` | What `spawn-at update` needs to know about the desktop. |
| `src/platform/non_linux.rs` | Stand-in for `platform::linux` on operating systems spawn-at does not support yet. |
| `src/platform/upgrade.rs` | Detects an existing spawn-at install and plans what `spawn-at install` does about it: install over it, do nothing, or a clean install that removes the old binary, extension and autostart entry first. |
| `src/prompts.rs` | Every dialoguer prompt and item list of the program, in one place, with a guard test. |
| `src/session.rs` | The end of a run: the question about logging out (or reloading the X11 Shell), the advice about it, and the log-out itself. |
| `src/target.rs` | Window Target Resolution Engine |
| `src/ui.rs` | Layout of the installer's terminal output: every block of output (a `=== ... ===` section or a plain group of lines) is separated from the previous one by exactly one blank line, and the output neither starts nor ends with a blank line. |
| `src/update.rs` | Update Manager & Release Checker |

### spawn-at-core (`crates/spawn-at-core/`)

| File | Module doc (first `//!` line of the file) |
| :--- | :--- |
| `crates/spawn-at-core/src/driver.rs` | Declarative Compositor Driver Abstraction |
| `crates/spawn-at-core/src/geometry.rs` | Pure Mathematical Geometry Engine |
| `crates/spawn-at-core/src/lib.rs` | spawn-at-core |
| `crates/spawn-at-core/src/ready_wait.rs` | Ready-wait state machine |
| `crates/spawn-at-core/src/resize_report.rs` | Resize report |
| `crates/spawn-at-core/src/settings.rs` | User settings: schema, validation, defaults, precedence and the `config` commands |
| `crates/spawn-at-core/src/settings/tests.rs` | no module header |
| `crates/spawn-at-core/src/wait_override.rs` | Per-spawn override of the resize wait |
| `crates/spawn-at-core/tests/golden_placement.rs` | no module header |
| `crates/spawn-at-core/tests/ready_wait_vectors.rs` | Runs every golden script in `tests/ready_wait_vectors/scripts/` through the Rust ready-wait machine with the virtual-clock driver described in that directory's README, and compares the outcome with the script's `expect`. |

### xtask (`crates/xtask/`)

| File | Module doc (first `//!` line of the file) |
| :--- | :--- |
| `crates/xtask/src/burn_cpu.rs` | `burn-cpu`: keeps N threads of this process spinning for S seconds, to put the machine under a known CPU load while a live measurement runs. |
| `crates/xtask/src/comment_only.rs` | `check-comment-only`: verifies a commit changes only comments relative to a base revision. |
| `crates/xtask/src/compiler_docs.rs` | Compiler-backed documentation check: collects rustc's `missing_docs` warnings. |
| `crates/xtask/src/doc_coverage.rs` | `check-doc-coverage`: text scan for undocumented items, merged with the compiler lint. |
| `crates/xtask/src/docs_check.rs` | `docs-check-src`: checks the markdown of the site (`docs-site/src/content/docs`) and the Starlight config before the site is built. |
| `crates/xtask/src/docs_cli.rs` | `docs-cli-reference`: writes one Starlight page per `spawn-at` command from the built binary's own `--help` output, and fails if a command that `--help` lists has no page. |
| `crates/xtask/src/docs_html.rs` | `docs-check-html`: checks the assembled site (Astro output plus `api/`) without any network access. |
| `crates/xtask/src/docs_site.rs` | `docs-site`: builds the whole documentation site the way CI does, and checks it. |
| `crates/xtask/src/main.rs` | xtask: Internal developer and CI tooling for spawn-at. |
| `crates/xtask/tests/burn_cpu_test.rs` | Runs the built `xtask` binary's `burn-cpu` subcommand as a user would. |

### spawn-at-verify, library and binaries (`crates/spawn-at-verify/src/`)

| File | Module doc (first `//!` line of the file) |
| :--- | :--- |
| `crates/spawn-at-verify/src/bin/fixture.rs` | Standalone GUI test fixture window application (`winit` + `softbuffer`). |
| `crates/spawn-at-verify/src/bin/fixture/behaviour.rs` | Hostile-app behaviours of the fixture, as pure decisions (harness only; never in the product). |
| `crates/spawn-at-verify/src/bin/fixture/font.rs` | Bitmap font table (10x16) generated from DejaVu Sans Mono Bold. |
| `crates/spawn-at-verify/src/bin/fixture/mod.rs` | Fixture window, renderer, and bitmap font modules. |
| `crates/spawn-at-verify/src/bin/fixture/renderer.rs` | Softbuffer frame drawing and bitmap text rendering. |
| `crates/spawn-at-verify/src/bin/fixture/trace.rs` | Event files of the fixture: a developer trace and the client-side events the harness reads. |
| `crates/spawn-at-verify/src/bin/fixture/window.rs` | Window creation, winit event loop, and signal handling. |
| `crates/spawn-at-verify/src/cli/args.rs` | CLI argument parser schemas and subcommand definitions. |
| `crates/spawn-at-verify/src/cli/calibrate.rs` | Handler for `spawn-at-verify calibrate`: repeated measurement of one case's cloak duration. |
| `crates/spawn-at-verify/src/cli/check.rs` | Handler for `spawn-at-verify check` subcommand. |
| `crates/spawn-at-verify/src/cli/list.rs` | Handler for `spawn-at-verify list` subcommand. |
| `crates/spawn-at-verify/src/cli/mod.rs` | CLI argument models and subcommand handlers. |
| `crates/spawn-at-verify/src/cli/render.rs` | Handler for `spawn-at-verify render`: prints the TUI drawer for a recorded real case. |
| `crates/spawn-at-verify/src/cli/report.rs` | Handler for `spawn-at-verify report` subcommand. |
| `crates/spawn-at-verify/src/cli/run.rs` | Handler for `spawn-at-verify run` subcommand. |
| `crates/spawn-at-verify/src/cli/run_summary.rs` | Counting, summary text and process exit code for `spawn-at-verify run`. |
| `crates/spawn-at-verify/src/cli/smoke_eval.rs` | Helper CLI utilities for smoke-live.sh JSON evaluation and placement verification. |
| `crates/spawn-at-verify/src/cli/tui.rs` | Handler for `spawn-at-verify tui` subcommand. |
| `crates/spawn-at-verify/src/display.rs` | Display refresh rate, read-only: `xrandr --query` on X11, Mutter's `GetCurrentState` over `gdbus` on Wayland, 60 Hz (said so) when neither gives one. |
| `crates/spawn-at-verify/src/ext_logging.rs` | Control of the GNOME extension's timing log over session D-Bus. |
| `crates/spawn-at-verify/src/lib.rs` | spawn-at-verify Verification Engine & Test Harness for `spawn-at`. |
| `crates/spawn-at-verify/src/loadavg.rs` | System load average (`/proc/loadavg`), recorded with each run so a slow batch can be told apart from a busy machine. |
| `crates/spawn-at-verify/src/loader/filter.rs` | Test filter criteria and pattern matching. |
| `crates/spawn-at-verify/src/loader/mod.rs` | Test loader and suite registry for spawn-at-verify. |
| `crates/spawn-at-verify/src/loader/registry.rs` | Test registry discovery, loading, and collection management. |
| `crates/spawn-at-verify/src/loader/tests.rs` | Unit tests for test loader, filtering, and tree visualization. |
| `crates/spawn-at-verify/src/loader/tree.rs` | Hierarchical formatting and summary generation for test definitions. |
| `crates/spawn-at-verify/src/log_source.rs` | Log Capture Abstraction (`LogSource` trait). |
| `crates/spawn-at-verify/src/main.rs` | CLI binary entrypoint for spawn-at-verify. |
| `crates/spawn-at-verify/src/no_config.rs` | Keeps the user's `spawn-at` config file out of harness runs. |
| `crates/spawn-at-verify/src/oracle/calculator.rs` | Mathematical placement calculations, anchor formulas, and pivot offsets. |
| `crates/spawn-at-verify/src/oracle/check.rs` | Comparison of an observed placement against the oracle's expected rectangle. |
| `crates/spawn-at-verify/src/oracle/mod.rs` | Mathematical placement oracle and golden vector cross-validation. |
| `crates/spawn-at-verify/src/oracle/tests/golden.rs` | Hand-computed golden placement calculations and tolerance checks. |
| `crates/spawn-at-verify/src/oracle/tests/min_size.rs` | Oracle with a fixture minimum: expected size is `max(requested, minimum)` per axis, the position is the anchor for that final size, and far-edge anchors must stay inside the workarea. |
| `crates/spawn-at-verify/src/oracle/tests/mod.rs` | Oracle test suite organizing hand-computed golden vectors and file-based checks. |
| `crates/spawn-at-verify/src/oracle/tests/vectors.rs` | Shared golden placement vector file cross-validation. |
| `crates/spawn-at-verify/src/preflight.rs` | Preflight Gate: binary freshness & git status inspection. |
| `crates/spawn-at-verify/src/probe.rs` | Environment probe: checks desktop environment, session type, and app availability. |
| `crates/spawn-at-verify/src/process.rs` | Safe Process Execution Pipeline |
| `crates/spawn-at-verify/src/report/builder.rs` | Session report builder, qualification evaluator, and artifact writer. |
| `crates/spawn-at-verify/src/report/markdown.rs` | Markdown session report rendering. |
| `crates/spawn-at-verify/src/report/mod.rs` | Verification session report models, redaction, and JSON/Markdown export. |
| `crates/spawn-at-verify/src/report/models.rs` | Session report and qualification summary data structures. |
| `crates/spawn-at-verify/src/report/redaction.rs` | Title redaction, privacy filtering, and timestamp formatting utilities. |
| `crates/spawn-at-verify/src/report/tests.rs` | Unit tests for report structures, redaction, and formatting. |
| `crates/spawn-at-verify/src/runner/engine.rs` | Test runner execution engine, repeat loops, and process orchestration. |
| `crates/spawn-at-verify/src/runner/executor.rs` | Single-test execution, process invocation, log capture, and attribution. |
| `crates/spawn-at-verify/src/runner/flash.rs` | Flash check of one run: where the fixture writes its events, how long the run watches after the reveal, and how the extension's log, the fixture's events and the clock anchor become a [`FlashInfo`]. |
| `crates/spawn-at-verify/src/runner/flash_tests.rs` | Wiring of the flash check: the journal slice, the fixture's event text and the anchor brackets become a result; the event directory is private to a run; a flash passes or fails the run depending on `--strict-flash`. |
| `crates/spawn-at-verify/src/runner/mod.rs` | Test execution engine, results aggregation, and validation checks. |
| `crates/spawn-at-verify/src/runner/models.rs` | Outcome models, execution options, and verdict resolution rules for test runs. |
| `crates/spawn-at-verify/src/runner/resize_expect_tests.rs` | `[expect.resize]`: the ready-wait outcome a spec declares, read from the CLI's `--json`. |
| `crates/spawn-at-verify/src/runner/tests.rs` | Unit tests for test runner execution, parsing, and verdict resolution. |
| `crates/spawn-at-verify/src/runner/trace_capture_tests.rs` | Runs `execute_single_test` against `/bin/echo` and a fake extension to check that logging is enabled before each run, and that a missing trace is recorded with its reason. |
| `crates/spawn-at-verify/src/runner/validation.rs` | Expectation verification, output matching, and action argument building. |
| `crates/spawn-at-verify/src/runner/wire_paint_tests.rs` | Wire paint times in the flash check: the client's commit read from the fixture's protocol dump replaces the fixture's `FIRST_PAINT` stamp in the verdict, and both times are reported. |
| `crates/spawn-at-verify/src/schema/matrix.rs` | Test definition matrix expansion and categorization helpers. |
| `crates/spawn-at-verify/src/schema/mod.rs` | Declarative TOML test definition schemas, matrix expansion, and validation rules. |
| `crates/spawn-at-verify/src/schema/suite.rs` | Test suite definition schema models. |
| `crates/spawn-at-verify/src/schema/tests.rs` | Unit tests for schema models and matrix expansion rules. |
| `crates/spawn-at-verify/src/schema/types.rs` | Core declarative TOML schema data models. |
| `crates/spawn-at-verify/src/trace/analyser.rs` | Trace invariant analysis, lifecycle verification, and timing budget checks. |
| `crates/spawn-at-verify/src/trace/flash.rs` | Flash detection: was the window shown before it was ready, or did it change after the reveal? |
| `crates/spawn-at-verify/src/trace/mod.rs` | Trace log event parser, lifecycle state machine, and budget checks. |
| `crates/spawn-at-verify/src/trace/models.rs` | Trace data models, events, and violation structures. |
| `crates/spawn-at-verify/src/trace/parser.rs` | Trace log parsing, event extraction, and regex tokenization. |
| `crates/spawn-at-verify/src/trace/phases.rs` | Phase gaps inside one window's placement trace, for diagnosing where cloak time goes. |
| `crates/spawn-at-verify/src/trace/report.rs` | Trace analysis summary and outcome reporting models. |
| `crates/spawn-at-verify/src/trace/state.rs` | Whether a log slice holds an analysable trace for a window, and if not, why not. |
| `crates/spawn-at-verify/src/trace/wait.rs` | The wait rule each run used, for A/B runs inside one login (`docs/DESIGN_READY_WAIT.md` section 10): what `run --wait-strategy / --wait-cap-ms` asked for, and what the extension says it did, read from the run's own log slice. |
| `crates/spawn-at-verify/src/trace/wire.rs` | Paint times read from the fixture's Wayland protocol dump. |
| `crates/spawn-at-verify/src/tracker/mod.rs` | Window tracking and window leak cleanup prevention across test sessions. |
| `crates/spawn-at-verify/src/tracker/state.rs` | Window tracker state, attribution algorithms, and window leak cleanup. |
| `crates/spawn-at-verify/src/tracker/tests.rs` | Unit tests for window tracker attribution, cap limits, and cleanup rules. |
| `crates/spawn-at-verify/src/tui/app/clipboard.rs` | Clipboard provider integrations (OSC 52, wl-copy, xclip) and drawer export. |
| `crates/spawn-at-verify/src/tui/app/input.rs` | Keyboard event handling and navigation dispatch. |
| `crates/spawn-at-verify/src/tui/app/mod.rs` | Interactive TUI application architecture (state, input, execution queue, clipboard). |
| `crates/spawn-at-verify/src/tui/app/queue.rs` | Execution queue processing, test running, and human review verdict recording. |
| `crates/spawn-at-verify/src/tui/app/state.rs` | TUI state models, interactive app state, and selection navigation. |
| `crates/spawn-at-verify/src/tui/logging.rs` | Extension Logging State Detection & Restoration Guard. |
| `crates/spawn-at-verify/src/tui/mod.rs` | Terminal User Interface (TUI) Module for spawn-at-verify. |
| `crates/spawn-at-verify/src/tui/render.rs` | Headless rendering of the detail drawer for recorded real runs. |
| `crates/spawn-at-verify/src/tui/views/builder.rs` | Drawer content line assembly builder. |
| `crates/spawn-at-verify/src/tui/views/constants.rs` | Drawer layout sizing constants and column offsets. |
| `crates/spawn-at-verify/src/tui/views/drawer/geometry_section.rs` | Drawer GEOMETRY section rendering: expected (oracle) vs actual placement with a verdict. |
| `crates/spawn-at-verify/src/tui/views/drawer/header_section.rs` | Drawer header section rendering (Title, ID, Mode, Description). |
| `crates/spawn-at-verify/src/tui/views/drawer/info_sections.rs` | Drawer informational sections: WHAT TO LOOK FOR, SPEC and OUTPUT (each a separate function so the drawer can order them). |
| `crates/spawn-at-verify/src/tui/views/drawer/mod.rs` | Drawer widget rendering and content assembly. |
| `crates/spawn-at-verify/src/tui/views/drawer/result_section.rs` | Drawer RESULT section rendering (Verdict, Cloak duration, provisional timing budget). |
| `crates/spawn-at-verify/src/tui/views/drawer/timeline_format.rs` | Formatting, parsing, and key humanisation for drawer timeline events. |
| `crates/spawn-at-verify/src/tui/views/drawer/timeline_section.rs` | Drawer TIMELINE section rendering. |
| `crates/spawn-at-verify/src/tui/views/footer.rs` | TUI bottom footer bars and responsive keybinding grids. |
| `crates/spawn-at-verify/src/tui/views/mod.rs` | TUI views module organizing layout, widgets, drawer, and modals. |
| `crates/spawn-at-verify/src/tui/views/modals/help.rs` | Keyboard shortcuts and navigation help overlay modal. |
| `crates/spawn-at-verify/src/tui/views/modals/mod.rs` | Modal overlays: keyboard shortcuts help and human review verdict dialog. |
| `crates/spawn-at-verify/src/tui/views/modals/verdict.rs` | Human review verdict input dialog modal. |
| `crates/spawn-at-verify/src/tui/views/panels/description.rs` | Test description pane and line wrapping count helpers. |
| `crates/spawn-at-verify/src/tui/views/panels/header.rs` | Top header banner and missing fixture warning panel. |
| `crates/spawn-at-verify/src/tui/views/panels/mod.rs` | Main dashboard panel widgets and calculations. |
| `crates/spawn-at-verify/src/tui/views/panels/result_box.rs` | Test result status display box and height calculation. |
| `crates/spawn-at-verify/src/tui/views/panels/tests_list.rs` | Test category tree and list widget panel. |
| `crates/spawn-at-verify/src/tui/views/theme.rs` | Theme styling, color mapping, and status indicators. |
| `crates/spawn-at-verify/src/tui/views/wrapping.rs` | Text tokenization, word wrapping, and dimension string normalisation. |

### spawn-at-verify, integration tests (`crates/spawn-at-verify/tests/`)

| File | Module doc (first `//!` line of the file) |
| :--- | :--- |
| `crates/spawn-at-verify/tests/category_library_test.rs` | Headless test verifying category schema, loader validation, and library coverage. |
| `crates/spawn-at-verify/tests/cli_exit_codes_test.rs` | The `run` exit codes are documented in `run --help` and match the summary policy. |
| `crates/spawn-at-verify/tests/fixture_test.rs` | no module header |
| `crates/spawn-at-verify/tests/flash_test.rs` | Flash detection on synthetic traces: the extension's log lines and the fixture's event file are written by hand so every timing in the assertions is a number in the test. |
| `crates/spawn-at-verify/tests/oracle_cross_check.rs` | Cross-check test comparing spawn-at-verify independent geometric oracle with spawn-at-core geometry engine across the full anchor x pivot matrix, including offset-origin and negative-origin workareas. |
| `crates/spawn-at-verify/tests/preflight_tools_test.rs` | `run` prints an install hint on stderr when the optional X11 tools are not on PATH. |
| `crates/spawn-at-verify/tests/render_test.rs` | `spawn-at-verify render`: recorded real cases only, correct case selected, nothing wider than the requested width, and the window-scoped timeline of the real delay-500 recording. |
| `crates/spawn-at-verify/tests/trace_analyser_test.rs` | Comprehensive test suite for the trace event parser and lifecycle verification engine. |
| `crates/spawn-at-verify/tests/trace_state_test.rs` | `assess_trace` must tell "analysed" from "no trace captured": an empty or foreign log slice is never a clean trace. |
| `crates/spawn-at-verify/tests/trace_window_scope_test.rs` | Regression tests: the analyser and `TestResult` must measure the window the fixture named in stdout, never a leftover window from an earlier run that the whole-second log slice picked up. |
| `crates/spawn-at-verify/tests/tui/app_flow.rs` | App workflow integration tests (initialization, navigation, filtering, verdicts). |
| `crates/spawn-at-verify/tests/tui/category_counts.rs` | Category heading counts in the tests list: only a final Pass counts as passed; tests that wait for a human verdict and failed tests are shown separately. |
| `crates/spawn-at-verify/tests/tui/drawer_content.rs` | Drawer content ordering, formatting, geometry table, and timeline tests. |
| `crates/spawn-at-verify/tests/tui/drawer_geometry.rs` | GEOMETRY table and RESULT "Geometry" row: Expected comes from the oracle, the mark comes from comparing it with Actual within the spec's tolerance, and the runner's verdict agrees. |
| `crates/spawn-at-verify/tests/tui/drawer_keys_clipboard.rs` | Drawer keybindings, shortcuts help modal, and clipboard copy/save tests. |
| `crates/spawn-at-verify/tests/tui/drawer_labels.rs` | Drawer-wide shared label column: width rule, cap, truncation and wrapped continuation. |
| `crates/spawn-at-verify/tests/tui/drawer_order.rs` | Drawer section order, anchor/margin de-duplication and the auto-test verdict row. |
| `crates/spawn-at-verify/tests/tui/drawer_trace.rs` | RESULT section when the log slice holds no trace for the window: "not analysed", never a clean mark, and never the run's wall-clock duration passed off as a cloak time. |
| `crates/spawn-at-verify/tests/tui/drawer_workarea.rs` | Which workarea the geometry check trusts: the probe is primary, the extension's logged bounds are a cross-check, and the RESULT section says so. |
| `crates/spawn-at-verify/tests/tui/flash.rs` | "PASS (flash)" in the TUI: its own symbol and colour in the list, its own verdict and rows in the drawer, a flash count on the category heading. |
| `crates/spawn-at-verify/tests/tui/footer_widths.rs` | Keys bar completeness at every width: no shortcut action may silently disappear. |
| `crates/spawn-at-verify/tests/tui/golden_invariants.rs` | Responsive golden file assertions and drawer grid invariant tests. |
| `crates/spawn-at-verify/tests/tui/layout_responsive.rs` | Responsive layout and breakpoint rendering integration tests. |
| `crates/spawn-at-verify/tests/tui/mod.rs` | Common test harness helpers and submodule declarations for TUI test suite. |
| `crates/spawn-at-verify/tests/tui/polish.rs` | UI polish, padding, spacing, and characterization specification tests. |
| `crates/spawn-at-verify/tests/tui_test.rs` | Integration test suite for the spawn-at-verify TUI interface. |

### GNOME extension (`assets/gnome/`)

| File | Module doc (first `//!` line of the file) |
| :--- | :--- |
| `assets/gnome/legacy/anchor.js` | spawn-at — pure anchor and bounding box calculation (Legacy GNOME 42-44). |
| `assets/gnome/legacy/cloak.js` | spawn-at — actor opacity cloaking and snapshot management (Legacy GNOME 42-44). |
| `assets/gnome/legacy/commit.js` | spawn-at — Mutter commit & frame presentation detection (Legacy GNOME 42-44). |
| `assets/gnome/legacy/dbus.js` | spawn-at — D-Bus interface definition and session export (Legacy GNOME 42-44). |
| `assets/gnome/legacy/extension.js` | spawn-at — precise window spawning, sizing, positioning and orchestration for GNOME Shell (Wayland / X11). |
| `assets/gnome/legacy/logger.js` | spawn-at — persistent session logging and enriched telemetry (Legacy GNOME 42-44). |
| `assets/gnome/legacy/pulse.js` | spawn-at — VTE / terminal cursor heartbeat and focus pulse tracking (Legacy GNOME 42-44). |
| `assets/gnome/legacy/validator.js` | Strict instruction validator with allow-listed operations and bounded numbers (Legacy GNOME 42-44). |
| `assets/gnome/modern/anchor.js` | spawn-at — pure anchor and bounding box calculation. |
| `assets/gnome/modern/cloak.js` | spawn-at — actor opacity cloaking and snapshot management. |
| `assets/gnome/modern/commit.js` | spawn-at — Mutter commit & frame presentation detection. |
| `assets/gnome/modern/dbus.js` | spawn-at — D-Bus interface definition and session export. |
| `assets/gnome/modern/extension.js` | spawn-at — precise window spawning, sizing, positioning and orchestration for GNOME Shell (Wayland / X11). |
| `assets/gnome/modern/logger.js` | spawn-at — persistent session logging and enriched telemetry. |
| `assets/gnome/modern/pulse.js` | spawn-at — VTE / terminal cursor heartbeat and focus pulse tracking. |
| `assets/gnome/modern/ready_wait.js` | spawn-at — ready-wait state machine (JS port of crates/spawn-at-core/src/ready_wait.rs). |
| `assets/gnome/modern/validator.js` | Strict instruction validator with allow-listed operations and bounded numbers. |
| `assets/gnome/modern/x11_observe.js` | spawn-at — observe-only logging of the paint signals of X11 and XWayland windows. |

### Node tests (`tests/`)

| File | Module doc (first `//!` line of the file) |
| :--- | :--- |
| `tests/cloak_test.js` | no module header |
| `tests/dispatcher_test.js` | no module header |
| `tests/extension_test.js` | no module header |
| `tests/gi_loader.js` | Node.js ESM custom loader to stub `gi://` and `resource:///` imports for unit/integration testing. |
| `tests/golden_anchor_test.js` | no module header |
| `tests/late_resize_test.js` | no module header |
| `tests/legacy_dispatcher_test.js` | no module header |
| `tests/ready_wait_test.js` | no module header |
| `tests/ready_wait_wiring_test.js` | no module header |
| `tests/wait_override_test.js` | no module header |
| `tests/x11_observe_test.js` | no module header |

## Specs read by `spawn-at-verify`

`verify/<category>/*.toml` are the test specs, `verify/suites/*.toml` the suite definitions and `verify/golden/` the shared golden data. `spawn-at-verify list` shows what is loaded; see [verify/README.md](../verify/README.md).

## Documentation site

The user guide is a Starlight (Astro) project in `docs-site/` (`astro.config.mjs`, `package.json`
with exact versions, `package-lock.json`, `.nvmrc`, `src/content/docs/`, `src/styles/custom.css`).
The rustdoc API docs are built from the source comments and published under `api/` of the same
site; the hand-written page about them is `rustdoc.md`, because a page at `/api/` would collide with
that folder. The CLI reference (`docs-site/src/content/docs/reference/spawn-at*.md`) is generated
from the built binary's `--help` at build time and is **not committed** (`.gitignore` lists it; only
`reference/index.md` is tracked), so it cannot drift from the binary. Every hand-written page ends
with a `## Sources` list.

Node.js and npm must be installed (the major version CI uses is in `docs-site/.nvmrc`). Then:

```bash
cargo xtask docs-site                      # everything below, in CI order; site in target/docs-site/site/
cargo xtask docs-cli-reference --bin target/debug/spawn-at   # only the generated reference (needs the built binary)
cargo xtask docs-check-src                 # markdown and astro.config.mjs: front matter, sidebar, Sources, forbidden text
(cd docs-site && npm ci --ignore-scripts && npm run build)   # the Starlight site alone, into docs-site/dist/
RUSTDOCFLAGS="-D warnings -A rustdoc::invalid_html_tags" cargo doc --no-deps --workspace --exclude xtask --locked
cargo xtask docs-check-html target/docs-site/site --generated-dir api/   # links, ids, rewritten flags, third-party requests
```

`xtask` is left out of the published API docs: it is internal tooling with no public API. The
`-A rustdoc::invalid_html_tags` is for one item: the help text of `--monitor` is the doc comment
`(<INDEX>, ...)` in `src/cli/args.rs`, which clap prints and rustdoc reads as an unclosed HTML tag;
fixing it would change `--help`. Every other rustdoc warning fails the build.

`.github/workflows/docs.yml` runs the same command on every pull request and, on a push to `main`,
deploys to GitHub Pages once the owner has enabled Pages with the "GitHub Actions" source. It makes
no third-party request: no external fonts, scripts or analytics, and Astro's telemetry is off
(`ASTRO_TELEMETRY_DISABLED=1` in the workflow, the npm scripts and the xtask command).

## Historical evidence

`docs/evidence/stage1/` holds raw command outputs (fmt, clippy, `--help`, test lists) captured when stage 1 of the earlier reorganization was proven to be a pure move. It is kept as a record only.
