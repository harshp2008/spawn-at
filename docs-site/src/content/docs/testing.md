---
title: "Testing and the verify tool"
---

How spawn-at is tested: which command runs which test, what needs a desktop, what is safe on a
development machine, and the current test counts. The test harness has two more pages:
[Running the verify TUI](/spawn-at/contributing/verify-tui/) and
[Writing a test spec](/spawn-at/contributing/test-specs/).

## Which command for what

| To do | Command | Needs |
| :-- | :-- | :-- |
| Check formatting | `cargo fmt --check` | nothing |
| Lint every crate, harness included | `cargo clippy --workspace --all-targets --locked --features spawn-at-verify/tui,spawn-at-verify/fixture -- -D warnings` | nothing |
| Test the CLI, the core logic and xtask | `cargo test --locked -p spawn-at -p spawn-at-core -p xtask` | nothing |
| Test the verify harness | `cargo test --locked -p spawn-at-verify --features tui,fixture` | nothing |
| Test the GNOME extension code | `node --test $(ls tests/*_test.js \| grep -v install_unsupported_test.js)` | Node |
| Run one Rust test | `cargo test --locked -p spawn-at-core <part of the test name>` | nothing |
| Check that the specs load | `target/debug/spawn-at-verify check` | the built harness |
| Run the headless Sway suite | `scripts/verify-sway.sh` | `sway` installed; it starts its own private Sway |
| Run the specs on GNOME | `target/debug/spawn-at-verify run --tag existing-window --kind auto` | an idle GNOME session |
| Build the documentation site | `cargo xtask docs-site` | Node and npm |

Plain `cargo test` at the repository root covers only the `spawn-at` package, because it is the only
default member. Name the other crates with `-p`, and give `spawn-at-verify` its features.

The harness rows need the binaries first: `cargo build -p spawn-at -p spawn-at-verify --features
spawn-at-verify/fixture`. The binary embeds its commit, so rebuild after every commit or the
preflight check blocks the run as a stale binary.

The first four `cargo` rows are what CI runs. The `node --test` row lists the 12 `*_test.js` files by
name. CI runs `node --test tests/*.js` instead, which also runs the 3 tests of
`tests/install_unsupported_test.js`.

Those 3 tests **run only in CI**. They execute `install.sh` against a stand-in `curl`, a stand-in
`spawn-at` and a temporary `HOME`, and the project does not run the installer script on a
development machine. The glob also counts the helper `tests/gi_loader.js` as one more test.

## What is tested, in layers

| Layer | What it covers | Runs |
| :-- | :-- | :-- |
| Rust unit and integration tests | Placement arithmetic, the CLI, command handlers against mock backends, the config commands, the installer logic | `cargo test`, in CI |
| Golden vectors | Stored inputs with expected outputs (`verify/golden/placement.json`, `tests/ready_wait_vectors/`), read by both the Rust and the JavaScript tests so the two implementations agree | in CI |
| Node tests | The GNOME extension, loaded with mocked GNOME objects; also the docs site's table-to-cards integration | `node --test`, in CI |
| Scripted-server tests | The Sway backend against a scripted fake of Sway's IPC socket, and the GNOME backend against D-Bus stubs | in CI |
| Headless Sway, live | `scripts/sway-live.sh` and `scripts/verify-sway.sh` against a private Sway that `scripts/sway-headless.sh` starts and stops | by hand, on any Linux machine with `sway` |
| `spawn-at-verify`, live GNOME | Placement, cloak and flash checks against a real GNOME session | by hand, attended |

Five tests stay ignored. Four in `crates/spawn-at-verify/tests/fixture_test.rs` need a live display.
One in `crates/spawn-at-verify/tests/tui/focus_chain_sway.rs` needs a private Sway, which
`scripts/focus-chain-sway.sh` starts.

## Test counts

This is the one table of counts. Other pages point here.

| Target | Run | Ignored | Command that prints the count |
| :-- | --: | --: | :-- |
| `spawn-at` | 475 | 0 | `cargo test --locked -p spawn-at -- --list` |
| `spawn-at-core` | 64 | 0 | `cargo test --locked -p spawn-at-core -- --list` |
| `xtask` | 96 | 0 | `cargo test --locked -p xtask -- --list` |
| `spawn-at-verify`, no features | 434 | 0 | `cargo test --locked -p spawn-at-verify -- --list` |
| `spawn-at-verify`, `tui` | 599 | 1 | `cargo test --locked -p spawn-at-verify --features tui -- --list` |
| `spawn-at-verify`, `tui,fixture` (what CI runs) | 625 | 5 | `cargo test --locked -p spawn-at-verify --features tui,fixture -- --list` |
| Node, 12 `*_test.js` files | 240 | 0 | `node --test tests/<file>`, once per file |
| Node, the install-script test | 3 | 0 | CI only |

Rust total with `tui,fixture`: 1260 run and 5 ignored (475 + 64 + 96 + 625). With the 240 Node tests
that run locally: 1500. With the 3 install-script tests, which only CI runs: 1503.

Last checked at commit `1a5a618`.

To get a Rust count, pipe the command through `grep -c ': test$'`. That is the number of tests
listed, ignored ones included. Add `--ignored` after `--list` to count only the ignored tests. Run is
listed minus ignored. `--list` runs nothing. **Do not quote a bare `cargo test --workspace` number**
for `spawn-at-verify`: it builds the crate without its features and gives a different set of tests.

The counts per binary and per module are in [Test plan](/spawn-at/project/test-plan/#12-counts-per-binary).

## What `spawn-at-verify` checks

`spawn-at-verify` is a developer-only crate, never in the release package. It drives the built
`spawn-at` binary through its command line only. A test is a TOML file under `verify/<category>/`.

- Placement. The expected rectangle comes from an independent arithmetic oracle that follows the
  documented meaning of anchor, pivot and margin, not the implementation. If the oracle and
  `spawn-at` disagree, the failure is reported; the oracle is not changed to fit.
- Cloak and trace. From the extension's structured log: the window stays invisible between its
  map and its reveal, the reveal happens once, and no cloak lingers. These checks read the GNOME
  extension's log and do not exist on Sway or X11. The cloak is explained in
  [Concepts](/spawn-at/concepts/#cloak-and-waiting-for-the-app).
- Flash. Computed from the test window's own paint events against the extension's reveal. No
  pixels are captured. See [Findings](/spawn-at/findings/) for what it can and cannot say.
- Resize outcomes for awkward apps: `reached`, `adjusted`, `timed-out`, `skipped`.
- Existing windows (`schema_version = 2` specs): the runner starts its own test window, acts on
  it by the id it recorded, checks the result, and tears everything down.

A test's `kind` is `auto` (automated assertions), `human` (a person looks) or `both`. A `both` test
stays Pending until a person answers `pass`, `fail`, `skip` or `flaky`. **A skipped test is never
counted as a pass.** A test whose environment does not match is Skipped with the reason, for example
`unsupported on this backend (sway): spec requires gnome`.

## Attended and unattended

| Run | Who and where | What it touches |
| :-- | :-- | :-- |
| `scripts/verify-sway.sh` | Unattended, any Linux machine with `sway` | A private headless Sway only. It runs the 15 entries tagged `headless-safe`. The runner refuses to run on Sway unless `SWAYSOCK` is inside a marked private directory |
| `spawn-at-verify run` on GNOME | Attended, an idle desktop | Specs tagged `moves-focus` open test windows that take focus and move; the `quick` suite excludes them |
| `Both` specs | Attended, in the TUI | `run` leaves a `Both` window open until the end of the run, so these go through the TUI |
| `scripts/smoke-live.sh` | The project owner, by hand | Starts real GNOME apps (`gnome-calculator`, `gnome-text-editor`). Contributors do not run it |

## Reports

`run` and `calibrate` write `report.json`, `report.md` and per-test slices under
`verify-results/<timestamp>-<host>/`. `report` re-renders an existing one. The TUI writes no report;
its only file is the drawer text saved with `w`. Window titles are redacted unless `--include-titles`
is given.

A build from uncommitted changes is flagged dirty, and **a dirty build can never qualify an
environment**. The qualification block of the report counts the required tests as passed, failed,
pending and skipped, apart from each other. Pending means still waiting for a human verdict.

`supported` is true only when every required test passed. When nothing failed and no required test
was skipped, but some are pending, `report.md` shows a PENDING badge. A failure or a required skip
still shows NOT QUALIFIED.

## Safety rules for contributors

- **Never run `spawn-at install` or `spawn-at uninstall` for real while testing.** They act on the
  live GNOME session. Test against temporary directories and injected facts.
- Test only with the fixture and your own scratch clients. The harness never closes a window it did
  not open: it records the existing windows first, closes only its own by id, and refuses to track
  more than 20 at once.
- Never kill by name: no `pkill`, no `killall`, no pattern match. Signal only a process id you
  recorded from a child you started.
- Set `LD_PRELOAD` only on the test client's own process, never in the shell, the user session
  environment or a file.
- Stop and ask the maintainer if gnome-shell or the extension crashes or restarts, you see a signal 5, 6 or 11, an
  "uncatchable exception", a Mutter window or focus assertion, or any journal line that mentions
  spawn-at.

## Limits of the test suite

- The catalog has 22 entries for existing windows. 15 passed on a private headless Sway and 7 were
  skipped there because they need GNOME. The original 45 entries only run `spawn`.
- Four tests named "supported on X11" pass on a connection error, so they do not test X11.
- The harness uses GNOME-only tools (`journalctl`, `gdbus`) for its logs and display probes.

## Not verified

- None of the 22 specs for existing windows has run on a GNOME session.
- The `Both` specs need a person, so no automated run covers them.
- The reports' PENDING badge is checked with fake reports in unit tests, not on a live session.
- The count of 3 tests in the install-script file is carried over from earlier documentation; it is
  not regenerated by the commands above.

## Sources

- `verify/README.md` (sections 4, 5, 6 and 10)
- `.github/workflows/ci.yml` (the commands CI runs)
- `docs/TEST_PLAN.md`, sections 1 and 7
- `docs/TEST_COVERAGE_AUDIT.md`, sections 1, 4 and 5
- `docs/KNOWLEDGE.md` ("Testing and fixtures")
- `docs/CODE_LAYOUT.md` ("Facts that constrain moving files")
- `scripts/verify-sway.sh` (header) and `scripts/focus-chain-sway.sh`
- the output of `cargo test --locked ... -- --list` for each row of the count table
