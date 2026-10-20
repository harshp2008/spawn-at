# Project history

Status: reference. This page records what changed in the documents and in how the project is tested, and when. It does not say how anything works now: the reference pages do that.

## Where each kind of note goes

| Kind of note | Where it goes |
| :-- | :-- |
| How it works, why it is this way, what to watch for | A plain present-tense statement on the page it belongs to |
| A change a user can see | [CHANGELOG.md](../CHANGELOG.md) |
| A dated measurement or experiment | [TEST_LOG.md](TEST_LOG.md) |
| What changed, when, or why something was added: "added for X", "was X, now Y", a branch name, a re-check on a date | This page |

Each entry is one line: the date, what changed, and the page it was taken from. "Date not recorded" means the source gave none.

## Documents

- 2026-10-09: [Legacy GNOME](legacy-gnome.md) was written. On 2026-10-21 its two trees were compared again with a read-only `diff`. (From the status lines of that page.)
- 2026-10-16: the documentation site moved from mdBook 0.5.4 to Starlight on Astro. (From the [Knowledge base](KNOWLEDGE.md), "Documentation site".)
- 2026-10-21: the Knowledge base was rewritten and shortened. Stale entries were corrected or removed, every file path, constant, function and test name in it was checked, and the page was reorganised. (From its status lines.)
- Date not recorded: [Code layout](CODE_LAYOUT.md) was first a reorganization proposal (target layout, move map, stages 1 to 5). Only stage 1, the modularization of `spawn-at-verify`, was carried out. The rest was never built and its paths no longer matched the tree, so the proposal was removed. It is in git history (`git log -- docs/CODE_LAYOUT.md`).
- Date not recorded: [Design: a Sway backend](DESIGN_SWAY.md), section 7. Before the Sway backend, `SPAWN_AT_BACKEND` was compared to `x11` only and any other value was ignored. The only change was that `sway` also selects a backend; every other input returned what it returned before.
- Date not recorded: [Design: a Sway backend](DESIGN_SWAY.md), section 10. The design had proposed the content size for question 3 and the focused workspace's output for question 4. The owner's answers (the whole container; the first output) replaced both.
- Date not recorded: the design document's section 3 on Sway said `--anchor cursor` fails. It was corrected to match section 12 and the code. (From the Sources of the Sway environment page.)

## Test plan and the verify tool

- Date not recorded: an earlier revision of [Test plan](TEST_PLAN.md) section 5 published min, median and max cloak durations and a two-mode explanation. They came from a trace analyser that could measure a leftover window of the previous run instead of the window named in the fixture's stdout. Commit `7ffa99c` fixed the analyser; the old numbers and the explanations built on them were withdrawn.
- Date not recorded: section 2.1 of the test plan was replaced by a summary. The earlier text is in git history. The corrections it needed are in section 6.0.
- Date not recorded: an earlier revision of section 6.2 said the minimum-aware logic of the extension "never runs". That was wrong; the logic runs and finds the minimum empirically under the cloak.
- 2026-10-13: `CHANGELOG.md` was added (commit `c1a34fa`), after the contract search of section 6.1. On 2026-10-19 its matches for min size, clamp and raised (lines 288 and 350-351) were checked: they are about the ready-wait, not about the min-size contract.
- Date not recorded: `spawn-at-verify run --repeat` has existed since commit `0f6bb36`. Earlier versions of the report kept only the first repetition; that was a limitation of the report, not a design choice. It now stores every repetition under `repeat_runs`.

## Knowledge base entries

- 2026-10-15: the runner chose the GNOME journal source and `SetLogging` from `XDG_CURRENT_DESKTOP` alone. With `SPAWN_AT_BACKEND=sway` or `x11` set it no longer does (commit `77ab5d2`, with the runner setup work).
- 2026-10-16: the focus return after an attended test did not exist for version 1 specs, and the owner's GNOME run showed the keys did not reach the TUI. It now runs for every judged test (`TuiApp::return_focus_to_terminal`).
- 2026-10-16: an entry said `escalate.rs` had never been compiled for macOS and Windows. It is compiled by the CI job `check-other-os`; only running it is still missing.
- 2026-10-20: `records.mjs` stopped moving the short fields of a card with CSS `order`, which made keyboard focus follow the table order instead of the order on screen. It writes them first in the HTML.
- 2026-10-21: the `unsupported` refusal for non-GNOME Wayland and unknown sessions first shipped in `v0.2.0-beta.5`. `git show v0.2.0-beta.4:src/platform/linux/registry.rs` has no `unsupported` and beta.5 has it.

## Design: tests of an existing window

These rows were the "Updates after the first implementation" table of [Design: tests of an existing window](DESIGN_RUNNER_SETUP.md), section 11.

- 2026-10-15: implemented on the branch `feat/runner-existing-window-setup`, as approved.
- 2026-10-16, branch `feat/trace-existing-window`: section 5 was implemented (5.5). `runner/flash.rs` gained about 70 lines (`evaluate_action`, `env_pairs`) and `trace/flash.rs` one field, so the line numbers of `runner/flash.rs` and `trace/flash.rs` in sections 5.1 to 5.4 are those of the code as designed. At that commit: `FlashCapture::apply` `runner/flash.rs:77`, `read_events` `:114`, `wants_flash_capture` `:141`, `apply_flash_policy` `:153`, `run_flash_check` `:348`, `detect_flash` `trace/flash.rs:472`, `label_visibility` `:619`. Open question 9.11 (the journal check) was answered by the owner's sample.
- 2026-10-16, branch `fix/tui-order-and-focus-return`: the focus return did not work on the owner's GNOME run. `TuiApp::return_focus_to_terminal` (`runner/focus_return.rs`) now returns focus after the automatic part of every judged test and again after the verdict, with a settle, up to three sends and a polled check after each. A `FocusReport` is shown in the Result box, and `RunOptions::caller_returns_focus` stops the executor from doing its own unrecorded return. It is covered on a private headless Sway (`scripts/focus-chain-sway.sh`); the GNOME-only reasons (focus-stealing prevention, the terminal missing from the extension's list) are shown by the report but cannot be tested here.
- 2026-10-16, same branch: why the focus return did not work. It existed only for version 2 specs, so version 1 `Both` specs (all of placement and cloak) never got focus back after the automatic part. The TUI's own refocus used an id read once at start through a 500 ms query whose failure was swallowed, then a fire-and-forget `focus --no-wait` that was never checked or retried and was sent right after an asynchronous close.
- 2026-10-16, branch `fix/tui-verdict-feedback`: the TUI stopped running a test or the verdict teardown inside the key handler. `TuiApp::handle_key` queues a `Pending` step that the event loop runs after the next redraw (`tui/app/queue.rs`), so the "synchronously on the UI thread" statements of sections 1 to 6 describe the code as it was when the design was written. The focus memory (`remember_focus`) is also used after the automatic part of a Both or Human test (`SetupSession::restore_focus`).
