//! `docs-mirror`: writes selected `docs/*.md` files as pages of the site's "Project documents"
//! section (`docs-site/src/content/docs/project/`) at build time, so `docs/` stays the only copy.
//!
//! The pages are generated and git ignores everything in that folder except the hand-written
//! `index.md`. Which files are mirrored is a fixed list (`MIRRORED`: file, page name, sidebar
//! order); `TEST_LOG.md` and `RELEASE_NOTES_*.md` are excluded on purpose (`EXCLUDED_FILES`,
//! `EXCLUDED_PREFIXES`). A `docs/*.md` file that is in neither list fails the run, so a new file
//! is always a decision. Outside code fences, the text of a file is changed in five ways:
//!
//! - the first `# ` heading becomes the front-matter `title` (double-quoted YAML) and is removed,
//!   because Starlight prints the title as the heading; `sidebar.order` and the short
//!   `sidebar.label` come from the list;
//! - a link to a mirrored sibling (`DESIGN_SWAY.md#x`) becomes `/spawn-at/project/design-sway/#x`;
//! - a link to an excluded file (`TEST_LOG.md#x`) or out of `docs/` (`../CHANGELOG.md`) becomes a
//!   GitHub `blob/main` URL; links to `#anchors` and to other sites are left alone;
//! - a mention such as `6.12`, `section 6.15` or `6.11.1` becomes a link to the experiment page of
//!   that number when the folder `experiments/` has one (`link_experiments`; never inside code,
//!   headings or existing links); the "Moved to TEST_LOG.md" line under a `### 6.NN` heading
//!   gets a second sentence with the link to the page on this site;
//! - a `## Sources` section naming the source file with its GitHub URL is appended.
//!
//! The sidebar reaches the pages through an `autogenerate` entry for the folder `project` in
//! `docs-site/astro.config.mjs`, which this module checks for, as `docs_cli` does for `reference`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::docs_check::has_sources_list;
use crate::docs_cli::{sidebar_autogenerates, yaml_quote};

/// One mirrored file.
pub struct Entry {
    /// File name in `docs/`.
    pub file: &'static str,
    /// Page name: the generated file is `<slug>.md` and the page is `/spawn-at/project/<slug>/`.
    pub slug: &'static str,
    /// Position in the sidebar group (`sidebar.order`); `project/index.md` is 0.
    pub order: u32,
    /// Short name in the sidebar (`sidebar.label`), in sentence case, written from the file's
    /// `# ` heading. The page title and the browser tab still follow the heading.
    pub label: &'static str,
}

/// Every mirrored file. The order is the sidebar order: how the code is laid out, how placement
/// behaves, the settings, the legacy extension, the design notes, then tests and house rules.
pub const MIRRORED: [Entry; 13] = [
    Entry {
        file: "CODE_LAYOUT.md",
        slug: "code-layout",
        order: 1,
        label: "Code layout",
    },
    Entry {
        file: "behavior-notes.md",
        slug: "behavior-notes",
        order: 2,
        label: "Behavior notes",
    },
    Entry {
        file: "CONFIG.md",
        slug: "config",
        order: 3,
        label: "Settings",
    },
    Entry {
        file: "legacy-gnome.md",
        slug: "legacy-gnome",
        order: 4,
        label: "Legacy GNOME (42–44)",
    },
    Entry {
        file: "DESIGN_PLATFORMS.md",
        slug: "design-platforms",
        order: 5,
        label: "Design: more platforms",
    },
    Entry {
        file: "DESIGN_SWAY.md",
        slug: "design-sway",
        order: 6,
        label: "Design: Sway backend",
    },
    Entry {
        file: "DESIGN_READY_WAIT.md",
        slug: "design-ready-wait",
        order: 7,
        label: "Design: ready-wait",
    },
    Entry {
        file: "DESIGN_RUNNER_SETUP.md",
        slug: "design-runner-setup",
        order: 8,
        label: "Design: runner setup",
    },
    Entry {
        file: "TEST_PLAN.md",
        slug: "test-plan",
        order: 9,
        label: "Test plan",
    },
    Entry {
        file: "TEST_COVERAGE_AUDIT.md",
        slug: "test-coverage-audit",
        order: 10,
        label: "Test coverage audit",
    },
    Entry {
        file: "REFACTOR_PROOFS.md",
        slug: "refactor-proofs",
        order: 11,
        label: "Refactor proofs",
    },
    Entry {
        file: "COMMENT_STYLE.md",
        slug: "comment-style",
        order: 12,
        label: "Comment style",
    },
    Entry {
        file: "KNOWLEDGE.md",
        slug: "knowledge",
        order: 13,
        label: "Knowledge base",
    },
];

/// Files of `docs/` that are not mirrored: the experiment log (a 1900-line page that names the
/// author's machine in run directories and journal lines) stays on GitHub.
pub const EXCLUDED_FILES: [&str; 1] = ["TEST_LOG.md"];

/// File-name prefixes that are not mirrored: the release notes are linked from the changelog page.
pub const EXCLUDED_PREFIXES: [&str; 1] = ["RELEASE_NOTES_"];

/// Where the pages are served.
const SITE_BASE: &str = "/spawn-at/project/";
/// Where files of the repository can be read.
const GITHUB_BLOB: &str = "https://github.com/harshp2008/spawn-at/blob/main/";

fn is_excluded(file: &str) -> bool {
    EXCLUDED_FILES.contains(&file) || EXCLUDED_PREFIXES.iter().any(|p| file.starts_with(p))
}

fn entry_for(file: &str) -> Option<&'static Entry> {
    MIRRORED.iter().find(|e| e.file == file)
}

/// Problems with the list against the `.md` file names present in `docs/`: a file that is neither
/// mirrored nor excluded, a mirrored file that does not exist, a slug or an order used twice, a
/// slug with a character other than `a-z`, `0-9` and `-`.
pub fn manifest_problems(present: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    for file in present {
        if entry_for(file).is_none() && !is_excluded(file) {
            problems.push(format!(
                "docs/{file}: neither mirrored nor excluded; add it to MIRRORED or \
                 EXCLUDED_FILES in crates/xtask/src/docs_mirror.rs"
            ));
        }
    }
    for entry in &MIRRORED {
        if !present.iter().any(|f| f == entry.file) {
            problems.push(format!(
                "docs/{}: listed in MIRRORED but the file does not exist",
                entry.file
            ));
        }
        if !entry
            .slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            problems.push(format!(
                "slug `{}` has a character other than a-z, 0-9 and -",
                entry.slug
            ));
        }
        if MIRRORED.iter().filter(|e| e.slug == entry.slug).count() > 1 {
            problems.push(format!("slug `{}` is used twice", entry.slug));
        }
        if MIRRORED.iter().filter(|e| e.order == entry.order).count() > 1 {
            problems.push(format!("sidebar order {} is used twice", entry.order));
        }
    }
    problems
}

/// New target for a link found in a mirrored file, or `None` when the link stays as written
/// (an `#anchor`, another site, a site-absolute path). `Err` for a `.md` file of `docs/` that is
/// not in either list.
fn rewrite_target(target: &str) -> Result<Option<String>, String> {
    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with('/')
        || target.starts_with("mailto:")
        || target.contains("://")
    {
        return Ok(None);
    }
    let (path, frag) = match target.find('#') {
        Some(i) => (&target[..i], &target[i..]),
        None => (target, ""),
    };
    if let Some(rest) = path.strip_prefix("../") {
        if rest.starts_with("../") {
            return Err(format!("`{target}` goes up more than one folder"));
        }
        return Ok(Some(format!("{GITHUB_BLOB}{rest}{frag}")));
    }
    if path.contains('/') {
        return Ok(Some(format!("{GITHUB_BLOB}docs/{path}{frag}")));
    }
    if let Some(entry) = entry_for(path) {
        return Ok(Some(format!("{SITE_BASE}{}/{frag}", entry.slug)));
    }
    if is_excluded(path) || !path.ends_with(".md") {
        return Ok(Some(format!("{GITHUB_BLOB}docs/{path}{frag}")));
    }
    Err(format!(
        "`{target}` links to a docs/ file that is neither mirrored nor excluded"
    ))
}

/// `line` with every `](target)` link target rewritten by `rewrite_target`. Inline code is not
/// skipped, because `docs_check::md_links` does not skip it either.
fn rewrite_links(line: &str) -> Result<String, String> {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(i) = rest.find("](") {
        let after = &rest[i + 2..];
        let Some(end) = after.find(')') else { break };
        let target = &after[..end];
        out.push_str(&rest[..i + 2]);
        match rewrite_target(target)? {
            Some(new) => out.push_str(&new),
            None => out.push_str(target),
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    Ok(out)
}

fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// The experiment pages of the site, by the number the docs use for them: `6.12` for the file
/// `6-12-why-revealed-at-pre-resize-size.md`, `6.25a` for `6-25a-window-ping.md`, `6.11.1` for
/// `6-11-1-final-state-oracle.md`. Built at mirror time from the file names in
/// `docs-site/src/content/docs/experiments/`, so a number is linked only while its page exists.
#[derive(Debug, Default)]
pub struct ExperimentMap {
    /// `6.12` -> `6-12-why-revealed-at-pre-resize-size`
    slugs: BTreeMap<String, String>,
    /// Smallest and largest two-digit number among the keys; a `6.NN` outside it is not a mention
    /// of an experiment (`6.85 ms`, `6.4`).
    lo: u32,
    hi: u32,
}

/// What the mirror did with the `6.NN` mentions of one page.
#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub struct LinkStats {
    /// Links made: numbers in the text and `Moved to TEST_LOG` stubs.
    pub linked: usize,
    /// Mentions of a number in range that has no page (`6.25`, `6.30`), left as they were.
    pub unlinked: usize,
}

/// The number the docs use for an experiment file, from its stem: `None` for `index` and `p8-...`.
fn experiment_key(stem: &str) -> Option<String> {
    let rest = stem.strip_prefix("6-")?;
    let b = rest.as_bytes();
    if b.len() < 3 || !b[0].is_ascii_digit() || !b[1].is_ascii_digit() {
        return None;
    }
    let mut key = format!("6.{}", &rest[..2]);
    let mut i = 2;
    if b[i].is_ascii_lowercase() {
        key.push(b[i] as char);
        i += 1;
    }
    if b.get(i) != Some(&b'-') {
        return None;
    }
    i += 1;
    let digits: String = rest[i..].chars().take_while(char::is_ascii_digit).collect();
    if !digits.is_empty() && rest[i + digits.len()..].starts_with('-') {
        key.push('.');
        key.push_str(&digits);
    }
    Some(key)
}

/// The map for the `.md` file names (with extension) found in the experiments folder.
pub fn experiment_map(names: &[String]) -> ExperimentMap {
    let mut map = ExperimentMap {
        slugs: BTreeMap::new(),
        lo: u32::MAX,
        hi: 0,
    };
    for name in names {
        let Some(stem) = name.strip_suffix(".md") else {
            continue;
        };
        let Some(key) = experiment_key(stem) else {
            continue;
        };
        let nn: u32 = key[2..4].parse().unwrap_or(0);
        map.lo = map.lo.min(nn);
        map.hi = map.hi.max(nn);
        map.slugs.insert(key, stem.to_string());
    }
    map
}

/// `.md` names in `dir`; empty when the folder does not exist (a unit test, or a checkout
/// without the site).
fn experiment_names(dir: &Path) -> Vec<String> {
    md_names(dir).unwrap_or_default()
}

impl ExperimentMap {
    fn url(&self, key: &str) -> Option<String> {
        self.slugs
            .get(key)
            .map(|slug| format!("/spawn-at/experiments/{slug}/"))
    }

    /// Letters used as a suffix in a key (`a`, `b`, `c` for `6.25a`).
    fn has_letter(&self, c: char) -> bool {
        self.slugs
            .keys()
            .any(|k| k.chars().nth(4).is_some_and(|x| x == c))
    }
}

/// A `6.NN` mention starting at `cs[i]`: where it ends and its key (`6.12`, `6.25a`, `6.11.1`),
/// or `None` when the text there is not one. Not a mention: a `6` after a digit, `.`, `v`, `_` or
/// a letter (`0.6.12`, `16.12`, `v6.12`, `x6.12`); a number of three digits (`6.123`); a
/// number outside the range of the experiment pages (`6.4`, `6.85`); a number followed by a
/// letter or digit that is not a page suffix (`6.12ms`); and a number followed by a unit (`6.12 ms`,
/// `6.12px`, `6.12 %`), which is a measurement.
fn mention_at(cs: &[char], i: usize, map: &ExperimentMap) -> Option<(usize, String)> {
    if cs.get(i) != Some(&'6') || cs.get(i + 1) != Some(&'.') {
        return None;
    }
    if i > 0 {
        let p = cs[i - 1];
        if p.is_ascii_digit() || p == '.' || p == '_' || p.is_alphabetic() {
            return None;
        }
    }
    let (d1, d2) = (*cs.get(i + 2)?, *cs.get(i + 3)?);
    if !d1.is_ascii_digit() || !d2.is_ascii_digit() {
        return None;
    }
    let nn = d1.to_digit(10)? * 10 + d2.to_digit(10)?;
    if map.slugs.is_empty() || nn < map.lo || nn > map.hi {
        return None;
    }
    let mut end = i + 4;
    let mut key = format!("6.{d1}{d2}");
    if let Some(&c) = cs.get(end) {
        if c.is_ascii_digit() {
            return None; // three digits
        }
        if c.is_ascii_lowercase() && map.has_letter(c) {
            key.push(c);
            end += 1;
        }
    }
    if cs.get(end) == Some(&'.') && cs.get(end + 1).is_some_and(char::is_ascii_digit) {
        key.push('.');
        end += 1;
        while let Some(&c) = cs.get(end) {
            if !c.is_ascii_digit() {
                break;
            }
            key.push(c);
            end += 1;
        }
    }
    if cs
        .get(end)
        .is_some_and(|c| c.is_alphanumeric() || *c == '_')
    {
        return None;
    }
    let mut u = end;
    if cs.get(u) == Some(&' ') {
        u += 1;
    }
    let tail: String = cs[u.min(cs.len())..].iter().take(3).collect();
    let unit = ["ms", "px", "Hz", "s", "%"].iter().any(|unit| {
        tail.starts_with(unit)
            && !tail
                .chars()
                .nth(unit.chars().count())
                .is_some_and(char::is_alphanumeric)
    });
    if unit {
        return None;
    }
    Some((end, key))
}

/// `line` with every `6.NN` mention that has a page turned into a link to that page; counts
/// the links made and the mentions left because no page has that number. The text of code spans,
/// of existing links (text and target), of `<...>` tags and autolinks and of bare URLs is never
/// touched. The caller does not pass headings or fenced lines.
fn link_experiments(line: &str, map: &ExperimentMap, stats: &mut LinkStats) -> String {
    let cs: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len() + 32);
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '`' {
            let n = cs[i..].iter().take_while(|&&x| x == '`').count();
            let mut j = i + n;
            let mut close = None;
            while j < cs.len() {
                if cs[j] == '`' {
                    let m = cs[j..].iter().take_while(|&&x| x == '`').count();
                    if m == n {
                        close = Some(j + m);
                        break;
                    }
                    j += m;
                } else {
                    j += 1;
                }
            }
            let stop = close.unwrap_or(i + n);
            out.extend(&cs[i..stop]);
            i = stop;
            continue;
        }
        if c == '[' {
            if let Some(k) = cs[i..].iter().position(|&x| x == ']').map(|k| i + k) {
                if cs.get(k + 1) == Some(&'(') {
                    if let Some(e) = cs[k..].iter().position(|&x| x == ')').map(|e| k + e) {
                        out.extend(&cs[i..=e]);
                        i = e + 1;
                        continue;
                    }
                }
            }
        }
        if c == '<' {
            if let Some(e) = cs[i..].iter().position(|&x| x == '>').map(|e| i + e) {
                out.extend(&cs[i..=e]);
                i = e + 1;
                continue;
            }
        }
        let rest: String = cs[i..].iter().take(8).collect();
        if rest.starts_with("http://") || rest.starts_with("https://") {
            let e = cs[i..]
                .iter()
                .position(|x| x.is_whitespace() || *x == ')' || *x == '>')
                .map_or(cs.len(), |e| i + e);
            out.extend(&cs[i..e]);
            i = e;
            continue;
        }
        if let Some((end, key)) = mention_at(&cs, i, map) {
            if let Some(url) = map.url(&key) {
                out.push_str(&format!("[{key}]({url})"));
                stats.linked += 1;
            } else {
                out.extend(&cs[i..end]);
                stats.unlinked += 1;
            }
            i = end;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// The number at the start of a heading's text (`### 6.12 Why ...` gives `6.12`), if it is a
/// mention in the sense of `mention_at`.
fn heading_key(line: &str, map: &ExperimentMap) -> Option<String> {
    let text = line.trim_start().trim_start_matches('#');
    if text.len() == line.trim_start().len() || !text.starts_with(' ') {
        return None;
    }
    let cs: Vec<char> = text.trim_start().chars().collect();
    mention_at(&cs, 0, map).map(|(_, key)| key)
}

/// The `## Sources` section of a mirrored page: its source file and the GitHub URL, which ends in
/// `.md` and so is written with `://` (the site check allows that for other sites).
fn sources_section(file: &str) -> String {
    format!(
        "\n## Sources\n\n- [`docs/{file}`]({GITHUB_BLOB}docs/{file}) in the repository. This page \
         is generated from that file when the site is built; edit that file, not this page.\n"
    )
}

/// True for a markdown heading line: one to six `#` and a space.
fn is_heading(line: &str) -> bool {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|&c| c == '#').count();
    (1..=6).contains(&hashes) && t[hashes..].starts_with(' ')
}

/// The page for `docs/<entry.file>` whose text is `text`, without links to experiment pages.
/// Used by the tests, which do not need the experiments folder; `run` calls `render_linked`.
#[cfg(test)]
pub fn render(entry: &Entry, text: &str) -> Result<String, String> {
    render_linked(entry, text, &ExperimentMap::default()).map(|(page, _)| page)
}

/// The page for `docs/<entry.file>` and what the mirror did with its `6.NN` mentions. Outside code
/// fences, headings, code spans and existing links, a `6.NN` that has an experiment page in `map`
/// becomes a link to it, and the "Moved to TEST_LOG.md" line under a `### 6.NN` heading of
/// `TEST_PLAN.md` gets a second sentence with the link to the page on this site.
pub fn render_linked(
    entry: &Entry,
    text: &str,
    map: &ExperimentMap,
) -> Result<(String, LinkStats), String> {
    let mut stats = LinkStats::default();
    let mut current_heading: Option<String> = None;
    let mut title = None;
    let mut body = String::new();
    let mut in_fence = false;
    for line in text.lines() {
        if is_fence(line) {
            in_fence = !in_fence;
            body.push_str(line);
            body.push('\n');
            continue;
        }
        if in_fence {
            body.push_str(line);
            body.push('\n');
            continue;
        }
        if title.is_none() {
            if let Some(t) = line.strip_prefix("# ") {
                title = Some(t.trim().to_string());
                continue;
            }
        }
        if line.trim_end() == "## Sources" {
            return Err(format!(
                "docs/{}: already has a `## Sources` heading; the mirror appends one",
                entry.file
            ));
        }
        let rewritten = rewrite_links(line).map_err(|e| format!("docs/{}: {e}", entry.file))?;
        if is_heading(line) {
            current_heading = heading_key(line, map);
            body.push_str(&rewritten);
        } else if rewritten.starts_with("Moved to [TEST_LOG.md](") && current_heading.is_some() {
            // the stub under a `### 6.NN` heading: the experiment log itself stays on GitHub, the
            // page of the same experiment on this site is added
            let key = current_heading.as_deref().unwrap_or_default();
            match (map.url(key), rewritten.strip_suffix('.')) {
                (Some(url), Some(head)) => {
                    body.push_str(&format!(
                        "{head}. Also on this site: [experiment {key}]({url})."
                    ));
                    stats.linked += 1;
                }
                _ => {
                    body.push_str(&rewritten);
                    stats.unlinked += 1;
                }
            }
        } else {
            body.push_str(&link_experiments(&rewritten, map, &mut stats));
        }
        body.push('\n');
    }
    let title =
        title.ok_or_else(|| format!("docs/{}: no `# ` heading to use as title", entry.file))?;
    let mut page = format!(
        "---\ntitle: {}\nsidebar:\n  order: {}\n  label: {}\n---\n\n",
        yaml_quote(&title),
        entry.order,
        yaml_quote(entry.label)
    );
    page.push_str(body.trim_start_matches('\n'));
    page.push_str(&sources_section(entry.file));
    debug_assert!(has_sources_list(&page));
    Ok((page, stats))
}

/// `.md` file names directly inside `dir`, sorted.
fn md_names(dir: &Path) -> Result<Vec<String>, String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {}", dir.display(), e))?;
    let mut names = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| format!("cannot read {}: {}", dir.display(), e))?
            .path();
        if path.is_file() && path.extension().is_some_and(|e| e == "md") {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Removes `*.md` files of `dir` that are not in `keep`: pages of a file that is no longer
/// mirrored. `index.md` (hand-written) and files of any other type are left alone.
fn remove_stale_pages(dir: &Path, keep: &[PathBuf]) -> Result<(), String> {
    for name in md_names(dir)? {
        let path = dir.join(&name);
        if name != "index.md" && !keep.contains(&path) {
            fs::remove_file(&path)
                .map_err(|e| format!("cannot remove {}: {}", path.display(), e))?;
        }
    }
    Ok(())
}

/// The files `generate_linked` wrote and, for each `docs/` file, what it did with the `6.NN`
/// mentions.
pub type Generated = (Vec<PathBuf>, Vec<(&'static str, LinkStats)>);

/// `generate_linked` without links to experiment pages. Used by the tests; `run` calls
/// `generate_linked`.
#[cfg(test)]
pub fn generate(
    docs_dir: &Path,
    out_dir: &Path,
    sidebar_config: &str,
) -> Result<Vec<PathBuf>, String> {
    generate_linked(docs_dir, out_dir, sidebar_config, None).map(|(written, _)| written)
}

/// Writes one page per `MIRRORED` entry into `out_dir` and removes stale ones. Fails, writing
/// nothing, if the list does not match `docs_dir`, if the sidebar in `sidebar_config` has no
/// `autogenerate` entry for `project`, or if a file cannot be turned into a page. With
/// `experiments_dir` (the folder of the experiment pages), `6.NN` mentions are linked to those
/// pages; the result also holds the counts per file.
pub fn generate_linked(
    docs_dir: &Path,
    out_dir: &Path,
    sidebar_config: &str,
    experiments_dir: Option<&Path>,
) -> Result<Generated, String> {
    let map = experiment_map(&experiments_dir.map(experiment_names).unwrap_or_default());
    let mut all_stats = Vec::new();
    let mut problems = manifest_problems(&md_names(docs_dir)?);
    if !sidebar_autogenerates(sidebar_config, "project") {
        problems.push(
            "the sidebar in astro.config.mjs has no `autogenerate: { directory: 'project' }` \
             entry, so the mirrored pages would not be in the menu"
                .to_string(),
        );
    }
    let mut pages = Vec::new();
    if problems.is_empty() {
        for entry in &MIRRORED {
            let path = docs_dir.join(entry.file);
            let text = fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
            match render_linked(entry, &text, &map) {
                Ok((page, stats)) => {
                    pages.push((out_dir.join(format!("{}.md", entry.slug)), page));
                    all_stats.push((entry.file, stats));
                }
                Err(e) => problems.push(e),
            }
        }
    }
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    fs::create_dir_all(out_dir)
        .map_err(|e| format!("cannot create {}: {}", out_dir.display(), e))?;
    let mut written = Vec::new();
    for (path, page) in pages {
        fs::write(&path, page).map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
        written.push(path);
    }
    remove_stale_pages(out_dir, &written)?;
    Ok((written, all_stats))
}

/// Entry point of `cargo xtask docs-mirror [--docs DIR] [--out DIR] [--config FILE]
/// [--experiments DIR]` (defaults `docs`, `docs-site/src/content/docs/project`,
/// `docs-site/astro.config.mjs` and `docs-site/src/content/docs/experiments`).
pub fn run(args: &[String]) -> Result<(), String> {
    let mut docs = PathBuf::from("docs");
    let mut out = PathBuf::from("docs-site/src/content/docs/project");
    let mut config = PathBuf::from("docs-site/astro.config.mjs");
    let mut experiments = PathBuf::from("docs-site/src/content/docs/experiments");
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--docs" => docs = PathBuf::from(value),
            "--out" => out = PathBuf::from(value),
            "--config" => config = PathBuf::from(value),
            "--experiments" => experiments = PathBuf::from(value),
            other => return Err(format!("unknown option {other}")),
        }
    }
    let config_text = fs::read_to_string(&config)
        .map_err(|e| format!("cannot read {}: {}", config.display(), e))?;
    let (written, stats) = generate_linked(&docs, &out, &config_text, Some(&experiments))?;
    println!(
        "==> wrote {} project pages to {}",
        written.len(),
        out.display()
    );
    let (mut linked, mut unlinked) = (0, 0);
    for (file, s) in &stats {
        if s.linked + s.unlinked > 0 {
            println!(
                "    {file}: {} links to experiment pages, {} mentions left unlinked (no page)",
                s.linked, s.unlinked
            );
        }
        linked += s.linked;
        unlinked += s.unlinked;
    }
    println!("    total: {linked} links to experiment pages, {unlinked} mentions left unlinked");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs_check::{forbidden_in, has_h1, md_links, split_front_matter};

    const CONFIG: &str = "items: [{ autogenerate: { directory: 'project' } }]";

    fn temp(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("xtask-docs-mirror-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn entry(file: &str) -> &'static Entry {
        entry_for(file).unwrap()
    }

    /// Names of every file the list covers, as `docs/` would hold them, plus the excluded ones.
    fn all_names() -> Vec<String> {
        let mut names: Vec<String> = MIRRORED.iter().map(|e| e.file.to_string()).collect();
        names.push("TEST_LOG.md".to_string());
        names.push("RELEASE_NOTES_0.2.1.md".to_string());
        names
    }

    /// A title with `&`, backticks, a double quote and a backslash stays one YAML scalar: the
    /// quote and the backslash are escaped, `&` and the backticks are kept. Unquoted, `&` would
    /// start a YAML anchor and the page would lose its title.
    #[test]
    fn test_title_is_yaml_quoted() {
        let page = render(
            entry("CONFIG.md"),
            "# Tests & `config` \"a\" \\ b\n\nText\n",
        )
        .unwrap();
        assert!(
            page.starts_with(
                "---\ntitle: \"Tests & `config` \\\"a\\\" \\\\ b\"\nsidebar:\n  order: 3\n  label: \"Settings\"\n---\n\nText\n"
            ),
            "{page}"
        );
        assert!(!page.contains("\n# "), "the H1 is removed: {page}");
    }

    /// The sidebar label is its own front-matter key under `sidebar:`; the page title keeps
    /// following the heading, so a long heading gets a short label and the page is still
    /// called by its heading.
    #[test]
    fn test_sidebar_label_is_separate_from_the_title() {
        let page = render(
            entry("legacy-gnome.md"),
            "# Legacy GNOME (42–44) Differences & Compatibility Analysis\n\nText\n",
        )
        .unwrap();
        let (front, _) = split_front_matter(&page).unwrap();
        assert!(
            front.contains("title: \"Legacy GNOME (42–44) Differences & Compatibility Analysis\""),
            "{front}"
        );
        assert!(
            front.contains("sidebar:\n  order: 4\n  label: \"Legacy GNOME (42–44)\""),
            "{front}"
        );
        assert_eq!(
            crate::docs_check::front_matter_title(front),
            "Legacy GNOME (42–44) Differences & Compatibility Analysis"
        );
    }

    /// Every label is filled in, starts with a capital letter, has no space at either end, is at
    /// most 24 characters (a sidebar line at the narrowest width of the desktop layout) and is
    /// used once, so two pages cannot look the same in the menu.
    #[test]
    fn test_labels_are_short_and_distinct() {
        let mut seen: Vec<&str> = Vec::new();
        for e in &MIRRORED {
            let label = e.label;
            assert!(!label.is_empty(), "{}: empty label", e.file);
            assert_eq!(label, label.trim(), "{}: spaces around the label", e.file);
            assert!(
                label.chars().next().is_some_and(char::is_uppercase),
                "{}: `{label}` does not start with a capital",
                e.file
            );
            assert!(
                label.chars().count() <= 24,
                "{}: `{label}` is longer than 24 characters",
                e.file
            );
            assert!(!seen.contains(&label), "label `{label}` is used twice");
            seen.push(label);
        }
    }

    /// A page without a `# ` heading outside a fence cannot get a title; a `# ` line inside a
    /// fence (a shell comment) is not one.
    #[test]
    fn test_title_comes_from_a_heading_outside_fences() {
        assert!(render(entry("CONFIG.md"), "text\n```\n# comment\n```\n").is_err());
        let page = render(entry("CONFIG.md"), "# T\n```\n# comment\n```\n").unwrap();
        assert!(page.contains("```\n# comment\n```"), "{page}");
    }

    /// Sibling links go to `/spawn-at/project/<slug>/` with the fragment kept; the same text in
    /// a fence is untouched, as is an in-page anchor and a link to another site.
    #[test]
    fn test_links_are_rewritten_outside_fences_only() {
        let text = "# T\n\nSee [sway](DESIGN_SWAY.md#4-x) and [self](#top) and \
                    [web](https://example.com/a.md).\n\n```text\n[sway](DESIGN_SWAY.md)\n```\n";
        let page = render(entry("CONFIG.md"), text).unwrap();
        assert!(
            page.contains("[sway](/spawn-at/project/design-sway/#4-x) and [self](#top) and [web](https://example.com/a.md)"),
            "{page}"
        );
        assert!(
            page.contains("```text\n[sway](DESIGN_SWAY.md)\n```"),
            "{page}"
        );
        let (_, body) = split_front_matter(&page).unwrap();
        assert_eq!(md_links(body), Vec::<String>::new());
    }

    /// A link to an excluded docs file (the experiment log, release notes) or to a file outside
    /// `docs/` becomes a GitHub URL with its fragment, because the site has no page for it.
    #[test]
    fn test_links_to_excluded_and_parent_files_go_to_github() {
        let text = "# T\n\n[log](TEST_LOG.md#610-x) [notes](RELEASE_NOTES_0.2.0.md) \
                    [cl](../CHANGELOG.md) [v](../verify/README.md) [s](../scripts/sway-headless.sh)\n";
        let page = render(entry("CONFIG.md"), text).unwrap();
        for expected in [
            "(https://github.com/harshp2008/spawn-at/blob/main/docs/TEST_LOG.md#610-x)",
            "(https://github.com/harshp2008/spawn-at/blob/main/docs/RELEASE_NOTES_0.2.0.md)",
            "(https://github.com/harshp2008/spawn-at/blob/main/CHANGELOG.md)",
            "(https://github.com/harshp2008/spawn-at/blob/main/verify/README.md)",
            "(https://github.com/harshp2008/spawn-at/blob/main/scripts/sway-headless.sh)",
        ] {
            assert!(page.contains(expected), "missing {expected} in:\n{page}");
        }
    }

    /// A link to a docs file that the lists do not know is an error, not a dead link.
    #[test]
    fn test_link_to_an_unlisted_docs_file_is_an_error() {
        let err = render(entry("CONFIG.md"), "# T\n\n[x](NEW_DOC.md)\n").unwrap_err();
        assert!(err.contains("NEW_DOC.md"), "{err}");
    }

    /// The appended Sources section names the file and its GitHub URL, passes the site rule, and
    /// a file that already has its own `## Sources` is refused rather than given two.
    #[test]
    fn test_sources_section_is_appended_once() {
        let page = render(entry("KNOWLEDGE.md"), "# T\n\nText\n").unwrap();
        assert!(
            page.ends_with(
                "- [`docs/KNOWLEDGE.md`](https://github.com/harshp2008/spawn-at/blob/main/docs/KNOWLEDGE.md) in the repository. This page is generated from that file when the site is built; edit that file, not this page.\n"
            ),
            "{page}"
        );
        assert!(has_sources_list(&page));
        assert!(render(entry("KNOWLEDGE.md"), "# T\n\n## Sources\n\n- x\n").is_err());
    }

    /// The complete set of names passes; an unlisted file, a missing mirrored file and a
    /// duplicate are each reported.
    #[test]
    fn test_manifest_completeness() {
        assert_eq!(manifest_problems(&all_names()), Vec::<String>::new());

        let mut with_new = all_names();
        with_new.push("NEW_DOC.md".to_string());
        let problems = manifest_problems(&with_new).join("\n");
        assert!(
            problems.contains("docs/NEW_DOC.md: neither mirrored nor excluded"),
            "{problems}"
        );

        let without_one: Vec<String> = all_names()
            .into_iter()
            .filter(|n| n != "TEST_PLAN.md")
            .collect();
        let problems = manifest_problems(&without_one).join("\n");
        assert!(
            problems.contains("docs/TEST_PLAN.md: listed in MIRRORED but the file does not exist"),
            "{problems}"
        );
    }

    /// The list itself has no repeated slug or order and only `a-z0-9-` in slugs (checked by
    /// `manifest_problems` over the full set of names).
    #[test]
    fn test_manifest_has_unique_slugs_and_orders() {
        let mut slugs: Vec<&str> = MIRRORED.iter().map(|e| e.slug).collect();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), MIRRORED.len());
        let mut orders: Vec<u32> = MIRRORED.iter().map(|e| e.order).collect();
        orders.sort_unstable();
        assert_eq!(orders, (1..=MIRRORED.len() as u32).collect::<Vec<_>>());
    }

    /// `generate` writes one page per entry, then a later run with a stale page in the folder
    /// removes it, keeps the hand-written `index.md` and leaves a non-markdown file alone.
    #[test]
    fn test_generate_writes_pages_and_removes_stale_ones() {
        let base = temp("generate");
        let docs = base.join("docs");
        let out = base.join("project");
        fs::create_dir_all(&docs).unwrap();
        fs::create_dir_all(&out).unwrap();
        for name in all_names() {
            fs::write(docs.join(&name), "# Title\n\nText\n").unwrap();
        }
        fs::write(out.join("index.md"), "hand-written").unwrap();
        fs::write(out.join("old-page.md"), "stale").unwrap();
        fs::write(out.join("notes.txt"), "other").unwrap();

        let written = generate(&docs, &out, CONFIG).unwrap();
        assert_eq!(written.len(), 13);
        assert!(out.join("test-plan.md").is_file());
        assert!(!out.join("old-page.md").exists(), "stale page removed");
        assert_eq!(
            fs::read_to_string(out.join("index.md")).unwrap(),
            "hand-written"
        );
        assert!(out.join("notes.txt").is_file(), "non-markdown file kept");
        assert!(
            !out.join("test-log.md").exists(),
            "excluded file not mirrored"
        );
        let _ = fs::remove_dir_all(&base);
    }

    /// `generate` writes nothing when `docs/` holds a file the lists do not know, or when the
    /// sidebar has no `autogenerate` entry for `project`.
    #[test]
    fn test_generate_fails_on_unlisted_file_or_missing_sidebar_entry() {
        let base = temp("fails");
        let docs = base.join("docs");
        let out = base.join("project");
        fs::create_dir_all(&docs).unwrap();
        for name in all_names() {
            fs::write(docs.join(&name), "# Title\n\nText\n").unwrap();
        }
        fs::write(docs.join("STRAY.md"), "# Stray\n").unwrap();
        let err = generate(&docs, &out, CONFIG).unwrap_err();
        assert!(
            err.contains("docs/STRAY.md: neither mirrored nor excluded"),
            "{err}"
        );
        assert!(!out.exists(), "nothing written");

        fs::remove_file(docs.join("STRAY.md")).unwrap();
        let err = generate(&docs, &out, "items: ['index']").unwrap_err();
        assert!(err.contains("autogenerate"), "{err}");
        assert!(!out.exists(), "nothing written");
        let _ = fs::remove_dir_all(&base);
    }

    /// `remove_stale_pages` keeps the listed pages and `index.md`, and removes any other `.md`.
    #[test]
    fn test_remove_stale_pages() {
        let dir = temp("stale");
        for name in ["index.md", "keep.md", "gone.md", "other.txt"] {
            fs::write(dir.join(name), "x").unwrap();
        }
        remove_stale_pages(&dir, &[dir.join("keep.md")]).unwrap();
        assert!(dir.join("index.md").is_file());
        assert!(dir.join("keep.md").is_file());
        assert!(!dir.join("gone.md").exists());
        assert!(dir.join("other.txt").is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    /// The real `docs/` folder matches the lists, and every real file renders into a page with
    /// no `# ` heading, no `.md` link, a Sources list and none of the forbidden text. This is the
    /// check that fails in `cargo test` (not only in `docs-site`) when someone adds a `docs/`
    /// file, or writes a path or name into a mirrored file that the site must not publish.
    #[test]
    fn test_real_docs_render_into_clean_pages() {
        let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
        assert_eq!(
            manifest_problems(&md_names(&docs).unwrap()),
            Vec::<String>::new()
        );
        for e in &MIRRORED {
            let text = fs::read_to_string(docs.join(e.file)).unwrap();
            let page = render(e, &text).unwrap_or_else(|err| panic!("{err}"));
            let (front, body) = split_front_matter(&page).unwrap();
            let title = crate::docs_check::front_matter_title(front);
            assert!(!title.is_empty(), "{}", e.file);
            // a label is a short form of the heading, never longer than the title it stands for
            assert!(
                e.label.chars().count() <= title.chars().count(),
                "{}: label `{}` is longer than the title `{title}`",
                e.file,
                e.label
            );
            assert!(!has_h1(body), "{}: has a # heading", e.file);
            assert_eq!(md_links(body), Vec::<String>::new(), "{}", e.file);
            assert!(has_sources_list(body), "{}", e.file);
            assert_eq!(forbidden_in(&page), Vec::<String>::new(), "{}", e.file);
        }
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// Experiment files as the site has them: two ranges of numbers, a lettered group (`6.25a`,
    /// `6.25b` but no `6.25`), a sub-number (`6.11.1`), and files that are not experiments.
    fn map() -> ExperimentMap {
        experiment_map(&names(&[
            "6-10-placement.md",
            "6-11-late-fix.md",
            "6-11-1-oracle.md",
            "6-12-why.md",
            "6-25a-ping.md",
            "6-25b-rig.md",
            "6-35-shim.md",
            "index.md",
            "p8-sway.md",
            "6-notes.txt",
        ]))
    }

    fn link(line: &str) -> (String, LinkStats) {
        let mut stats = LinkStats::default();
        let out = link_experiments(line, &map(), &mut stats);
        (out, stats)
    }

    fn url(slug: &str) -> String {
        format!("/spawn-at/experiments/{slug}/")
    }

    /// The number of a file is read from its name: `6-12-why.md` is `6.12`, `6-25a-ping.md` is
    /// `6.25a`, `6-11-1-oracle.md` is `6.11.1` (not `6.11`, which is `6-11-late-fix.md`);
    /// `index.md`, `p8-sway.md` and a non-markdown file have none, and `6.25` has no page.
    #[test]
    fn test_experiment_numbers_come_from_file_names() {
        let m = map();
        for (key, slug) in [
            ("6.10", "6-10-placement"),
            ("6.11", "6-11-late-fix"),
            ("6.11.1", "6-11-1-oracle"),
            ("6.12", "6-12-why"),
            ("6.25a", "6-25a-ping"),
            ("6.25b", "6-25b-rig"),
            ("6.35", "6-35-shim"),
        ] {
            assert_eq!(m.url(key), Some(url(slug)), "{key}");
        }
        assert_eq!(m.slugs.len(), 7, "{:?}", m.slugs);
        assert_eq!(m.url("6.25"), None);
        assert_eq!((m.lo, m.hi), (10, 35));
        assert_eq!(
            experiment_key("6-21-default-cap-250-wire-paint-time"),
            Some("6.21".into())
        );
        assert_eq!(experiment_key("index"), None);
        assert_eq!(experiment_key("p8-sway-flicker-not-run"), None);
        assert_eq!(experiment_key("6-25abc-x"), None);
    }

    /// The forms the docs use: a bare number, `section 6.NN`, `sections 6.NN to 6.MM` and `6.NN
    /// and 6.MM`, a number in parentheses, a range with a hyphen, a number before a sentence
    /// end, at the start of a line, with a letter, and with a sub-number. Only the number is linked.
    #[test]
    fn test_mentions_with_a_page_become_links() {
        let l = |n: &str, s: &str| format!("[{n}]({})", url(s));
        let cases = [
            (
                "see section 6.10 and 6.12.".to_string(),
                format!(
                    "see section {} and {}.",
                    l("6.10", "6-10-placement"),
                    l("6.12", "6-12-why")
                ),
                2,
            ),
            (
                "Sections 6.10 to 6.35 moved".to_string(),
                format!(
                    "Sections {} to {} moved",
                    l("6.10", "6-10-placement"),
                    l("6.35", "6-35-shim")
                ),
                2,
            ),
            (
                "(`TEST_PLAN.md` 6.11, 6.11.1 and 6.25a)".to_string(),
                format!(
                    "(`TEST_PLAN.md` {}, {} and {})",
                    l("6.11", "6-11-late-fix"),
                    l("6.11.1", "6-11-1-oracle"),
                    l("6.25a", "6-25a-ping")
                ),
                3,
            ),
            (
                "6.10-6.12 measured".to_string(),
                format!(
                    "{}-{} measured",
                    l("6.10", "6-10-placement"),
                    l("6.12", "6-12-why")
                ),
                2,
            ),
            (
                "6.12: the first line".to_string(),
                format!("{}: the first line", l("6.12", "6-12-why")),
                1,
            ),
        ];
        for (input, expected, n) in cases {
            let (out, stats) = link(&input);
            assert_eq!(out, expected, "{input}");
            assert_eq!(
                stats,
                LinkStats {
                    linked: n,
                    unlinked: 0
                },
                "{input}"
            );
        }
    }

    /// Things that look like `6.NN` and are not: a version (`0.6.12`, `v6.12`, `6.12.1`), a number
    /// with more digits before or after (`16.12`, `6.123`, `6.1210`), a letter before or after
    /// (`x6.12`, `6.12ms`), a value with a unit (`6.12 ms`, `6.12px`, `6.12 %`, `6.12 s`), a
    /// number outside the pages (`6.4`, `6.85 ms`) and an identifier (`_6.12`). None is changed;
    /// the ones that are a number of an experiment with a sub-number that has no page are counted.
    #[test]
    fn test_things_that_are_not_mentions_stay_as_they_are() {
        for input in [
            "version 0.6.12 of it",
            "v6.12 and V6.12",
            "x6.12 and a6.12 and _6.12",
            "16.12 and 26.12 and 6.6.12",
            "6.123 and 6.1210",
            "6.12ms and 6.12abc",
            "took 6.12 ms",
            "took 6.12ms and 6.12px and 6.12 %",
            "took 6.12 s.",
            "6.4 and 6.9 and 6.85 ms",
        ] {
            let (out, stats) = link(input);
            assert_eq!(out, input, "{input}");
            assert_eq!(stats, LinkStats::default(), "{input}");
        }
        // 6.12.1 is not 6.12: no page has it, so nothing is linked and the mention is counted
        let (out, stats) = link("see 6.12.1 and 6.10.5");
        assert_eq!(out, "see 6.12.1 and 6.10.5");
        assert_eq!(
            stats,
            LinkStats {
                linked: 0,
                unlinked: 2
            }
        );
        // a word that starts with `s` after the number is not the unit `s`
        let (out, stats) = link("the 6.12 simulation");
        assert!(out.contains("[6.12]("), "{out}");
        assert_eq!(stats.linked, 1);
    }

    /// Text that must not change: code spans (one or two backticks), the text and the target of a
    /// link, an HTML tag, a bare URL. Text next to them is still linked.
    #[test]
    fn test_protected_text_is_never_linked() {
        let l = format!("[6.12]({})", url("6-12-why"));
        for input in [
            "`6.10` is code",
            "``a `6.10` b`` is code",
            "[6.10](https://example.org/x) is a link",
            "[text](https://example.org/6.10) is a link",
            "<a href=\"6.10\">x</a>",
            "https://example.org/docs/6.10 is a url",
            "(https://example.org/6.10)",
            "<https://example.org/6.10>",
        ] {
            let (out, stats) = link(input);
            assert_eq!(out, input, "{input}");
            assert_eq!(stats, LinkStats::default(), "{input}");
        }
        // the tags are protected, the text between them is prose and is linked
        let (out, stats) = link("`6.10` and 6.12 and [6.10](u) and <b>6.10</b>");
        let l10 = format!("[6.10]({})", url("6-10-placement"));
        assert_eq!(
            out,
            format!("`6.10` and {l} and [6.10](u) and <b>{l10}</b>")
        );
        assert_eq!(stats.linked, 2);
    }

    /// A number in range with no page is counted and left as written: `6.20` and `6.30` have no
    /// file, `6.25` has only `6.25a` and `6.25b`.
    #[test]
    fn test_numbers_without_a_page_are_counted_not_linked() {
        let (out, stats) = link("6.20, 6.25 and 6.30");
        assert_eq!(out, "6.20, 6.25 and 6.30");
        assert_eq!(
            stats,
            LinkStats {
                linked: 0,
                unlinked: 3
            }
        );
    }

    /// Whole page: fenced text and headings are untouched, the blockquote and the prose are
    /// linked, and the "Moved to TEST_LOG.md" line under a heading of a number with a page gets the
    /// link to the page on this site, while the one under a number without a page is left alone
    /// and counted.
    #[test]
    fn test_render_links_prose_and_stubs_but_not_fences_or_headings() {
        let text = "# T\n\n> Sections 6.10 to 6.35 moved to [TEST_LOG.md](TEST_LOG.md#x).\n\n\
                    ### 6.10 Placement 6.12\n\n\
                    Moved to [TEST_LOG.md](TEST_LOG.md#610-placement).\n\n\
                    ### 6.20 No page here\n\n\
                    Moved to [TEST_LOG.md](TEST_LOG.md#620-no-page).\n\n\
                    ```text\nsee 6.10 in a fence\n```\n\n\
                    Prose with 6.12 and `6.12`.\n";
        let (page, stats) = render_linked(entry("TEST_PLAN.md"), text, &map()).unwrap();
        let l10 = format!("[6.10]({})", url("6-10-placement"));
        let l35 = format!("[6.35]({})", url("6-35-shim"));
        assert!(
            page.contains(&format!(
                "> Sections {l10} to {l35} moved to [TEST_LOG.md]("
            )),
            "{page}"
        );
        assert!(
            page.contains("\n### 6.10 Placement 6.12\n"),
            "heading unchanged: {page}"
        );
        assert!(
            page.contains(&format!(
                "docs/TEST_LOG.md#610-placement). Also on this site: [experiment 6.10]({}).\n",
                url("6-10-placement")
            )),
            "{page}"
        );
        assert!(page.contains("\n### 6.20 No page here\n\nMoved to [TEST_LOG.md](https://github.com/harshp2008/spawn-at/blob/main/docs/TEST_LOG.md#620-no-page).\n"), "{page}");
        assert!(page.contains("```text\nsee 6.10 in a fence\n```"), "{page}");
        assert!(
            page.contains(&format!(
                "Prose with [6.12]({}) and `6.12`.",
                url("6-12-why")
            )),
            "{page}"
        );
        // 6.10 and 6.35 in the quote, the 6.10 stub, 6.12 in the prose: 4 links; the 6.20 stub: 1
        assert_eq!(
            stats,
            LinkStats {
                linked: 4,
                unlinked: 1
            }
        );
        // the page still passes the site rules: no `.md` link, no `# ` heading, no forbidden text
        let (_, body) = split_front_matter(&page).unwrap();
        assert_eq!(md_links(body), Vec::<String>::new());
        assert_eq!(forbidden_in(&page), Vec::<String>::new());
    }

    /// With no experiment pages (a checkout without the site) nothing is linked and nothing is
    /// counted, whatever the text says.
    #[test]
    fn test_no_experiment_pages_means_no_links() {
        let none = experiment_map(&[]);
        let (page, stats) =
            render_linked(entry("TEST_PLAN.md"), "# T\n\nSee 6.12 and 6.10.\n", &none).unwrap();
        assert!(page.contains("See 6.12 and 6.10."), "{page}");
        assert_eq!(stats, LinkStats::default());
    }

    /// `generate_linked` reads the file names of the experiments folder at run time, links to
    /// them, and returns the counts per file; `generate` (no folder) links nothing.
    #[test]
    fn test_generate_linked_uses_the_experiments_folder() {
        let base = temp("linked");
        let (docs, out, exp) = (
            base.join("docs"),
            base.join("project"),
            base.join("experiments"),
        );
        for d in [&docs, &exp] {
            fs::create_dir_all(d).unwrap();
        }
        for name in all_names() {
            fs::write(docs.join(&name), "# Title\n\nText\n").unwrap();
        }
        fs::write(docs.join("TEST_PLAN.md"), "# Title\n\nSee 6.12 and 6.33.\n").unwrap();
        fs::write(exp.join("6-12-why.md"), "x").unwrap();
        fs::write(exp.join("6-33-sync.md"), "x").unwrap();
        let (written, stats) = generate_linked(&docs, &out, CONFIG, Some(&exp)).unwrap();
        assert_eq!(written.len(), 13);
        let plan = fs::read_to_string(out.join("test-plan.md")).unwrap();
        assert!(plan.contains("See [6.12](/spawn-at/experiments/6-12-why/) and [6.33](/spawn-at/experiments/6-33-sync/)."), "{plan}");
        let plan_stats = stats.iter().find(|(f, _)| *f == "TEST_PLAN.md").unwrap().1;
        assert_eq!(
            plan_stats,
            LinkStats {
                linked: 2,
                unlinked: 0
            }
        );
        generate(&docs, &out, CONFIG).unwrap();
        let plain = fs::read_to_string(out.join("test-plan.md")).unwrap();
        assert!(plain.contains("See 6.12 and 6.33."), "{plain}");
        let _ = fs::remove_dir_all(&base);
    }

    /// The real documents and the real experiment folder: every link made points to a file that
    /// exists, headings and fences keep their text, and the pages still pass the site rules.
    #[test]
    fn test_real_docs_link_only_to_experiment_pages_that_exist() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let exp = root.join("docs-site/src/content/docs/experiments");
        let map = experiment_map(&experiment_names(&exp));
        assert!(
            map.slugs.len() >= 25,
            "{} experiment pages",
            map.slugs.len()
        );
        let mut total = 0;
        for e in &MIRRORED {
            let text = fs::read_to_string(root.join("docs").join(e.file)).unwrap();
            let (page, stats) = render_linked(e, &text, &map).unwrap();
            total += stats.linked;
            let mut rest = page.as_str();
            while let Some(i) = rest.find("](/spawn-at/experiments/") {
                let after = &rest[i + "](/spawn-at/experiments/".len()..];
                let slug = &after[..after.find("/)").unwrap()];
                assert!(
                    exp.join(format!("{slug}.md")).is_file(),
                    "{}: {slug}",
                    e.file
                );
                rest = after;
            }
            for line in page.lines().filter(|l| l.starts_with('#')) {
                assert!(
                    !line.contains("](/spawn-at/experiments/"),
                    "{}: {line}",
                    e.file
                );
            }
            let (_, body) = split_front_matter(&page).unwrap();
            assert_eq!(md_links(body), Vec::<String>::new(), "{}", e.file);
            assert_eq!(forbidden_in(&page), Vec::<String>::new(), "{}", e.file);
        }
        assert!(total >= 100, "only {total} links were made");
    }
}
