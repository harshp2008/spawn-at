//! `check-doc-coverage`: text scan for undocumented items, merged with the compiler lint.

use crate::compiler_docs;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;

fn collect_rs_files(path: &Path, rs_files: &mut Vec<PathBuf>) {
    if path.is_file() {
        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            rs_files.push(path.to_path_buf());
        }
    } else if path.is_dir() {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    collect_rs_files(&p, rs_files);
                } else if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("rs") {
                    rs_files.push(p);
                }
            }
        }
    }
}

fn is_fn_start(line: &str) -> Option<String> {
    let s = line.trim_start();
    // (pub(...) )? (async )? (unsafe )? fn name
    let mut rest = s;
    if rest.starts_with("pub") {
        rest = rest[3..].trim_start();
        if rest.starts_with('(') {
            {
                let idx = rest.find(')')?;
                rest = rest[idx + 1..].trim_start();
            }
        }
    }
    if rest.starts_with("async") {
        rest = rest[5..].trim_start();
    }
    if rest.starts_with("unsafe") {
        rest = rest[6..].trim_start();
    }
    if rest.starts_with("extern") {
        rest = rest[6..].trim_start();
        if rest.starts_with('"') {
            if let Some(idx) = rest[1..].find('"') {
                rest = rest[idx + 2..].trim_start();
            }
        }
    }
    if rest.starts_with("fn ") || rest.starts_with("fn\t") {
        let fn_tail = rest[2..].trim_start();
        let name_end = fn_tail
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(fn_tail.len());
        let name = &fn_tail[..name_end];
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

fn is_pub_item_start(line: &str) -> Option<(&'static str, String)> {
    let s = line.trim_start();
    if !s.starts_with("pub") {
        return None;
    }
    let mut rest = s[3..].trim_start();
    if rest.starts_with('(') {
        {
            let idx = rest.find(')')?;
            rest = rest[idx + 1..].trim_start();
        }
    }

    let keywords = [
        ("struct", "struct"),
        ("enum", "enum"),
        ("trait", "trait"),
        ("type", "type"),
        ("const", "const"),
        ("static", "static"),
    ];

    for (kw, desc) in keywords {
        if let Some(after_kw) = rest.strip_prefix(kw) {
            let after_kw = after_kw.trim_start();
            let name_end = after_kw
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(after_kw.len());
            let name = &after_kw[..name_end];
            if !name.is_empty() {
                return Some((desc, name.to_string()));
            }
        }
    }

    None
}

/// Prints, as information only, how many items under `dirs` lack a `///` doc (text scan only).
/// Never affects the exit code: test functions are commented lightly by design.
fn print_informational_coverage(label: &str, dirs: &[String]) {
    let mut files = Vec::new();
    for d in dirs {
        collect_rs_files(Path::new(d), &mut files);
    }
    files.sort();
    let misses = scan_files(&files);
    let total: usize = misses.values().map(Vec::len).sum();
    println!(
        "\nINFO ({}, informational, not a failure): {} items without a /// doc across {} of {} files",
        label,
        total,
        misses.len(),
        files.len()
    );
}

/// Text-scans `rs_files` and returns, per file with misses, `(line, description)` pairs:
/// a missing `//!` file header and any `fn` / `pub` item without a `///` doc comment.
fn scan_files(rs_files: &[PathBuf]) -> BTreeMap<String, Vec<(usize, String)>> {
    let mut file_misses: BTreeMap<String, Vec<(usize, String)>> = BTreeMap::new();

    for file_path in rs_files {
        let display_path = file_path.to_string_lossy().to_string();
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().collect();
        let mut misses = Vec::new();

        // 1. Check for //! file header
        let mut has_header = false;
        for line in &lines {
            let s = line.trim();
            if s.is_empty() {
                continue;
            }
            if s.starts_with("//!") {
                has_header = true;
                break;
            }
            if !s.starts_with("//") {
                break;
            }
        }

        if !has_header {
            misses.push((1, "missing //! file header doc comment".to_string()));
        }

        // 2. Check items
        for (i, raw_line) in lines.iter().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with("//") || line.starts_with("/*") {
                continue;
            }

            let fn_match = is_fn_start(raw_line);
            let pub_match = is_pub_item_start(raw_line);

            let (item_kind, target_name) = if let Some(name) = fn_match {
                ("fn", name)
            } else if let Some((kind, name)) = pub_match {
                (kind, name)
            } else {
                continue;
            };

            // Look backwards past attributes (#[...]) and blank lines
            let mut j = i;
            let mut has_doc = false;
            let mut inside_attr = false;
            while j > 0 {
                j -= 1;
                let prev = lines[j].trim();
                if prev.is_empty() {
                    continue;
                }
                if prev.ends_with(']') {
                    inside_attr = true;
                }
                if inside_attr {
                    if prev.starts_with("#[") || prev.starts_with("#![") {
                        inside_attr = false;
                    }
                    continue;
                }
                if prev.starts_with("///") {
                    has_doc = true;
                    break;
                }
                break;
            }

            if !has_doc {
                misses.push((
                    i + 1,
                    format!("missing /// doc for {} '{}'", item_kind, target_name),
                ));
            }
        }

        if !misses.is_empty() {
            file_misses.insert(display_path, misses);
        }
    }
    file_misses
}
/// Runs the doc-coverage check over `target_dirs` (text scan plus the compiler lint); exits 1 on
/// any miss. `informational_dirs` get a count only, never a failure.
pub(crate) fn run_check_doc_coverage(target_dirs: &[String], informational_dirs: &[String]) {
    let mut rs_files = Vec::new();
    for d in target_dirs {
        collect_rs_files(Path::new(d), &mut rs_files);
    }
    rs_files.sort();

    let mut file_misses = scan_files(&rs_files);
    let mut total_misses: usize = file_misses.values().map(Vec::len).sum();

    // 3. Merge the compiler's `missing_docs` findings, which cover items the text scan cannot
    // see (struct fields, variant fields). Skipped when no target dir is in spawn-at-verify.
    if target_dirs.iter().any(|d| d.contains("spawn-at-verify")) {
        match compiler_docs::compiler_missing_docs(target_dirs) {
            Ok(found) => {
                for m in found {
                    let entry = file_misses.entry(m.path).or_default();
                    if !entry.iter().any(|(l, _)| *l == m.line) {
                        entry.push((m.line, format!("compiler: {}", m.message)));
                        total_misses += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("{}", e);
                exit(1);
            }
        }
    }

    if total_misses > 0 {
        println!(
            "FAILED: Found {} documentation misses across {} files:\n",
            total_misses,
            file_misses.len()
        );
        for (fp, misses) in &file_misses {
            println!("=== {} ({} misses) ===", fp, misses.len());
            for (line_no, desc) in misses {
                println!("  Line {:4}: {}", line_no, desc);
            }
            println!();
        }
        if !informational_dirs.is_empty() {
            print_informational_coverage("tests/", informational_dirs);
        }
        exit(1);
    } else {
        println!(
            "SUCCESS: ZERO documentation misses across all {} files!",
            rs_files.len()
        );
        if !informational_dirs.is_empty() {
            print_informational_coverage("tests/", informational_dirs);
        }
        exit(0);
    }
}
