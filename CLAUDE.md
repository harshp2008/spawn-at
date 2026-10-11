# spawn-at project rules

## Language
- All committed tooling is Rust. Exceptions: GNOME extension JS, node tests, and thin shell wrappers that only call cargo or a binary.
- No Python in committed files (no python3 heredocs, no `python3 -c`).
- Throwaway scripts live only in `scratch/` (gitignored). Never commit them.

## Working style
- Never read a file over 400 lines whole. Use grep -n and line ranges.
- Paste raw command output. Never condense it by hand. Never use `|| true` to hide failures.
- Never claim something is verified unless you ran it. Say what you could not run.
- One commit per step, conventional commit messages.

## Refactors
- Pure-move refactors need proof before commit: sorted test names per binary, sha256 of goldens/fixtures/specs, `--help` / `run --help` / `list` diffs, `git diff -M --stat`. All diffs must be empty.
- Never mix moves and logic changes in one commit.

## Comments
- Write comments by reading the function body, never from the function name.
- Name concrete values, widths, strings and what failure the test catches. No template filler.
- If unsure, leave it uncommented. A missing comment beats a wrong one.
- Comment-only commits must pass `cargo xtask check-comment-only <rev>`.

## Tests
- Report counts per binary using `cargo test -p spawn-at-verify --features tui,fixture`, plus the workspace total. Never use a bare `cargo test --workspace` number for spawn-at-verify.
