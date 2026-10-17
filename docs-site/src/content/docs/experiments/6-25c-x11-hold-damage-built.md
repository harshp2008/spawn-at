---
title: "6.25c Hold the cloak: built, not run"
---

## Title

`x11-hold-damage`, built and tested offline, not installed and not run in a Shell (X11 investigation,
stage 1, phase 3)

## ID

TEST_LOG 6.25c (the third part of section 6.25 of `docs/TEST_LOG.md`). The live run is
[6.26](/spawn-at/experiments/6-26-x11-hold-damage-live/).

## Date

2026-10-14. The commit that added it, `196ac20`, is dated 2026-10-14 in `git log`.

## Build

Tag: none recorded. Tree: branch `refactor/v0.2.0-beta.1`, HEAD `b09e27f` at the start of the
section. The extension change is the commit `196ac20`, "experimental x11-hold-damage wait for
XWayland/X11 (off by default, not installed)". It was never installed in a Shell for this section.
The output of `spawn-at --version` is not recorded.

## Question

The log calls this the "hold-the-cloak experiment": for an XWayland or X11 window with a size
request, keep the window cloaked after `set_size`, ignore the first `damaged` event and end the wait
on the second. The open question the log says the live run answers: whether the paint's `damaged`
arrives while the window stays cloaked.

## Setup and environment

Node tests (extension code with mocked GNOME objects) and Rust tests only. The code was a new
module `x11_hold.js` and a hidden strategy value. The log states: not installed, not run in a Shell,
no default changed.

## Commands run

Exact command lines not recorded. By the log: the test suites were run, and the log gives
`cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` as clean.

How the experiment was selected (from the log): the hidden flag `--wait-strategy x11-hold-damage`,
with `--wait-cap-ms` for its cap. The config file and `spawn-at config set` refused the value
(`wait.strategy must be auto, legacy or ready-wait`); `auto` stayed the default.

## Raw result location

`scratch/p26/` (gitignored). Not in the repository.

## Result

The rule as built: after `set_size` the window stays cloaked; the first `damaged` is ignored; the
second ends the wait as `reached` with `reason=x11-second-damaged`; if none comes, the cap
(`cap_ms`, default 250) ends it as `timed-out` with `reason=x11-cap`, or `x11-hard` when the cap is
1500. A window that goes away ends it as `skipped` / `x11-unmanaged`; a missing `damaged` signal ends
it at once as `skipped` / `x11-no-damaged-signal`.

Unaffected, by test: a native Wayland window given the value got the same log lines as with no
override apart from the `READY_WAIT_OVERRIDE` note; an X11 spawn without the value, or with it but
no size request, took the original rule.

Tests added: 8 in `tests/x11_hold_test.js`, 4 in `tests/ready_wait_wiring_test.js`, 1 in
`wait_override.rs` (plus the round-trip list), 1 assertion in `settings/tests.rs`, 1 in
`mechanics.rs`. Counts after: `spawn-at` 206 (was 205), `spawn-at-core` 65, `xtask` 10,
`spawn-at-verify` (`tui,fixture`) 402 passed and 4 ignored, node 188 (was 176).

## Verdict

Built and tested offline only. The log lists the open question the live run was to answer: whether the
paint's `damaged` arrives while the window stays cloaked. The answer is in [6.26](/spawn-at/experiments/6-26-x11-hold-damage-live/).

## Not verified

Everything about a live Shell: the log says the behaviour was "not byte-checked against a live Shell".
The files to install for the live stage (`x11_hold.js` and edits to `extension.js`, `commit.js` and
`validator.js`) were listed but not installed in this section. The code is not in the tree; it was
removed by a later revert (`80ced02`).

## Sources

- `docs/TEST_LOG.md` section 6.25, "Phase 3a", "Phase 3c", "Phase 3d" (lines 1385-1397)
- `git log` for `196ac20` and `80ced02`
- [X11 and XWayland](/spawn-at/environments/x11-xwayland/)
