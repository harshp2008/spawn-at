---
title: "Running the verify TUI"
---

The TUI is the interactive front end of `spawn-at-verify`, the repository's test harness. It lists
the test specs, runs one or many, asks you to judge the `both` tests by eye, and shows the result.
It is for attended runs on a GNOME desktop. There is no web interface. For the automated runs and
the counts, see [Testing and the verify tool](/spawn-at/testing/).

## What it needs

- A terminal at least 60 columns wide and 15 rows high. A smaller one shows "Terminal too small".
- The `tui` and `fixture` features. `tui` builds the interface. `fixture` builds the small test
  window (`target/debug/fixture`) that most specs launch. Without it the header shows "FIXTURE
  MISSING" and the command to build it.
- A built `spawn-at` binary, `target/debug/spawn-at` by default. It must match the current commit, or
  the preflight check blocks every test as a stale binary.
- For the cloak and flash checks: a GNOME Wayland session with the spawn-at extension, and
  `journalctl --user`. These checks read the extension's log. On Sway or X11 they do not exist, and
  specs that need GNOME show as Skipped with the reason.

Run it only on an idle desktop. The specs **open windows that take focus** and move them. The TUI
closes only the windows it opened, when you quit. The one other window it acts on is the one that
had focus when it started: it sends that window a `focus` command to give focus back.

## Start it

```bash
cargo build -p spawn-at -p spawn-at-verify --features spawn-at-verify/tui,spawn-at-verify/fixture
target/debug/spawn-at-verify tui
```

`tui --help` lists the options. All are optional.

| Option | Meaning |
| :-- | :-- |
| `--bin <BIN>` | The `spawn-at` binary under test (default `target/debug/spawn-at`) |
| `--dir <DIR>` | The folder of specs (default: the `verify/` folder it finds) |
| `--suite <SUITE>` | Start with one suite, for example `quick`, `visual` or `release-gate` |
| `--category <CATEGORY>` | Start with the specs under one category path, for example `placement/` |
| `--tag <TAGS>` | Start with specs that have a tag. Repeat the option for more than one |

There is no `--kind` option. To judge the `both` specs of the existing-window group, start with
`--tag human-review`.

## The screen

The screen has a header, the test list, a description pane, a result box and a footer of keys. A
terminal narrower than 75 columns stacks the panes; a wider one puts the list on the left.

| Part | What it shows |
| :-- | :-- |
| Header | The title, tags for the environment, and the FIXTURE MISSING warning |
| Tests | The specs grouped by category, each with a status mark. Order: PREFLIGHT, FOCUS, QUERIES, ROBUSTNESS, PLACEMENT, CLOAK & DELAYED PAINT, TRANSFORMS, GENERAL |
| Description | What the selected spec is and what it requires |
| Result | The outcome of the selected spec and its diagnostics |
| Footer | The keys that work now, or the verdict question while a test waits for you |

The status marks are `✔` pass, `✖` fail, `-` skipped, `!` flaky and `·` pending. The mode of a spec is
`auto` (automatic check), `you` (your visual check) or `both`.

The **drawer** is the full-screen detail view of the selected spec. Its sections, in order, are the
title and scenario, RESULT, WHAT TO LOOK FOR, GEOMETRY (expected against actual rectangle), SPEC,
OUTPUT and TIMELINE. TIMELINE is last because it is the longest.

## Keys

In the list:

| Key | Action |
| :-- | :-- |
| `j` / `Down`, `k` / `Up` | Move the selection, or scroll the description pane when it is active |
| `Tab` | Switch between the list and the description pane |
| `g` / `Home`, `G` | First and last spec, or top and bottom of the description pane |
| `]` / `PageDown`, `[` / `PageUp` | Scroll the description pane by 6 rows |
| `Space` | Fold or unfold the selected category |
| `f` | Cycle the filter: All, Human (Review Required), Auto Only, Failed Only |
| `Ctrl+f` | Cycle the category filter |
| `r` | Run the selected spec |
| `c` or `Ctrl+r` | Run the selected category |
| `a` | Run every spec that matches the filter |
| `v` | Open the verdict dialog |
| `Enter` or `d` | Open the drawer |
| `?` | Open the help overlay |
| `q` | Quit |
| `Ctrl+c` | Emergency quit: close the test windows and leave at once |

In the drawer:

| Key | Action |
| :-- | :-- |
| `Esc`, `Enter` or `d` | Close the drawer |
| `j` / `Down`, `k` / `Up` | Scroll one row |
| `PageDown` / `PageUp` | Scroll 8 rows |
| `g` / `Home`, `G` | Top and bottom |
| `t` | Switch between the raw and the compact view |
| `y` | Copy the drawer text to the clipboard |
| `w` | Save the drawer text to a file |
| `?`, `q` | Help, quit |

While a `both` test waits for you, `p` records pass, `x` fail, `s` skip and `k` flaky. `Esc` closes the
test window without a verdict. `r` runs the test again. `d` opens the drawer.

## Judge a `both` spec

1. Select the spec and press `r`. The test window opens and the automatic part runs. The row shows
   "Running" first, because the key only records your request and the work starts after the next
   redraw.
2. The test window stays open. The TUI gives focus back to your terminal so that your keys reach the
   TUI, not the test window. A raised terminal can cover the small window: move or shrink the
   terminal before you press `r`.
3. Read the question under WHAT TO LOOK FOR, watch the window, and press `p`, `x`, `s` or `k`.
4. The row and the Result box change at once. The window closes after that frame.

A test that is not `both` closes its window when its automatic part ends.

## Read a result

- Geometry. GEOMETRY compares the expected rectangle from the oracle with the actual one. The
  oracle is an independent arithmetic model of anchor, pivot and margin.
- Trace. The Trace row says `clean, no violations` when the extension's log shows one reveal, the
  cloak before it, and no size or position change after it. When the log holds no event for the
  window, it says `not analysed (no trace captured)`, with a Reason row, and the test ends as
  `Pass (trace unchecked)`. That is not a clean pass.
- Flash. A placement that is right but showed the window before the app had painted its final
  size ends as `PASS (flash)`. No pixels are captured, so a clean result means no late paint or move
  was found. See [Findings](/spawn-at/findings/).
- Cloak and Budget. The cloak time is the extension's reveal minus its cloak. The budget row
  compares it with a provisional 100 ms and is advice, not a pass or fail rule.
- Timeline. For the specs of existing windows, the extension's own log lines for the windows the
  run opened, with the milliseconds since the first line of each action. Lines of other windows are
  dropped.

## Where files go

**The TUI writes no report.** Results stay on screen. Its only file is the drawer text saved with
`w`, as `verify-results/<test id with / replaced by ->.log.txt`. `verify-results/` is ignored by git.
A `both` test that still waits for a verdict is recorded nowhere unless you run the same tests with
`spawn-at-verify run`, which writes `report.json`, `report.md` and per-test slices under
`verify-results/<timestamp>-<host>/`.

At start the TUI reads the extension's logging state from
`~/.local/state/spawn-at/session.log` (or `$XDG_STATE_HOME/spawn-at/session.log`) and, if absent,
from `SPAWN_AT_DEBUG`. On exit it restores that state, so it does not switch off logging you had
turned on.

## Leaving safely

`q`, `Ctrl+c`, `SIGINT`, `SIGTERM` and `SIGHUP` all run the same teardown: the terminal leaves raw
mode and the alternate screen, the windows the TUI opened are closed by id, and the extension's
logging goes back to its starting state. A panic restores the terminal before the message prints.
The TUI does not capture the mouse, so a wheel notch arrives as `Up` or `Down`.

## Not verified

- The TUI was not run to write this page. The keys and screens come from the source and
  `tui --help`.
- That the focus really returns to the terminal on a real GNOME session is not verified. It is
  covered on a private headless Sway by `scripts/focus-chain-sway.sh`.
- The behavior of the specs of existing windows on GNOME has not been run.

## Sources

- `crates/spawn-at-verify/src/tui/app/input.rs` (all keys)
- `crates/spawn-at-verify/src/tui/views/modals/help.rs` (the help overlay: keys, status marks, modes)
- `crates/spawn-at-verify/src/tui/views/mod.rs` (layout, the 60x15 minimum, the 75-column breakpoint)
- `crates/spawn-at-verify/src/tui/views/drawer/mod.rs` (the order of the drawer sections)
- `crates/spawn-at-verify/src/tui/mod.rs`, `tui/logging.rs`, `tui/app/clipboard.rs` (start-up, logging
  state, teardown, clipboard)
- `crates/spawn-at-verify/src/tui/app/state.rs` (filter modes)
- `verify/README.md` (section 9, TUI; section 10, reporting)
- the output of `spawn-at-verify --help` and `spawn-at-verify tui --help`
