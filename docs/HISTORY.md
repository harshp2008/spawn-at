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

- 2026-10-09: [Legacy GNOME](legacy-gnome.md) was written. On 2026-10-21 its two trees were compared again with a read-only `diff`.
- 2026-10-16: the documentation site moved from mdBook 0.5.4 to Starlight on Astro. (From the [Knowledge base](KNOWLEDGE.md), "Documentation site".)
- 2026-10-21: the Knowledge base was rewritten and shortened. Stale entries were corrected or removed, every file path, constant, function and test name in it was checked, and the page was reorganised.
- Date not recorded: [Code layout](CODE_LAYOUT.md) was first a reorganization proposal (target layout, move map, stages 1 to 5). Only stage 1, the modularization of `spawn-at-verify`, was carried out. The rest was never built and its paths no longer matched the tree.
- Date not recorded: that proposal was removed from the page. It is in git history (`git log -- docs/CODE_LAYOUT.md`).

## Design: a Sway backend

- Date not recorded, section 7: before the Sway backend, `SPAWN_AT_BACKEND` was compared to `x11` only and any other value was ignored. The only change was that `sway` also selects a backend; every other input returned what it returned before.
- Date not recorded, section 10: the design had proposed the content size for question 3 and the focused workspace's output for question 4. The owner's answers (the whole container; the first output) replaced both.
- Date not recorded: the design's section 3 said `--anchor cursor` fails. It was corrected to match section 12 and the code. (From the Sources of the Sway environment page.)

## Test plan

- Date not recorded: an earlier revision of section 5 published min, median and max cloak durations and a two-mode explanation. They came from a trace analyser that could measure a leftover window of the previous run instead of the window named in the fixture's stdout.
- Date not recorded: before the fix, neighbouring runs repeatedly reported identical values (57.64 ms twice, 8.20 ms three times). Commit `7ffa99c` fixed the analyser. The old numbers and the explanations built on them were withdrawn.
- Date not recorded: single `run`s of `delay-0`, `delay-50`, `delay-150` and `placement/bottom-right` gave about 30 - 44 ms before the two regimes were understood.
- Date not recorded: section 2.1 was replaced by a summary. The earlier text is in git history, and the corrections it needed are in section 6.0.
- Date not recorded: an earlier revision of section 6.2 said the minimum-aware logic of the extension "never runs". That was wrong. The logic runs and finds the minimum empirically under the cloak.
- 2026-10-13: `CHANGELOG.md` was added (commit `c1a34fa`), after the contract search of section 6.1.
- 2026-10-19: the matches of `CHANGELOG.md` for min size, clamp and raised (lines 288 and 350-351) were checked. They are about the ready-wait, not about the min-size contract.
- Date not recorded: `spawn-at-verify run --repeat` has existed since commit `0f6bb36`. Earlier versions of the report kept only the first repetition, which was a limitation of the report. It now stores every repetition under `repeat_runs`.

## Test counts and catalog

- 2026-10-20: the test counts were measured on the branch `docs-rewrite-1`: `spawn-at` 460, `spawn-at-core` 64, `spawn-at-verify` 625 run and 5 ignored with `tui,fixture`, `xtask` 96.
- 2026-10-20: the same run counted 240 Node tests in the 12 `*_test.js` files, and 243 with the 3 install-script tests, which were left to CI. The Testing page had quoted counts taken on 2026-10-19 on the branch `docs-records-cards`.
- 2026-10-20: the 47 Covered and 12 Not written rows of the catalog in section 2 were counted with a file-exists check on all 59 paths.
- 2026-10-20: `spawn-at-verify list` printed 67 tests (55 auto, 12 both) with the debug binary built on 2026-10-18, newer than every file under `crates/spawn-at-verify/src` and `verify/`.
- Date not recorded: the first batch of window-command tests through the built binary (`tests/sway_backend_cli_test.rs`) had 10 tests. The file has 19 now.

## Sway live checks

- 2026-10-15: the first run of the 22 specs for existing windows on a private headless Sway failed one spec, `transforms/reposition-by-id`. A fresh tiled window is as large as the work area, so "size unchanged, 400x300" did not hold. The spec became GNOME-only.
- Date not recorded: while the backend was built, `--size 600 400` with a real border and title bar (`for_window ... border normal 3`) was tried by hand on the private Sway. It gave a container of exactly 600x400 (content 594x372). This is also in [DESIGN_SWAY.md](DESIGN_SWAY.md), section 4.

## Knowledge base entries

- 2026-10-15: the runner chose the GNOME journal source and `SetLogging` from `XDG_CURRENT_DESKTOP` alone. With `SPAWN_AT_BACKEND=sway` or `x11` set it no longer does (commit `77ab5d2`).
- 2026-10-16: the focus return after an attended test did not exist for version 1 specs, and the owner's GNOME run showed that the keys did not reach the TUI. It now runs for every judged test.
- 2026-10-16: an entry said `escalate.rs` had never been compiled for macOS and Windows. The CI job `check-other-os` compiles it. Only running it is still missing.
- 2026-10-20: `records.mjs` stopped moving the short fields of a card with CSS `order`, which made keyboard focus follow the table order and not the order on screen. It writes them first in the HTML.
- 2026-10-21: the `unsupported` refusal for non-GNOME Wayland and unknown sessions first shipped in `v0.2.0-beta.5`. `git show v0.2.0-beta.4:src/platform/linux/registry.rs` has no `unsupported`, and beta.5 has it.

## Design: tests of an existing window

These entries were the "Updates after the first implementation" table of [Design: tests of an existing window](DESIGN_RUNNER_SETUP.md), section 11.

- 2026-10-15: implemented on the branch `feat/runner-existing-window-setup`, as approved.
- 2026-10-16, branch `feat/trace-existing-window`: section 5 was implemented (5.5). `runner/flash.rs` gained about 70 lines (`evaluate_action`, `env_pairs`) and `trace/flash.rs` one field, so the line numbers of those files in sections 5.1 to 5.4 are those of the code as designed.
- 2026-10-16, same branch: the line numbers after the change were `FlashCapture::apply` `runner/flash.rs:77`, `read_events` `:114`, `wants_flash_capture` `:141`, `apply_flash_policy` `:153`, `run_flash_check` `:348`, `detect_flash` `trace/flash.rs:472` and `label_visibility` `:619`.
- 2026-10-16, same branch: open question 9.11 (the journal check) was answered by the owner's sample.
- 2026-10-16, branch `fix/tui-order-and-focus-return`: the focus return did not work on the owner's GNOME run. `TuiApp::return_focus_to_terminal` (`runner/focus_return.rs`) now returns focus after the automatic part of every judged test and again after the verdict.
- 2026-10-16, same branch: a settle, up to three sends and a polled check after each were added. A `FocusReport` is shown in the Result box, and `RunOptions::caller_returns_focus` stops the executor from doing its own unrecorded return.
- 2026-10-16, same branch: the return is covered on a private headless Sway (`scripts/focus-chain-sway.sh`). The GNOME-only reasons (focus-stealing prevention, the terminal missing from the extension's list) are shown by the report but cannot be tested here.
- 2026-10-16, same branch, why it did not work: the return existed only for version 2 specs, so version 1 `Both` specs (all of placement and cloak) never got focus back after the automatic part.
- 2026-10-16, same branch: the TUI's own refocus used an id read once at start through a 500 ms query whose failure was swallowed. It then sent a fire-and-forget `focus --no-wait` that was never checked or retried, right after an asynchronous close.
- 2026-10-16, branch `fix/tui-verdict-feedback`: the TUI stopped running a test or the verdict teardown inside the key handler. `TuiApp::handle_key` queues a `Pending` step that the event loop runs after the next redraw (`tui/app/queue.rs`).
- 2026-10-16, same branch: so the "synchronously on the UI thread" statements of sections 1 to 6 describe the code as it was when the design was written. The focus memory (`remember_focus`) is also used after the automatic part of a Both or Human test (`SetupSession::restore_focus`).
