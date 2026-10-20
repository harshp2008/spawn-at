---
title: "Writing the docs"
---

How the pages of this guide are written and checked. It covers the hand-written pages in
`docs-site/src/content/docs/`. Rust code comments have their own rules in `docs/COMMENT_STYLE.md`
and are not covered here. Everything below is taken from the checks in `crates/xtask/src/` and the
conventions already used by the pages; where the repository states no rule, the text says so.

## Purpose

The guide tells a user or a contributor what spawn-at does, and tells them plainly what nobody has
verified. A page that sounds sure about something nobody ran does harm, so the rules below are about
evidence first and style second.

## The truth rule

- Every statement comes from a file in the repository or from a command that was run. The page names
  the file (with line numbers or a heading) in its `## Sources` list, or shows the command and its
  output.
- Anything else is written as "not verified", in the page's own `## Not verified` list or inline. The
  guide's front page defines the word: it means no test or run in the repository covers it.
- Code that was read but not run is said to be read: "read from source, not run".
- Do not invent a symptom, a step, a number or a version. If the repository does not record a cause,
  write that it records none.
- If a page has to describe behavior that looks like a bug, it describes what the code does, says so,
  and the finding is reported to the owner; the page does not hide it and the code is not changed in a
  docs change.
- The repository's contributor rules ("Working style") say the same for reports: never claim something is verified
  unless it was run, and always list what could not be run (a real GNOME Wayland session, X11, other
  GNOME versions, macOS, Windows).
- State no release status ("in beta", "from v0.x") in a timeless page. The changelog, the release
  notes and the Build field of an experiment record are history and keep their version words.

## Anatomy of a page

A hand-written page is a Markdown file under `docs-site/src/content/docs/`. `cargo xtask
docs-check-src` fails when:

- the file has no front matter (a `---` block) or no non-empty `title:`;
- the page repeats its title as a `# ` heading (Starlight prints the title as the heading, so the body
  starts with `##`);
- the body has fewer than 2 lines of text;
- the page is not named in the sidebar in `docs-site/astro.config.mjs` (the generated CLI reference
  is covered by an `autogenerate` entry instead);
- one of the four environment pages is missing;
- a hand-written page has no `## Sources` heading followed by a list;
- a link ends in `.md`;
- the config does not set `markdown.smartypants: false`;
- the text contains something that must not be published (below).

A page that adds new content therefore needs, in the same commit, its sidebar entry. Put the summary
first, then the body, then `## Not verified`, then `## Sources` last.

## Links

Link to other pages as `/spawn-at/<slug>/`, with the base written out and a trailing slash, for
example `[Update](/spawn-at/update/)`. Starlight does not rewrite links, so a relative link to a `.md`
file gives a 404 and is rejected by the check. A link to a heading uses its generated id:
`/spawn-at/update/#the-menu`. `cargo xtask docs-check-html` follows every relative link, script,
stylesheet and image in the built site and fails on a missing file or a `#fragment` the page does not
have. Links to other sites are counted and listed, not fetched.

The guide makes no third-party request: no external fonts, scripts or analytics. The same check fails if
any file names a known font, CDN or analytics host.

## Text that must not be published

`crates/xtask/src/docs_check.rs` (`FORBIDDEN`, 14 entries) rejects these anywhere in a page, ignoring
case. The exact strings are in that constant; this page does not repeat them, because a page that
contained them would fail the check.

| Class | Why |
| :--- | :--- |
| AI attribution: a co-author trailer, a tool-credit line, a session link or id, the vendor's domain and the no-reply address, the robot emoji | the guide carries no machine attribution |
| Two marker words for unfinished work, and one filler word for placeholder text (the markers match as whole words) | a published page must be finished |
| An absolute path under a user's home directory, in the Linux, macOS and Windows forms | it names a person's machine |
| The owner's first name, as a whole word | the guide does not use it |

The repository owner's account name in a URL is allowed, because the match is whole-word only. The
one string that contains the owner's name on purpose is the extension identifier, which the check
allows only as that exact string (`ALLOWED`, with a test): not extended by letters or digits and not
in another letter case. For example the Files and paths page writes `spawn-at@harsh.local`. Anything
else that needs one of these words is rewritten.

## Flags and dashes

`markdown.smartypants` is off in `astro.config.mjs`, because smart punctuation would turn `--id` in
prose into an en dash and `...` into an ellipsis. `docs-check-html` fails if a dash glued to the front
of a word appears in the built text. Write flags and paths in backticks anyway, and write the long form
of a flag (`--margin-top`) the way `--help` prints it.

## Tables

The site style (`src/styles/tables.css`) centers every table and makes it as wide as its content, up
to the width of the column; a table that is too wide scrolls inside its own box. Header cells wrap.
Every column keeps a floor of 12 characters, so a table with many columns, or with code pieces of up
to 20 characters in several of them, scrolls sideways instead of squeezing a column; keep the columns
of a table few and the cells short.

A table cell has at most 1200 characters of text (link targets, tags and the marks of code and
emphasis are not counted). `cargo xtask docs-check-src` fails on a longer cell, naming the page, the
table, the row key, the column and the length, and it checks the mirrored `project/` pages, so it
covers `docs/` too; the `experiments/` pages are evidence and are exempt. A long cell is a paragraph:
keep a short version (a few words or the key numbers) in the table and put the full text in a list
after it, under a bold lead-in that names the row, split at its natural joints.

A table or a code block that scrolls sideways shows a soft shadow (`src/styles/scroll-hints.css`) at
the edge that has more content behind it, and none at an edge that has been reached or when nothing
overflows. It is CSS only (`background-attachment: local`) and reads in both themes. A table shown as
records has no such hint, because it is not a table any more.

## Long tables as records

A table with a long cell cannot be read in a 45rem column, because each cell gets about 25
characters. On the pages under `project/` (the pages written from `docs/`), a table whose longest
body cell has 240 characters or more is shown as records instead: one card per row, in the order of
the rows. The first column is the key, in a gutter at the left of the card. The other columns are
fields in the body of the card, labelled with the column header (empty cells are left out). A field
whose text is 40 characters or fewer is a short fact: a small label over its value, side by side with
the other short ones. A longer one is a label over full-width text. The short fields come first in the
markup, then the long ones, each group in the order of the table, so what you see, what Tab reaches
and what a screen reader or a copy gives are in the same order. When the keys are sentences (the median key is longer than 48
characters), the key stands above the fields for the whole list, and on a narrow column every card
stacks.

A list in which any field has more than 400 characters of text (not counting the key column, markup
or empty cells) is shown as **titled cards**: the key is the title at the top of the card in bold
monospace, with a hairline under it, then the short fields as small label and value pairs in a
wrapping row, then each long field as a label over full-width text, with no gutter. The rule is per
list, not per card, so the cards of one list look alike; a field of exactly 400 characters does not
trigger it. It is the same layout as the stacked key, and steps are never changed by it.
`src/integrations/records.mjs` rewrites the table in the built page and
`src/styles/records.css` draws it. The page text, links, ids and inline code are the same as in the
table, and the search index is made from the text as written. The source stays a table, so GitHub
still shows a table.

When every key starts with a whole number (`1`, `2 (fixed in step 1)`, `12 files`; not `6.22` or
`A1`), the list is a list of steps instead: the gutter holds the number in large type, with the rest
of the key as a note under it (one pair of parentheses around the note is dropped). If only some of
the keys start with a number, the list is a normal list of cards. When the numbers are labels and
not an order, write them with a word ("Leak 1"), or use `style=cards` or `style=titled`.

A comment on the line directly above the table, before its header row, chooses the layout by hand.
GitHub shows nothing for it, and the page shows nothing for it:

```markdown
<!-- site: records -->
<!-- site: records key=2 -->
<!-- site: records style=steps -->
<!-- site: records style=cards -->
<!-- site: records style=titled -->
<!-- site: records key=2 style=cards -->
<!-- site: table -->
```

- `site: records` shows the table as records on any page, hand-written ones included. Hand-written
  pages are never converted without it.
- `site: records key=N` does the same with column N (counted from 1) as the key instead of column 1.
- `site: records style=steps` makes steps. The build fails, naming the page, the table and the row,
  when a key does not start with a number.
- `site: records style=cards` makes cards with the key in a gutter, even where every key starts with
  a number or a field is longer than 400 characters (a long key still stacks).
- `site: records style=titled` makes titled cards even where every field is short and where every key
  starts with a number (then the list is titled cards, not steps).
- `key=N` and `style=...` can be given together, each once, in any order. Only `records` takes them.
- `site: table` keeps a plain table where the length rule would convert it; use it for a matrix
  whose columns are read side by side. It takes no option.

A table with one column is never converted. `cargo xtask docs-check-src` fails, with the file and
line, on a comment that is not one of these forms, on `key=N` with N that is not a whole number, is 0
or is larger than the number of columns, on a `style` other than `steps`, `cards` or `titled`, on an option
repeated, on an option after `site: table`, on a comment that is not directly above a table, and on
`records` for a table with fewer than 2 columns. The check reads the mirrored `project/` pages, so
it covers the comments written in `docs/`. That the keys of a `style=steps` table are numbers is
checked by the site build, which reads the cells.

## Line breaks in inline code

Inline code breaks after a `/`, after `::`, after a `,` and at spaces, and nowhere else: not inside a
name, not at `_`, `.` or `-`, not in a line reference such as `session.rs:279-291`, not between `(`
and `)`. A piece between two break points that is longer than 20 characters is the exception: it moves
to the next line whole, and is cut anywhere only when it is wider than its line (a table column can be
narrower than a 50-character test name). The break points are added to the built pages by
`src/integrations/code-breaks.mjs`, an Astro integration that runs after Starlight, as `<wbr>`
elements and as spans around the pieces that must not break; they add no characters, so copying code
gives the text as written and the search index is made from the text as written. The page source needs
nothing special, and a page served by `astro dev` does not have the break points.

## Hiding the table of contents

On a screen at least 72rem wide, a button in the header (a script in `src/scripts/toc-toggle.js`,
added to every page by `astro.config.mjs`) hides the "On this page" column and widens the content
from 45rem to 56rem: text, lists, tables, code blocks and asides all use that one width (set once, in
`src/styles/toc-toggle.css`). The choice is kept in the browser's
`localStorage`. Without JavaScript the button does not exist and the column stays.

## Sources section style

The site style (`src/styles/custom.css`) shows the `## Sources` heading and its list smaller and
muted, under a thin rule. The heading and the list stay in the page source and `cargo xtask
docs-check-src` still requires them; the style only changes how they look. The list is not
collapsed, because the site's Markdown processor does not run rehype plugins without a new
dependency.

## Generated and hand-written pages

- The CLI reference (`reference/spawn-at*.md`) is generated from the built binary's `--help` at build time
  by `cargo xtask docs-cli-reference`. It is not committed (`.gitignore` lists it; only
  `reference/index.md` is tracked) and has no `Sources` list. To change it, change the `--help` text in
  the source; the reference follows. A wording problem in a generated page (for example a flag whose
  `--help` text differs from its verified behavior) is explained on a hand-written page that links to
  it.
- The "Project documents" pages (`project/<name>.md`) are written by `cargo xtask docs-mirror` from
  13 files of `docs/`, listed in `crates/xtask/src/docs_mirror.rs` with their page name and sidebar
  order. `docs/` is the source: edit the file there, never the page. The pages are not committed
  (`.gitignore` lists them; only `project/index.md` is tracked). The mirror turns the `# ` heading
  into the front-matter title, rewrites links to other mirrored files to `/spawn-at/project/<name>/`,
  sends links to files it does not mirror (`TEST_LOG.md`, `RELEASE_NOTES_*.md`, `../CHANGELOG.md`)
  to GitHub, and appends a `## Sources` section. It also gives each page a short sidebar label (the
  `label` of its entry in the list; the page title still follows the heading) and links a mention of
  an experiment, such as `6.12`, `section 6.15` or `6.11.1`, to the page of that experiment in
  `experiments/`, when the folder has a file for that number. It never links inside a heading, a code
  span, a code block, an existing link, a tag or a URL; it does not link a number that follows a digit,
  a `.`, a `v` or a letter, that is followed by a digit, a `.` and a digit, a letter or a unit (`0.6.12`,
  `16.12`, `6.12.1`, `6.12 ms`), or that has no page (`6.25`, `6.30`), and under each `### 6.NN`
  heading of `TEST_PLAN.md` it adds the link to the page next to the "Moved to TEST_LOG.md" line. The
  run prints the number of links and of mentions left unlinked per file. A new file in `docs/` fails
  the build until it is added to the mirrored list or the excluded list. The site rules still apply to the result, so a
  `docs/` file that is mirrored cannot contain a home path, the owner's name or the other forbidden
  text.
- Everything else is hand-written and must follow the anatomy above.

## Building and checking

```bash
cargo xtask docs-site                       # everything, in CI order; site in target/docs-site/site/
cargo xtask docs-check-src                  # markdown, sidebar, Sources, forbidden text
cargo xtask docs-cli-reference --bin target/debug/spawn-at   # only the generated reference
cargo xtask docs-mirror                     # only the Project documents pages, from docs/
cargo xtask docs-check-html target/docs-site/site --generated-dir api/   # links, ids, dashes, requests
```

`docs-site` needs Node and npm (the major version is in `docs-site/.nvmrc`) and runs `npm ci
--ignore-scripts` in `docs-site/`. Set `ASTRO_TELEMETRY_DISABLED=1` for any manual `astro` or `npm`
run there. The CI job is `.github/workflows/docs.yml`; it runs the same command on every pull request.
The checks above are listed in `docs/CODE_LAYOUT.md` ("Documentation site").

## Review checklist

- Every statement has a source in `## Sources` or a command shown with its output.
- What was not run is under `## Not verified`; code read but not run is marked as such.
- The page is in the sidebar, has front matter and a `title`, has no `# ` heading, and ends with
  `## Sources` and a list.
- Links are `/spawn-at/<slug>/`, none ends in `.md`, and the anchors exist.
- No forbidden text, no home path, no first name, no placeholder words.
- Numbers (test counts, line counts, versions) are from a run, with the date; counts that changed are
  updated in the same commit that changed them (the contributor rules, "Git and PRs").
- `cargo xtask docs-site` exits 0.

## Page template

Copy this to a new file, fill it in and add the slug to the sidebar:

````markdown
---
title: "Name of the page"
---

One or two sentences that say what the page covers and what it was written from.

## Heading of the first section

The body. Quote exact strings, flags and paths in backticks. Give line numbers for source claims.

## Not verified

- What was not run, with the reason.

## Sources

- `path/to/file` (heading or line numbers)
- the output of `command --help`
````

## Experiment record template

One page per experiment, under `experiments/`. It reads as a short report: an opening paragraph, a
table of fields with the verdict in it, then three `##` sections. The verdict comes first, in the
table, because a reader wants the outcome before the method. A field the source does not record is
written "not recorded"; never infer a build, a date or a command.

````markdown
---
title: "ID Short name"
---

One to three sentences: what was tested, and the outcome in plain words. Include the full title of the
experiment if it says more than the page title.

| Field | Value |
| :-- | :-- |
| **ID** | The id in the source (for example TEST_LOG 6.30), with links to the record before and after it |
| **Date** | The date the source gives, as YYYY-MM-DD |
| **Build** | The tag, or "DEV BUILD" with the commit; if only a tree or commit is recorded say so and infer no version; otherwise "not recorded" |
| **Verdict** | The source's verdict, in its words; if it gives none, say so |
| **Raw results** | Where they are; "not in the repository" if they are in a gitignored directory |

## What was tested

**Question**

What the experiment was meant to find out, in the source's words.

**Setup and environment**

The session, the machine, the load, the sample sizes, the arms.

**Commands run**

The commands, or "Exact command lines not recorded." followed by the prose the source gives.

## Result

The numbers and observations, as the source gives them. Use `###` or bold lead-ins for phases.

## Not verified

What the experiment did not cover.

## Sources

- `path/to/log` (section and lines)
````

## Not verified

- The CI job `docs.yml` was read, not run, for this page. The commands in the build section were taken
  from `docs/CODE_LAYOUT.md` and `crates/xtask/src/main.rs`.
- Whether the sidebar check accepts a page nested in a new folder without a folder entry: this page is
  the first nested under `contributing/`, and the check ran with it.

## Sources

- `crates/xtask/src/docs_check.rs` (module header, `FORBIDDEN`, `ALLOWED`, `check`)
- `crates/xtask/src/docs_html.rs` (module header)
- `docs/CODE_LAYOUT.md` ("Documentation site")
- `docs-site/astro.config.mjs` (sidebar, `markdown.smartypants`)
- `docs-site/src/content/docs/index.md` (the "not verified" definition and the Sources convention)
- `docs/COMMENT_STYLE.md` ("Truth rule") and the contributor rules ("Working style", "Git and PRs")
