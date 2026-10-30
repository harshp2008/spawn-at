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
- Never use `pkill`, `killall` or any kill matched by a name or pattern: only signal PIDs you recorded yourself from a child you started.
- `scripts/smoke-live.sh` launches real GNOME apps (`gnome-calculator`, `gnome-text-editor`). The owner runs it by hand; Claude Code never runs it.

## Build and test commands (what CI runs, see `.github/workflows/ci.yml`)
- Plain `cargo build` / `cargo test` at the root cover only the `spawn-at` package (`default-members = ["."]`). Name the other crates.
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --locked --features spawn-at-verify/tui,spawn-at-verify/fixture -- -D warnings`
- `cargo test --locked -p spawn-at -p spawn-at-core -p xtask`
- `cargo test --locked -p spawn-at-verify --features tui,fixture` (5 tests stay ignored: 4 in `fixture_test.rs` need a live display, 1 in `focus_chain_sway.rs` needs a private Sway)
- `node --test tests/*.js`
- Release binary: `cargo build --release --locked` (produces `target/release/spawn-at`).
- Docs site (CI job `docs`, `.github/workflows/docs.yml`): `cargo xtask docs-site`. It needs Node + npm (major in `docs-site/.nvmrc`), runs `npm ci --ignore-scripts` in `docs-site/` and builds with Starlight (Astro). Pieces: `cargo xtask docs-check-src`, `cargo xtask docs-cli-reference`, `cargo xtask docs-check-html <dir>`. Set `ASTRO_TELEMETRY_DISABLED=1` for any manual `astro` or `npm` run there.

## Paths
- The repo path has a space (`~/Documents/GITHUB PROJECTS/spawn-at`). Always quote it.
- `scratch/` is gitignored working space (rules, baselines, audits, fake-curl test kit). Archived material is in `~/spawn-at-archive/`.

## Recording results (after every run)

This section is identical in AGENTS.md; change both together.

Why: we must always be able to see what we tried, what we saw, and what we still do not know. A run is not finished until it is written down.

When: after every run that measures or tests something, big or small, and after anything that changes what we believe. Do it before you send your final report. Runs that found nothing, or failed, are written down too.

Where things go:
- Small raw data, logs and sample files: docs/evidence/<run-name>/data/
- The story of the run in plain English: docs/evidence/<run-name>/findings.md. README.md in the same folder says what the run was and how to read the files.
- Short facts worth keeping: docs/KNOWLEDGE.md, under the right heading. Dead ends go under "Dead ends".
- Anything a user will notice: CHANGELOG.md under Unreleased.
- The website page for the run: write the entry in docs/TEST_LOG.md. The experiments page 6.NN is built from it. Pages go live only through a docs pull request that the owner merges.
- One list of every run: docs/evidence/INDEX.md. One row per run: name, date, the question, the result in one line, the branch, and the website page or "not on the site yet". Add your row in the same commit as your findings.
- A mistake in something we wrote earlier: fix it where it appears, add a dated "Correction" line that says what was wrong and what is true, and list it under "Corrections" in your findings.md.

Be honest:
1. Say what you did, what you saw and how sure you are.
2. Label each claim. MEASURED: you ran it, and you name the file or cell. READ: from source code or docs, and you name the file or link. GUESS: your reasoning, not tested.
3. Give counts as "N of M runs", with the app, size, build and conditions.
4. Say what was not run, and why.
5. Never delete or quietly rewrite a wrong or failed result. Leave it, add a dated correction, and mark the old page "Superseded by ...".
6. If a new result disagrees with an old one, say so on both pages.
7. Do not say a fix works unless you measured before and after under the same conditions.

Write in plain English:
- Short sentences, one idea each.
- Use everyday words. Use a technical word only when there is no plain way to say it, and explain it in one sentence the first time it appears on a page.
- Use numbers, not adjectives: "17 of 20 runs", not "most runs".
- Say what it means for the person using spawn-at.
- Keep bold and code formatting light.
- How to shape a page for a run is in docs-site/src/content/docs/contributing/writing-docs.md, section "Recording a run".

Before you finish a run, copy this checklist into your final report and mark each line done, not done (with the reason), or not needed:
- [ ] findings.md and README.md written in docs/evidence/<run-name>/
- [ ] row added to docs/evidence/INDEX.md
- [ ] new facts and dead ends added to docs/KNOWLEDGE.md
- [ ] CHANGELOG.md updated if a user would notice
- [ ] entry added to docs/TEST_LOG.md
- [ ] corrections made where earlier statements were wrong
- [ ] "not run" list and "not sure" list written

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