//! `docs-check-src`: checks the markdown of the site (`docs-site/src/content/docs`) and the
//! Starlight config before the site is built.
//!
//! It fails when: a page has no front matter or no non-empty `title`; a page repeats its title as
//! a `# ` heading (Starlight prints the title as the heading); a page has fewer than 2 lines of
//! body text; a page is not named in the sidebar of `astro.config.mjs` (the generated CLI
//! reference and the mirrored `project/` pages are covered by an `autogenerate` entry instead);
//! one of the four environment pages is
//! missing; a hand-written page has no `## Sources` list; a link ends in `.md` (Starlight serves
//! no such file; links are `/spawn-at/<slug>/`); the config does not set
//! `markdown.smartypants: false`; or any page contains text that must not be published (AI
//! attribution, `TODO`, `TBD`, `lorem`, an absolute home-directory path, the owner's first name).
//! The one string allowed to contain that name is the extension identifier `spawn-at@harsh.local`
//! (`ALLOWED`). It also checks the `<!-- site: ... -->` comments above tables (see
//! `directive_problems`); the mirrored `project/` pages carry the comments of `docs/`, so the
//! sources of `docs/` are checked through them. A table cell of more than `MAX_CELL_CHARS`
//! (1200) characters of text fails too (see `long_cell_problems`), on every page except the
//! `experiments/` pages, which are evidence. An image reference fails when its file is missing
//! from `docs-site/public/img/`, has no alt text or points to another site (see `docs_images`);
//! the run then prints how many images there are, how many are placeholders and how many bytes
//! the folder holds.

use std::fs;
use std::path::{Path, PathBuf};

use crate::docs_cli::sidebar_autogenerates;
use crate::docs_images;

/// Pages every build must have, relative to the content folder.
pub const ENVIRONMENT_PAGES: [&str; 4] = [
    "environments/gnome-wayland.md",
    "environments/sway.md",
    "environments/x11-xwayland.md",
    "environments/unsupported-desktops.md",
];

/// Text that must not appear in any page: (needle, whole word only, reason). Matching ignores case.
const FORBIDDEN: [(&str, bool, &str); 14] = [
    ("co-authored-by", false, "AI attribution trailer"),
    ("generated with", false, "AI attribution line"),
    ("claude.ai", false, "AI session link"),
    ("anthropic.com", false, "AI vendor link"),
    ("noreply@anthropic", false, "AI attribution address"),
    ("session_0", false, "AI session id"),
    ("todo", true, "unfinished marker"),
    ("tbd", true, "unfinished marker"),
    ("lorem", true, "placeholder text"),
    ("/home/", false, "absolute path under /home"),
    ("/users/", false, "absolute path under /Users"),
    ("\\users\\", false, "absolute Windows user path"),
    ("harsh", true, "the owner's name"),
    ("🤖", false, "AI attribution marker"),
];

/// Splits a page into its front-matter text and the body after it. `None` if the page does not
/// start with a `---` line or the block is never closed by another `---` line.
pub fn split_front_matter(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("---\n")?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

/// The value of the top-level `title:` key of a front-matter block, without surrounding quotes.
/// Empty when the key is missing or has no value.
pub fn front_matter_title(front: &str) -> String {
    for line in front.lines() {
        if let Some(value) = line.strip_prefix("title:") {
            return value
                .trim()
                .trim_matches(|c| c == '"' || c == '\'')
                .to_string();
        }
    }
    String::new()
}

/// Non-blank lines that are not headings.
fn body_lines(text: &str) -> usize {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .count()
}

/// Lines of `body` outside code fences, with their 1-based numbers (counted from the body).
pub(crate) fn prose_lines(body: &str) -> Vec<(usize, &str)> {
    let mut in_fence = false;
    let mut out = Vec::new();
    for (i, line) in body.lines().enumerate() {
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            out.push((i + 1, line));
        }
    }
    out
}

/// True if the body has a line `# ...` (a level-1 heading) outside code fences.
pub fn has_h1(body: &str) -> bool {
    prose_lines(body).iter().any(|(_, l)| l.starts_with("# "))
}

/// Link targets in `body` (outside code fences) that end in `.md`, with or without a `#fragment`.
/// The book this site replaces linked pages that way; Starlight does not serve those files.
pub fn md_links(body: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (_, line) in prose_lines(body) {
        let mut rest = line;
        while let Some(i) = rest.find("](") {
            let after = &rest[i + 2..];
            let Some(end) = after.find(')') else { break };
            let target = &after[..end];
            let path = target.split('#').next().unwrap_or("");
            if path.ends_with(".md") && !target.contains("://") {
                found.push(target.to_string());
            }
            rest = &after[end..];
        }
    }
    found
}

/// True if `hay` holds `needle` (ASCII or not) ignoring case; with `whole_word` the characters on
/// both sides must not be letters or digits.
pub fn contains_text(hay: &str, needle: &str, whole_word: bool) -> bool {
    let hay = hay.to_lowercase();
    let needle = needle.to_lowercase();
    let mut from = 0;
    while let Some(i) = hay[from..].find(&needle) {
        let start = from + i;
        let end = start + needle.len();
        if !whole_word {
            return true;
        }
        let before = hay[..start].chars().next_back();
        let after = hay[end..].chars().next();
        let edge = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
        if edge(before) && edge(after) {
            return true;
        }
        from = start + needle.len().max(1);
    }
    false
}

/// Strings that contain a forbidden word but are published on purpose. Only the exact text is
/// allowed: `spawn-at@harsh.local` is the identifier of the GNOME Shell extension, so the Files
/// and paths page has to name it. Every other `harsh` still fails.
const ALLOWED: [&str; 1] = ["spawn-at@harsh.local"];

/// `text` with each allowed string replaced by a space. An occurrence counts only when it is
/// not part of a longer word (the characters on both sides are not letters or digits), so
/// `xspawn-at@harsh.local` or `spawn-at@harsh.localhost` are left as they are and still fail.
fn without_allowed(text: &str) -> String {
    let mut out = text.to_string();
    for allowed in ALLOWED {
        let mut result = String::with_capacity(out.len());
        let mut rest = out.as_str();
        let mut before: Option<char> = None;
        while let Some(i) = rest.find(allowed) {
            let end = i + allowed.len();
            let prev = rest[..i].chars().next_back().or(before);
            let next = rest[end..].chars().next();
            let edge = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
            if edge(prev) && edge(next) {
                result.push_str(&rest[..i]);
                result.push(' ');
            } else {
                result.push_str(&rest[..end]);
            }
            before = rest[..end].chars().next_back();
            rest = &rest[end..];
        }
        result.push_str(rest);
        out = result;
    }
    out
}

/// Reasons the text may not be published; empty when it is clean.
pub fn forbidden_in(text: &str) -> Vec<String> {
    let text = without_allowed(text);
    FORBIDDEN
        .iter()
        .filter(|(needle, word, _)| contains_text(&text, needle, *word))
        .map(|(needle, _, why)| format!("contains `{needle}` ({why})"))
        .collect()
}

/// True if the page has a `## Sources` heading followed by at least one list item before the
/// next heading.
pub fn has_sources_list(text: &str) -> bool {
    let mut in_sources = false;
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with('#') {
            if in_sources {
                return false;
            }
            in_sources = t.trim_start_matches('#').trim() == "Sources" && t.starts_with("## ");
        } else if in_sources && (t.starts_with("- ") || t.starts_with("* ")) {
            return true;
        }
    }
    false
}

/// Cells of one table row as GFM splits them: at every `|` that is not preceded by a backslash
/// (code spans do not protect a pipe), after dropping one leading and one trailing pipe.
fn split_cells(line: &str) -> Vec<&str> {
    let mut t = line.trim();
    t = t.strip_prefix('|').unwrap_or(t);
    if t.ends_with('|') && !t.ends_with("\\|") {
        t = &t[..t.len() - 1];
    }
    let bytes = t.as_bytes();
    let mut cells = Vec::new();
    let (mut start, mut i) = (0, 0);
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'|' => {
                cells.push(t[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    cells.push(t[start..].trim());
    cells
}

/// The column count if `header` and `delimiter` are the first two lines of a GFM table (a delimiter
/// row of `:?-+:?` cells with as many cells as the header row), else `None`.
fn table_columns(header: &str, delimiter: &str) -> Option<usize> {
    if !header.contains('|') || !delimiter.contains('-') {
        return None;
    }
    let cols = split_cells(header).len();
    let delim = split_cells(delimiter);
    let ok = delim.len() == cols
        && delim.iter().all(|c| {
            let c = c.strip_prefix(':').unwrap_or(c);
            let c = c.strip_suffix(':').unwrap_or(c);
            !c.is_empty() && c.bytes().all(|b| b == b'-')
        });
    ok.then_some(cols)
}

/// A table cell may have at most this many characters of text (link targets, tags and the marks
/// of code and emphasis not counted). A longer cell is a paragraph that wants a list after the
/// table; `experiments/` pages are evidence and are not checked.
pub const MAX_CELL_CHARS: usize = 1200;

/// Characters of text in a table cell: `\|` is one `|`, a link counts its text and not its target,
/// a tag and the marks `` ` `` and `*` count nothing.
fn cell_text_len(cell: &str) -> usize {
    let cell = cell.replace("\\|", "|");
    let mut text = String::with_capacity(cell.len());
    let mut rest = cell.as_str();
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            if let Some(end) = rest.find('>') {
                rest = &rest[end + 1..];
                continue;
            }
        }
        if c == ']' && rest[1..].starts_with('(') {
            if let Some(end) = rest.find(')') {
                rest = &rest[end + 1..];
                continue;
            }
        }
        if c != '`' && c != '*' && c != '[' {
            text.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    text.trim().chars().count()
}

/// Cells over `MAX_CELL_CHARS` characters of text in the tables of one page body, as (1-based line
/// in `body`, message with the table, the row key, the column and the length). Tables in code
/// fences are not tables.
pub fn long_cell_problems(body: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = body.lines().collect();
    let mut problems = Vec::new();
    let mut in_fence = false;
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
        }
        let is_table = !in_fence
            && lines
                .get(i + 1)
                .is_some_and(|d| table_columns(lines[i], d).is_some());
        if !is_table {
            i += 1;
            continue;
        }
        let header: Vec<String> = split_cells(lines[i])
            .iter()
            .map(|h| h.replace('`', ""))
            .collect();
        let mut j = i + 2;
        while j < lines.len() && lines[j].trim_start().starts_with('|') {
            let cells = split_cells(lines[j]);
            for (c, cell) in cells.iter().enumerate() {
                let len = cell_text_len(cell);
                if len > MAX_CELL_CHARS {
                    let key = row_label(cells[0]);
                    problems.push((
                        j + 1,
                        format!(
                            "table `{}`: the cell of row `{key}`, column `{}`, has {len} characters of text (at most {MAX_CELL_CHARS}); keep a short version in the cell and move the full text into a list after the table",
                            header.first().map(String::as_str).unwrap_or(""),
                            header.get(c).map(String::as_str).unwrap_or("?")
                        ),
                    ));
                }
            }
            j += 1;
        }
        i = j;
    }
    problems
}

/// The first cell of a row as a short label for a message: marks removed, at most 40 characters.
fn row_label(cell: &str) -> String {
    let plain: String = cell.chars().filter(|c| *c != '`' && *c != '*').collect();
    plain.chars().take(40).collect()
}

const DIRECTIVE_FORMS: &str =
    "use `site: records`, `site: records key=N`, `site: records style=steps`, `site: records style=cards`, `site: records style=titled` or `site: table`";

/// Problems with the `<!-- site: ... -->` comments of one page body, as (1-based line in `body`,
/// message). The comments choose how the site shows the table on the next line
/// (`docs-site/src/integrations/records.mjs` reads them from the built page): `site: records`,
/// `site: records key=N` (column N, counted from 1, is the key), `site: records style=steps`,
/// `style=cards` or `style=titled` (`key=N` and one style combine, each once) and `site: table`,
/// which takes no option. A
/// comment must be alone on its line, directly above a table; `records` needs at least 2 columns; N
/// must not be larger than the column count. That every key of a `style=steps` table starts with a
/// number is checked by the build, which reads the cells. Comments inside code fences and other
/// comments are ignored.
pub fn directive_problems(body: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = body.lines().collect();
    let mut problems = Vec::new();
    let mut in_fence = false;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let Some(inner) = t
            .trim_end()
            .strip_prefix("<!--")
            .and_then(|r| r.strip_suffix("-->"))
            .map(str::trim)
        else {
            continue;
        };
        if inner.split(|c: char| c == ':' || c.is_whitespace()).next() != Some("site") {
            continue;
        }
        let mut err = |msg: String| problems.push((i + 1, msg));
        let Some(rest) = inner.strip_prefix("site:") else {
            err(format!("`{inner}` needs a colon: {DIRECTIVE_FORMS}"));
            continue;
        };
        let mut words = rest.split_whitespace();
        let mode = words.next().unwrap_or("");
        if mode != "records" && mode != "table" {
            err(format!("unknown directive `{inner}` ({DIRECTIVE_FORMS})"));
            continue;
        }
        let mut key = None;
        let mut style_seen = false;
        let mut bad = false;
        for opt in words {
            if let Some(v) = opt.strip_prefix("key=") {
                if mode == "table" {
                    err("`site: table` takes no key".to_string());
                    bad = true;
                } else if key.is_some() {
                    err(format!("key given twice in `{inner}`"));
                    bad = true;
                } else {
                    match v.parse::<usize>() {
                        Ok(0) => {
                            err("key=0: the columns are numbered from 1".to_string());
                            bad = true;
                        }
                        Ok(n) => key = Some(n),
                        Err(_) => {
                            err(format!("key=`{v}` is not a whole number"));
                            bad = true;
                        }
                    }
                }
            } else if let Some(v) = opt.strip_prefix("style=") {
                if mode == "table" {
                    err("`site: table` takes no style".to_string());
                    bad = true;
                } else if !matches!(v, "steps" | "cards" | "titled") {
                    err(format!(
                        "unknown style `{v}` (style=steps, style=cards or style=titled)"
                    ));
                    bad = true;
                } else if style_seen {
                    err(format!("style given twice in `{inner}`"));
                    bad = true;
                }
                style_seen = true;
            } else {
                err(format!("unknown option `{opt}` in `{inner}`"));
                bad = true;
            }
        }
        if bad {
            continue;
        }
        let table = match (lines.get(i + 1), lines.get(i + 2)) {
            (Some(h), Some(d)) if !h.trim_start().starts_with("```") => table_columns(h, d),
            _ => None,
        };
        let Some(cols) = table else {
            err(format!(
                "`{inner}` must be on the line directly above a table"
            ));
            continue;
        };
        if mode == "records" && cols < 2 {
            err(format!(
                "`{inner}`: records need a table with at least 2 columns, this one has {cols}"
            ));
        } else if let Some(n) = key.filter(|&n| n > cols) {
            err(format!("key={n} but the table has {cols} columns"));
        }
    }
    problems
}

/// The sidebar name of a page file: `install.md` is `install`, `commands/index.md` is `commands`,
/// and the top-level `index.md` is `index`.
pub fn page_slug(rel: &str) -> String {
    let no_ext = rel.strip_suffix(".md").unwrap_or(rel);
    match no_ext.strip_suffix("/index") {
        Some(dir) => dir.to_string(),
        None => no_ext.to_string(),
    }
}

/// True if `config` names the page `slug` as a string, in `'single'` or `"double"` quotes. Text
/// search, not a JavaScript parse: `'install'` and `slug: 'commands'` both match.
pub fn sidebar_names(config: &str, slug: &str) -> bool {
    config.contains(&format!("'{slug}'")) || config.contains(&format!("\"{slug}\""))
}

/// True if the config turns smart punctuation off. Left on, markdown turns `--id` in prose into
/// an en dash.
pub fn disables_smartypants(config: &str) -> bool {
    config
        .lines()
        .any(|l| !l.trim_start().starts_with("//") && l.contains("smartypants: false"))
}

fn markdown_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            markdown_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
    Ok(())
}

/// A page generated from the binary (not hand-written, so it has no `Sources` list and the
/// sidebar reaches it through `autogenerate`).
fn is_generated(rel: &str) -> bool {
    rel.starts_with("reference/") && rel != "reference/index.md"
}

/// A page written by `docs-mirror` from a file of `docs/`. Unlike a generated reference page it
/// has a `## Sources` list (the mirror appends one) and every other rule applies; only the
/// sidebar-name rule is replaced by the `autogenerate` entry for the folder `project`.
fn is_mirrored(rel: &str) -> bool {
    rel.starts_with("project/") && rel != "project/index.md"
}

/// The folder of the images, `public/img/` next to the sidebar config (`docs-site/`).
fn image_dir(config: &Path) -> PathBuf {
    config
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join("public/img")
}

/// All problems found under `docs` (the content folder) with the sidebar config at `config`,
/// one line each. Empty means the site source passes. Only the tests call it; `run` calls
/// `check_with_images`.
#[cfg(test)]
pub fn check(docs: &Path, config: &Path) -> Result<Vec<String>, String> {
    check_with_images(docs, config).map(|(problems, _)| problems)
}

/// `check`, and the report on the images folder (see `docs_images`).
pub fn check_with_images(
    docs: &Path,
    config: &Path,
) -> Result<(Vec<String>, docs_images::ImageReport), String> {
    let config_text = fs::read_to_string(config)
        .map_err(|e| format!("cannot read {}: {}", config.display(), e))?;
    let mut problems = Vec::new();
    let img_dir = image_dir(config);
    let mut used_images = std::collections::BTreeSet::new();

    if !disables_smartypants(&config_text) {
        problems.push(format!(
            "{}: `markdown: {{ smartypants: false }}` is not set, so `--id` in prose would be \
             rendered with an en dash",
            config.display()
        ));
    }
    if !sidebar_autogenerates(&config_text, "reference") {
        problems.push(format!(
            "{}: no `autogenerate: {{ directory: 'reference' }}` sidebar entry for the CLI reference",
            config.display()
        ));
    }
    for rel in ENVIRONMENT_PAGES {
        if !docs.join(rel).is_file() {
            problems.push(format!("{rel}: environment page does not exist"));
        }
    }

    let mut files = Vec::new();
    markdown_files(docs, &mut files)
        .map_err(|e| format!("cannot read {}: {}", docs.display(), e))?;
    files.sort();
    for file in files {
        let rel = file
            .strip_prefix(docs)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        let text = fs::read_to_string(&file)
            .map_err(|e| format!("cannot read {}: {}", file.display(), e))?;
        for why in forbidden_in(&text) {
            problems.push(format!("{rel}: {why}"));
        }
        let Some((front, body)) = split_front_matter(&text) else {
            problems.push(format!(
                "{rel}: no front matter (a `---` block with `title:`)"
            ));
            continue;
        };
        if front_matter_title(front).is_empty() {
            problems.push(format!("{rel}: front matter has no `title`"));
        }
        if has_h1(body) {
            problems.push(format!(
                "{rel}: has a `# ` heading; Starlight prints the title as the heading"
            ));
        }
        if body_lines(body) < 2 {
            problems.push(format!(
                "{rel}: empty page (fewer than 2 lines of body text)"
            ));
        }
        // Lines of `body` are counted from the end of the front matter; the report uses page lines.
        let front_lines = text[..text.len() - body.len()].matches('\n').count();
        let origin = crate::docs_mirror::MIRRORED
            .iter()
            .find(|e| rel == format!("project/{}.md", e.slug))
            .map(|e| format!(" (generated from docs/{})", e.file))
            .unwrap_or_default();
        for (line, why) in directive_problems(body) {
            problems.push(format!("{rel}:{}{origin}: {why}", front_lines + line));
        }
        if !rel.starts_with("experiments/") {
            for (line, why) in long_cell_problems(body) {
                problems.push(format!("{rel}:{}{origin}: {why}", front_lines + line));
            }
        }
        for target in md_links(body) {
            problems.push(format!(
                "{rel}: link to `{target}` ends in .md; link to /spawn-at/<slug>/ instead"
            ));
        }
        let refs = docs_images::image_refs(&prose_lines(body));
        for (line, why) in docs_images::image_problems(&refs, &img_dir) {
            problems.push(format!("{rel}:{}{origin}: {why}", front_lines + line));
        }
        used_images.extend(docs_images::used_names(&refs));
        if !is_generated(&rel) {
            if !has_sources_list(body) {
                problems.push(format!("{rel}: no `## Sources` heading with a list"));
            }
            let covered = is_mirrored(&rel) && sidebar_autogenerates(&config_text, "project");
            if !covered && !sidebar_names(&config_text, &page_slug(&rel)) {
                problems.push(format!(
                    "{rel}: page `{}` is not named in the sidebar of {}",
                    page_slug(&rel),
                    config.display()
                ));
            }
        }
    }
    let report = docs_images::image_report(&img_dir, &used_images)?;
    Ok((problems, report))
}

/// Entry point of `cargo xtask docs-check-src [DOCS_DIR] [--config FILE]` (defaults
/// `docs-site/src/content/docs` and `docs-site/astro.config.mjs`).
pub fn run(args: &[String]) -> Result<(), String> {
    let mut docs = PathBuf::from("docs-site/src/content/docs");
    let mut config = PathBuf::from("docs-site/astro.config.mjs");
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--config" => {
                config = PathBuf::from(it.next().ok_or("--config needs a value")?);
            }
            other if !other.starts_with("--") => docs = PathBuf::from(other),
            other => return Err(format!("unknown option {other}")),
        }
    }
    let (problems, images) = check_with_images(&docs, &config)?;
    if problems.is_empty() {
        println!("==> site source OK ({})", docs.display());
        for line in docs_images::summary(&images, &image_dir(&config)) {
            println!("{line}");
        }
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `todo` as a word is forbidden but `autodocs`/`mastodon` are not; `harsh` as a word is
    /// forbidden but the repository owner's account name `harshp2008` is not.
    #[test]
    fn test_whole_word_matching() {
        assert!(contains_text("a TODO here", "todo", true));
        assert!(!contains_text("mastodon todos", "todo", true));
        assert!(contains_text("by Harsh.", "harsh", true));
        assert!(!contains_text(
            "github.com/harshp2008/spawn-at",
            "harsh",
            true
        ));
    }

    /// Each forbidden class is caught: attribution trailer, session link, TODO, lorem, an
    /// absolute home path.
    #[test]
    fn test_forbidden_in_catches_each_class() {
        for bad in [
            "Co-Authored-By: Someone",
            "Generated with a tool",
            "see https://claude.ai/code/session_01",
            "TODO: write this",
            "Lorem ipsum",
            "/home/someone/project",
            "TBD",
        ] {
            assert!(!forbidden_in(bad).is_empty(), "not caught: {bad}");
        }
        assert!(forbidden_in("spawn-at spawn --anchor center gnome-calculator").is_empty());
    }

    /// The extension identifier `spawn-at@harsh.local` passes, alone, in prose, in code spans, in
    /// a path and twice in a line; the name `harsh` anywhere else still fails, also next to the
    /// identifier, and so do the identifier extended by letters or digits, other casing and a
    /// look-alike. A failure of the first group means the Files and paths page cannot name the
    /// extension; a failure of the second means the allowance leaks.
    #[test]
    fn test_extension_id_is_allowed_and_nothing_else_with_harsh() {
        for ok in [
            "spawn-at@harsh.local",
            "The id is `spawn-at@harsh.local`.",
            "~/.local/share/gnome-shell/extensions/spawn-at@harsh.local/",
            "gnome-extensions enable spawn-at@harsh.local, gnome-extensions disable spawn-at@harsh.local",
            "(spawn-at@harsh.local)",
        ] {
            assert!(forbidden_in(ok).is_empty(), "wrongly rejected: {ok}");
        }
        for bad in [
            "harsh",
            "by Harsh.",
            "spawn-at@harsh.local and harsh",
            "harsh and spawn-at@harsh.local",
            "xspawn-at@harsh.local",
            "spawn-at@harsh.localhost",
            "SPAWN-AT@HARSH.LOCAL",
            "spawn-at@harsh.example",
            "other@harsh.local",
            "/home/someone/spawn-at@harsh.local",
        ] {
            assert!(!forbidden_in(bad).is_empty(), "wrongly accepted: {bad}");
        }
    }

    /// A Sources list needs the `##` heading and at least one item; an empty heading, a deeper
    /// heading, or items under another heading do not count.
    #[test]
    fn test_has_sources_list() {
        assert!(has_sources_list("# T\ntext\n## Sources\n\n- `README.md`\n"));
        assert!(!has_sources_list("# T\n## Sources\n\n## Next\n- item\n"));
        assert!(!has_sources_list("# T\n### Sources\n- item\n"));
        assert!(!has_sources_list("# T\n- item\n"));
    }

    /// Front matter is the block between the first two `---` lines; a page without the opening
    /// line, or with a block that never closes, has none. The body starts after the closing line.
    #[test]
    fn test_split_front_matter() {
        let (front, body) = split_front_matter("---\ntitle: \"A\"\n---\n\nText\n").unwrap();
        assert_eq!(front, "title: \"A\"\n");
        assert_eq!(body, "\nText\n");
        assert!(split_front_matter("# A\n\nText\n").is_none());
        assert!(split_front_matter("---\ntitle: A\n\nText\n").is_none());
    }

    /// Quoted, single-quoted and bare titles read the same; a missing or empty `title` gives "".
    #[test]
    fn test_front_matter_title() {
        assert_eq!(front_matter_title("title: \"Install\"\n"), "Install");
        assert_eq!(front_matter_title("title: 'Install'\n"), "Install");
        assert_eq!(
            front_matter_title("sidebar:\n  label: X\ntitle: Install\n"),
            "Install"
        );
        assert_eq!(front_matter_title("title:\n"), "");
        assert_eq!(front_matter_title("description: x\n"), "");
    }

    /// A `# ` line is a duplicate title, but `## ` is not, and a `# comment` inside a code fence
    /// (a shell comment) is not a heading.
    #[test]
    fn test_has_h1_ignores_fences_and_h2() {
        assert!(has_h1("Text\n# Title\n"));
        assert!(!has_h1("## Section\n```bash\n# comment\n```\n"));
    }

    /// Links ending in `.md` are found with and without a fragment; site links, external links
    /// and `.md` text inside a code fence are not reported.
    #[test]
    fn test_md_links() {
        let body = "[a](findings.md) [b](../x/y.md#frag) [c](/spawn-at/findings/) \
                    [d](https://example.org/readme.md)\n```\n[e](hidden.md)\n```\n";
        assert_eq!(md_links(body), vec!["findings.md", "../x/y.md#frag"]);
    }

    const TABLE3: &str = "| a | b | c |\n| :-- | --: | :-: |\n| 1 | 2 | 3 |\n";

    /// The three forms in the docs, with and without a blank line above, with spaces inside the
    /// comment varied, on a 1-column table for `table`, and key = the last column, are accepted;
    /// ordinary comments and a directive inside a code fence are not looked at.
    #[test]
    fn test_directives_valid_forms_pass() {
        for text in [
            "<!-- site: records -->",
            "<!--site:records-->",
            "<!-- site: records key=3 -->",
            "<!-- site:   records   key=1 -->",
            "<!-- site: table -->",
            "<!-- site: records style=steps -->",
            "<!-- site: records style=cards -->",
            "<!-- site: records style=titled -->",
            "<!-- site: records key=2 style=cards -->",
            "<!-- site: records key=2 style=titled -->",
            "<!-- site: records style=steps key=3 -->",
        ] {
            let body = format!("Text\n\n{text}\n{TABLE3}");
            assert!(directive_problems(&body).is_empty(), "{text}");
        }
        assert!(directive_problems("<!-- site: table -->\n| a |\n| - |\n| 1 |\n").is_empty());
        assert!(directive_problems("<!-- a note -->\n<!-- sitemap -->\ntext\n").is_empty());
        let fenced = "```\n<!-- site: bogus -->\n```\n";
        assert!(directive_problems(fenced).is_empty());
    }

    /// Each kind of mistake is reported once, with the 1-based line of the comment.
    #[test]
    fn test_directives_each_error() {
        let cases: [(&str, &str); 20] = [
            ("<!-- site: record -->", "unknown directive `site: record`"),
            ("<!-- site: -->", "unknown directive `site:`"),
            ("<!-- site records -->", "needs a colon"),
            (
                "<!-- site: records key=x -->",
                "key=`x` is not a whole number",
            ),
            (
                "<!-- site: records key=-1 -->",
                "key=`-1` is not a whole number",
            ),
            ("<!-- site: records key=0 -->", "numbered from 1"),
            (
                "<!-- site: records key=4 -->",
                "key=4 but the table has 3 columns",
            ),
            ("<!-- site: table key=2 -->", "`site: table` takes no key"),
            ("<!-- site: records wide -->", "unknown option `wide`"),
            (
                "<!-- site: records style=rail -->",
                "unknown style `rail` (style=steps, style=cards or style=titled)",
            ),
            (
                "<!-- site: records style= -->",
                "unknown style `` (style=steps, style=cards or style=titled)",
            ),
            (
                "<!-- site: records style=Titled -->",
                "unknown style `Titled` (style=steps, style=cards or style=titled)",
            ),
            (
                "<!-- site: table style=steps -->",
                "`site: table` takes no style",
            ),
            (
                "<!-- site: table style=titled -->",
                "`site: table` takes no style",
            ),
            (
                "<!-- site: records style=steps style=cards -->",
                "style given twice",
            ),
            (
                "<!-- site: records style=cards style=titled -->",
                "style given twice",
            ),
            (
                "<!-- site: records style=titled style=titled -->",
                "style given twice",
            ),
            (
                "<!-- site: records style=titled key=2 key=3 -->",
                "key given twice",
            ),
            ("<!-- site: records key=1 key=2 -->", "key given twice"),
            (
                "<!-- site: records style=steps key=2 key=3 -->",
                "key given twice",
            ),
        ];
        for (comment, expected) in cases {
            let body = format!("intro\n{comment}\n{TABLE3}");
            let found = directive_problems(&body);
            assert_eq!(found.len(), 1, "{comment}: {found:?}");
            assert_eq!(found[0].0, 2, "{comment}");
            assert!(found[0].1.contains(expected), "{comment}: {}", found[0].1);
        }
    }

    /// `style=` is a records option: both values pass in either order with `key=N`, a bad value is
    /// reported once (and not also as a key problem), and on `site: table` it is an error whatever
    /// its value, with or without a key beside it.
    #[test]
    fn test_directive_style_option() {
        for text in [
            "<!-- site: records style=steps key=2 -->",
            "<!-- site: records key=2 style=steps -->",
            "<!-- site: records key=3 style=cards -->",
            "<!-- site: records style=titled key=2 -->",
            "<!-- site: records key=3 style=titled -->",
        ] {
            let body = format!("{text}\n{TABLE3}");
            assert!(directive_problems(&body).is_empty(), "{text}");
        }
        for (text, expected) in [
            (
                "<!-- site: records key=2 style=Steps -->",
                "unknown style `Steps`",
            ),
            (
                "<!-- site: table style=cards -->",
                "`site: table` takes no style",
            ),
            (
                "<!-- site: table style=bogus -->",
                "`site: table` takes no style",
            ),
        ] {
            let found = directive_problems(&format!("{text}\n{TABLE3}"));
            assert_eq!(found.len(), 1, "{text}: {found:?}");
            assert!(found[0].1.contains(expected), "{text}: {}", found[0].1);
        }
        let found =
            directive_problems(&format!("<!-- site: table key=2 style=cards -->\n{TABLE3}"));
        assert_eq!(found.len(), 2, "{found:?}");
    }

    /// The comment must be on the line right above the table header: a blank line, text, a
    /// non-table line pair, a code fence or the end of the page after it is an error, and so is a
    /// table whose delimiter row has another cell count than the header.
    #[test]
    fn test_directives_must_sit_above_a_table() {
        for body in [
            format!("<!-- site: records -->\n\n{TABLE3}"),
            "<!-- site: records -->\nplain text\nmore text\n".to_string(),
            "<!-- site: table -->\n```\n| a | b |\n| - | - |\n```\n".to_string(),
            "<!-- site: table -->".to_string(),
            "<!-- site: records -->\n| a | b |\n| - | - | - |\n".to_string(),
        ] {
            let found = directive_problems(&body);
            assert_eq!(found.len(), 1, "{body:?}: {found:?}");
            assert!(found[0].1.contains("directly above a table"), "{found:?}");
        }
    }

    /// `records` and `key` on a table with one column are errors, `table` is not; a pipe inside
    /// code or escaped with a backslash does not add a column.
    #[test]
    fn test_directives_need_two_columns_for_records() {
        let one = "| a |\n| - |\n| 1 |\n";
        for text in ["<!-- site: records -->", "<!-- site: records key=1 -->"] {
            let found = directive_problems(&format!("{text}\n{one}"));
            assert_eq!(found.len(), 1, "{text}");
            assert!(found[0].1.contains("at least 2 columns"), "{found:?}");
        }
        let escaped = "<!-- site: records key=2 -->\n| `a\\|b` | c |\n| - | - |\n| 1 | 2 |\n";
        assert!(directive_problems(escaped).is_empty());
        let unescaped = "<!-- site: records key=3 -->\n| a | b |\n| - | - |\n";
        assert_eq!(directive_problems(unescaped).len(), 1);
    }

    /// GFM cell splitting: leading and trailing pipes are optional, `\|` stays in the cell, and
    /// a pipe inside a code span still splits.
    #[test]
    fn test_split_cells() {
        assert_eq!(split_cells("| a | b |"), vec!["a", "b"]);
        assert_eq!(split_cells("a | b"), vec!["a", "b"]);
        assert_eq!(split_cells("| `x\\|y` | z |"), vec!["`x\\|y`", "z"]);
        assert_eq!(split_cells("| `x|y` | z |"), vec!["`x", "y`", "z"]);
        assert_eq!(split_cells("| a | |"), vec!["a", ""]);
    }

    /// `check` reports a directive error with the page line (the front matter counts) and, for a
    /// mirrored page, the `docs/` file the page comes from.
    #[test]
    fn test_check_reports_directive_errors_with_origin() {
        let body = format!("---\ntitle: \"P\"\n---\n\nText\n<!-- site: records key=9 -->\n{TABLE3}\n## Sources\n\n- [s](https://example.org/x)\n");
        let mut files: Vec<(&str, &str)> = vec![
            ("index.md", GOOD_PAGE),
            ("project/design-platforms.md", body.as_str()),
        ];
        for p in ENVIRONMENT_PAGES {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("directive-origin", &files);
        let mut names: Vec<String> = vec!["index".to_string()];
        names.extend(env_slugs());
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let with_project = format!(
            "{}\n{{ autogenerate: {{ directory: 'project' }} }}",
            config_for(&refs)
        );
        fs::write(&config, with_project).unwrap();
        let problems = check(&docs, &config).unwrap().join("\n");
        assert!(
            problems.contains(
                "project/design-platforms.md:6 (generated from docs/DESIGN_PLATFORMS.md): \
                 key=9 but the table has 3 columns"
            ),
            "{problems}"
        );
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }

    fn table_with_cell(n: usize) -> String {
        format!(
            "| Key | Text |\n| :-- | :-- |\n| `row one` | {} |\n",
            "x".repeat(n)
        )
    }

    /// A cell counts its text: a link counts its words and not its target, a tag and the marks of
    /// code and emphasis count nothing, and `\|` is one character.
    #[test]
    fn test_cell_text_len_counts_text_only() {
        assert_eq!(cell_text_len("abc"), 3);
        assert_eq!(
            cell_text_len("[abc](https://example.org/a/very/long/target)"),
            3
        );
        assert_eq!(cell_text_len("`ab` **cd** <wbr>e"), 7); // "ab cd e"
        assert_eq!(cell_text_len("a\\|b"), 3);
        assert_eq!(cell_text_len("  padded  "), 6);
        assert_eq!(cell_text_len("é€漢"), 3);
    }

    /// A cell of 1200 characters passes and one of 1201 fails, naming the table, the row key, the
    /// column header and the length; a cell in a second column and a second table are found too.
    #[test]
    fn test_long_cell_limit_is_1200_characters() {
        assert_eq!(MAX_CELL_CHARS, 1200);
        assert!(long_cell_problems(&table_with_cell(1200)).is_empty());
        let found = long_cell_problems(&format!("intro\n\n{}", table_with_cell(1201)));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, 5, "the line of the row, counted from 1");
        for part in [
            "table `Key`",
            "row `row one`",
            "column `Text`",
            "has 1201 characters of text (at most 1200)",
            "list after the table",
        ] {
            assert!(found[0].1.contains(part), "{part}: {}", found[0].1);
        }
        let two = format!(
            "{}\n| Key | A | B |\n| - | - | - |\n| k | {} | short |\n| k2 | x | {} |\n",
            table_with_cell(10),
            "y".repeat(1300),
            "z".repeat(1500)
        );
        let found = long_cell_problems(&two);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].1.contains("column `A`") && found[1].1.contains("column `B`"));
    }

    /// A table inside a code fence, and lines with pipes that are not a table (no delimiter
    /// row), are not looked at.
    #[test]
    fn test_long_cells_ignore_fences_and_non_tables() {
        let long = "x".repeat(1500);
        let fenced = format!("```\n| a | b |\n| - | - |\n| 1 | {long} |\n```\n");
        assert!(long_cell_problems(&fenced).is_empty());
        let not_table = format!("| a | b |\n| 1 | {long} |\n");
        assert!(long_cell_problems(&not_table).is_empty());
        let tilde = format!("~~~\n{}~~~\n", table_with_cell(1500));
        assert!(long_cell_problems(&tilde).is_empty());
    }

    /// `check` reports a long cell with the page line and, for a mirrored page, the `docs/` file; the
    /// same cell on an `experiments/` page (evidence) is not reported.
    #[test]
    fn test_check_reports_long_cells_but_not_on_experiments() {
        let page = |table: &str| {
            format!("---\ntitle: \"P\"\n---\n\nText\n\n{table}\n## Sources\n\n- [s](https://example.org/x)\n")
        };
        let long = page(&table_with_cell(1300));
        let mut files: Vec<(&str, &str)> = vec![
            ("index.md", GOOD_PAGE),
            ("project/test-plan.md", long.as_str()),
            ("experiments/6-99-x.md", long.as_str()),
        ];
        for p in ENVIRONMENT_PAGES {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("long-cells", &files);
        let mut names: Vec<String> = vec!["index".to_string()];
        names.extend(env_slugs());
        names.push("experiments/6-99-x".to_string());
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let with_project = format!(
            "{}\n{{ autogenerate: {{ directory: 'project' }} }}",
            config_for(&refs)
        );
        fs::write(&config, with_project).unwrap();
        let problems = check(&docs, &config).unwrap().join("\n");
        assert!(
            problems
                .contains("project/test-plan.md:9 (generated from docs/TEST_PLAN.md): table `Key`"),
            "{problems}"
        );
        assert!(
            problems.contains("has 1300 characters of text"),
            "{problems}"
        );
        assert!(!problems.contains("experiments/6-99-x.md"), "{problems}");
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }

    /// The real mirrored documents have no cell over the limit (the same check as `check` on the
    /// generated pages, so a long cell fails `cargo test` and not only the site build).
    #[test]
    fn test_real_docs_have_no_cell_over_the_limit() {
        let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
        for entry in &crate::docs_mirror::MIRRORED {
            let text = fs::read_to_string(docs.join(entry.file)).unwrap();
            let found = long_cell_problems(&text);
            assert!(found.is_empty(), "docs/{}: {found:?}", entry.file);
        }
    }

    /// Every directive in the real `docs/` files, as written into the mirrored pages, is valid.
    /// A typo in a comment would otherwise only show as a table that is not converted.
    #[test]
    fn test_real_docs_directives_are_valid() {
        let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
        for entry in &crate::docs_mirror::MIRRORED {
            let text = fs::read_to_string(docs.join(entry.file)).unwrap();
            let page = crate::docs_mirror::render(entry, &text).unwrap();
            let (_, body) = split_front_matter(&page).unwrap();
            let found = directive_problems(body);
            assert!(found.is_empty(), "docs/{}: {found:?}", entry.file);
        }
    }

    /// File names map to the name the sidebar uses: folder index pages take the folder name.
    #[test]
    fn test_page_slug() {
        assert_eq!(page_slug("install.md"), "install");
        assert_eq!(page_slug("commands/index.md"), "commands");
        assert_eq!(page_slug("environments/sway.md"), "environments/sway");
        assert_eq!(page_slug("index.md"), "index");
    }

    /// A slug counts as named only as a whole quoted string: `'commands'` does not satisfy
    /// `commands/spawn`, and an unquoted mention in a comment does not count.
    #[test]
    fn test_sidebar_names() {
        let config =
            "items: ['install', { slug: \"commands\", label: 'Overview' }, 'commands/spawn']";
        assert!(sidebar_names(config, "install"));
        assert!(sidebar_names(config, "commands"));
        assert!(sidebar_names(config, "commands/spawn"));
        assert!(!sidebar_names(config, "commands/query"));
        assert!(!sidebar_names("// install\n", "install"));
    }

    /// `smartypants: false` passes; leaving it out, `true`, or only a comment that mentions it
    /// fails. This is the setting that stops `--id` in prose from becoming an en dash.
    #[test]
    fn test_disables_smartypants() {
        assert!(disables_smartypants("markdown: { smartypants: false },"));
        assert!(!disables_smartypants("markdown: { smartypants: true },"));
        assert!(!disables_smartypants("export default {};"));
        assert!(!disables_smartypants("  // smartypants: false\n"));
    }

    const GOOD_PAGE: &str =
        "---\ntitle: \"P\"\n---\n\nSome real text.\nMore text.\n\n## Sources\n\n- `README.md`\n";

    fn site(tag: &str, files: &[(&str, &str)]) -> (PathBuf, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("xtask-docs-check-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        for (rel, text) in files {
            let path = dir.join("docs").join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        (dir.join("docs"), dir.join("astro.config.mjs"))
    }

    fn config_for(pages: &[&str]) -> String {
        let names: Vec<String> = pages.iter().map(|p| format!("'{p}'")).collect();
        format!(
            "markdown: {{ smartypants: false }},\nitems: [{}, {{ autogenerate: {{ directory: 'reference' }} }}]",
            names.join(", ")
        )
    }

    fn env_slugs() -> Vec<String> {
        ENVIRONMENT_PAGES.iter().map(|p| page_slug(p)).collect()
    }

    /// A complete small site passes with no problems.
    #[test]
    fn test_check_passes_a_complete_site() {
        let mut files: Vec<(&str, &str)> = vec![("index.md", GOOD_PAGE)];
        for p in ENVIRONMENT_PAGES {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("complete", &files);
        let mut names: Vec<String> = vec!["index".to_string()];
        names.extend(env_slugs());
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        fs::write(&config, config_for(&refs)).unwrap();
        assert_eq!(check(&docs, &config).unwrap(), Vec::<String>::new());
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }

    /// A missing environment page, a page without front matter, a page with an H1, an empty
    /// page, a `.md` link, a page without Sources, a page missing from the sidebar and a page
    /// with a forbidden word each produce their own problem line.
    #[test]
    fn test_check_reports_each_kind_of_problem() {
        let mut files: Vec<(&str, &str)> = vec![
            ("index.md", "---\ntitle: \"I\"\n---\n"),
            ("nofront.md", "# Plain\n\nText\nMore\n"),
            ("h1.md", "---\ntitle: \"H\"\n---\n\n# H\n\nText\nMore\n"),
            (
                "link.md",
                "---\ntitle: \"L\"\n---\n\nsee [x](findings.md)\nMore\n",
            ),
            ("stray.md", "---\ntitle: \"S\"\n---\n\nTODO later\nMore\n"),
            ("untitled.md", "---\ndescription: d\n---\n\nText\nMore\n"),
        ];
        for p in &ENVIRONMENT_PAGES[..3] {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("problems", &files);
        fs::write(&config, config_for(&["index", "h1", "link", "stray"])).unwrap();
        let problems = check(&docs, &config).unwrap().join("\n");
        for expected in [
            "unsupported-desktops.md: environment page does not exist",
            "index.md: empty page",
            "index.md: no `## Sources`",
            "nofront.md: no front matter",
            "h1.md: has a `# ` heading",
            "link.md: link to `findings.md` ends in .md",
            "stray.md: contains `todo`",
            "untitled.md: front matter has no `title`",
            "untitled.md: page `untitled` is not named in the sidebar",
        ] {
            assert!(
                problems.contains(expected),
                "missing `{expected}` in:\n{problems}"
            );
        }
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }

    /// A config without `smartypants: false` or without the reference `autogenerate` entry fails
    /// even when every page is fine.
    #[test]
    fn test_check_reports_config_problems() {
        let mut files: Vec<(&str, &str)> = vec![("index.md", GOOD_PAGE)];
        for p in ENVIRONMENT_PAGES {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("config", &files);
        fs::write(&config, "items: ['index']").unwrap();
        let problems = check(&docs, &config).unwrap().join("\n");
        assert!(problems.contains("smartypants: false"), "{problems}");
        assert!(problems.contains("autogenerate"), "{problems}");
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }

    /// Generated reference pages are exempt from the Sources rule and the sidebar-name rule
    /// (the sidebar autogenerates them) but not from the word rules.
    #[test]
    fn test_generated_pages_are_exempt_from_sources_only() {
        assert!(is_generated("reference/spawn-at-spawn.md"));
        assert!(!is_generated("reference/index.md"));
        assert!(!is_generated("environments/sway.md"));
    }

    /// Only `project/<page>.md` counts as mirrored; the hand-written `project/index.md` and the
    /// generated reference pages do not.
    #[test]
    fn test_is_mirrored() {
        assert!(is_mirrored("project/test-plan.md"));
        assert!(!is_mirrored("project/index.md"));
        assert!(!is_mirrored("reference/spawn-at-spawn.md"));
        assert!(!is_mirrored("install.md"));
    }

    /// A mirrored page needs no sidebar entry of its own when the config autogenerates the
    /// `project` folder, and fails the sidebar rule when it does not.
    #[test]
    fn test_mirrored_page_is_covered_by_the_project_autogenerate_entry() {
        let mut files: Vec<(&str, &str)> =
            vec![("index.md", GOOD_PAGE), ("project/test-plan.md", GOOD_PAGE)];
        for p in ENVIRONMENT_PAGES {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("mirrored-sidebar", &files);
        let mut names: Vec<String> = vec!["index".to_string()];
        names.extend(env_slugs());
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();

        fs::write(&config, config_for(&refs)).unwrap();
        let problems = check(&docs, &config).unwrap().join("\n");
        assert!(
            problems.contains("project/test-plan.md: page `project/test-plan` is not named"),
            "{problems}"
        );

        let with_project = format!(
            "{}\n{{ autogenerate: {{ directory: 'project' }} }}",
            config_for(&refs)
        );
        fs::write(&config, with_project).unwrap();
        assert_eq!(check(&docs, &config).unwrap(), Vec::<String>::new());
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }

    /// The `autogenerate` exemption covers the sidebar rule only: a mirrored page with a `# `
    /// heading, a `.md` link, no Sources list or forbidden text still fails.
    #[test]
    fn test_mirrored_page_still_obeys_every_other_rule() {
        let bad = "---\ntitle: \"P\"\n---\n\n# P\n\nSee [x](CONFIG.md)\nTODO\n";
        let mut files: Vec<(&str, &str)> = vec![("index.md", GOOD_PAGE), ("project/bad.md", bad)];
        for p in ENVIRONMENT_PAGES {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("mirrored-rules", &files);
        let mut names: Vec<String> = vec!["index".to_string()];
        names.extend(env_slugs());
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let with_project = format!(
            "{}\n{{ autogenerate: {{ directory: 'project' }} }}",
            config_for(&refs)
        );
        fs::write(&config, with_project).unwrap();
        let problems = check(&docs, &config).unwrap().join("\n");
        for expected in [
            "project/bad.md: has a `# ` heading",
            "project/bad.md: link to `CONFIG.md` ends in .md",
            "project/bad.md: no `## Sources`",
            "project/bad.md: contains `todo`",
        ] {
            assert!(
                problems.contains(expected),
                "missing `{expected}` in:\n{problems}"
            );
        }
        assert!(!problems.contains("not named in the sidebar"), "{problems}");
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }

    /// A generated page with front matter and body passes without a Sources list or a sidebar
    /// entry of its own.
    #[test]
    fn test_check_accepts_generated_page_without_sources_or_sidebar_entry() {
        let mut files: Vec<(&str, &str)> = vec![
            ("index.md", GOOD_PAGE),
            (
                "reference/spawn-at-spawn.md",
                "---\ntitle: \"spawn-at spawn\"\n---\n\nGenerated text.\nMore.\n",
            ),
        ];
        for p in ENVIRONMENT_PAGES {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("generated", &files);
        let mut names: Vec<String> = vec!["index".to_string()];
        names.extend(env_slugs());
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        fs::write(&config, config_for(&refs)).unwrap();
        assert_eq!(check(&docs, &config).unwrap(), Vec::<String>::new());
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }

    /// An image in `public/img/` next to the config passes and is counted (one placeholder, one
    /// real, one used); a missing file, an empty alt text and an address on another site each give
    /// a problem with the page and line.
    #[test]
    fn test_check_reads_image_references_and_reports_the_folder() {
        let page = "---\ntitle: \"P\"\n---\n\nSome real text.\n\n\
                    ![A terminal in the corner](/spawn-at/img/ok.svg)\n\n\
                    ![](/spawn-at/img/ok.svg)\n\n\
                    ![Gone](/spawn-at/img/gone.webp)\n\n\
                    ![Far](https://example.org/a.png)\n\n\
                    The syntax `![alt](/spawn-at/img/not-an-image.svg)` in a code span is text.\n\n\
                    ## Sources\n\n- `README.md`\n";
        let mut files: Vec<(&str, &str)> = vec![("index.md", page)];
        for p in ENVIRONMENT_PAGES {
            files.push((p, GOOD_PAGE));
        }
        let (docs, config) = site("images", &files);
        let mut names: Vec<String> = vec!["index".to_string()];
        names.extend(env_slugs());
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        fs::write(&config, config_for(&refs)).unwrap();
        let img = config.parent().unwrap().join("public/img");
        fs::create_dir_all(&img).unwrap();
        fs::write(img.join("ok.svg"), "<svg>Placeholder: a thing</svg>").unwrap();
        fs::write(img.join("real.webp"), "RIFF").unwrap();
        let (problems, report) = check_with_images(&docs, &config).unwrap();
        let problems = problems.join("\n");
        assert!(
            problems.contains("index.md:9: image `/spawn-at/img/ok.svg` has no alt text"),
            "{problems}"
        );
        assert!(
            problems.contains("index.md:11: image `gone.webp` does not exist"),
            "{problems}"
        );
        assert!(
            problems.contains("index.md:13: image `https://example.org/a.png` is on another site"),
            "{problems}"
        );
        assert!(!problems.contains("not-an-image"), "{problems}");
        assert_eq!(problems.lines().count(), 3, "{problems}");
        assert_eq!((report.files, report.placeholders, report.used), (2, 1, 1));
        assert_eq!(report.used_placeholders, 1);
        let _ = fs::remove_dir_all(docs.parent().unwrap());
    }
}
