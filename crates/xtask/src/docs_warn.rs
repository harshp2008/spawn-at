//! Readability warnings of `docs-check-src`. They never fail the run: `docs_check::run` prints one
//! summary line per rule and the exit code stays what the errors decide.
//!
//! Each rule is a number measured from the Markdown source of a page and a threshold taken from the
//! audit of the guide (rules W1 to W16). The measure is a count over the source, not over the
//! rendered page, so a number here can differ by a few words from one read off the HTML. Not
//! implemented: W4 (paragraphs taller than 4 lines at 1440 px) needs a browser to lay the page out.
//!
//! Three kinds of page: *hand* (written by hand under the content folder), *project* (mirrored from
//! `docs/` into `project/`, which the thresholds treat more loosely) and *exempt* (the experiment
//! records, which are evidence, and the generated CLI reference). Exempt pages are not measured.
//! Text in code fences, the `## Sources` list and HTML comments is left out of every word count.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use crate::docs_check::{markdown_files, split_front_matter};

/// Which thresholds apply to a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Hand,
    Project,
    Exempt,
}

/// The kind of the page at `rel` (path under the content folder, `/` separated).
pub fn kind_of(rel: &str) -> Kind {
    let experiment_record = rel.starts_with("experiments/") && rel != "experiments/index.md";
    let generated = rel.starts_with("reference/") && rel != "reference/index.md";
    if experiment_record || generated {
        Kind::Exempt
    } else if rel.starts_with("project/") && rel != "project/index.md" {
        Kind::Project
    } else {
        Kind::Hand
    }
}

/// One thing a rule found on a page.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub rule: &'static str,
    pub page: String,
    /// Line in the page file, 0 when the finding is about the whole page.
    pub line: usize,
    /// The measured number, which orders the findings of a rule from worst to least.
    pub value: f64,
    /// What the summary prints for the page: `value` with a unit where the rule has two
    /// measures (W3, W6, W10, W12).
    pub shown: String,
    pub text: String,
}

/// What each rule is about, for the summary line.
pub const RULES: [(&str, &str); 15] = [
    ("W1", "paragraph over 100 words (hand) or 150 (project)"),
    ("W2", "list item over 60 words (hand) or 100 (project)"),
    (
        "W3",
        "sentence over 45 words, or over 10% of sentences over 25 words",
    ),
    ("W5", "page of 300 words or more with no bold"),
    (
        "W6",
        "hand page over 400 words with under 4 or over 25 bold spans per 1000 words",
    ),
    (
        "W7",
        "more than half of more than 8 bold spans are 1 or 2 word labels",
    ),
    (
        "W8",
        "bold span over 12 words, or 2 bold spans in over 3 paragraphs",
    ),
    (
        "W9",
        "over 10 code spans per 100 words (hand) or 12 (project)",
    ),
    (
        "W10",
        "page over 800 words with under 3 headings per 1000 words, or over 20 h3",
    ),
    ("W11", "page over 2500 words (hand) or 6000 (project)"),
    ("W12", "Sources over 12% of the page words, or over 8 items"),
    ("W13", "`file:line` reference in the prose of a hand page"),
    (
        "W14",
        "repeated paragraph or list item, or a second Status line",
    ),
    (
        "W15",
        "mirrored page whose first paragraph is not `**Status:**`",
    ),
    ("W16", "table cell over 350 characters"),
];

/// A block of the page body.
#[derive(Debug, PartialEq)]
enum Block {
    Heading { level: usize },
    Paragraph { line: usize, text: String },
    Item { line: usize, text: String },
    Cell { line: usize, text: String },
}

/// What the scan of a page body found: its blocks and the Sources list.
#[derive(Debug, Default)]
struct Body {
    blocks: Vec<Block>,
    sources_words: usize,
    sources_items: usize,
    /// Body lines (1-based) at column 0 that start with `Status:` or `**Status:**`.
    status_lines: Vec<usize>,
}

fn marker_len(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    for m in ["- ", "* ", "+ "] {
        if trimmed.starts_with(m) {
            return Some(indent + m.len());
        }
    }
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 && digits < 4 {
        let rest = &trimmed[digits..];
        if rest.starts_with(". ") || rest.starts_with(") ") {
            return Some(indent + digits + 2);
        }
    }
    None
}

fn table_cells(line: &str) -> Option<Vec<String>> {
    let t = line.trim();
    if !t.starts_with('|') {
        return None;
    }
    let inner = t.trim_start_matches('|');
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let cells: Vec<String> = inner
        .replace("\\|", "\u{1}")
        .split('|')
        .map(|c| c.replace('\u{1}', "|").trim().to_string())
        .collect();
    let separator = cells
        .iter()
        .all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')));
    // A separator row (`| :-- | --: |`) is part of the table and holds no text.
    Some(if separator { Vec::new() } else { cells })
}

/// Scans the body of a page (the text after the front matter). Line numbers start at 1 for the
/// first line of `body`.
fn scan(body: &str) -> Body {
    let mut out = Body::default();
    let mut in_fence = false;
    let mut in_sources = false;
    let mut cur: Option<(bool, usize, String)> = None; // (is item, line, text)
    let mut blank_before = false;
    let flush = |cur: &mut Option<(bool, usize, String)>, blocks: &mut Vec<Block>| {
        if let Some((item, line, text)) = cur.take() {
            blocks.push(if item {
                Block::Item { line, text }
            } else {
                Block::Paragraph { line, text }
            });
        }
    };
    for (i, line) in body.lines().enumerate() {
        let n = i + 1;
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            flush(&mut cur, &mut out.blocks);
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if t.is_empty() {
            blank_before = true;
            continue;
        }
        let after_blank = std::mem::take(&mut blank_before);
        if line.starts_with("**Status:**") || line.starts_with("Status:") {
            out.status_lines.push(n);
        }
        if t.starts_with("<!--") || t.starts_with(":::") {
            flush(&mut cur, &mut out.blocks);
            continue;
        }
        if line.starts_with('#') {
            flush(&mut cur, &mut out.blocks);
            let level = line.chars().take_while(|&c| c == '#').count();
            in_sources = level == 2 && line[level..].trim() == "Sources";
            if !in_sources {
                out.blocks.push(Block::Heading { level });
            }
            continue;
        }
        if in_sources {
            if marker_len(line).is_some() {
                out.sources_items += 1;
            }
            out.sources_words += plain(line).split_whitespace().count();
            continue;
        }
        if let Some(cells) = table_cells(line) {
            flush(&mut cur, &mut out.blocks);
            for text in cells {
                out.blocks.push(Block::Cell { line: n, text });
            }
            continue;
        }
        if let Some(m) = marker_len(line) {
            flush(&mut cur, &mut out.blocks);
            cur = Some((true, n, line[m..].to_string()));
            continue;
        }
        let continues_item = matches!(cur, Some((true, _, _)))
            && (!after_blank || line.starts_with("  ") || line.starts_with('\t'));
        let continues_paragraph = matches!(cur, Some((false, _, _))) && !after_blank;
        if continues_item || continues_paragraph {
            if let Some((_, _, text)) = cur.as_mut() {
                text.push(' ');
                text.push_str(t.trim_start_matches("> "));
            }
        } else {
            flush(&mut cur, &mut out.blocks);
            cur = Some((false, n, t.trim_start_matches("> ").to_string()));
        }
    }
    flush(&mut cur, &mut out.blocks);
    out
}

/// `text` as words read by a person: images dropped, a link replaced by its label, emphasis and
/// backtick marks removed (the words inside a code span are kept).
fn plain(text: &str) -> String {
    let mut s = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '!' && chars.get(i + 1) == Some(&'[') {
            if let Some(close) = find_link_end(&chars, i + 1) {
                i = close + 1;
                continue;
            }
        }
        if c == '[' {
            if let Some(close) = find_link_end(&chars, i) {
                let label_end = chars[i..]
                    .iter()
                    .position(|&x| x == ']')
                    .map_or(i, |p| i + p);
                s.extend(&chars[i + 1..label_end]);
                i = close + 1;
                continue;
            }
        }
        if c == '<'
            && chars
                .get(i + 1)
                .is_some_and(|x| x.is_ascii_alphabetic() || *x == '/')
        {
            if let Some(p) = chars[i..].iter().position(|&x| x == '>') {
                i += p + 1;
                continue;
            }
        }
        if !matches!(c, '`' | '*') {
            s.push(c);
        }
        i += 1;
    }
    s
}

/// Index of the `)` that ends the link whose `[` is at `open`, if there is a `](`...`)` after it.
fn find_link_end(chars: &[char], open: usize) -> Option<usize> {
    let close = open + chars[open..].iter().position(|&x| x == ']')?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    chars[close..]
        .iter()
        .position(|&x| x == ')')
        .map(|p| close + p)
}

/// `text` with every code span replaced by the single word `c`, so that `**bold `code` text**`
/// keeps its `**` pairs and a span is measured in words.
fn mask_code(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_code = false;
    for ch in text.chars() {
        if ch == '`' {
            if in_code {
                out.push('c');
            }
            in_code = !in_code;
        } else if !in_code {
            out.push(ch);
        }
    }
    out
}

fn code_spans(text: &str) -> usize {
    text.matches('`').count() / 2
}

/// The text of each `**...**` span in `text`.
fn bold_spans(text: &str) -> Vec<String> {
    let masked = mask_code(text);
    let mut spans = Vec::new();
    let mut rest = masked.as_str();
    while let Some(i) = rest.find("**") {
        let after = &rest[i + 2..];
        let Some(j) = after.find("**") else { break };
        let span = after[..j].trim();
        if !span.is_empty() {
            spans.push(plain(span));
        }
        rest = &after[j + 2..];
    }
    spans
}

fn words(text: &str) -> usize {
    plain(text).split_whitespace().count()
}

/// Lengths in words of the sentences of `text`. A sentence ends at `.`, `!` or `?` followed by a
/// space and a capital, a digit, or an opening mark, so `e.g. the`, `v0.2.1` and `main.rs` do not
/// end one.
fn sentence_lengths(text: &str) -> Vec<usize> {
    let plain = plain(text);
    let chars: Vec<char> = plain.chars().collect();
    let mut out = Vec::new();
    let mut start = 0;
    for i in 0..chars.len() {
        if !matches!(chars[i], '.' | '!' | '?') {
            continue;
        }
        let mut j = i + 1;
        while j < chars.len() && matches!(chars[j], ')' | '"' | '\u{201d}') {
            j += 1;
        }
        if chars.get(j) != Some(&' ') {
            continue;
        }
        let next = chars[j..].iter().find(|c| **c != ' ');
        if next.is_some_and(|c| c.is_uppercase() || c.is_ascii_digit() || matches!(c, '(' | '"')) {
            let n = chars[start..=i]
                .iter()
                .collect::<String>()
                .split_whitespace()
                .count();
            if n > 0 {
                out.push(n);
            }
            start = j;
        }
    }
    let n = chars[start..]
        .iter()
        .collect::<String>()
        .split_whitespace()
        .count();
    if n > 0 {
        out.push(n);
    }
    out
}

/// True if `text` holds `name.ext:NUMBER` for a source-file extension.
fn has_file_line(text: &str) -> bool {
    for ext in [".rs:", ".js:", ".mjs:", ".md:", ".toml:", ".sh:", ".yml:"] {
        let mut rest = text;
        while let Some(i) = rest.find(ext) {
            let after = &rest[i + ext.len()..];
            if after.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return true;
            }
            rest = after;
        }
    }
    false
}

fn finding(rule: &'static str, page: &str, line: usize, value: f64, text: String) -> Finding {
    Finding {
        rule,
        page: page.to_string(),
        line,
        value,
        shown: fmt_value(value),
        text,
    }
}

fn shown(mut f: Finding, shown: String) -> Finding {
    f.shown = shown;
    f
}

/// The findings for one page. `rel` is its path under the content folder and `text` the whole file.
pub fn page_findings(rel: &str, text: &str) -> Vec<Finding> {
    let kind = kind_of(rel);
    if kind == Kind::Exempt {
        return Vec::new();
    }
    let Some((_, body)) = split_front_matter(text) else {
        return Vec::new();
    };
    let front_lines = text[..text.len() - body.len()].matches('\n').count();
    let page = rel;
    let hand = kind == Kind::Hand;
    let at = |n: usize| front_lines + n;
    let scanned = scan(body);
    let mut out = Vec::new();

    let mut total_words = 0;
    let mut all_text = String::new();
    let mut spans: Vec<(usize, Vec<String>)> = Vec::new();
    let mut sentences: Vec<(usize, usize)> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    let (mut h2, mut h3) = (0, 0);
    let mut first_paragraph: Option<(usize, &str)> = None;
    for block in &scanned.blocks {
        match block {
            Block::Heading { level, .. } => {
                h2 += usize::from(*level == 2);
                h3 += usize::from(*level == 3);
            }
            Block::Paragraph { line, text } | Block::Item { line, text } => {
                let n = words(text);
                total_words += n;
                all_text.push_str(text);
                all_text.push('\n');
                let item = matches!(block, Block::Item { .. });
                let limit = match (item, hand) {
                    (false, true) => 100,
                    (false, false) => 150,
                    (true, true) => 60,
                    (true, false) => 100,
                };
                if n > limit {
                    let what = if item { "list item" } else { "paragraph" };
                    let rule = if item { "W2" } else { "W1" };
                    out.push(finding(
                        rule,
                        page,
                        at(*line),
                        n as f64,
                        format!("{what} of {n} words (limit {limit})"),
                    ));
                }
                for s in sentence_lengths(text) {
                    sentences.push((at(*line), s));
                }
                let found = bold_spans(text);
                if !found.is_empty() {
                    spans.push((at(*line), found));
                }
                if n >= 8 {
                    let key = plain(text)
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .to_lowercase();
                    if let Some(first) = seen.insert(key, at(*line)) {
                        out.push(finding(
                            "W14",
                            page,
                            at(*line),
                            n as f64,
                            format!("same text as line {first} ({n} words)"),
                        ));
                    }
                }
                if !item && first_paragraph.is_none() {
                    first_paragraph = Some((*line, text));
                }
                if hand && has_file_line(text) {
                    out.push(finding(
                        "W13",
                        page,
                        at(*line),
                        1.0,
                        "`file:line` reference in prose; move it to Sources".to_string(),
                    ));
                }
            }
            Block::Cell { line, text } => {
                let n = words(text);
                total_words += n;
                all_text.push_str(text);
                all_text.push('\n');
                let chars = plain(text).chars().count();
                if chars > 350 {
                    out.push(finding(
                        "W16",
                        page,
                        at(*line),
                        chars as f64,
                        format!("table cell of {chars} characters (limit 350)"),
                    ));
                }
                let found = bold_spans(text);
                if !found.is_empty() {
                    spans.push((at(*line), found));
                }
            }
        }
    }

    // W3: sentences over 45 words, and a page where more than a tenth run over 25.
    let long: Vec<&(usize, usize)> = sentences.iter().filter(|(_, n)| *n > 45).collect();
    if let Some((line, n)) = long.iter().max_by_key(|(_, n)| *n).map(|x| **x) {
        out.push(finding(
            "W3",
            page,
            line,
            n as f64,
            format!(
                "{} sentence(s) over 45 words, the longest has {n}",
                long.len()
            ),
        ));
    }
    let over25 = sentences.iter().filter(|(_, n)| *n > 25).count();
    if !sentences.is_empty() && over25 * 10 > sentences.len() {
        let pct = over25 as f64 / sentences.len() as f64 * 100.0;
        out.push(shown(
            finding(
                "W3",
                page,
                0,
                pct / 2.0,
                format!(
                    "{over25} of {} sentences are over 25 words",
                    sentences.len()
                ),
            ),
            format!("{pct:.0}% long"),
        ));
    }

    // Bold: W5 to W8.
    let all_spans: Vec<&String> = spans.iter().flat_map(|(_, s)| s.iter()).collect();
    let bold = all_spans.len();
    if total_words >= 300 && bold == 0 {
        out.push(finding(
            "W5",
            page,
            0,
            total_words as f64,
            format!("{total_words} words and no bold"),
        ));
    }
    let per_1000 = bold as f64 * 1000.0 / total_words.max(1) as f64;
    if hand && total_words > 400 && !(4.0..=25.0).contains(&per_1000) {
        // A page with no bold is W5's; W6 reports a thin or crowded page that has some.
        if bold > 0 {
            out.push(shown(
                finding(
                    "W6",
                    page,
                    0,
                    if per_1000 < 4.0 {
                        4.0 - per_1000
                    } else {
                        per_1000 - 25.0
                    },
                    format!("{bold} bold spans in {total_words} words ({per_1000:.1} per 1000)"),
                ),
                format!("{per_1000:.1}/1000"),
            ));
        }
    }
    let labels = all_spans
        .iter()
        .filter(|s| s.split_whitespace().count() <= 2)
        .count();
    if bold > 8 && labels * 2 > bold {
        out.push(finding(
            "W7",
            page,
            0,
            labels as f64,
            format!("{labels} of {bold} bold spans are one or two words"),
        ));
    }
    let long_spans = all_spans
        .iter()
        .filter(|s| s.split_whitespace().count() > 12)
        .count();
    let crowded = spans.iter().filter(|(_, s)| s.len() >= 2).count();
    if long_spans > 0 || crowded > 3 {
        out.push(finding(
            "W8",
            page,
            0,
            (long_spans + crowded) as f64,
            format!(
                "{long_spans} bold span(s) over 12 words, {crowded} paragraph(s) with 2 or more"
            ),
        ));
    }

    // W9: code spans per 100 words.
    let codes = code_spans(&all_text);
    let density = codes as f64 * 100.0 / total_words.max(1) as f64;
    let limit = if hand { 10.0 } else { 12.0 };
    if total_words >= 100 && density > limit {
        out.push(finding(
            "W9",
            page,
            0,
            density,
            format!(
                "{codes} code spans in {total_words} words ({density:.1} per 100, limit {limit})"
            ),
        ));
    }

    // W10: structure.
    let headings = (h2 + h3) as f64 * 1000.0 / total_words.max(1) as f64;
    if total_words > 800 && headings < 3.0 {
        out.push(shown(
            finding(
                "W10",
                page,
                0,
                3.0 - headings,
                format!(
                    "{} headings in {total_words} words ({headings:.1} per 1000)",
                    h2 + h3
                ),
            ),
            format!("{headings:.1}/1000"),
        ));
    }
    if h3 > 20 {
        out.push(shown(
            finding("W10", page, 0, h3 as f64, format!("{h3} h3 headings")),
            format!("{h3} h3"),
        ));
    }

    // W11: length.
    let max_words = if hand { 2500 } else { 6000 };
    if total_words > max_words {
        out.push(finding(
            "W11",
            page,
            0,
            total_words as f64,
            format!("{total_words} words (limit {max_words})"),
        ));
    }

    // W12: the Sources list against the page.
    let share = scanned.sources_words as f64 * 100.0 / total_words.max(1) as f64;
    if share > 12.0 || scanned.sources_items > 8 {
        out.push(shown(
            finding(
                "W12",
                page,
                0,
                share.max(scanned.sources_items as f64),
                format!(
                    "Sources has {} items and {} words ({share:.1}% of the page)",
                    scanned.sources_items, scanned.sources_words
                ),
            ),
            format!("{} items, {share:.1}%", scanned.sources_items),
        ));
    }

    // W14: a second Status line.
    if scanned.status_lines.len() > 1 {
        out.push(finding(
            "W14",
            page,
            at(scanned.status_lines[1]),
            scanned.status_lines.len() as f64,
            format!("{} lines start with Status", scanned.status_lines.len()),
        ));
    }

    // W15: a mirrored page opens with its Status line.
    if kind == Kind::Project {
        let ok = first_paragraph.is_some_and(|(_, t)| t.starts_with("**Status:**"));
        if !ok {
            out.push(finding(
                "W15",
                page,
                first_paragraph.map_or(0, |(l, _)| at(l)),
                1.0,
                "the first paragraph is not a `**Status:**` line".to_string(),
            ));
        }
    }
    out
}

/// The findings of every page under `docs` (the content folder).
pub fn warnings(docs: &Path) -> Result<Vec<Finding>, String> {
    let mut files = Vec::new();
    markdown_files(docs, &mut files)
        .map_err(|e| format!("cannot read {}: {}", docs.display(), e))?;
    files.sort();
    let mut out = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(docs)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        let text = fs::read_to_string(&file)
            .map_err(|e| format!("cannot read {}: {}", file.display(), e))?;
        out.extend(page_findings(&rel, &text));
    }
    Ok(out)
}

/// The summary lines for `findings`: for each rule that found something, its name, how many
/// findings on how many pages, and the five pages with the worst value. With `full`, each finding
/// follows on its own line.
pub fn summary_lines(findings: &[Finding], full: bool) -> Vec<String> {
    let mut lines = Vec::new();
    for (rule, what) in RULES {
        let mine: Vec<&Finding> = findings.iter().filter(|f| f.rule == rule).collect();
        if mine.is_empty() {
            continue;
        }
        // The worst finding of each page decides its place and the number printed.
        let mut worst: BTreeMap<&str, &Finding> = BTreeMap::new();
        for f in &mine {
            let w = worst.entry(f.page.as_str()).or_insert(f);
            if f.value > w.value {
                *w = f;
            }
        }
        let mut ranked: Vec<(&str, &Finding)> = worst.into_iter().collect();
        ranked.sort_by(|a, b| {
            b.1.value
                .partial_cmp(&a.1.value)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.cmp(b.0))
        });
        let top: Vec<String> = ranked
            .iter()
            .take(5)
            .map(|(p, f)| format!("{p} ({})", f.shown))
            .collect();
        lines.push(format!(
            "    warning {rule}: {what}: {} on {} pages; worst: {}",
            mine.len(),
            ranked.len(),
            top.join(", ")
        ));
        if full {
            for f in mine {
                let place = if f.line > 0 {
                    format!("{}:{}", f.page, f.line)
                } else {
                    f.page.clone()
                };
                lines.push(format!("        {place}: {}", f.text));
            }
        }
    }
    lines
}

fn fmt_value(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(body: &str) -> String {
        format!("---\ntitle: \"T\"\n---\n\n{body}\n")
    }

    fn rules(rel: &str, body: &str) -> Vec<&'static str> {
        page_findings(rel, &page(body))
            .iter()
            .map(|f| f.rule)
            .collect()
    }

    fn sentence(words: usize) -> String {
        format!("Word{}.", " word".repeat(words - 1))
    }

    /// A table's separator row is not text: a table of one 300-word cell has 300 words and its
    /// `| :-- |` row adds none and is not a paragraph.
    #[test]
    fn test_table_separator_row_is_not_a_paragraph() {
        let scanned = scan("| A | B |\n| :-- | --: |\n| x | y |\n");
        let cells: Vec<&str> = scanned
            .blocks
            .iter()
            .map(|b| match b {
                Block::Cell { text, .. } => text.as_str(),
                other => panic!("not a cell: {other:?}"),
            })
            .collect();
        assert_eq!(cells, vec!["A", "B", "x", "y"]);
    }

    /// A paragraph of 101 words warns on a hand page and not at 100; the same 101 are fine on a
    /// project page (limit 150). The finding names the file line of the paragraph's first line.
    #[test]
    fn test_w1_paragraph_limits_differ_by_kind() {
        let p = |n: usize| format!("Intro **bold** here.\n\n{}\n", vec!["x. "; n].concat());
        // 101 one-word sentences: W3 stays quiet at 25 words per sentence.
        assert!(rules("a.md", &p(101)).contains(&"W1"));
        assert!(!rules("a.md", &p(100)).contains(&"W1"));
        assert!(!rules("project/a.md", &p(101)).contains(&"W1"));
        assert!(rules("project/a.md", &p(151)).contains(&"W1"));
        let found = page_findings("a.md", &page(&p(101)));
        let w1 = found.iter().find(|f| f.rule == "W1").unwrap();
        assert_eq!((w1.line, w1.value as usize), (7, 101), "{w1:?}");
    }

    /// A list item of 61 words warns on a hand page; continuation lines belong to the item; a
    /// table row is not an item.
    #[test]
    fn test_w2_list_item_with_continuation_lines() {
        let body = format!(
            "- short\n- {}\n  {}\n",
            vec!["a"; 30].join(" "),
            sentence(31)
        );
        assert_eq!(
            rules("a.md", &body).iter().filter(|r| **r == "W2").count(),
            1
        );
        let body = format!("- {}\n", vec!["a"; 60].join(" "));
        assert!(!rules("a.md", &body).contains(&"W2"));
    }

    /// One sentence of 46 words warns; `e.g. the`, a version number and a file name do not end a
    /// sentence; a page with 2 of 10 sentences over 25 words warns on the share.
    #[test]
    fn test_w3_sentence_length_and_share() {
        let shorts = "Short one. ".repeat(9);
        assert!(rules("a.md", &format!("{} {shorts}", sentence(46))).contains(&"W3"));
        assert!(!rules("a.md", &format!("{} {shorts}", sentence(45))).contains(&"W3"));
        assert_eq!(
            sentence_lengths("Use e.g. the v0.2.1 build of main.rs now. Next one."),
            vec![8, 2]
        );
        let mut body = String::new();
        for _ in 0..2 {
            body.push_str(&sentence(30));
            body.push(' ');
        }
        for _ in 0..8 {
            body.push_str("Short one. ");
        }
        let found = page_findings("a.md", &page(&body));
        let w3: Vec<_> = found.iter().filter(|f| f.rule == "W3").collect();
        assert_eq!(w3.len(), 1, "{w3:?}");
        assert!(w3[0].text.contains("2 of 10 sentences"), "{w3:?}");
    }

    /// Bold rules: no bold in 300 words is W5; 9 one-word bold labels is W7; a 13-word span is
    /// W8; code inside a bold span counts as one word; W6 is for hand pages only.
    #[test]
    fn test_bold_rules() {
        let filler = vec!["word"; 310].join(" ");
        assert!(rules("a.md", &filler).contains(&"W5"));
        assert!(!rules("a.md", &vec!["word"; 290].join(" ")).contains(&"W5"));
        let labels: String = (0..9)
            .map(|i| format!("- **Cause{i}.** text here\n"))
            .collect();
        assert!(rules("a.md", &labels).contains(&"W7"));
        let long = format!("**{}** and the rest.", vec!["w"; 13].join(" "));
        assert!(rules("a.md", &long).contains(&"W8"));
        let with_code = "**`a b c d e f g h i j k l m n` text**";
        assert!(!rules("a.md", with_code).contains(&"W8"));
        let thin = format!("One **bold** word.\n\n{}", vec!["word"; 500].join(" "));
        assert!(rules("a.md", &thin).contains(&"W6"));
        assert!(!rules("project/a.md", &thin).contains(&"W6"));
    }

    /// 11 code spans in 100 words is over the hand limit (10) and under the project limit (12).
    #[test]
    fn test_w9_code_density() {
        let body = format!("{} {}", "`c` ".repeat(11), vec!["word"; 89].join(" "));
        assert!(rules("a.md", &body).contains(&"W9"));
        assert!(!rules("project/a.md", &body).contains(&"W9"));
    }

    /// Headings: 900 words with one h2 warns (W10); 21 h3 warn; a page of 300 words does not.
    #[test]
    fn test_w10_headings_and_w11_length() {
        let long = format!("## One\n\n{}\n", vec!["word"; 900].join(" "));
        assert!(rules("a.md", &long).contains(&"W10"));
        assert!(!rules("a.md", &vec!["word"; 300].join(" ")).contains(&"W10"));
        let h3s: String = (0..21).map(|i| format!("### H{i}\n\ntext\n\n")).collect();
        assert!(rules("a.md", &h3s).contains(&"W10"));
        let huge = vec!["word"; 2501].join(" ");
        assert!(rules("a.md", &huge).contains(&"W11"));
        assert!(!rules("project/a.md", &huge).contains(&"W11"));
    }

    /// Sources: 9 items warn; the Sources words do not count as page words; `file:line` in prose
    /// warns on a hand page and not inside Sources or on a project page.
    #[test]
    fn test_w12_sources_and_w13_file_line() {
        let items: String = (0..9).map(|i| format!("- `src/f{i}.rs`\n")).collect();
        let body = format!("Some text here.\n\n## Sources\n\n{items}");
        assert!(rules("a.md", &body).contains(&"W12"));
        let body = "See main.rs:12 for it.\n\n## Sources\n\n- `src/main.rs:12`\n";
        let w13: Vec<_> = rules("a.md", body)
            .into_iter()
            .filter(|r| *r == "W13")
            .collect();
        assert_eq!(w13.len(), 1);
        assert!(!rules("project/a.md", body).contains(&"W13"));
    }

    /// Two equal paragraphs of 8 words or more warn; short repeats do not; a second Status line
    /// warns and so does a mirrored page that does not open with Status.
    #[test]
    fn test_w14_duplicates_and_status_and_w15() {
        let dup = "This paragraph says the same thing twice over.\n\nThis paragraph says the same thing twice over.\n";
        assert!(rules("a.md", dup).contains(&"W14"));
        assert!(!rules("a.md", "None.\n\nNone.\n").contains(&"W14"));
        let two = "**Status:** one.\n\nStatus: two.\n";
        assert!(rules("project/a.md", two).contains(&"W14"));
        assert!(!rules("project/a.md", two).contains(&"W15"));
        assert!(rules("project/a.md", "Intro first.\n\n**Status:** late.\n").contains(&"W15"));
        assert!(!rules("project/index.md", "Intro first.\n").contains(&"W15"));
    }

    /// A 351-character cell warns, a 350 one does not; the separator row is not a cell.
    #[test]
    fn test_w16_long_cell() {
        let table = |n: usize| format!("| A | B |\n| :-- | :-- |\n| {} | ok |\n", "x".repeat(n));
        assert!(rules("a.md", &table(351)).contains(&"W16"));
        assert!(!rules("a.md", &table(350)).contains(&"W16"));
    }

    /// Experiment records and the generated CLI reference are not measured; code fences, HTML
    /// comments and the Sources list are not text.
    #[test]
    fn test_exempt_pages_and_ignored_text() {
        let huge = vec!["word"; 3000].join(" ");
        assert!(rules("experiments/6-10-x.md", &huge).is_empty());
        assert!(rules("reference/spawn-at-spawn.md", &huge).is_empty());
        assert!(!rules("experiments/index.md", &huge).is_empty());
        let fenced = format!(
            "Short text here.\n\n```\n{}\n```\n\n<!-- site: x -->\n",
            huge
        );
        assert!(
            rules("a.md", &fenced).is_empty(),
            "{:?}",
            rules("a.md", &fenced)
        );
    }

    /// The summary has one line per rule that found something, with the count, the number of
    /// pages and the five worst pages in order; `full` adds a line per finding.
    #[test]
    fn test_summary_lines_rank_the_five_worst_pages() {
        let mk = |page: &str, v: f64| Finding {
            rule: "W11",
            page: page.to_string(),
            line: 0,
            value: v,
            shown: fmt_value(v),
            text: format!("{v} words"),
        };
        let findings: Vec<Finding> = (1..=7)
            .map(|i| mk(&format!("p{i}.md"), 3000.0 + i as f64))
            .collect();
        let lines = summary_lines(&findings, false);
        assert_eq!(lines.len(), 1);
        assert!(
            lines[0].starts_with("    warning W11: page over 2500 words"),
            "{}",
            lines[0]
        );
        assert!(lines[0].contains(": 7 on 7 pages; worst: p7.md (3007), p6.md (3006), p5.md (3005), p4.md (3004), p3.md (3003)"), "{}", lines[0]);
        assert_eq!(summary_lines(&findings, true).len(), 8);
        assert!(summary_lines(&[], false).is_empty());
    }
}
