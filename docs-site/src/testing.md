# Testing and the verify tool

## What is tested, in layers

| Layer | What it covers | Runs where |
| :--- | :--- | :--- |
| Rust unit and integration tests | Placement arithmetic, the CLI, command handlers against mock backends, the config commands, the installer logic | `cargo test`, in CI |
| Golden vectors | `verify/golden/placement.json` and `tests/ready_wait_vectors/`, read by both Rust and JavaScript tests so the two implementations agree | in CI |
| Node tests (`tests/*.js`) | The GNOME extension, loaded with mocked GNOME objects | `node --test tests/*.js`, in CI |
| Scripted-server tests | The Sway driver against a scripted fake of Sway's IPC socket, and the GNOME driver against D-Bus stubs | in CI |
| Headless Sway, live | `scripts/sway-live.sh` and `scripts/verify-sway.sh` against a private Sway started and stopped by `scripts/sway-headless.sh` | by hand, on any Linux machine with `sway` |
| `spawn-at-verify`, live GNOME | Placement, cloak and flash checks against a real GNOME session | by hand, on a GNOME session, attended |

Counts measured on `main` at `8fd1cc5` on 2026-10-16, before the documentation tooling was added:
`spawn-at` 443, `spawn-at-core` 64, `xtask` 10, `spawn-at-verify` 619 passed and 5 ignored (with
`--features tui,fixture`), Node 204. Five tests stay ignored: four need a live display, one needs a
private Sway.

## What `spawn-at-verify` checks

`spawn-at-verify` is a developer-only crate (never in the release package) that drives the built
`spawn-at` binary through its command line only. A test is a TOML file under `verify/<category>/`.

- **Placement.** The expected rectangle comes from an independent arithmetic oracle that follows the
  documented meaning of anchor, pivot and margin, not the implementation. If the oracle and
  `spawn-at` disagree, the failure is reported; the oracle is not changed to fit.
- **Cloak and trace.** From the extension's structured log: the window stays invisible between its
  map and its reveal, the reveal happens once, and no cloak lingers. These checks read the GNOME
  extension's log and do not exist on Sway or X11.
- **Flash.** Computed from the test window's own paint events against the extension's reveal. No
  pixels. See [Findings](findings.md) for what it can and cannot say.
- **Resize outcomes** for awkward apps: `reached`, `adjusted`, `timed-out`, `skipped`.
- **Existing windows** (`schema_version = 2` specs): the runner starts its own test window, acts on
  it by the id it recorded, checks the result, and tears everything down.

A test's `kind` is `auto` (automated assertions), `human` (a person looks) or `both`. A `both` test
stays Pending until a person answers `pass`, `fail`, `skip` or `flaky`. A skipped test is never
counted as a pass. A test whose environment does not match is Skipped with the reason, for example
`unsupported on this backend (sway): spec requires gnome`.

## Attended and unattended

- **Unattended, safe anywhere:** `scripts/verify-sway.sh` starts a private headless Sway, runs the
  specs tagged `headless-safe` (15 entries) against it, and stops it. Nothing of your desktop is
  touched. The runner refuses to run on Sway unless `SWAYSOCK` is inside a marked private directory.
- **Attended, GNOME:** specs that open test windows take focus and move them, so they run by hand
  when the desktop is idle. They carry the tag `moves-focus`, which the `quick` suite excludes.
  `Both` specs are run in the TUI, because `run` leaves a Both window open until the end of the run.
- **Never run automatically:** `scripts/smoke-live.sh` starts real GNOME apps
  (`gnome-calculator`, `gnome-text-editor`). The project owner runs it by hand.

## How to run it

```bash
# the binary embeds its commit; rebuild after every commit or the preflight blocks the run
cargo build -p spawn-at -p spawn-at-verify --features spawn-at-verify/fixture

target/debug/spawn-at-verify list                    # what is loaded
target/debug/spawn-at-verify check                   # validate the specs
target/debug/spawn-at-verify run --tag existing-window --kind auto   # attended, GNOME
cargo run -p spawn-at-verify --features tui,fixture -- tui           # the terminal UI
scripts/verify-sway.sh                               # unattended, private headless Sway
```

`run` and `calibrate` write `report.json`, `report.md` and per-test slices under
`verify-results/<timestamp>-<host>/`. `report` re-renders an existing one. The TUI writes no report;
its only file is the drawer text saved with `w`. Window titles are redacted unless
`--include-titles` is given. A build from uncommitted changes is flagged dirty, and a dirty build
can never qualify an environment.

## The TUI keys

| Key | Action |
| :--- | :--- |
| `j` / `Down`, `k` / `Up` | Move in the list, or scroll the open drawer |
| `Enter` or `d` | Open or close the detail drawer |
| `PageDown` / `PageUp`, `Home`, `G` | Scroll the drawer by page, to the top, to the end |
| `[` / `]` | Scroll the drawer while staying in the list |
| `r` | Run the selected test |
| `a` | Run every test matching the filter |
| `p` `x` `s` `k` | Verdict: pass, fail, skip, flaky (after a `both` test's automatic part) |
| `v` | Open the verdict dialog |
| `f` | Cycle the filter: All, Human only, Auto only, Failed only |
| `y` / `w` | Copy the drawer text / save it under `verify-results/` |
| `Esc` | Close the drawer or the dialog; on a waiting test, close its window without a verdict |
| `q` or `Ctrl+C` | Quit, closing the windows the run opened and restoring the extension's logging state |

On GNOME the TUI returns focus to your terminal after the automatic part of an attended test. A
raised terminal can cover the small test window; move or shrink the terminal first.

## Safety rules for contributors

These come from the project rules and the harness design.

- **Never run `spawn-at install` or `spawn-at uninstall` for real while testing.** They act on the
  live GNOME session. Test against temporary directories and injected facts.
- **Test only with the fixture and your own scratch clients.** The harness never closes a window it
  did not open: it records the existing windows first, closes only its own by id, and refuses to
  track more than 20 at once.
- **No kills by name.** Never `pkill`, `killall` or any pattern match. Signal only a process id you
  recorded from a child you started.
- **`LD_PRELOAD` only on the test client's own process**, never in the shell, the user session
  environment or a file.
- **Stop and ask** if gnome-shell or the extension crashes or restarts, you see a signal 5, 6 or 11,
  an "uncatchable exception", a Mutter window or focus assertion, or any journal line mentioning
  spawn-at.

## Sources

- `verify/README.md` (sections 1 to 10)
- `CLAUDE.md` ("Hard safety rules", "Tests", "Build and test commands")
- `docs/TEST_COVERAGE_AUDIT.md`, sections 1, 4 and 5
- `docs/TEST_PLAN.md`, section 7 and the suite counts
- `docs/KNOWLEDGE.md` ("Testing and fixtures")
- `docs/CODE_LAYOUT.md` ("Facts that constrain moving files")
- the counts come from `cargo test --locked -p spawn-at -p spawn-at-core -p xtask`, `cargo test --locked -p spawn-at-verify --features tui,fixture` and `node --test tests/*.js`, run on `main` at `8fd1cc5`
