//! xtask: Internal developer and CI tooling for spawn-at.
//!
//! Provides `check-comment-only` and `check-doc-coverage` without external dependencies.
//! `check-doc-coverage` combines a text scan with the compiler's `missing_docs` lint.

mod compiler_docs;

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

fn main() {
    let mut args = env::args().skip(1);
    let subcommand = match args.next() {
        Some(cmd) => cmd,
        None => {
            eprintln!("Usage: cargo xtask <subcommand> [args...]");
            eprintln!("Subcommands:");
            eprintln!("  check-comment-only <git-rev> [path]");
            eprintln!("  check-doc-coverage [path...]");
            exit(1);
        }
    };

    match subcommand.as_str() {
        "check-comment-only" => {
            let base_commit = match args.next() {
                Some(rev) => rev,
                None => {
                    eprintln!("Usage: cargo xtask check-comment-only <git-rev> [path]");
                    exit(1);
                }
            };
            let target_path = args
                .next()
                .unwrap_or_else(|| "crates/spawn-at-verify".to_string());
            run_check_comment_only(&base_commit, &target_path);
        }
        "check-doc-coverage" => {
            let paths: Vec<String> = args.collect();
            let target_paths = if paths.is_empty() {
                vec![
                    "crates/spawn-at-verify/src".to_string(),
                    "crates/spawn-at-verify/tests".to_string(),
                ]
            } else {
                paths
            };
            run_check_doc_coverage(&target_paths);
        }
        other => {
            eprintln!("Unknown subcommand: {}", other);
            eprintln!("Available subcommands: check-comment-only, check-doc-coverage");
            exit(1);
        }
    }
}

/// Strips comments and doc attributes, normalizing non-comment code lines for whitespace comparison.
fn strip_comments_and_fmt(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_block = 0usize;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let mut clean_line = String::new();
        let mut chars = line.chars().peekable();
        let mut in_str = false;
        let mut escaped = false;

        while let Some(ch) = chars.next() {
            if in_block > 0 {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    in_block -= 1;
                } else if ch == '/' && chars.peek() == Some(&'*') {
                    chars.next();
                    in_block += 1;
                }
                continue;
            }

            if in_str {
                clean_line.push(ch);
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_str = false;
                }
                continue;
            }

            if ch == '"' {
                in_str = true;
                clean_line.push(ch);
            } else if ch == '/' && chars.peek() == Some(&'/') {
                // Line comment
                break;
            } else if ch == '/' && chars.peek() == Some(&'*') {
                chars.next();
                in_block += 1;
            } else {
                clean_line.push(ch);
            }
        }

        let trimmed = clean_line.trim();
        if trimmed == "#![warn(missing_docs)]" {
            continue;
        }
        if !trimmed.is_empty() {
            let normalized = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
            out.push(normalized);
        }
    }

    out
}

/// Generates unified diff lines between two slices of strings.
fn compute_diff(base: &[String], curr: &[String], file_name: &str) -> Vec<String> {
    if base == curr {
        return Vec::new();
    }

    // Myers / dynamic programming LCS
    let n = base.len();
    let m = curr.len();

    // If file is very large, simple difference scan
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in 0..n {
        for j in 0..m {
            if base[i] == curr[j] {
                lcs[i + 1][j + 1] = lcs[i][j] + 1;
            } else {
                lcs[i + 1][j + 1] = lcs[i + 1][j].max(lcs[i][j + 1]);
            }
        }
    }

    let mut diff = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && base[i - 1] == curr[j - 1] {
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || lcs[i][j - 1] >= lcs[i - 1][j]) {
            diff.push(format!("+ {}", curr[j - 1]));
            j -= 1;
        } else if i > 0 && (j == 0 || lcs[i][j - 1] < lcs[i - 1][j]) {
            diff.push(format!("- {}", base[i - 1]));
            i -= 1;
        }
    }

    diff.reverse();
    if !diff.is_empty() {
        let mut header = vec![
            format!("--- base:{}", file_name),
            format!("+++ current:{}", file_name),
        ];
        header.extend(diff);
        header
    } else {
        Vec::new()
    }
}

fn run_check_comment_only(base_commit: &str, target_dir: &str) {
    println!(
        "==> Checking that changes since {} in {} are comment-only...",
        base_commit, target_dir
    );

    let output = Command::new("git")
        .args(["diff", "--name-only", base_commit, "--", target_dir])
        .output()
        .expect("failed to execute git diff");

    if !output.status.success() {
        eprintln!("git diff failed: {:?}", output);
        exit(1);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let changed_files: Vec<&str> = stdout
        .lines()
        .map(|s| s.trim())
        .filter(|s| s.ends_with(".rs"))
        .collect();

    if changed_files.is_empty() {
        println!(
            "No .rs files changed in {} compared to {}.",
            target_dir, base_commit
        );
        exit(0);
    }

    let mut has_failure = false;

    for file_path in changed_files {
        let show_arg = format!("{}:{}", base_commit, file_path);
        let res_base = Command::new("git")
            .args(["show", &show_arg])
            .output()
            .expect("failed to execute git show");

        if !res_base.status.success() {
            println!("Skipping newly created file or git error on {}", file_path);
            continue;
        }

        let base_content = String::from_utf8_lossy(&res_base.stdout);
        let curr_content = if Path::new(file_path).exists() {
            fs::read_to_string(file_path).unwrap_or_default()
        } else {
            let res_curr = Command::new("git")
                .args(["show", &format!("HEAD:{}", file_path)])
                .output()
                .expect("failed to execute git show HEAD");
            String::from_utf8_lossy(&res_curr.stdout).to_string()
        };

        let base_stripped = strip_comments_and_fmt(&base_content);
        let curr_stripped = strip_comments_and_fmt(&curr_content);

        let diff = compute_diff(&base_stripped, &curr_stripped, file_path);

        if !diff.is_empty() {
            println!("FAILED: Non-comment change detected in {}:", file_path);
            for d in diff.iter().take(25) {
                println!("  {}", d);
            }
            if diff.len() > 25 {
                println!("   ... and {} more diff lines", diff.len() - 25);
            }
            has_failure = true;
        } else {
            println!("  OK (comments only): {}", file_path);
        }
    }

    if has_failure {
        exit(1);
    } else {
        println!("ALL checked files contain ONLY comment changes!");
    }
}

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
            if let Some(idx) = rest.find(')') {
                rest = rest[idx + 1..].trim_start();
            } else {
                return None;
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
        if let Some(idx) = rest.find(')') {
            rest = rest[idx + 1..].trim_start();
        } else {
            return None;
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
        if rest.starts_with(kw) {
            let after_kw = rest[kw.len()..].trim_start();
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

fn run_check_doc_coverage(target_dirs: &[String]) {
    let mut rs_files = Vec::new();
    for d in target_dirs {
        collect_rs_files(Path::new(d), &mut rs_files);
    }
    rs_files.sort();

    let mut total_misses = 0usize;
    let mut file_misses: BTreeMap<String, Vec<(usize, String)>> = BTreeMap::new();

    for file_path in &rs_files {
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
            total_misses += misses.len();
            file_misses.insert(display_path, misses);
        }
    }

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
        exit(1);
    } else {
        println!(
            "SUCCESS: ZERO documentation misses across all {} files!",
            rs_files.len()
        );
        exit(0);
    }
}
