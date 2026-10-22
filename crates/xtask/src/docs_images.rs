//! Images of the guide: the checks `docs-check-src` runs on the image references of the pages and
//! the report of what `docs-site/public/img/` holds.
//!
//! A page shows an image with plain Markdown, `![alt text](/spawn-at/img/<file>)`, where `<file>`
//! is a file directly in `docs-site/public/img/` (the site is served under `/spawn-at/`, and
//! Starlight does not rewrite the path, as for links). A reference fails the check when its file
//! does not exist, when its alt text is empty, when it points anywhere but `/spawn-at/img/`
//! (including a URL on another site: the guide loads nothing from elsewhere), or when the name
//! leaves the folder. A placeholder is an SVG that contains the word `Placeholder`; the check does
//! not fail on one, it counts them, and prints the bytes of the whole folder next to the budget so
//! that the size stays in view.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// The path every image reference starts with.
pub const IMG_BASE: &str = "/spawn-at/img/";

/// What `docs-site/public/img/` may weigh in all before the check says so: 5 MB, which is 23 % of
/// the size of the repository's `.git` folder when it was set (22 MB).
pub const IMG_BUDGET_BYTES: u64 = 5 * 1024 * 1024;

/// One `![alt](target)` in a page.
#[derive(Debug, PartialEq, Eq)]
pub struct ImageRef {
    /// Line in the body (1-based, counted like `docs_check`'s other line numbers).
    pub line: usize,
    pub alt: String,
    /// The link target without a title: `/spawn-at/img/x.svg`.
    pub target: String,
}

/// `line` with the text between backticks replaced by spaces, so that the syntax quoted in a code
/// span (as the rules page does) is not read as an image.
fn without_code_spans(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            out.push(' ');
        } else if in_code {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// The images referenced in `prose`, the lines of a page body outside code fences with their
/// numbers (as `docs_check::prose_lines` returns them).
pub fn image_refs(prose: &[(usize, &str)]) -> Vec<ImageRef> {
    let mut found = Vec::new();
    for (number, line) in prose {
        let line = without_code_spans(line);
        let mut rest = line.as_str();
        while let Some(i) = rest.find("![") {
            let after = &rest[i + 2..];
            let Some(j) = after.find("](") else { break };
            let alt = &after[..j];
            let tail = &after[j + 2..];
            let Some(end) = tail.find(')') else { break };
            let target = tail[..end].split_whitespace().next().unwrap_or("");
            found.push(ImageRef {
                line: *number,
                alt: alt.to_string(),
                target: target.to_string(),
            });
            rest = &tail[end..];
        }
    }
    found
}

/// The file name a reference points at, if it is a plain name inside `/spawn-at/img/`.
fn image_name(target: &str) -> Option<&str> {
    let name = target.strip_prefix(IMG_BASE)?;
    let name = name.split(['#', '?']).next().unwrap_or("");
    let plain = !name.is_empty() && !name.contains(['/', '\\']) && name != ".." && name != ".";
    plain.then_some(name)
}

/// Problems with `refs`, as (line, text), checked against the files in `img_dir`.
pub fn image_problems(refs: &[ImageRef], img_dir: &Path) -> Vec<(usize, String)> {
    let mut problems = Vec::new();
    for r in refs {
        if r.alt.trim().is_empty() {
            problems.push((
                r.line,
                format!("image `{}` has no alt text; say what it shows", r.target),
            ));
        }
        if r.target.contains("://") || r.target.starts_with("//") {
            problems.push((
                r.line,
                format!(
                    "image `{}` is on another site; the guide loads nothing from elsewhere",
                    r.target
                ),
            ));
            continue;
        }
        match image_name(&r.target) {
            None => problems.push((
                r.line,
                format!(
                    "image `{}` must be `{IMG_BASE}<file>`, a file directly in docs-site/public/img/",
                    r.target
                ),
            )),
            Some(name) if !img_dir.join(name).is_file() => problems.push((
                r.line,
                format!("image `{name}` does not exist in docs-site/public/img/"),
            )),
            Some(_) => {}
        }
    }
    problems
}

/// The file names of the images in `refs` that point into `/spawn-at/img/`.
pub fn used_names(refs: &[ImageRef]) -> BTreeSet<String> {
    refs.iter()
        .filter_map(|r| image_name(&r.target).map(str::to_string))
        .collect()
}

/// A placeholder image: an SVG that says `Placeholder`.
pub fn is_placeholder(name: &str, content: &str) -> bool {
    name.ends_with(".svg") && content.contains("Placeholder")
}

/// What the folder holds and how much of it the pages use.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ImageReport {
    pub files: usize,
    pub bytes: u64,
    /// Placeholder files in the folder.
    pub placeholders: usize,
    /// Distinct images that pages reference (and that exist).
    pub used: usize,
    /// Of those, how many are placeholders.
    pub used_placeholders: usize,
}

/// The report for `img_dir`, given the names the pages reference. A missing folder is an empty one.
pub fn image_report(img_dir: &Path, used: &BTreeSet<String>) -> Result<ImageReport, String> {
    let mut report = ImageReport::default();
    let Ok(entries) = fs::read_dir(img_dir) else {
        return Ok(report);
    };
    for entry in entries {
        let path = entry
            .map_err(|e| format!("cannot read {}: {}", img_dir.display(), e))?
            .path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let meta =
            fs::metadata(&path).map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
        report.files += 1;
        report.bytes += meta.len();
        let placeholder = name.ends_with(".svg")
            && fs::read_to_string(&path).is_ok_and(|text| is_placeholder(&name, &text));
        if placeholder {
            report.placeholders += 1;
        }
        if used.contains(&name) {
            report.used += 1;
            if placeholder {
                report.used_placeholders += 1;
            }
        }
    }
    Ok(report)
}

/// The summary line `docs-check-src` prints, and a second line when the folder is over budget.
pub fn summary(report: &ImageReport, dir: &Path) -> Vec<String> {
    let mut lines = vec![format!(
        "==> images: {} files, {} in {} (budget {}); {} used on pages, {} of them placeholders; \
         {} placeholder files in all",
        report.files,
        human_bytes(report.bytes),
        dir.display(),
        human_bytes(IMG_BUDGET_BYTES),
        report.used,
        report.used_placeholders,
        report.placeholders
    )];
    if report.bytes > IMG_BUDGET_BYTES {
        lines.push(format!(
            "    warning: {} is over the {} budget for images",
            dir.display(),
            human_bytes(IMG_BUDGET_BYTES)
        ));
    }
    lines
}

fn human_bytes(n: u64) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    } else if n >= 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{n} bytes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("xtask-docs-images-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn lines(text: &str) -> Vec<(usize, &str)> {
        text.lines().enumerate().map(|(i, l)| (i + 1, l)).collect()
    }

    /// An image with alt text and a title is read; the title is not part of the target; two on one
    /// line are both found; the syntax inside a code span and a plain link are not images.
    #[test]
    fn test_image_refs_are_found_and_code_spans_and_links_are_not() {
        let text = "![One](/spawn-at/img/a.svg \"title\") and ![Two](/spawn-at/img/b.webp)\n\
                    quoted: `![alt](/spawn-at/img/c.svg)` and [link](/spawn-at/img/d.svg)\n";
        let refs = image_refs(&lines(text));
        assert_eq!(
            refs,
            vec![
                ImageRef {
                    line: 1,
                    alt: "One".to_string(),
                    target: "/spawn-at/img/a.svg".to_string()
                },
                ImageRef {
                    line: 1,
                    alt: "Two".to_string(),
                    target: "/spawn-at/img/b.webp".to_string()
                },
            ]
        );
    }

    /// An existing file with alt text passes; a missing file, an empty alt text, an address on
    /// another site, a path outside `/spawn-at/img/` and a name that leaves the folder each give
    /// their own problem.
    #[test]
    fn test_image_problems_each_kind() {
        let dir = temp("problems");
        fs::write(dir.join("ok.svg"), "<svg>Placeholder</svg>").unwrap();
        let text = "![Fine](/spawn-at/img/ok.svg)\n\
                    ![Gone](/spawn-at/img/gone.png)\n\
                    ![](/spawn-at/img/ok.svg)\n\
                    ![Far](https://example.org/x.png)\n\
                    ![Proto](//cdn.example/x.png)\n\
                    ![Wrong](/img/ok.svg)\n\
                    ![Up](/spawn-at/img/../x.png)\n\
                    ![Sub](/spawn-at/img/sub/x.png)\n";
        let problems = image_problems(&image_refs(&lines(text)), &dir);
        let all: Vec<String> = problems.iter().map(|(n, t)| format!("{n}: {t}")).collect();
        let joined = all.join("\n");
        assert!(!joined.contains("1: "), "{joined}");
        assert!(
            joined.contains("2: image `gone.png` does not exist"),
            "{joined}"
        );
        assert!(
            joined.contains("3: image `/spawn-at/img/ok.svg` has no alt text"),
            "{joined}"
        );
        assert!(
            joined.contains("4: image `https://example.org/x.png` is on another site"),
            "{joined}"
        );
        assert!(
            joined.contains("5: image `//cdn.example/x.png` is on another site"),
            "{joined}"
        );
        assert!(
            joined.contains("6: image `/img/ok.svg` must be `/spawn-at/img/<file>`"),
            "{joined}"
        );
        assert!(
            joined.contains("7: image `/spawn-at/img/../x.png` must be"),
            "{joined}"
        );
        assert!(
            joined.contains("8: image `/spawn-at/img/sub/x.png` must be"),
            "{joined}"
        );
        assert_eq!(problems.len(), 7, "{joined}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// A placeholder is an SVG that says `Placeholder`; a real SVG and a raster file are not. The
    /// report counts files, bytes, placeholders and what pages use; a missing folder is empty.
    #[test]
    fn test_report_counts_files_bytes_and_placeholders() {
        let dir = temp("report");
        fs::write(dir.join("p1.svg"), "<svg><text>Placeholder: a</text></svg>").unwrap();
        fs::write(dir.join("p2.svg"), "<svg><text>Placeholder: b</text></svg>").unwrap();
        fs::write(dir.join("real.svg"), "<svg><text>Real</text></svg>").unwrap();
        fs::write(
            dir.join("shot.webp"),
            "Placeholder in a binary is not an SVG",
        )
        .unwrap();
        let used: BTreeSet<String> = ["p1.svg", "real.svg", "missing.png"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let report = image_report(&dir, &used).unwrap();
        assert_eq!(report.files, 4);
        assert_eq!(report.placeholders, 2);
        assert_eq!(report.used, 2);
        assert_eq!(report.used_placeholders, 1);
        let expected: u64 = ["p1.svg", "p2.svg", "real.svg", "shot.webp"]
            .iter()
            .map(|f| fs::metadata(dir.join(f)).unwrap().len())
            .sum();
        assert_eq!(report.bytes, expected);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(
            image_report(&dir.join("none"), &BTreeSet::new()).unwrap(),
            ImageReport::default()
        );
    }

    /// The summary names the files, the bytes, the budget and the placeholders; over budget it adds
    /// a warning line.
    #[test]
    fn test_summary_shows_the_budget_and_warns_over_it() {
        let report = ImageReport {
            files: 3,
            bytes: 2048,
            placeholders: 2,
            used: 1,
            used_placeholders: 1,
        };
        let lines = summary(&report, Path::new("docs-site/public/img"));
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("3 files, 2.0 KB in docs-site/public/img (budget 5.0 MB)"));
        assert!(lines[0].contains("1 used on pages, 1 of them placeholders; 2 placeholder files"));
        let big = ImageReport {
            bytes: IMG_BUDGET_BYTES + 1,
            ..Default::default()
        };
        assert_eq!(summary(&big, Path::new("d")).len(), 2);
    }
}
