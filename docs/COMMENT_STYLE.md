# Comment style

Rules for comments in Rust code. Comment-only commits must pass
`scripts/check-comment-only-product.sh <rev>` (wraps `cargo xtask check-comment-only`).

## Where comments go

- **Module header (`//!`)**: every module has one. Say what the module is for, who calls it,
  and what it must not do (for example "no I/O", "does not name a display server").
- **Public items (`///`)**: every public item has one. Say what it does, and where they apply:
  the errors it returns, when it panics, the units of its numbers (ms, px), and its side effects.
- **Non-obvious code (`//`)**: explain WHY (the constraint, the failure it avoids), not WHAT the
  next line does. Name concrete values and strings (`30 ms`, `"ready-wait"`) instead of "a short time".

## Truth rule

- Every statement must be checkable from the code or an existing doc. Open the function body
  and its callers before writing; never write a comment from the name alone.
- If you are not sure, say less, or write "not verified". A missing comment beats a wrong one.
- No invented history ("used to", "was added because") unless a commit or doc says so.
- A comment that contradicts the code is fixed or deleted, and listed in the PR report.
  The code is never changed to match a comment.

## Public docs versus other files

- `///` and `//!` appear on the public rustdoc site. They describe only the item itself and its
  contract.
- Statements about OTHER files ("no code under `src/` reads this", "only the Sway driver compares
  it", "`spawn` always sets 1") go stale. Put them in a plain `//` comment next to the item,
  worded as a fact about the code at this commit, or leave them out.
- Mark anything you could not verify as "not verified" instead of guessing.

## Form

- Keep comments short. One sentence is often enough.
- No examples in `///` unless they use a ```` ```text ```` fence. A Rust fence becomes a doctest,
  which changes the test counts and can fail.
- Link related items with rustdoc links (`[`Policy`]`, `[`crate::ready_wait::Outcome`]`) only if
  they resolve; `cargo doc` runs with warnings as errors.
- Tests: leave test code alone. Fix a comment inside a test only if it is wrong.

## Forbidden

- Template filler ("Returns the value", "Creates a new instance").
- Restating the type or the name.
- Claims about other components that you did not read (extension, compositor, GNOME behaviour).
- Changing code, strings, attributes, imports or code whitespace in a comment-only change.
