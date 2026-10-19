# Proofs for behavior-preserving refactors

The platform restructuring in [DESIGN_PLATFORMS.md](DESIGN_PLATFORMS.md) section 6 is a series of small steps that must not change what spawn-at does on Linux. After each step the same snapshot is taken and compared with the one taken before the first step.

## What is captured

| Item | How |
| :-- | :-- |
| Sorted test names per binary | `cargo test --locked -p spawn-at -- --list`, the same for `spawn-at-core`, `xtask`, and `spawn-at-verify --features tui,fixture`; the `name: test` lines, sorted |
| `--help` of `spawn-at` and every subcommand | read from the `Commands:` list of the help, nested ones too (`config show`, `query layout`, ...), plus the hidden `unminimize`; `install` and `uninstall` included (24 files) |
| Hashes of goldens and vectors | `sha256sum` of `verify/golden/placement.json` and every tracked file under `tests/ready_wait_vectors/` |
| Installer behavior | the 48-case fake-curl matrix of `install.sh` (stdout, stderr and exit status per case) |
| Release binary size | `stat -c %s target/release/spawn-at`; printed for information only, because moving code changes embedded file paths and can change the size by a few bytes |

## Commands

The scripts live in `scratch/baseline_platform/` (gitignored; they are throwaway tooling around cargo and the binary):

```bash
# once, on the commit before the refactor starts: take the baseline
bash scratch/baseline_platform/capture.sh scratch/baseline_platform/base

# after each step: capture again and diff against the baseline
bash scratch/baseline_platform/compare.sh        # prints PASS, or the exact differences
```

If `compare.sh` does not print `PASS`, the step is not behavior-preserving: stop, do not push, and do not edit the baseline to match.

The matrix needs the fake-curl helpers in `scratch/b2/` (`matrix.sh`, `fakebin/`, `good.tgz`, `bad.tgz`, `nojq/`).

## Checks that go with every step

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --locked --features spawn-at-verify/tui,spawn-at-verify/fixture -- -D warnings
cargo test --locked -p spawn-at -p spawn-at-core -p xtask          # 273 / 64 / 10 (as first recorded, in v0.2.0-beta.4)
cargo test --locked -p spawn-at-verify --features tui,fixture      # 404 passed, 4 ignored (as first recorded, in v0.2.0-beta.4)
node --test tests/*.js                                             # 183 (as first recorded, in v0.2.0-beta.4)
cargo build --release --locked
cargo check --locked -p spawn-at --target x86_64-pc-windows-gnu
cargo check --locked -p spawn-at --target x86_64-apple-darwin
cargo check --locked -p spawn-at --target aarch64-apple-darwin
```

For a pure move also: `git diff -M --stat` (renames are detected, additions equal deletions apart from `mod` lines and visibility) and `git diff -M --color-moved=dimmed-zebra --stat`.

## What these proofs do not cover

Nothing here runs a live GNOME session. The live suites (`spawn-at-verify` on a real GNOME 46 Wayland session, the 4 ignored tests, the `hostile` specs) must be run by the owner before a release that contains a refactor step.

## The release binary size is not a gate

Moving code between modules changes how rustc splits the crate into codegen units, which changes inlining of generic code (toml, zbus, tokio closures) and so the size, with no change in behavior. Observed: merging the two duplicate `/proc` helpers into one (step 2) grew `.text` by 21,792 bytes (0.36%); a single added comment line moved the binary by 264 bytes. A symbol-by-symbol comparison of the step 2 build showed the same functions, with one `is_process_descendant` (1,207 bytes) replacing two, and re-sized monomorphised generics. So the size is printed for information only; the gates are the test names, the help text, the hashes and the installer matrix.

## Proving a split is a pure move

A mechanical check that complements `git diff -M`: normalise every line of the old file and of the new files (strip leading and trailing blanks, drop empty lines, sort) and compare the two sets with `comm`. After the split of `gnome/mod.rs` the only lines on either side are re-wrapped lines (rustfmt after the tests lost one level of indentation), the imports and `mod` lines, the new file headers, and the two one-line forwards. Commands: `comm -23 old.txt new.txt` and `comm -13 old.txt new.txt`.
