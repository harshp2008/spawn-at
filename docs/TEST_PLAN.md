# spawn-at Test Library & Verification Plan

> Sections 6.10 to 6.35 (the dated experiment log) moved to [TEST_LOG.md](TEST_LOG.md). Every old heading stays below as a stub that links to its new place, so "TEST_PLAN 6.x" citations keep working.
>
> Paths under `scratch/` or `verify-results/` cited in this plan and in TEST_LOG.md are gitignored working files of the author's machine and are **not in the repository**: the numbers in the text are what is kept.

## 1. Overview & Architecture

`spawn-at-verify` is the dedicated verification engine and test harness for the `spawn-at` window placement CLI and GNOME Shell extension. It validates zero-flicker window creation, exact geometry placement against a mathematical oracle, and timeline event sequencing via compositor trace analysis.

Tests are defined declaratively in `verify/<category>/<test-id>.toml` specifications, executed against a real Wayland GNOME session, and presented via both an automated CLI runner and a headless/interactive Terminal User Interface (TUI).

### 1.1 Test Suite Counts & Feature Gates Baseline

Automated test execution across the workspace is partitioned by crate and feature flags:

| Target / Feature Set | Tests Passed | Tests Ignored | Breakdown by Target / Binary |
| :--- | :---: | :---: | :--- |
| **Workspace: `spawn-at` CLI** | 460 | 0 | 13 added for the 0.2.1 fixes (measured 2026-10-18 on `fix/v0-2-1-batch`): 3 in `config.rs` (headless install keeps existing `[update]` values, fills only missing keys, flags override their own key), 2 in `platform/installer.rs` (the login entry's `Exec=` with `--skip-bin` is the installed copy, never the update's temp dir; the order and fallbacks), 4 in `update.rs` (`update --check` caches nothing; the cache slot by pre-release; the silent check honours `auto_check = false`; it caches the newest tags otherwise), 1 in `session.rs` (the skipped log-out line names no flag), 3 in `platform/linux/gnome/deploy.rs` (the warnings after a failed `gnome-extensions enable` / `disable` do not claim success); 4 added for the PATH advice (`platform/installer.rs`: the guarded `case ":$PATH:"` line with the exact directory, no `>>` in the text, the `~/.profile` hint only for `<home>/.local/bin`, a directory with a space or quote single-quoted); 53 added for command coverage batch 1 (audit proposals P1, P2, P5 and gap 5, `docs/TEST_COVERAGE_AUDIT.md`): 31 in `commands/state_command_tests.rs` (`run_focus`, `run_defocus`, `run_maximize`, `run_minimize`, `run_restore`, `apply_focus_policy` against a recording backend and a default-methods backend: the driver call and its arguments, window not found, no selector, the unsupported-capability text, driver errors, and 5 characterization tests that pin the dropped 2 s state wait: the command returns Ok when the window never reaches the state), 11 in `platform/linux/gnome/tests_control.rs` (D-Bus stub service on the session bus, like the close stubs: exact method names and arguments of `FocusWindow`, `DefocusWindow`, `SetWindowState` for all four states, `MoveWindow`, the exact `ExecuteBatch` JSON of a size-only transform, a `false` reply giving `TargetNotFound`, a missing method giving `IpcError`), 10 in `tests/sway_backend_cli_test.rs` (the built binary against a recording stand-in socket: `focus` and `close` by id and class, `transform` command text with and without the default focus, unknown id exits 1 and sends nothing, `query layout|windows --json`, and `defocus`, `maximize`, `minimize`, `restore`, `query pointer` exiting 1 with the "does not support this feature" text) and 1 in `commands/spawn.rs` (characterization: `spawn --focus/--no-focus/--defocus` arm the same batch as no flag, see gap 5); 4 added for the login autostart entry on unsupported desktops: 3 in `config.rs` (`autostart_file_tests`, temporary `XDG_CONFIG_HOME`: the entry is byte-identical on GNOME Wayland, a KDE X11 session and KDE Wayland with `SPAWN_AT_BACKEND=x11`, with no backend variable in it; uninstall removes it on each and is harmless when it is gone; a reinstall overwrites a stale file and follows a new binary path) and 1 in `tests/unsupported_desktop_cli_test.rs` (the built binary, `update --check --headless` on KDE Wayland with a cleared environment, empty `PATH` and temporary `HOME`: exit 0, silent, no unsupported-desktop error); 6 added for the unsupported-desktop check: 4 in `platform/linux/registry.rs` (GNOME supported in every session type; non-GNOME Wayland or no session type without an override is unsupported and the message names the desktop, the session, GNOME, `SPAWN_AT_BACKEND=sway` and `SPAWN_AT_BACKEND=x11` with "may or may not work"; a non-GNOME X11 session is supported; forced `x11` and `sway` are supported and their candidates unchanged) and 2 in `tests/unsupported_desktop_cli_test.rs` (the built binary, display variables removed: KDE or sway Wayland and an unset desktop exit 1 with the message and an empty stdout; an X11 session and a forced `x11` on Wayland are not refused); 12 added for selecting Sway: 3 in `platform/linux/registry.rs` (forced `sway` in any case selects Sway for every desktop and D-Bus result and never asks for the GNOME connection, written as a separate test because the old-selection copy knows no `sway`; near values such as ` sway`, `sway `, `swayx` are ignored exactly as before, compared against the old copy; Sway is never a candidate without the variable, and the `sway` desktop alone still gets X11) and 9 in `tests/sway_backend_cli_test.rs` (the built binary against a stand-in IPC socket in a temporary directory, `DISPLAY` and `WAYLAND_DISPLAY` removed: forced Sway with `SWAYSOCK` unset, empty, missing, a plain file, or a socket that answers like HTTP exits 1 with a message naming `$SWAYSOCK` and no X11 words; `query layout` and `query windows` read the recorded replies, also for `SWAY`; ` sway`, `sway `, `swayx` and the `sway` desktop alone never connect to the socket); 35 added for Sway placement, close, focus and transform (EXPERIMENTAL): 5 in `platform/linux/sway/command.rs` (the exact command strings, negative coordinates, zero sizes, the content size that makes the whole container the requested size), 9 in `sway/matching.rs` (launched pid, descendant pid, app id and X11 class by the shared hint rules, empty and `*` hints never match, non-windows never match, windows from before `arm` never chosen, the 300 ms grace for an app id match, first candidate kept) and 21 in `sway/tests_write.rs` against the scripted fake IPC server (`arm` subscribes before the snapshot; one chained command for top-left, bottom-right and centre anchors with a size; a work area away from the origin gives a global position; no `resize` without `--size`; a border and title bar corrected once; a window raised to its minimum size re-anchored with its real size; exclusive focus; a refused command carries Sway's text; `post_spawn` without `arm` is an error; `disarm`; skipped old windows, other windows and garbage events; close, focus and transform by id, pid or class; an unknown target sends nothing; declared capabilities; maximize, minimize, defocus and pointer unsupported); 10 added for the read-only Sway driver (`platform/linux/sway/client.rs` and `mod.rs`, against a scripted fake IPC server in a temporary directory, `sway/fake.rs`): the `$SWAYSOCK` messages for unset, empty and missing sockets, layout and window list over the socket, cursor position unsupported, a socket that is not Sway (wrong magic, or JSON that is not outputs) refused, `connect` sends only `GET_OUTPUTS`, name and capabilities; 25 added for the Sway backend, IPC codec and types (EXPERIMENTAL, `docs/DESIGN_SWAY.md`): 12 in `platform/linux/sway/ipc.rs` (the i3-ipc frame: golden bytes for `GET_WORKSPACES` and `SUBSCRIBE` requests and for the subscribe, command and workspaces replies and a window event frame, all read from a private headless Sway 1.9; round trip of every request type; the type numbers; a stream with the wrong magic, a length over the limit and truncated frames are errors) and 13 in `platform/linux/sway/types.rs` (monitor layout of one output, of a scale-2 output with a 30 px bar and of two outputs, inactive outputs skipped; window metadata of a tiled and of a floating window from recorded `GET_TREE` JSON in `tests/fixtures/sway/`, scratchpad, XWayland and fullscreen windows; the four recorded `window` events; command and subscribe reply checking; parse errors name the reply); 4 added for the backend registry (step 10): 3 in `platform/linux/registry.rs` (every combination equals the old selection; the candidate table; GNOME is only tried when it is a candidate) and 1 in `platform/null_backend.rs`; 3 added for the update hooks (step 9): `platform/update_hooks.rs` (the asset name follows the target triple, update.rs reads no desktop variable, the provider exists); 2 added for the session actions (step 8): `platform/session_actions.rs` (session.rs names no compositor module; the provider exists); 4 added for the host (step 7): `platform/linux/host.rs` (the bin dirs, the autostart path, the executable test, and that a detached child leads its own session); 3 added for the helper status (step 6): `platform/capability.rs` thresholds, messages byte for byte, and the default `helper_status` reading the reported protocol; 4 added for the provisioner (step 5): 3 in `platform/provisioner.rs` (a backend without one needs nothing, X11 has none, the backend methods forward to a provisioner) and 1 in `gnome/tests.rs` (the GNOME driver is its own provisioner); 5 added for the capabilities (step 4 of the platform refactor): 4 in `platform/capability.rs` (the default follows the `supports_*` methods, X11 reports them, `LinuxBackend` forwards them, the narrow views forward to the backend) and 1 in `gnome/tests.rs` (the GNOME driver reports them and needs the helper); 6 added for prompt width: 4 in `prompts.rs` (every prompt and item fits 70 columns and has no trailing colon; the check refuses the old 81-character question; the registry lists every constant; no prompt literal is left outside prompts.rs), 1 in `session.rs` (the end-of-run question) and 1 in `update.rs` (the update menu items), and the existing-install question test now uses the same check; 3 more session.rs tests for the log-out question asked last (14 in all: the question is the last prompt in install, clean install, update, uninstall and X11; No leaves the warning last; skip and headless ask nothing; a failing step asks nothing; a source guard that the question text is only in session.rs); 2 added for the existing-install prompt (items short enough for an 80-column terminal; one clear "Existing: ... Installing: ..." line); 11 added for the end-of-run session action: 11 in `session.rs` (interactive Yes, headless true and false, "no log-out needed", an explicit request beating it, X11, the clean-install removal phase, standalone uninstall, a failing step, finish runs once, source guards), all with a recorder that asserts the order of steps, advice and the log-out; 18 added for the existing-install flow: 15 in `platform/upgrade.rs` (finding binaries, `-V` with failure and timeout, version parsing, the plan for the flag with and without an install, non-interactive, interactive default, prompt failure, same, newer and pre-release versions, the removal order and that the config file survives), 2 in `ui.rs` (no hand-printed headings; a clean-install run has single blank lines) and 1 in `tests/install_notice_cli_test.rs` (`--clean-install` in the help); 13 added for skipping the log-out advice: 12 in `platform/linux/gnome/logout.rs` (one per row of the decision: first install, changed file, installed after the Shell started, inside the 2 s margin, not active, extra or missing file, read errors, plus parsers and a temporary-directory read) and 1 green-line colour test in `ui.rs`; 9 added for the installer output: 8 unit tests in `ui.rs` (first section has no leading blank line, one blank line between blocks, no double blank line for any sequence of 8 blocks, heading colour, and the log-out warning: no escape codes piped, none with NO_COLOR, bold yellow when colour is on, exact sentence printed) and 1 in `tests/install_notice_cli_test.rs` (dry-run output has single blank lines); 2 added for the autostart path (`config.rs`: `XDG_CONFIG_HOME` set, and unset/empty/relative giving the unchanged `~/.config` path); CLI flag parsing, IPC client, fallback, and reposition tests; 6 added for extension protocol 6: 3 `--json` `resize` shape/compatibility tests (`commands/spawn.rs`) and 3 claim tests against protocol 5 and 6 stub services on the session bus (`platform/linux/gnome/mod.rs`); 11 added for the per-spawn wait override: 4 flag tests (`cli/tests.rs`: parse before the command, default absent, bad text still parses, hidden from short and long help), 3 `run_spawn` tests with a recording backend (`commands/spawn.rs`: flags reach the armed entry, no flags leave no override and no `wait` key, bad values dropped with one warning each) and 4 instruction tests (`platform/linux/gnome/mechanics.rs`: default bytes unchanged, override fields only when set, independent fields, old JSON still reads); 16 added for `spawn-at config` (docs/CONFIG.md): 10 in `tests/config_cli_test.rs` (the built binary with a temporary `XDG_CONFIG_HOME`), 3 `run_spawn` tests (`commands/spawn.rs`: default instruction bytes unchanged, file values reach the instruction and flags win, a bad config warns and the spawn goes on), 1 `config.rs` (the update checker keeps settings and unknown keys) and 2 `platform/linux/config_store.rs`; 8 added for the untested-platform install notice: 4 unit tests in `platform/install_notice.rs` (46 on Wayland silent, 42-44 legacy notice, other versions and X11, the confirmation gate) and 4 in `tests/install_notice_cli_test.rs` (the built binary with a stand-in `gnome-shell`, `install --dry-run --headless`; they return early on a machine without a GNOME session) |
| **Workspace: `spawn-at-core`** | 64 | 0 | 60 unit tests (`lib.rs`: 21 geometry/driver + 9 `ready_wait` + 4 `resize_report` + 9 `wait_override` + 17 `settings`) + 2 golden placement integration tests (`tests/golden_placement.rs`) + 2 `tests/ready_wait_vectors.rs` (20 shared golden scripts in one test, plus the policy check) |
| **Workspace: `xtask`** | 54 | 0 | 52 unit tests (`src/main.rs`: 6 `burn_cpu`, 2 `compiler_docs`, 16 `docs_check`, 14 `docs_cli`, 11 `docs_html`, 3 `docs_site`; counted 2026-10-18 with `cargo test -p xtask -- --list`) + 2 `burn_cpu_test` (`tests/burn_cpu_test.rs`, runs the built binary) |
| **Workspace: Node.js (Extension)** | 204 | 0 | 18 added in `tests/window_control_test.js` (the extension's `DefocusWindow` modes `desktop`, `window` and the most-recently-used fallback, windows skipped when minimized or `skip_taskbar`, the unset-input-focus fallback; `SetWindowState` `maximize`, `unmaximize`, `minimize`, `unminimize`, `restore` in its three cases, unknown state and target return false; a window on another workspace is not found by `FocusWindow`, `DefocusWindow` or `SetWindowState`); 3 in `tests/install_unsupported_test.js` (`install.sh` with a stand-in curl and spawn-at and no terminal: non-GNOME Wayland prints the notice, exits 1 and downloads nothing; `--allow-unsupported` installs, runs the installer with `SPAWN_AT_BACKEND=x11` and does not forward the flag; supported environments print nothing new, with and without the flag); 9 in `tests/wait_override_test.js` (the validator's reading of the per-spawn override: absent adds nothing, `auto` adds nothing, bounds 1 to 1500, bad values named and cut to 32 characters, legacy validator drops the fields) and 11 more in `tests/ready_wait_wiring_test.js` (44 there now: override honoured, invalid ignored, default unchanged line for line, scoped to one spawn on one extension instance); the rest as before: Golden anchor math, extension characterization, legacy/modern dispatchers, late-resize re-anchor (`tests/late_resize_test.js`), 31 in `tests/ready_wait_test.js` (the same 20 golden scripts one test each, the presence and policy checks, 8 mirrored unit cases, 1 portability check), and 33 of the 44 in `tests/ready_wait_wiring_test.js` (the 20 golden scripts again through `CommitLatch.waitForReady` on a virtual clock, plus 13 extension scenarios through `_runBatch`; the other 11 are the override tests above) (`node --test tests/*.js`); 17 added for the observe-only X11 logging (DESIGN_READY_WAIT section 12): 11 in `tests/x11_observe_test.js` (formats, default-off does no work, handler set empty after the window ends, damaged unavailable noted once) and 6 in `tests/ready_wait_wiring_test.js` (the same through the extension: logging off adds no handler or line, logging on gives the joinable lines and the same outcome, disable ends a watch) |
| **`spawn-at-verify` (bare default)** | 428 | 0 | 329 lib + 0 main + 12 `action_trace_test` + 1 `catalog_golden_test` + 5 `category_library_test` + 6 `suite_wiring_test` + 5 `run_signal_test` + 4 `run_sway_guard_test` + 7 `cli_exit_codes_test` + 20 `flash_test` + 5 `oracle_cross_check` + 1 `preflight_tools_test` + 21 `trace_analyser_test` + 6 `trace_state_test` + 6 `trace_window_scope_test`. The 6 lib tests and the `render_test` and `tui_test` binaries that need `feature = "tui"` are not compiled. |
| **`spawn-at-verify --features tui`** | 593 | 1 | 335 lib + 0 main + 12 `action_trace_test` + 1 `catalog_golden_test` + 5 `category_library_test` + 6 `suite_wiring_test` + 5 `run_signal_test` + 4 `run_sway_guard_test` + 7 `cli_exit_codes_test` + 20 `flash_test` + 5 `oracle_cross_check` + 1 `preflight_tools_test` + 13 `render_test` + 21 `trace_analyser_test` + 6 `trace_state_test` + 6 `trace_window_scope_test` + 145 `tui_test` (+ 1 ignored) + 1 `panic_hook_test` |
| **`spawn-at-verify --features tui,fixture`** | 625 | 5 | 625 measured 2026-10-18 on `fix/v0-2-1-batch` (622 on `main` before it: 3 added by `6f02fba`, then 3 here in `report/tests.rs`: pending required tests are not failed and give a PENDING badge, a failure or required skip still gives NOT QUALIFIED, an old `report.json` without `pending_required` loads); the per-binary breakdown after this is from the earlier measure: 335 lib + 22 fixture binary (`src/bin/fixture.rs`) + 0 main + 12 `action_trace_test` + 1 `catalog_golden_test` + 5 `category_library_test` + 5 `run_signal_test` + 4 `run_sway_guard_test` + 6 `suite_wiring_test` (every category a suite names has specs or is in a commented known-empty list; every spec is selected by some suite; the unreachable categories `focus` and `platform` are pinned) + 7 `cli_exit_codes_test` + 4 `fixture_test` (+ 4 ignored) + 20 `flash_test` + 5 `oracle_cross_check` + 1 `preflight_tools_test` + 13 `render_test` + 21 `trace_analyser_test` + 6 `trace_state_test` + 6 `trace_window_scope_test` + 145 `tui_test` (+ 1 ignored) + 1 `panic_hook_test` |
| **Total (Rust crates, `spawn-at-verify` counted with `tui,fixture`)** | 1203 | 5 | 460 + 64 + 54 + 625 (2026-10-18, `fix/v0-2-1-batch`; the earlier total used 10 for `xtask`); with the 204 Node tests (201 run here, 3 left to CI): 1407 passed. Older detail follows, measured before: 1140 = 447 + 64 + 10 + 619; with the 204 Node tests: 1344 passed (the 6 added by the scroll smoothing, all in `tests/tui/scroll_work.rs`: 1000 queued events are 2 draws and no rebuild with the offset equal to the clamped sum of the accelerated steps and no movement afterwards, one slow notch moves the base rows, mixed events and a resize keep their order, the ends clamp, Escape after a burst, the content cache rebuilds only on resize / raw toggle / new result / invalidation; the three end-marker tests were updated for the extra blank row below the rule (`rows[len-2]` is the rule, `base + 3` rows, the content-height test looks for the rule and `external close`); before that, the 3 added by the centred end marker, in `tests/tui/drawer_scroll.rs`: centred and exactly the inner width at 60/61/80/119/200 with the odd fill on the right and the cut below the text width, the drawn marker row at the supported widths with the row count unchanged, rebuilt on resize; the two marker tests of the previous change were updated for the new text (`last.contains(" end of details ")` instead of an exact left-aligned line); before that, the 4 added by the end-of-details marker, in `tests/tui/drawer_scroll.rs`: blank row then marker at the end, the arrows follow the position, content that fits shows the marker and no arrow, copy and save carry neither marker nor arrows; no existing test changed and no golden changed, as the goldens are built from the content without the marker; before that, the 9 added by the drawer scroll fix: 7 in `tests/tui/drawer_scroll.rs` (End and held PageDown past the end at four sizes, scroll back and Escape, Escape from any scroll state, a resize while scrolled, the maximum against the drawn frame, the details pane, the draw alone with a huge stored offset), 1 in `tests/tui/action_trace_render.rs` (one reason worded alike on every event), 1 `tests/panic_hook_test.rs`; no existing test changed; before that, the 7 added by the shared timeline renderer, all in `tests/tui/action_trace_render.rs`: golden text of the real transform and focus lines, spawn and action blocks identical to the column, humanised values and nothing dropped, missing pid left out, blank rows and wrapping, Trace-row wording; seven assertions in `tests/tui/action_trace_display.rs` that pinned the old noisy format were updated deliberately (`event(s)` count, `+0.57 ms` offsets, the `Extension logs` and `Flash note` rows, the panel and markdown text); before that, the 33 added by the extension trace of actions on top of the order and focus-return fix, all in `spawn-at-verify`: 12 in `tests/action_trace_test.rs` (the owner's real transform and focus journal lines into a timeline, cloak to reveal 7.03 ms, another window's lines excluded, the window of an action, every reason string, the inventory classes), 14 in `runner/action_trace_tests.rs` (a fake journal and a stand-in fixture: boundaries, two consecutive actions, a failing action, queries, failures, no extension, off by default, the `SetLogging` call count, transform flash certain / possible / clean) and 7 in `tests/tui/action_trace_display.rs` (drawer rows, timeline, Result panel, markdown and JSON report, and one screen with both the trace and the focus-return lines); counts re-measured after merging main; before that, the 14 added by the order and focus-return fix: 2 in `tests/tui/sidebar_order.rs` (the sidebar order built from the real catalog, as a list and as drawn), 9 in `runner/focus_return_tests.rs` (scripted compositor: already focused, retried, never lands, refused with exit code and message, no terminal, owned or vanished window never targeted, failed list is unknown), 3 in `tests/tui/verdict_feedback.rs` (the Result box chain, a failed return, no terminal found), plus 1 ignored `tests/tui/focus_chain_sway.rs` run by `scripts/focus-chain-sway.sh` on a private headless Sway; no existing test changed); before that, the 14 added by the verdict-feedback fix, all in `tests/tui/verdict_feedback.rs` against stand-in scripts: the selected-row marker in nine states, a verdict key in the next frame while the teardown is pending, each of `p x s k`, a teardown problem as a warning with the verdict kept, Esc records nothing, `r` resets, one verdict per run, "Running" shown before the work starts, an idle app has nothing pending, focus returned before the prompt with only `focus` sent to the terminal's id, a failed return as a warning, nothing sent when no window was focused, Auto unchanged, the guard refusing any other command on the user's window; one existing test, `app_flow::test_queue_pauses_on_human_case_and_resumes_only_after_verdict`, gained a `run_all_pending()` call because the key no longer runs the teardown itself); before that, the 5 added by the catalog specs: 1 matrix test of `expect.rect.anchor` in `schema/v2_tests.rs` and 4 in `tests/tui/v2_flow.rs`: the 22 catalog entries listed and grouped, the drawer lines of a real spec, a Both spec's confirmation flow, a Skipped spec; the catalog now loads 67 specs, the original 45 pinned by `catalog_golden_test`) (re-measured on the branch `feat/runner-existing-window-setup`; per-binary sums of the `test result:` lines). The 110 added since command coverage batch 1 are all in `spawn-at-verify`: 1 `catalog_golden_test`, 1 in `suite_wiring_test` (`quick` leaves out `moves-focus`), 36 `schema/v2_tests.rs` (every load rule of design 2.3, run-time substitution, matrix substitution), 26 `runner/owned_tests.rs` (ownership by recorded `Child`, ambiguous and foreign windows untouched, close by id only while the title is ours, SIGTERM then SIGKILL through the handle, teardown on drop and on panic), 25 `runner/executor_v2_tests.rs` (the executor with a fake compositor and fake actions: recorded id substituted, setup failure is `Error` and runs no action, a changed target stops the run, checks, `${nonexistent}`, Both keeps the window, interrupt, backend skip, refocus), 8 `backend.rs` (backend mirror, journal rule, requirement, private-Sway guard), 5 `run_signal_test` and 4 `run_sway_guard_test` (the real `run` binary against stand-in scripts), 4 `tui/v2_flow.rs`. |

The three `spawn-at-verify` rows and the total were re-measured on 2026-10-13 at commit `1de817f` (`cargo test -p spawn-at-verify` bare, `--features tui`, `--features tui,fixture`, summed with `awk` over the `test result:` lines; `cargo test --workspace` 427 = 158 + 23 + 10 + 236); 0 failures. The `spawn-at-core` (34), `spawn-at` (158), `xtask` (10) and Node (101) rows were re-measured on 2026-10-13 at commit `902dff3` after the ready-wait core and its JS port (section 8 of `DESIGN_READY_WAIT.md`): `cargo test --workspace` 438 = 158 + 34 + 10 + 236, `node --test tests/*.js` 101 pass, 0 failures; the verify rows did not change (352 / 338 / 236). A bare `cargo test --workspace` builds `spawn-at-verify` without features, so its number for that crate is the 236 row, not 352; do not quote it for the verify crate. The `spawn-at` (164), `spawn-at-core` (38), `xtask` (10) and Node (134) rows and the total were re-measured on 2026-10-13 after the ready-wait wiring (section 8 and 9 of `DESIGN_READY_WAIT.md`): `cargo test --workspace --features spawn-at-verify/tui,spawn-at-verify/fixture` 564 passed, 4 ignored (164 + 38 + 352 + 10), `node --test tests/*.js` 134 pass, 0 failures; the `spawn-at-verify` count with `tui,fixture` is unchanged at 352 (the bare 236 and `tui` 338 rows were not re-run). Baseline before section 6.15 (`7400e57`): 305 with `tui,fixture`; section 6.15 took it to 341 (`1da549d`), section 6.16 to 352.

**Re-measured 2026-10-13 after the per-spawn wait override** (section 10 of `DESIGN_READY_WAIT.md`; commits `44ee6b7`, `a1bb985`, `8edebca`, `80ef74a`), all on the new tree, 0 failures: `cargo test -p spawn-at-verify` 252 (was 236), `--features tui` 354 (338), `--features tui,fixture` 368 passed and 4 ignored (352); `spawn-at` 175 (164), `spawn-at-core` 47 (38), `xtask` 10 (10); `cargo test -p spawn-at -p spawn-at-core -p xtask` plus the `tui,fixture` run summed per binary with a throwaway local script (not in the repository): **600 passed, 4 ignored**; `node --test tests/*.js` 154 (134). The 16 new verify tests are 9 in `trace/wait.rs`, 4 in `runner/tests.rs`, 1 in `report/tests.rs` and 2 in `cli_exit_codes_test`. Not re-run: a bare `cargo test --workspace` number (it builds `spawn-at-verify` without features; do not quote it for that crate). Nothing here ran in a GNOME Shell.

**Re-measured 2026-10-13 after the default cap 250 and the wire paint time** (commits `e9d5c77`, `8d65a48`; section 6.21), 0 failures: `cargo test -p spawn-at-verify` 265 (was 252), `--features tui` 367 (354), `--features tui,fixture` 381 passed and 4 ignored (368); `spawn-at` 175, `spawn-at-core` 47, `xtask` 10 unchanged; `node --test tests/*.js` 154 unchanged. Total with `tui,fixture`: 613 Rust, 767 with node. The 13 new verify tests are 5 in `trace/wire.rs`, 6 in `runner/wire_paint_tests.rs`, 1 in `report/tests.rs` and 1 in `cli_exit_codes_test`. Summed per binary from the `test result:` lines; the bare `cargo test --workspace` (497, default features) is not quoted for `spawn-at-verify`.

**Re-measured 2026-10-13 after `spawn-at config`** (`docs/CONFIG.md`), 0 failures, summed per binary from the `test result:` lines: `spawn-at` 191 (was 175), `spawn-at-core` 64 (47), `xtask` 10; `spawn-at-verify` bare 267 (265), `--features tui` 369 (367), `--features tui,fixture` 383 passed and 4 ignored (381; the 2 new tests are in `no_config.rs`); node 154 unchanged. Total with `tui,fixture`: 648 Rust, 802 with node. No new live section: the one live check (a harness run and a direct spawn under a user config of `cap_ms = 7`) is recorded in `docs/CONFIG.md`.

**Re-measured 2026-10-13 after the untested-platform install notice**, 0 failures, summed per binary from the `test result:` lines: `spawn-at` 199 (was 191), `spawn-at-core` 64, `xtask` 10, `spawn-at-verify` `tui,fixture` 383 passed and 4 ignored (bare 267, `tui` 369); node 154. Total with `tui,fixture`: 656 Rust, 810 with node. Run here: GNOME Shell 46.0 on Wayland, where the real `install --dry-run --headless` prints no notice; the 42-44, 45 and 47 cases run against a stand-in `gnome-shell` on `PATH`, never on a real GNOME of those versions.

**Re-measured 2026-10-13 after the observe-only X11 logging**, 0 failures, summed per binary: `spawn-at` 199, `spawn-at-core` 64, `xtask` 10 (unchanged); `spawn-at-verify` bare 269 (was 267), `--features tui` 371 (369), `--features tui,fixture` 385 passed and 4 ignored (383; the 2 new tests are in `log_source.rs` and `runner/wire_paint_tests.rs`); node 171 (154). Total with `tui,fixture`: 658 Rust, 829 with node. Nothing ran in a GNOME Shell; the extension code is exercised against mocks only.

**Re-measured 2026-10-14 after the update-channel fix** (`all` is a deprecated alias of `beta`; section 1.1 rows above updated), 0 failures, summed per binary from the `test result:` lines: `spawn-at` 209 (was 205: 195 unit tests in `src/main.rs` + 10 `config_cli_test` + 4 `install_notice_cli_test`; 4 new in `update.rs`: `all` in a config file offers pre-releases, beta and stable offer what they should, `install` writes only `stable` or `beta`, only `all` is an alias; the `install --update-channel` parse check was added to an existing CLI test), `spawn-at-core` 64, `xtask` 10, `spawn-at-verify` `--features tui,fixture` 404 passed and 4 ignored (unchanged), node 183; Rust total 687, with node 870.

**Re-measured 2026-10-13 after the autostart path fix and the texture reader** (commits `f77b415`, `dddc95e`), 0 failures, summed per binary from the `test result:` lines: `spawn-at` 202 (was 199; 3 new in `config.rs`: absolute `Exec=` path, quoting of a path with a space, `$` and `"`, refusal of a relative path), `spawn-at-core` 64, `xtask` 10 (unchanged); `spawn-at-verify` `--features tui,fixture` 402 passed and 4 ignored (unchanged; bare and `tui` rows not re-run); node 176 (was 171; 5 new in `tests/x11_observe_test.js` for `readTextureSize` and its log fields). Total with `tui,fixture`: 678 Rust, 854 with node. Nothing ran in a GNOME Shell.

**Re-measured 2026-10-13 after the hostile-app fixture flags and cells** (commit `70dc120`, section 6.22), 0 failures, summed per binary: `spawn-at` 199, `spawn-at-core` 64, `xtask` 10 (unchanged); `spawn-at-verify` bare 276 (was 269), `--features tui` 378 (371), `--features tui,fixture` 402 passed and 4 ignored (385); node 171 (unchanged). The 17 new tests: 10 in the fixture binary (7 `fixture/behaviour.rs`, 3 flag parsing), 6 in `runner/resize_expect_tests.rs`, 1 in `category_library_test`. Total with `tui,fixture`: 675 Rust, 846 with node.

#### `fixture_test` Ignored Tests Documentation
The 4 tests marked `#[ignore]` in `crates/spawn-at-verify/tests/fixture_test.rs` require a live graphical display session (compositor) to execute:
1. `test_fixture_clean_exit_timeout`: Opens a live test window with softbuffer/winit event loop and verifies timing-based self-termination. Requires an active Wayland compositor or X11 display server.
2. `test_fixture_print_ready_output`: Spawns the GUI fixture process and waits for synchronization handshake token `[fixture] READY` emitted once the window surface renders. Requires live display server connection.
3. `test_fixture_signal_termination`: Spawns a live graphical test window and tests asynchronous SIGTERM handling during the active event loop. Requires live display server connection.
4. `test_fixture_size_matches_query_windows_on_all_protocols`: Spawns GUI test windows across Wayland and X11 backends and invokes `spawn-at query windows` via D-Bus to cross-check reported dimensions against requested frame bounds. Requires active GNOME Shell compositor running the `spawn-at` extension.

---

## 2. 59-Case Test Catalog across 8 Categories

The 59 rows are the planned catalogue: 36 are **Covered** (a spec file exists and `spawn-at-verify list` shows it: 36 tests) and 23 are **Not written** (the spec file named in the last column does not exist; checked for all 23). Nothing here was dropped by a decision; the rows are simply not written. The last 18 Covered rows are the placement resize specs of section 6.10. Since 6.22 `spawn-at-verify list` shows 45 tests: these 36 plus the nine `hostile/` cells, which are not rows of the 59.

| ID | Title | Mode | Status | Spec / Test Path |
| :--- | :--- | :---: | :---: | :--- |
| **Category: basics** | | | | |
| `preflight/binary-head` | Binary version matches repository HEAD | Auto | **Covered** | `verify/preflight/binary-head.toml` |
| `basics/version-and-help` | CLI `--help` and `--version` formatted output | Auto | **Not written** | `verify/basics/version-and-help.toml` (file does not exist) |
| `basics/instant-paint` | Instant paint without artificial delay | Auto | **Not written** | `verify/basics/instant-paint.toml` (file does not exist) |
| `basics/shell-extension-active` | Extension IPC handshake and version ping | Auto | **Not written** | `verify/basics/shell-extension-active.toml` (file does not exist) |
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
| `placement/area-screen-vs-workarea` | Boundary bounds: screen vs workarea vs unbounded | Auto | **Not written** | `verify/placement/area-screen-vs-workarea.toml` (file does not exist) |
| `placement/clamp-bounds` | Clamping window when position exceeds area | Auto | **Not written** | `verify/placement/clamp-bounds.toml` (file does not exist) |
| `placement/explicit-pos` | Explicit coordinates `--pos X Y` with custom pivot | Auto | **Not written** | `verify/placement/explicit-pos.toml` (file does not exist) |
| **Category: transforms** | | | | |
| `transforms/reposition-by-id` | Reposition existing window by numeric ID | Auto | **Covered** (GNOME; attended) | `verify/transforms/reposition-by-id.toml` |
| `transforms/maximize-and-restore` | Maximize window and restore original dimensions | Both | **Covered** (GNOME; attended) | `verify/transforms/maximize-and-restore.toml` |
| `transforms/minimize-and-restore` | Minimize window and restore it (the planned `transforms/minimize-and-focus`) | Both | **Covered** (GNOME; attended) | `verify/transforms/minimize-and-restore.toml` |
| `transforms/resize-by-id` | Resize existing window dimensions | Auto | **Covered** (GNOME attended; headless Sway unattended) | `verify/transforms/resize-by-id.toml` |
| **Category: focus** | | | | |
| `focus/activate-fixture` | Focus one of two fixture windows by id (the planned `focus/activate-by-title`) | Auto | **Covered** (GNOME attended; headless Sway unattended) | `verify/focus/activate-fixture.toml` |
| `focus/defocus-active-window` | Relinquish keyboard focus to another window | Auto | **Covered** (GNOME; attended) | `verify/focus/defocus-active-window.toml` |
| `focus/passive-spawn-no-focus` | Spawn window passively without stealing focus (`--no-focus`) | Auto | **Not written** | `verify/focus/passive-spawn-no-focus.toml` (file does not exist) |
| `focus/switch-between-windows` | Alternate focus across two managed test windows | Auto | **Covered** (GNOME attended; headless Sway unattended) | `verify/focus/switch-between-windows.toml` |
| **Category: animation-and-cloak** | | | | |
| `cloak/delayed-paint/delay-0` | App paints slowly (0 ms delay) | Auto | **Covered** | `verify/cloak/delayed-paint.toml` (one spec, `[matrix] delay = [0, 50, 150, 500]`; this row is its `delay = 0` expansion) |
| `cloak/delayed-paint/delay-50` | App paints slowly (50 ms delay) | Auto | **Covered** | `verify/cloak/delayed-paint.toml` (one spec, `[matrix] delay = [0, 50, 150, 500]`; this row is its `delay = 50` expansion) |
| `cloak/delayed-paint/delay-150` | App paints slowly (150 ms delay) | Auto | **Covered** | `verify/cloak/delayed-paint.toml` (one spec, `[matrix] delay = [0, 50, 150, 500]`; this row is its `delay = 150` expansion) |
| `cloak/delayed-paint/delay-500` | App paints slowly (500 ms delay) | Auto | **Covered** | `verify/cloak/delayed-paint.toml` (one spec, `[matrix] delay = [0, 50, 150, 500]`; this row is its `delay = 500` expansion) |
| `animation/minimize-animation` | Minimize/unminimize animation flicker regression | Both | **Not written** | `verify/animation/minimize-animation.toml` (file does not exist) |
| **Category: queries** | | | | |
| `queries/query-layout-json` | Query display monitor and workarea layout | Auto | **Covered** (GNOME; headless Sway unattended) | `verify/queries/query-layout-json.toml` |
| `queries/query-windows-lists-fixture` | Query compositor windows; a fixture window is listed | Auto | **Covered** (GNOME attended; headless Sway unattended) | `verify/queries/query-windows-lists-fixture.toml` |
| `queries/query-pointer` | Query current mouse pointer screen coordinates | Auto | **Covered** (GNOME only; Sway contract in `robustness/sway-query-pointer-unsupported`) | `verify/queries/query-pointer.toml` |
| **Category: robustness** | | | | |
| `robustness/invalid-anchor-arg` | Clean error report on invalid anchor CLI value | Auto | **Not written** | `verify/robustness/invalid-anchor-arg.toml` (file does not exist) |
| `robustness/app-crash-early-exit` | Graceful cleanup when spawned process crashes during cloak | Auto | **Not written** | `verify/robustness/app-crash-early-exit.toml` (file does not exist) |
| `robustness/nonexistent-window-id` | Transform non-existent window ID returns clean error | Auto | **Covered** (GNOME; headless Sway unattended) | `verify/robustness/nonexistent-window-id.toml` |
| **Category: platform** | | | | |
| `platform/x11-fallback` | Platform guard and fallback under X11 session | Auto | **Not written** | `verify/platform/x11-fallback.toml` (file does not exist) |
| `platform/multi-monitor-secondary` | Target placement to secondary monitor index | Both | **Not written** | `verify/platform/multi-monitor-secondary.toml` (file does not exist) |

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
| `spawn --area <AREA>` | **Covered** | `test_oracle_vs_core_explicit_pos_all_three_area_modes_cross_check` | Spec not written: `placement/area-screen-vs-workarea`. |
| `spawn --monitor <MON>` | **Not written** | Spec not written: `platform/multi-monitor-secondary` | Requires multi-head display environment. |
| `spawn --anchor <ANCHOR>`| **Covered** | `verify/placement/bottom-right.toml` | Tested all 9 anchors in oracle golden test vectors. |
| `spawn --pivot <PIVOT>` | **Covered** | `test_golden_default_pivot_all_nine_anchors` | Cross-checked across core and oracle matrix. |
| `spawn -p, --pos <X> <Y>` | **Covered** | `test_golden_clamp_explicit_pos_0_0` | Absolute and offset positioning cross-checked. |
| `spawn -s, --size <W> <H>`| **Covered** | `verify/placement/bottom-right.toml` | Sets requested initial target dimensions (400×300). |
| `spawn --clamp <CLAMP>` | **Covered** | `test_golden_window_larger_than_workarea_clamp_true` | Clamping behavior verified mathematically. |
| `spawn -m, --margin <M>` | **Covered** | `verify/placement/bottom-right.toml` | Universal margin tested against oracle math. |
| `spawn --margin-{t,b,l,r}`| **Covered** | `test_oracle_vs_core_directional_margins_cross_check`| Directional boundary margins verified. |
| `spawn --focus` | **Characterized: no effect** | `commands::spawn::tests::test_spawn_focus_modifiers_have_no_effect_today` | Accepted and parsed, never read by `spawn`: the batch always has `FocusIntent::Exclusive`. The test pins that; it is not a behavior contract. |
| `spawn --no-focus` | **Characterized: no effect** | same test; live spec `focus/passive-spawn-no-focus` not written | Accepted, currently no effect on `spawn` (works on `transform`, `maximize`, `restore`). |
| `spawn --defocus` | **Characterized: no effect** | same test; live spec `focus/defocus-active-window` not written | Accepted, currently no effect on `spawn`. |
| `spawn --json` | **Covered** | `verify/placement/bottom-right.toml` | Tested in CLI execution and parsed by TUI/runner. |
| `spawn --no-wait` | **Not written** | Spec not written: `basics/no-wait-async` (not a row of the section 2 catalog) | Asynchronous launch without compositor ack wait. |
| `transform` | **Covered** | `transforms/resize-by-id`, `reposition-by-id`, `anchor-corners` (5), `resize-flash`, `move-flash` | Dynamically moves/resizes live window; the two `-flash` specs are Both and measure nothing. |
| `query` | **Covered** | `queries/query-windows-lists-fixture`, `query-layout-json`, `query-pointer` | Compositor state inspector. |
| `focus` | **Covered** | `focus/activate-fixture`, `focus/switch-between-windows` | Brings window to foreground. |
| `defocus` | **Covered** | `focus/defocus-active-window` (GNOME), `robustness/sway-defocus-unsupported` | Unfocuses active window. |
| `maximize` | **Covered** | `transforms/maximize-and-restore` (GNOME, Both), `robustness/sway-maximize-unsupported` | Compositor window maximization. |
| `minimize` | **Covered** | `transforms/minimize-and-restore` (GNOME, Both) | Compositor window minimization. |
| `restore` | **Covered** | `transforms/maximize-and-restore`, `transforms/minimize-and-restore` | Returns maximized/minimized window to normal floating. |
| `close` | **Covered** | `test_runner_cleanup_only_ever_passes_registered_ids_to_close` | Exercised by window tracker cleanup routines. |
| `-h, --help` | **Not written** | Spec not written: `basics/version-and-help` | Standard help synopsis. |

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

Searched: `README.md`, `docs/behavior-notes.md`, `docs/legacy-gnome.md`, `AI_DEV/*.md` (since deleted; see git history), `scripts/smoke-live.sh`, `spawn-at spawn --help`, code comments, tests and `git log -S` for: min size, minimum, clamp, raised, size_raised, raised_w, raised_h, clampedSafeSize, reportedMinSize, "min size unreported". There is no CHANGELOG file in the repo. `git log -S` found no commit message that states a rule beyond the ones quoted below (`4ae8f97` added the README text, `39470eb` added the per-axis flags, `bdedd3e` added the sub-minimum smoke cases, `0c548c7` is "wip: fix sub-minimum geometry overshoot centering for legacy GTK3/VTE clients").

| # | Behaviour | Stated contract (quote and citation) | Status |
| :-: | :--- | :--- | :--- |
| 1 | Window smaller than requested is raised to the app's minimum | "Apps that refuse a requested size can still end up larger than you asked for" (`README.md:149-150`); "Toolkit coverage: ... including apps that enforce a minimum window size" (`README.md:148`) | stated |
| 2 | Position uses the window's final size | "...but they're positioned using their final size, so anchors land correctly." (`README.md:150`) | stated |
| 3 | Late resize after reveal is corrected | "If an app resizes itself after reveal ..., a 1-second listener re-applies your anchor so the window doesn't drift." (`README.md:110`) | stated |
| 4 | Wait for the app, not the compositor | "the extension watches the app's buffer settle (30ms quiet period, 120ms minimum, 1500ms hard cap) before revealing" (`README.md:103`) | stated (settle rule), but silent on what happens when the app has not answered inside 30 ms |
| 5 | Anchor math with a minimum | w' = min(max(w_t, w_min), w - m_l - m_r), anchor x/y from w', h' (`AI_DEV/spawn-at_master_architecture_blueprint.md` (since deleted; see git history), section 9.4 "Anchoring"). Design document, not user documentation | stated (internal) |
| 6 | Minimum larger than available area | "the anchored edge's margin wins; the opposite edge may overflow the work area. The anchored edge is never pushed." (blueprint 9.4, "Post-clamp margin re-evaluation", `:993`). CLI text: "Window is oversized (WxH); bottom and right margins ignored." (`src/diagnostics.rs:1296`) | stated (internal and CLI message) |
| 7 | Geometry floor before asking the compositor | "Enforces a minimum geometry floor of 100x60px before passing dimensions to Mutter" (`AI_DEV/system_survey_and_understanding.md:223` (since deleted; see git history); constants `assets/gnome/modern/extension.js:33-34`) | stated (internal) |
| 8 | Exit code when the size was raised | `scripts/smoke-live.sh` sub-minimum cases (30x20 at bottom-right) "Assert zero exit code, no false timeout warnings, and workarea bounds" (commit `bdedd3e` message) | stated (test and commit message only) |
| 9 | What the CLI prints for a raised size | Diagnostic enum doc: "The requested size was below toolkit minimums and was expanded." (`src/diagnostics.rs:149`); per-axis wording at `src/diagnostics.rs:240-257`, unit-tested at `:654-726` | stated; the number in it is the size when spawn-at read the window, wrong only while the window is still resizing (6.2 item 9, 6.4 row 4) |
| 10 | Default margin when `--margin` is omitted | `--help` says only "Universal margin in pixels applied to all boundaries". Code: `self.margin.unwrap_or(16)` (`src/cli/args.rs:134`). An example in `docs/behavior-notes.md:19` shows "clamped to (16, 16)" | **UNDEFINED** in user docs |
| 11 | Window larger than the workarea | blueprint formula clamps the size to workarea minus margins (row 5); CLI only warns (row 6). Observed: the window is not clamped by spawn-at (6.3) | **UNDEFINED** for the size itself |
| 12 | Request below the app minimum with the app's minimum unknown to the extension | `FALLBACK_USED reason="min size unreported" usingFrame=` (`extension.js:1002-1007`) is the empirical path of 6.2 item 3: the settled frame is used. No user document describes it | stated in code and by the author; **not in user docs** |
| 13 | Which size an anchor uses while the app has not yet answered | none | **UNDEFINED** (the code uses the current frame, 6.2) |
| 14 | Resize arriving more than 1 s after the last correction | README says "1-second listener" (`:110`); `ANCHOR_RETIRE_QUIET_MS = 1000` (`extension.js:35`) | stated, limits not documented |

### 6.2 Code path of a request below the app's minimum

1. **CLI sends the request.** `spawn` passes the size to the extension as a batch `SetSize`, `WaitForCommit`, `SetPositionAnchored`. The extension reports the result through the `SpawnClaimed` signal with `size_raised` (`assets/gnome/modern/dbus.js:76`, `src/platform/linux/gnome/dbus.rs:36`).
2. **`_stepSetSize`** (`extension.js:736-782`) reads the minimum with `_getMinSize` (`:298-303`: `window.get_min_size?.()`, `[0, 0]` when absent), computes `floorW = max(100, minW)`, `floorH = max(60, minH)` and `safeW = max(w, floorW)`, `safeH = max(h, floorH)`, stores them as `ctx.clampedSafeSize`, then calls `move_resize_frame(true, x, y, safeW, safeH)`. **`clampedSafeSize` protects** the compositor request from a frame smaller than 100x60 (per `AI_DEV/system_survey_and_understanding.md:223` (since deleted; see git history): client-side-decoration padding subtraction giving negative widget allocations). It is a floor for the request, not a measured app minimum.
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
   - **What the code does not have (searched, not found):** (1) no step that deliberately requests a tiny size to provoke the minimum: the only sizes sent are the user's (`mechanics.rs:149-152`, `src/platform/linux/gnome/driver.rs:302`), raised to at least 100x60 by the geometry floor (`extension.js:741-751`); `30x20` appears only as a user example in `scripts/smoke-live.sh:168-175` and in unit tests; (2) no stored or reused minimum per app; (3) nothing that tells "settled at the minimum" from "still resizing": the only discriminators are the 30 ms quiet timer and equality with the target or safe size (`commit.js:77-83`); `_isToolkitClamped` (`extension.js:906-910`, `frame > target`) only feeds the focus-pulse decision at `UNCLOAK_4B` (`:822-824`). That missing discriminator is exactly the section 2.1 bug: a late resize is read as "settled at the minimum". Whether other Shell versions expose `get_min_size` is not known (HYPOTHESIS: GNOME 45 to 48 behave the same; not tested).
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

Moved to [TEST_LOG.md](TEST_LOG.md#610-placement-resize-specs-set-loader-rule-and-live-results-2026-10-12).

### 6.11 Live results after the late-resize fix (2026-10-12)

Moved to [TEST_LOG.md](TEST_LOG.md#611-live-results-after-the-late-resize-fix-2026-10-12).

### 6.11.1 Final-state oracle, post-reveal warning and re-measure (2026-10-12)

Moved to [TEST_LOG.md](TEST_LOG.md#6111-final-state-oracle-post-reveal-warning-and-re-measure-2026-10-12).

### 6.12 Why the window is revealed at its pre-resize size: investigation (2026-10-12, read-only, nothing implemented)

Moved to [TEST_LOG.md](TEST_LOG.md#612-why-the-window-is-revealed-at-its-pre-resize-size-investigation-2026-10-12-read-only-nothing-implemented).

### 6.13 H4: does the cloak starve the app of frame callbacks? (2026-10-12, no extension change)

Moved to [TEST_LOG.md](TEST_LOG.md#613-h4-does-the-cloak-starve-the-app-of-frame-callbacks-2026-10-12-no-extension-change).

### 6.14 Mutter API leads and an "arrival wait" simulation (2026-10-12, read-only, nothing implemented)

Moved to [TEST_LOG.md](TEST_LOG.md#614-mutter-api-leads-and-an-arrival-wait-simulation-2026-10-12-read-only-nothing-implemented).

### 6.15 Flash-aware verdict: audit, ground truth from the client, validation on X11 (2026-10-13, harness only)

Moved to [TEST_LOG.md](TEST_LOG.md#615-flash-aware-verdict-audit-ground-truth-from-the-client-validation-on-x11-2026-10-13-harness-only).

### 6.16 Flash: numeric headline, per-screen visibility label (2026-10-13, harness only)

Moved to [TEST_LOG.md](TEST_LOG.md#616-flash-numeric-headline-per-screen-visibility-label-2026-10-13-harness-only).

### 6.17 BASELINE before the ready-wait change: flash on native Wayland vs XWayland vs X11 (2026-10-13, harness and docs only)

Moved to [TEST_LOG.md](TEST_LOG.md#617-baseline-before-the-ready-wait-change-flash-on-native-wayland-vs-xwayland-vs-x11-2026-10-13-harness-and-docs-only).

### 6.18 Ready-wait: live validation against the 6.17 baseline (2026-10-13, no code change)

Moved to [TEST_LOG.md](TEST_LOG.md#618-ready-wait-live-validation-against-the-617-baseline-2026-10-13-no-code-change).

### 6.19 Ready-wait: same-session A/B/C with the per-spawn override (2026-10-13, no code change)

Moved to [TEST_LOG.md](TEST_LOG.md#619-ready-wait-same-session-abc-with-the-per-spawn-override-2026-10-13-no-code-change).

### 6.20 Why arms B and C still flash under load: classification and a protocol-level re-run (2026-10-13, no code change)

Moved to [TEST_LOG.md](TEST_LOG.md#620-why-arms-b-and-c-still-flash-under-load-classification-and-a-protocol-level-re-run-2026-10-13-no-code-change).

### 6.21 Default cap 250 ms and the wire paint time: code, tests and a traced loaded A/B/C (2026-10-13)

Moved to [TEST_LOG.md](TEST_LOG.md#621-default-cap-250-ms-and-the-wire-paint-time-code-tests-and-a-traced-loaded-abc-2026-10-13).

### 6.22 Hostile apps: fixture flags, nine catalog cells and a live A/C run (2026-10-13)

Moved to [TEST_LOG.md](TEST_LOG.md#622-hostile-apps-fixture-flags-nine-catalog-cells-and-a-live-ac-run-2026-10-13).

### 6.23 Post-reinstall live check: default cap, A/D regression, XWayland signal study (2026-10-13)

Moved to [TEST_LOG.md](TEST_LOG.md#623-post-reinstall-live-check-default-cap-ad-regression-xwayland-signal-study-2026-10-13).

### 6.24 Texture reader live check and texture size as a signal (2026-10-13)

Moved to [TEST_LOG.md](TEST_LOG.md#624-texture-reader-live-check-and-texture-size-as-a-signal-2026-10-13).

### 6.25 X11 investigation, stage 1: capture rig, hold-the-cloak experiment, ping (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#625-x11-investigation-stage-1-capture-rig-hold-the-cloak-experiment-ping-2026-10-14).

### 6.26 x11-hold-damage, live A/B on the XWayland fixture (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#626-x11-hold-damage-live-ab-on-the-xwayland-fixture-2026-10-14).

### 6.27 X11 pre-map resize and pixel probe, stage 1 (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#627-x11-pre-map-resize-and-pixel-probe-stage-1-2026-10-14).

### 6.28 x11-pixel-probe and x11-premap, live on the XWayland fixture (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#628-x11-pixel-probe-and-x11-premap-live-on-the-xwayland-fixture-2026-10-14).

### 6.29 x11-pixel-probe, live retry with the Mtk.Rectangle fix (2026-10-14): stopped after phase 1, the Shell crashed in the batch

Moved to [TEST_LOG.md](TEST_LOG.md#629-x11-pixel-probe-live-retry-with-the-mtkrectangle-fix-2026-10-14-stopped-after-phase-1-the-shell-crashed-in-the-batch).

### 6.30 Regression after the removal of the X11 experiments (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#630-regression-after-the-removal-of-the-x11-experiments-2026-10-14).

### 6.31 X sync counter (`_NET_WM_SYNC_REQUEST_COUNTER`) as a client-side ready signal: feasibility (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#631-x-sync-counter-_net_wm_sync_request_counter-as-a-client-side-ready-signal-feasibility-2026-10-14).

### 6.32 A sync-capable X11 client against the winit fixture, legacy rule (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#632-a-sync-capable-x11-client-against-the-winit-fixture-legacy-rule-2026-10-14).

### 6.33 Does the X sync counter move when a client repaints? Live check with the GTK4 client (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#633-does-the-x-sync-counter-move-when-a-client-repaints-live-check-with-the-gtk4-client-2026-10-14).

### 6.34 Answering a sync request that we send ourselves: smoke (2026-10-14) (stopped after the smoke because counter A never moved)

Moved to [TEST_LOG.md](TEST_LOG.md#634-answering-a-sync-request-that-we-send-ourselves-smoke-2026-10-14-stopped-after-the-smoke-because-counter-a-never-moved).

### 6.35 Born at the target geometry: an LD_PRELOAD shim on the test client (2026-10-14)

Moved to [TEST_LOG.md](TEST_LOG.md#635-born-at-the-target-geometry-an-ld_preload-shim-on-the-test-client-2026-10-14).


## 7. Sway backend (EXPERIMENTAL)

Design, measurements and the list of decisions: [DESIGN_SWAY.md](DESIGN_SWAY.md). Nothing in this section touches GNOME; the Sway backend is only reachable with `SPAWN_AT_BACKEND=sway`.

### 7.1 Unit and integration tests (run in CI, no Sway needed)

| Group | Where | What it pins |
| :-- | :-- | :-- |
| Frame codec | `platform/linux/sway/ipc.rs` | Golden bytes read from a private headless Sway 1.9 (request frames, subscribe/command/workspaces replies, a `window` event), limits and errors. |
| JSON and conversions | `platform/linux/sway/types.rs`, `tests/fixtures/sway/` | Layout of one output, of a scale-2 output with a 30 px bar and of two outputs; window metadata of a tiled and a floating window; scratchpad, XWayland and fullscreen cases; the four recorded `window` events; reply checking. |
| Command text, matching | `sway/command.rs`, `sway/matching.rs` | The exact `RUN_COMMAND` strings; pid, descendant pid and app id rules, the 300 ms grace, never a window from before `arm`. |
| Driver against a scripted socket | `sway/mod.rs`, `sway/tests_write.rs`, `sway/fake.rs` | Layout and windows over the socket; `arm` then `post_spawn` placing with each anchor family; no `resize` without `--size`; border and title bar corrected once; a window raised to its minimum re-anchored with its real size; close, focus, transform; unsupported operations say so. |
| Window commands through the binary | `tests/sway_backend_cli_test.rs` (10 tests, batch 1) | The built binary against a recording stand-in socket: `focus` and `close` send `[con_id=5] focus` / `kill`; `transform` sends the place command (and a `focus` unless `--no-focus`); an unknown id exits 1 and sends nothing; `query layout\|windows --json`; `defocus`, `maximize`, `minimize`, `restore` and `query pointer` exit 1 with "does not support this feature" and send nothing. |
| Selection | `platform/linux/registry.rs`, `tests/sway_backend_cli_test.rs` | Only the forced value `sway` (any case, nothing trimmed) selects Sway; the old-selection equivalence test is untouched; forced Sway without a usable `$SWAYSOCK` exits 1 with a message naming it and never falls back to X11; the `sway` desktop alone does not connect to the socket. |

### 7.2 Live cases: `scripts/sway-live.sh`

Runs two private headless Sway sessions through `scripts/sway-headless.sh` with the repo's fixture window (a native Wayland client), compares everything with `swaymsg`, closes each window by its recorded id and checks that no process or directory is left. Result of the run on 2026-10-15 (Sway 1.9, one machine, GNOME session running and untouched, `gnome-shell` PID unchanged):

```text
 passed: 62   failed: 0
```

| Session | Cases |
| :-- | :-- |
| A: one 1920x1080 output | `query layout` equals `swaymsg`; center; top-right margin 24; bottom-right margin 16 with size 30x20 (Sway raises it to 75x50 and the window is placed with that size: 1829,1014); bottom-right with 600x400; top-left 500x300 with `--no-wait`; `transform` to bottom-right 700x500 (1195,555); close by id and by class; forced Sway with `SWAYSOCK` unset, empty and missing exits 1 naming `$SWAYSOCK`. Every position within 1 px of the expectation computed from the work area `swaymsg` reports; `spawn --json` `window_id` equals the container id; `query windows` equals `swaymsg`. |
| B: scale 2, 30 px bar, second output at x=960 | `query layout` equals `swaymsg` (logical pixels, work areas start at y=30); center on monitor 0; bottom-right with size 400x300 and top-left on monitor 1. |

Also tried by hand on the same private Sway while building (not in the script): a real border and title bar (`for_window ... border normal 3`): `--size 600 400` gave a container of exactly 600x400 at the requested position (content 594x372).

### 7.3 Not verified

A real Sway session or real applications; XWayland windows; Sway versions other than 1.9; i3; frame timing before the first paint (whether a window shows at its tiled position first); `--area screen` and `--area unbounded`; clients that map at a different size than the one asked; fractional scales other than 1 and 2; more than two outputs; output names other than `HEADLESS-*`; `scripts/sway-live.sh` has not been run with `shellcheck` (not installed here; `bash -n` passes).

### 7.4 The verify runner on a private headless Sway (2026-10-15)

`scripts/verify-sway.sh` (design `docs/DESIGN_RUNNER_SETUP.md` section 4.3) was run on Sway 1.9 with seven throwaway version 2 specs kept in `scratch/` (not committed, not in the catalog): `query windows` lists the fixture, resize by id with `[expect.rect]`, focus the first of two windows (`focused` / `unfocused` checks), close by id (`gone` and `process_exited` checks), defocus unsupported (exit 1 and the "does not support this feature" text), a nonexistent id, and one spec for GNOME only. Result: 6 passed, 1 skipped with `unsupported on this backend (sway): spec requires gnome`, 0 failed, 0 blocked; the private Sway and its directory were removed and no fixture process was left. A `SIGTERM` sent to the wrapper alone, during `--repeat 30`, ended the run with exit code 130 after "Interrupted: 1 of the selected tests ran", stopped Sway, and left nothing. During the runs `gnome-shell` kept its PID and the user journal held no line mentioning spawn-at. Not covered: real Sway, other Sway versions, outputs other than one 1920x1080 `HEADLESS-1`, the `Both` kind (needs a person), flash and trace (Sway has neither).

### 7.5 The catalog specs for an existing window on a private headless Sway (2026-10-15)

`scripts/verify-sway.sh` (default selection, the `headless-safe` tag) on Sway 1.9: 15 passed, 0 failed, 0 skipped, 0 blocked: `focus/activate-fixture`, `focus/switch-between-windows`, `queries/query-layout-json`, `queries/query-windows-lists-fixture`, `robustness/close-by-id`, `robustness/nonexistent-window-id`, `robustness/sway-defocus-unsupported`, `robustness/sway-maximize-unsupported`, `robustness/sway-query-pointer-unsupported`, `transforms/resize-by-id` and the five `transforms/anchor-corners/*`. With `-- --tag existing-window` all 22 entries ran: the same 15 passed and 7 were Skipped with `unsupported on this backend (sway): spec requires gnome` (`focus/defocus-active-window`, `queries/query-pointer`, `transforms/maximize-and-restore`, `minimize-and-restore`, `move-flash`, `reposition-by-id`, `resize-flash`). The first run failed one spec, `transforms/reposition-by-id`: on Sway a fresh tiled window is as large as the work area, so a position-only transform keeps 1920x1080, not the fixture's 400x300; the spec became GNOME-only. `gnome-shell` kept its PID and the user journal held no line mentioning spawn-at. **None of the GNOME specs has been run**; the Both specs need a person.
