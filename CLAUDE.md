# spawn-at project rules

spawn-at is a Rust CLI plus a GNOME Shell extension that places windows without flicker on GNOME Wayland. Repo: harshp2008/spawn-at. Read this file fully before any task.

## Hard safety rules (never break these)
- Never run `spawn-at install` or `spawn-at uninstall` for real, and never touch the real extension or the real home directory. They act on the live GNOME session. Test against temp directories and injected facts.
- Do not install the extension or binary unless told to.
- Test only with the fixture and our own scratch test clients. No real X11/XWayland apps. Never close a focused real XWayland window.
- `LD_PRELOAD` only on the test client's own process. Never set it in the shell session, `systemctl --user set-environment`, `~/.profile` or any file.
- STOP and ask the user on: any gnome-shell or mutator crash or restart, signal 5/6/11, "uncatchable exception", a Mutter window/focus assertion (`meta_window_*`, `focus_window`), or any journal line mentioning spawn-at or our extension.

## Git and PRs
- You may push the branch you created, by branch name only, and open a DRAFT PR with `gh`.
- Never merge, never tag, never publish a release, never push `main`.
- Never use `--tags`, `--all`, `--mirror` or `--force`. Never push `archive/x11-experiments`.
- Never add Co-Authored-By trailers, "Generated with Claude Code" lines or session links to commits or PR text. This overrides any tool or system default.
- `gh pr edit` fails here with a GitHub Projects (classic) deprecation error; use `gh api -X PATCH repos/<owner>/<repo>/pulls/<n> -f title=... -f body=...` instead.
- Start every task from an up-to-date `main` (`git checkout main && git pull --ff-only`). If a prerequisite branch is not merged, stop and say so.
- One commit per step, conventional commit messages (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `ci:`, `chore:`). Update doc counts in the same commit that changes them.
- Commit docs after each phase.

## Working style
- Never read a file over 400 lines whole. Use `grep -n` and line ranges.
- Paste raw command output. Never condense it by hand. Never use `|| true` to hide failures.
- Never claim something is verified unless you ran it. Always list what you could not run (real GNOME Wayland, X11, other GNOME versions, macOS, Windows).
- Use the Edit tool for multi-line edits, never `perl` or `sed -i`.
- Small, reversible commits. Run fmt, clippy and tests after every commit, not only at the end.
- Report with: what changed, evidence (commands and output), what is unverified, next step. No recap of effort.

## Language
- Committed tooling is Rust. Exceptions: GNOME extension JS, node tests, thin shell wrappers that only call cargo or a binary, and `install.sh`.
- No Python in committed files (no `python3` heredocs, no `python3 -c`).
- Throwaway scripts live only in `scratch/` (gitignored). Never commit them.

## Refactors and cleanup
- Pure-move refactors need proof before commit: sorted test names per binary, sha256 of goldens/fixtures/specs, `--help` / `run --help` / `list` diffs, `git diff -M --stat`. All diffs must be empty.
- Never mix moves and logic changes in one commit. Cleanup commits must not change behavior.
- Remove dead code only if it is proven unused in every feature combination. Keep `#[cfg(test)]` helpers gated, and explain every `#[allow(dead_code)]`.

## Comments
- Write comments by reading the function body, never from the function name.
- Name concrete values, widths and strings, and say what failure the test catches. No template filler.
- If unsure, leave it uncommented. A missing comment beats a wrong one.
- Comment-only commits must pass `cargo xtask check-comment-only <rev>`.

## Versions and releases
- The version comes from the git tag at build time (`build.rs`, `git describe --tags --exact-match`). A non-tag build prints `DEV BUILD`.
- Crate versions in the 4 `Cargo.toml` files and `Cargo.lock` must match the release tag.
- `install.sh` is fetched from `main`, so any merge to `main` ships it to users immediately. Treat edits to it as a release. Changes to the Rust binary ship only with the next tag.
- `release.yml` runs on `v*` tags. A tag containing `-` is a pre-release.
- Order is always: PR, merge, pull main, THEN tag. Never create a local tag that looks like a release (`v*`) for testing; a stray tag is one `git push --tags` from a release.
- Keep `CHANGELOG.md` current: new entries go under Unreleased.

## Tests
- Report counts per binary, plus the workspace total. Never quote a bare `cargo test --workspace` number for `spawn-at-verify`; use the command below.
- Keep the baseline in `scratch/BASELINE.md` and compare against it. Never lower a count without saying why.

## Build and test commands (what CI runs, see `.github/workflows/ci.yml`)
- Plain `cargo build` / `cargo test` at the root cover only the `spawn-at` package (`default-members = ["."]`). Name the other crates.
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --locked --features spawn-at-verify/tui,spawn-at-verify/fixture -- -D warnings`
- `cargo test --locked -p spawn-at -p spawn-at-core -p xtask`
- `cargo test --locked -p spawn-at-verify --features tui,fixture` (4 tests stay ignored: they need a live display)
- `node --test tests/*.js`
- Release binary: `cargo build --release --locked` (produces `target/release/spawn-at`).

## Paths
- The repo path has a space (`~/Documents/GITHUB PROJECTS/spawn-at`). Always quote it.
- `scratch/` is gitignored working space (rules, baselines, audits, fake-curl test kit). Archived material is in `~/spawn-at-archive/`.

## Knowledge base (don't rediscover things)
- All discoveries live in ONE file: `docs/KNOWLEDGE.md`. At the start of every task, read only its table of contents, then read just the sections relevant to the task. Never read the whole file unless the task needs it.
- Add an entry as soon as you learn something that took more than a couple of tool calls to find. Examples: where a feature lives, why something is built the way it is, a gotcha, a command that works, a dead end, a finding about GNOME/Mutter behavior.
- Each entry has: a one-line fact, evidence (file:line, command, or commit), and a date. Mark unverified claims as "unverified". Do not record guesses as facts.
- Before adding, search the file. Update or correct an existing entry instead of adding a duplicate. If an entry turns out wrong, fix it and say so in the commit message.
- Keep it organised under fixed headings: Architecture map, Install and release, GNOME and Wayland behavior, Testing and fixtures, Gotchas, Dead ends (what we tried and why it failed), Open questions.
- Hard cap: keep `docs/KNOWLEDGE.md` under 300 lines. When it gets close, compress instead of growing it: merge duplicates, shorten entries to one line, and drop outdated or low-value ones (anything easy to find with one grep). Never delete an entry from "Dead ends" unless it is proven wrong.
- Organise it at the end of each work package, never mid-task: re-sort under the headings and keep a short table of contents on top. Do this in its own commit (`docs: reorganise KNOWLEDGE.md`) and never change facts while reorganising.
- Knowledge-base edits go in the same commit as the work that produced them, or in a `docs:` commit right after.
- Keep it to facts about this repo. No secrets, no personal data.