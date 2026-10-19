# Design: tests of an existing window in the verify runner

Status: **approved and implemented, 2026-10-15 (see section 11 for what was built, where, and what differs).** Sections 1 to 10 are the design as it was approved, written when `main` was at `6d232c1` and describing the code at `ca11523`; they are kept as written, so their statements about "the current model" are about that code. Claims marked **(read)** come from reading the cited code, **(ran)** from running a command for this document (only `--help` and `list` style reads, and the existing test suites; nothing live), **(unverified)** from neither.

This document answers audit gap 8 and proposals P6 and P7 of `docs/TEST_COVERAGE_AUDIT.md` (section 7 item 8, section 6 rows P6 and P7).

## Table of contents

1. [Current model](#1-current-model)
2. [Proposed spec model](#2-proposed-spec-model)
3. [Safety](#3-safety)
4. [Backends](#4-backends)
5. [Flicker for transform (P7)](#5-flicker-for-transform-p7)
6. [The TUI and reports](#6-the-tui-and-reports)
7. [Catalog specs proposed for batch 3](#7-catalog-specs-proposed-for-batch-3)
8. [Test plan, implementation plan, risks, alternatives](#8-test-plan-implementation-plan-risks-alternatives)
9. [Open questions for the owner](#9-open-questions-for-the-owner)
10. [Citations](#10-citations)
11. [Implementation status](#11-implementation-status)

## 1. Current model

**Defined.** A spec is one TOML file, deserialized into `TestDefinition` (`crates/spawn-at-verify/src/schema/types.rs:218`). The parts that matter here:

- `subject: Option<TestSubject>`: at most one app to launch, `app = "fixture"` or `"system:<app>"` (`crates/spawn-at-verify/src/schema/types.rs:246`, `crates/spawn-at-verify/src/schema/types.rs:72`).
- `action: TestAction`: exactly one `command` plus `args` (`crates/spawn-at-verify/src/schema/types.rs:248`, `crates/spawn-at-verify/src/schema/types.rs:97`). It is a required field and a single struct, not a list. **This is where "one action per test" is enforced in the data model.**
- `expect: TestExpect` (`crates/spawn-at-verify/src/schema/types.rs:176`): `exit_code`, `stderr_absent`, `stderr_present`, `diagnostics`, `never_visible_off_target`, `rect` (oracle), `resize`. There is **no `stdout` expectation** at all, so even `query layout --json` cannot be asserted on today.
- No struct has `deny_unknown_fields` (searched **(ran)**: no match in `src/`), so a key a binary does not know is silently dropped.

**Loaded.** `TestRegistry::load_from_dir` scans `verify/**.toml` except `suites/` (`crates/spawn-at-verify/src/loader/registry.rs:20`, `crates/spawn-at-verify/src/loader/registry.rs:212-229`). The id must equal the path without extension (`crates/spawn-at-verify/src/loader/registry.rs:50`), an `[expect.rect]` must give its margin (`crates/spawn-at-verify/src/loader/registry.rs:61-73`), and the category must be in `VALID_CATEGORIES` (`crates/spawn-at-verify/src/loader/registry.rs:75`, `crates/spawn-at-verify/src/schema/matrix.rs:156`). A `[matrix]` expands into several tests and substitutes `{key}` in title, description, subject and action strings (`crates/spawn-at-verify/src/schema/matrix.rs:67-150`); an unknown `{name}` is left as it is.

**Selected.** Suites are `verify/suites/*.toml`: `categories`, `tags` (with `!tag`), `required_only` (`crates/spawn-at-verify/src/schema/suite.rs:6-52`). A suite category name is mapped to a real one by an alias list inside `SuiteDefinition::matches` (`suite.rs:36-41`), and the same list is copied a second time in `TestRegistry::filter` (`crates/spawn-at-verify/src/loader/registry.rs:150-155`). The report maps ids to categories a third time (`crates/spawn-at-verify/src/runner/models.rs:227-247`). The alias list has no `lifecycle` to `focus` entry, which `crates/spawn-at-verify/tests/suite_wiring_test.rs` records as a known gap.

**Run.** `TestRunner::run_detailed` loops over the selected tests and calls `execute_single_test` once per test and repeat (`crates/spawn-at-verify/src/runner/engine.rs:139-168`), then closes any tracked window (`engine.rs:171`). For one test (`crates/spawn-at-verify/src/runner/executor.rs`):

| Step | Where |
| :-- | :-- |
| preflight block, environment skip, dry run, `preflight-binary-check` | `executor.rs:39`, `:67`, `:92`, `:118` |
| fixture binary must exist, **only checked when the action is `spawn`** | `executor.rs:164-194` |
| enable the extension's timing log; start the flash capture (**only `spawn` of the fixture**) | `executor.rs:201-211`, `crates/spawn-at-verify/src/runner/flash.rs:126-130` |
| build the command line; the fixture is passed to `spawn-at spawn` after `--` with `--exit-after-ms` 10000 (Auto) or 300000 (Human, Both) as a crash backstop | `crates/spawn-at-verify/src/runner/validation.rs:17-92` (backstop `:70-81`) |
| snapshot window ids, run **one** `Command` with the step timeout (5 s) | `executor.rs:213-228`, `crates/spawn-at-verify/src/runner/models.rs:367` |
| attribute windows, **only for `spawn`** | `executor.rs:274` (calls `tracker.attribute_action_windows`, `crates/spawn-at-verify/src/tracker/state.rs:76`) |
| the target of a `transform` is a literal `--id` in the spec args | `executor.rs:327-338` |
| re-read the window 1300 ms later when the printed rectangle fails | `executor.rs:345-353`, `crates/spawn-at-verify/src/oracle/check.rs:96` and `:312` |
| flash check, wait-rule info, trace assessment, validation, flash policy | `executor.rs:370`, `:378`, `:386`, `:401`, `:505` |

**Traced.** The log slice of the run is read with the log source (`executor.rs:236`); on GNOME the extension's timing log is switched on over D-Bus before each test (`crates/spawn-at-verify/src/ext_logging.rs:226`, `executor.rs:202`). Whether the log source is the GNOME journal is decided from `XDG_CURRENT_DESKTOP` alone (`crates/spawn-at-verify/src/runner/engine.rs:49-58`).

**Reported.** `spawn-at-verify run` and `calibrate` write `report.json`, `report.md` and `slices/` (`crates/spawn-at-verify/src/cli/run.rs:126`). A required test that ends `Skipped` makes the qualification report say "not supported" (`crates/spawn-at-verify/src/report/builder.rs:30-38`, `:45`).

**Shown in the TUI.** `TuiApp::execute_test` builds a runner and calls `execute_single_test` **synchronously on the UI thread** (`crates/spawn-at-verify/src/tui/app/queue.rs:86-117`). An Auto test then closes the tracked windows (`queue.rs:66-69`); a Both or Human test leaves its window up and waits for a verdict (`queue.rs:70-76`), and `record_verdict` closes them (`queue.rs:184`). The drawer's SPEC section prints the action and the subject (`crates/spawn-at-verify/src/tui/views/drawer/info_sections.rs:81-128`).

**Process facts that the design depends on.**

- The fixture is started **by `spawn-at spawn`**, not by the runner (`validation.rs:47-69`). The runner never holds a handle to the fixture process: `attribute_action_windows` takes a `fixture_pid`, and `executor.rs:281` passes `None`. The only protection against a leaked fixture is its own `--exit-after-ms` and the close-by-id of the tracked window (`tracker/state.rs:226-236`).
- `execute_with_timeout` runs each step in its own process group and kills the group with SIGKILL on timeout (`crates/spawn-at-verify/src/process.rs:57-63`, `:95-101`).
- `RunOptions::test_timeout` exists (`crates/spawn-at-verify/src/runner/models.rs:338`) and has no reader **(ran: only its definition and default match)**.
- Cleanup paths: end of `run_detailed` (`engine.rs:171`), TUI `Ctrl+C` key (`crates/spawn-at-verify/src/tui/app/input.rs:10-15`), the TUI's SIGTERM, SIGINT and SIGHUP flag (`crates/spawn-at-verify/src/tui/mod.rs:78-85`, `:140-149`), and `close_test_windows` after each test (`tui/app/state.rs:372`). `run` installs **no** signal handler and the crate sets no panic hook and no `panic = "abort"` **(ran: no match)**. In raw mode `Ctrl+C` is a key event, and the event loop is blocked while a test runs, so it is only seen between tests (`crates/spawn-at-verify/src/tui/mod.rs:113-126` against `crates/spawn-at-verify/src/tui/app/queue.rs:106`).
- `WindowTracker` derives `Clone` and holds only ids (`tracker/state.rs:26-32`); nothing clones it **(ran)**.

**What this means.** A test of an existing window needs (a) a window that exists before the action, (b) its id, (c) more than one command in some tests, (d) assertions on the window after the action. None of the four exists. The planned catalog rows are listed in `docs/TEST_PLAN.md:98-127`.

## 2. Proposed spec model

### 2.1 Principle

Add **optional** things to a spec; change nothing that exists. A spec without the new keys is a version 1 spec and runs through the same code path as today. A spec with `schema_version = 2` may add: `[[setup]]` (launch our own fixture), `[[then]]` (more actions), `[[check]]` (assertions on the recorded window), and the new `[expect]` keys. `[action]` stays required and stays a single table: it is the **measured action** (the one whose flash, exit code and stderr are judged). Teardown is not a key; it is not configurable, because it is the safety mechanism (section 3).

### 2.2 Before and after

Before (today, `verify/placement/bottom-right.toml`, abbreviated): the runner launches the fixture through `spawn-at spawn` and places it.

```toml
id = "placement/bottom-right"
kind = "both"
[subject]
app = "fixture"
args = ["--title", "Verify-Placement-BottomRight", "--app-id", "verify.placement.bottomright"]
[action]
command = "spawn"
args = ["--class", "verify.placement.bottomright", "--anchor", "bottom-right", "--size", "400", "300", "--margin", "16"]
[expect]
exit_code = 0
[expect.rect]
anchor = "bottom-right"
size = [400, 300]
margin = 16
tolerance_px = 1
```

After (new, `verify/transforms/resize-by-id.toml`): the runner launches the fixture itself, finds its window, then transforms **that** window.

```toml
schema_version = 2                      # absent means 1: every current spec
id = "transforms/resize-by-id"
category = "transforms"
title = "Resize an existing window by id"
description = "Launches the fixture at 400x300, resizes that window to 600x450 centred, and checks the rectangle."
kind = "auto"
tags = ["needs-fixture", "existing-window", "moves-focus"]

[requires]
backend = ["gnome", "sway"]            # new value "sway": section 4

[[setup]]                              # one fixture, launched by the runner, not by spawn-at
name  = "w"                            # ${w} is its recorded window id
args  = ["--width", "400", "--height", "300"]   # allow-listed fixture flags only
# title, app-id, --exit-after-ms and the event directory are set by the runner, never by the spec

[action]                               # the measured action; ${w} only in --id
command = "transform"
args = ["--id", "${w}", "--size", "600", "450", "--anchor", "center", "--margin", "16", "--no-focus"]

[expect]
exit_code = 0
stderr_absent = ["[spawn-at] ERROR:"]
[expect.rect]                          # existing oracle, judged on the recorded window
anchor = "center"
size = [600, 450]
margin = 16
tolerance_px = 2
```

`[expect.rect]` is reused unchanged: for a v2 spec the runner always re-reads the recorded window after the action (the 1300 ms settle of `executor.rs:345-353`, but unconditional) and passes it as `final_rect`, which `check_geometry_final` already prefers over a printed rectangle (`crates/spawn-at-verify/src/oracle/check.rs:325-333`). No new oracle is needed.

A longer example, with a second action and checks between them (`transforms/maximize-and-restore`):

```toml
schema_version = 2
[[setup]]
name = "w"
[action]
command = "maximize"
args = ["--id", "${w}", "--no-focus"]
[[then]]                                # extra actions, run in order after [action]
command = "restore"
args = ["--id", "${w}", "--no-focus"]
[[check]]
after = 0                               # 0 = after [action], 1 = after the first [[then]]; default = after the last
window = "w"
state = "maximized"                     # present | gone | maximized | minimized | normal | focused | unfocused
[[check]]
after = 1
window = "w"
state = "normal"
[[check]]
after = 1
window = "w"
same_rect_as = "before"                 # equal (tolerance 2 px) to the rectangle read just before [action]
```

New `[expect]` keys (v2 only): `stdout_present` (substrings, `${w}` allowed), `stdout_lists_window = "w"` (stdout is a JSON array with an entry whose id and title are `w`'s). These cover `query windows` and `query layout`.

### 2.3 Rules the loader enforces for v2 (new code, only reached by v2 files)

1. `[[setup]]`, `[[then]]`, `[[check]]` and the new `[expect]` keys in a file without `schema_version = 2` are a load error. `schema_version` greater than 2 is a load error.
2. `command` in `[action]` and `[[then]]` is one of `focus`, `defocus`, `transform`, `maximize`, `minimize`, `restore`, `close`, `query`. (`spawn` stays v1; `install`, `uninstall`, `update`, `config` are never allowed here.)
3. For the window commands the **only** selector is `--id ${name}` with `name` a declared `[[setup]]` name (or the special `${nonexistent}`, section 3.5). A literal `--id`, `--class`/`-c`, `--title`/`-t`, `--pid`, `--focused` is a load error, and so is `defocus` without `--id` (with no selector it targets the focused window: `DefocusArgs::get_selector`, `src/cli/args.rs:614`).
4. A placeholder may name only a declared setup or a matrix key; setup names may not equal a matrix key. At most 3 setups per spec.
5. `[[setup]].args` may contain only fixture flags from an allow-list (`--width`, `--height`, `--min-width`, `--min-height`, `--decorations`, `--paint-delay-ms`, `--expect`, `--state-colors`, the hostile flags of `crates/spawn-at-verify/src/bin/fixture.rs:39-100`). `--title`, `--app-id`, `--exit-after-ms`, `--protocol` are set by the runner.
6. Matrix substitution (`matrix.rs:67-150`) is extended to `[[setup]]`, `[[then]]`, `[[check]]` strings.

### 2.4 Proof that existing specs keep their exact meaning

1. **No field of an existing struct changes.** `TestDefinition` gains `schema_version` (default 1), `setup`, `then`, `check`, all `#[serde(default)]`; `TestExpect` gains defaulted keys. A v1 file parses to the same values for every field it can set.
2. **Golden of the whole catalog.** Before the first commit, record `sha256` of `serde_json::to_string_pretty` of the registry (all 45 tests, expanded) and of `spawn-at-verify list` and `list --suite <each of the five>`; after each commit the digests must be equal. New fields serialize as their defaults, so the check is made with those fields skipped (`skip_serializing_if` for default values, as `Entry.wait` already does, `crates/spawn-at-core/src/driver.rs`), or the golden is taken over the v1 fields only. Either way a run that differs fails the commit.
3. **Executor path.** A v1 test never enters the new code: `execute_single_test` branches once at the top on `test.schema_version`, and the v1 body is untouched (the new v2 path lives in a new file). `crates/spawn-at-verify/src/runner/tests.rs`, `trace_capture_tests.rs` and `crates/spawn-at-verify/tests/tui/` must pass unedited.
4. **Old binary, new spec.** Because unknown keys are ignored today, an *old* `spawn-at-verify` reading a v2 file would drop `[[setup]]` and run `transform --id ${w}`. The literal `${w}` is not a number, so `spawn-at` rejects it with exit 2 and the test fails loudly instead of passing. A stale verify binary is otherwise not detected (preflight checks `spawn-at`, not the verify binary: `crates/spawn-at-verify/src/preflight.rs`); the two are built from one commit by CI.

## 3. Safety

This is the most important section. The rule: **the runner acts only on windows and processes it created itself in this test, identified by data it recorded itself, and refuses to act on anything else.**

### 3.1 What the smoke-live.sh incident was, and which guards are copied

`scripts/smoke-live.sh` once killed a shared `gnome-terminal-server`, because the PID of a window can belong to a process that hosts many windows (header comment, `scripts/smoke-live.sh:9-16`; the evidence that terminal windows share one `gnome-terminal-server` pid is `docs/TEST_LOG.md:348`). Its guards, and what this design does with each:

| smoke-live.sh guard | Copied as |
| :-- | :-- |
| the window to close is found by diffing ids before and after the spawn, closed by that recorded id, never by class or name (`:10-11`) | Window id recorded at setup; close **by id only**; an additional re-check that the id still carries our unique title (3.3). |
| a process is killed only if `safe_to_kill` passes: not this script, not an ancestor (the terminal), not a PID that owned a window before the spawn, not the focused window's PID, and it hosts exactly one window (`:12-14`, `:73-82`) | The runner never kills a PID **found** anywhere. It signals only a `Child` it spawned and has not yet reaped (3.2). The ancestor and "owned a window before" cases cannot arise because the PID is never taken from `query windows` or `/proc`. |
| "no gnome-terminal case"; terminal cases use the repo's own fixture (`:15-16`) | Setup launches **only our fixture**. `system:<app>` subjects are not allowed in `[[setup]]`. |
| fail closed: an unknown focused window means no kill at all (`:75-76`) | Any doubt (ambiguous match, pid mismatch, failed query) is an error that kills only our own child. |
| `sway-live.sh`: kill a fixture only if `comm` is `fixture` and its environment names the private socket; refuse any call whose `SWAYSOCK` is outside the private directory (`scripts/sway-live.sh:94-104`, `:126`, `:132-133`) | Section 4: the runner refuses v2 specs on `sway` unless `SWAYSOCK` is inside a directory carrying the `.spawn-at-sway-harness` marker. |

### 3.2 Ownership: the runner holds the `Child`

Setup launches the fixture **directly** (`std::process::Command`), not through `spawn-at spawn`, so the runner owns an `OwnedFixture { child: Child, title, window_id, event_dir }`:

- **Unique title per run and per setup**: `verify-<run-uuid>-<test-index>-<name>`, where the run id is the process id plus a start timestamp plus a counter (as `DIR_SEQ` does for the flash directory, `crates/spawn-at-verify/src/runner/flash.rs:39`, `:63-67`). The same string is the fixture's `--title` and `--app-id` suffix.
- **Backstop**: the runner always passes `--exit-after-ms` (10000 for Auto, 300000 for Both), exactly as `validation.rs:70-81` does today, so a SIGKILLed runner leaves a fixture that ends itself.
- **Own process group**, as `process.rs:57-63` does, so a terminal `Ctrl+C` (SIGINT to the foreground group) is not the only signal path, and so the group id equals the child pid we created.
- **Signals only through the handle**: `Child::kill()` (SIGKILL) or `libc::kill(child.id(), SIGTERM)` **only while `child.try_wait()` returns `Ok(None)`**, that is, before the child is reaped, so the PID cannot have been recycled. Never `kill` by a PID read from `query windows`, `/proc`, `pgrep` or a title.
- **Window id** is recorded only when the window matching `title == unique title` **and** `pid == child.id()` appears in `query windows --json` exactly once. The extension reports the fixture's own pid for its window (the harness already relies on this: `window_pid`, `crates/spawn-at-verify/src/runner/flash.rs:173-177`). On Sway the pid field is the window's pid, which for a directly launched native Wayland client is its own **(unverified; one look at `swaymsg -t get_tree` in a private headless Sway settles it)**. If the pid check cannot be made on a backend, the unique title alone is used and the spec's tag records it.

### 3.3 The target must be recorded, and is re-verified before every action

- The loader forbids any selector except `--id ${name}` (2.3 rule 3), so a spec cannot name a user's window.
- At run time the runner substitutes `${name}` only from the id it recorded. Just before **each** action it re-reads `query windows --json` and checks that the recorded id is present with the unique title and the child's pid. If not (the window moved on, or was replaced), it **does not run the action**: result `Error`, message "target changed: ...".
- `defocus` and any command that defaults to "the focused window" can never run without `--id` (loader rule), and the focused-window default is never relied on.

### 3.4 Teardown runs on every path

`OwnedFixture` implements `Drop`; a `SetupSession` (all fixtures of one test) is held by the tracker so every existing cleanup path reaches it. Order, for each fixture, bounded by timeouts:

1. If a window id was recorded and `query windows` still shows that id **with our title**: `spawn-at close --id <id> --no-wait` (5 s timeout, as `tracker/state.rs:226-236`). If the id is gone: skip. If the id exists with a **different** title: skip and warn (an id must never be closed because it equals a number we remember).
2. Wait up to 1 s for `child.try_wait()`.
3. Still running: SIGTERM through the handle, wait 1 s, then `child.kill()` and `child.wait()`.
4. Remove the event directory (existing `FlashCapture::drop`, `runner/flash.rs:117-122`).

| Path | How teardown is reached |
| :-- | :-- |
| normal end, assertion failure, setup error | the session is dropped at the end of `execute_single_test`'s v2 branch; Auto tests tear down there. |
| action timeout | `execute_with_timeout` kills only the action's own process group (`process.rs:95-101`); the session is then dropped normally. The action's target fixture is not in that group (the fixture has its own). |
| Both / Human test | the test must keep its window until the verdict, so the session is **moved into the tracker** instead of dropped; `record_verdict` and `close_test_windows` (`queue.rs:184`, `tui/app/state.rs:372`) drop it. |
| TUI `Ctrl+C`, quit, SIGTERM, SIGHUP | the existing paths call `cleanup_test_windows` (`input.rs:10-15`, `crates/spawn-at-verify/src/tui/mod.rs:140-149`); that function is extended to drop the session too. The event loop is blocked during a test, so the delay is bounded by setup (5 s) + action (5 s) + settle (1.3 s) + teardown (about 3 s), **about 15 s worst case**, and is stated in the docs. |
| `spawn-at-verify run` + `Ctrl+C` / SIGTERM | **new**: `run` today has no handler (it simply dies). The design adds a `signal-hook` flag checked between steps and inside the setup wait; `signal-hook` is already an optional dependency (`crates/spawn-at-verify/Cargo.toml`, feature `fixture`/`tui`) and would become non-optional. Without the flag, a killed `run` leaves a window until the fixture's `--exit-after-ms` ends it (10 s). |
| panic | Rust unwinds (no `panic = "abort"`, **(ran)**), so `Drop` runs. A test injects a panic and asserts the fake fixture was killed. |
| SIGKILL of the runner | cannot be handled; the fixture's `--exit-after-ms` ends the process, and the window goes with it. |

After teardown the runner re-reads `query windows` and records "left behind: `<id>`" in the result message if the recorded id is still there (audit P12). It is a warning in the first version, not a failure (open question 9.9).

### 3.5 Setup that fails halfway

Every row ends the test as `Error` ("setup failed: ..."), never `AutoFail`, because nothing about the product was judged. **No action is run.** Everything already created is torn down by 3.4.

| Failure | Result |
| :-- | :-- |
| fixture binary missing | `Error` before anything is launched (as `executor.rs:164-194`, extended to v2). |
| the fixture process exits at once (bad flags, no display) | `Error` with its exit code and stderr; the `Child` is reaped; nothing to close. |
| no window with our title after the setup timeout (5 s) | `Error`; our child is killed through the handle. No id was recorded, so nothing is closed by id. |
| two windows match, or one matches by title but its pid is not our child's | `Error` "ambiguous"; our child is killed; **neither window is touched** (a window we cannot prove ours is never closed). |
| `query windows` itself fails (extension down, stale protocol) | `Error`; our child is killed. |
| setup 1 of 2 succeeded, setup 2 failed | `Error`; setup 1's window is closed by its recorded id and its child reaped. |
| the runner is blocked/preflight fails | `Blocked`, as today (`executor.rs:39-64`), before any setup. |

For tests that need an id that must **not** exist (a clean error for a missing window, `robustness/nonexistent-window-id`), the placeholder `${nonexistent}` is defined as `max(id seen in query windows) + 1000000`, re-verified absent immediately before the action. It is the only way a spec can name an id it did not record, and it names one the runner has just proved is not a window.

### 3.6 The user's focus

A freshly mapped fixture takes focus on GNOME (Mutter's default for new windows; the runner does not change any GNOME setting). Directly launched, it does so without the cloak that `spawn` uses. What the design does:

- Every v2 spec that launches a window or acts on focus carries the tag `moves-focus` (audit gap 18: no such tag exists today). The `quick` suite gets `!moves-focus` added to its tags when the first such spec lands, so a quick run never steals focus by accident. `full` runs them.
- Before the first setup of a run the runner records the id of the currently focused window (the TUI already does this for the terminal, `crates/spawn-at-verify/src/tui/app/state.rs:197-214`); teardown refocuses it with `spawn-at focus --id <recorded> --no-wait` **only if that window still exists and is not focused** (as `close_test_windows` does, `crates/spawn-at-verify/src/tui/app/state.rs:372-380`). This is best effort and is stated as such.
- Specs use `--no-focus` on `transform`, `maximize`, `restore` where they do not test focus, so the action itself does not move focus. (`spawn --no-focus` is not available: it has no effect, audit gap 5.)
- Specs whose point is a visual judgment, or that animate (maximize, minimize, restore, transform flash), are `kind = "both"`: the TUI stops with the `watch` question, which also tells the owner that the run is attended.
- On headless Sway there is no user to disturb (section 4).

## 4. Backends

### 4.1 Support table

Declared support is read from the drivers (`src/platform/linux/gnome/driver.rs`, `src/platform/linux/sway/mod.rs:185-215`, `src/platform/mod.rs:327-343`, `src/platform/linux/sway/tests_write.rs:527`); the built-binary behavior is pinned by `tests/sway_backend_cli_test.rs` (the "unsupported" text is `crates/spawn-at-core/src/driver.rs:98-103`, exit code 1).

| Command | GNOME (live, owner by hand) | Sway (headless, private) | X11 |
| :-- | :-- | :-- | :-- |
| `transform` | yes | yes | out of scope here (P11) |
| `focus` | yes | yes (`[con_id=N] focus`) | out of scope |
| `defocus` | yes | **unsupported** | out of scope |
| `maximize`, `minimize`, `restore` | yes | **unsupported** | out of scope |
| `close` | yes, needs protocol 3 | yes (`kill`) | out of scope |
| `query windows`, `query layout` | yes | yes | out of scope |
| `query pointer` | yes | **unsupported** | out of scope |
| flash measurement | yes (extension clock) | no (no extension, no clock anchor) | no |
| timing log / trace | yes | no (no-op log source) | no |

### 4.2 Skipped, not failed

**The `Capabilities` struct cannot be used by the runner**: it lives in the binary crate (`src/platform/capability.rs:26`), which `spawn-at-verify` does not depend on (it dev-depends only on `spawn-at-core`, `crates/spawn-at-verify/Cargo.toml:68`), and it has no flag for focus, defocus, state or pointer (audit section 2). So support is declared **by the spec**:

- `[requires] backend = ["gnome", "sway"]` already exists (`crates/spawn-at-verify/src/schema/types.rs:24`); the value `"sway"` is new. `EnvironmentInfo::check_requirements` today knows only `gnome` and `x11` (`crates/spawn-at-verify/src/probe.rs:86-101`), so a `sway` requirement would always be unsatisfied. The change: `EnvironmentInfo` gets a `backend` field, set from `SPAWN_AT_BACKEND` (lowercased, exactly the forced values of the binary's registry) and otherwise from the existing desktop rule. This mirrors the binary's selection; it can drift from it, and the only fully honest source would be a `spawn-at query backend` command, which is a production change and is **not** proposed here (open question 9.4).
- A spec run on a backend not in its list ends `Skipped` with a fixed message **"unsupported on this backend (sway): spec requires gnome"**. No special status: `Skipped` already exists everywhere (`crates/spawn-at-verify/src/runner/models.rs:32`, `resolve_final_status` at `crates/spawn-at-verify/src/runner/models.rs:251-269`).
- If a spec **lists** a backend and the command still answers "does not support this feature", that is a **failure** (the spec claims support), not a skip.
- The unsupported behavior itself is tested as a positive: contract specs for Sway (`backend = ["sway"]`) assert exit 1 and `stderr_present = ["does not support this feature: defocus_window"]` (section 7).
- **Consequence for qualification**: a required test that is skipped makes `supported` false (`report/builder.rs:30-48`). v2 specs must therefore be `required = false` at first (as the nine hostile specs are), or a per-backend requirement notion must be added. Open question 9.5.

### 4.3 Running on headless Sway, unattended

Yes, in principle: `scripts/sway-headless.sh` starts a private, headless Sway in a `0700` directory with its own runtime dir and sockets, no XWayland, and prints exports for a subshell (`scripts/sway-headless.sh:2-26`), and `scripts/sway-live.sh` already runs the full command set against it unattended with the same fixture. Nothing in CI installs Sway today **(ran: no `sway` in `.github/workflows/`)**, so "unattended" means the owner's machine or a future job (open question 9.8).

What the runner needs for it, and the traps found while reading:

1. **A thin wrapper**, `scripts/verify-sway.sh` (allowed: it only calls binaries): `sway-headless.sh start`, run the verify binary in a subshell with `SPAWN_AT_BACKEND=sway`, `SWAYSOCK`, `WAYLAND_DISPLAY` from the harness and **`DISPLAY` and `XDG_CURRENT_DESKTOP` unset**, then `sway-headless.sh stop` in a trap. `sway-live.sh` sets the variables on each `spawn-at` call only (`sway-live.sh:133`); the wrapper does it on the one child process.
2. **Log source trap.** `TestRunner::new` chooses the GNOME journal source and the GNOME `SetLogging` control from `XDG_CURRENT_DESKTOP` alone (`engine.rs:49-58`). Run from a GNOME user's shell, a "Sway" run would still call `gdbus` on the **live GNOME session's** extension. The runner must pick the no-op source when the backend is not GNOME (decided in commit 6), and the wrapper unsets the variable as a second guard.
3. **Private-session guard in the runner.** For `backend = sway`, a v2 spec runs only if `SWAYSOCK` is inside a directory that holds the `.spawn-at-sway-harness` marker and is owned by the user and is not `XDG_RUNTIME_DIR` (the checks of `check_dir`, `scripts/sway-headless.sh:52-60`). Otherwise the result is `Blocked` with that reason. This makes "never point the runner at the owner's real Sway session" a property of the runner, not of the wrapper.
4. **No flash, no trace** on Sway: specs that assert them are skipped with "not measurable on this backend" (section 5).

## 5. Flicker for transform (P7)

### 5.1 What is measured, and what exists

Today's measurement (audit section 4.2, **(read)** again here): the fixture logs `FIRST_PAINT WxH` once per new size, plus `RESIZED` and `MOVED`, in `CLOCK_MONOTONIC` to `fixture-<pid>.events` (`crates/spawn-at-verify/src/bin/fixture/trace.rs:87-102`, `window.rs:266-270`, `:351`); with the wire dump the paint time is the client's `wl_surface.commit`. The extension logs `CLOAK`/`UNCLOAK` on its own millisecond clock, tied to the monotonic clock by a `ClockAnchor` from `ANCHOR_SAMPLES = 4` bracketed `SetLogging(true)` calls (`crates/spawn-at-verify/src/trace/flash.rs:40`, `crates/spawn-at-verify/src/runner/flash.rs:58-74`; uncertainty 1.5-3.0 ms as recorded in `docs/TEST_LOG.md` 6.16). `detect_flash` (`trace/flash.rs:466`) calls it a flash when the reveal comes before the first paint at the final size, or the size or position changes after the reveal by more than `GEOMETRY_TOLERANCE_PX = 2` within `POST_REVEAL_WATCH_MS = 1500` (`trace/flash.rs:34`, `:37`).

A runtime transform sends the batch Snapshot, Cloak, SetSize, WaitForCommit(500 ms), (SetPositionAnchored), Uncloak, DestroySnapshot (`src/platform/linux/gnome/driver.rs:289-329`), and the extension logs `CLOAK` and `UNCLOAK` for it in `_runBatch` (`assets/gnome/modern/extension.js:730`, `:744`). During the cloak the snapshot of the old content is what the user sees, so the thing to measure is the same as for spawn: **was the real window revealed before the client had painted the new size, and did anything change after the reveal.**

### 5.2 What must be added

All of it in the verify crate, none in the product:

1. **Launch for the capture**: the setup fixture gets `FlashCapture::apply` (`runner/flash.rs:77-82`), i.e. `SPAWN_AT_FIXTURE_EVENT_DIR` and, by default, `SPAWN_AT_FIXTURE_TRACE_DIR`. Because the runner owns the `Child`, the events file is `fixture-<child pid>.events` and no guess is needed (today `read_events` falls back to "the only file", `crates/spawn-at-verify/src/runner/flash.rs:99-114`).
2. **Allow capture for the new actions**: `wants_flash_capture` is `action.command == "spawn"` (`runner/flash.rs:126-130`); for v2 it is "the measured action is `transform` and the spec has a setup with `flash = true`".
3. **Anchor close to the action**: take the bracketed `SetLogging` calls after the window is mapped and just before the action, so the anchor is fresh.
4. **Judge the transform's own reveal**: `detect_flash` takes the first `UNCLOAK` in the events it is given. The runner passes it only the extension events **after the action's start** (converted with the anchor), so the setup window's own history cannot be picked. The client events need no cut: `client_flashes` already takes, as the content shown at the reveal, "the newest paint certainly before it" (`crates/spawn-at-verify/src/trace/flash.rs:684-696`), which is the paint at the setup size. The "nothing painted at the reveal" kind cannot occur for a transform; the relevant kind is `size-change-after-reveal`.
5. **Plumbing**: the existing `run_flash_check` (`runner/flash.rs:266-341`) with `window = recorded id`; `FlashInfo` is already in `TestResult` and already rendered.

Resolved by the owner's journal sample (2026-10-16, `crates/spawn-at-verify/tests/fixtures/real_transform_existing_window_trace.log`): a runtime transform's lines carry `win=[id=... pid=... class=...]` like a spawn's, so no product change is needed. The same sample showed that `focus` logs RAISE and ACTIVATE with the id, which this section first assumed could never be traced.

### 5.3 Thresholds and labels

Unchanged from spawn: a numeric flash is a lead strictly above `max(clock uncertainty, --flash-threshold-ms)`; the label is `certain` when the lead is at least one frame period (16.7 ms at 60 Hz) and `possible` below it; `visible_probability = min(1, lead/frame)` is, in the code's own words, a hypothesis (`trace/flash.rs:613-637`, audit gap 10). A run that passed and flashed keeps its pass and says so, unless `--strict-flash` (`runner/flash.rs:138-163`). The transform specs are **informational by default**: they do not fail a run on a flash, and they must not be `required` (open question 9.5).

### 5.4 What can not be measured

- **Position-only transforms** have no paint, so only the extension's `ANCHOR_CORRECTION` after the reveal is evidence (`crates/spawn-at-verify/src/trace/flash.rs:499-525`); the client's `Moved` event is not delivered by winit on native Wayland (it exists for X11, `window.rs:351`; **unverified** for GNOME Wayland, the audit notes X11 reports it). The spec says "limited evidence".
- **focus, defocus**: no geometry or paint change, so there is nothing to compare with the reveal. `focus` does have a trace (RAISE and ACTIVATE with the window id, section 5.5); `defocus` has none for its target.
- **close**: nothing is revealed and the extension logs nothing for it (section 5.5). What can be checked is that the window leaves `query windows` and the fixture process ends (section 7).
- **maximize, minimize, restore**: Mutter's animation is by design not a flicker, and a direct state change has no cloak, so there is no flash analysis. The extension logs nothing for maximize and unmaximize, and only MINIMIZE / UNMINIMIZE for the others (section 5.5).
- **Any backend without the extension** (Sway, X11): there is no `UNCLOAK` and no clock anchor (`runner/flash.rs:59-62` returns `None`).
- **Pixels**: nothing is captured. A clean result means no late paint or late move was found in the runs made, on one machine and one compositor version, with the fixture and not a real app. **It is not a guarantee that no frame was ever wrong.**

### 5.5 Implemented: what the extension logs per command, and how it is captured

**Status: implemented** in `crates/spawn-at-verify` (`src/trace/action.rs`, `src/runner/action_trace.rs`, hooks in `src/runner/executor_v2.rs`). Nothing under `assets/gnome/` or `src/` changed; the table below was read from `assets/gnome/modern/` (the `legacy/` extension has the same call sites at other line numbers). Every `logTime` call that passes a window writes `win=[id=N pid=N class="..." ...]` (`logger.js:297`, `formatWindowMeta`), and the logger buffers lines and writes them from an idle callback (`logger.js:316`), so the journal's own timestamp is the flush time and only the `+N ms` stamp is the event time.

| Command | What it logs (extension.js unless noted) | Carries the window id | Start/end for a duration |
|---|---|---|---|
| `transform` (`ExecuteBatch`, `:1592`, logs nothing itself) | `BATCH_STEP :715`, `SNAPSHOT_CREATED` (`cloak.js:150`) or `SNAPSHOT_SKIPPED :727`, `CLOAK :730`, `SET_SIZE_REQUEST :786`, `READY_WAIT_START :844`, `POSITION_SET :953`, `UNCLOAK :1129`, `UNCLOAK_4D_REVEAL :1133`, `ANCHOR_ARMED :1238`, `SNAPSHOT_DESTROYED` (`cloak.js:163`) | yes, all | yes: `CLOAK` to `UNCLOAK_4D_REVEAL` |
| `focus` (`FocusWindow :1707`) | `RAISE :1469`, `ACTIVATE :1472`; with a batch pending on the window `FOCUS_DEFERRED :1720` or `FOCUS_BATCH_ALREADY_APPLIED :1715` instead | yes | `RAISE` to `ACTIVATE` (sub-millisecond) |
| `minimize` (`SetWindowState :1737`) | `MINIMIZE :1756` | yes | no |
| `unminimize`, `restore` | `UNMINIMIZE :1763` / `:1771` (restore only if the window was minimized) | yes | no |
| `maximize`, `unmaximize` | nothing (`_maximize :303`, `_unmaximize :307` have no log call); `ANCHOR_RELEASED reason=EXTERNAL_STATE` (`:1742`, written at `:1275`) only if an anchor was armed | yes, when written | no |
| `defocus` (`DefocusWindow :1730`) | nothing in `desktop` mode; in `window` mode and the default, `RAISE`/`ACTIVATE` (`:1490`-`:1508`) for the window that **receives** focus, never for the target | not the target's | no |
| `close` (`CloseWindow :1787`) | nothing; `ANCHOR_RELEASED reason=EXTERNAL_CLOSE` (`:1792`, written at `:1275`) only if an anchor was armed, which is the line seen after a transform | yes, when written | no |
| queries (`GetWindows :1663`, `GetLayout :1631`, `GetWorkareas :1620`, `GetCursor :1611`, `GetPointer :1616`) | nothing | n/a | n/a |
| every new window (`:486`, `:531`) | `WINDOW_CREATED`, `ACTOR_MAP` for **every** window on the desktop, the user's included | yes | no |

Usable trace: `transform`, `focus`. Partial: `minimize`, `unminimize`, `restore`. Nothing for the target: `defocus`, `maximize`, `unmaximize`, `close`, queries. These are product facts for the owner, not defects the runner works around; the extension is a byte-for-byte contract (`docs/DESIGN_PLATFORMS.md` section 7) and was not touched.

**Capture** (`RunOptions::action_trace`, off by default so `cargo test` never reaches the live extension; `run` and the TUI turn it on). Per test: one `ensure_enabled` (the call every version 1 run makes), and, only if the test has a `transform`, the fixture event directory plus `ANCHOR_SAMPLES = 4` bracketed `SetLogging(true)` calls (what a fixture `spawn` makes). Per action: before it, the journal is read from the test's start second and the newest `+N ms` in it is kept as a floor; after it, the runner waits 250 ms, reads again and keeps the lines that name a **runner-owned window id**, are newer than the floor and are not `WINDOW_CREATED`/`ACTOR_MAP`/`REJECT_UNMATCHED`; for `transform` and `focus` it looks again (up to ten times, 100 ms apart) until `UNCLOAK_4D_REVEAL` / `ACTIVATE` is in. Lines of any other window, and lines without a window, are dropped before anything is stored, shown or written; the result's `log_slice` holds only the kept lines. Offsets are milliseconds from the first kept line of the action; with a clock anchor the lag from the action's start to that line is shown as "First line".

**Differences from 5.2**: the setup fixture gets the event directory through `SetupSession::with_env` (not `FlashCapture::apply`); the action's own lines are selected by the floor above, not by converting the start with the anchor (the anchor is used for the reveal time and the "First line" lag); `detect_flash` receives the fixture's events from the action's start **plus the newest paint before it**, so a paint at the new size after the reveal reads as "content was 400x300" (a size change) rather than "nothing was painted"; the post-reveal watch is 1.5 s + 0.1 s as for spawn; the first transform of a test sets the result's flash fields, informational only (no spec opts in; `--strict-flash` does not apply to version 2).

**Display** (2026-10-16, second pass): the action timeline is drawn by the same code as a spawn's. `timeline_format::parse_timeline_lines` turns journal lines into events (the spawn path's `parse_compact_timeline` now calls it), `timeline_blocks::push_event_blocks` draws them (name line, offset right-aligned with one decimal, aligned humanised rows, a blank row between events), and `clean_trace_val` humanises values (sizes `W×H`, frames `x,y W×H`, long floats to one decimal, `AT_TARGET` as `at target`; unknown values unchanged). The heading carries the window id and, when a captured line has one, the pid; one caption says offsets count from the first line of each action; an action label is shown only when several actions ran. The Trace row uses the spawn wording (`✔ clean, no violations · 16 events`) from checks applied to a transform's lines (one reveal, cloak before reveal, no size or position step or correction after the reveal); an action without a cloak phase says `checks not applied: this command has no cloak phase`, never a fake clean. The markdown report uses the same parser (with the `tui` feature). JSON is unchanged.

**Reasons** shown when there is no timeline (`TraceStatus::reason`): `not applicable: the extension logs nothing for this command`; `no extension on this backend (<backend>)`; `capture failed: <why>`; `no lines for this window in the slice`. None of them changes a verdict. Cloak and Budget show `n/a` for an action without a cloak phase.

**Limits**: no pixels are captured. A clean flash result means no late paint or move was found in the runs made, on one machine and one compositor version, with the fixture and not a real app; it is not a guarantee. The `possible` bucket (a lead below one frame period) has a known noise level (`docs/TEST_COVERAGE_AUDIT.md` gap 10). Unverified on real GNOME: that a real run of the new capture produces the timelines the unit tests build from the owner's sample lines.

## 6. The TUI and reports

**Ids and groups.** v2 specs live under `verify/transforms/`, `verify/focus/`, `verify/queries/`, `verify/robustness/`, so the id equals the path (`crates/spawn-at-verify/src/loader/registry.rs:50`) and the category is the first segment. The existing empty directories `verify/transform`, `verify/query`, `verify/lifecycle`, ... hold only a README; the new ones are different directories (`docs/TEST_COVERAGE_AUDIT.md` gap 7). `category_rank` already orders `transforms`, `focus`, `queries`, `robustness` (`crates/spawn-at-verify/src/tui/app/state.rs:71-90`) and `ResultCategory` already maps them (`runner/models.rs:227-247`, `crates/spawn-at-verify/src/report/markdown.rs:92-104`). The suites need the `lifecycle` to `focus` alias in both copies (`suite.rs:36-41`, `crates/spawn-at-verify/src/loader/registry.rs:150-155`) and `suite_wiring_test.rs` will fail until its known-empty list is updated, which is its purpose.

**Auto vs Both in the TUI.** Auto specs show PASS/FAIL, the geometry table for `[expect.rect]`, and the new **SETUP** and **THEN/CHECKS** lines in the drawer's SPEC section (`info_sections.rs:81-128` gains: setup names and flags, each action, each check with its result). Both specs show Pending until `p`/`x`, with the `watch` text; the fixture window stays up until the verdict (the session lives in the tracker, 3.4). What the human confirms is stated per spec in section 7 and is always about what a command cannot measure: animation, a jump, a blank frame, the focus ring. The Flash row and the `✱` count already render events (`docs/TEST_LOG.md` 6.16).

**The TUI writes no `report.md`.** Only `run` and `calibrate` do (`crates/spawn-at-verify/src/cli/run.rs:126`, `crates/spawn-at-verify/src/cli/calibrate.rs:376`); the TUI saves the drawer text on `w` (`tui/app/clipboard.rs:36`, `tui/app/state.rs:241`). **Recommendation: not part of this work.** It is independent of the runner change, small (call `write_report_artifacts` with the `results` map on quit), and has its own decisions (when to write; whether a pending Both row is a failure in a file). Proposed as a separate change after batch 3; the new Both specs are then recorded only through `run`, which prints Pending rows for them, until then.

## 7. Catalog specs proposed for batch 3

**Implemented 2026-10-16; the differences are in 11.5.**

"Unattended on headless Sway" means the spec can run by itself in the private Sway with no person; **no GNOME spec is unattended** (every one launches a window and takes focus, section 3.6). All are `required = false` (9.5). All `needs-fixture`, `existing-window`; all carry `moves-focus` except the pure queries on Sway.

| Id | Setup | Action | Assertion | Kind | GNOME | Sway | Unattended on headless Sway |
| :-- | :-- | :-- | :-- | :-- | :-- | :-- | :-- |
| `queries/query-windows-lists-fixture` | `w` 400x300 | `query windows --json` | exit 0; `stdout_lists_window = "w"`; `[[check]] w present` | Auto | yes | yes | yes |
| `queries/query-layout-json` | none | `query layout --json` | exit 0; `stdout_present = ["\"monitors\""]` | Auto | yes | yes | yes |
| `queries/query-pointer` | none | `query pointer` | exit 0, stdout has two integers | Auto | yes | unsupported (skipped) | n/a |
| `transforms/resize-by-id` | `w` 400x300 | `transform --id ${w} --size 600 450 --anchor center -m 16 --no-focus` | `[expect.rect]` centre 600x450, tolerance 2 | Auto | yes | yes | yes |
| `transforms/reposition-by-id` | `w` 400x300 | `transform --id ${w} --anchor top-left -m 40 --no-focus` (no `--size`) | rect top-left, margin 40, size unchanged 400x300 | Auto | yes | yes | yes |
| `transforms/anchor-corners` | `w` 400x300 | `transform ... --anchor {anchor}`, `[matrix] anchor = [top-left, top-right, bottom-left, bottom-right, center]` | rect per anchor | Auto | yes | yes | yes |
| `transforms/resize-flash` | `w` 400x300, `flash = true` | `transform --id ${w} --size 700 500 --no-focus` | rect; flash events reported (informational); `watch`: "Did the window change size once, without a blank or old-size frame?" | Both | yes | skipped (no flash basis) | n/a |
| `transforms/move-flash` | `w`, `flash = true` | `transform --id ${w} --anchor bottom-right -m 16 --no-focus` | rect; extension correction only ("limited evidence") | Both | yes | skipped | n/a |
| `transforms/maximize-and-restore` | `w` | `maximize`, then `[[then]] restore` | after 0 `maximized`; after 1 `normal` and `same_rect_as = "before"`; `watch`: "Did it maximize and come back without a jump?" | Both | yes | unsupported (skipped) | n/a |
| `transforms/minimize-and-restore` | `w` | `minimize`, then `restore` | after 0 `minimized`; after 1 `normal`; `watch` on the animation (`docs/TEST_PLAN.md` section 4) | Both | yes | unsupported (skipped) | n/a |
| `focus/activate-fixture` | `a`, then `b` (b is focused) | `focus --id ${a}` | `a` focused, `b` unfocused | Auto | yes | yes | yes |
| `focus/switch-between-windows` | `a`, `b` | `focus --id ${a}`, `[[then]] focus --id ${b}` | alternating `focused`/`unfocused` after each | Auto | yes | yes | yes |
| `focus/defocus-active-window` | `a`, `b` | `defocus --id ${b} --to-window <title of a>` | `b` unfocused; **uses the unique title of `a`, the only title a spec may name, via `${a.title}`** | Auto | yes | unsupported (skipped) | n/a |
| `robustness/close-by-id` | `w` | `close --id ${w}` | exit 0; `w` gone from `query windows`; the fixture `Child` exits within 2 s | Auto | yes | yes | yes |
| `robustness/nonexistent-window-id` | none | `transform --id ${nonexistent} --size 400 300` | exit 1; `stderr_present = ["No window with id"]` | Auto | yes | yes | yes |
| `robustness/sway-defocus-unsupported` | `w` | `defocus --id ${w}` | exit 1; `stderr_present = ["does not support this feature: defocus_window"]` | Auto | skipped (`backend = ["sway"]`) | yes | yes |
| `robustness/sway-maximize-unsupported` | `w` | `maximize --id ${w}` | same, `set_window_state` | Auto | skipped | yes | yes |
| `robustness/sway-query-pointer-unsupported` | none | `query pointer` | exit 1, `get_cursor_position` | Auto | skipped | yes | yes |

Not proposed, and why: `focus/passive-spawn-no-focus` (`spawn --no-focus`): the flag has no effect today (audit gap 5, `commands::spawn::tests::test_spawn_focus_modifiers_have_no_effect_today`), so a passing spec would be false and a failing one a known failure; it waits for the owner's decision on gap 5. Multi-monitor and X11 specs: P11 and the multi-monitor row are separate.

`${a.title}` is the one extra placeholder (the unique title, usable only as the argument of `--to-window` of `defocus`). `defocus --to-window` selects by title, and the title is the runner's own unique string.

## 8. Test plan, implementation plan, risks, alternatives

### 8.1 Tests of the runner change (no live session needed)

All with fakes, in the style of `FakeBin` (`crates/spawn-at-verify/src/runner/trace_capture_tests.rs:386`) and `crates/spawn-at-verify/tests/tui/`:

1. **Schema**: all 45 existing specs parse to identical values (the golden of 2.4); v2 parse; each loader rule of 2.3 has a failing example (v2 keys in a v1 file, literal `--id`, `--class`, `--focused`, bare `defocus`, unknown placeholder, matrix collision, 4 setups, unknown `schema_version`).
2. **Placeholder substitution** and matrix extension.
3. **OwnedFixture with a fake fixture script and a fake `spawn-at` script** (the fake answers `query windows --json` from a file the test controls): records the id by title **and** pid; a decoy window with the same title and another pid is rejected and its (real, test-spawned) process is asserted **still alive**; no window appears, two windows appear, the fixture exits at once, `query` fails, the second of two setups fails: each ends `Error`, runs no action (the fake records its calls), reaps the child, and closes by id only what was recorded.
4. **Teardown order and paths**: close-by-id before any signal; id gone is skipped; id with a different title is skipped and warned; a child that ignores SIGTERM is SIGKILLed; a panic injected in a step still tears down (`catch_unwind`); a step timeout still tears down; no signal is ever sent to a pid the test did not spawn (the fake `query` lists a decoy pid; the test asserts the decoy survives).
5. **Re-verification**: mutate the fake's window list between setup and action; the action is not run and the result is `Error`.
6. **Checks**: `state`, `same_rect_as`, `after`, `stdout_lists_window`, `[expect.rect]` on the re-read rectangle.
7. **Backends**: `check_requirements` with `backend = sway`, the `SPAWN_AT_BACKEND` mirror, the private-session guard (a socket outside a marked directory is `Blocked`), the no-op log source for a non-GNOME backend (no `gdbus` call, asserted with the fake logging control).
8. **TUI headless** (`tests/tui`): a v2 spec renders SETUP/THEN/CHECKS without panicking (audit P13); a Both v2 test keeps its fake fixture alive until `record_verdict`, then it is closed; `Ctrl+C` after a Both test closes it.
9. **Flash**: `detect_flash` fed the events after an action start picks the transform's `UNCLOAK`; a synthetic file with a setup paint and a late new-size paint after the reveal gives `size-change-after-reveal`; position-only gives only the extension correction.
10. **Doc counts and citations** by script, as in this document.

### 8.2 Implementation plan, small commits (each: fmt, clippy `-D warnings`, all suites, golden digests of 2.4)

1. `feat(verify): schema_version and v2 keys, loader rules` (schema only; no runner change; goldens).
2. `feat(verify): placeholders and matrix substitution for v2 fields`.
3. `feat(verify): OwnedFixture and SetupSession` (launch, identify, teardown, `Drop`), with the fake-based tests; not wired.
4. `feat(verify): run v2 tests in the executor` (new file; v1 path untouched), `run` signal flag, Error semantics, re-verification.
5. `feat(verify): checks and stdout expectations`.
6. `feat(verify): backend detection, sway requirement, unsupported skip, private-session guard, non-GNOME log source`.
7. `feat(verify): TUI drawer and Both-mode session ownership`; tests.
8. `feat(verify): flash measurement for transform` (after the owner's journal check, 5.2).
9. `chore: scripts/verify-sway.sh` thin wrapper and its `--dry-run`; docs.
10. Then batch 3 (separate PRs): 3a queries and Sway contract specs (safest, unattended), 3b transforms, 3c focus and close, 3d state commands and flash specs (Both). The `lifecycle` to `focus` alias and the suite tags change with 3c.

Docs in the same commits; `docs/KNOWLEDGE.md` and `CHANGELOG.md` per the repo rules.

### 8.3 Risks

- **The directly launched fixture is not a spawn.** It is not placed or cloaked by spawn-at, which is what these tests want, but it means these specs say nothing about `spawn`. Intended.
- **Focus theft** on GNOME is unavoidable and only mitigated (3.6).
- **`query windows` pid/title semantics** differ per backend (unverified on Sway, 3.2); the guards degrade to title-only plus a tag, and a mismatch is an error, never an action.
- **Id reuse.** Ids are monotonic on GNOME and Sway as far as read, but the design does not depend on it: close re-checks the title.
- **Timing flakiness**: setup waits poll `query windows` every 100 ms for 5 s; the settle is 1300 ms; a loaded machine can exceed them. Failures are `Error` (setup) not `Fail`, and `--repeat` exists.
- **Stale verify binary** and the ignored unknown keys (2.4 item 4).
- **Blocking UI**: the TUI still blocks during a test (`queue.rs:106`); the 15 s bound is documented, not removed. Moving execution to a thread is a separate change.
- **Runner change touches the safety-critical tracker**: contained by keeping v1 paths byte-identical and testing teardown first (commit 3 before 4).
- **A runtime-transform trace without window id** (5.2) could force a JS change.

### 8.4 Alternatives considered and rejected

| Alternative | Why not |
| :-- | :-- |
| A shell wrapper: launch the fixture, read its id, then call `spawn-at-verify run --glob` with the id in the environment | One id per process cannot serve a suite; no per-test teardown on failure or in the TUI; duplicates the guards in shell (the incident was a shell script); no TUI rows; the spec still could not name the id. |
| Launch the setup fixture with `spawn-at spawn` as today | The runner would again hold no process handle (`executor.rs:281`), so a leaked fixture could only be stopped by pid discovery, which is what the rules forbid; and it would test `spawn` inside every non-spawn test. Kept only as an idea for specs that *want* a placed start. |
| Make `[action]` an array (`[[action]]`) | Changes the meaning and the type of a required v1 field; every v1 spec would need touching or an untagged enum. `[[then]]` adds without changing. |
| Judge every command only by its exit code | `transform` and the state commands return 0 when the state never changed (audit gap 6, pinned by `src/commands/state_command_tests.rs`), so a query after is the only honest check. |
| One shared fixture for the whole suite | Faster, but one test's state (focus, maximized) leaks into the next and a failure strands later tests. Possible later as an opt-in. |
| Real apps (gnome-calculator, terminal) as targets | The incident; the repository's contributor rules forbid it; their windows are shared or slow. |
| Read capabilities from the binary's `Capabilities` | Not reachable from the verify crate and incomplete (4.2). |
| Screen capture for flicker | No capture path exists (`docs/DESIGN_READY_WAIT.md` section 12, the ScreenCast attempt); out of scope. |
| Run Sway tests in CI now | No Sway in the workflows; availability on the runner is unverified. |

## 9. Open questions for the owner

Each with my recommendation.

1. **Launch the setup fixture directly (runner owns the `Child`) or through `spawn-at spawn`?** Recommend **directly**: process ownership is the core safety property (3.2).
2. **Should the TUI write `report.json`/`report.md`?** Recommend **a separate change after batch 3**, not in this work (section 6).
3. **A distinct `Unsupported` result instead of `Skipped` with a fixed message?** Recommend **Skipped plus the fixed prefix** for now: no change to statuses, report or TUI. Revisit when the first Sway qualification row is wanted.
4. **How does the runner learn the backend?** Mirror `SPAWN_AT_BACKEND` and the desktop rule now. A `spawn-at query backend` command would be exact but is a product change; recommend **not now**.
5. **Required or not?** Recommend **`required = false`** for every v2 spec at first, so `release-gate` and `supported` do not change, and revisit per backend (a skipped required test blocks "supported", 4.2).
6. **Focus policy:** tag `moves-focus`, `!moves-focus` in `quick`, best-effort refocus of the recorded window. Recommend **yes**. Any GNOME run of these specs is attended.
7. **Handle `Ctrl+C` and SIGTERM in `run`** (promote `signal-hook` to a normal dependency). Recommend **yes**, in commit 4.
8. **Headless Sway in CI?** Recommend **not in this work**; first run `scripts/verify-sway.sh` by hand, then decide whether a non-required job is worth installing Sway on the runner (unverified that it is available).
9. **Leftover after teardown**: warning or failure? Recommend **warning in the first version**, failure once it has been quiet for a release.
10. **Two or more windows in one spec** (focus switching needs two): allowed up to 3. Recommend **yes**.
11. **Journal check for the transform trace** (5.2): the owner reads the journal once, read-only, around one manual `spawn-at transform`, and confirms `CLOAK`/`UNCLOAK` carry the window id. Recommend doing it before commit 8.
12. **`spawn --focus/--no-focus/--defocus`** (audit gap 5) decides whether `focus/passive-spawn-no-focus` can exist. Recommend deciding it before batch 3c, independently of this design.

## 10. Citations

All `path:line` references in this file are checked by a script (`scratch/cite_design.sh`, not committed): each path must resolve to one tracked file, each line must be within the file, and the printed line is compared by eye with the claim. Line numbers in sections 1 to 10 are for `main` at `ca11523` (read with `git show ca11523:<path>`); those in section 11 are for the tree of the pull request that implemented it. Documents cited without a line are cited by section.

## 11. Implementation status

Implemented on branch `feat/runner-existing-window-setup`, 2026-10-15, as approved (owner decisions on section 9 below). The line citations of **this section** are checked against the current tree; those of sections 1 to 10 describe `main` at `ca11523`, before the implementation, and are checked against that commit (`git show ca11523:<path>`).

Update 2026-10-16 (`feat/trace-existing-window`): section 5 is implemented (5.5). `runner/flash.rs` gained about 70 lines (`evaluate_action`, `env_pairs`) and `trace/flash.rs` one field, so the `runner/flash.rs` and `trace/flash.rs` line numbers cited in sections 5.1 to 5.4 are those of the code as designed; today `FlashCapture::apply` is `runner/flash.rs:77`, `read_events` `:114`, `wants_flash_capture` `:141`, `apply_flash_policy` `:153`, `run_flash_check` `:348`, `detect_flash` `trace/flash.rs:472`, `label_visibility` `:619`. Open question 11 (the journal check) is answered by the owner's sample.

Update 2026-10-16 (`fix/tui-order-and-focus-return`): the focus return above did not work on the owner's GNOME run, for reasons visible in the code: it existed only for version 2 specs (version 1 `Both` specs, i.e. all of placement and cloak, never got focus back after the automatic part); the TUI's own refocus used an id read once at start through a 500 ms query whose failure was swallowed, then a fire-and-forget `focus --no-wait` that was never checked or retried and was sent right after an asynchronous close. Now the TUI (`TuiApp::return_focus_to_terminal`, `runner/focus_return.rs`) does the return after the automatic part of every judged test and again after the verdict, with a settle, up to three sends, a polled check after each, and a `FocusReport` (recorded terminal, each command's exit code and message, what had focus afterwards) shown in the Result box. `RunOptions::caller_returns_focus` stops the executor from doing its own unrecorded return. Covered on a private headless Sway (`scripts/focus-chain-sway.sh`); GNOME-only reasons (focus-stealing prevention, the terminal missing from the extension's list) are shown by the report but cannot be tested here.

Update 2026-10-16 (`fix/tui-verdict-feedback`): the TUI no longer runs a test or the verdict teardown inside the key handler. `TuiApp::handle_key` queues a `Pending` step that the event loop runs after the next redraw (`tui/app/queue.rs`), so the "synchronously on the UI thread" sentences and the `queue.rs`/`tui/app/state.rs` line numbers in sections 1 to 6 describe the code as it was when the design was written. The focus memory (`remember_focus`) is now also used after the automatic part of a Both or Human test (`SetupSession::restore_focus`).

### 11.1 Commits, in order

| Plan item (section 8.2) | Commit | Where |
| :-- | :-- | :-- |
| (added) golden of the loaded catalog | `ea4b986` | `crates/spawn-at-verify/tests/catalog_golden_test.rs:52`, `crates/spawn-at-verify/tests/golden/catalog_v1.json` |
| 1 schema and loader rules | `18c979a` | `crates/spawn-at-verify/src/schema/types.rs:233` (`TestSetup`), `:269` (`TestCheck`), `:290` (`SpecV2`); `crates/spawn-at-verify/src/schema/v2.rs:267` (`validate`), `:137` (`validate_action`), `:50` (`SETUP_FLAGS`) |
| 2 placeholders, matrix | `3ced4c4` | `crates/spawn-at-verify/src/schema/v2.rs:450` (`substitute`), `crates/spawn-at-verify/src/schema/matrix.rs` (`expand_matrix`) |
| 3 `SetupSession` | `a2b8312` | `crates/spawn-at-verify/src/runner/owned.rs:319` (`SetupSession`), `:466` (`launch`), `:576` (`verify_target`), `:653` (`teardown`), `:750` (`Drop`) |
| 4 executor, `run` signals, 5 checks | `c78321f` | `crates/spawn-at-verify/src/runner/executor.rs:138` (the one branch), `crates/spawn-at-verify/src/runner/executor_v2.rs:174` (`execute_v2_test`), `:254` (`run_v2_steps`), `:144` (`eval_check`), `crates/spawn-at-verify/src/cli/run.rs:97` (signal handlers), `crates/spawn-at-verify/src/tracker/state.rs:31` (`OwnedSessions`) |
| 6 backend, Skipped, Sway guard, journal fix | `77ab5d2` | `crates/spawn-at-verify/src/backend.rs:36`, `:53`, `:114`; `crates/spawn-at-verify/src/runner/executor.rs:125`; `crates/spawn-at-verify/src/runner/engine.rs:68` |
| 7 TUI, moves-focus | `38d3c46` | `crates/spawn-at-verify/src/tui/views/drawer/info_sections.rs:102`, `verify/suites/quick.toml`, `crates/spawn-at-verify/tests/tui/v2_flow.rs:217` |
| 9 wrapper | `63b9772`, `4c6c2b0` | `scripts/verify-sway.sh:101` |
| (added) repeat after a signal | `ee56044` | `crates/spawn-at-verify/src/runner/engine.rs:197` |
| 8 flash for transform, 10 catalog specs | not done | the next steps; no catalog spec was added |

### 11.2 Deviations from the design, and why

- **Checks landed with the executor** (design commits 4 and 5 are one commit): the executor cannot be tested without them.
- **`[expect]` judges `[action]`; every `[[then]]` must exit 0** (a nonzero exit is a `Fail`). The design left "the last action" and "the measured action" both readable; the measured action won, as section 2.1 says.
- **`moves-focus` is enforced**, not just recommended: a spec with a `[[setup]]` and no such tag is a load error (`crates/spawn-at-verify/src/schema/v2.rs:13`, `:267`).
- **More load errors than listed in 2.3**: `never_visible_off_target`, `[expect.resize]` and `diagnostics` in version 2, `[expect.rect]` without a setup, `--id=...`, `--`, a setup `timeout_ms` outside 1 to 60000. All have a test.
- **`flash` in `[[setup]]` is absent**: it belongs to the flash commit; `[[setup]]` rejects unknown keys (`deny_unknown_fields`), so adding it later is a visible change.
- **The Sway guard applies to every test** when the backend is `sway`, not only to version 2 specs, so no version 1 test can reach a real Sway either.
- **`check_requirements` for version 1 specs is backend-aware** when `SPAWN_AT_BACKEND` forces a backend (`gnome` means the backend is GNOME, not just the desktop). Unforced, the meaning is the old one; the loaded catalog and the `list` output are identical to `main`. The message for a version 1 mismatch is unchanged; the fixed "unsupported on this backend" text is for version 2 specs.
- **Refocus** uses `spawn-at focus --id <id> --no-wait` on the window that had focus at the start (`crates/spawn-at-verify/src/runner/owned.rs:404`, `:653`); it is the one deliberate action on a window the run did not create, as section 3.6 describes, and has tests for "still focused" and "gone".
- **`signal-hook` became a normal dependency** (it was optional behind `fixture` and `tui`); `Cargo.lock` is unchanged and the three non-Linux checks pass.
- **A signal during `--repeat`** drops the cut-short repetition instead of calling the test flaky (found on a live run; `crates/spawn-at-verify/src/runner/engine.rs:197`).
- **`scripts/verify-sway.sh` runs the runner in the background and forwards a signal** (found on a live run: bash held the trap back while the child ran).
- **Not changed**: the TUI's `ExtensionLoggingGuard` still talks to the GNOME extension (the TUI is for attended GNOME runs); `run --help` is unchanged and does not mention exit code 130 (it is in `verify/README.md`).

### 11.3 Owner decisions applied (section 9)

1 launch directly, the runner holds the `Child`; 2 the TUI still writes no `report.md` (not part of this work); 3 `Skipped` with the fixed message, no new status; 4 `SPAWN_AT_BACKEND` is mirrored, no `query backend`; 5 version 2 specs stay `required = false` (none exist yet); 6 the tag `moves-focus`, excluded from `quick`; 7 `run` handles Ctrl-C, SIGTERM and SIGHUP; 8 no Sway in CI; 9 a window left after teardown is a warning; 10 up to 3 setups; 11 the journal check for the transform trace is deferred to the flash commit; 12 nothing depends on `spawn --no-focus`.

### 11.4 The safety guarantees and their tests

| Guarantee | Test |
| :-- | :-- |
| the runner owns the `Child`; signals only through it | `crates/spawn-at-verify/src/runner/owned_tests.rs`: `test_a_window_with_our_title_but_another_pid_is_not_ours_and_survives`, `test_a_child_that_ignores_sigterm_is_killed` |
| a window is recorded only for exactly one title match with the child's pid | `test_launch_records_the_window_by_unique_title_and_child_pid`, `test_two_windows_with_the_title_are_ambiguous_and_nothing_is_closed` |
| the only selector is `--id ${name}`; literal ids, `-c`, `-t`, `--pid`, `--focused`, bare `defocus` refused | `crates/spawn-at-verify/src/schema/v2_tests.rs`: `test_literal_id_is_a_load_error`, `test_other_selectors_are_load_errors`, `test_bare_defocus_is_a_load_error`, `test_window_commands_need_exactly_one_id` |
| the runner re-validates a spec the loader would refuse | `crates/spawn-at-verify/src/runner/executor_v2_tests.rs`: `test_a_spec_the_loader_would_refuse_is_refused_by_the_runner_too` |
| the target is re-verified before each action | `test_a_changed_target_stops_the_run_before_the_next_action` |
| close only by id and only while the id carries our title | `owned_tests.rs`: `test_teardown_skips_the_close_when_the_id_carries_another_title`, `test_teardown_does_not_close_when_the_title_cannot_be_checked` |
| teardown on a normal end, a setup error, a timeout, a panic | `test_dropping_the_session_tears_down`, `test_a_panic_still_tears_down`, `executor_v2_tests.rs`: `test_a_panic_in_an_action_still_tears_down`, `test_an_action_that_cannot_run_is_an_error_and_tears_down` |
| teardown on Ctrl-C, SIGTERM, SIGHUP | `crates/spawn-at-verify/tests/run_signal_test.rs`: `test_sigterm_ends_run_with_teardown_and_exit_130` and the SIGINT, SIGHUP and `--repeat` variants |
| a halfway setup failure is `Error`, never `Fail`, and runs no action | `executor_v2_tests.rs`: `test_halfway_setup_failure_is_an_error_and_runs_no_action`, `test_second_setup_failing_closes_the_first_and_runs_nothing` |
| an ambiguous match touches nothing | `owned_tests.rs`: `test_two_windows_with_the_title_are_ambiguous_and_nothing_is_closed` |
| a Both test keeps its window until the verdict; every cleanup path reaches it | `executor_v2_tests.rs`: `test_a_test_a_person_judges_keeps_its_window_until_the_tracker_cleans_up`, `crates/spawn-at-verify/tests/tui/v2_flow.rs`: `test_a_both_test_keeps_its_window_until_the_verdict`, `test_ctrl_c_while_a_both_test_waits_closes_its_window` |
| no GNOME journal or `SetLogging` when another backend is forced | `crates/spawn-at-verify/src/backend.rs` tests, `crates/spawn-at-verify/tests/run_sway_guard_test.rs`: `test_gnome_journal_is_not_used_when_sway_is_forced` |
| no run against a Sway that is not the private session | `backend.rs`: `test_guard_refuses_everything_else`, `run_sway_guard_test.rs`: `test_a_sway_that_is_not_the_private_session_blocks_the_test` |
| `quick` never selects a window-launching spec | `crates/spawn-at-verify/tests/suite_wiring_test.rs`: `test_quick_leaves_out_moves_focus_specs_and_full_keeps_them`, `v2_tests.rs`: `test_a_spec_that_launches_a_window_must_carry_the_moves_focus_tag` |
| the 45 existing specs and the five suites are unchanged | `crates/spawn-at-verify/tests/catalog_golden_test.rs:52`, and the `list` hashes in the pull request |

Each guard in the table was broken on purpose once (loader rules, `owned.rs`, `executor_v2.rs`, `backend.rs`, the signal handler, the repeat fix, the quick tag), a test failed each time, and the code was restored; the pull request lists them. One guard cannot be broken alone: the `--id` value check in `validate_action` is also caught by the later placeholder check (both layers have tests).

### 11.5 Section 7 (the catalog specs): implemented 2026-10-16

All the specs of section 7 exist under `verify/queries/`, `verify/transforms/`, `verify/focus/` and `verify/robustness/` (18 files; the anchor matrix expands to five entries, so 22 catalog entries and 67 in all with the original 45). Differences from the table of section 7, with reasons:

- **Ids**: `queries/query-windows-lists-fixture`, `queries/query-layout-json`, `focus/activate-fixture` and `transforms/minimize-and-restore` are the names used (the plan's `focus/activate-by-title` and `transforms/minimize-and-focus` did not match what a version 2 spec can do).
- **`transforms/reposition-by-id` is GNOME only**, not "gnome, sway": on Sway a fresh tiled window is as large as the work area, so a position-only transform keeps 1920x1080 and the spec's "size unchanged, 400x300" fails (found on the first private headless Sway run). Sway's placement is covered by `resize-by-id` and `anchor-corners`.
- **`transforms/anchor-corners` needed the matrix to reach `[expect.rect]`**: `anchor` and `pivot` there are now substituted too (`crates/spawn-at-verify/src/schema/matrix.rs`); a spec with no `{key}` in them is unchanged (the catalog golden proves it for the 45).
- **`focus/defocus-active-window`** names `${a.title}` as the destination as designed; on GNOME a destination is looked up by class or application id, so the extension falls back to the most recently used window, which is `a`. The spec says so.
- **`queries/query-layout-json` and `robustness/nonexistent-window-id` launch no window** and carry no `moves-focus` tag; the loader requires it only for specs with a `[[setup]]`.
- **No spec carries the tag `visual`**, so the `visual` suite is unchanged; the four Both specs carry `human-review` (`spawn-at-verify tui --tag human-review`).
- **The `lifecycle` to `focus` alias** was added to both copies of the alias list (`crates/spawn-at-verify/src/schema/suite.rs`, `crates/spawn-at-verify/src/loader/registry.rs`); `focus` is reachable and the known-empty list of `suite_wiring_test` shrank from eight names to four.
- **Not done, as asked**: no flash measurement (`resize-flash` and `move-flash` only ask the human), no spec that depends on `spawn --no-focus`, no GNOME run.
